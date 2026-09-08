<script setup lang="ts">
import type { CraftSelection } from '~/composables/useCrafting'

const route = useRoute()
const { formatItemKind, formatCount } = useFormat()
const { buildTree, flatten, rawMaterials, hasUnresolved, queue } = useCrafting()
const toast = useToast()

const target = computed(() => (route.query.item as string) || '')
const amount = computed(() => Math.max(1, Number(route.query.amount) || 1))
const selections = ref<Record<string, CraftSelection>>({})
const tree = computed(() => (target.value ? buildTree(target.value, amount.value, selections.value) : null))
const pending = computed(() => (tree.value ? hasUnresolved(tree.value) : false))
const stepCount = computed(() => (tree.value ? flatten(tree.value).filter((row) => row.node.type === 'recipe').length : 0))
const materialCount = computed(() => (tree.value ? rawMaterials(tree.value).size : 0))

// Reset picks when the planned item changes.
watch([target, amount], () => { selections.value = {} })

function withoutBranch(key: string) {
  const next: Record<string, CraftSelection> = {}
  for (const [selectionKey, selection] of Object.entries(selections.value)) {
    if (selectionKey !== key && !selectionKey.startsWith(`${key}.`)) next[selectionKey] = selection
  }
  return next
}

function select(key: string, selection: CraftSelection) {
  selections.value = { ...withoutBranch(key), [key]: selection }
}

function unselect(key: string) {
  const next = withoutBranch(key)
  selections.value = next
}

function confirm() {
  if (pending.value) return
  const id = queue(target.value, amount.value, { ...selections.value })
  toast.add({ title: 'Craft queued', description: `${amount.value}× ${formatItemKind(target.value)}`, icon: 'i-lucide-check-circle', color: 'success' })
  navigateTo(`/crafting/${id}`)
}
</script>

<template>
  <section class="flex h-[calc(100dvh-3rem)] min-w-0 flex-col lg:h-[calc(100dvh-5rem)]">
    <UButton to="/crafting" color="neutral" variant="link" icon="i-lucide-arrow-left" class="mb-3 shrink-0 self-start px-0">Back to crafting</UButton>

    <template v-if="tree">
      <div class="relative flex min-h-0 flex-1 flex-col">
        <header class="flex shrink-0 flex-wrap items-center justify-between gap-3">
        <div>
          <p class="text-[10px] uppercase tracking-wide text-primary">Plan · not queued</p>
          <div class="mt-1 flex items-center gap-2">
            <h2 class="text-2xl font-semibold capitalize tracking-tight text-highlighted sm:text-3xl">{{ formatItemKind(target) }}</h2>
            <UBadge color="neutral" variant="soft">×{{ formatCount(amount) }}</UBadge>
          </div>
          <p class="mt-1 text-xs text-muted">{{ stepCount }} craft step{{ stepCount === 1 ? '' : 's' }} · {{ materialCount }} raw material{{ materialCount === 1 ? '' : 's' }}</p>
        </div>
        <div class="flex items-center gap-2">
          <UButton to="/crafting" color="neutral" variant="ghost">Cancel</UButton>
          <UButton color="primary" :icon="pending ? 'i-lucide-git-fork' : 'i-lucide-check'" :disabled="pending" @click="confirm">{{ pending ? 'Choices required' : 'Queue &amp; confirm' }}</UButton>
        </div>
        </header>

        <CraftTree :tree="tree" class="mt-4" @select="select" @unselect="unselect" />
      </div>
    </template>

    <UEmpty v-else icon="i-lucide-search-x" title="Nothing to plan" description="Choose an item from the crafting page." class="py-16" />
  </section>
</template>
