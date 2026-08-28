use std::time::Duration;

use anyhow::anyhow;
use azalea::{
    BlockPos, Client,
    container::ContainerHandle,
    pathfinder::{PathfinderClientExt, goals::ReachBlockPosGoal},
};
use common::{
    item_stack::ItemStack,
    vec::Point,
    work::{WorkUnit, WorkUnitData},
    work_units::{index_container::IndexContainerWorkUnit, transfer::TransferWorkUnit},
};
use uuid::Uuid;

// Time to let inventory packets round-trip after an action. May need tuning
// against a live server.
const SETTLE: Duration = Duration::from_millis(300);

/// Executes a work unit against the world, filling its result fields in place.
pub async fn execute(client: &Client, mut unit: WorkUnit) -> Result<WorkUnit, anyhow::Error> {
    match &mut unit.data {
        WorkUnitData::IndexContainer(index) => index_container(client, index).await?,
        WorkUnitData::Transfer(transfer) => transfer_items(client, transfer).await?,
    }
    Ok(unit)
}

fn block_pos(point: &Point) -> BlockPos {
    BlockPos::new(
        point.x.floor() as i32,
        point.y.floor() as i32,
        point.z.floor() as i32,
    )
}

async fn open_container(client: &Client, point: &Point) -> Result<ContainerHandle, anyhow::Error> {
    let pos = block_pos(point);
    let chunks = client.world()?.read().chunks.clone();
    client.goto(ReachBlockPosGoal::new(pos, chunks)).await;
    client
        .open_container_at(pos)
        .await?
        .ok_or_else(|| anyhow!("could not open container at {pos:?}"))
}

fn read_container(
    handle: &ContainerHandle,
    container_id: Uuid,
) -> Result<Vec<ItemStack>, anyhow::Error> {
    let contents = handle
        .contents()
        .ok_or_else(|| anyhow!("container closed while reading"))?;
    let mut stacks = Vec::new();
    for (slot, item) in contents.iter().enumerate() {
        if let Some(data) = item.as_present() {
            stacks.push(ItemStack {
                id: Uuid::new_v4(),
                container_id,
                item_kind: data.kind.to_string(),
                slot: slot as i32,
                // Component/NBT capture (enchantments, custom names) is a follow-up.
                components: serde_json::json!({}),
                quantity: data.count,
                components_digest: String::new(),
            });
        }
    }
    Ok(stacks)
}

async fn index_container(
    client: &Client,
    unit: &mut IndexContainerWorkUnit,
) -> Result<(), anyhow::Error> {
    let handle = open_container(client, &unit.position).await?;
    tokio::time::sleep(SETTLE).await;
    let stacks = read_container(&handle, unit.container_id)?;
    drop(handle);
    unit.output = Some(stacks);
    Ok(())
}

async fn transfer_items(
    client: &Client,
    unit: &mut TransferWorkUnit,
) -> Result<(), anyhow::Error> {
    let transfer = unit.transfer.clone();

    // Source: stage each move's quantity into the bot's own inventory, since two
    // world containers can't be open at once.
    let source = open_container(client, &transfer.from_position).await?;
    tokio::time::sleep(SETTLE).await;

    let source_slots = source
        .slots()
        .ok_or_else(|| anyhow!("source container closed"))?;
    // Player inventory is always the last 36 slots of any menu.
    let source_player_start = source_slots.len().saturating_sub(36);
    let mut free_offsets: Vec<usize> = (0..36)
        .filter(|o| {
            source_slots
                .get(source_player_start + o)
                .is_some_and(|s| s.is_empty())
        })
        .collect();

    // (move index, player-inventory offset) for each staged move.
    let mut staged: Vec<(usize, usize)> = Vec::new();
    for (i, mv) in transfer.moves.iter().enumerate() {
        let Some(offset) = free_offsets.pop() else {
            tracing::warn!("no free inventory slot to stage transfer move");
            break;
        };
        let src_count = source_slots.get(mv.from_slot).map(|s| s.count()).unwrap_or(0);
        if src_count <= 0 {
            continue;
        }
        let player_slot = source_player_start + offset;
        if mv.quantity as i32 >= src_count {
            // Whole stack: pick up and drop into the inventory slot.
            source.left_click(mv.from_slot);
            source.left_click(player_slot);
        } else {
            // Partial: pick up the stack, deposit N one-by-one, return the rest.
            source.left_click(mv.from_slot);
            for _ in 0..mv.quantity {
                source.right_click(player_slot);
            }
            source.left_click(mv.from_slot);
        }
        staged.push((i, offset));
        tokio::time::sleep(SETTLE).await;
    }

    tokio::time::sleep(SETTLE).await;
    let from_contents = read_container(&source, transfer.from_container)?;
    drop(source);

    // Destination: place each staged item into its target slot.
    let dest = open_container(client, &transfer.to_position).await?;
    tokio::time::sleep(SETTLE).await;
    let dest_slots = dest
        .slots()
        .ok_or_else(|| anyhow!("destination container closed"))?;
    let dest_player_start = dest_slots.len().saturating_sub(36);

    for (i, offset) in &staged {
        let mv = &transfer.moves[*i];
        source_left_then(&dest, dest_player_start + *offset, mv.to_slot);
        unit.completed.push(*i);
        tokio::time::sleep(SETTLE).await;
    }

    tokio::time::sleep(SETTLE).await;
    let to_contents = read_container(&dest, transfer.to_container)?;
    drop(dest);

    unit.from_contents = Some(from_contents);
    unit.to_contents = Some(to_contents);
    Ok(())
}

/// Pick up the whole stack in `from` and place it into `to`.
fn source_left_then(handle: &ContainerHandle, from: usize, to: usize) {
    handle.left_click(from);
    handle.left_click(to);
}
