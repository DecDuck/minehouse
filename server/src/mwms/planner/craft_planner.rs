use std::{
    collections::BTreeMap,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use azalea_registry::builtin::ItemKind;
use common::{
    item_stack::item_kind_stack_size,
    mwms::transfers::{ItemMove, Transfer},
    vec::Point,
    work::WorkUnit,
    work_units::{
        craft::{CraftLocations, CraftProgress, CraftRecipe, CraftWorkUnit},
        transfer::TransferWorkUnit,
    },
};
use tokio::{sync::Notify, time::MissedTickBehavior};
use tracing::{debug, error};
use uuid::Uuid;

use crate::{
    db::{
        Cube,
        container::Container,
        craft_job::{
            CraftOperationKind, NewCraftTransferKind, NewCraftTransferOperation, ReadyCraftNode,
        },
        crafting_engine::CraftingRegionAssignment,
        storage::ItemKindStackRow,
    },
    state::MinehouseState,
};

const RECONCILE_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Clone, Default)]
pub struct CraftPlannerWake {
    notify: Arc<Notify>,
    recovery_ready: Arc<AtomicBool>,
}

impl CraftPlannerWake {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn notify(&self) {
        self.notify.notify_one();
    }

    pub fn mark_recovery_ready(&self) {
        self.recovery_ready.store(true, Ordering::Release);
        self.notify();
    }

    fn is_recovery_ready(&self) -> bool {
        self.recovery_ready.load(Ordering::Acquire)
    }
}

pub struct CraftPlanner {
    state: Arc<MinehouseState>,
    wake: CraftPlannerWake,
}

impl CraftPlanner {
    pub fn new(state: Arc<MinehouseState>, wake: CraftPlannerWake) -> Self {
        Self { state, wake }
    }

    pub async fn run(self) {
        let mut interval = tokio::time::interval(RECONCILE_INTERVAL);
        interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

        loop {
            tokio::select! {
                _ = interval.tick() => {}
                _ = self.wake.notify.notified() => {}
            }
            if let Err(error) = self.reconcile().await {
                error!(?error, "craft planner reconciliation failed");
            }
        }
    }

    async fn reconcile(&self) -> Result<(), anyhow::Error> {
        if !self.wake.is_recovery_ready() {
            debug!("craft planner waiting for startup inventory reconciliation");
            return Ok(());
        }
        let ready = self.state.db.reconcile_ready_craft_nodes().await?;
        debug!(ready_nodes = ready.len(), "reconciled durable craft plans");
        let mut transfer_active = self.state.db.has_active_craft_transfer_operations().await?;
        if !transfer_active {
            if let Some(node) = self.state.db.fetch_cleanup_craft_nodes().await?.first() {
                transfer_active = self
                    .prepare_cleanup(node.id, node.assigned_region_id)
                    .await?;
            }
        }
        if !transfer_active {
            for node in ready {
                let Some(assignment) = self
                    .state
                    .db
                    .try_lease_crafting_region(node.spec.id, node.recipe.engine_type)
                    .await?
                else {
                    continue;
                };
                if self
                    .prepare_raw_material_staging(&node, &assignment)
                    .await?
                {
                    break;
                }
            }
        }
        for node in self.state.db.fetch_queued_craft_nodes().await? {
            let Some(assignment) = self
                .state
                .db
                .try_lease_crafting_region(node.spec.id, node.recipe.engine_type)
                .await?
            else {
                continue;
            };
            self.dispatch_craft(&node, &assignment).await?;
        }
        self.dispatch_persisted_operations().await?;
        Ok(())
    }

