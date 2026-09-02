use std::time::Duration;

use anyhow::Context;
use azalea::{
    BlockPos, Client,
    core::position::ChunkPos,
    pathfinder::{PathfinderClientExt, goals::BlockPosGoal},
};
use common::{
    vec::{Point, PointRegion},
    work::{WorkUnit, WorkUnitData},
    work_units::index_region::IndexChunkWorkUnitContainer,
};
use worker_core::client::WorkUnitClient;

const LOAD_CHECKS: usize = 100;
const LOAD_CHECK_INTERVAL: Duration = Duration::from_millis(100);

fn container_capacity(block_id: &str) -> Option<usize> {
    Some(match block_id {
        "chest"
        | "trapped_chest"
        | "barrel"
        | "shulker_box"
        | "white_shulker_box"
        | "orange_shulker_box"
        | "magenta_shulker_box"
        | "light_blue_shulker_box"
        | "yellow_shulker_box"
        | "lime_shulker_box"
        | "pink_shulker_box"
        | "gray_shulker_box"
        | "light_gray_shulker_box"
        | "cyan_shulker_box"
        | "purple_shulker_box"
        | "blue_shulker_box"
        | "brown_shulker_box"
        | "green_shulker_box"
        | "red_shulker_box"
        | "black_shulker_box" => 27,
        "hopper" => 5,
        "dropper" | "dispenser" | "crafter" => 9,
        "furnace" | "blast_furnace" | "smoker" => 3,
        "brewing_stand" => 5,
        "chiseled_bookshelf" => 6,
        _ => return None,
    })
}

fn chunk_distance_squared(chunk: &PointRegion, position: BlockPos) -> i64 {
    let chunk_x = chunk.pos1.x as i32 / 16;
    let chunk_z = chunk.pos1.z as i32 / 16;
    let player_chunk_x = position.x.div_euclid(16);
    let player_chunk_z = position.z.div_euclid(16);
    let dx = i64::from(chunk_x - player_chunk_x);
    let dz = i64::from(chunk_z - player_chunk_z);
    dx * dx + dz * dz
}

async fn ensure_chunk_loaded(client: &Client, chunk: &PointRegion) -> Result<(), anyhow::Error> {
    let chunk_pos = ChunkPos::new(chunk.pos1.x as i32 / 16, chunk.pos1.z as i32 / 16);
    let loaded = || -> Result<bool, anyhow::Error> {
        Ok(client.world()?.read().chunks.get(&chunk_pos).is_some())
    };

    if loaded()? {
        return Ok(());
    }

    let current_position = client
        .entity()
        .position()
        .context("could not read Minecraft client position")?;
    let target = BlockPos::new(
        chunk.pos1.x as i32 + 8,
        current_position.y.floor() as i32,
        chunk.pos1.z as i32 + 8,
    );
    client.goto(BlockPosGoal(target)).await;

    for _ in 0..LOAD_CHECKS {
        if loaded()? {
            return Ok(());
        }
        tokio::time::sleep(LOAD_CHECK_INTERVAL).await;
    }

    anyhow::bail!("chunk ({}, {}) did not load", chunk_pos.x, chunk_pos.z)
}

fn scan_chunk(
    client: &Client,
    region: &PointRegion,
) -> Result<Vec<IndexChunkWorkUnitContainer>, anyhow::Error> {
    let world = client.world()?;
    let world = world.read();
    let min_x = region.pos1.x.min(region.pos2.x).ceil() as i32;
    let max_x = region.pos1.x.max(region.pos2.x).floor() as i32;
    let min_y = region.pos1.y.min(region.pos2.y).ceil() as i32;
    let max_y = region.pos1.y.max(region.pos2.y).floor() as i32;
    let min_z = region.pos1.z.min(region.pos2.z).ceil() as i32;
    let max_z = region.pos1.z.max(region.pos2.z).floor() as i32;
    let mut containers = Vec::new();

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            for z in min_z..=max_z {
                let position = BlockPos::new(x, y, z);
                let Some(state) = world.get_block_state(position) else {
                    continue;
                };
                let Some(capacity) = container_capacity(state.to_trait().id()) else {
                    continue;
                };
                containers.push(IndexChunkWorkUnitContainer {
                    point: Point {
                        x: x as f64,
                        y: y as f64,
                        z: z as f64,
                    },
                    capacity,
                });
            }
        }
    }

    Ok(containers)
}

pub async fn index_region(
    mut work_unit: WorkUnit,
    client: &WorkUnitClient,
    mc_client: &Client,
) -> Result<(), anyhow::Error> {
    let WorkUnitData::IndexRegion(mut data) = work_unit.data else {
        anyhow::bail!("warehouse worker received a non-region work unit")
    };

    let player_position = mc_client
        .entity()
        .position()
        .context("could not read Minecraft client position")?;
    let mut chunks = data.region_range.covered_chunks();
    chunks.sort_by_key(|chunk| chunk_distance_squared(chunk, player_position.into()));

    let already_scanned = data.output_chunks_scanned.min(chunks.len());
    for chunk in chunks.into_iter().skip(already_scanned) {
        ensure_chunk_loaded(mc_client, &chunk).await?;
        data.output_containers
            .extend(scan_chunk(mc_client, &chunk)?);
        data.output_chunks_scanned += 1;
        work_unit.data = WorkUnitData::IndexRegion(data.clone());
        client
            .submit(work_unit.clone())
            .await
            .context("failed to submit index-region progress")?;
    }

    Ok(())
}
