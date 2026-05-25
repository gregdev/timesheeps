<script setup lang="ts">
  import { ref, computed, watch } from 'vue'
  import { useRoute, useRouter } from 'vue-router'
  import { format, parseISO } from 'date-fns'
  import { api } from '../api'
  import { useDayStore } from '../stores/day'
  import { useSettingsStore } from '../stores/settings'
  import type { SearchResults } from '../schemas'
  import SearchDayTimeline from '../components/SearchDayTimeline.vue'
  import { useAppColour } from '../composables/useAppColour'

  const route = useRoute()
  const router = useRouter()
  const dayStore = useDayStore()
  const settingsStore = useSettingsStore()
  const { appColour: appColor } = useAppColour()

  const loading = ref(false)
  const results = ref<SearchResults | null>(null)
  const error = ref<string | null>(null)

  const query = computed(() => (route.query.q as string) || '')

  async function doSearch(q: string) {
    if (!q.trim()) {
      results.value = null
      return
    }
    loading.value = true
    error.value = null
    try {
      results.value = await api.search(q.trim())
    } catch (err) {
      error.value = err instanceof Error ? err.message : String(err)
    } finally {
      loading.value = false
    }
  }

  watch(query, (q) => doSearch(q), { immediate: true })

  // Aggregate matched blocks across all days by (appName, windowTitle)
  const windowTotals = computed(() => {
    if (!results.value) return []
    const map = new Map<string, { appName: string; windowTitle: string; totalSecs: number }>()
    for (const day of results.value.days) {
      for (const block of day.matchedBlocks) {
        const key = `${block.appName}\x00${block.windowTitle}`
        const existing = map.get(key)
        if (existing) {
          existing.totalSecs += block.durationSecs
        } else {
          map.set(key, {
            appName: block.appName,
            windowTitle: block.windowTitle,
            totalSecs: block.durationSecs,
          })
        }
      }
    }
    return [...map.values()].sort((a, b) => b.totalSecs - a.totalSecs)
  })

  const totalMatchedSecs = computed(() => {
    if (!results.value) return 0
    return results.value.days.reduce((s, d) => s + d.totalMatchedSecs, 0)
  })

  const totalResultCount = computed(() => {
    if (!results.value) return 0
    return results.value.days.reduce((s, d) => s + d.matchedBlocks.length, 0)
  })

  function formatDuration(secs: number): string {
    const h = Math.floor(secs / 3600)
    const m = Math.floor((secs % 3600) / 60)
    if (h === 0) return `${m}m`
    if (m === 0) return `${h}h`
    return `${h}h ${m}m`
  }

  function formatDate(dateStr: string): string {
    return format(parseISO(dateStr), 'EEEE, MMMM d, yyyy')
  }

  function formatTimeRange(startIso: string, endIso: string): string {
    const s = new Date(startIso)
    const e = new Date(endIso)
    const fmt = (d: Date) =>
      `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`
    return `${fmt(s)} – ${fmt(e)}`
  }

  function minutesToTime(min: number): string {
    const h = Math.floor(min / 60)
    const m = min % 60
    return `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}`
  }

  function cleanTitle(appName: string, title: string): string {
    const parts = title.split(' \u2014 ')
    if (parts.length > 1 && parts[parts.length - 1].toLowerCase() === appName.toLowerCase()) {
      parts.pop()
    }
    const cleaned = parts.join(' \u2014 ')
    return cleaned.toLowerCase() === appName.toLowerCase() ? '' : cleaned
  }

  async function goToDay(date: string) {
    await dayStore.loadDay(date)
    router.push('/')
  }
</script>

