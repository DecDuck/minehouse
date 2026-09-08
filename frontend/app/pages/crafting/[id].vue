<script setup lang="ts">
const route = useRoute()
const { formatItemKind, formatCount } = useFormat()
const { getCraft, buildTree, flatten, rawMaterials, statusesFor } = useCrafting()

const craft = computed(() => getCraft(route.params.id as string))
const tree = computed(() => (craft.value ? buildTree(craft.value.target, craft.value.amount, craft.value.selections, '0', false, true) : null))
const stepCount = computed(() => (tree.value ? flatten(tree.value).filter((row) => row.node.type === 'recipe').length : 0))
const materialCount = computed(() => (tree.value ? rawMaterials(tree.value).size : 0))
const statuses = computed(() =>
  tree.value && craft.value ? statusesFor(tree.value, craft.value.progress) : undefined)
</script>

<template>
  <section class="flex h-[calc(100dvh-3rem)] min-w-0 flex-col lg:h-[calc(100dvh-5rem)]">
    <UButton to="/crafting" color="neutral" variant="link" icon="i-lucide-arrow-left" class="mb-3 shrink-0 self-start px-0">Back to crafting</UButton>

    <template v-if="craft && tree">
      <div class="flex min-h-0 flex-1 flex-col">
        <header class="flex shrink-0 flex-wrap items-center justify-between gap-3">
        <div>
          <div class="flex items-center gap-2">
            <h2 class="text-2xl font-semibold capitalize tracking-tight text-highlighted sm:text-3xl">{{ formatItemKind(craft.target) }}</h2>
            <UBadge color="neutral" variant="soft">×{{ formatCount(craft.amount) }}</UBadge>
          </div>
          <p class="mt-1 text-xs text-muted"><span class="mono">{{ craft.id }}</span> · {{ stepCount }} craft step{{ stepCount === 1 ? '' : 's' }} · {{ materialCount }} raw material{{ materialCount === 1 ? '' : 's' }} · started {{ craft.startedAt }}</p>
        </div>
        <UBadge :color="craft.progress >= 1 ? 'success' : 'primary'" variant="subtle">{{ craft.progress >= 1 ? 'Complete' : `${Math.round(craft.progress * 100)}% complete` }}</UBadge>
        </header>

        <UProgress :model-value="Math.round(craft.progress * 100)" :color="craft.progress >= 1 ? 'success' : 'primary'" class="mt-4 shrink-0" />

        <CraftTree :tree="tree" :statuses="statuses" class="mt-4" />
      </div>
    </template>

    <UEmpty v-else icon="i-lucide-search-x" title="Craft not found" description="This craft is no longer in the queue." class="py-16" />
  </section>
</template>
