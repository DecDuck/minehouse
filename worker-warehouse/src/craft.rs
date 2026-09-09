use anyhow::Context as _;
use azalea::{
    BlockPos, Client,
    container::ContainerHandle,
    pathfinder::{PathfinderClientExt as _, PathfinderOpts, goals::RadiusGoal},
    registry::builtin::ItemKind,
};
use common::{
    item_stack::{ItemStack, item_kind_stack_size},
    vec::Point,
    work::{WorkUnit, WorkUnitData},
};
use uuid::Uuid;
use worker_core::client::WorkUnitClient;

use crate::index_container::indexed_item_stacks;

const CRAFT_CHECKPOINT_BATCH: u32 = 64;

async fn open_container(
    client: &Client,
    position: Point,
) -> Result<ContainerHandle, anyhow::Error> {
    let block = BlockPos::new(position.x as i32, position.y as i32, position.z as i32);
    client
        .goto_with_opts(
            RadiusGoal::new(block.center(), 3.0),
            PathfinderOpts::new().allow_mining(false),
        )
        .await;
    client.look_at(block.center());
    client
        .open_container_at(block)
        .await?
        .context("craft container did not open")
}

async fn snapshot(
    client: &Client,
    handle: &ContainerHandle,
    id: Uuid,
) -> Result<Vec<ItemStack>, anyhow::Error> {
    let contents = handle
        .contents()
        .context("craft container closed before snapshot")?;
    client.with_registry_holder(|registries| indexed_item_stacks(id, &contents, registries))?
}

pub async fn craft(
    mut work_unit: WorkUnit,
    client: &WorkUnitClient,
    mc_client: &Client,
) -> Result<(), anyhow::Error> {
    let WorkUnitData::Craft(mut data) = work_unit.data.clone() else {
        anyhow::bail!("warehouse worker received a non-craft work unit")
    };
    if data.progress.completed_crafts >= data.recipe.crafts {
        return Ok(());
    }

    let engine = BlockPos::new(
        data.locations.engine.x as i32,
        data.locations.engine.y as i32,
        data.locations.engine.z as i32,
    );
    mc_client
        .goto_with_opts(
            RadiusGoal::new(engine.center(), 3.0),
            PathfinderOpts::new().allow_mining(false),
        )
        .await;

    let input_handle = open_container(mc_client, data.locations.input_container).await?;
    let mut input = snapshot(mc_client, &input_handle, data.locations.input_container_id).await?;
    input_handle.close();
    let same_container = data.locations.input_container_id == data.locations.output_container_id;
    let mut output = if same_container {
        input.clone()
    } else {
        let output_handle = open_container(mc_client, data.locations.output_container).await?;
        let output = snapshot(
            mc_client,
            &output_handle,
            data.locations.output_container_id,
        )
        .await?;
        output_handle.close();
        output
    };

    while data.progress.completed_crafts < data.recipe.crafts {
        let batch =
            (data.recipe.crafts - data.progress.completed_crafts).min(CRAFT_CHECKPOINT_BATCH);
        if same_container {
            apply_craft_batch(
                &mut output,
                &data.recipe.ingredients,
                data.recipe.output_item_kind,
                data.recipe.output_yield,
                batch,
                data.locations.output_container_id,
                data.locations.output_container_capacity,
            )?;
            input = output.clone();
        } else {
            consume_ingredients(&mut input, &data.recipe.ingredients, batch)?;
            add_output(
                &mut output,
                data.recipe.output_item_kind,
                data.recipe.output_yield.saturating_mul(batch),
                data.locations.output_container_id,
                data.locations.output_container_capacity,
            )?;
        }
        data.progress.completed_crafts += batch;
        data.progress.input_snapshot = Some(input.clone());
        data.progress.output_snapshot = Some(output.clone());
        work_unit.data = WorkUnitData::Craft(data.clone());
        let done = client.submit(work_unit.clone()).await?;
        anyhow::ensure!(
            done == (data.progress.completed_crafts >= data.recipe.crafts),
            "craft checkpoint completion disagrees with work unit progress"
        );
    }
    Ok(())
}

fn apply_craft_batch(
    contents: &mut Vec<ItemStack>,
    ingredients: &[common::work_units::craft::CraftIngredient],
    output_item_kind: ItemKind,
    output_yield: u32,
    crafts: u32,
    container_id: Uuid,
    capacity: u64,
) -> Result<(), anyhow::Error> {
    consume_ingredients(contents, ingredients, crafts)?;
    add_output(
        contents,
        output_item_kind,
        output_yield.saturating_mul(crafts),
        container_id,
        capacity,
    )?;
    Ok(())
}

