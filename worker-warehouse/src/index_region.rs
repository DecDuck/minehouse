use std::time::Duration;

use anyhow::Context;
use azalea::{
    BlockPos, Client,
    core::position::ChunkPos,
    pathfinder::{PathfinderClientExt, goals::XZGoal},
};
use common::{
    vec::{Point, PointRegion},
    work::{WorkUnit, WorkUnitData},
    work_units::index_region::IndexChunkWorkUnitContainer,
};
use worker_core::client::WorkUnitClient;

const LOAD_CHECKS: usize = 100;
const LOAD_CHECK_INTERVAL: Duration = Duration::from_millis(100);
const INITIAL_LOAD_CHECKS: usize = 20;

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

fn chunk_position(chunk: &PointRegion) -> ChunkPos {
    ChunkPos::new(
        (chunk.pos1.x.floor() as i32).div_euclid(16),
        (chunk.pos1.z.floor() as i32).div_euclid(16),
    )
}

fn chunk_distance_squared(chunk: &PointRegion, position: BlockPos) -> i64 {
    let chunk_position = chunk_position(chunk);
    let player_chunk_x = position.x.div_euclid(16);
    let player_chunk_z = position.z.div_euclid(16);
    let dx = i64::from(chunk_position.x - player_chunk_x);
    let dz = i64::from(chunk_position.z - player_chunk_z);
    dx * dx + dz * dz
}

fn is_chunk_loaded(client: &Client, chunk: &PointRegion) -> Result<bool, anyhow::Error> {
    Ok(client
        .world()?
        .read()
        .chunks
        .get(&chunk_position(chunk))
        .is_some())
}

async fn wait_for_initial_chunks(
    client: &Client,
    chunks: &[PointRegion],
) -> Result<(), anyhow::Error> {
    for _ in 0..INITIAL_LOAD_CHECKS {
        let mut all_loaded = true;
        for chunk in chunks {
            if !is_chunk_loaded(client, chunk)? {
                all_loaded = false;
                break;
            }
        }
        if all_loaded {
            return Ok(());
        }
        tokio::time::sleep(LOAD_CHECK_INTERVAL).await;
    }

    Ok(())
}

async fn ensure_chunk_loaded(client: &Client, chunk: &PointRegion) -> Result<(), anyhow::Error> {
    let chunk_pos = chunk_position(chunk);

    if is_chunk_loaded(client, chunk)? {
        return Ok(());
    }

    client.start_goto(XZGoal {
        x: chunk.pos1.x as i32 + 8,
        z: chunk.pos1.z as i32 + 8,
    });

    for _ in 0..LOAD_CHECKS {
        match is_chunk_loaded(client, chunk) {
            Ok(true) => {
                client.stop_pathfinding();
                return Ok(());
            }
            Ok(false) => {}
            Err(error) => {
                client.stop_pathfinding();
                return Err(error);
            }
        }
        tokio::time::sleep(LOAD_CHECK_INTERVAL).await;
    }

    client.stop_pathfinding();
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
    chunks.retain(|chunk| !data.output_chunks_scanned.contains(chunk));
    chunks.sort_by_key(|chunk| chunk_distance_squared(chunk, player_position.into()));
    wait_for_initial_chunks(mc_client, &chunks).await?;

    for chunk in chunks {
        ensure_chunk_loaded(mc_client, &chunk).await?;
        let scan_region = data
            .region_range
            .intersection(&chunk)
            .context("covered chunk did not intersect index region")?;
        data.output_containers
            .extend(scan_chunk(mc_client, &scan_region)?);
        data.output_chunks_scanned.push(chunk);
        work_unit.data = WorkUnitData::IndexRegion(data.clone());
        client
            .submit(work_unit.clone())
            .await
            .context("failed to submit index-region progress")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use common::vec::{Point, PointRegion};

    use super::chunk_position;

    #[test]
    fn chunk_position_floors_negative_block_coordinates() {
        let chunk = PointRegion {
            pos1: Point {
                x: -1.0,
                y: 0.0,
                z: -17.0,
            },
            pos2: Point {
                x: -1.0,
                y: 0.0,
                z: -17.0,
            },
        };

        let position = chunk_position(&chunk);

        assert_eq!(position.x, -1);
        assert_eq!(position.z, -2);
    }
}
