use std::{collections::HashMap, time::Duration};

use anyhow::{Context as _, ensure};
use azalea::{
    BlockPos, Client,
    container::ContainerHandle,
    pathfinder::{PathfinderClientExt as _, PathfinderOpts, goals::RadiusGoal},
};
use azalea_inventory::{ItemStack as AzaleaItemStack, Menu, item::MaxStackSizeExt as _};
use common::{
    item_stack::ItemStack,
    mwms::transfers::ItemMove,
    vec::Point,
    work::{WorkUnit, WorkUnitData},
    work_units::transfer::{StagedTransferMove, TransferWorkUnit},
};
use uuid::Uuid;
use worker_core::client::WorkUnitClient;

use crate::index_container::indexed_item_stacks;

const INVENTORY_SETTLE_TIME: Duration = Duration::from_millis(300);

fn block_position(point: Point) -> BlockPos {
    BlockPos::new(point.x as i32, point.y as i32, point.z as i32)
}

async fn open_container(
    client: &Client,
    position: Point,
) -> Result<ContainerHandle, anyhow::Error> {
    let position = block_position(position);
    client
        .goto_with_opts(
            RadiusGoal::new(position.center(), 3.0),
            PathfinderOpts::new().allow_mining(false),
        )
        .await;
    client.look_at(position.center());
    client
        .open_container_at(position)
        .await?
        .with_context(|| format!("container at {position:?} did not open"))
}

fn empty_player_slots(menu: &Menu) -> Vec<(usize, usize)> {
    let start = *menu.player_slots_range().start();
    menu.player_slots_range()
        .filter(|slot| menu.slot(*slot).is_some_and(AzaleaItemStack::is_empty))
        .map(|slot| (slot, slot - start))
        .collect()
}

fn player_slot(menu: &Menu, offset: usize) -> Option<usize> {
    let range = menu.player_slots_range();
    let slot = range.start().checked_add(offset)?;
    range.contains(&slot).then_some(slot)
}

fn same_item(left: &AzaleaItemStack, right: &AzaleaItemStack) -> bool {
    match (left.as_present(), right.as_present()) {
        (Some(left), Some(right)) => left.is_same_item_and_components(right),
        _ => false,
    }
}

fn destination_move_already_applied(
    destination: &AzaleaItemStack,
    staged: &AzaleaItemStack,
    quantity: u32,
) -> bool {
    !destination.is_empty()
        && (staged.is_empty() || same_item(destination, staged))
        && destination.count() >= quantity as i32
}

fn destination_capacity(destination: &AzaleaItemStack, staged: &AzaleaItemStack) -> Option<u32> {
    let staged = staged.as_present()?;
    if destination.is_empty() {
        return u32::try_from(staged.kind.max_stack_size()).ok();
    }
    let destination = destination.as_present()?;
    if !destination.is_same_item_and_components(staged) {
        return None;
    }
    u32::try_from(staged.kind.max_stack_size() - destination.count)
        .ok()
        .filter(|capacity| *capacity > 0)
}

fn read_container(
    handle: &ContainerHandle,
    container_id: Uuid,
    client: &Client,
) -> Result<Vec<ItemStack>, anyhow::Error> {
    let contents = handle
        .contents()
        .context("container closed before its contents could be read")?;
    client.with_registry_holder(|registries| {
        indexed_item_stacks(container_id, &contents, registries)
    })?
}

async fn settle_inventory() {
    tokio::time::sleep(INVENTORY_SETTLE_TIME).await;
}

fn pending_move_batches(
    moves: &[ItemMove],
    completed: &[usize],
    inventory_slots: usize,
) -> Vec<Vec<usize>> {
    let mut batch_by_source: HashMap<usize, usize> = HashMap::new();
    let mut batches: Vec<Vec<usize>> = Vec::new();

    for (move_index, item_move) in moves.iter().enumerate() {
        if completed.contains(&move_index) {
            continue;
        }
        if let Some(batch_index) = batch_by_source.get(&item_move.from_slot) {
            batches[*batch_index].push(move_index);
        } else if batches.len() < inventory_slots {
            batch_by_source.insert(item_move.from_slot, batches.len());
            batches.push(vec![move_index]);
        }
    }

    batches
}

