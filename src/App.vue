<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { RouterView, RouterLink, useRoute } from 'vue-router'
import { listen } from '@tauri-apps/api/event'
import { useProjectsStore } from './stores/projects'
import { useSettingsStore } from './stores/settings'
import { useDayStore } from './stores/day'
import IdlePrompt from './components/IdlePrompt.vue'
import type { IdleReturnEvent } from './schemas'

const route = useRoute()
const projectsStore = useProjectsStore()
const settingsStore = useSettingsStore()
const dayStore = useDayStore()
const idleEvent = ref<IdleReturnEvent | null>(null)

onMounted(async () => {
  await Promise.all([projectsStore.load(), settingsStore.load()])
  await dayStore.loadDay()
  listen<IdleReturnEvent>('idle-return', (event) => {
    idleEvent.value = event.payload
  })
})
</script>

<template>
  <div class="app">
    <nav class="app-nav">
      <span class="brand">timesheeps</span>
      <RouterLink to="/" class="nav-link" :class="{ active: route.path === '/' }">Timeline</RouterLink>
      <RouterLink to="/settings" class="nav-link" :class="{ active: route.path === '/settings' }">Settings</RouterLink>
    </nav>
    <main class="app-main">
      <RouterView />
    </main>
    <IdlePrompt v-if="idleEvent" :event="idleEvent" @dismiss="idleEvent = null" />
  </div>
</template>

<style scoped>
.app { display: flex; flex-direction: column; height: 100vh; overflow: hidden; }
.app-nav { height: var(--nav-height); display: flex; align-items: center; gap: 4px; padding: 0 16px; border-bottom: 1px solid var(--border); background: var(--surface); flex-shrink: 0; }
.brand { font-weight: 600; font-size: 14px; margin-right: 12px; color: var(--text); }
.nav-link { padding: 5px 12px; border-radius: var(--radius); color: var(--text-muted); text-decoration: none; font-weight: 500; font-size: 13px; transition: background 0.15s, color 0.15s; }
.nav-link:hover { background: var(--surface-2); color: var(--text); }
.nav-link.active { background: var(--primary); color: #fff; }
.app-main { flex: 1; overflow: hidden; display: flex; flex-direction: column; }
</style>
