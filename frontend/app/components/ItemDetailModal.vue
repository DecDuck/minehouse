<script setup lang="ts">
import type { components } from '~/api-types'

const props = defineProps<{ itemKind: string | null }>()
const open = defineModel<boolean>('open', { default: false })

const api = useMinehouseApi()
const { formatItemKind, formatCount, regionColor } = useFormat()

const details = ref<components['schemas']['ItemKindDetails'] | null>(null)
const pending = ref(false)
const failed = ref(false)

async function load() {
  if (!props.itemKind) return
  pending.value = true
  failed.value = false
  details.value = null
  const response = await api.GET('/api/v1/storage/{item_kind}', { params: { path: { item_kind: props.itemKind } } })
  if (response.error) failed.value = true
  else details.value = response.data
  pending.value = false
}

watch([open, () => props.itemKind], () => {
  if (open.value) load()
})
</script>

<template>
  <UModal v-model:open="open" :title="itemKind ? formatItemKind(itemKind) : 'Item details'" :description="itemKind || undefined" :ui="{ content: 'sm:max-w-4xl' }">
    <template #body>
      <UAlert v-if="pending" color="primary" variant="subtle" title="Loading item details" icon="i-lucide-loader-circle" />
      <UAlert v-else-if="failed" color="error" variant="subtle" title="Details unavailable" description="Could not load the indexed stacks for this item." />
      <template v-else-if="details">
        <div class="grid gap-3 sm:grid-cols-3">
          <UCard variant="subtle"><p class="text-xs text-muted">Total quantity</p><p class="mt-1 text-2xl font-semibold text-highlighted">{{ formatCount(details.total_quantity) }}</p></UCard>
          <UCard variant="subtle"><p class="text-xs text-muted">Stacks</p><p class="mt-1 text-2xl font-semibold text-highlighted">{{ formatCount(details.stack_count) }}</p></UCard>
          <UCard variant="subtle"><p class="text-xs text-muted">Containers</p><p class="mt-1 text-2xl font-semibold text-highlighted">{{ formatCount(details.container_count) }}</p></UCard>
        </div>
        <div v-if="details.stacks.length" class="mt-5 overflow-hidden rounded-lg border border-default">
          <div class="sticky top-0 grid grid-cols-[1.2fr_1fr_60px_70px] gap-3 border-b border-default bg-elevated px-3 py-2 text-[10px] font-medium uppercase tracking-wide text-muted sm:grid-cols-[1.5fr_1fr_60px_70px_1.5fr]"><span>Location</span><span>Region</span><span>Slot</span><span class="text-right">Qty</span><span class="hidden sm:block">Components</span></div>
          <div class="max-h-[50vh] overflow-y-auto divide-y divide-default">
            <div v-for="stack in details.stacks" :key="stack.stack_id" class="grid grid-cols-[1.2fr_1fr_60px_70px] items-center gap-3 px-3 py-2.5 text-xs transition hover:bg-elevated/50 sm:grid-cols-[1.5fr_1fr_60px_70px_1.5fr]">
              <div><p class="mono font-medium text-highlighted">{{ stack.position.x }}, {{ stack.position.y }}, {{ stack.position.z }}</p><p class="mono mt-0.5 text-[9px] text-muted">{{ stack.container_id.slice(0, 8) }}</p></div>
              <div><UBadge :color="regionColor(stack.region_type)" variant="subtle" size="sm" class="capitalize">{{ stack.region_type }}</UBadge><p class="mono mt-0.5 text-[9px] text-muted">{{ stack.region_id.slice(0, 8) }}</p></div>
              <span class="mono text-muted">{{ stack.slot }}</span>
              <strong class="text-right tabular-nums text-highlighted">{{ formatCount(stack.quantity) }}</strong>
              <pre class="mono hidden max-w-full truncate text-[10px] text-muted sm:block" :title="JSON.stringify(stack.components)">{{ JSON.stringify(stack.components) }}</pre>
            </div>
          </div>
        </div>
        <UEmpty v-else icon="i-lucide-package-open" title="Not in storage" description="This item has no indexed stacks." class="py-8" />
      </template>
    </template>
  </UModal>
</template>
