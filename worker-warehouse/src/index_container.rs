use anyhow::Context as _;
use azalea::{
    BlockPos, Client,
    pathfinder::{PathfinderClientExt as _, PathfinderOpts, goals::RadiusGoal},
};
use azalea_core::registry_holder::RegistryHolder;
use azalea_inventory::{DataComponentPatch, ItemStack as AzaleaItemStack};
use common::{
    item_stack::ItemStack,
    work::{WorkUnit, WorkUnitData},
};
use uuid::Uuid;
use worker_core::client::WorkUnitClient;

pub(crate) fn indexed_item_stacks(
    container_id: Uuid,
    contents: &[AzaleaItemStack],
    registries: &RegistryHolder,
) -> Result<Vec<ItemStack>, anyhow::Error> {
    contents
        .iter()
        .enumerate()
        .filter_map(|(slot, item_stack)| item_stack.as_present().map(|item| (slot, item)))
        .map(|(slot, item)| {
            Ok(ItemStack {
                id: Uuid::new_v4(),
                container_id,
                item_kind: item.kind.to_string(),
                slot: i32::try_from(slot)
                    .context("container slot exceeds PostgreSQL integer range")?,
                components: indexed_components(&item.component_patch, registries),
                quantity: item.count,
                components_digest: String::new(),
            })
        })
        .collect()
}

fn indexed_components(
    patch: &DataComponentPatch,
    registries: &RegistryHolder,
) -> serde_json::Value {
    let mut components = serde_json::Map::new();
    for (kind, component) in patch.iter() {
        let value = component
            .map(|component| serde_json::Value::from(component.crc_hash(registries).0))
            .unwrap_or(serde_json::Value::Null);
        components.insert(kind.to_string(), value);
    }
    serde_json::Value::Object(components)
}

pub async fn index_container(
    mut work_unit: WorkUnit,
    client: &WorkUnitClient,
    mc_client: &Client,
) -> Result<(), anyhow::Error> {
    let WorkUnitData::IndexContainer(mut data) = work_unit.data else {
        anyhow::bail!("warehouse worker received a non-container work unit")
    };

    let position = BlockPos::new(
        data.position.x as i32,
        data.position.y as i32,
        data.position.z as i32,
    );
    mc_client
        .goto_with_opts(
            RadiusGoal::new(position.center(), 3.0),
            PathfinderOpts::new().allow_mining(false),
        )
        .await;
    mc_client.look_at(position.center());

    let container = mc_client
        .open_container_at(position)
        .await?
        .context("container did not open")?;
    let contents = container
        .contents()
        .context("container closed before its contents could be read")?;
    let output = mc_client.with_registry_holder(|registries| {
        indexed_item_stacks(data.container_id, &contents, registries)
    })??;
    container.close();

    data.output = Some(output);
    work_unit.data = WorkUnitData::IndexContainer(data);
    if !client.submit(work_unit).await? {
        anyhow::bail!("completed container scan was not accepted as done")
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use azalea::registry::{DataRegistry, builtin::ItemKind, data::Enchantment};
    use azalea_core::registry_holder::RegistryHolder;
    use azalea_inventory::{ItemStack as AzaleaItemStack, components::Enchantments};
    use serde_json::json;
    use uuid::Uuid;

    use super::indexed_item_stacks;

    #[test]
    fn translates_non_empty_container_slots() {
        let container_id = Uuid::new_v4();
        let contents = vec![
            AzaleaItemStack::Empty,
            AzaleaItemStack::new(ItemKind::Stone, 12),
        ];

        let stacks =
            indexed_item_stacks(container_id, &contents, &RegistryHolder::default()).unwrap();

        assert_eq!(stacks.len(), 1);
        assert_eq!(stacks[0].container_id, container_id);
        assert_eq!(stacks[0].item_kind, "minecraft:stone");
        assert_eq!(stacks[0].slot, 1);
        assert_eq!(stacks[0].components, json!({}));
        assert_eq!(stacks[0].quantity, 12);
    }

    #[test]
    fn translates_an_empty_container_to_no_stacks() {
        let stacks = indexed_item_stacks(
            Uuid::new_v4(),
            &[AzaleaItemStack::Empty],
            &RegistryHolder::default(),
        )
        .unwrap();

        assert!(stacks.is_empty());
    }

    #[test]
    fn translates_components_with_structured_map_keys() {
        let enchantments = Enchantments {
            levels: HashMap::from([(<Enchantment as DataRegistry>::new_raw(0), 1)]),
        };
        let contents =
            [AzaleaItemStack::new(ItemKind::DiamondSword, 1).with_component(enchantments)];

        let stacks =
            indexed_item_stacks(Uuid::new_v4(), &contents, &RegistryHolder::default()).unwrap();

        let components = stacks[0].components.as_object().unwrap();
        assert!(components["minecraft:enchantments"].is_number());
    }
}
