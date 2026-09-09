<script setup lang="ts">
const { formatItemKind, formatCount } = useFormat()
const { crafts, recipes, loadRecipes, loadCrafts } = useCrafting()
await loadRecipes()
await loadCrafts()
const items = computed(() => recipes.value.map((recipe) => ({ label: formatItemKind(recipe.output_item_kind), value: recipe.output_item_kind })))
const target = ref('')
const amount = ref(4)
function plan() {
    if (target.value && amount.value > 0) navigateTo({ path: '/crafting/new', query: { item: target.value, amount: amount.value } })
}
const active = computed(() => crafts.value.filter((craft) => craft.state === 'queued' || craft.state === 'waiting' || craft.state === 'running').length)
let refreshTimer: ReturnType<typeof setInterval> | undefined
onMounted(() => { refreshTimer = setInterval(() => { void loadCrafts() }, 2000) })
onUnmounted(() => { if (refreshTimer) clearInterval(refreshTimer) })
</script>

<template>
    <section>
        <header class="flex items-center justify-between gap-4">
            <div>
                <h2 class="text-2xl font-semibold tracking-tight text-highlighted sm:text-3xl">Crafting</h2>
                <p class="mt-1 text-xs text-muted">Queue new crafts and monitor operations in progress.</p>
            </div>
            <div class="flex items-center gap-2">
                <div class="rounded-lg border border-default bg-elevated/50 px-3 py-1.5 text-right">
                    <p class="text-[10px] uppercase tracking-wide text-muted">Active</p>
                    <p class="text-base font-semibold tabular-nums text-highlighted">{{ active }}</p>
                </div>
                <UBadge color="neutral" variant="soft" size="sm">{{ crafts.length }}</UBadge>
            </div>
        </header>
        <UCard class="mt-4" :ui="{ body: 'p-2 sm:p-3' }">
            <div class="flex flex-wrap items-end gap-2">
                <UFormField label="Item" class="min-w-48 flex-1">
                    <USelectMenu v-model="target" :items="items" value-key="value" icon="i-lucide-search"
                        class="w-full" />
                </UFormField>
                <UFormField label="Amount" class="w-28">
                    <UInput v-model.number="amount" type="number" min="1" class="w-full" />
                </UFormField>``
                <UButton color="primary" icon="i-lucide-arrow-right" trailing @click="plan">Plan craft</UButton>
            </div>
        </UCard>
        <div class="mt-6">
            <div class="mb-2 flex items-center justify-between">
                <h3 class="text-sm font-semibold text-highlighted">Operations</h3>
                <UBadge color="neutral" variant="soft" size="sm">{{ crafts.length }}</UBadge>
            </div>
            <div class="grid gap-2 sm:grid-cols-2 xl:grid-cols-3">
                <UCard v-for="craft in crafts" :key="craft.id" as="button" variant="subtle"
                    class="text-left transition hover:ring-2 hover:ring-primary/50" :ui="{ body: 'p-3' }"
                    @click="navigateTo(`/crafting/${craft.id}`)">
                    <div class="flex items-center justify-between gap-2"><span
                            class="truncate font-medium capitalize text-highlighted">{{ formatItemKind(craft.target_item_kind)
                            }}</span>
                        <UBadge
                            :color="craft.state === 'completed' ? 'success' : craft.state === 'failed' ? 'error' : 'neutral'"
                            variant="soft" size="sm">{{ craft.state }}</UBadge>
                    </div>
                    <UProgress
                        :model-value="craft.target_quantity ? Math.round(craft.completed_quantity / craft.target_quantity * 100) : 0"
                        :color="craft.state === 'completed' ? 'success' : craft.state === 'failed' ? 'error' : 'primary'"
                        size="sm" class="mt-3" />
                    <div class="mt-1.5 flex items-center justify-between text-[10px] text-muted"><span class="mono">{{
                        craft.id }}</span><span>{{ craft.completed_quantity }}/{{ craft.target_quantity }}</span></div>
                </UCard>
                <UEmpty v-if="!crafts.length" class="col-span-full py-12" icon="i-lucide-inbox" title="No crafts queued"
                    description="Queue a craft to see it here." />
            </div>
        </div>
    </section>
</template>
