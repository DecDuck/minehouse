<script setup lang="ts">
import type { components } from '~/api-types'

type WorkUnit = components['schemas']['WorkUnitStatus']
type WorkUnitState = components['schemas']['WorkUnitStatusState']

const api = useMinehouseApi()
const view = ref<'active' | 'all' | 'completed'>('active')
const { data: status, pending, error, refresh } = await useAsyncData('system-status', async () => {
  const response = await api.GET('/api/v1/status')
  if (response.error) throw response.error
  return response.data
})

let refreshTimer: ReturnType<typeof setInterval> | undefined
onMounted(() => {
  refreshTimer = setInterval(() => refresh(), 2000)
})
onBeforeUnmount(() => clearInterval(refreshTimer))

const visibleWorkUnits = computed(() => {
  const units = status.value?.work_units ?? []
  if (view.value === 'completed') return units.filter(unit => unit.state === 'completed')
  if (view.value === 'active') return units.filter(unit => unit.state !== 'completed')
  return units
})

const viewOptions = [
  { label: 'Active', value: 'active' },
  { label: 'All retained', value: 'all' },
  { label: 'Completed', value: 'completed' },
]

const labels: Record<string, string> = {
  index_regions: 'Index regions',
  cycle_count: 'Cycle count',
  putaway: 'Putaway',
  craft: 'Craft',
  index_region: 'Index region',
  index_container: 'Index container',
  transfer: 'Transfer',
}

function label(value: string) {
  return labels[value] ?? value.replaceAll('_', ' ')
}

function stateColor(state: WorkUnitState): 'warning' | 'info' | 'success' {
  if (state === 'claimed') return 'info'
  if (state === 'completed') return 'success'
  return 'warning'
}

function progress(unit: WorkUnit) {
  if (unit.completed_steps == null || unit.total_steps == null) return null
  if (unit.total_steps === 0) return unit.state === 'completed' ? 100 : 0
  return Math.min(100, Math.round((unit.completed_steps / unit.total_steps) * 100))
}
</script>

