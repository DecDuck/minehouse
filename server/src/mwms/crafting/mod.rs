use std::collections::{HashMap, HashSet};

use common::work_units::craft::CraftIngredient;
use oasgen::OaSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::db::{DatabaseHandle, recipe::Recipe};

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
#[serde(rename_all = "lowercase")]
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
    let recipes = db.fetch_all_recipes().await?;
    let mut recipes_by_item: HashMap<String, Vec<Recipe>> = HashMap::new();
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
            .entry(recipe.output_item_kind.clone())
            .or_default()
            .push(recipe);
    }
    let mut item_kinds = recipes_by_item.keys().cloned().collect::<HashSet<_>>();
    item_kinds.insert(request.item_kind.clone());
    for recipe_list in recipes_by_item.values() {
        for recipe in recipe_list {
            item_kinds.extend(
                recipe
                    .ingredients
                    .iter()
                    .map(|ingredient| ingredient.item_kind.clone()),
            );
        }
    }
    let mut available = HashMap::new();
    for item_kind in item_kinds {
        let quantity = db
            .fetch_item_kind_stacks(&item_kind)
            .await?
            .into_iter()
            .map(|stack| stack.quantity.max(0) as u32)
            .sum();
        available.insert(item_kind.clone(), quantity);
    }
    let root = build_node(
        &request.item_kind,
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
        .map(|(item_kind, quantity)| RawMaterialRequirement {
            available: available.get(&item_kind).copied().unwrap_or(0),
            item_kind,
            quantity,
        })
        .collect();
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
    item_kind: &str,
    quantity: u32,
    key: &str,
    selections: &HashMap<String, CraftSelection>,
    recipes_by_item: &HashMap<String, Vec<Recipe>>,
    available: &HashMap<String, u32>,
    active: &mut HashSet<String>,
    preview: bool,
) -> Result<CraftNode, anyhow::Error> {
    anyhow::ensure!(
        active.insert(item_kind.to_owned()),
        "craft recipe cycle detected at {item_kind}"
    );
    let result = (|| {
        let item_recipes = recipes_by_item.get(item_kind).cloned().unwrap_or_default();
        let selection = selections.get(key);
        if matches!(selection, Some(CraftSelection::Storage)) {
            return Ok(storage_node(item_kind, quantity, key, available, true));
        }
        if matches!(selection, Some(CraftSelection::Any)) {
            return Ok(CraftNode {
                key: key.to_owned(),
                item_kind: item_kind.to_owned(),
                quantity,
                children: Vec::new(),
                kind: CraftNodeKind::Any,
            });
        }
        if item_recipes.len() > 1 && selection.is_none() {
            if preview {
                return Ok(CraftNode {
                    key: key.to_owned(),
                    item_kind: item_kind.to_owned(),
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
                    item_kind: item_kind.to_owned(),
                    quantity,
                    children: Vec::new(),
                    kind: CraftNodeKind::Any,
                })?,
            });
            options.push(CraftOption::Storage {
                available: available.get(item_kind).copied().unwrap_or(0) >= quantity,
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
                item_kind: item_kind.to_owned(),
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
    active.remove(item_kind);
    result
}

fn build_recipe_node(
    item_kind: &str,
    quantity: u32,
    node_key: &str,
    child_key: &str,
    recipe: &Recipe,
    selections: &HashMap<String, CraftSelection>,
    recipes_by_item: &HashMap<String, Vec<Recipe>>,
    available: &HashMap<String, u32>,
    active: &mut HashSet<String>,
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
                &ingredient.item_kind,
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
        item_kind: item_kind.to_owned(),
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
    item_kind: &str,
    quantity: u32,
    key: &str,
    available: &HashMap<String, u32>,
    selected: bool,
) -> CraftNode {
    CraftNode {
        key: key.to_owned(),
        item_kind: item_kind.to_owned(),
        quantity,
        children: Vec::new(),
        kind: CraftNodeKind::Storage {
            available: available.get(item_kind).copied().unwrap_or(0),
            selected,
        },
    }
}

fn recipe_available(recipe: &Recipe, quantity: u32, available: &HashMap<String, u32>) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn recipe(id: u128, output: &str, output_yield: u32, ingredients: &[(&str, u32)]) -> Recipe {
        Recipe {
            id: Uuid::from_u128(id),
            output_item_kind: output.to_owned(),
            output_yield,
            engine_type: "crafting_table".to_owned(),
            ingredients: ingredients
                .iter()
                .map(|(item_kind, quantity)| CraftIngredient {
                    item_kind: (*item_kind).to_owned(),
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
                map.entry(recipe.output_item_kind.clone())
                    .or_insert_with(Vec::new)
                    .push(recipe);
                map
            });
        let available = available
            .iter()
            .map(|(item, quantity)| ((*item).to_owned(), *quantity))
            .collect();
        build_node(
            item_kind,
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
            "widget",
            5,
            &[recipe(1, "widget", 2, &[("material", 3)])],
            HashMap::new(),
            &[("material", 20)],
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
            "widget",
            1,
            &[
                recipe(1, "widget", 1, &[("stone", 1)]),
                recipe(2, "widget", 1, &[("wood", 1)]),
            ],
            HashMap::new(),
            &[("stone", 1)],
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
            "widget",
            1,
            &[recipe(1, "widget", 1, &[("material", 1)])],
            selections,
            &[("material", 1)],
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
            "widget",
            1,
            &[recipe(1, "widget", 1, &[("material", 2)])],
            selections,
            &[],
        )
        .unwrap();

        assert!(matches!(root.kind, CraftNodeKind::Any));
        let mut raw = HashMap::new();
        collect_raw_materials(&root, &mut raw);
        assert_eq!(raw.get("widget"), Some(&1));
    }

    #[test]
    fn rejects_recipe_cycles() {
        let error = node(
            "a",
            1,
            &[
                recipe(1, "a", 1, &[("b", 1)]),
                recipe(2, "b", 1, &[("a", 1)]),
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
            "widget",
            1,
            &[recipe(1, "widget", 1, &[("material", 1)])],
            selections,
            &[],
        )
        .unwrap_err();
        assert!(error.to_string().contains("does not produce widget"));
    }
}
