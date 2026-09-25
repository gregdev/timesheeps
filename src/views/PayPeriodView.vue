<script setup lang="ts">
  import { ref, computed, watch } from 'vue'
  import { useRouter } from 'vue-router'
  import { format, addDays, subDays, differenceInDays, startOfWeek, parseISO } from 'date-fns'
  import { api } from '../api'
  import { useSettingsStore } from '../stores/settings'
  import { useProjectsStore } from '../stores/projects'
  import { useDayStore } from '../stores/day'
  import { usePeriodGrid } from '../composables/usePeriodGrid'
  import { localDateKey } from '../composables/useFormat'
  import type { PeriodDayData } from '../composables/usePeriodGrid'
  import PeriodNav from '../components/PeriodNav.vue'
  import PeriodGrid from '../components/PeriodGrid.vue'
  import PeriodSummary from '../components/PeriodSummary.vue'

  const settingsStore = useSettingsStore()
  const projectsStore = useProjectsStore()
  const dayStore = useDayStore()
  const router = useRouter()

  const loading = ref(false)
  const dayData = ref<Map<string, PeriodDayData>>(new Map())

  // ── Pay period computation ────────────────────────────────────────────────

  function currentPayPeriodStart(anchorStr: string, freq: 'weekly' | 'fortnightly'): Date {
    const anchor = parseISO(anchorStr)
    const today = new Date()

    if (freq === 'weekly') {
      const dow = anchor.getDay() as 0 | 1 | 2 | 3 | 4 | 5 | 6
      return startOfWeek(today, { weekStartsOn: dow })
    }

    // Fortnightly periods are anchored, not weekday-based, so count how many
    // whole periods have elapsed. `Math.floor` is correct on both sides of the
    // anchor: for a date before it, `ceil` would round towards the anchor and
    // return the *following* period, which does not contain today.
    const diff = differenceInDays(today, anchor)
    const periodsElapsed = Math.floor(diff / 14)
    return addDays(anchor, periodsElapsed * 14)
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
  const periodDayStrings = computed(() => periodDays.value.map((d) => localDateKey(d)))

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
    const todayStr = localDateKey(new Date())
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
      const map = new Map<string, PeriodDayData>()

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

  const { projectDayMinutes, dayTotalMinutes, hasUnlogged, fmtMin, entriesInRange, usedProjectIds } =
    usePeriodGrid(dayData)

  const gridColumns = computed(() => `160px repeat(${periodLength.value}, minmax(50px, 1fr))`)

  const periodProjects = computed(() => {
    const used = usedProjectIds()
    return projectsStore.active.filter((p) => used.has(p.id))
  })

  const allEntries = computed(() => entriesInRange())
</script>

<template>
  <div class="pay-period-view">
    <PeriodNav
      :label="periodLabel"
      :is-current="isCurrentPeriod"
      current-label="This period"
      label-min-width="220px"
      @prev="prevPeriod"
      @next="nextPeriod"
      @current="thisPeriod"
    >
      <span class="period-type-badge">
        {{ freq === 'fortnightly' ? 'Fortnightly' : 'Weekly' }}
      </span>
    </PeriodNav>

    <div class="view-body">
      <div v-if="loading" class="loading-msg">Loading…</div>

      <div v-else class="period-scroll">
        <PeriodGrid
          :days="periodDays"
          :day-strings="periodDayStrings"
          :projects="periodProjects"
          :project-day-minutes="projectDayMinutes"
          :day-total-minutes="dayTotalMinutes"
          :has-unlogged="hasUnlogged"
          :fmt-min="fmtMin"
          empty-message="No time logged this period"
          :grid-columns="gridColumns"
          @navigate-to="navigateTo"
        />
        <PeriodSummary :entries="allEntries" />
      </div>
    </div>
  </div>
</template>

<style scoped>
  .pay-period-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .view-body {
    padding: var(--space-4) var(--space-5);
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    flex: 1;
    overflow: hidden;
  }

  .loading-msg {
    color: var(--text-muted);
    font-size: var(--text-sm);
  }

  .period-scroll {
    flex: 1;
    overflow: auto;
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .period-type-badge {
    margin-left: var(--space-1);
    font-size: var(--text-xs);
    color: var(--text-faint);
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 10px;
    padding: 2px var(--space-2);
  }
</style>
