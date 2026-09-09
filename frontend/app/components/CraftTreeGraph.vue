<script setup lang="ts">
import type { CraftNode, CraftNodeStatus, CraftOption, CraftSelection } from '~/types/crafting'
import { flattenCraftTree } from '~/utils/crafting'

const props = defineProps<{
  tree: CraftNode
  statuses?: Map<string, CraftNodeStatus>
  readOnly?: boolean
}>()
const emit = defineEmits<{ select: [key: string, selection: CraftSelection]; unselect: [key: string] }>()

const { formatItemKind, formatCount } = useFormat()
const NODE_W = 208
const NODE_H = 60
const COL = 280
const GAP = 24
const PAD = 36
const markerId = `craft-arrow-${useId()}`

// Choice boxes grow taller with each option; estimate so the canvas sizes right.
function nodeHeight(node: CraftNode): number {
  if (node.type !== 'choice') return NODE_H
  let h = 38
  for (const option of node.options) h += 34 + (option.type === 'recipe' ? flattenCraftTree(option.preview).length * 22 : 0)
  return h
}

interface Placed {
  key: string
  x: number
  y: number
  h: number
  depth: number
  node: CraftNode
}

// Right-to-left layout: root sits on the right, dependencies fan out leftward.
// Leaves stack top-to-bottom by height; parents centre on their children.
const graph = computed(() => {
  const placed = new Map<string, Placed>()
  let cursor = PAD
  let maxDepth = 0

  const assign = (node: CraftNode, depth: number): number => {
    maxDepth = Math.max(maxDepth, depth)
    const h = nodeHeight(node)
    let y: number
    if (!node.children.length) {
      y = cursor
      cursor += h + GAP
    } else {
      const centers = node.children.map((child) => assign(child, depth + 1))
      y = (Math.min(...centers) + Math.max(...centers)) / 2 - h / 2
    }
    placed.set(node.key, { key: node.key, x: 0, y, h, depth, node })
    return y + h / 2
  }
  assign(props.tree, 0)

  for (const item of placed.values()) item.x = PAD + (maxDepth - item.depth) * COL

  const edges: { key: string; path: string; status?: string }[] = []
  for (const parent of placed.values()) {
    for (const child of parent.node.children) {
      const from = placed.get(child.key)!
      const sx = from.x + NODE_W
      const sy = from.y + from.h / 2
      const ex = parent.x
      const ey = parent.y + parent.h / 2
      const dx = Math.max((ex - sx) / 2, 24)
      edges.push({
        key: `${parent.key}->${child.key}`,
        path: `M ${sx} ${sy} C ${sx + dx} ${sy}, ${ex - dx} ${ey}, ${ex} ${ey}`,
        status: props.statuses?.get(child.key),
      })
    }
  }

  const nodes = [...placed.values()]
  const width = Math.max(...nodes.map((item) => item.x)) + NODE_W + PAD
  const height = Math.max(...nodes.map((item) => item.y + item.h)) + PAD
  return { nodes, edges, width, height }
})

const edgeColor = (status?: string) =>
  status === 'done' ? 'text-success' : status === 'active' ? 'text-primary' : status === 'failed' ? 'text-error' : status === 'blocked' ? 'text-warning' : 'text-muted/60'

const statusMeta = {
  done: { icon: 'i-lucide-check', color: 'text-success', ring: 'ring-success/40' },
  active: { icon: 'i-lucide-loader-circle', color: 'text-primary', ring: 'ring-primary/50' },
  pending: { icon: 'i-lucide-circle', color: 'text-muted', ring: 'ring-default' },
  blocked: { icon: 'i-lucide-pause', color: 'text-warning', ring: 'ring-warning/40' },
  failed: { icon: 'i-lucide-circle-x', color: 'text-error', ring: 'ring-error/40' },
} as const

const viewport = ref<HTMLElement>()
const pan = reactive({ x: 0, y: 0 })
const zoom = ref(1)
const dragging = ref(false)
const moved = ref(false)
let origin = { x: 0, y: 0 }
let start = { x: 0, y: 0 }
let pointerId = 0
let captured = false

const selectedItemKind = ref<string | null>(null)
const detailOpen = ref(false)

const MIN_ZOOM = 0.4
const MAX_ZOOM = 2
const ZOOM_STEP = 0.1

function clampZoom(value: number) {
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, value))
}