<template>
  <div class="search-view">
    <!-- Main results column -->
    <div class="search-main">
      <div class="search-header">
        <template v-if="loading">Searching…</template>
        <template v-else-if="error">
          <span class="search-error">Error: {{ error }}</span>
        </template>
        <template v-else-if="results">
          <span class="search-count">
            {{ totalResultCount }} match{{ totalResultCount === 1 ? '' : 'es' }}
          </span>
          <span class="search-sep">·</span>
          <span class="search-total">{{ formatDuration(totalMatchedSecs) }} total</span>
          <template v-if="results.noteMatches.length > 0">
            <span class="search-sep">·</span>
            <span class="search-notes">
              {{ results.noteMatches.length }} note
              {{ results.noteMatches.length === 1 ? 'match' : 'matches' }}
            </span>
          </template>
        </template>
        <template v-else>
          <span class="search-hint">Search window titles, app names, and time entry notes</span>
        </template>
      </div>

      <template v-if="results">
        <div
          v-if="results.days.length === 0 && results.noteMatches.length === 0"
          class="search-empty"
        >
          No results for "{{ query }}"
        </div>

        <div v-for="day in results.days" :key="day.date" class="day-group">
          <div class="day-header">
            <span class="day-date">{{ formatDate(day.date) }}</span>
            <span class="day-meta">
              {{ day.matchedBlocks.length }}
              match{{ day.matchedBlocks.length === 1 ? '' : 'es' }}
              ·
              {{ formatDuration(day.totalMatchedSecs) }}
            </span>
            <button class="day-link" @click="goToDay(day.date)">View day →</button>
          </div>

          <SearchDayTimeline
            :all-blocks="day.allBlocks"
            :matched-blocks="day.matchedBlocks"
            :start-hour="settingsStore.settings.timelineStartHour"
            :end-hour="settingsStore.settings.timelineEndHour"
          />

          <ul class="match-list">
            <li v-for="block in day.matchedBlocks" :key="block.startedAt" class="match-item">
              <span class="match-time">
                {{ formatTimeRange(block.startedAt, block.endedAt) }}
              </span>
              <span class="match-app" :style="{ color: appColor(block.appName) }">
                {{ block.appName }}
              </span>
              <span class="match-title">{{ block.windowTitle }}</span>
              <span class="match-dur">{{ formatDuration(block.durationSecs) }}</span>
            </li>
          </ul>
        </div>

        <!-- Time entry note matches -->
        <div v-if="results.noteMatches.length > 0" class="note-section">
          <div class="note-section-header">Time Entry Notes ({{ results.noteMatches.length }})</div>
          <ul class="note-list">
            <li v-for="entry in results.noteMatches" :key="entry.id" class="note-item">
              <span class="note-date">{{ entry.date }}</span>
              <span class="note-time">
                {{ minutesToTime(entry.startMinutes) }} – {{ minutesToTime(entry.endMinutes) }}
              </span>
              <span class="note-text">{{ entry.note }}</span>
              <span class="note-dur">
                {{ formatDuration((entry.endMinutes - entry.startMinutes) * 60) }}
              </span>
            </li>
          </ul>
        </div>
      </template>
    </div>

    <!-- Window totals sidebar -->
    <aside v-if="results && windowTotals.length > 0" class="ws-sidebar">
      <div class="ws-header">
        <span class="ws-title">Matched Windows</span>
        <span class="ws-total">{{ formatDuration(totalMatchedSecs) }}</span>
      </div>
      <ul class="ws-list">
        <li v-for="item in windowTotals" :key="item.appName + item.windowTitle" class="ws-item">
          <div class="ws-bar-wrap">
            <div
              class="ws-bar"
              :style="{
                width: (item.totalSecs / (windowTotals[0]?.totalSecs || 1)) * 100 + '%',
                background: appColor(item.appName),
              }"
            />
          </div>
          <div class="ws-labels">
            <span class="ws-app" :style="{ color: appColor(item.appName) }">
              {{ item.appName }}
            </span>
            <span v-if="cleanTitle(item.appName, item.windowTitle)" class="ws-window">
              {{ cleanTitle(item.appName, item.windowTitle) }}
            </span>
          </div>
          <span class="ws-dur">{{ formatDuration(item.totalSecs) }}</span>
        </li>
      </ul>
    </aside>
  </div>
</template>

