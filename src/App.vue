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
  const showSearchHelp = ref(false)

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

    if (!q) {
      return
    }

    router.push({ path: '/search', query: { q } })
  }

  function cancelSearch() {
    searchQuery.value = ''
    searchInput.value?.blur()

    if (route.path === '/search') {
      router.back()
    }
  }

  function onKeyDown(e: KeyboardEvent) {
    if (e.metaKey || e.ctrlKey || e.altKey) {
      return
    }

    const tag = (e.target as HTMLElement).tagName

    if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') {
      return
    }
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
      if (focused) {
        dayStore.refreshCurrentDate()
      }
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
        <button
          class="search-help-btn"
          :class="{ active: showSearchHelp }"
          title="Search operators"
          @click.stop="showSearchHelp = !showSearchHelp"
        >
          ?
        </button>
        <template v-if="showSearchHelp">
          <div class="search-help-backdrop" @click="showSearchHelp = false" />
          <div class="search-help-popover">
            <div class="search-help-title">Search operators</div>
            <table class="search-help-table">
              <tbody>
                <tr>
                  <td><code>word</code></td>
                  <td>Match app or title</td>
                </tr>
                <tr>
                  <td><code>word1 word2</code></td>
                  <td>Both must match (AND)</td>
                </tr>
                <tr>
                  <td><code>app:word</code></td>
                  <td>App name only</td>
                </tr>
                <tr>
                  <td><code>title:word</code></td>
                  <td>Window title only</td>
                </tr>
                <tr>
                  <td><code>-word</code></td>
                  <td>Exclude results</td>
                </tr>
                <tr>
                  <td><code>date:2026-05-25</code></td>
                  <td>Exact date</td>
                </tr>
                <tr>
                  <td><code>after:2026-05-01</code></td>
                  <td>From date onward</td>
                </tr>
                <tr>
                  <td><code>before:2026-05-25</code></td>
                  <td>Up to date</td>
                </tr>
              </tbody>
            </table>
          </div>
        </template>
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
    position: relative;
  }

  .search-help-btn {
    width: 20px;
    height: 20px;
    margin-left: var(--space-1);
    border-radius: 50%;
    border: 1px solid var(--border);
    background: none;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition:
      background 0.15s,
      color 0.15s;
  }

  .search-help-btn:hover,
  .search-help-btn.active {
    background: var(--surface-2);
    color: var(--text);
  }

  .search-help-backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
  }

  .search-help-popover {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    z-index: 51;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    box-shadow: 0 4px 16px rgb(0 0 0 / 15%);
    padding: var(--space-3);
    min-width: 280px;
  }

  .search-help-title {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: var(--space-2);
  }

  .search-help-table {
    border-collapse: collapse;
    width: 100%;
    font-size: var(--text-xs);
  }

  .search-help-table td {
    padding: 3px var(--space-2) 3px 0;
    color: var(--text);
    vertical-align: top;
  }

  .search-help-table td:last-child {
    color: var(--text-faint);
  }

  .search-help-table code {
    font-family: monospace;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 1px 4px;
    font-size: 11px;
    white-space: nowrap;
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
