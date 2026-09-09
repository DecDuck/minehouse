<script setup lang="ts">
import { executionStatuses } from '~/utils/crafting'

const route = useRoute()
const { formatItemKind, formatCount } = useFormat()
const { loadCraftDetail } = useCrafting()
const craftId = route.params.id as string
const detail = ref(await loadCraftDetail(craftId))
let refreshTimer: ReturnType<typeof setInterval> | undefined
async function refresh() { detail.value = await loadCraftDetail(craftId) }
onMounted(() => { refreshTimer = setInterval(() => { void refresh() }, 2000) })
onUnmounted(() => { if (refreshTimer) clearInterval(refreshTimer) })

const craft = computed(() => detail.value.job)
const tree = computed(() => detail.value.root)
const statuses = computed(() => executionStatuses(tree.value))
</script>

<template>
  <section class="flex h-[calc(100dvh-3rem)] min-w-0 flex-col lg:h-[calc(100dvh-5rem)]">
    <UButton to="/crafting" color="neutral" variant="link" icon="i-lucide-arrow-left" class="mb-3 shrink-0 self-start px-0">Back to crafting</UButton>

    <template v-if="craft">
      <div class="flex min-h-0 flex-1 flex-col">
        <header class="flex shrink-0 flex-wrap items-center justify-between gap-3">
        <div>
          <div class="flex items-center gap-2">
            <h2 class="text-2xl font-semibold capitalize tracking-tight text-highlighted sm:text-3xl">{{ formatItemKind(craft.target_item_kind) }}</h2>
            <UBadge color="neutral" variant="soft">×{{ formatCount(craft.target_quantity) }}</UBadge>
          </div>
          <p class="mt-1 text-xs text-muted"><span class="mono">{{ craft.id }}</span> · {{ craft.completed_quantity }}/{{ craft.target_quantity }} completed</p>
        </div>
        <UBadge :color="craft.state === 'completed' ? 'success' : craft.state === 'failed' ? 'error' : 'primary'" variant="subtle">{{ craft.state }}</UBadge>
        </header>

        <UProgress :model-value="craft.target_quantity ? Math.round(craft.completed_quantity / craft.target_quantity * 100) : 0" :color="craft.state === 'completed' ? 'success' : craft.state === 'failed' ? 'error' : 'primary'" class="mt-4 shrink-0" />

        <p v-if="craft.error" class="mt-4 text-sm text-error">{{ craft.error }}</p>

        <CraftTree v-if="tree" :tree="tree" :statuses="statuses" read-only class="mt-4" />
      </div>
    </template>

    <UEmpty v-else icon="i-lucide-search-x" title="Craft not found" description="This craft is no longer in the queue." class="py-16" />
  </section>
</template>
