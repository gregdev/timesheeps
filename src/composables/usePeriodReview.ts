import { watch, ref, type Ref } from 'vue'
import { api } from '../api'
import type { PeriodReview } from '../schemas'
import type { PeriodDayData } from './usePeriodGrid'

/**
 * Load a date range as one `get_period_review` call.
 *
 * The week and pay-period views used to issue two IPC calls per day and then
 * infer "needs attention" from a bare `hasActivity` flag — which could not see
 * that six tracked hours had only one logged. One call returns the entries,
 * tracked totals, unclaimed suggestions, gaps and notes together, so the grid
 * and the review panel cannot disagree about a day.
 */
export function usePeriodReview(dayStrings: Ref<string[]>) {
  const loading = ref(false)
  const review = ref<PeriodReview | null>(null)
  const dayData = ref<Map<string, PeriodDayData>>(new Map())
  const loadError = ref<string | null>(null)

  async function load(silent = false) {
    const dates = dayStrings.value

    if (dates.length === 0) {
      return
    }

    if (!silent) {
      loading.value = true
    }

    try {
      const result = await api.getPeriodReview(dates[0], dates[dates.length - 1])
      review.value = result
      dayData.value = new Map(
        result.days.map((day) => [
          day.date,
          {
            date: day.date,
            entries: day.entries,
            hasActivity: day.trackedSecs > 0,
            trackedMinutes: Math.round(day.trackedSecs / 60),
            expectedMinutes: day.expectedMinutes,
            note: day.note,
            attention: day.attention,
          },
        ]),
      )
      loadError.value = null
    } catch (err) {
      loadError.value = err instanceof Error ? err.message : String(err)
      console.error('[timesheeps] getPeriodReview failed:', err)
    } finally {
      if (!silent) {
        loading.value = false
      }
    }
  }

  // The listener receives the new array as an argument; passing it straight to
  // `load` would land in the `silent` parameter (arrays are truthy), so the
  // spinner would never appear on a period change.
  watch(dayStrings, () => load(), { immediate: true })

  /** Refresh after an edit without replacing the view with a loading message. */
  const reload = () => load(true)

  return { loading, review, dayData, loadError, reload }
}
