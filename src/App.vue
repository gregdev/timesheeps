<script setup lang="ts">
  import { onMounted, onUnmounted, ref } from 'vue'
  import { RouterView, RouterLink, useRoute } from 'vue-router'
  import { listen } from '@tauri-apps/api/event'
  import { useProjectsStore } from './stores/projects'
  import { useSettingsStore } from './stores/settings'
  import { useDayStore } from './stores/day'
  import IdlePrompt from './components/IdlePrompt.vue'
  import TooltipOverlay from './components/TooltipOverlay.vue'
  import ContextMenu from './components/ContextMenu.vue'
  import type { IdleReturnEvent } from './schemas'

  const route = useRoute()
  const projectsStore = useProjectsStore()
  const settingsStore = useSettingsStore()
  const dayStore = useDayStore()
  const idleEvent = ref<IdleReturnEvent | null>(null)

  function onKeyDown(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey) return
    const tag = (e.target as HTMLElement).tagName
    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return
    if (e.key === 'ArrowLeft') {
      e.preventDefault()
      dayStore.prevDay()
    } else if (e.key === 'ArrowRight') {
      e.preventDefault()
      dayStore.nextDay()
    } else if (e.key === 't' || e.key === 'T') {
      dayStore.goToday()
    }
  }

  onMounted(async () => {
    window.addEventListener('keydown', onKeyDown)
    await Promise.all([
      projectsStore
        .load()
        .catch((e: unknown) => console.error('[timesheeps] projects load failed:', e)),
      settingsStore
        .load()
        .catch((e: unknown) => console.error('[timesheeps] settings load failed:', e)),
    ])
    await dayStore.loadDay()
    listen<IdleReturnEvent>('idle-return', (event) => {
      idleEvent.value = event.payload
    })
  })

  onUnmounted(() => {
    window.removeEventListener('keydown', onKeyDown)
  })
</script>

<template>
  <div class="app">
    <nav class="app-nav">
      <RouterLink to="/" class="nav-link" :class="{ active: route.path === '/' }">
        Timeline
      </RouterLink>

      <RouterLink to="/week" class="nav-link" :class="{ active: route.path === '/week' }">
        Week
      </RouterLink>

      <RouterLink to="/settings" class="nav-link" :class="{ active: route.path === '/settings' }">
        Settings
      </RouterLink>
    </nav>

    <main class="app-main">
      <RouterView />
    </main>

    <IdlePrompt v-if="idleEvent" :event="idleEvent" @dismiss="idleEvent = null" />
    <TooltipOverlay />
    <ContextMenu />
  </div>
</template>

<style scoped>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }

  .app-nav {
    height: var(--nav-height);
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 16px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    flex-shrink: 0;
  }

  .nav-link {
    padding: 5px 12px;
    border-radius: var(--radius);
    color: var(--text-muted);
    text-decoration: none;
    font-weight: 500;
    font-size: 13px;
    transition:
      background 0.15s,
      color 0.15s;
  }

  .nav-link:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .nav-link.active {
    background: var(--primary);
    color: #fff;
  }

  .app-main {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }
</style>