    pub(crate) async fn prepare_raw_material_staging(
        &self,
        node: &ReadyCraftNode,
        assignment: &CraftingRegionAssignment,
    ) -> Result<bool, anyhow::Error> {
        let raw_requirements = self
            .state
            .db
            .fetch_craft_material_requirements(node.spec.id)
            .await?;
        let dependency_requirements = self
            .state
            .db
            .fetch_craft_dependency_requirements(node.spec.id)
            .await?;
        let destination = self
            .state
            .db
            .fetch_container(assignment.container_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("processing container disappeared"))?;
        let mut requirements = raw_requirements
            .into_iter()
            .map(|requirement| StagingRequirement {
                item_kind: requirement.item_kind,
                quantity: requirement.quantity,
                source: StagingSource::Storage,
            })
            .collect::<Vec<_>>();
        let mut stacks = Vec::new();
        for requirement in &requirements {
            stacks.extend(
                self.state
                    .db
                    .fetch_item_kind_stacks(&requirement.item_kind)
                    .await?
                    .into_iter()
                    .filter(|stack| stack.region_type != "processing")
                    .map(|stack| (requirement.item_kind.clone(), StagingSource::Storage, stack)),
            );
        }
        for requirement in dependency_requirements {
            if requirement.region_id == assignment.region_id {
                continue;
            }
            stacks.extend(
                self.state
                    .db
                    .fetch_item_kind_stacks(&requirement.requirement.item_kind)
                    .await?
                    .into_iter()
                    .filter(|stack| stack.region_id == requirement.region_id)
                    .map(|stack| {
                        (
                            requirement.requirement.item_kind.clone(),
                            StagingSource::Dependency(requirement.source_node_id),
                            stack,
                        )
                    }),
            );
            requirements.push(StagingRequirement {
                item_kind: requirement.requirement.item_kind,
                quantity: requirement.requirement.quantity,
                source: StagingSource::Dependency(requirement.source_node_id),
            });
        }
        let Some((work_units, mut staged_capacity)) =
            build_staging_work_units(&requirements, &stacks, &destination)?
        else {
            self.state
                .db
                .release_crafting_region(node.spec.id, assignment.inherited_from)
                .await?;
            return Ok(false);
        };
        let remaining_crafts = node
            .recipe
            .planned_crafts
            .saturating_sub(node.completed_crafts);
        for ingredient in &node.recipe.ingredients {
            if !staged_capacity.consume(
                &ingredient.item_kind.to_string(),
                ingredient.quantity.saturating_mul(remaining_crafts),
            ) {
                self.state
                    .db
                    .release_crafting_region(node.spec.id, assignment.inherited_from)
                    .await?;
                return Ok(false);
            }
        }
        let output_quantity = node.recipe.output_yield.saturating_mul(remaining_crafts);
        if staged_capacity
            .allocate(&node.spec.item_kind, "", output_quantity)
            .is_none()
        {
            self.state
                .db
                .release_crafting_region(node.spec.id, assignment.inherited_from)
                .await?;
            return Ok(false);
        }
        if !self
            .state
            .db
            .register_craft_staging_operations(node.spec.id, &work_units)
            .await?
        {
            return Ok(false);
        }
        Ok(true)
    }

    async fn dispatch_craft(
        &self,
        node: &ReadyCraftNode,
        assignment: &CraftingRegionAssignment,
    ) -> Result<bool, anyhow::Error> {
        let work_unit: WorkUnit = CraftWorkUnit {
            recipe: CraftRecipe {
                recipe_id: node.recipe.recipe_id,
                output_item_kind: ItemKind::from_str(&node.spec.item_kind).map_err(|_| {
                    anyhow::anyhow!("unknown persisted item {}", node.spec.item_kind)
                })?,
                output_yield: node.recipe.output_yield,
                ingredients: node.recipe.ingredients.clone(),
                crafts: node.recipe.planned_crafts,
            },
            locations: CraftLocations {
                engine: cube_point(assignment.engine_position),
                input_container_id: assignment.container_id,
                input_container: cube_point(assignment.container_position),
                output_container_id: assignment.container_id,
                output_container: cube_point(assignment.container_position),
                output_container_capacity: u64::try_from(assignment.container_capacity)?,
            },
            progress: CraftProgress {
                completed_crafts: u32::try_from(node.completed_crafts)?,
                input_snapshot: None,
                output_snapshot: None,
            },
        }
        .into();
        if !self
            .state
            .db
            .register_craft_operation(node.spec.id, &work_unit)
            .await?
        {
            return Ok(false);
        }
        Ok(true)
    }

