<script setup lang="ts">
import type { CraftNode } from '~/types/crafting'

defineProps<{ node: CraftNode; depth?: number }>()

const { formatItemKind, formatCount } = useFormat()
</script>

<template>
  <div :class="depth ? 'ml-3 border-l border-dashed border-default pl-2.5' : ''">
    <div class="flex items-center gap-1.5 py-1 text-[11px]">
      <UIcon
        :name="node.type === 'deferred' ? 'i-lucide-git-fork' : node.type === 'recipe' ? 'i-lucide-hammer' : node.type === 'any' ? 'i-lucide-shuffle' : 'i-lucide-package'"
        class="size-3 shrink-0"
        :class="node.type === 'deferred' ? 'text-warning' : node.type === 'recipe' ? 'text-primary' : node.type === 'any' ? 'text-info' : 'text-muted'"
      />
      <span class="truncate capitalize" :class="node.type === 'recipe' || node.type === 'any' ? 'text-highlighted' : 'text-muted'">{{ formatItemKind(node.item) }}</span>
      <span class="tabular-nums text-muted">×{{ formatCount(node.quantity) }}</span>
      <UBadge
        v-if="node.type === 'deferred'"
        color="warning"
        variant="soft"
        size="sm"
        class="ml-auto shrink-0"
        title="This ingredient also has multiple recipes. You'll pick one after choosing this recipe."
      >choose later</UBadge>
    </div>
    <CraftOptionPreview v-for="child in node.children" :key="child.key" :node="child" :depth="(depth ?? 0) + 1" />
  </div>
</template>
