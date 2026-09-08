use common::work_units::craft::CraftIngredient;
use uuid::Uuid;

use super::DatabaseHandle;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RecipeRow {
    pub id: Uuid,
    pub output_item_kind: String,
    pub output_yield: i32,
    pub engine_type: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RecipeIngredientRow {
    pub recipe_id: Uuid,
    pub item_kind: String,
    pub quantity: i32,
    pub position: i32,
}

#[derive(Debug, Clone)]
pub struct Recipe {
    pub id: Uuid,
    pub output_item_kind: String,
    pub output_yield: u32,
    pub engine_type: String,
    pub ingredients: Vec<CraftIngredient>,
}

impl DatabaseHandle {
    pub async fn fetch_all_recipes(&self) -> Result<Vec<Recipe>, sqlx::Error> {
        self.fetch_recipes(None).await
    }

    pub async fn fetch_recipes_for(&self, item_kind: &str) -> Result<Vec<Recipe>, sqlx::Error> {
        self.fetch_recipes(Some(item_kind)).await
    }

    async fn fetch_recipes(&self, item_kind: Option<&str>) -> Result<Vec<Recipe>, sqlx::Error> {
        let rows = sqlx::query_as!(
            RecipeRow,
            "select id, output_item_kind, output_yield, engine_type from recipe where ($1::text is null or output_item_kind = $1) order by output_item_kind, id",
            item_kind,
        )
        .fetch_all(&self.pool)
        .await?;
        let ingredients = sqlx::query_as!(
            RecipeIngredientRow,
            "select recipe_id, item_kind, quantity, position from recipe_ingredient order by recipe_id, position",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut by_recipe = std::collections::HashMap::<Uuid, Vec<_>>::new();
        for ingredient in ingredients {
            let quantity = u32::try_from(ingredient.quantity).map_err(|_| {
                sqlx::Error::Protocol("recipe ingredient quantity must be positive".into())
            })?;
            by_recipe
                .entry(ingredient.recipe_id)
                .or_default()
                .push(CraftIngredient {
                    item_kind: ingredient.item_kind,
                    quantity,
                });
        }
        rows.into_iter()
            .map(|row| {
                Ok(Recipe {
                    id: row.id,
                    output_item_kind: row.output_item_kind,
                    output_yield: u32::try_from(row.output_yield).map_err(|_| {
                        sqlx::Error::Protocol("recipe yield must be positive".into())
                    })?,
                    engine_type: row.engine_type,
                    ingredients: by_recipe.remove(&row.id).unwrap_or_default(),
                })
            })
            .collect()
    }
}
