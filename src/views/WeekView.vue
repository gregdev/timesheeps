<script setup lang="ts">
  import { ref, computed, watch } from 'vue'
  import { useRouter } from 'vue-router'
  import { format, addDays, addWeeks, subWeeks, startOfWeek } from 'date-fns'
  import { api } from '../api'
  import { useSettingsStore } from '../stores/settings'
  import { useProjectsStore } from '../stores/projects'
  import { useDayStore } from '../stores/day'
  import { usePeriodGrid } from '../composables/usePeriodGrid'
  import type { PeriodDayData } from '../composables/usePeriodGrid'
  import PeriodNav from '../components/PeriodNav.vue'
  import PeriodGrid from '../components/PeriodGrid.vue'
  import PeriodSummary from '../components/PeriodSummary.vue'
  import type { TimeEntry } from '../schemas'

  const settingsStore = useSettingsStore()
  const projectsStore = useProjectsStore()
  const dayStore = useDayStore()
  const router = useRouter()

  const loading = ref(false)
  const dayData = ref<Map<string, PeriodDayData>>(new Map())

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
      const map = new Map<string, PeriodDayData>()

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

  const { projectDayMinutes, dayTotalMinutes, hasUnlogged, fmtMin } = usePeriodGrid(dayData)

  const weekProjects = computed(() => {
    const usedIds = new Set<number>()
    for (const data of dayData.value.values()) {
      for (const entry of data.entries) usedIds.add(entry.projectId)
    }
    return projectsStore.projects.filter((p) => !p.archivedAt && usedIds.has(p.id))
  })

  const allEntries = computed(() => {
    const entries: TimeEntry[] = []
    for (const data of dayData.value.values()) entries.push(...data.entries)
    return entries
  })

  const gridColumns = '180px repeat(7, 1fr)'
</script>

<template>
  <div class="week-view">
    <PeriodNav
      :label="weekLabel"
      :is-current="isCurrentWeek"
      current-label="This week"
      label-min-width="190px"
      @prev="prevWeek"
      @next="nextWeek"
      @current="thisWeek"
    />

    <div class="view-body">
      <div v-if="loading" class="loading-msg">Loading…</div>

      <template v-else>
        <PeriodGrid
          :days="weekDays"
          :day-strings="weekDayStrings"
          :projects="weekProjects"
          :project-day-minutes="projectDayMinutes"
          :day-total-minutes="dayTotalMinutes"
          :has-unlogged="hasUnlogged"
          :fmt-min="fmtMin"
          empty-message="No time logged this week"
          :grid-columns="gridColumns"
          @navigate-to="navigateTo"
        />
        <PeriodSummary :entries="allEntries" />
      </template>
    </div>
  </div>
</template>

<style scoped>
  .week-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: auto;
  }

  .view-body {
    padding: var(--space-4) var(--space-5);
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  /* ── Navigation ────────────────────────────────────────────────────────────── */

  .loading-msg {
    color: var(--text-muted);
    font-size: var(--text-sm);
  }
</style>
