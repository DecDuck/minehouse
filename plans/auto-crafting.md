# End-to-End Auto-Crafting Plan

## Goal

Replace the mock-only frontend crafting flow with a backend-driven pipeline:

- Recipes are stored in the database and seeded from the current frontend catalog.
- The backend owns a selection-aware craft-tree resolver.
- The existing transfer system gathers ingredients into a processing area and stores outputs afterward.
- A new craft work unit is distributed to warehouse workers.
- The actual Azalea in-game crafting interaction is stubbed initially, but the work-unit boundary supports implementing it later.

## Decisions

- **Recipe source:** New database tables, manually seeded by porting the catalog from `frontend/app/composables/useCrafting.ts`.
- **Execution scope:** Full pipeline design, with the physical in-game craft action stubbed initially.
- **Resolver ownership:** Backend owns a pure, selection-aware resolver. The frontend retains interactive per-node choices.
- **Ingredient movement:** Reuse `TransferWorkUnit` and the existing storage endpoint/planner system.
- **Unresolved choices:** Submission is blocked while any `Choice` node remains unresolved, matching the mock's behavior.
- **Any choice:** `Any` is only used when explicitly selected by the user. It is never an implicit fallback for unresolved choices.

## Current Architecture

- Work units are `IndexRegion`, `IndexContainer`, and `Transfer` in `common/src/work.rs`.
- The planner directly queues work units, waits for completion, and persists snapshots. There is no current server-side `WorkUnitAction` layer.
- `PlannerRequest` currently contains `IndexRegions`, `CycleCount`, and `Putaway` and is exposed through `POST /api/v1/queue`.
- Planner storage endpoints currently cover Bulk and Putaway regions. Processing regions are skipped.
- `crafting_engine` and `generic_crafting_engine` model engine positions but are currently unused.
- `planner/putaway.rs` provides the template for lowering logical movement plans into `TransferWorkUnit`s.
- The frontend mock contains the recipe catalog, recursive tree resolver, selections, mock queue, and progress display.

## Data Type Scaffold

These are the proposed contracts before implementation. Keep database rows, planner/API DTOs, and worker wire types separate where their lifetimes or validation rules differ.

### Shared Rust Types (`common`)

Use stable UUIDs for recipes and path keys for tree selections:

```rust
pub type RecipeId = uuid::Uuid;
pub type CraftNodeKey = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftIngredient {
   pub item_kind: String,
   pub quantity: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CraftSelection {
   Recipe { recipe_id: RecipeId },
   Storage,
   Any,
}
```

