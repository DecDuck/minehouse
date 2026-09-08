export interface CraftNodeBase {
  key: string
  item: string
  quantity: number
  children: CraftNode[]
}

export interface RecipeCraftNode extends CraftNodeBase {
  type: 'recipe'
  crafts: number
  recipeIndex: number
  selected: boolean
}

export interface StorageCraftNode extends CraftNodeBase {
  type: 'storage'
  selected: boolean
}

export interface ChoiceCraftNode extends CraftNodeBase {
  type: 'choice'
  options: CraftOption[]
}

export interface DeferredCraftNode extends CraftNodeBase {
  type: 'deferred'
}

export interface AnyRecipeCraftNode extends CraftNodeBase {
  type: 'any'
  selected: true
}

export type CraftNode = RecipeCraftNode | StorageCraftNode | ChoiceCraftNode | DeferredCraftNode | AnyRecipeCraftNode

export interface RecipeCraftOption {
  type: 'recipe'
  index: number
  preview: RecipeCraftNode
}

export interface StorageCraftOption {
  type: 'storage'
  preview: StorageCraftNode
}

export interface AnyRecipeCraftOption {
  type: 'any'
  preview: AnyRecipeCraftNode
}

export type CraftOption = RecipeCraftOption | StorageCraftOption | AnyRecipeCraftOption

export type CraftSelection =
  | { type: 'recipe'; recipeIndex: number }
  | { type: 'storage' }
  | { type: 'any' }

export interface Craft {
  id: string
  target: string
  amount: number
  startedAt: string
  progress: number
  selections: Record<string, CraftSelection>
}

interface Recipe {
  yield: number
  ingredients: { item: string; count: number }[]
}