fn consume_ingredients(
    contents: &mut Vec<ItemStack>,
    ingredients: &[common::work_units::craft::CraftIngredient],
    crafts: u32,
) -> Result<(), anyhow::Error> {
    let mut required_by_item = std::collections::BTreeMap::<String, u32>::new();
    for ingredient in ingredients {
        let item_kind = ingredient.item_kind.to_string();
        let required = required_by_item.entry(item_kind).or_default();
        *required = required.saturating_add(ingredient.quantity.saturating_mul(crafts));
    }
    for (item_kind, required) in &required_by_item {
        let available = contents
            .iter()
            .filter(|stack| stack.item_kind == *item_kind)
            .map(|stack| stack.quantity.max(0) as u32)
            .sum::<u32>();
        anyhow::ensure!(
            available >= *required,
            "insufficient {} in craft input container",
            item_kind
        );
    }
    for (item_kind, required) in required_by_item {
        let mut remaining = required;
        for stack in contents
            .iter_mut()
            .filter(|stack| stack.item_kind == item_kind)
        {
            let used = remaining.min(stack.quantity.max(0) as u32);
            stack.quantity -= used as i32;
            remaining -= used;
            if remaining == 0 {
                break;
            }
        }
        anyhow::ensure!(
            remaining == 0,
            "insufficient {} in craft input container",
            item_kind
        );
    }
    contents.retain(|stack| stack.quantity > 0);
    Ok(())
}

fn add_output(
    contents: &mut Vec<ItemStack>,
    output_item_kind: ItemKind,
    quantity: u32,
    container_id: Uuid,
    capacity: u64,
) -> Result<(), anyhow::Error> {
    let mut next = contents.clone();
    let output_item_kind = output_item_kind.to_string();
    let max_quantity = item_kind_stack_size(&output_item_kind);
    let mut remaining = quantity;
    for stack in next
        .iter_mut()
        .filter(|stack| stack.item_kind == output_item_kind && stack.components_digest.is_empty())
    {
        let current = u32::try_from(stack.quantity.max(0))?;
        let placed = remaining.min(max_quantity.saturating_sub(current));
        stack.quantity = i32::try_from(current.saturating_add(placed))?;
        remaining -= placed;
        if remaining == 0 {
            *contents = next;
            return Ok(());
        }
    }
    for slot in 0..usize::try_from(capacity)? {
        if next.iter().any(|stack| stack.slot == slot as i32) {
            continue;
        }
        let placed = remaining.min(max_quantity);
        next.push(ItemStack {
            id: Uuid::new_v4(),
            container_id,
            item_kind: output_item_kind.clone(),
            slot: i32::try_from(slot)?,
            components: serde_json::json!({}),
            quantity: i32::try_from(placed)?,
            components_digest: String::new(),
        });
        remaining -= placed;
        if remaining == 0 {
            *contents = next;
            return Ok(());
        }
    }
    anyhow::bail!("craft output exceeds container capacity")
}

#[cfg(test)]
mod tests {
    use common::work_units::craft::CraftIngredient;
    use serde_json::json;

    use super::*;

    fn stack(container_id: Uuid, item_kind: &str, quantity: i32, slot: i32) -> ItemStack {
        ItemStack {
            id: Uuid::new_v4(),
            container_id,
            item_kind: item_kind.to_owned(),
            slot,
            components: json!({}),
            quantity,
            components_digest: String::new(),
        }
    }

    #[test]
    fn same_container_batches_consume_inputs_and_accumulate_outputs() {
        let container_id = Uuid::new_v4();
        let ingredients = vec![CraftIngredient {
            item_kind: ItemKind::IronIngot,
            quantity: 2,
        }];
        let mut contents = vec![stack(container_id, "minecraft:iron_ingot", 10, 0)];

        apply_craft_batch(
            &mut contents,
            &ingredients,
            ItemKind::Hopper,
            1,
            3,
            container_id,
            2,
        )
        .unwrap();
        apply_craft_batch(
            &mut contents,
            &ingredients,
            ItemKind::Hopper,
            1,
            2,
            container_id,
            2,
        )
        .unwrap();

        assert!(
            !contents
                .iter()
                .any(|stack| stack.item_kind == "minecraft:iron_ingot")
        );
        assert_eq!(
            contents
                .iter()
                .find(|stack| stack.item_kind == "minecraft:hopper")
                .unwrap()
                .quantity,
            5
        );
    }

    #[test]
    fn batch_rejects_insufficient_inputs_without_adding_output() {
        let container_id = Uuid::new_v4();
        let ingredients = vec![CraftIngredient {
            item_kind: ItemKind::IronIngot,
            quantity: 2,
        }];
        let mut contents = vec![stack(container_id, "minecraft:iron_ingot", 3, 0)];

        let error = apply_craft_batch(
            &mut contents,
            &ingredients,
            ItemKind::Hopper,
            1,
            2,
            container_id,
            1,
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("insufficient minecraft:iron_ingot")
        );
        assert_eq!(contents[0].quantity, 3);
        assert!(
            !contents
                .iter()
                .any(|stack| stack.item_kind == "minecraft:hopper")
        );
    }

    #[test]
    fn output_splits_across_slots_at_item_stack_limit() {
        let container_id = Uuid::new_v4();
        let mut contents = Vec::new();

        add_output(&mut contents, ItemKind::IronIngot, 65, container_id, 2).unwrap();

        contents.sort_by_key(|stack| stack.slot);
        assert_eq!(contents.len(), 2);
        assert_eq!(contents[0].quantity, 64);
        assert_eq!(contents[1].quantity, 1);
    }

    #[test]
    fn output_rejects_non_stackable_overflow() {
        let container_id = Uuid::new_v4();
        let mut contents = Vec::new();

        let error =
            add_output(&mut contents, ItemKind::DiamondSword, 2, container_id, 1).unwrap_err();

        assert!(error.to_string().contains("exceeds container capacity"));
        assert!(contents.is_empty());
    }
}
