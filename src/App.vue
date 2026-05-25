<script setup lang="ts">
  import { onMounted, onUnmounted, ref, watch } from 'vue'
  import { RouterView, RouterLink, useRoute, useRouter } from 'vue-router'
  import { listen } from '@tauri-apps/api/event'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { useProjectsStore } from './stores/projects'
  import { useSettingsStore } from './stores/settings'
  import { useDayStore } from './stores/day'
  import IdlePrompt from './components/IdlePrompt.vue'
  import TooltipOverlay from './components/TooltipOverlay.vue'
  import ContextMenu from './components/ContextMenu.vue'
  import type { IdleReturnEvent } from './schemas'

  const route = useRoute()
  const router = useRouter()
  const projectsStore = useProjectsStore()
  const settingsStore = useSettingsStore()
  const dayStore = useDayStore()
  const idleEvent = ref<IdleReturnEvent | null>(null)
  const searchQuery = ref('')
  const searchInput = ref<HTMLInputElement | null>(null)

  // Keep search input in sync when navigating to /search
  watch(
    () => route.query.q,
    (q) => {
      searchQuery.value = (q as string) || ''
    },
    { immediate: true },
  )

  function doSearch() {
    const q = searchQuery.value.trim()
    if (!q) return
    router.push({ path: '/search', query: { q } })
  }

  function cancelSearch() {
    searchQuery.value = ''
    searchInput.value?.blur()
    if (route.path === '/search') router.back()
  }

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
    } else if (e.key === '/') {
      e.preventDefault()
      searchInput.value?.focus()
      searchInput.value?.select()
    }
  }

  let unlistenFocus: (() => void) | null = null

  onMounted(async () => {
    window.addEventListener('keydown', onKeyDown)
    unlistenFocus = await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
      if (focused) dayStore.refreshCurrentDate()
    })
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
    unlistenFocus?.()
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

      <RouterLink
        to="/pay-period"
        class="nav-link"
        :class="{ active: route.path === '/pay-period' }"
      >
        Pay Period
      </RouterLink>

      <RouterLink to="/settings" class="nav-link" :class="{ active: route.path === '/settings' }">
        Settings
      </RouterLink>

      <RouterLink to="/about" class="nav-link" :class="{ active: route.path === '/about' }">
        About
      </RouterLink>

      <div class="nav-search">
        <input
          ref="searchInput"
          v-model="searchQuery"
          type="search"
          class="search-input"
          placeholder="Search… (/)"
          @keydown.enter="doSearch"
          @keydown.escape="cancelSearch"
        />
      </div>
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
    gap: var(--space-1);
    padding: 0 var(--space-4);
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    flex-shrink: 0;
  }

  .nav-link {
    padding: 5px var(--space-3);
    border-radius: var(--radius);
    color: var(--text-muted);
    text-decoration: none;
    font-weight: 500;
    font-size: var(--text-sm);
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

  .nav-search {
    margin-left: auto;
    display: flex;
    align-items: center;
  }

  .search-input {
    width: 190px;
    height: 28px;
    padding: 0 10px;
    border-radius: 14px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text);
    font-size: var(--text-sm);
    outline: none;
    transition:
      border-color 0.15s,
      width 0.2s;
  }

  .search-input:focus {
    border-color: var(--primary);
    width: 260px;
  }

  .search-input::placeholder {
    color: var(--text-muted);
  }

  /* Chrome/Safari search cancel button */
  .search-input::-webkit-search-cancel-button {
    opacity: 0.4;
    cursor: pointer;
  }
</style>
