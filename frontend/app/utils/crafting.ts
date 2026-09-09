import type { components } from '~/api-types'
import type { CraftNode, CraftNodeStatus } from '~/types/crafting'

export function normalizePlanNode(node: components['schemas']['CraftNode']): CraftNode {
  const base = {
    key: node.key,
    item: node.item_kind,
    quantity: node.quantity,
    children: node.children.map(normalizePlanNode),
  }
  if (node.kind.type === 'recipe') return { ...base, type: 'recipe', crafts: node.kind.crafts, recipeId: node.kind.recipe_id, selected: node.kind.selected }
  if (node.kind.type === 'storage') return { ...base, type: 'storage', selected: node.kind.selected, available: node.kind.available }
  if (node.kind.type === 'any') return { ...base, type: 'any', selected: true }
  if (node.kind.type === 'deferred') return { ...base, type: 'deferred' }
  return {
    ...base,
    type: 'choice',
    options: node.kind.options.map((option) => {
      if (option.type === 'recipe') return { type: 'recipe', recipeId: option.recipe_id, preview: normalizePlanNode(option.preview as unknown as components['schemas']['CraftNode']), available: option.available }
      if (option.type === 'storage') return { type: 'storage', preview: normalizePlanNode(option.preview as unknown as components['schemas']['CraftNode']), available: option.available }
      return { type: 'any', preview: normalizePlanNode(option.preview as unknown as components['schemas']['CraftNode']) }
    }),
  }
}

export function normalizeExecutionNode(node: components['schemas']['CraftExecutionNode']): CraftNode {
  const base = {
    key: node.key,
    item: node.item_kind,
    quantity: node.required_quantity,
    completedQuantity: node.completed_quantity,
    state: node.state,
    assignedRegionId: node.assigned_region_id,
    error: node.error,
    operation: node.operation ?? null,
    children: node.children.map(normalizeExecutionNode),
  }
  if (node.kind.type === 'recipe') return { ...base, type: 'recipe', crafts: node.kind.planned_crafts, completedCrafts: node.kind.completed_crafts, recipeId: node.kind.recipe_id, selected: true, engineType: node.kind.engine_type }
  if (node.kind.type === 'storage') return { ...base, type: 'storage', selected: true, available: node.completed_quantity }
  return { ...base, type: 'any', selected: true }
}

export function flattenCraftTree(node: CraftNode, depth = 0, rows: { node: CraftNode; depth: number }[] = []) {
  rows.push({ node, depth })
  for (const child of node.children) flattenCraftTree(child, depth + 1, rows)
  return rows
}

export function rawCraftMaterials(node: CraftNode, totals = new Map<string, number>()) {
  if (node.type === 'storage' || node.type === 'any') totals.set(node.item, (totals.get(node.item) ?? 0) + node.quantity)
  else for (const child of node.children) rawCraftMaterials(child, totals)
  return totals
}

export function executionStatuses(node: CraftNode, statuses = new Map<string, CraftNodeStatus>()) {
  const status: CraftNodeStatus = node.state === 'completed' ? 'done'
    : node.state === 'staging' || node.state === 'queued' || node.state === 'running' ? 'active'
      : node.state === 'failed' || node.state === 'cancelled' ? 'failed'
        : node.state === 'blocked' || node.state === 'retry_wait' ? 'blocked'
          : 'pending'
  statuses.set(node.key, status)
  for (const child of node.children) executionStatuses(child, statuses)
  return statuses
}