fn move_quantity(
    container: &ContainerHandle,
    from_slot: usize,
    to_slot: usize,
    quantity: u32,
    available: u32,
) {
    container.left_click(from_slot);
    if quantity == available {
        container.left_click(to_slot);
    } else {
        for _ in 0..quantity {
            container.right_click(to_slot);
        }
        container.left_click(from_slot);
    }
}

struct SourceBatch {
    move_indices: Vec<usize>,
    player_slot: usize,
    player_slot_offset: usize,
    source_slot: usize,
    source_stack: AzaleaItemStack,
    source_count: i32,
    quantity: u32,
}

async fn stage_moves(client: &Client, unit: &mut TransferWorkUnit) -> Result<(), anyhow::Error> {
    let source = open_container(client, unit.transfer.from_position).await?;
    let menu = source
        .menu()?
        .context("source container closed before it could be read")?;
    let available_slots = empty_player_slots(&menu);
    ensure!(
        !available_slots.is_empty(),
        "no empty player inventory slot for transfer"
    );
    let batches =
        pending_move_batches(&unit.transfer.moves, &unit.completed, available_slots.len());
    ensure!(!batches.is_empty(), "transfer has no moves to stage");

    let mut source_batches = Vec::with_capacity(batches.len());
    for (move_indices, (player_slot, player_slot_offset)) in
        batches.into_iter().zip(available_slots)
    {
        let source_slot = unit.transfer.moves[move_indices[0]].from_slot;
        let quantity = move_indices
            .iter()
            .try_fold(0_u32, |quantity, move_index| {
                let item_move = unit.transfer.moves[*move_index];
                ensure!(
                    item_move.quantity > 0,
                    "transfer move quantity must be positive"
                );
                quantity
                    .checked_add(item_move.quantity)
                    .context("staged transfer quantity exceeds u32")
            })?;
        let source_stack = menu
            .slot(source_slot)
            .with_context(|| format!("source slot {source_slot} is out of bounds"))?
            .clone();
        let source_count = source_stack.count();
        ensure!(source_count > 0, "source slot {source_slot} is empty");
        ensure!(
            quantity <= source_count as u32,
            "source slot {source_slot} contains {source_count} items, fewer than the batched {quantity}",
        );

        source_batches.push(SourceBatch {
            move_indices,
            player_slot,
            player_slot_offset,
            source_slot,
            source_stack,
            source_count,
            quantity,
        });
    }

    for batch in &source_batches {
        move_quantity(
            &source,
            batch.source_slot,
            batch.player_slot,
            batch.quantity,
            batch.source_count as u32,
        );
    }
    settle_inventory().await;

    let menu = source
        .menu()?
        .context("source container closed while staging transfer")?;
    for batch in &source_batches {
        let remaining = menu
            .slot(batch.source_slot)
            .context("source slot disappeared while staging transfer")?;
        ensure!(
            remaining.count() == batch.source_count - batch.quantity as i32,
            "source slot did not accept transfer click"
        );
        let player_slot = player_slot(&menu, batch.player_slot_offset)
            .context("player inventory slot disappeared while staging transfer")?;
        let player_stack = menu
            .slot(player_slot)
            .context("player inventory slot disappeared while staging transfer")?;
        ensure!(
            player_stack.count() == batch.quantity as i32
                && same_item(&batch.source_stack, player_stack),
            "player inventory did not receive the staged items"
        );
    }

    unit.from_contents = Some(read_container(
        &source,
        unit.transfer.from_container,
        client,
    )?);
    unit.staged = source_batches
        .into_iter()
        .flat_map(|batch| {
            batch
                .move_indices
                .into_iter()
                .map(move |move_index| StagedTransferMove {
                    move_index,
                    player_slot_offset: batch.player_slot_offset,
                })
        })
        .collect();
    source.close();
    Ok(())
}