function center() {
  if (!viewport.value) return
  pan.x = Math.round((viewport.value.clientWidth - graph.value.width * zoom.value) / 2)
  pan.y = Math.round((viewport.value.clientHeight - graph.value.height * zoom.value) / 2)
}

function zoomAt(nextZoom: number, clientX: number, clientY: number) {
  if (!viewport.value) return
  const clamped = clampZoom(nextZoom)
  if (clamped === zoom.value) return

  const rect = viewport.value.getBoundingClientRect()
  const pointerX = clientX - rect.left
  const pointerY = clientY - rect.top
  const graphX = (pointerX - pan.x) / zoom.value
  const graphY = (pointerY - pan.y) / zoom.value

  zoom.value = clamped
  pan.x = pointerX - graphX * clamped
  pan.y = pointerY - graphY * clamped
}

function zoomFromCenter(delta: number) {
  if (!viewport.value) return
  const rect = viewport.value.getBoundingClientRect()
  zoomAt(zoom.value + delta, rect.left + rect.width / 2, rect.top + rect.height / 2)
}

function onWheel(event: WheelEvent) {
  const factor = Math.exp(-event.deltaY * 0.0015)
  zoomAt(zoom.value * factor, event.clientX, event.clientY)
}

function resetView() {
  zoom.value = 1
  center()
}

function onPointerDown(event: PointerEvent) {
  dragging.value = true
  moved.value = false
  captured = false
  pointerId = event.pointerId
  origin = { x: event.clientX - pan.x, y: event.clientY - pan.y }
  start = { x: event.clientX, y: event.clientY }
}
function onPointerMove(event: PointerEvent) {
  if (!dragging.value) return
  // Capture only once a real drag begins, so plain clicks still reach nodes.
  if (!moved.value && (Math.abs(event.clientX - start.x) > 4 || Math.abs(event.clientY - start.y) > 4)) {
    moved.value = true
    ;(event.currentTarget as HTMLElement).setPointerCapture(pointerId)
    captured = true
  }
  if (!moved.value) return
  pan.x = event.clientX - origin.x
  pan.y = event.clientY - origin.y
}
function onPointerUp() {
  dragging.value = false
  if (captured) {
    viewport.value?.releasePointerCapture(pointerId)
    captured = false
  }
}

// Ignore clicks that were actually pan drags.
function onNodeClick(item: string) {
  if (moved.value) return
  selectedItemKind.value = item
  detailOpen.value = true
}

function pick(key: string, option: CraftOption) {
  if (moved.value || props.readOnly) return
  let selection: CraftSelection
  if (option.type === 'recipe') selection = { type: 'recipe', recipe_id: option.recipeId }
  else if (option.type === 'storage') selection = { type: 'storage' }
  else selection = { type: 'any' }
  emit('select', key, selection)
}

function sourceFromStorage(key: string) {
  if (props.readOnly) return
  emit('select', key, { type: 'storage' })
}

function unpick(key: string) {
  if (props.readOnly) return
  emit('unselect', key)
}

onMounted(center)
</script>