    async fn prepare_cleanup(&self, node_id: Uuid, region_id: Uuid) -> Result<bool, anyhow::Error> {
        let containers = self.state.db.fetch_all_containers().await?;
        let source = containers
            .iter()
            .find(|container| container.region_id == region_id)
            .ok_or_else(|| anyhow::anyhow!("leased processing region has no container"))?;
        let bulk_region_ids = self
            .state
            .db
            .fetch_all_regions()
            .await?
            .into_iter()
            .filter(|region| region.r#type == crate::db::container_region::RegionType::Bulk)
            .map(|region| region.id)
            .collect::<std::collections::HashSet<_>>();
        let destinations = containers
            .iter()
            .filter(|container| bulk_region_ids.contains(&container.region_id))
            .collect::<Vec<_>>();
        let Some(operations) = build_cleanup_work_units(source, &destinations)? else {
            return Ok(false);
        };
        if !self
            .state
            .db
            .register_cleanup_operations(node_id, &operations)
            .await?
        {
            return Ok(false);
        }
        Ok(true)
    }

    async fn dispatch_persisted_operations(&self) -> Result<(), anyhow::Error> {
        for operation in self.state.db.fetch_dispatchable_craft_operations().await? {
            if self.state.pool.contains_work_unit(&operation.work_unit.id) {
                continue;
            }
            let priority = match operation.kind {
                CraftOperationKind::CleanupTransfer => 30,
                CraftOperationKind::StageTransfer | CraftOperationKind::DependencyTransfer => 20,
                CraftOperationKind::Craft => 10,
            };
            self.state
                .pool
                .queue_work_unit(operation.work_unit, Some(priority));
        }
        Ok(())
    }
}

#[derive(Debug)]
struct StagingRequirement {
    item_kind: String,
    quantity: u32,
    source: StagingSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum StagingSource {
    Storage,
    Dependency(Uuid),
}

#[derive(Debug, Clone)]
struct VirtualStack {
    item_kind: String,
    components_digest: String,
    quantity: u32,
    max_quantity: u32,
}

#[derive(Debug, Clone)]
struct ContainerCapacity {
    capacity: usize,
    slots: BTreeMap<usize, VirtualStack>,
}

impl ContainerCapacity {
    fn new(container: &Container) -> Result<Self, anyhow::Error> {
        let slots = container
            .contents
            .iter()
            .map(|(slot, stack)| {
                Ok((
                    *slot,
                    VirtualStack {
                        item_kind: stack.item_kind.clone(),
                        components_digest: stack.components_digest.clone(),
                        quantity: u32::try_from(stack.quantity)?,
                        max_quantity: item_kind_stack_size(&stack.item_kind),
                    },
                ))
            })
            .collect::<Result<_, anyhow::Error>>()?;
        Ok(Self {
            capacity: usize::try_from(container.capacity)?,
            slots,
        })
    }

    fn allocate(
        &mut self,
        item_kind: &str,
        components_digest: &str,
        quantity: u32,
    ) -> Option<Vec<(usize, u32)>> {
        let mut next = self.clone();
        let mut remaining = quantity;
        let mut placements = Vec::new();
        for (slot, stack) in &mut next.slots {
            if stack.item_kind != item_kind
                || stack.components_digest != components_digest
                || stack.quantity >= stack.max_quantity
            {
                continue;
            }
            let placed = remaining.min(stack.max_quantity - stack.quantity);
            stack.quantity += placed;
            remaining -= placed;
            placements.push((*slot, placed));
            if remaining == 0 {
                *self = next;
                return Some(placements);
            }
        }
        let max_quantity = item_kind_stack_size(item_kind);
        for slot in 0..next.capacity {
            if next.slots.contains_key(&slot) {
                continue;
            }
            let placed = remaining.min(max_quantity);
            next.slots.insert(
                slot,
                VirtualStack {
                    item_kind: item_kind.to_owned(),
                    components_digest: components_digest.to_owned(),
                    quantity: placed,
                    max_quantity,
                },
            );
            remaining -= placed;
            placements.push((slot, placed));
            if remaining == 0 {
                *self = next;
                return Some(placements);
            }
        }
        None
    }

