<script setup lang="ts">
const api = useMinehouseApi()
const { formatItemKind, formatCount, regionColor } = useFormat()
const { data: groups, pending, error, refresh } = await useAsyncData('containers-grouped', async () => {
  const response = await api.GET('/api/v1/containers/grouped')
  if (response.error) throw response.error
  return response.data
})
const totalContainers = computed(() =>
  (groups.value || []).reduce((sum, group) => sum + group.containers.length, 0))
const accordionItems = computed(() =>
  (groups.value || []).map((group) => ({ value: group.region_id, label: group.region_type ?? 'unknown', group })))
const selected = ref<string | null>(null)
const detailOpen = ref(false)
const current = computed(() =>
  groups.value?.flatMap((group) => group.containers).find((container) => container.id === selected.value))
const currentRegionType = computed(() =>
  groups.value?.find((group) => group.containers.some((container) => container.id === selected.value))?.region_type)
const fillPercent = (container: { contents: unknown[]; capacity: number }) =>
  container.capacity ? Math.round((container.contents.length / container.capacity) * 100) : 0
const slotGrid = computed(() => {
  const container = current.value
  if (!container) return []
  const bySlot = new Map(container.contents.map((item) => [item.slot, item]))
  const maxSlot = container.contents.reduce((max, item) => Math.max(max, item.slot), -1)
  const rows = Math.max(Math.ceil(Math.ceil((maxSlot + 1) / 9) / 3) * 3, 3)
  return Array.from({ length: rows * 9 }, (_, slot) => bySlot.get(slot) ?? null)
})
function open(id: string) {
  selected.value = id
  detailOpen.value = true
}
</script>

<template>
  <section>
    <header class="flex items-center justify-between gap-4">
      <div>
        <h2 class="text-2xl font-semibold tracking-tight text-highlighted sm:text-3xl">Containers</h2>
        <p class="mt-1 text-xs text-muted">Indexed storage locations, grouped by region.</p>
      </div>
      <div class="flex items-center gap-2">
        <div class="rounded-lg border border-default bg-elevated/50 px-3 py-1.5 text-right"><p class="text-[10px] uppercase tracking-wide text-muted">Containers</p><p class="text-base font-semibold tabular-nums text-highlighted">{{ totalContainers ? formatCount(totalContainers) : '—' }}</p></div>
        <UButton color="neutral" variant="outline" icon="i-lucide-refresh-cw" :loading="pending" @click="refresh()">Refresh</UButton>
      </div>
    </header>
    <UAlert v-if="error" class="mt-4" color="error" variant="subtle" title="Containers unavailable" description="Could not reach the API." />
    <div v-else-if="pending" class="mt-4 grid gap-2 sm:grid-cols-2 xl:grid-cols-4">
      <USkeleton v-for="n in 12" :key="n" class="h-28" />
    </div>
    <UEmpty v-else-if="!totalContainers" class="mt-10 py-16" icon="i-lucide-box" title="No containers indexed" />
    <UAccordion v-else type="multiple" :items="accordionItems" :default-value="accordionItems.map((item) => item.value)" class="mt-4">
      <template #default="{ item }">
        <div class="flex flex-1 items-center gap-2">
          <UBadge :color="regionColor(item.group.region_type ?? '')" variant="subtle" class="capitalize">{{ item.group.region_type ?? 'unknown' }}</UBadge>
          <span class="mono text-[10px] text-muted">{{ item.group.region_id.slice(0, 8) }}</span>
          <span class="text-xs text-muted">· {{ item.group.containers.length }} container{{ item.group.containers.length === 1 ? '' : 's' }}</span>
        </div>
      </template>
      <template #content="{ item }">
        <div class="grid gap-2 pb-4 sm:grid-cols-2 xl:grid-cols-3 2xl:grid-cols-4">
          <UCard v-for="container in item.group.containers" :key="container.id" as="button" variant="subtle" class="text-left transition hover:ring-2 hover:ring-primary/50" :ui="{ body: 'p-3' }" @click="open(container.id)">
            <div class="flex items-center justify-between">
              <span class="mono text-[10px] text-primary">{{ container.id.slice(0, 8) }}</span>
              <UBadge color="neutral" variant="soft" size="sm">{{ container.contents.length }}/{{ container.capacity }}</UBadge>
            </div>
            <p class="mono mt-3 text-lg font-semibold text-highlighted">{{ container.position.x }}, {{ container.position.y }}, {{ container.position.z }}</p>
            <UProgress :model-value="fillPercent(container)" :color="fillPercent(container) >= 90 ? 'error' : 'primary'" size="sm" class="mt-3" />
            <p class="mt-1.5 text-[10px] text-muted">{{ fillPercent(container) }}% full</p>
          </UCard>
        </div>
      </template>
    </UAccordion>

    <UModal v-model:open="detailOpen" :title="current ? `Container ${current.id.slice(0, 8)}` : 'Container'" :ui="{ content: 'sm:max-w-2xl' }">
      <template #body>
        <div v-if="current">
          <div class="grid gap-2 sm:grid-cols-3">
            <div class="rounded-lg border border-default bg-elevated/40 p-3"><p class="text-[10px] uppercase tracking-wide text-muted">Slots</p><p class="mt-0.5 text-lg font-semibold tabular-nums text-highlighted">{{ current.contents.length }}/{{ current.capacity }}</p></div>
            <div class="rounded-lg border border-default bg-elevated/40 p-3"><p class="text-[10px] uppercase tracking-wide text-muted">Position</p><p class="mono mt-0.5 text-sm font-semibold text-highlighted">{{ current.position.x }}, {{ current.position.y }}, {{ current.position.z }}</p></div>
            <div class="rounded-lg border border-default bg-elevated/40 p-3"><p class="text-[10px] uppercase tracking-wide text-muted">Region</p><UBadge :color="regionColor(currentRegionType ?? '')" variant="subtle" size="sm" class="mt-1 capitalize">{{ currentRegionType ?? current.region_id.slice(0, 8) }}</UBadge></div>
          </div>
          <div class="mt-4 max-h-[50vh] overflow-y-auto">
            <div v-if="current.contents.length" class="grid grid-cols-9 gap-1">
              <div v-for="(item, slot) in slotGrid" :key="slot" class="aspect-square rounded border" :class="item ? 'border-default bg-elevated/60 p-1' : 'flex items-center justify-center border-dashed border-default/50 bg-transparent'">
                <div v-if="item" class="flex h-full flex-col justify-between" :title="`${formatItemKind(item.item_kind)} · ${item.quantity} · slot ${item.slot}`">
                  <span class="truncate text-[9px] capitalize leading-tight text-muted">{{ formatItemKind(item.item_kind) }}</span>
                  <span class="self-end text-xs font-semibold tabular-nums leading-none text-highlighted">{{ formatCount(item.quantity) }}</span>
                </div>
                <UIcon v-else name="i-lucide-grip" class="size-3 text-muted/30" />
              </div>
            </div>
            <UEmpty v-else class="py-8" icon="i-lucide-inbox" title="Empty container" />
          </div>
        </div>
      </template>
    </UModal>
  </section>
</template>