<template>
  <div
    ref="viewport"
    class="relative h-full min-h-0 min-w-0 flex-1 touch-none select-none overflow-hidden rounded-lg border border-neutral-800 bg-neutral-950 [background-image:radial-gradient(rgba(148,163,184,0.28)_1px,transparent_1px)] [background-size:24px_24px]"
    :class="dragging ? 'cursor-grabbing' : 'cursor-grab'"
    @pointerdown="onPointerDown"
    @pointermove="onPointerMove"
    @pointerup="onPointerUp"
    @pointerleave="onPointerUp"
    @wheel.prevent="onWheel"
  >
    <div class="absolute right-2 top-2 z-10 flex items-center gap-1 rounded-md border border-neutral-700 bg-neutral-900/90 p-1 shadow backdrop-blur-sm" @pointerdown.stop>
      <UButton color="neutral" variant="ghost" size="xs" icon="i-lucide-zoom-out" square title="Zoom out" :disabled="zoom <= MIN_ZOOM" @click="zoomFromCenter(-ZOOM_STEP)" />
      <span class="w-10 text-center text-[10px] tabular-nums text-neutral-300">{{ Math.round(zoom * 100) }}%</span>
      <UButton color="neutral" variant="ghost" size="xs" icon="i-lucide-zoom-in" square title="Zoom in" :disabled="zoom >= MAX_ZOOM" @click="zoomFromCenter(ZOOM_STEP)" />
      <UButton color="neutral" variant="ghost" size="xs" icon="i-lucide-locate-fixed" square title="Reset view" @click="resetView" />
    </div>

    <div class="absolute left-0 top-0 origin-top-left" :style="{ transform: `translate(${pan.x}px, ${pan.y}px) scale(${zoom})`, width: `${graph.width}px`, height: `${graph.height}px` }">
      <svg class="absolute inset-0 overflow-visible" :width="graph.width" :height="graph.height">
        <defs>
          <marker :id="markerId" markerWidth="7" markerHeight="7" refX="6" refY="3" orient="auto">
            <path d="M0,0 L6,3 L0,6 Z" fill="context-stroke" />
          </marker>
        </defs>
        <path
          v-for="edge in graph.edges"
          :key="edge.key"
          :d="edge.path"
          fill="none"
          stroke="currentColor"
          stroke-width="1.5"
          :class="['craft-edge', edgeColor(edge.status)]"
          :marker-end="`url(#${markerId})`"
        />
      </svg>

      <div
        v-for="item in graph.nodes"
        :key="item.key"
        class="craft-node absolute"
        :style="{ left: `${item.x}px`, top: `${item.y}px`, width: `${NODE_W}px`, '--craft-delay': `${Math.min(item.depth, 8) * 70}ms` }"
      >
        <div v-if="item.node.type === 'choice'" class="craft-choice rounded-lg border-2 border-dashed border-warning/60 bg-elevated/95 p-1.5 shadow-sm backdrop-blur-sm">
          <div class="flex items-center gap-1.5 px-0.5 pb-1">
            <UIcon name="i-lucide-git-fork" class="size-3.5 shrink-0 text-warning" />
            <span class="truncate text-xs font-semibold capitalize text-highlighted" :title="formatItemKind(item.node.item)">{{ formatItemKind(item.node.item) }}</span>
            <span class="tabular-nums text-xs text-muted">×{{ formatCount(item.node.quantity) }}</span>
            <UBadge color="warning" variant="soft" size="sm" class="ml-auto shrink-0">pick 1</UBadge>
          </div>
          <div class="space-y-1.5">
            <button v-for="(option, optionIndex) in item.node.options" :key="option.type === 'recipe' ? option.recipeId : option.type" type="button" :disabled="readOnly" class="craft-option block w-full rounded-md border border-default bg-default/40 p-1.5 text-left transition hover:border-primary hover:ring-2 hover:ring-primary/50 disabled:cursor-default disabled:opacity-100 disabled:hover:border-default disabled:hover:ring-0" :style="{ '--craft-delay': `${optionIndex * 80}ms` }" @click="pick(item.key, option)">
              <div v-if="option.type === 'any'" class="flex items-center gap-1.5 py-0.5 text-[11px]">
                <UIcon name="i-lucide-shuffle" class="size-3 shrink-0 text-info" />
                <span class="font-medium text-highlighted">Use any recipe</span>
              </div>
              <div v-else-if="option.type === 'storage'" class="flex items-center gap-1.5 py-0.5 text-[11px]">
                <UIcon name="i-lucide-package-search" class="size-3 shrink-0 text-warning" />
                <span class="font-medium text-highlighted">From storage</span>
                <UBadge :color="option.available ? 'success' : 'error'" variant="soft" size="sm" class="ml-auto">{{ option.available ? 'available' : 'not enough' }}</UBadge>
              </div>
              <div v-else class="flex items-center gap-1.5">
                <CraftOptionPreview :node="option.preview" class="min-w-0 flex-1" />
                <UBadge :color="option.available ? 'success' : 'error'" variant="soft" size="sm" class="shrink-0">{{ option.available ? 'available' : 'not enough' }}</UBadge>
              </div>
            </button>
          </div>
        </div>
        <button v-else type="button" class="w-full rounded-lg border border-default bg-elevated/90 px-2.5 py-2 text-left shadow-sm backdrop-blur-sm transition hover:ring-2 hover:ring-primary/50" :class="statuses && statuses.get(item.key) === 'active' ? 'ring-1 ring-primary/50' : ''" @click="onNodeClick(item.node.item)">
          <div class="flex items-center gap-1.5">
            <span v-if="statuses" class="flex size-4 shrink-0 items-center justify-center rounded-full ring-1" :class="[statusMeta[statuses.get(item.key) ?? 'pending'].color, statusMeta[statuses.get(item.key) ?? 'pending'].ring]">
              <UIcon :name="statusMeta[statuses.get(item.key) ?? 'pending'].icon" class="size-2.5" :class="statuses.get(item.key) === 'active' ? 'animate-spin' : ''" />
            </span>
            <UIcon :name="item.node.type === 'recipe' ? 'i-lucide-hammer' : item.node.type === 'any' ? 'i-lucide-shuffle' : 'i-lucide-package'" class="size-3.5 shrink-0" :class="item.node.type === 'recipe' ? 'text-primary' : item.node.type === 'any' ? 'text-info' : 'text-muted'" />
            <span class="truncate text-xs font-medium capitalize" :class="item.node.type === 'recipe' || item.node.type === 'any' ? 'text-highlighted' : 'text-muted'" :title="formatItemKind(item.node.item)">{{ formatItemKind(item.node.item) }}</span>
          </div>
          <div class="mt-1.5 flex items-center justify-between">
            <UBadge :color="item.node.type === 'recipe' ? 'neutral' : item.node.type === 'any' ? 'info' : 'warning'" variant="soft" size="sm">×{{ formatCount(item.node.quantity) }}</UBadge>
            <div class="min-w-0 text-right text-[9px] uppercase tracking-wide">
              <span v-if="item.node.type === 'recipe' && item.node.completedCrafts !== undefined" class="block tabular-nums" :class="item.node.completedCrafts >= item.node.crafts ? 'text-success' : 'text-primary'">{{ item.node.completedCrafts }}/{{ item.node.crafts }} crafts</span>
              <span v-else-if="item.node.type === 'storage' && item.node.completedQuantity !== undefined" class="block tabular-nums" :class="item.node.completedQuantity >= item.node.quantity ? 'text-success' : 'text-warning'">{{ item.node.completedQuantity }}/{{ item.node.quantity }} staged</span>
              <span v-else-if="item.node.type === 'storage'" class="block" :class="item.node.available >= item.node.quantity ? 'text-success' : 'text-error'">{{ item.node.available }}/{{ item.node.quantity }} in storage</span>
              <span v-else-if="item.node.type === 'any'" class="block text-info">any recipe</span>
              <span v-if="item.node.operation" class="block truncate text-muted" :title="item.node.operation.error ?? `${item.node.operation.kind}: ${item.node.operation.state}`">{{ item.node.operation.kind }} · {{ item.node.operation.state }}</span>
            </div>
          </div>
        </button>
        <UButton
          v-if="!readOnly && !statuses && (item.node.type === 'recipe' || item.node.type === 'storage' || item.node.type === 'any') && item.node.selected"
          color="warning"
          variant="solid"
          size="xs"
          icon="i-lucide-undo-2"
          square
          class="absolute -right-2 -top-2 z-10 rounded-full shadow"
          title="Change recipe"
          @pointerdown.stop
          @click="unpick(item.key)"
        />
        <UButton
          v-if="!readOnly && !statuses && item.node.type === 'recipe' && !item.node.selected"
          color="neutral"
          variant="solid"
          size="xs"
          icon="i-lucide-package-search"
          square
          class="absolute -bottom-2 -right-2 z-10 rounded-full shadow"
          title="Wait for this item from storage"
          @pointerdown.stop
          @click="sourceFromStorage(item.key)"
        />
      </div>
    </div>

    <ItemDetailModal v-model:open="detailOpen" :item-kind="selectedItemKind" />
  </div>
</template>

<style scoped>
.craft-node {
  animation: craft-node-in 420ms cubic-bezier(0.22, 1, 0.36, 1) both;
  animation-delay: var(--craft-delay, 0ms);
}

.craft-edge {
  stroke-dasharray: 7 7;
  animation: craft-edge-flow 1.8s linear infinite;
}

.craft-choice {
  animation: craft-choice-in 360ms ease-out both;
}

.craft-option {
  animation: craft-option-in 300ms ease-out both;
  animation-delay: var(--craft-delay, 0ms);
}

@keyframes craft-node-in {
  from { opacity: 0; transform: translateY(8px) scale(0.97); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

@keyframes craft-choice-in {
  from { opacity: 0; transform: scale(0.98); }
  to { opacity: 1; transform: scale(1); }
}

@keyframes craft-option-in {
  from { opacity: 0; transform: translateX(-5px); }
  to { opacity: 1; transform: translateX(0); }
}

@keyframes craft-edge-flow {
  to { stroke-dashoffset: -14; }
}

@media (prefers-reduced-motion: reduce) {
  .craft-node,
  .craft-edge,
  .craft-choice,
  .craft-option {
    animation: none;
  }
}
</style>
