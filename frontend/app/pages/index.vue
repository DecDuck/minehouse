<script setup lang="ts">
const api = useMinehouseApi()
const { formatItemKind, formatCount } = useFormat()
const query = ref('')
const regionId = ref<string | undefined>()
const minimumQuantity = ref<number | undefined>()
const sortBy = ref<'name' | 'quantity' | 'stacks' | 'containers'>('name')
const sortDirection = ref<'asc' | 'desc'>('asc')
const selectedItemKind = ref<string | null>(null)
const detailOpen = ref(false)
const hasFilters = computed(() => !!query.value || !!regionId.value || !!minimumQuantity.value)
function clearFilters() {
  query.value = ''
  regionId.value = undefined
  minimumQuantity.value = undefined
}
const { data: regions } = await api.GET('/api/v1/regions')
const { data: items, pending, error, refresh } = await useAsyncData('storage', async () => {
  const response = await api.GET('/api/v1/storage', { params: { query: {
    q: query.value || undefined,
    region_id: regionId.value || undefined,
    min_quantity: minimumQuantity.value || undefined,
    sort_by: sortBy.value,
    sort_direction: sortDirection.value,
  } } })
  if (response.error) throw response.error
  return response.data
}, { watch: [query, regionId, minimumQuantity, sortBy, sortDirection] })
const totalQuantity = computed(() => (items.value || []).reduce((sum, item) => sum + item.quantity, 0))
function openDetails(itemKind: string) {
  selectedItemKind.value = itemKind
  detailOpen.value = true
}
</script>

<template>
  <section>
    <header class="flex items-center justify-between gap-4">
      <div>
        <h2 class="text-2xl font-semibold tracking-tight text-highlighted sm:text-3xl">Storage</h2>
        <p class="mt-1 text-xs text-muted">Indexed warehouse inventory, combined by item kind.</p>
      </div>
      <div class="flex gap-2">
        <div class="rounded-lg border border-default bg-elevated/50 px-3 py-1.5 text-right"><p class="text-[10px] uppercase tracking-wide text-muted">Items</p><p class="text-base font-semibold tabular-nums text-highlighted">{{ items?.length ? formatCount(items.length) : '—' }}</p></div>
        <div class="rounded-lg border border-default bg-elevated/50 px-3 py-1.5 text-right"><p class="text-[10px] uppercase tracking-wide text-muted">Quantity</p><p class="text-base font-semibold tabular-nums text-highlighted">{{ totalQuantity ? formatCount(totalQuantity) : '—' }}</p></div>
      </div>
    </header>
    <UCard class="my-4" :ui="{ body: 'p-2 sm:p-3' }">
      <div class="flex flex-wrap items-center gap-2">
        <UInput v-model="query" icon="i-lucide-search" placeholder="Search item kind" aria-label="Search items" class="min-w-40 flex-1" />
        <USelect v-model="regionId" :items="(regions || []).map((region) => ({ label: `${region.type} · ${region.id.slice(0, 8)}`, value: region.id }))" placeholder="All regions" aria-label="Filter region" class="w-40" />
        <UInput v-model.number="minimumQuantity" type="number" min="1" placeholder="Min qty" aria-label="Minimum quantity" class="w-24" />
        <div class="flex items-center gap-1">
          <USelect v-model="sortBy" :items="[{ label: 'Name', value: 'name' }, { label: 'Quantity', value: 'quantity' }, { label: 'Stacks', value: 'stacks' }, { label: 'Containers', value: 'containers' }]" icon="i-lucide-arrow-up-down" aria-label="Sort items by" class="w-36" />
          <UButton color="neutral" variant="outline" square :icon="sortDirection === 'asc' ? 'i-lucide-arrow-up-narrow-wide' : 'i-lucide-arrow-down-wide-narrow'" :aria-label="sortDirection === 'asc' ? 'Ascending' : 'Descending'" @click="sortDirection = sortDirection === 'asc' ? 'desc' : 'asc'" />
        </div>
        <UButton v-if="hasFilters" color="neutral" variant="ghost" icon="i-lucide-x" @click="clearFilters">Clear</UButton>
        <UButton color="neutral" variant="solid" icon="i-lucide-refresh-cw" :loading="pending" @click="refresh()">Refresh</UButton>
      </div>
    </UCard>
    <UAlert v-if="error" color="error" variant="subtle" title="Storage unavailable" description="Could not reach the API. Check the server address." />
    <UCard v-else :ui="{ body: 'p-1 sm:p-2' }">
      <div v-if="pending" class="grid grid-cols-3 gap-1 sm:grid-cols-4 md:grid-cols-6 lg:grid-cols-8 2xl:grid-cols-10">
        <USkeleton v-for="n in 24" :key="n" class="h-[4.5rem]" />
      </div>
      <div v-else-if="items?.length" class="grid grid-cols-3 gap-1 sm:grid-cols-4 md:grid-cols-6 lg:grid-cols-8 2xl:grid-cols-10">
        <UCard v-for="item in items" :key="item.item_kind" as="button" variant="subtle" class="min-h-18 cursor-pointer text-left transition hover:ring-2 hover:ring-primary/50 hover:bg-elevated" :ui="{ body: 'p-1.5' }" @click="openDetails(item.item_kind)">
          <p class="truncate text-[13px] font-medium capitalize leading-tight text-highlighted" :title="formatItemKind(item.item_kind)">{{ formatItemKind(item.item_kind) }}</p>
          <p class="mt-1.5 text-lg font-semibold leading-none tabular-nums text-highlighted">{{ formatCount(item.quantity) }}</p>
          <p class="mt-1 truncate text-[8px] uppercase leading-tight tracking-wide text-muted">{{ formatCount(item.stack_count) }} stacks · {{ formatCount(item.container_count) }} containers</p>
        </UCard>
      </div>
      <UEmpty v-else icon="i-lucide-package-open" title="No indexed items" description="No items match the current filters." class="py-14" />
    </UCard>
    <ItemDetailModal v-model:open="detailOpen" :item-kind="selectedItemKind" />
  </section>
</template>
