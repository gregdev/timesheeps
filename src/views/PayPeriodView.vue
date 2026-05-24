<script setup lang="ts">
  import { ref, computed, watch } from 'vue'
  import { useRouter } from 'vue-router'
  import {
    format,
    addDays,
    subDays,
    differenceInDays,
    startOfWeek,
    isToday,
    parseISO,
  } from 'date-fns'
  import { api } from '../api'
  import { useSettingsStore } from '../stores/settings'
  import { useProjectsStore } from '../stores/projects'
  import { useDayStore } from '../stores/day'
  import { useTimeline } from '../composables/useTimeline'
  import PeriodSummary from '../components/PeriodSummary.vue'
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

  // ── Pay period computation ────────────────────────────────────────────────

  function currentPayPeriodStart(anchorStr: string, freq: 'weekly' | 'fortnightly'): Date {
    const anchor = parseISO(anchorStr)
    const today = new Date()
    if (freq === 'weekly') {
      const dow = anchor.getDay() as 0 | 1 | 2 | 3 | 4 | 5 | 6
      return startOfWeek(today, { weekStartsOn: dow })
    } else {
      const diff = differenceInDays(today, anchor)
      const periodsElapsed = diff >= 0 ? Math.floor(diff / 14) : Math.ceil(diff / 14)
      return addDays(anchor, periodsElapsed * 14)
    }
  }

  const freq = computed(
    () => settingsStore.settings.payScheduleFrequency as 'weekly' | 'fortnightly',
  )
  const anchor = computed(() => settingsStore.settings.payScheduleAnchor)
  const periodLength = computed(() => (freq.value === 'fortnightly' ? 14 : 7))

  const periodStart = ref(currentPayPeriodStart(anchor.value, freq.value))

  // Re-anchor when settings change
  watch([freq, anchor], ([f, a]) => {
    periodStart.value = currentPayPeriodStart(a, f as 'weekly' | 'fortnightly')
  })

  const periodDays = computed(() =>
    Array.from({ length: periodLength.value }, (_, i) => addDays(periodStart.value, i)),
  )
  const periodDayStrings = computed(() => periodDays.value.map((d) => format(d, 'yyyy-MM-dd')))

  const periodLabel = computed(() => {
    const start = periodDays.value[0]
    const end = periodDays.value[periodLength.value - 1]
    if (start.getFullYear() === end.getFullYear()) {
      if (start.getMonth() === end.getMonth()) {
        return `${format(start, 'MMM d')} – ${format(end, 'd, yyyy')}`
      }
      return `${format(start, 'MMM d')} – ${format(end, 'MMM d, yyyy')}`
    }
    return `${format(start, 'MMM d, yyyy')} – ${format(end, 'MMM d, yyyy')}`
  })

  const isCurrentPeriod = computed(() => {
    const todayStr = format(new Date(), 'yyyy-MM-dd')
    return periodDayStrings.value.includes(todayStr)
  })

  // ── Data loading ──────────────────────────────────────────────────────────

  async function loadPeriod() {
    loading.value = true
    const dates = periodDayStrings.value
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
      console.error('[timesheeps] loadPeriod failed:', err)
    } finally {
      loading.value = false
    }
  }

  watch(periodDayStrings, loadPeriod, { immediate: true })

  // ── Navigation ────────────────────────────────────────────────────────────

  function prevPeriod() {
    periodStart.value = subDays(periodStart.value, periodLength.value)
  }

  function nextPeriod() {
    periodStart.value = addDays(periodStart.value, periodLength.value)
  }

  function thisPeriod() {
    periodStart.value = currentPayPeriodStart(anchor.value, freq.value)
  }

  async function navigateTo(date: string) {
    await dayStore.loadDay(date)
    router.push('/')
  }

  // ── Grid helpers ─────────────────────────────────────────────────────────

  const gridStyle = computed(() => ({
    gridTemplateColumns: `160px repeat(${periodLength.value}, minmax(50px, 1fr))`,
  }))

  const periodProjects = computed(() => {
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
    if (!data) return 0
    return data.entries
      .filter((e) => e.projectId === projectId)
      .reduce((sum, e) => sum + (e.endMinutes - e.startMinutes), 0)
  }

  function dayTotalMinutes(date: string): number {
    const data = dayData.value.get(date)
    if (!data) return 0
    return data.entries.reduce((sum, e) => sum + (e.endMinutes - e.startMinutes), 0)
  }

  function hasUnlogged(date: string): boolean {
    const data = dayData.value.get(date)
    if (!data) return false
    return data.hasActivity && dayTotalMinutes(date) === 0
  }

  function fmtMin(min: number): string {
    if (min === 0) return ''
    return formatDuration(min)
  }

  const allEntries = computed(() => {
    const entries: TimeEntry[] = []
    for (const data of dayData.value.values()) {
      entries.push(...data.entries)
    }
    return entries
  })
</script>

<template>
  <div class="pay-period-view">
    <div class="period-nav">
      <button class="btn-ghost nav-btn" @click="prevPeriod">‹</button>

      <span class="period-label" :class="{ 'current-period': isCurrentPeriod }">
        {{ periodLabel }}
      </span>

      <button class="btn-ghost nav-btn" @click="nextPeriod">›</button>

      <button v-if="!isCurrentPeriod" class="btn-secondary this-period-btn" @click="thisPeriod">
        This period
      </button>

      <span class="period-type-badge">{{ freq === 'fortnightly' ? 'Fortnightly' : 'Weekly' }}</span>
    </div>

    <div v-if="loading" class="loading-msg">Loading…</div>

    <div v-else class="period-scroll">
      <div class="period-grid">
        <!-- Header row -->
        <div class="grid-row header-row" :style="gridStyle">
          <div class="cell label-cell"></div>

          <div
            v-for="(date, i) in periodDayStrings"
            :key="date"
            class="cell day-header"
            :class="{ today: isToday(periodDays[i]) }"
            @click="navigateTo(date)"
          >
            <span class="day-name">{{ format(periodDays[i], 'EEE') }}</span>
            <span class="day-date">{{ format(periodDays[i], 'M/d') }}</span>
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
        <div v-if="periodProjects.length === 0" class="grid-row empty-row" :style="gridStyle">
          <div class="cell empty-cell">No time logged this period</div>
        </div>

        <!-- Project rows -->
        <div
          v-for="project in periodProjects"
          :key="project.id"
          class="grid-row project-row"
          :style="gridStyle"
        >
          <div class="cell project-label">
            <span class="dot" :style="{ background: project.color }" />
            <span class="project-name">{{ project.name }}</span>
          </div>

          <div
            v-for="date in periodDayStrings"
            :key="date"
            class="cell entry-cell"
            :class="{ 'has-value': projectDayMinutes(project.id, date) > 0 }"
            @click="navigateTo(date)"
          >
            {{ fmtMin(projectDayMinutes(project.id, date)) }}
          </div>
        </div>

        <!-- Total row -->
        <div class="grid-row total-row" :style="gridStyle">
          <div class="cell total-label">Total</div>

          <div
            v-for="date in periodDayStrings"
            :key="date"
            class="cell total-cell"
            :class="{ 'has-value': dayTotalMinutes(date) > 0 }"
            @click="navigateTo(date)"
          >
            {{ fmtMin(dayTotalMinutes(date)) }}
          </div>
        </div>
      </div>

      <PeriodSummary :entries="allEntries" />
    </div>
  </div>
</template>

<style scoped>
  .pay-period-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    padding: 16px 20px;
    gap: 16px;
    overflow: hidden;
  }

  /* ── Navigation ────────────────────────────────────────────────────────────── */

  .period-nav {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .nav-btn {
    font-size: 18px;
    line-height: 1;
    padding: 4px 10px;
  }

  .period-label {
    font-size: 14px;
    font-weight: 600;
    min-width: 220px;
    text-align: center;
  }

  .period-label.current-period {
    color: var(--primary);
  }

  .this-period-btn {
    margin-left: 4px;
    font-size: 12px;
    padding: 4px 10px;
  }

  .period-type-badge {
    margin-left: 4px;
    font-size: 11px;
    color: var(--text-faint);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 2px 8px;
  }

  .loading-msg {
    color: var(--text-muted);
    font-size: 13px;
  }

  /* ── Scroll container ──────────────────────────────────────────────────────── */

  .period-scroll {
    flex: 1;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  /* ── Grid ──────────────────────────────────────────────────────────────────── */

  .period-grid {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    flex-shrink: 0;
    min-width: max-content;
  }

  .grid-row {
    display: grid;
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