    fn available(&self, item_kind: &str, components_digest: &str) -> u32 {
        let max_quantity = item_kind_stack_size(item_kind);
        let merge_capacity = self
            .slots
            .values()
            .filter(|stack| {
                stack.item_kind == item_kind && stack.components_digest == components_digest
            })
            .map(|stack| stack.max_quantity.saturating_sub(stack.quantity))
            .sum::<u32>();
        let empty_slots = self.capacity.saturating_sub(self.slots.len());
        merge_capacity.saturating_add(
            u32::try_from(empty_slots)
                .unwrap_or(u32::MAX)
                .saturating_mul(max_quantity),
        )
    }

    fn consume(&mut self, item_kind: &str, quantity: u32) -> bool {
        let mut next = self.clone();
        let mut remaining = quantity;
        let slots = next.slots.keys().copied().collect::<Vec<_>>();
        for slot in slots {
            let Some(stack) = next.slots.get_mut(&slot) else {
                continue;
            };
            if stack.item_kind != item_kind {
                continue;
            }
            let consumed = remaining.min(stack.quantity);
            stack.quantity -= consumed;
            remaining -= consumed;
            if stack.quantity == 0 {
                next.slots.remove(&slot);
            }
            if remaining == 0 {
                *self = next;
                return true;
            }
        }
        false
    }
}

fn build_staging_work_units(
    requirements: &[StagingRequirement],
    stacks: &[(String, StagingSource, ItemKindStackRow)],
    destination: &Container,
) -> Result<Option<(Vec<NewCraftTransferOperation>, ContainerCapacity)>, anyhow::Error> {
    let mut capacity = ContainerCapacity::new(destination)?;
    let mut by_source = BTreeMap::<(StagingSource, Uuid), (Point, Vec<ItemMove>)>::new();

    for requirement in requirements {
        let mut remaining = requirement.quantity;
        for (item_kind, source, stack) in stacks.iter().filter(|(item_kind, source, stack)| {
            *item_kind == requirement.item_kind
                && *source == requirement.source
                && stack.container_id != destination.id
                && stack.quantity > 0
        }) {
            if remaining == 0 {
                break;
            }
            let quantity = remaining.min(u32::try_from(stack.quantity)?);
            let Some(placements) = capacity.allocate(item_kind, &stack.components_digest, quantity)
            else {
                return Ok(None);
            };
            let from_slot = usize::try_from(stack.slot)?;
            for (to_slot, placed) in placements {
                by_source
                    .entry((*source, stack.container_id))
                    .or_insert_with(|| (cube_point(stack.position), Vec::new()))
                    .1
                    .push(ItemMove {
                        from_slot,
                        to_slot,
                        quantity: placed,
                    });
            }
            remaining -= quantity;
        }
        if remaining != 0 {
            return Ok(None);
        }
    }

    Ok(Some((
        by_source
            .into_iter()
            .map(
                |((source, from_container), (from_position, moves))| NewCraftTransferOperation {
                    kind: match source {
                        StagingSource::Storage => NewCraftTransferKind::Stage,
                        StagingSource::Dependency(source_node_id) => {
                            NewCraftTransferKind::Dependency { source_node_id }
                        }
                    },
                    work_unit: TransferWorkUnit {
                        transfer: Transfer {
                            from_container,
                            from_position,
                            to_container: destination.id,
                            to_position: cube_point(destination.position),
                            moves,
                        },
                        completed: Vec::new(),
                        staged: Vec::new(),
                        from_contents: None,
                        to_contents: None,
                    }
                    .into(),
                },
            )
            .collect(),
        capacity,
    )))
}

fn cube_point(cube: Cube) -> Point {
    Point {
        x: cube.x1,
        y: cube.y1,
        z: cube.z1,
    }
}

fn build_cleanup_work_units(
    source: &Container,
    destinations: &[&Container],
) -> Result<Option<Vec<NewCraftTransferOperation>>, anyhow::Error> {
    let mut capacities = destinations
        .iter()
        .map(|destination| Ok((destination.id, ContainerCapacity::new(destination)?)))
        .collect::<Result<BTreeMap<_, _>, anyhow::Error>>()?;
    let mut by_destination = BTreeMap::<Uuid, (Point, Vec<ItemMove>)>::new();

    let mut source_stacks = source.contents.values().collect::<Vec<_>>();
    source_stacks.sort_by_key(|stack| stack.slot);
    for stack in source_stacks {
        let mut remaining = u32::try_from(stack.quantity)?;
        for destination in destinations {
            if remaining == 0 {
                break;
            }
            let Some(capacity) = capacities.get_mut(&destination.id) else {
                continue;
            };
            let attempt =
                remaining.min(capacity.available(&stack.item_kind, &stack.components_digest));
            if attempt == 0 {
                continue;
            }
            let Some(placements) =
                capacity.allocate(&stack.item_kind, &stack.components_digest, attempt)
            else {
                continue;
            };
            for (to_slot, placed) in placements {
                by_destination
                    .entry(destination.id)
                    .or_insert_with(|| (cube_point(destination.position), Vec::new()))
                    .1
                    .push(ItemMove {
                        from_slot: usize::try_from(stack.slot)?,
                        to_slot,
                        quantity: placed,
                    });
                remaining -= placed;
            }
        }
        if remaining != 0 {
            return Ok(None);
        }
    }

    Ok(Some(
        by_destination
            .into_iter()
            .map(
                |(to_container, (to_position, moves))| NewCraftTransferOperation {
                    kind: NewCraftTransferKind::Cleanup,
                    work_unit: TransferWorkUnit {
                        transfer: Transfer {
                            from_container: source.id,
                            from_position: cube_point(source.position),
                            to_container,
                            to_position,
                            moves,
                        },
                        completed: Vec::new(),
                        staged: Vec::new(),
                        from_contents: None,
                        to_contents: None,
                    }
                    .into(),
                },
            )
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use common::item_stack::ItemStack;
    use common::work::WorkUnitData;
    use serde_json::json;

    use super::*;

    fn cube(x: f64) -> Cube {
        Cube {
            x1: x,
            y1: 0.0,
            z1: 0.0,
            x2: x,
            y2: 0.0,
            z2: 0.0,
        }
    }

    fn stack(container_id: Uuid, quantity: i32, slot: i32) -> ItemKindStackRow {
        ItemKindStackRow {
            stack_id: Uuid::new_v4(),
            container_id,
            region_id: Uuid::new_v4(),
            region_type: "bulk".to_owned(),
            position: cube(slot as f64),
            slot,
            quantity,
            components: json!({}),
            components_digest: String::new(),
        }
    }

    fn destination(capacity: u64) -> Container {
        Container {
            id: Uuid::new_v4(),
            region_id: Uuid::new_v4(),
            position: cube(100.0),
            capacity,
            contents: HashMap::new(),
        }
    }

    fn stored_item(container_id: Uuid, slot: i32, quantity: i32) -> ItemStack {
        ItemStack {
            id: Uuid::new_v4(),
            container_id,
            item_kind: "widget".to_owned(),
            slot,
            components: json!({}),
            quantity,
            components_digest: String::new(),
        }
    }

    fn stored_kind(container_id: Uuid, item_kind: &str, slot: i32, quantity: i32) -> ItemStack {
        ItemStack {
            item_kind: item_kind.to_owned(),
            ..stored_item(container_id, slot, quantity)
        }
    }

    #[test]
    fn staging_groups_moves_by_source_container() {
        let source = Uuid::new_v4();
        let stacks = vec![
            (
                "iron".to_owned(),
                StagingSource::Storage,
                stack(source, 4, 0),
            ),
            (
                "iron".to_owned(),
                StagingSource::Storage,
                stack(source, 4, 1),
            ),
        ];
        let requirements = vec![StagingRequirement {
            item_kind: "iron".to_owned(),
            quantity: 6,
            source: StagingSource::Storage,
        }];

        let (work, _) = build_staging_work_units(&requirements, &stacks, &destination(4))
            .unwrap()
            .unwrap();

        assert_eq!(work.len(), 1);
        let WorkUnitData::Transfer(transfer) = &work[0].work_unit.data else {
            panic!("expected transfer work unit")
        };
        assert_eq!(transfer.transfer.moves.len(), 2);
        assert_eq!(
            transfer
                .transfer
                .moves
                .iter()
                .map(|item_move| item_move.quantity)
                .sum::<u32>(),
            6
        );
        assert_eq!(
            transfer.transfer.moves[0].to_slot,
            transfer.transfer.moves[1].to_slot
        );
    }

    #[test]
    fn staging_waits_when_materials_or_slots_are_insufficient() {
        let source = Uuid::new_v4();
        let requirements = vec![StagingRequirement {
            item_kind: "iron".to_owned(),
            quantity: 6,
            source: StagingSource::Storage,
        }];
        let stacks = vec![(
            "iron".to_owned(),
            StagingSource::Storage,
            stack(source, 5, 0),
        )];
        assert!(
            build_staging_work_units(&requirements, &stacks, &destination(4))
                .unwrap()
                .is_none()
        );

        let requirements = vec![StagingRequirement {
            item_kind: "iron".to_owned(),
            quantity: 65,
            source: StagingSource::Storage,
        }];
        let stacks = vec![
            (
                "iron".to_owned(),
                StagingSource::Storage,
                stack(source, 32, 0),
            ),
            (
                "iron".to_owned(),
                StagingSource::Storage,
                stack(source, 33, 1),
            ),
        ];
        assert!(
            build_staging_work_units(&requirements, &stacks, &destination(1))
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn staging_merges_partial_stacks_and_splits_at_stack_limits() {
        let source = Uuid::new_v4();
        let mut merge_destination = destination(1);
        merge_destination.contents.insert(
            0,
            stored_kind(merge_destination.id, "minecraft:iron_ingot", 0, 60),
        );
        let requirements = vec![StagingRequirement {
            item_kind: "minecraft:iron_ingot".to_owned(),
            quantity: 4,
            source: StagingSource::Storage,
        }];
        let stacks = vec![(
            "minecraft:iron_ingot".to_owned(),
            StagingSource::Storage,
            stack(source, 4, 0),
        )];
        let (operations, _) = build_staging_work_units(&requirements, &stacks, &merge_destination)
            .unwrap()
            .unwrap();
        let WorkUnitData::Transfer(transfer) = &operations[0].work_unit.data else {
            panic!("expected transfer")
        };
        assert_eq!(transfer.transfer.moves[0].to_slot, 0);

        let requirements = vec![StagingRequirement {
            item_kind: "minecraft:iron_ingot".to_owned(),
            quantity: 65,
            source: StagingSource::Storage,
        }];
        let stacks = vec![(
            "minecraft:iron_ingot".to_owned(),
            StagingSource::Storage,
            stack(source, 65, 0),
        )];
        let (operations, _) = build_staging_work_units(&requirements, &stacks, &destination(2))
            .unwrap()
            .unwrap();
        let WorkUnitData::Transfer(transfer) = &operations[0].work_unit.data else {
            panic!("expected transfer")
        };
        assert_eq!(transfer.transfer.moves.len(), 2);
        assert_eq!(transfer.transfer.moves[0].quantity, 64);
        assert_eq!(transfer.transfer.moves[1].quantity, 1);
    }

    #[test]
    fn capacity_simulation_accounts_for_consumed_inputs_and_output_stack_size() {
        let mut container = destination(2);
        container
            .contents
            .insert(0, stored_kind(container.id, "minecraft:iron_ingot", 0, 64));
        let mut capacity = ContainerCapacity::new(&container).unwrap();
        assert!(capacity.consume("minecraft:iron_ingot", 64));
        assert!(capacity.allocate("minecraft:hopper", "", 64).is_some());

        let mut full = destination(1);
        full.contents
            .insert(0, stored_kind(full.id, "minecraft:iron_ingot", 0, 1));
        let mut capacity = ContainerCapacity::new(&full).unwrap();
        assert!(
            capacity
                .allocate("minecraft:diamond_sword", "", 1)
                .is_none()
        );
    }

    #[test]
    fn dependency_staging_does_not_mix_equal_items_from_other_nodes() {
        let first_node = Uuid::new_v4();
        let second_node = Uuid::new_v4();
        let first_source = Uuid::new_v4();
        let second_source = Uuid::new_v4();
        let requirements = vec![StagingRequirement {
            item_kind: "iron".to_owned(),
            quantity: 4,
            source: StagingSource::Dependency(first_node),
        }];
        let stacks = vec![
            (
                "iron".to_owned(),
                StagingSource::Dependency(first_node),
                stack(first_source, 4, 0),
            ),
            (
                "iron".to_owned(),
                StagingSource::Dependency(second_node),
                stack(second_source, 64, 0),
            ),
        ];

        let (operations, _) = build_staging_work_units(&requirements, &stacks, &destination(4))
            .unwrap()
            .unwrap();

        assert_eq!(operations.len(), 1);
        assert!(matches!(
            operations[0].kind,
            NewCraftTransferKind::Dependency { source_node_id } if source_node_id == first_node
        ));
        let WorkUnitData::Transfer(transfer) = &operations[0].work_unit.data else {
            panic!("expected transfer work unit")
        };
        assert_eq!(transfer.transfer.from_container, first_source);
    }

    #[test]
    fn cleanup_spreads_stacks_across_available_bulk_slots() {
        let mut source = destination(2);
        source.contents.insert(0, stored_item(source.id, 0, 64));
        source.contents.insert(1, stored_item(source.id, 1, 1));
        let first = destination(1);
        let second = destination(1);

        let operations = build_cleanup_work_units(&source, &[&first, &second])
            .unwrap()
            .unwrap();

        assert_eq!(operations.len(), 2);
        assert!(
            operations
                .iter()
                .all(|operation| matches!(operation.kind, NewCraftTransferKind::Cleanup))
        );
        assert!(
            build_cleanup_work_units(&source, &[&first])
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn cleanup_merges_into_partial_bulk_stacks_before_using_empty_slots() {
        let mut source = destination(1);
        source
            .contents
            .insert(0, stored_kind(source.id, "minecraft:iron_ingot", 0, 4));
        let mut bulk = destination(1);
        bulk.contents
            .insert(0, stored_kind(bulk.id, "minecraft:iron_ingot", 0, 60));

        let operations = build_cleanup_work_units(&source, &[&bulk])
            .unwrap()
            .unwrap();
        let WorkUnitData::Transfer(transfer) = &operations[0].work_unit.data else {
            panic!("expected transfer")
        };
        assert_eq!(transfer.transfer.moves[0].to_slot, 0);
        assert_eq!(transfer.transfer.moves[0].quantity, 4);
    }
}
