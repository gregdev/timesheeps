<script setup lang="ts">
  import { ref, computed, watch } from 'vue'
  import { useRouter } from 'vue-router'
  import { format, addDays, addWeeks, subWeeks, startOfWeek, isToday } from 'date-fns'
  import { api } from '../api'
  import { useSettingsStore } from '../stores/settings'
  import { useProjectsStore } from '../stores/projects'
  import { useDayStore } from '../stores/day'
  import { useTimeline } from '../composables/useTimeline'
  import type { TimeEntry } from '../schemas'

  const settingsStore = useSettingsStore()
  const projectsStore = useProjectsStore()
  const dayStore = useDayStore()
  const router = useRouter()
  const { formatDuration } = useTimeline()

  interface DayData {
    date: string
    entries: TimeEntry[]
    hasActivity: boolean
  }

  const loading = ref(false)
  const dayData = ref<Map<string, DayData>>(new Map())

  const weekStartsOn = computed(() => settingsStore.settings.weekStartsOn as 0 | 1)
  const weekStart = ref(startOfWeek(new Date(), { weekStartsOn: weekStartsOn.value }))

  const weekDays = computed(() => Array.from({ length: 7 }, (_, i) => addDays(weekStart.value, i)))
  const weekDayStrings = computed(() => weekDays.value.map((d) => format(d, 'yyyy-MM-dd')))

  const weekLabel = computed(() => {
    const start = weekDays.value[0]
    const end = weekDays.value[6]

    if (start.getFullYear() === end.getFullYear()) {
      if (start.getMonth() === end.getMonth()) {
        return `${format(start, 'MMM d')} – ${format(end, 'd, yyyy')}`
      }

      return `${format(start, 'MMM d')} – ${format(end, 'MMM d, yyyy')}`
    }

    return `${format(start, 'MMM d, yyyy')} – ${format(end, 'MMM d, yyyy')}`
  })

  const isCurrentWeek = computed(() => {
    const todayStr = format(new Date(), 'yyyy-MM-dd')
    return weekDayStrings.value.includes(todayStr)
  })

  async function loadWeek() {
    loading.value = true
    const dates = weekDayStrings.value

    try {
      const results = await Promise.all(
        dates.map((date) =>
          Promise.all([api.getTimeEntriesForDay(date), api.getWindowSummaryForDay(date)]),
        ),
      )
      const map = new Map<string, DayData>()

      for (let i = 0; i < dates.length; i++) {
        const [entries, summary] = results[i]
        map.set(dates[i], { date: dates[i], entries, hasActivity: summary.length > 0 })
      }

      dayData.value = map
    } catch (err) {
      console.error('[timesheeps] loadWeek failed:', err)
    } finally {
      loading.value = false
    }
  }

  watch(weekDayStrings, loadWeek, { immediate: true })

  function prevWeek() {
    weekStart.value = subWeeks(weekStart.value, 1)
  }

  function nextWeek() {
    weekStart.value = addWeeks(weekStart.value, 1)
  }

  function thisWeek() {
    weekStart.value = startOfWeek(new Date(), { weekStartsOn: weekStartsOn.value })
  }

  async function navigateTo(date: string) {
    await dayStore.loadDay(date)
    router.push('/')
  }

  const weekProjects = computed(() => {
    const usedIds = new Set<number>()

    for (const data of dayData.value.values()) {
      for (const entry of data.entries) {
        usedIds.add(entry.projectId)
      }
    }

    return projectsStore.projects.filter((p) => !p.archivedAt && usedIds.has(p.id))
  })

  function projectDayMinutes(projectId: number, date: string): number {
    const data = dayData.value.get(date)

    if (!data) {
      return 0
    }

    return data.entries
      .filter((e) => e.projectId === projectId)
      .reduce((sum, e) => sum + (e.endMinutes - e.startMinutes), 0)
  }

  function dayTotalMinutes(date: string): number {
    const data = dayData.value.get(date)

    if (!data) {
      return 0
    }

    return data.entries.reduce((sum, e) => sum + (e.endMinutes - e.startMinutes), 0)
  }

  function hasUnlogged(date: string): boolean {
    const data = dayData.value.get(date)

    if (!data) {
      return false
    }

    return data.hasActivity && dayTotalMinutes(date) === 0
  }

  function fmtMin(min: number): string {
    if (min === 0) {
      return ''
    }

    return formatDuration(min)
  }
</script>