const recipes: Record<string, Recipe[]> = {
  'minecraft:hopper': [{ yield: 1, ingredients: [{ item: 'minecraft:iron_ingot', count: 5 }, { item: 'minecraft:chest', count: 1 }] }],
  'minecraft:chest': [{ yield: 1, ingredients: [{ item: 'minecraft:oak_planks', count: 8 }] }],
  'minecraft:crafting_table': [{ yield: 1, ingredients: [{ item: 'minecraft:oak_planks', count: 4 }] }],
  'minecraft:piston': [{ yield: 1, ingredients: [{ item: 'minecraft:oak_planks', count: 3 }, { item: 'minecraft:cobblestone', count: 4 }, { item: 'minecraft:iron_ingot', count: 1 }, { item: 'minecraft:redstone', count: 1 }] }],
  'minecraft:iron_pickaxe': [{ yield: 1, ingredients: [{ item: 'minecraft:iron_ingot', count: 3 }, { item: 'minecraft:stick', count: 2 }] }],
  'minecraft:oak_planks': [
    { yield: 4, ingredients: [{ item: 'minecraft:oak_log', count: 1 }] },
    { yield: 4, ingredients: [{ item: 'minecraft:oak_wood', count: 1 }] },
  ],
  'minecraft:stick': [{ yield: 4, ingredients: [{ item: 'minecraft:oak_planks', count: 2 }] }],
  'minecraft:iron_ingot': [
    { yield: 1, ingredients: [{ item: 'minecraft:iron_ore', count: 1 }] },
    { yield: 9, ingredients: [{ item: 'minecraft:iron_block', count: 1 }] },
  ],
  'minehouse:quantum_computer': [{ yield: 1, ingredients: [
    { item: 'minehouse:quantum_core', count: 2 },
    { item: 'minehouse:logic_board', count: 4 },
    { item: 'minehouse:energy_cell', count: 3 },
    { item: 'minehouse:reinforced_frame', count: 1 },
  ] }],
  'minehouse:quantum_core': [
    { yield: 1, ingredients: [{ item: 'minehouse:entangled_crystal', count: 2 }, { item: 'minehouse:logic_board', count: 1 }, { item: 'minecraft:nether_star', count: 1 }] },
    { yield: 2, ingredients: [{ item: 'minehouse:stabilized_crystal', count: 3 }, { item: 'minehouse:energy_cell', count: 1 }] },
  ],
  'minehouse:logic_board': [{ yield: 2, ingredients: [
    { item: 'minehouse:silicon_wafer', count: 2 },
    { item: 'minehouse:copper_trace', count: 4 },
    { item: 'minehouse:microchip', count: 3 },
  ] }],
  'minehouse:energy_cell': [
    { yield: 1, ingredients: [{ item: 'minehouse:charged_battery', count: 2 }, { item: 'minehouse:stabilized_crystal', count: 1 }, { item: 'minecraft:redstone', count: 8 }] },
    { yield: 3, ingredients: [{ item: 'minehouse:high_density_battery', count: 1 }, { item: 'minecraft:glowstone_dust', count: 4 }] },
  ],
  'minehouse:reinforced_frame': [{ yield: 1, ingredients: [
    { item: 'minehouse:titanium_plate', count: 6 },
    { item: 'minehouse:carbon_fiber', count: 4 },
    { item: 'minecraft:obsidian', count: 2 },
  ] }],
  'minehouse:entangled_crystal': [{ yield: 1, ingredients: [{ item: 'minehouse:raw_crystal', count: 3 }, { item: 'minehouse:resonance_dust', count: 5 }, { item: 'minecraft:diamond', count: 1 }] }],
  'minehouse:stabilized_crystal': [
    { yield: 2, ingredients: [{ item: 'minehouse:raw_crystal', count: 1 }, { item: 'minehouse:resonance_dust', count: 2 }] },
    { yield: 1, ingredients: [{ item: 'minehouse:entangled_crystal', count: 1 }, { item: 'minecraft:glass', count: 4 }] },
  ],
  'minehouse:silicon_wafer': [{ yield: 4, ingredients: [{ item: 'minehouse:purified_sand', count: 3 }, { item: 'minecraft:quartz', count: 1 }] }],
  'minehouse:copper_trace': [{ yield: 8, ingredients: [{ item: 'minecraft:copper_ingot', count: 2 }, { item: 'minecraft:redstone', count: 1 }] }],
  'minehouse:microchip': [
    { yield: 4, ingredients: [{ item: 'minehouse:silicon_wafer', count: 1 }, { item: 'minecraft:gold_nugget', count: 4 }, { item: 'minecraft:redstone', count: 2 }] },
    { yield: 1, ingredients: [{ item: 'minehouse:logic_board', count: 1 }, { item: 'minecraft:diamond', count: 1 }] },
  ],
  'minehouse:charged_battery': [{ yield: 2, ingredients: [{ item: 'minehouse:empty_battery', count: 2 }, { item: 'minecraft:iron_ingot', count: 1 }, { item: 'minecraft:redstone', count: 4 }] }],
  'minehouse:high_density_battery': [{ yield: 1, ingredients: [{ item: 'minehouse:charged_battery', count: 4 }, { item: 'minehouse:stabilized_crystal', count: 2 }, { item: 'minecraft:gold_ingot', count: 2 }] }],
  'minehouse:titanium_plate': [{ yield: 2, ingredients: [{ item: 'minehouse:titanium_ingot', count: 3 }, { item: 'minecraft:iron_ingot', count: 1 }] }],
  'minehouse:titanium_ingot': [
    { yield: 1, ingredients: [{ item: 'minehouse:titanium_ore', count: 2 }, { item: 'minecraft:coal', count: 1 }] },
    { yield: 3, ingredients: [{ item: 'minehouse:titanium_scrap', count: 5 }, { item: 'minecraft:iron_ingot', count: 1 }] },
  ],
  'minehouse:carbon_fiber': [{ yield: 4, ingredients: [{ item: 'minehouse:polymer_resin', count: 2 }, { item: 'minecraft:string', count: 8 }] }],
  'minehouse:polymer_resin': [{ yield: 2, ingredients: [{ item: 'minehouse:raw_resin', count: 3 }, { item: 'minecraft:slime_ball', count: 1 }] }],
  'minehouse:purified_sand': [{ yield: 2, ingredients: [{ item: 'minecraft:sand', count: 4 }, { item: 'minecraft:clay_ball', count: 1 }] }],
  'minehouse:resonance_dust': [{ yield: 4, ingredients: [{ item: 'minecraft:amethyst_shard', count: 2 }, { item: 'minecraft:glowstone_dust', count: 3 }] }],
}

// Module-scoped so the mock queue persists across route navigation.
const crafts = ref<Craft[]>([
  { id: 'cft-8a21', target: 'minecraft:piston', amount: 12, startedAt: '2m ago', progress: 0.62, selections: {} },
  { id: 'cft-4f70', target: 'minecraft:iron_pickaxe', amount: 3, startedAt: '9m ago', progress: 0.25, selections: {} },
  { id: 'cft-1c05', target: 'minecraft:chest', amount: 64, startedAt: '18m ago', progress: 0.88, selections: {} },
])

