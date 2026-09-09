use std::{
    collections::{HashMap, HashSet},
    str::FromStr,
};

use azalea_registry::builtin::ItemKind;
use common::work_units::craft::CraftIngredient;
use oasgen::OaSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{
    DatabaseHandle,
    craft_job::{
        CraftJobNodeData, CraftJobNodeRecord, CraftNodeSpec, NewCraftJobNode, NewCraftJobNodeKind,
    },
    recipe::Recipe,
};

pub type CraftNodeKey = String;

#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
pub struct CraftPlanRequest {
    pub item_kind: String,
    pub amount: u32,
    #[serde(default)]
    pub selections: HashMap<CraftNodeKey, CraftSelection>,
    #[serde(default)]
    pub job_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, OaSchema)]
pub struct CraftJobStatus {
    pub id: String,
    pub target_item_kind: String,
    pub target_quantity: u32,
    pub selections: HashMap<CraftNodeKey, CraftSelection>,
    pub state: CraftJobState,
    pub completed_quantity: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, OaSchema)]
pub struct CraftJobDetail {
    pub job: CraftJobStatus,
    pub root: CraftExecutionNode,
}

#[derive(Debug, Clone, Serialize, OaSchema)]
pub struct CraftExecutionNode {
    pub key: String,
    pub item_kind: String,
    pub required_quantity: u32,
    pub completed_quantity: u32,
    pub state: String,
    pub assigned_region_id: Option<String>,
    pub error: Option<String>,
    pub operation: Option<CraftExecutionOperation>,
    pub children: Vec<CraftExecutionNode>,
    pub kind: CraftExecutionNodeKind,
}