<template>
  <div class="week-view">
    <div class="week-nav">
      <button class="btn-ghost nav-btn" @click="prevWeek">‹</button>

      <span class="week-label" :class="{ 'current-week': isCurrentWeek }">{{ weekLabel }}</span>

      <button class="btn-ghost nav-btn" @click="nextWeek">›</button>

      <button v-if="!isCurrentWeek" class="btn-secondary this-week-btn" @click="thisWeek">
        This week
      </button>
    </div>

    <div v-if="loading" class="loading-msg">Loading…</div>

    <div v-else class="week-grid">
      <!-- Header row -->
      <div class="grid-row header-row">
        <div class="cell label-cell"></div>

        <div
          v-for="(date, i) in weekDayStrings"
          :key="date"
          class="cell day-header"
          :class="{ today: isToday(weekDays[i]) }"
          @click="navigateTo(date)"
        >
          <span class="day-name">{{ format(weekDays[i], 'EEE') }}</span>
          <span class="day-date">{{ format(weekDays[i], 'M/d') }}</span>
          <span
            v-if="hasUnlogged(date)"
            class="unlogged-dot"
            title="Activity recorded but no time logged"
          >
            ●
          </span>
        </div>
      </div>

      <!-- Empty state -->
      <div v-if="weekProjects.length === 0" class="grid-row empty-row">
        <div class="cell empty-cell">No time logged this week</div>
      </div>

      <!-- Project rows -->
      <div v-for="project in weekProjects" :key="project.id" class="grid-row project-row">
        <div class="cell project-label">
          <span class="dot" :style="{ background: project.color }" />
          <span class="project-name">{{ project.name }}</span>
        </div>

        <div
          v-for="date in weekDayStrings"
          :key="date"
          class="cell entry-cell"
          :class="{ 'has-value': projectDayMinutes(project.id, date) > 0 }"
          @click="navigateTo(date)"
        >
          {{ fmtMin(projectDayMinutes(project.id, date)) }}
        </div>
      </div>

      <!-- Total row -->
      <div class="grid-row total-row">
        <div class="cell total-label">Total</div>

        <div
          v-for="date in weekDayStrings"
          :key="date"
          class="cell total-cell"
          :class="{ 'has-value': dayTotalMinutes(date) > 0 }"
          @click="navigateTo(date)"
        >
          {{ fmtMin(dayTotalMinutes(date)) }}
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
  .week-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 16px 20px;
    gap: 16px;
    overflow: auto;
  }

  /* ── Navigation ────────────────────────────────────────────────────────────── */

  .week-nav {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .nav-btn {
    font-size: 18px;
    line-height: 1;
    padding: 4px 10px;
  }

  .week-label {
    font-size: 14px;
    font-weight: 600;
    min-width: 190px;
    text-align: center;
  }

  .week-label.current-week {
    color: var(--primary);
  }

  .this-week-btn {
    margin-left: 4px;
    font-size: 12px;
    padding: 4px 10px;
  }

  .loading-msg {
    color: var(--text-muted);
    font-size: 13px;
  }

  /* ── Grid ──────────────────────────────────────────────────────────────────── */

  .week-grid {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    flex-shrink: 0;
  }

  .grid-row {
    display: grid;
    grid-template-columns: 180px repeat(7, 1fr);
  }

  .cell {
    padding: 8px 10px;
    font-size: 12px;
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    min-width: 0;
  }

  .cell:last-child {
    border-right: none;
  }

  /* ── Header ────────────────────────────────────────────────────────────────── */

  .header-row .cell {
    background: var(--surface-2);
  }

  .day-header {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    cursor: pointer;
    transition: background 0.1s;
    user-select: none;
  }

  .day-header:hover {
    background: var(--surface);
  }

  .day-header.today {
    background: color-mix(in srgb, var(--primary) 8%, var(--surface-2));
  }

  .day-name {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
  }

  .day-header.today .day-name {
    color: var(--primary);
  }

  .day-date {
    font-size: 13px;
    font-weight: 500;
  }

  .day-header.today .day-date {
    color: var(--primary);
    font-weight: 700;
  }

  .unlogged-dot {
    font-size: 8px;
    color: #f97316;
    line-height: 1;
    margin-top: 1px;
  }

  /* ── Empty state ────────────────────────────────────────────────────────────── */

  .empty-cell {
    grid-column: 1 / -1;
    background: var(--surface);
    color: var(--text-muted);
    text-align: center;
    padding: 20px;
  }

  /* ── Project rows ──────────────────────────────────────────────────────────── */

  .project-row .cell {
    background: var(--surface);
    transition: background 0.1s;
  }

  .total-row .cell {
    background: var(--surface-2);
    border-bottom: none;
  }

  .project-row:hover .cell {
    background: var(--surface-2);
  }

  .project-label {
    display: flex;
    align-items: center;
    gap: 7px;
    font-weight: 500;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .project-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-cell {
    text-align: right;
    cursor: pointer;
    color: var(--text-muted);
  }

  .entry-cell.has-value {
    color: var(--text);
    font-weight: 500;
  }

  /* ── Total row ─────────────────────────────────────────────────────────────── */

  .total-label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    display: flex;
    align-items: center;
  }

  .total-cell {
    text-align: right;
    cursor: pointer;
    font-weight: 600;
    color: var(--text-muted);
    transition: background 0.1s;
  }

  .total-cell:hover {
    background: color-mix(in srgb, var(--border) 40%, var(--surface-2));
  }

  .total-cell.has-value {
    color: var(--primary);
  }
</style>
