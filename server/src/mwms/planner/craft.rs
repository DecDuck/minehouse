use std::time::{Duration, Instant};

use anyhow::{Context as _, ensure};
use common::{
    mwms::transfers::{ItemMove, Transfer},
    vec::Point,
    work::{WorkUnit, WorkUnitData},
    work_units::craft::{CraftLocations, CraftProgress, CraftRecipe, CraftWorkUnit},
};
use uuid::Uuid;

use crate::{
    db::container_region::RegionType,
    db::recipe::Recipe,
    mwms::crafting::{CraftJobState, CraftNodeKind, CraftPlanRequest, resolve_plan},
    mwms::planner::Planner,
};

pub(crate) enum CraftAttempt {
    Waiting,
    Completed,
}

impl Planner {
    pub(crate) async fn craft(
        &self,
        request: CraftPlanRequest,
    ) -> Result<CraftAttempt, anyhow::Error> {
        let job_id = request
            .job_id
            .as_deref()
            .and_then(|id| Uuid::parse_str(id).ok());
        let plan = resolve_plan(&self.state.db, &request).await?;
        ensure!(!plan.unresolved, "craft plan contains unresolved choices");
        if !matches!(plan.root.kind, CraftNodeKind::Recipe { .. }) {
            anyhow::bail!("craft target must resolve to a recipe");
        }
        if plan
            .raw_materials
            .iter()
            .any(|requirement| requirement.quantity > requirement.available)
        {
            if let Some(job_id) = job_id {
                self.state
                    .db
                    .update_craft_job(job_id, CraftJobState::Waiting, 0, None)
                    .await?;
            }
            return Ok(CraftAttempt::Waiting);
        }
        if let Some(job_id) = job_id {
            self.state
                .db
                .update_craft_job(job_id, CraftJobState::Running, 0, None)
                .await?;
        }
        let recipes = self
            .state
            .db
            .fetch_all_recipes()
            .await?
            .into_iter()
            .map(|recipe| (recipe.id, recipe))
            .collect::<std::collections::HashMap<_, _>>();

        let regions = self.state.db.fetch_all_regions().await?;
        let processing_region_ids = regions
            .iter()
            .filter(|region| region.r#type == RegionType::Processing)
            .map(|region| region.id)
            .collect::<std::collections::HashSet<_>>();
        let processing_container = self
            .state
            .db
            .fetch_all_containers()
            .await?
            .into_iter()
            .find(|container| processing_region_ids.contains(&container.region_id))
            .context("no processing container is configured")?;
        let mut processing_slots = (0..processing_container.capacity as usize)
            .filter(|slot| !processing_container.contents.contains_key(slot));
        let mut source_containers = self
            .state
            .db
            .fetch_all_containers()
            .await?
            .into_iter()
            .filter(|container| container.id != processing_container.id)
            .collect::<Vec<_>>();
        let position = |cube: crate::db::Cube| Point {
            x: cube.x1,
            y: cube.y1,
            z: cube.z1,
        };
        for requirement in &plan.raw_materials {
            let mut remaining = requirement.quantity;
            for source in source_containers.iter_mut() {
                let stacks = source.contents.values().cloned().collect::<Vec<_>>();
                for stack in stacks {
                    if stack.item_kind != requirement.item_kind || remaining == 0 {
                        continue;
                    }
                    let quantity = remaining.min(stack.quantity.max(0) as u32);
                    let to_slot = processing_slots
                        .next()
                        .context("processing container has no free slots")?;
                    let transfer = Transfer {
                        from_container: source.id,
                        from_position: position(source.position),
                        to_container: processing_container.id,
                        to_position: position(processing_container.position),
                        moves: vec![ItemMove {
                            from_slot: stack.slot as usize,
                            to_slot,
                            quantity,
                        }],
                    };
                    execute_transfer(self, transfer).await?;
                    if let Some(source_stack) = source.contents.get_mut(&(stack.slot as usize)) {
                        source_stack.quantity -= quantity as i32;
                    }
                    remaining -= quantity;
                }
            }
            ensure!(
                remaining == 0,
                "insufficient indexed storage for {}",
                requirement.item_kind
            );
        }
        let engine = self
            .state
            .db
            .fetch_crafting_engine(RegionType::Processing)
            .await?
            .context("no processing crafting engine is configured")?;

        let mut recipe_nodes = Vec::new();
        collect_recipe_nodes(&plan.root, &mut recipe_nodes);
        for node in recipe_nodes {
            execute_recipe_node(
                self,
                node,
                &recipes,
                position(engine.position),
                processing_container.id,
                position(processing_container.position),
            )
            .await?;
        }
        move_processing_to_bulk(self, processing_container.id).await?;
        if let Some(job_id) = job_id {
            self.state
                .db
                .update_craft_job(job_id, CraftJobState::Completed, request.amount, None)
                .await?;
        }
        Ok(CraftAttempt::Completed)
    }
}

fn collect_recipe_nodes<'a>(
    node: &'a crate::mwms::crafting::CraftNode,
    nodes: &mut Vec<&'a crate::mwms::crafting::CraftNode>,
) {
    for child in &node.children {
        collect_recipe_nodes(child, nodes);
    }
    if matches!(node.kind, CraftNodeKind::Recipe { .. }) {
        nodes.push(node);
    }
}