<template>
  <section>
    <header class="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
      <div>
        <div class="flex items-center gap-2">
          <h2 class="text-2xl font-semibold tracking-tight text-highlighted sm:text-3xl">Operations</h2>
          <UBadge color="success" variant="subtle">
            <span class="mr-1.5 inline-block size-1.5 rounded-full bg-current motion-safe:animate-pulse" />Live
          </UBadge>
        </div>
        <p class="mt-1 text-xs text-muted">Planner requests and worker execution state.</p>
      </div>
      <UButton color="neutral" variant="outline" icon="i-lucide-refresh-cw" :loading="pending" @click="refresh()">Refresh</UButton>
    </header>

    <UAlert v-if="error" class="mt-4" color="error" variant="subtle" title="Operations unavailable" description="Could not reach the status API." />

    <template v-else-if="status">
      <div class="mt-5 grid grid-cols-2 border-y border-default sm:grid-cols-5">
        <div class="border-b border-r border-default p-3 sm:border-b-0">
          <p class="text-[10px] uppercase tracking-wide text-muted">Planner queue</p>
          <p class="mt-1 text-2xl font-semibold tabular-nums text-highlighted">{{ status.planner_queue.length }}</p>
        </div>
        <div class="border-b border-default p-3 sm:border-b-0 sm:border-r">
          <p class="text-[10px] uppercase tracking-wide text-muted">Queued work</p>
          <p class="mt-1 text-2xl font-semibold tabular-nums text-warning">{{ status.work_unit_pool.queued }}</p>
        </div>
        <div class="border-r border-default p-3 sm:border-r">
          <p class="text-[10px] uppercase tracking-wide text-muted">Claimed</p>
          <p class="mt-1 text-2xl font-semibold tabular-nums text-info">{{ status.work_unit_pool.claimed }}</p>
        </div>
        <div class="p-3 sm:border-r sm:border-default">
          <p class="text-[10px] uppercase tracking-wide text-muted">Completed</p>
          <p class="mt-1 text-2xl font-semibold tabular-nums text-success">{{ status.work_unit_pool.completed }}</p>
        </div>
        <div class="col-span-2 border-t border-default p-3 sm:col-span-1 sm:border-t-0">
          <p class="text-[10px] uppercase tracking-wide text-muted">Pool total</p>
          <p class="mt-1 text-2xl font-semibold tabular-nums text-highlighted">{{ status.work_unit_pool.total }}</p>
        </div>
      </div>

      <div class="mt-8 grid gap-8 xl:grid-cols-[minmax(18rem,0.7fr)_minmax(32rem,1.6fr)]">
        <section aria-labelledby="planner-heading">
          <div class="flex items-end justify-between border-b border-default pb-2">
            <div>
              <h3 id="planner-heading" class="text-base font-semibold text-highlighted">Planner queue</h3>
              <p class="mt-0.5 text-[11px] text-muted">Pending requests in dispatch order.</p>
            </div>
            <span class="mono text-xs text-muted">{{ status.planner_queue.length }}</span>
          </div>
          <ol v-if="status.planner_queue.length" class="divide-y divide-default">
            <li v-for="request in status.planner_queue" :key="request.position" class="flex items-center gap-3 py-3">
              <span class="mono flex size-7 shrink-0 items-center justify-center border border-default text-xs text-muted">{{ request.position }}</span>
              <div class="min-w-0 flex-1">
                <p class="text-sm font-medium text-highlighted">{{ label(request.kind) }}</p>
                <p v-if="request.detail" class="mono mt-0.5 truncate text-[11px] text-muted">{{ request.detail }}</p>
              </div>
            </li>
          </ol>
          <UEmpty v-else icon="i-lucide-list-checks" title="Planner queue clear" class="py-12" />
        </section>

        <section aria-labelledby="pool-heading">
          <div class="flex flex-wrap items-end justify-between gap-3 border-b border-default pb-2">
            <div>
              <h3 id="pool-heading" class="text-base font-semibold text-highlighted">Work-unit pool</h3>
              <p class="mt-0.5 text-[11px] text-muted">Completed units are retained in this pool and included in its total.</p>
            </div>
            <USelect v-model="view" :items="viewOptions" aria-label="Filter work units" class="w-36" />
          </div>

          <div v-if="visibleWorkUnits.length" class="divide-y divide-default">
            <article v-for="unit in visibleWorkUnits" :key="unit.id" class="grid gap-3 py-3 sm:grid-cols-[minmax(0,1fr)_8rem] sm:items-center">
              <div class="min-w-0">
                <div class="flex flex-wrap items-center gap-2">
                  <UBadge :color="stateColor(unit.state)" variant="subtle" size="sm" class="capitalize">{{ unit.state }}</UBadge>
                  <span class="text-sm font-medium text-highlighted">{{ label(unit.kind) }}</span>
                  <span class="mono text-[10px] text-muted">{{ unit.id.slice(0, 8) }}</span>
                  <span class="mono text-[10px] text-muted">P{{ unit.priority }}</span>
                </div>
                <p class="mt-1 truncate text-xs text-muted" :title="unit.detail">{{ unit.detail }}</p>
              </div>
              <div v-if="progress(unit) != null" class="min-w-0">
                <div class="mb-1 flex justify-between text-[10px] text-muted">
                  <span>{{ unit.completed_steps }}/{{ unit.total_steps }}</span>
                  <span>{{ progress(unit) }}%</span>
                </div>
                <UProgress :model-value="progress(unit) ?? 0" :color="unit.state === 'completed' ? 'success' : 'primary'" size="sm" />
              </div>
            </article>
          </div>
          <UEmpty v-else icon="i-lucide-inbox" :title="view === 'active' ? 'No active work units' : 'No work units in this view'" class="py-12" />
        </section>
      </div>
    </template>

    <div v-else class="mt-5 grid gap-3 sm:grid-cols-2 xl:grid-cols-4">
      <USkeleton v-for="item in 8" :key="item" class="h-24" />
    </div>
  </section>
</template>