async fn deposit_staged_moves(
    client: &Client,
    unit: &mut TransferWorkUnit,
) -> Result<Vec<usize>, anyhow::Error> {
    ensure!(!unit.staged.is_empty(), "transfer has no staged moves");
    let destination = open_container(client, unit.transfer.to_position).await?;
    let mut expected_targets: HashMap<usize, (AzaleaItemStack, i32)> = HashMap::new();
    let mut remaining_by_player_slot = unit.staged.iter().try_fold(
        HashMap::<usize, u32>::new(),
        |mut remaining, staged_move| {
            let item_move = unit
                .transfer
                .moves
                .get(staged_move.move_index)
                .context("staged transfer move is out of bounds")?;
            let quantity = remaining.entry(staged_move.player_slot_offset).or_default();
            *quantity = quantity
                .checked_add(item_move.quantity)
                .context("staged transfer quantity exceeds u32")?;
            Ok::<_, anyhow::Error>(remaining)
        },
    )?;
    for staged_move in &unit.staged {
        let item_move = unit
            .transfer
            .moves
            .get(staged_move.move_index)
            .context("staged transfer move is out of bounds")?;
        ensure!(
            !unit.completed.contains(&staged_move.move_index),
            "staged transfer move is already completed"
        );
        let menu = destination
            .menu()?
            .context("destination container closed before it could be read")?;
        let player_slot = player_slot(&menu, staged_move.player_slot_offset)
            .context("staged player inventory slot is out of bounds")?;
        let staged_stack = menu
            .slot(player_slot)
            .context("staged player inventory slot disappeared")?
            .clone();
        let target_before = menu
            .slot(item_move.to_slot)
            .with_context(|| format!("destination slot {} is out of bounds", item_move.to_slot))?
            .clone();
        if destination_move_already_applied(&target_before, &staged_stack, item_move.quantity) {
            *remaining_by_player_slot
                .get_mut(&staged_move.player_slot_offset)
                .context("staged player inventory quantity is missing")? -= item_move.quantity;
            continue;
        }
        ensure!(
            staged_stack.count()
                == *remaining_by_player_slot
                    .get(&staged_move.player_slot_offset)
                    .context("staged player inventory quantity is missing")?
                    as i32,
            "staged player inventory slot does not contain the batched quantity"
        );
        let capacity = destination_capacity(&target_before, &staged_stack)
            .context("destination slot cannot accept the staged item")?;
        ensure!(
            capacity >= item_move.quantity,
            "destination slot does not have enough capacity"
        );

        let available = *remaining_by_player_slot
            .get(&staged_move.player_slot_offset)
            .context("staged player inventory quantity is missing")?;
        move_quantity(
            &destination,
            player_slot,
            item_move.to_slot,
            item_move.quantity,
            available,
        );
        *remaining_by_player_slot
            .get_mut(&staged_move.player_slot_offset)
            .context("staged player inventory quantity is missing")? -= item_move.quantity;
        expected_targets.insert(
            item_move.to_slot,
            (
                staged_stack,
                target_before.count() + item_move.quantity as i32,
            ),
        );
    }
    settle_inventory().await;

    let menu = destination
        .menu()?
        .context("destination container closed while depositing transfer")?;
    for staged_move in &unit.staged {
        let player_slot = player_slot(&menu, staged_move.player_slot_offset)
            .context("staged player inventory slot is out of bounds")?;
        ensure!(
            menu.slot(player_slot)
                .is_some_and(AzaleaItemStack::is_empty),
            "player inventory did not release the staged items"
        );
    }
    for (target_slot, (staged_stack, expected_count)) in expected_targets {
        let target_after = menu
            .slot(target_slot)
            .context("destination slot disappeared while depositing transfer")?;
        ensure!(
            target_after.count() == expected_count && same_item(&staged_stack, target_after),
            "destination slot did not receive the staged items"
        );
    }

    unit.to_contents = Some(read_container(
        &destination,
        unit.transfer.to_container,
        client,
    )?);
    destination.close();
    Ok(unit.staged.iter().map(|staged| staged.move_index).collect())
}