Add `CraftWorkUnit` in `common/src/work_units/craft.rs`. It must be self-contained enough for a worker to resume after reconnecting:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftRecipe {
   pub recipe_id: RecipeId,
   pub output_item_kind: String,
   pub output_yield: u32,
   pub ingredients: Vec<CraftIngredient>,
   pub crafts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftLocations {
   pub engine: Point,
   pub input_container: Point,
   pub output_container: Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftProgress {
   pub completed_crafts: u32,
   pub input_snapshot: Option<Vec<ItemStack>>,
   pub output_snapshot: Option<Vec<ItemStack>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CraftWorkUnit {
   pub recipe: CraftRecipe,
   pub locations: CraftLocations,
   pub progress: CraftProgress,
}

impl WorkUnitDone for CraftWorkUnit {
   fn is_done(&self) -> bool {
      self.progress.completed_crafts == self.recipe.crafts
         && self.progress.output_snapshot.is_some()
   }
}
```

Then add `Craft(CraftWorkUnit)` to `WorkUnitData`. `CraftRecipe` is immutable execution input, `CraftLocations` identifies the physical route, and `CraftProgress` contains resumable state. Snapshots are the worker's last accepted container state rather than an implicit database transaction.

### Database Models (`server`)

Keep SQL-facing rows simple and close to the migration:

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RecipeRow {
   pub id: RecipeId,
   pub output_item_kind: String,
   pub output_yield: i32,
   pub engine_type: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct RecipeIngredientRow {
   pub recipe_id: RecipeId,
   pub item_kind: String,
   pub quantity: i32,
   pub position: i32,
}
```

The database layer should convert these rows into a validated domain recipe:

```rust
pub struct Recipe {
   pub id: RecipeId,
   pub output_item_kind: String,
   pub output_yield: u32,
   pub engine_type: CraftingEngineType,
   pub ingredients: Vec<CraftIngredient>,
}
```

`CraftingEngineType` should be an application enum matching the database enum (`CraftingTable`, `Smithing`, `Stonecutter`, or the existing generic/empty type). Reject zero yields, zero ingredient counts, duplicate positions, and missing ingredients while loading recipes.

### Resolver and API DTOs (`server/src/mwms/crafting`)

The resolver should return one serializable shape for both preview and planner validation:

```rust
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
#[serde(tag = "type")]
pub enum CraftNodeKind {
   Recipe {
      recipe_id: RecipeId,
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
#[serde(tag = "type")]
pub enum CraftOption {
   Recipe {
      recipe_id: RecipeId,
      preview: Box<CraftNode>,
      available: bool,
   },
   Storage {
      preview: Box<CraftNode>,
      available: bool,
   },
   Any {
      preview: Box<CraftNode>,
   },
}
```

The plan request and queue request should use the same selection map:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
pub struct CraftPlanRequest {
   pub item_kind: String,
   pub amount: u32,
   pub selections: std::collections::HashMap<CraftNodeKey, CraftSelection>,
}

pub enum PlannerRequest {
   IndexRegions,
   CycleCount,
   Putaway,
   Craft(CraftPlanRequest),
}
```

`POST /api/v1/crafting/plan` accepts `CraftPlanRequest` and returns `CraftPlan`. `POST /api/v1/queue` carries the same `CraftPlanRequest` inside the Craft planner variant, so the planner can re-resolve and validate the exact selections against live storage.

### Worker Status Types

If the queue UI needs progress without exposing raw work-pool internals, project it into a dedicated status DTO:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
pub struct CraftJobStatus {
   pub id: uuid::Uuid,
   pub target_item_kind: String,
   pub target_quantity: u32,
   pub state: CraftJobState,
   pub completed_quantity: u32,
   pub total_quantity: u32,
   pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, OaSchema)]
pub enum CraftJobState {
   Queued,
   Running,
   WaitingForChoice,
   Completed,
   Failed,
}
```

The frontend can map `CraftNode` directly to its existing `CraftNode` union, replacing positional `recipeIndex` with `recipeId` and adding `available` quantities to option previews. The existing `CraftSelection` shape remains conceptually unchanged, so `CraftTreeGraph.vue` does not need to own resolver logic.

## Implementation Phases

### Phase 0: Recipe Database Model and Seed

1. Add `server/migrations/3_recipes.sql` with:
   - `recipe(id, output_item_kind, yield, engine_type)`.
   - `recipe_ingredient(recipe_id, item_kind, count, position)`.
   - Foreign keys, positive-count checks, and an index on output item kind.
   - Support for multiple recipes producing the same item.
2. Add `server/src/db/recipe.rs` with `Recipe` and `RecipeIngredient` models and methods such as:
   - `fetch_all_recipes()`.
   - `fetch_recipes_for(item_kind)`.
   - Recipe insertion/seed helpers if needed.
3. Register the database module in `server/src/db/mod.rs`.
4. Port the frontend catalog, including custom `minehouse:` recipes and multi-recipe items, into migration seed data.
5. Add database loading for crafting engines in `server/src/db/crafting_engine.rs` so the planner can select engines by role.

### Phase 1: Selection-Aware Backend Resolver

Create `server/src/mwms/crafting/` with a resolver that ports the behavior of the frontend `buildTree` function.

The resolver should be stateless:

```text
resolve(target, amount, selections, storage_snapshot) -> ResolvedTree
```

The shared selection model should use stable recipe IDs rather than positional recipe indexes:

```text
Recipe { recipe_id }
Storage
Any
```

The resolved node model should support:

- Recipe nodes.
- Storage nodes.
- Choice nodes containing recipe, storage, and explicit-any options.
- Any nodes.
- Deferred preview nodes where needed.

The backend owns deterministic node-key generation using stable path-based keys. The frontend sends those keys back with each selection request, preventing frontend/backend tree-key drift.

Behavior:

1. Use an explicit selection when one exists.
2. Return unresolved `Choice` nodes for ambiguous items without a selection.
3. Preserve the mock's recursive quantity expansion: `ceil(quantity / yield)` crafts and multiplied ingredient quantities.
4. Use current storage data to show whether each option is sourceable and how much is available.
5. Add cycle detection and clear validation errors.
6. Return a serializable `ResolvedTree`/`CraftPlan` shared by the preview endpoint and planner.

### Phase 2: Craft Work Unit and Worker Stub

1. Add `common/src/work_units/craft.rs` with a serializable `CraftWorkUnit` containing:
   - Recipe ID and craft count.
   - Engine position.
   - Input and output container positions.
   - Checkpoint/progress fields.
   - Output snapshot.
2. Add the `Craft` variant to `WorkUnitData` and export the module.
3. Add `worker-warehouse/src/craft.rs`.
4. Implement the initial worker behavior as a stub:
   - Move to the selected engine.
   - Consume the declared inputs from the processing container.
   - Synthesize the expected output stack.
   - Submit the completed snapshot.
5. Wire Craft into `worker-warehouse/src/imple.rs`, including distance selection and dispatch.
6. Leave the physical Azalea recipe interaction behind this work-unit boundary for a later implementation.

### Phase 3: Planner Integration

1. Extend `PlannerRequest` with:

```text
Craft {
    item_kind,
    amount,
    selections
}
```

2. Add a Craft branch to planner request handling.
3. Add `server/src/mwms/planner/craft.rs` to:
   - Resolve the submitted selections against live storage.
   - Reject the request if any `Choice` remains unresolved.
   - Validate explicit `Any` selections against live availability.
   - Select a processing region/container and compatible crafting engine.
   - Gather ingredients from Bulk into Processing using existing transfer planning.
   - Queue and await Craft work units in dependency order.
   - Move outputs back into storage using the existing putaway flow.
   - Persist container snapshots after each completed stage.
4. Extend planner endpoint synchronization to support Processing storage, either through a dedicated endpoint implementation or a narrowly scoped staging endpoint.
5. Assign crafting engines using the loaded engine records and their processing role.

### Phase 4: HTTP API and Generated Types

1. Keep the existing queue endpoint for submission. Its `PlannerRequest` schema will gain the Craft variant.
2. Add `server/src/api/crafting.rs` with:
   - `GET /api/v1/recipes` for the item/recipe catalog.
   - `POST /api/v1/crafting/plan` accepting `{ item, amount, selections }` and returning the resolved tree, raw materials, option availability, and unresolved status.
   - Optional `GET /api/v1/crafting` for active craft jobs and progress.
3. Register the handlers in `server/src/api/mod.rs` and `server/src/main.rs`.
4. Regenerate `frontend/openapi.yaml` and `frontend/app/api-types.ts`.

The plan endpoint must be `POST`, not `GET`, because selections are a structured request body and the endpoint is called after each interactive choice.

### Phase 5: Frontend Integration

1. Update `useCrafting.ts` to remove the hardcoded recipe catalog and mock queue.
2. Load item kinds and recipes from `GET /api/v1/recipes`.
3. Replace the local tree builder with calls to `POST /api/v1/crafting/plan` using the current `{ item, amount, selections }` state.
4. Preserve the current interactive flow:
   - `CraftTreeGraph.vue` emits recipe, storage, and any selections.
   - Each selection triggers a new plan request.
   - Newly exposed nested choices appear in the refreshed tree.
   - Unselecting a branch removes that branch and descendant selections.
5. Map the backend tree into the existing `CraftTree`, `CraftTreeGraph`, and `CraftOptionPreview` component shapes where practical.
6. Display the backend-provided storage availability and option metadata.
7. Block confirmation while the plan has unresolved choices.
8. Submit the complete selection map through:

```text
POST /api/v1/queue
{
  "Craft": {
    "item_kind": "...",
    "amount": 1,
    "selections": { ... }
  }
}
```

9. Replace mock queue/progress values with backend status polling or a dedicated craft-status endpoint.
10. Keep `ItemDetailModal.vue` using the existing real storage lookup.

## Validation

1. Run `cargo check` and focused tests for `common`, `server`, and `worker-warehouse` after each backend phase.
2. Apply the migration and verify seeded recipes and engine records against the development database.
3. Unit-test the resolver for:
   - Existing storage satisfying a leaf.
   - Shortfall-only crafting.
   - Multiple recipe choices.
   - Explicit storage and any selections.
   - Unresolved choices.
   - Cycles and missing recipes.
4. Verify `POST /api/v1/crafting/plan` returns the expected tree and unresolved state.
5. Verify the Craft queue request is accepted and appears in the planner/work pool.
6. Run the planner with the stub worker and confirm transfers, craft units, output snapshots, and persistence complete in dependency order.
7. Run frontend type checking/build and verify the interactive graph can resolve every choice before submission.

## Remaining Design Choices

These do not change the interactive selection contract:

1. **Recipe tie-breaking:** availability-first is recommended; alternatives are cheapest-depth or first-defined.
2. **Processing storage:** a dedicated Processing `StorageEndpoint` is recommended over a hardcoded staging container.
3. **Craft status:** expose a dedicated status projection rather than leaking raw work-pool internals to the frontend.
