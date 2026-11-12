<script setup lang="ts">
  import { ref, computed, watch } from 'vue'
  import { useRouter } from 'vue-router'
  import { format, addDays, addWeeks, subWeeks, startOfWeek } from 'date-fns'
  import { useSettingsStore } from '../stores/settings'
  import { useProjectsStore } from '../stores/projects'
  import { useDayStore } from '../stores/day'
  import { usePeriodGrid } from '../composables/usePeriodGrid'
  import { usePeriodReview } from '../composables/usePeriodReview'
  import { localDateKey } from '../composables/useFormat'
  import PeriodNav from '../components/PeriodNav.vue'
  import PeriodGrid from '../components/PeriodGrid.vue'
  import PeriodSummary from '../components/PeriodSummary.vue'
  import PeriodReviewPanel from '../components/PeriodReviewPanel.vue'

  const settingsStore = useSettingsStore()
  const projectsStore = useProjectsStore()
  const dayStore = useDayStore()
  const router = useRouter()

  const weekStartsOn = computed(() => settingsStore.settings.weekStartsOn as 0 | 1)
  const weekStart = ref(startOfWeek(new Date(), { weekStartsOn: weekStartsOn.value }))

  const weekDays = computed(() => Array.from({ length: 7 }, (_, i) => addDays(weekStart.value, i)))
  const weekDayStrings = computed(() => weekDays.value.map((d) => localDateKey(d)))

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
    const todayStr = localDateKey(new Date())
    return weekDayStrings.value.includes(todayStr)
  })

  // One call for the whole week: entries plus review state per day.
  const { loading, review, dayData, reload } = usePeriodReview(weekDayStrings)

  // Changing "week starts on" should move the visible week, but only while the
  // user is looking at the current one — otherwise it would yank them away from
  // a week they deliberately navigated to.
  watch(weekStartsOn, (dow) => {
    if (isCurrentWeek.value) {
      weekStart.value = startOfWeek(new Date(), { weekStartsOn: dow })
    }
  })

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

  const {
    projectDayMinutes,
    dayTotalMinutes,
    hasUnlogged,
    fmtMin,
    entriesInRange,
    usedProjectIds,
  } = usePeriodGrid(dayData)

  const weekProjects = computed(() => {
    const used = usedProjectIds()
    return projectsStore.active.filter((p) => used.has(p.id))
  })

  const allEntries = computed(() => entriesInRange())

  function projectName(id: number): string {
    return projectsStore.projects.find((p) => p.id === id)?.name ?? 'Unknown'
  }

  function projectColor(id: number): string {
    return projectsStore.projects.find((p) => p.id === id)?.color ?? 'var(--border)'
  }

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
        <PeriodReviewPanel
          :review="review"
          :project-name="projectName"
          :project-color="projectColor"
          @navigate="navigateTo"
          @changed="reload"
        />
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