pub async fn transfer(
    mut work_unit: WorkUnit,
    control_client: &WorkUnitClient,
    minecraft_client: &Client,
) -> Result<(), anyhow::Error> {
    let WorkUnitData::Transfer(mut data) = work_unit.data else {
        anyhow::bail!("warehouse worker received a non-transfer work unit")
    };

    for completed in &data.completed {
        ensure!(
            *completed < data.transfer.moves.len(),
            "completed transfer move is out of bounds"
        );
    }

    loop {
        if data.staged.is_empty() {
            if (0..data.transfer.moves.len()).all(|index| data.completed.contains(&index)) {
                break;
            }
            stage_moves(minecraft_client, &mut data).await?;
            work_unit.data = WorkUnitData::Transfer(data.clone());
            ensure!(
                !control_client.submit(work_unit.clone()).await?,
                "staged transfer was accepted as complete"
            );
        }

        let move_indices = deposit_staged_moves(minecraft_client, &mut data).await?;
        data.staged.clear();
        data.completed.extend(move_indices);
        work_unit.data = WorkUnitData::Transfer(data.clone());
        let done = control_client.submit(work_unit.clone()).await?;
        if done {
            return Ok(());
        }
    }

    anyhow::bail!("transfer has no remaining moves but was not accepted as complete")
}

#[cfg(test)]
mod tests {
    use azalea::registry::builtin::{ItemKind, MenuKind};

    use super::*;

    #[test]
    fn finds_player_slots_relative_to_each_menu() {
        for kind in [MenuKind::Generic9x3, MenuKind::Hopper] {
            let mut menu = Menu::from_kind(kind);
            let (slot, offset) = empty_player_slots(&menu)[0];
            assert_eq!(offset, 0);
            assert_eq!(Some(slot), player_slot(&menu, offset));
            assert_eq!(slot, *menu.player_slots_range().start());
            assert_eq!(empty_player_slots(&menu).len(), 36);

            *menu.slot_mut(slot).unwrap() = AzaleaItemStack::new(ItemKind::Dirt, 1);
            let available = empty_player_slots(&menu);
            assert_eq!(available.len(), 35);
            assert!(
                !available
                    .iter()
                    .any(|(available_slot, _)| *available_slot == slot)
            );
        }
    }

    #[test]
    fn destination_requires_matching_item_and_capacity() {
        let staged = AzaleaItemStack::new(ItemKind::Stone, 10);

        assert_eq!(
            destination_capacity(&AzaleaItemStack::Empty, &staged),
            Some(64)
        );
        assert_eq!(
            destination_capacity(&AzaleaItemStack::new(ItemKind::Stone, 60), &staged),
            Some(4)
        );
        assert_eq!(
            destination_capacity(&AzaleaItemStack::new(ItemKind::Dirt, 1), &staged),
            None
        );
    }

    #[test]
    fn recognizes_a_move_that_was_deposited_before_retry() {
        let destination = AzaleaItemStack::new(ItemKind::Stone, 3);
        let staged = AzaleaItemStack::new(ItemKind::Stone, 3);

        assert!(destination_move_already_applied(&destination, &staged, 3));
        assert!(destination_move_already_applied(
            &destination,
            &AzaleaItemStack::Empty,
            3
        ));
        assert!(!destination_move_already_applied(
            &AzaleaItemStack::Empty,
            &staged,
            3
        ));
    }

    #[test]
    fn batches_all_moves_from_the_same_source_slot_together() {
        let moves = vec![
            ItemMove {
                from_slot: 6,
                to_slot: 0,
                quantity: 32,
            },
            ItemMove {
                from_slot: 7,
                to_slot: 1,
                quantity: 64,
            },
            ItemMove {
                from_slot: 6,
                to_slot: 2,
                quantity: 32,
            },
            ItemMove {
                from_slot: 8,
                to_slot: 3,
                quantity: 64,
            },
        ];

        assert_eq!(
            pending_move_batches(&moves, &[], 2),
            vec![vec![0, 2], vec![1]]
        );
    }
}
