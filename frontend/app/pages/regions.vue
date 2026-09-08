<script setup lang="ts">
import type { components } from '../api-types'
type Region = components['schemas']['Region']
type RegionRequest = components['schemas']['RegionRequest']
const api = useMinehouseApi()
const { regionColor } = useFormat()
const blank = (): RegionRequest => ({ type: 'bulk', priority: 0, world_region: { pos1: { x: 0, y: 0, z: 0 }, pos2: { x: 0, y: 0, z: 0 } } })
const form = ref<RegionRequest>(blank())
const editing = ref<string | null>(null)
const { data: regions, pending, refresh } = await useAsyncData('regions', async () => { const response = await api.GET('/api/v1/regions'); if (response.error) throw response.error; return response.data })
function edit(region: Region) { editing.value = region.id; form.value = JSON.parse(JSON.stringify(region)) }
function reset() { editing.value = null; form.value = blank() }
function volume(region: Region) {
  const { pos1, pos2 } = region.world_region
  return (Math.abs(pos2.x - pos1.x) + 1) * (Math.abs(pos2.y - pos1.y) + 1) * (Math.abs(pos2.z - pos1.z) + 1)
}
async function save() { if (editing.value) await api.PUT('/api/v1/regions/{id}', { params: { path: { id: editing.value } }, body: form.value }); else await api.POST('/api/v1/regions', { body: form.value }); reset(); await refresh() }
async function remove(id: string) { await api.DELETE('/api/v1/regions/{id}', { params: { path: { id } } }); if (editing.value === id) reset(); await refresh() }
</script>

<template>
  <section>
    <header class="flex items-center justify-between gap-4">
      <div>
        <h2 class="text-2xl font-semibold tracking-tight text-highlighted sm:text-3xl">Regions</h2>
        <p class="mt-1 text-xs text-muted">Non-overlapping areas for the warehouse planner.</p>
      </div>
      <UButton color="primary" icon="i-lucide-plus" @click="reset">New region</UButton>
    </header>
    <div class="mt-4 grid gap-4 xl:grid-cols-[minmax(0,1fr)_390px]"><UCard><div v-if="pending" class="space-y-2"><USkeleton v-for="n in 4" :key="n" class="h-16" /></div><div v-else class="divide-y divide-default"><div v-for="region in regions" :key="region.id" class="flex flex-col gap-3 py-4 first:pt-0 last:pb-0 sm:flex-row sm:items-center sm:justify-between"><div><div class="flex flex-wrap items-center gap-2"><UBadge :color="regionColor(region.type)" variant="subtle" class="capitalize">{{ region.type }}</UBadge><span class="mono text-[10px] text-muted">priority {{ region.priority }}</span><span class="mono text-[10px] text-muted">· {{ volume(region).toLocaleString() }} blocks</span></div><p class="mono mt-2 text-xs text-muted">{{ region.world_region.pos1.x }}, {{ region.world_region.pos1.y }}, {{ region.world_region.pos1.z }} → {{ region.world_region.pos2.x }}, {{ region.world_region.pos2.y }}, {{ region.world_region.pos2.z }}</p></div><div class="flex gap-2"><UButton color="neutral" variant="outline" size="sm" @click="edit(region)">Edit</UButton><UButton color="error" variant="ghost" size="sm" @click="remove(region.id)">Delete</UButton></div></div><UEmpty v-if="!regions?.length" class="py-10" icon="i-lucide-map" title="No regions defined" /></div></UCard>
      <UCard class="self-start xl:sticky xl:top-6"><UForm :state="form" @submit="save"><div class="flex items-center justify-between"><div><p class="text-xs text-muted">Region editor</p><h3 class="mt-1 text-lg font-semibold text-highlighted">{{ editing ? 'Edit region' : 'New region' }}</h3></div><UButton v-if="editing" type="button" color="neutral" variant="link" size="sm" @click="reset">Cancel</UButton></div><UFormField label="Type" class="mt-6"><USelect v-model="form.type" :items="['bulk', 'pickface', 'putaway', 'processing', 'order']" class="w-full" /></UFormField><UFormField label="Priority" class="mt-4"><UInput v-model.number="form.priority" type="number" class="w-full" /></UFormField><p class="mt-6 text-xs font-medium text-muted">Corner one</p><div class="mt-2 grid grid-cols-3 gap-2"><UInput v-for="axis in ['x', 'y', 'z']" :key="axis" v-model.number="form.world_region.pos1[axis as 'x'|'y'|'z']" :placeholder="axis" type="number" step="any" /></div><p class="mt-5 text-xs font-medium text-muted">Corner two</p><div class="mt-2 grid grid-cols-3 gap-2"><UInput v-for="axis in ['x', 'y', 'z']" :key="axis" v-model.number="form.world_region.pos2[axis as 'x'|'y'|'z']" :placeholder="axis" type="number" step="any" /></div><UButton class="mt-7 w-full" type="submit" icon="i-lucide-save">{{ editing ? 'Save changes' : 'Create region' }}</UButton></UForm></UCard>
    </div>
  </section>
</template>