<style scoped>
  .search-view {
    display: flex;
    height: 100%;
    overflow: hidden;
  }

  /* ── Main scroll area ─────────────────────────────────────────────────────── */

  .search-main {
    flex: 1;
    overflow-y: auto;
    padding: var(--space-4) var(--space-5);
    min-width: 0;
  }

  .search-header {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-bottom: var(--space-5);
    font-size: var(--text-sm);
    color: var(--text-muted);
    min-height: 24px;
  }

  .search-count {
    font-weight: 600;
    color: var(--text);
  }

  .search-sep {
    opacity: 0.4;
  }

  .search-total {
    color: var(--primary);
    font-weight: 500;
  }

  .search-hint {
    font-style: italic;
  }

  .search-error {
    color: var(--danger);
  }

  .search-empty {
    padding: var(--space-10) 0;
    text-align: center;
    color: var(--text-muted);
    font-size: var(--text-sm);
  }

  /* ── Day groups ──────────────────────────────────────────────────────────── */

  .day-group {
    margin-bottom: var(--space-7);
  }

  .day-header {
    display: flex;
    align-items: baseline;
    gap: var(--space-3);
    margin-bottom: var(--space-1);
  }

  .day-date {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
  }

  .day-meta {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .day-link {
    margin-left: auto;
    font-size: var(--text-xs);
    color: var(--primary);
    background: none;
    border: none;
    cursor: pointer;
    padding: 0;
    font-weight: 500;
  }

  .day-link:hover {
    text-decoration: underline;
  }

  /* ── Match list ──────────────────────────────────────────────────────────── */

  .match-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .match-item {
    display: grid;
    grid-template-columns: 110px auto 1fr auto;
    align-items: center;
    gap: 10px;
    padding: 5px 8px;
    border-radius: var(--radius);
    font-size: 12px;
    transition: background 0.12s;
  }

  .match-item:hover {
    background: color-mix(in srgb, var(--border) 40%, transparent);
  }

  .match-time {
    color: var(--text-muted);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .match-app {
    font-weight: 600;
    white-space: nowrap;
  }

  .match-title {
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .match-dur {
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  /* ── Note matches ────────────────────────────────────────────────────────── */

  .note-section {
    margin-top: 24px;
  }

  .note-section-header {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
    margin-bottom: 8px;
    padding: 0 8px;
  }

  .note-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .note-item {
    display: grid;
    grid-template-columns: 90px 110px 1fr auto;
    align-items: center;
    gap: 10px;
    padding: 5px 8px;
    border-radius: var(--radius);
    font-size: 12px;
    transition: background 0.12s;
  }

  .note-item:hover {
    background: color-mix(in srgb, var(--border) 40%, transparent);
  }

  .note-date {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .note-time {
    color: var(--text-muted);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .note-text {
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .note-dur {
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  /* ── Window totals sidebar ───────────────────────────────────────────────── */

  .ws-sidebar {
    width: 220px;
    flex-shrink: 0;
    border-left: 1px solid var(--border);
    background: var(--surface);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .ws-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 10px 14px 8px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .ws-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .ws-total {
    font-size: 11px;
    color: var(--text-muted);
  }

  .ws-list {
    list-style: none;
    margin: 0;
    padding: 6px 0;
    overflow-y: auto;
    flex: 1;
  }

  .ws-item {
    display: grid;
    grid-template-columns: 1fr auto;
    grid-template-rows: auto auto;
    gap: 0 8px;
    padding: 6px 14px;
    transition: background 0.12s;
  }

  .ws-item:hover {
    background: color-mix(in srgb, var(--border) 40%, transparent);
  }

  .ws-bar-wrap {
    grid-column: 1 / -1;
    height: 3px;
    background: var(--border);
    border-radius: 2px;
    margin-bottom: 5px;
    overflow: hidden;
  }

  .ws-bar {
    height: 100%;
    border-radius: 2px;
    opacity: 0.7;
    transition: width 0.3s ease;
  }

  .ws-labels {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .ws-app {
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ws-window {
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ws-dur {
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    align-self: center;
  }
</style>
