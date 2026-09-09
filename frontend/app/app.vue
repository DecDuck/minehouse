<script setup lang="ts">
const route = useRoute()
const nav = [
  { to: '/', label: 'Storage', icon: 'i-lucide-package-search' },
  { to: '/containers', label: 'Containers', icon: 'i-lucide-boxes' },
  { to: '/crafting', label: 'Crafting', icon: 'i-lucide-hammer' },
  { to: '/operations', label: 'Operations', icon: 'i-lucide-activity' },
  { to: '/regions', label: 'Regions', icon: 'i-lucide-map' },
]
</script>

<template>
  <UApp>
    <div class="min-h-screen bg-transparent lg:flex">
      <aside class="border-b border-blue-950/80 bg-[#00030a] text-white shadow-[8px_0_30px_rgba(0,68,255,0.08)] lg:fixed lg:inset-y-0 lg:flex lg:w-72 lg:flex-col lg:border-b-0 lg:border-r lg:border-blue-950/80">
        <div class="px-5 py-5 lg:px-6 lg:py-7">
          <NuxtLink to="/" class="block">
            <p class="mono text-[10px] uppercase tracking-[0.22em] text-blue-400">Minehouse / control</p>
            <h1 class="mt-2 text-2xl font-semibold tracking-tight">Minehouse</h1>
          </NuxtLink>
        </div>
        <nav class="flex gap-2 overflow-x-auto px-5 pb-5 lg:mt-4 lg:grid lg:overflow-visible lg:px-4" aria-label="Primary navigation">
          <UButton v-for="item in nav" :key="item.to" :to="item.to" :variant="route.path === item.to ? 'solid' : 'ghost'" color="neutral" class="justify-start whitespace-nowrap lg:w-full" :icon="item.icon">
            {{ item.label }}
          </UButton>
        </nav>
      </aside>
      <main class="min-h-screen min-w-0 flex-1 bg-black lg:ml-72">
        <div class="w-full px-4 py-6 sm:px-6 lg:px-10 lg:py-10">
          <NuxtPage />
        </div>
      </main>
    </div>
  </UApp>
</template>
