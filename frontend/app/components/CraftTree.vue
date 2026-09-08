<script setup lang="ts">
import type { CraftNode, CraftSelection } from '~/composables/useCrafting'

const props = defineProps<{
  tree: CraftNode
  statuses?: Map<string, 'done' | 'active' | 'pending'>
}>()
const emit = defineEmits<{ select: [key: string, selection: CraftSelection]; unselect: [key: string] }>()

const { formatItemKind, formatCount } = useFormat()
const { rawMaterials } = useCrafting()

const materials = computed(() => [...rawMaterials(props.tree).entries()].sort((a, b) => b[1] - a[1]))
</script>

<template>
  <div class="flex h-full min-h-0 flex-1 flex-col">
    <div v-if="materials.length" class="shrink-0">
      <p class="text-[10px] uppercase tracking-wide text-muted">Raw materials required</p>
      <div class="mt-2 flex flex-wrap gap-2">
        <div v-for="[item, count] in materials" :key="item" class="flex items-center gap-1.5 rounded-md border border-default bg-elevated/40 px-2 py-1 text-xs">
          <UIcon name="i-lucide-package" class="size-3 text-muted" />
          <span class="capitalize text-highlighted">{{ formatItemKind(item) }}</span>
          <span class="tabular-nums font-semibold text-highlighted">×{{ formatCount(count) }}</span>
        </div>
      </div>
    </div>

    <CraftTreeGraph :tree="tree" :statuses="statuses" class="mt-4 min-h-0 flex-1" @select="(key, selection) => emit('select', key, selection)" @unselect="(key) => emit('unselect', key)" />
  </div>
</template>