async fn execute_recipe_node(
    planner: &Planner,
    node: &crate::mwms::crafting::CraftNode,
    recipes: &std::collections::HashMap<Uuid, Recipe>,
    engine: Point,
    container_id: Uuid,
    container_position: Point,
) -> Result<(), anyhow::Error> {
    let CraftNodeKind::Recipe {
        recipe_id, crafts, ..
    } = &node.kind
    else {
        return Ok(());
    };
    let recipe_id = Uuid::parse_str(recipe_id).context("resolved plan has an invalid recipe id")?;
    let recipe = recipes
        .get(&recipe_id)
        .context("resolved recipe was not found")?;
    let work_unit: WorkUnit = CraftWorkUnit {
        recipe: CraftRecipe {
            recipe_id,
            output_item_kind: recipe.output_item_kind.clone(),
            output_yield: recipe.output_yield,
            ingredients: recipe.ingredients.clone(),
            crafts: *crafts,
        },
        locations: CraftLocations {
            engine,
            input_container_id: container_id,
            input_container: container_position,
            output_container_id: container_id,
            output_container: container_position,
        },
        progress: CraftProgress {
            completed_crafts: 0,
            input_snapshot: None,
            output_snapshot: None,
        },
    }
    .into();
    let id = work_unit.id;
    planner.state.pool.queue_work_unit(work_unit, Some(10));
    let completed = planner
        .state
        .pool
        .wait_work_unit(&id, Instant::now() + Duration::from_hours(2))
        .await?;
    ensure!(
        matches!(completed.data, WorkUnitData::Craft(_)),
        "craft work unit returned the wrong data type"
    );
    Ok(())
}

async fn execute_transfer(planner: &Planner, transfer: Transfer) -> Result<(), anyhow::Error> {
    let work_unit: WorkUnit = common::work_units::transfer::TransferWorkUnit {
        transfer,
        completed: Vec::new(),
        staged: Vec::new(),
        from_contents: None,
        to_contents: None,
    }
    .into();
    let id = work_unit.id;
    planner.state.pool.queue_work_unit(work_unit, Some(20));
    let completed = planner
        .state
        .pool
        .wait_work_unit(&id, Instant::now() + Duration::from_hours(2))
        .await?;
    let WorkUnitData::Transfer(transfer) = completed.data else {
        anyhow::bail!("ingredient transfer returned the wrong work-unit type")
    };
    let from_contents = transfer
        .from_contents
        .context("ingredient transfer has no source snapshot")?;
    let to_contents = transfer
        .to_contents
        .context("ingredient transfer has no destination snapshot")?;
    planner
        .state
        .db
        .replace_container_item_stacks(transfer.transfer.from_container, &from_contents)
        .await?;
    planner
        .state
        .db
        .replace_container_item_stacks(transfer.transfer.to_container, &to_contents)
        .await?;
    Ok(())
}

async fn move_processing_to_bulk(
    planner: &Planner,
    processing_id: Uuid,
) -> Result<(), anyhow::Error> {
    let regions = planner.state.db.fetch_all_regions().await?;
    let bulk_region_ids = regions
        .iter()
        .filter(|region| region.r#type == RegionType::Bulk)
        .map(|region| region.id)
        .collect::<std::collections::HashSet<_>>();
    let mut containers = planner.state.db.fetch_all_containers().await?;
    let source = containers
        .iter()
        .find(|container| container.id == processing_id)
        .cloned()
        .context("processing container disappeared before output storage")?;
    let mut destinations = containers
        .iter_mut()
        .filter(|container| bulk_region_ids.contains(&container.region_id))
        .collect::<Vec<_>>();
    let source_stacks = source.contents.values().cloned().collect::<Vec<_>>();
    let position = |cube: crate::db::Cube| Point {
        x: cube.x1,
        y: cube.y1,
        z: cube.z1,
    };
    for stack in source_stacks {
        let destination = destinations
            .iter_mut()
            .find(|container| container.contents.len() < container.capacity as usize)
            .context("bulk storage has no free slot for craft output")?;
        let to_slot = (0..destination.capacity as usize)
            .find(|slot| !destination.contents.contains_key(slot))
            .context("bulk storage has no free slot for craft output")?;
        execute_transfer(
            planner,
            Transfer {
                from_container: source.id,
                from_position: position(source.position),
                to_container: destination.id,
                to_position: position(destination.position),
                moves: vec![ItemMove {
                    from_slot: stack.slot as usize,
                    to_slot,
                    quantity: stack.quantity.max(0) as u32,
                }],
            },
        )
        .await?;
        destination.contents.remove(&(to_slot as usize));
        destination.contents.insert(to_slot, stack.clone());
    }
    Ok(())
}
