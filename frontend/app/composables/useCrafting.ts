import type { components } from '~/api-types'
import type { CraftDetail, CraftPlan, CraftSelection } from '~/types/crafting'
import { normalizeExecutionNode, normalizePlanNode } from '~/utils/crafting'

export function useCrafting() {
  const api = useMinehouseApi()
  const recipes = ref<components['schemas']['RecipeResponse'][]>([])
  const crafts = ref<components['schemas']['CraftJobStatus'][]>([])
  const loading = ref(false)
  async function loadRecipes() { const { data } = await api.GET('/api/v1/recipes'); recipes.value = data ?? []; return recipes.value }
  async function plan(item_kind: string, amount: number, selections: Record<string, CraftSelection> = {}): Promise<CraftPlan | null> {
    loading.value = true
    try { const response = await api.POST('/api/v1/crafting/plan', { body: { item_kind, amount, selections } }); if (response.error) throw response.error; return response.data ? { ...response.data, root: normalizePlanNode(response.data.root) } : null }
    finally { loading.value = false }
  }
  async function loadCrafts() {
    const { data } = await api.GET('/api/v1/crafting')
    crafts.value = data ?? []
    return crafts.value
  }
  async function loadCraftDetail(id: string): Promise<CraftDetail> {
    const response = await api.GET('/api/v1/crafting/{id}', { params: { path: { id } } })
    if (response.error) throw response.error
    if (!response.data) throw new Error(`craft ${id} was not found`)
    return { job: response.data.job, root: normalizeExecutionNode(response.data.root) }
  }
  async function queue(target: string, amount: number, selections: Record<string, CraftSelection> = {}) {
    const response = await api.POST('/api/v1/crafting', { body: { item_kind: target, amount, selections } })
    if (response.error) throw response.error
    const id = response.data?.job_id
    if (!id) throw new Error('craft queue did not return a job id')
    await loadCrafts()
    return id
  }
  return { crafts, recipes, loading, loadRecipes, loadCrafts, loadCraftDetail, plan, queue }
}