#[derive(Debug, Clone, Serialize, OaSchema)]
pub struct CraftExecutionOperation {
    pub kind: String,
    pub state: String,
    pub completed_amount: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, OaSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CraftExecutionNodeKind {
    Recipe {
        recipe_id: String,
        engine_type: String,
        planned_crafts: u32,
        completed_crafts: u32,
        output_yield: u32,
    },
    Storage,
    Any,
}

#[derive(Debug, Clone, Copy, Serialize, OaSchema, sqlx::Type)]
#[serde(rename_all = "lowercase")]
#[sqlx(type_name = "craft_job_state", rename_all = "snake_case")]
pub enum CraftJobState {
    Queued,
    Waiting,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CraftSelection {
    Recipe { recipe_id: String },
    Storage,
    Any,
}

#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
pub struct CraftPlan {
    pub target_item_kind: String,
    pub target_quantity: u32,
    pub root: CraftNode,
    pub raw_materials: Vec<RawMaterialRequirement>,
    pub unresolved: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
pub struct RawMaterialRequirement {
    pub item_kind: String,
    pub quantity: u32,
    pub available: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
pub struct CraftNode {
    pub key: CraftNodeKey,
    pub item_kind: String,
    pub quantity: u32,
    pub children: Vec<CraftNode>,
    pub kind: CraftNodeKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CraftNodeKind {
    Recipe {
        recipe_id: String,
        crafts: u32,
        output_yield: u32,
        selected: bool,
    },
    Storage {
        available: u32,
        selected: bool,
    },
    Choice {
        options: Vec<CraftOption>,
    },
    Any,
    Deferred,
}

#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CraftOption {
    Recipe {
        recipe_id: String,
        preview: serde_json::Value,
        available: bool,
    },
    Storage {
        preview: serde_json::Value,
        available: bool,
    },
    Any {
        preview: serde_json::Value,
    },
}

pub async fn resolve_plan(
    db: &DatabaseHandle,
    request: &CraftPlanRequest,
) -> Result<CraftPlan, anyhow::Error> {
    anyhow::ensure!(request.amount > 0, "craft amount must be positive");
    let target_item_kind = ItemKind::from_str(&request.item_kind)
        .map_err(|_| anyhow::anyhow!("unknown item kind {}", request.item_kind))?;
    let recipes = db.fetch_all_recipes().await?;
    let mut recipes_by_item: HashMap<ItemKind, Vec<Recipe>> = HashMap::new();
    for recipe in recipes {
        anyhow::ensure!(
            recipe.output_yield > 0,
            "recipe {} has an invalid yield",
            recipe.id
        );
        anyhow::ensure!(
            !recipe.ingredients.is_empty(),
            "recipe {} has no ingredients",
            recipe.id
        );
        recipes_by_item
            .entry(recipe.output_item_kind)
            .or_default()
            .push(recipe);
    }
    let mut item_kinds = recipes_by_item.keys().cloned().collect::<HashSet<_>>();
    item_kinds.insert(target_item_kind);
    for recipe_list in recipes_by_item.values() {
        for recipe in recipe_list {
            item_kinds.extend(
                recipe
                    .ingredients
                    .iter()
                    .map(|ingredient| ingredient.item_kind),
            );
        }
    }
    let mut available = HashMap::new();
    for item_kind in item_kinds {
        let item_kind_name = item_kind.to_string();
        let quantity = db
            .fetch_item_kind_stacks(&item_kind_name)
            .await?
            .into_iter()
            .map(|stack| stack.quantity.max(0) as u32)
            .sum();
        available.insert(item_kind, quantity);
    }
    let root = build_node(
        target_item_kind,
        request.amount,
        "0",
        &request.selections,
        &recipes_by_item,
        &available,
        &mut HashSet::new(),
        false,
    )?;
    let mut raw = HashMap::new();
    collect_raw_materials(&root, &mut raw);
    let raw_materials = raw
        .into_iter()
        .map(|(item_kind, quantity)| {
            let typed_item_kind = ItemKind::from_str(&item_kind)
                .map_err(|_| anyhow::anyhow!("unknown resolved item kind {item_kind}"))?;
            Ok(RawMaterialRequirement {
                available: available.get(&typed_item_kind).copied().unwrap_or(0),
                item_kind,
                quantity,
            })
        })
        .collect::<Result<Vec<_>, anyhow::Error>>()?;
    let unresolved = contains_choice(&root);
    Ok(CraftPlan {
        target_item_kind: request.item_kind.clone(),
        target_quantity: request.amount,
        root,
        raw_materials,
        unresolved,
    })
}

fn build_node(
    item_kind: ItemKind,
    quantity: u32,
    key: &str,
    selections: &HashMap<String, CraftSelection>,
    recipes_by_item: &HashMap<ItemKind, Vec<Recipe>>,
    available: &HashMap<ItemKind, u32>,
    active: &mut HashSet<ItemKind>,
    preview: bool,
) -> Result<CraftNode, anyhow::Error> {
    anyhow::ensure!(
        active.insert(item_kind),
        "craft recipe cycle detected at {item_kind}"
    );
    let result = (|| {
        let item_recipes = recipes_by_item.get(&item_kind).cloned().unwrap_or_default();
        let selection = selections.get(key);
        if matches!(selection, Some(CraftSelection::Storage)) {
            return Ok(storage_node(item_kind, quantity, key, available, true));
        }
        if matches!(selection, Some(CraftSelection::Any)) {
            return Ok(CraftNode {
                key: key.to_owned(),
                item_kind: item_kind.to_string(),
                quantity,
                children: Vec::new(),
                kind: CraftNodeKind::Any,
            });
        }
        if item_recipes.len() > 1 && selection.is_none() {
            if preview {
                return Ok(CraftNode {
                    key: key.to_owned(),
                    item_kind: item_kind.to_string(),
                    quantity,
                    children: Vec::new(),
                    kind: CraftNodeKind::Deferred,
                });
            }
            let mut options = Vec::new();
            for recipe in &item_recipes {
                let option_key = format!("{key}#{}", recipe.id);
                let preview_node = build_recipe_node(
                    item_kind,
                    quantity,
                    key,
                    &option_key,
                    recipe,
                    selections,
                    recipes_by_item,
                    available,
                    active,
                    true,
                )?;
                options.push(CraftOption::Recipe {
                    recipe_id: recipe.id.to_string(),
                    available: recipe_available(recipe, quantity, available),
                    preview: serde_json::to_value(preview_node)?,
                });
            }
            options.push(CraftOption::Any {
                preview: serde_json::to_value(CraftNode {
                    key: format!("{key}#any"),
                    item_kind: item_kind.to_string(),
                    quantity,
                    children: Vec::new(),
                    kind: CraftNodeKind::Any,
                })?,
            });
            options.push(CraftOption::Storage {
                available: available.get(&item_kind).copied().unwrap_or(0) >= quantity,
                preview: serde_json::to_value(storage_node(
                    item_kind,
                    quantity,
                    &format!("{key}#storage"),
                    available,
                    false,
                ))?,
            });
            return Ok(CraftNode {
                key: key.to_owned(),
                item_kind: item_kind.to_string(),
                quantity,
                children: Vec::new(),
                kind: CraftNodeKind::Choice { options },
            });
        }
        if item_recipes.is_empty() {
            return Ok(storage_node(item_kind, quantity, key, available, false));
        }
        let recipe = match selection {
            Some(CraftSelection::Recipe { recipe_id }) => {
                let recipe_id = Uuid::parse_str(recipe_id)
                    .map_err(|_| anyhow::anyhow!("invalid recipe id {recipe_id}"))?;
                item_recipes
                    .iter()
                    .find(|recipe| recipe.id == recipe_id)
                    .ok_or_else(|| {
                        anyhow::anyhow!("recipe {recipe_id} does not produce {item_kind}")
                    })?
            }
            _ => &item_recipes[0],
        };
        build_recipe_node(
            item_kind,
            quantity,
            key,
            key,
            recipe,
            selections,
            recipes_by_item,
            available,
            active,
            preview,
        )
    })();
    active.remove(&item_kind);
    result
}

fn build_recipe_node(
    item_kind: ItemKind,
    quantity: u32,
    node_key: &str,
    child_key: &str,
    recipe: &Recipe,
    selections: &HashMap<String, CraftSelection>,
    recipes_by_item: &HashMap<ItemKind, Vec<Recipe>>,
    available: &HashMap<ItemKind, u32>,
    active: &mut HashSet<ItemKind>,
    preview: bool,
) -> Result<CraftNode, anyhow::Error> {
    let crafts = quantity.div_ceil(recipe.output_yield);
    let children = recipe
        .ingredients
        .iter()
        .enumerate()
        .map(|(index, ingredient)| {
            let key = format!("{child_key}.{index}");
            build_node(
                ingredient.item_kind,
                ingredient.quantity.saturating_mul(crafts),
                &key,
                selections,
                recipes_by_item,
                available,
                active,
                preview,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CraftNode {
        key: node_key.to_owned(),
        item_kind: item_kind.to_string(),
        quantity,
        children,
        kind: CraftNodeKind::Recipe {
            recipe_id: recipe.id.to_string(),
            crafts,
            output_yield: recipe.output_yield,
            selected: selections.get(node_key).is_some(),
        },
    })
}

fn storage_node(
    item_kind: ItemKind,
    quantity: u32,
    key: &str,
    available: &HashMap<ItemKind, u32>,
    selected: bool,
) -> CraftNode {
    CraftNode {
        key: key.to_owned(),
        item_kind: item_kind.to_string(),
        quantity,
        children: Vec::new(),
        kind: CraftNodeKind::Storage {
            available: available.get(&item_kind).copied().unwrap_or(0),
            selected,
        },
    }
}

fn recipe_available(recipe: &Recipe, quantity: u32, available: &HashMap<ItemKind, u32>) -> bool {
    let crafts = quantity.div_ceil(recipe.output_yield);
    recipe.ingredients.iter().all(|ingredient| {
        available.get(&ingredient.item_kind).copied().unwrap_or(0)
            >= ingredient.quantity.saturating_mul(crafts)
    })
}

fn contains_choice(node: &CraftNode) -> bool {
    matches!(node.kind, CraftNodeKind::Choice { .. }) || node.children.iter().any(contains_choice)
}

fn collect_raw_materials(node: &CraftNode, raw: &mut HashMap<String, u32>) {
    if matches!(
        node.kind,
        CraftNodeKind::Storage { .. } | CraftNodeKind::Any
    ) {
        *raw.entry(node.item_kind.clone()).or_default() += node.quantity;
    }
    for child in &node.children {
        collect_raw_materials(child, raw);
    }
}

pub fn recipe_ingredients(recipe: &Recipe) -> Vec<CraftIngredient> {
    recipe.ingredients.clone()
}

pub fn execution_tree(
    records: Vec<CraftJobNodeRecord>,
) -> Result<CraftExecutionNode, anyhow::Error> {
    let root_ids = records
        .iter()
        .filter(|record| record.parent_id.is_none())
        .map(|record| record.spec.id)
        .collect::<Vec<_>>();
    anyhow::ensure!(
        root_ids.len() == 1,
        "craft job must have exactly one root node"
    );
    let root_id = root_ids[0];
    let mut children = HashMap::<Uuid, Vec<(i32, Uuid)>>::new();
    let mut records = records
        .into_iter()
        .map(|record| {
            if let Some(parent_id) = record.parent_id {
                children
                    .entry(parent_id)
                    .or_default()
                    .push((record.child_order, record.spec.id));
            }
            (record.spec.id, record)
        })
        .collect::<HashMap<_, _>>();
    for child_ids in children.values_mut() {
        child_ids.sort_by_key(|(child_order, _)| *child_order);
    }
    build_execution_tree(root_id, &mut records, &children)
}

fn build_execution_tree(
    id: Uuid,
    records: &mut HashMap<Uuid, CraftJobNodeRecord>,
    children: &HashMap<Uuid, Vec<(i32, Uuid)>>,
) -> Result<CraftExecutionNode, anyhow::Error> {
    let record = records
        .remove(&id)
        .ok_or_else(|| anyhow::anyhow!("craft node {id} was not found"))?;
    let node_children = children
        .get(&id)
        .into_iter()
        .flatten()
        .map(|(_, child_id)| build_execution_tree(*child_id, records, children))
        .collect::<Result<Vec<_>, _>>()?;
    let kind = match record.kind {
        CraftJobNodeData::Recipe {
            recipe,
            completed_crafts,
        } => CraftExecutionNodeKind::Recipe {
            recipe_id: recipe.recipe_id.to_string(),
            engine_type: recipe.engine_type.as_str().to_owned(),
            planned_crafts: recipe.planned_crafts,
            completed_crafts,
            output_yield: recipe.output_yield,
        },
        CraftJobNodeData::Storage => CraftExecutionNodeKind::Storage,
        CraftJobNodeData::Any => CraftExecutionNodeKind::Any,
    };
    let operation = match record.operation {
        Some(operation) => Some(CraftExecutionOperation {
            kind: operation.kind.as_str().to_owned(),
            state: operation.state.as_str().to_owned(),
            completed_amount: u32::try_from(operation.completed_amount)?,
            error: operation.error,
        }),
        None => None,
    };
    Ok(CraftExecutionNode {
        key: record.spec.key,
        item_kind: record.spec.item_kind,
        required_quantity: record.spec.required_quantity,
        completed_quantity: u32::try_from(record.completed_quantity)?,
        state: record.state.as_str().to_owned(),
        assigned_region_id: record.assigned_region_id.map(|id| id.to_string()),
        error: record.error,
        operation,
        children: node_children,
        kind,
    })
}

pub async fn execution_nodes(
    db: &DatabaseHandle,
    root: &CraftNode,
) -> Result<Vec<NewCraftJobNode>, anyhow::Error> {
    execution_nodes_from_recipes(root, db.fetch_all_recipes().await?)
}

fn execution_nodes_from_recipes(
    root: &CraftNode,
    recipes: Vec<Recipe>,
) -> Result<Vec<NewCraftJobNode>, anyhow::Error> {
    let recipes = recipes
        .into_iter()
        .map(|recipe| (recipe.id, recipe))
        .collect::<HashMap<_, _>>();
    let mut nodes = Vec::new();
    append_execution_node(root, None, 0, &recipes, &mut nodes)?;
    Ok(nodes)
}

fn append_execution_node(
    node: &CraftNode,
    parent_id: Option<Uuid>,
    child_order: i32,
    recipes: &HashMap<Uuid, Recipe>,
    nodes: &mut Vec<NewCraftJobNode>,
) -> Result<(), anyhow::Error> {
    let id = Uuid::new_v4();
    let execution_node = match &node.kind {
        CraftNodeKind::Recipe {
            recipe_id,
            crafts,
            output_yield,
            ..
        } => {
            let recipe_id = Uuid::parse_str(recipe_id)?;
            let recipe = recipes
                .get(&recipe_id)
                .ok_or_else(|| anyhow::anyhow!("recipe {recipe_id} disappeared before queueing"))?;
            anyhow::ensure!(
                recipe.output_yield == *output_yield,
                "recipe {recipe_id} changed while queueing"
            );
            NewCraftJobNode {
                spec: CraftNodeSpec {
                    id,
                    key: node.key.clone(),
                    item_kind: node.item_kind.clone(),
                    required_quantity: node.quantity,
                },
                parent_id,
                child_order,
                kind: NewCraftJobNodeKind::Recipe(crate::db::craft_job::CraftRecipeSnapshot {
                    recipe_id,
                    engine_type: recipe.engine_type,
                    ingredients: recipe.ingredients.clone(),
                    planned_crafts: *crafts,
                    output_yield: *output_yield,
                }),
            }
        }
        CraftNodeKind::Storage { .. } => NewCraftJobNode {
            spec: CraftNodeSpec {
                id,
                key: node.key.clone(),
                item_kind: node.item_kind.clone(),
                required_quantity: node.quantity,
            },
            parent_id,
            child_order,
            kind: NewCraftJobNodeKind::Storage,
        },
        CraftNodeKind::Any => NewCraftJobNode {
            spec: CraftNodeSpec {
                id,
                key: node.key.clone(),
                item_kind: node.item_kind.clone(),
                required_quantity: node.quantity,
            },
            parent_id,
            child_order,
            kind: NewCraftJobNodeKind::Any,
        },
        CraftNodeKind::Choice { .. } | CraftNodeKind::Deferred => {
            anyhow::bail!("craft plan contains unresolved choices")
        }
    };
    nodes.push(execution_node);
    for (index, child) in node.children.iter().enumerate() {
        append_execution_node(child, Some(id), i32::try_from(index)?, recipes, nodes)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::db::craft_job::{
        CraftJobOperationRecord, CraftNodeState, CraftOperationKind, CraftOperationState,
    };

    use super::*;

    fn recipe(id: u128, output: &str, output_yield: u32, ingredients: &[(&str, u32)]) -> Recipe {
        Recipe {
            id: Uuid::from_u128(id),
            output_item_kind: ItemKind::from_str(output).unwrap(),
            output_yield,
            engine_type: crate::db::crafting_engine::CraftingEngineType::CraftingTable,
            ingredients: ingredients
                .iter()
                .map(|(item_kind, quantity)| CraftIngredient {
                    item_kind: ItemKind::from_str(item_kind).unwrap(),
                    quantity: *quantity,
                })
                .collect(),
        }
    }

    fn node(
        item_kind: &str,
        quantity: u32,
        recipes: &[Recipe],
        selections: HashMap<String, CraftSelection>,
        available: &[(&str, u32)],
    ) -> Result<CraftNode, anyhow::Error> {
        let recipes_by_item = recipes
            .iter()
            .cloned()
            .fold(HashMap::new(), |mut map, recipe| {
                map.entry(recipe.output_item_kind)
                    .or_insert_with(Vec::new)
                    .push(recipe);
                map
            });
        let available = available
            .iter()
            .map(|(item, quantity)| (ItemKind::from_str(item).unwrap(), *quantity))
            .collect();
        build_node(
            ItemKind::from_str(item_kind).unwrap(),
            quantity,
            "0",
            &selections,
            &recipes_by_item,
            &available,
            &mut HashSet::new(),
            false,
        )
    }

    #[test]
    fn expands_recipe_quantity_using_ceiling_crafts() {
        let root = node(
            "minecraft:hopper",
            5,
            &[recipe(
                1,
                "minecraft:hopper",
                2,
                &[("minecraft:iron_ingot", 3)],
            )],
            HashMap::new(),
            &[("minecraft:iron_ingot", 20)],
        )
        .unwrap();

        let CraftNodeKind::Recipe { crafts, .. } = root.kind else {
            panic!("expected recipe root")
        };
        assert_eq!(crafts, 3);
        assert_eq!(root.children[0].quantity, 9);
    }

    #[test]
    fn returns_unresolved_choice_for_multiple_recipes() {
        let root = node(
            "minecraft:hopper",
            1,
            &[
                recipe(1, "minecraft:hopper", 1, &[("minecraft:stone", 1)]),
                recipe(2, "minecraft:hopper", 1, &[("minecraft:oak_log", 1)]),
            ],
            HashMap::new(),
            &[("minecraft:stone", 1)],
        )
        .unwrap();

        let CraftNodeKind::Choice { options } = &root.kind else {
            panic!("expected choice root")
        };
        assert_eq!(options.len(), 4);
        assert!(matches!(
            options[0],
            CraftOption::Recipe {
                available: true,
                ..
            }
        ));
        assert!(matches!(
            options[1],
            CraftOption::Recipe {
                available: false,
                ..
            }
        ));
        assert!(contains_choice(&root));
    }

    #[test]
    fn explicit_storage_selection_is_marked_selected() {
        let mut selections = HashMap::new();
        selections.insert("0.0".to_owned(), CraftSelection::Storage);
        let root = node(
            "minecraft:hopper",
            1,
            &[recipe(
                1,
                "minecraft:hopper",
                1,
                &[("minecraft:iron_ingot", 1)],
            )],
            selections,
            &[("minecraft:iron_ingot", 1)],
        )
        .unwrap();

        let CraftNodeKind::Storage {
            selected,
            available,
        } = root.children[0].kind
        else {
            panic!("expected storage child")
        };
        assert!(selected);
        assert_eq!(available, 1);
    }

    #[test]
    fn explicit_any_selection_becomes_raw_material() {
        let mut selections = HashMap::new();
        selections.insert("0".to_owned(), CraftSelection::Any);
        let root = node(
            "minecraft:hopper",
            1,
            &[recipe(
                1,
                "minecraft:hopper",
                1,
                &[("minecraft:iron_ingot", 2)],
            )],
            selections,
            &[],
        )
        .unwrap();

        assert!(matches!(root.kind, CraftNodeKind::Any));
        let mut raw = HashMap::new();
        collect_raw_materials(&root, &mut raw);
        assert_eq!(raw.get("minecraft:hopper"), Some(&1));
    }

    #[test]
    fn rejects_recipe_cycles() {
        let error = node(
            "minecraft:stick",
            1,
            &[
                recipe(1, "minecraft:stick", 1, &[("minecraft:oak_planks", 1)]),
                recipe(2, "minecraft:oak_planks", 1, &[("minecraft:stick", 1)]),
            ],
            HashMap::new(),
            &[],
        )
        .unwrap_err();
        assert!(error.to_string().contains("cycle detected"));
    }

    #[test]
    fn rejects_unknown_explicit_recipe() {
        let mut selections = HashMap::new();
        selections.insert(
            "0".to_owned(),
            CraftSelection::Recipe {
                recipe_id: Uuid::from_u128(99).to_string(),
            },
        );
        let error = node(
            "minecraft:hopper",
            1,
            &[recipe(
                1,
                "minecraft:hopper",
                1,
                &[("minecraft:iron_ingot", 1)],
            )],
            selections,
            &[],
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("does not produce minecraft:hopper")
        );
    }

    #[test]
    fn execution_nodes_preserve_tree_and_recipe_snapshot() {
        let recipes = vec![
            recipe(1, "minecraft:hopper", 2, &[("minecraft:chest", 3)]),
            recipe(2, "minecraft:chest", 1, &[("minecraft:oak_planks", 4)]),
        ];
        let root = node("minecraft:hopper", 5, &recipes, HashMap::new(), &[]).unwrap();

        let nodes = execution_nodes_from_recipes(&root, recipes).unwrap();

        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0].spec.key, "0");
        let NewCraftJobNodeKind::Recipe(recipe) = &nodes[0].kind else {
            panic!("expected recipe node")
        };
        assert_eq!(recipe.planned_crafts, 3);
        assert_eq!(recipe.engine_type.as_str(), "crafting_table");
        assert_eq!(recipe.ingredients[0].quantity, 3);
        assert_eq!(nodes[1].parent_id, Some(nodes[0].spec.id));
        assert_eq!(nodes[2].parent_id, Some(nodes[1].spec.id));
        assert!(matches!(nodes[2].kind, NewCraftJobNodeKind::Storage));
        assert_eq!(nodes[2].spec.required_quantity, 36);
    }

    #[test]
    fn execution_nodes_reject_unresolved_choices() {
        let recipes = vec![
            recipe(1, "minecraft:hopper", 1, &[("minecraft:stone", 1)]),
            recipe(2, "minecraft:hopper", 1, &[("minecraft:oak_log", 1)]),
        ];
        let root = node("minecraft:hopper", 1, &recipes, HashMap::new(), &[]).unwrap();

        let error = execution_nodes_from_recipes(&root, recipes).unwrap_err();

        assert!(error.to_string().contains("unresolved choices"));
    }

    #[test]
    fn rebuilds_persisted_execution_tree_with_progress() {
        let root_id = Uuid::new_v4();
        let first_id = Uuid::new_v4();
        let second_id = Uuid::new_v4();
        let record = |id, parent_id, key: &str, child_order, item_kind: &str| CraftJobNodeRecord {
            spec: CraftNodeSpec {
                id,
                key: key.to_owned(),
                item_kind: item_kind.to_owned(),
                required_quantity: 10,
            },
            parent_id,
            child_order,
            kind: if parent_id.is_none() {
                CraftJobNodeData::Recipe {
                    recipe: crate::db::craft_job::CraftRecipeSnapshot {
                        recipe_id: Uuid::from_u128(1),
                        engine_type: crate::db::crafting_engine::CraftingEngineType::CraftingTable,
                        ingredients: Vec::new(),
                        planned_crafts: 10,
                        output_yield: 1,
                    },
                    completed_crafts: 4,
                }
            } else {
                CraftJobNodeData::Storage
            },
            completed_quantity: if parent_id.is_none() { 4 } else { 10 },
            state: if parent_id.is_none() {
                CraftNodeState::Running
            } else {
                CraftNodeState::Completed
            },
            assigned_region_id: None,
            error: None,
            operation: parent_id.is_none().then_some(CraftJobOperationRecord {
                kind: CraftOperationKind::Craft,
                state: CraftOperationState::Running,
                completed_amount: 4,
                error: None,
            }),
        };

        let root = execution_tree(vec![
            record(second_id, Some(root_id), "0.1", 1, "second"),
            record(root_id, None, "0", 0, "widget"),
            record(first_id, Some(root_id), "0.0", 0, "first"),
        ])
        .unwrap();

        assert_eq!(root.completed_quantity, 4);
        assert_eq!(root.children[0].item_kind, "first");
        assert_eq!(root.children[1].item_kind, "second");
        assert_eq!(root.operation.unwrap().completed_amount, 4);
        let CraftExecutionNodeKind::Recipe {
            planned_crafts,
            completed_crafts,
            ..
        } = root.kind
        else {
            panic!("expected recipe root")
        };
        assert_eq!(planned_crafts, 10);
        assert_eq!(completed_crafts, 4);
    }
}
