import type { components } from '~/api-types'

export type CraftSelection = components['schemas']['CraftSelection']
export type CraftJobStatus = components['schemas']['CraftJobStatus']
export type CraftNodeStatus = 'done' | 'active' | 'pending' | 'blocked' | 'failed'

type CraftNodeBase = {
  key: string
  item: string
  quantity: number
  children: CraftNode[]
  completedQuantity?: number
  state?: string
  assignedRegionId?: string | null
  error?: string | null
  operation?: components['schemas']['CraftExecutionOperation'] | null
}

export type CraftNode = CraftNodeBase & (
  | { type: 'recipe'; crafts: number; completedCrafts?: number; recipeId: string; selected: boolean; engineType?: string }
  | { type: 'storage'; selected: boolean; available: number }
  | { type: 'choice'; options: CraftOption[] }
  | { type: 'deferred' }
  | { type: 'any'; selected: true }
)

export type CraftOption =
  | { type: 'recipe'; recipeId: string; preview: CraftNode; available: boolean }
  | { type: 'storage'; preview: CraftNode; available: boolean }
  | { type: 'any'; preview: CraftNode }

export type CraftPlan = Omit<components['schemas']['CraftPlan'], 'root'> & { root: CraftNode }
export type CraftDetail = { job: CraftJobStatus; root: CraftNode }
