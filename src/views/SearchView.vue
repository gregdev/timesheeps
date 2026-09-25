<script setup lang="ts">
  import { ref, computed, watch } from 'vue'
  import { useRoute, useRouter } from 'vue-router'
  import { format, parseISO } from 'date-fns'
  import { api } from '../api'
  import { useDayStore } from '../stores/day'
  import { useSettingsStore } from '../stores/settings'
  import type { ActivityBlock, SearchResults } from '../schemas'
  import SearchDayTimeline from '../components/SearchDayTimeline.vue'
  import SearchMatchItem from '../components/SearchMatchItem.vue'
  import ProjectPickerModal from '../components/ProjectPickerModal.vue'
  import { useAppColour } from '../composables/useAppColour'
  import { useEntryModal, type EntryRange } from '../composables/useEntryModal'
  import { useContextMenu } from '../composables/useContextMenu'
  import { useTimeline } from '../composables/useTimeline'
  import {
    cleanWindowTitle,
    formatMinutes,
    formatSeconds,
    localDateKey,
    minutesToClock,
  } from '../composables/useFormat'

  const route = useRoute()
  const router = useRouter()
  const dayStore = useDayStore()
  const settingsStore = useSettingsStore()
  const { appColour: appColor } = useAppColour()
  const { pendingCreate, editingEntry } = useEntryModal()
  const { open: openMenu } = useContextMenu()
  const { isoToMinutes } = useTimeline()

  const targetDate = ref<string | null>(null)
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
    if (!results.value) {
      return []
    }

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

  /** Bars are relative to the widest result, which is not necessarily the first. */
  const maxWindowSecs = computed(() =>
    windowTotals.value.reduce((max, i) => Math.max(max, i.totalSecs), 1),
  )

  const totalMatchedSecs = computed(() => {
    if (!results.value) {
      return 0
    }

    return results.value.days.reduce((s, d) => s + d.totalMatchedSecs, 0)
  })

  const totalResultCount = computed(() => {
    if (!results.value) {
      return 0
    }

    return results.value.days.reduce((s, d) => s + d.matchedBlocks.length, 0)
  })

  function formatDate(dateStr: string): string {
    return format(parseISO(dateStr), 'EEEE, MMMM d, yyyy')
  }

  async function goToDay(date: string) {
    await dayStore.loadDay(date)
    router.push('/')
  }

  // ── Context menu handlers for match items ─────────────────────────────────

  function onTrackToProject(block: ActivityBlock) {
    // `startedAt` is UTC — derive the *local* day the block belongs to.
    targetDate.value = localDateKey(block.startedAt)

    const startMinutes = Math.round(isoToMinutes(block.startedAt))
    const endMinutes = Math.round(isoToMinutes(block.endedAt))

    pendingCreate.value = {
      startMinutes,
      endMinutes,
      note: block.appName,
    }
  }

  async function onCreateIgnoreRule(appName: string) {
    await settingsStore.createRule('app_name', appName)
    // Ignoring an app changes what the search returns, so re-run it.
    doSearch(query.value)
  }

  async function onDeleteBlock(block: ActivityBlock) {
    await api.deleteActivityBlock(block.startedAt, block.endedAt, block.appName, block.windowTitle)
    // Refresh search results
    doSearch(query.value)
  }

  // ── Context menu for window totals sidebar ────────────────────────────────

  function onWsItemContextMenu(
    e: MouseEvent,
    item: { appName: string; windowTitle: string; totalSecs: number },
  ) {
    openMenu(e, [
      {
        label: 'Track to project…',
        action: () => {
          // The sidebar's total is summed from one day's matched blocks for this
          // app+title pair, so track that same day and those same blocks.
          const day = results.value?.days.find((d) =>
            d.matchedBlocks.some(
              (b) => b.appName === item.appName && b.windowTitle === item.windowTitle,
            ),
          )

          if (!day) {
            window.alert(`No matching activity to track for "${item.windowTitle}".`)
            return
          }

          // Blocks are already merged into sessions, so each is one range — and they
          // stay separate entries, never one span swallowing the gaps between them.
          const ranges = day.matchedBlocks
            .filter((b) => b.appName === item.appName && b.windowTitle === item.windowTitle)
            .sort((a, b) => a.startedAt.localeCompare(b.startedAt))
            .map((b) => ({
              startMinutes: Math.round(isoToMinutes(b.startedAt)),
              endMinutes: Math.round(isoToMinutes(b.endedAt)),
            }))
            .filter((r) => r.endMinutes > r.startMinutes)

          if (ranges.length === 0) {
            window.alert(`No sessions to track for "${item.windowTitle}".`)
            return
          }

          targetDate.value = day.date
          pendingCreate.value = {
            startMinutes: ranges[0].startMinutes,
            endMinutes: ranges[0].endMinutes,
            ranges,
            note: item.appName,
          }
        },
      },
      {
        label: 'Create ignore rule',
        action: async () => {
          await settingsStore.createRule('app_name', item.appName)
          doSearch(query.value)
        },
      },
      {
        label: 'Delete all',
        danger: true,
        action: async () => {
          await api.deleteActivityByAppTitle(item.appName, item.windowTitle)
          doSearch(query.value)
        },
      },
    ])
  }

  // ── Modal handlers ────────────────────────────────────────────────────────

  async function onModalSave(
    projectId: number,
    ranges: EntryRange[],
    note: string,
    // The modal always emits this; search results never offer auto-track.
    _autoTrack: boolean,
  ) {
    if (editingEntry.value) {
      const [range] = ranges

      await dayStore.updateEntry(
        editingEntry.value.id,
        projectId,
        range.startMinutes,
        range.endMinutes,
        note,
      )
      editingEntry.value = null
    } else if (pendingCreate.value) {
      // Ensure the correct day is loaded
      if (targetDate.value) {
        await dayStore.loadDay(targetDate.value)
        targetDate.value = null
      }

      await dayStore.createEntries(projectId, ranges, note)
      pendingCreate.value = null
    }
  }

  async function onModalDelete(id: number) {
    await dayStore.deleteEntry(id)
    editingEntry.value = null
  }

  function onModalCancel() {
    pendingCreate.value = null
    editingEntry.value = null
    targetDate.value = null
  }

  /** Whichever entry the picker is editing/creating — it covers both flows. */
  const modalTarget = computed(() => pendingCreate.value ?? editingEntry.value)

  /** Search flows always cover a single range, never several sessions. */
  const modalRanges = computed<EntryRange[]>(() => {
    if (editingEntry.value) {
      return [
        {
          startMinutes: editingEntry.value.startMinutes,
          endMinutes: editingEntry.value.endMinutes,
        },
      ]
    }

    const pending = pendingCreate.value

    if (!pending) {
      return []
    }

    return pending.ranges?.length
      ? pending.ranges
      : [{ startMinutes: pending.startMinutes, endMinutes: pending.endMinutes }]
  })

  const modalProjectId = computed<number | null>(() => modalTarget.value?.projectId ?? null)
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
          <span class="search-total">{{ formatSeconds(totalMatchedSecs) }} total</span>
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
              {{ formatSeconds(day.totalMatchedSecs) }}
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
            <SearchMatchItem
              v-for="block in day.matchedBlocks"
              :key="block.startedAt"
              :block="block"
              @track-to-project="onTrackToProject"
              @create-ignore-rule="onCreateIgnoreRule"
              @delete="onDeleteBlock"
            />
          </ul>
        </div>

        <!-- Time entry note matches -->
        <div v-if="results.noteMatches.length > 0" class="note-section">
          <div class="note-section-header">Time Entry Notes ({{ results.noteMatches.length }})</div>
          <ul class="note-list">
            <li v-for="entry in results.noteMatches" :key="entry.id" class="note-item">
              <span class="note-date">{{ entry.date }}</span>
              <span class="note-time">
                {{ minutesToClock(entry.startMinutes) }} – {{ minutesToClock(entry.endMinutes) }}
              </span>
              <span class="note-text">{{ entry.note }}</span>
              <span class="note-dur">
                {{ formatMinutes(entry.endMinutes - entry.startMinutes) }}
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
        <span class="ws-total">{{ formatSeconds(totalMatchedSecs) }}</span>
      </div>
      <ul class="ws-list">
        <li
          v-for="item in windowTotals"
          :key="item.appName + item.windowTitle"
          class="ws-item"
          data-tooltip="Right-click for actions"
          @contextmenu="onWsItemContextMenu($event, item)"
        >
          <div class="ws-bar-wrap">
            <div
              class="ws-bar"
              :style="{
                width: (item.totalSecs / maxWindowSecs) * 100 + '%',
                background: appColor(item.appName),
              }"
            />
          </div>
          <div class="ws-labels">
            <span class="ws-app" :style="{ color: appColor(item.appName) }">
              {{ item.appName }}
            </span>
            <span v-if="cleanWindowTitle(item.appName, item.windowTitle)" class="ws-window">
              {{ cleanWindowTitle(item.appName, item.windowTitle) }}
            </span>
          </div>
          <span class="ws-dur">{{ formatSeconds(item.totalSecs) }}</span>
        </li>
      </ul>
    </aside>

    <!-- Project picker modal for Track to project… -->
    <ProjectPickerModal
      v-if="modalTarget"
      :initial-ranges="modalRanges"
      :initial-project-id="modalProjectId"
      :initial-note="modalTarget.note"
      :entry-id="editingEntry?.id ?? null"
      @save="onModalSave"
      @delete="onModalDelete"
      @cancel="onModalCancel"
    />
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