function buildRecipeNode(
  item: string,
  quantity: number,
  recipe: Recipe,
  recipeIndex: number,
  selections: Record<string, CraftSelection>,
  key: string,
  preview: boolean,
  autoResolve: boolean,
  selected: boolean,
): RecipeCraftNode {
  const crafts = Math.ceil(quantity / recipe.yield)
  const children = recipe.ingredients.map((ingredient, index) =>
    buildTree(ingredient.item, ingredient.count * crafts, selections, `${key}.${index}`, preview, autoResolve))
  return { type: 'recipe', key, item, quantity, crafts, recipeIndex, selected, children }
}

function buildTree(
  item: string,
  quantity: number,
  selections: Record<string, CraftSelection> = {},
  key = '0',
  preview = false,
  autoResolve = false,
): CraftNode {
  const itemRecipes = recipes[item]
  if (!itemRecipes?.length) return { type: 'storage', key, item, quantity, selected: false, children: [] }

  const selection = selections[key]
  if (selection?.type === 'storage') return { type: 'storage', key, item, quantity, selected: true, children: [] }
  if (selection?.type === 'any') return { type: 'any', key, item, quantity, selected: true, children: [] }

  if (itemRecipes.length > 1) {
    const recipeIndex = selection?.type === 'recipe' ? selection.recipeIndex : autoResolve ? 0 : undefined
    if (recipeIndex === undefined) {
      if (preview) return { type: 'deferred', key, item, quantity, children: [] }

      const options: CraftOption[] = itemRecipes.map((recipe, index) => ({
        type: 'recipe',
        index,
        preview: buildRecipeNode(item, quantity, recipe, index, selections, `${key}#${index}`, true, autoResolve, false),
      }))
      options.push({
        type: 'any',
        preview: { type: 'any', key: `${key}#any`, item, quantity, selected: true, children: [] },
      })
      options.push({
        type: 'storage',
        preview: { type: 'storage', key: `${key}#storage`, item, quantity, selected: false, children: [] },
      })
      return { type: 'choice', key, item, quantity, options, children: [] }
    }

    return buildRecipeNode(item, quantity, itemRecipes[recipeIndex]!, recipeIndex, selections, key, preview, autoResolve, selection?.type === 'recipe')
  }

  return buildRecipeNode(item, quantity, itemRecipes[0]!, 0, selections, key, preview, autoResolve, false)
}

function hasUnresolved(node: CraftNode): boolean {
  return node.type === 'choice' || node.children.some(hasUnresolved)
}


function flatten(node: CraftNode, depth = 0, rows: { node: CraftNode; depth: number }[] = []) {
  rows.push({ node, depth })
  for (const child of node.children) flatten(child, depth + 1, rows)
  return rows
}

// Assembly order: dependencies before parents.
function postOrder(node: CraftNode, keys: string[] = []) {
  for (const child of node.children) postOrder(child, keys)
  keys.push(node.key)
  return keys
}

function rawMaterials(node: CraftNode, totals = new Map<string, number>()) {
  if (node.type === 'storage') totals.set(node.item, (totals.get(node.item) ?? 0) + node.quantity)
  else for (const child of node.children) rawMaterials(child, totals)
  return totals
}

function statusesFor(node: CraftNode, progress: number) {
  const map = new Map<string, 'done' | 'active' | 'pending'>()
  const order = postOrder(node)
  const done = Math.floor(order.length * progress)
  order.forEach((key, index) => map.set(key, index < done ? 'done' : index === done ? 'active' : 'pending'))
  return map
}

export function useCrafting() {
  const itemKinds = Object.keys(recipes)
  const getCraft = (id: string) => crafts.value.find((craft) => craft.id === id) ?? null
  function queue(target: string, amount: number, selections: Record<string, CraftSelection> = {}) {
    const id = `cft-${Math.random().toString(16).slice(2, 6)}`
    crafts.value.unshift({ id, target, amount, startedAt: 'just now', progress: 0, selections })
    return id
  }
  return { crafts, itemKinds, buildTree, flatten, postOrder, rawMaterials, statusesFor, hasUnresolved, getCraft, queue }
}
