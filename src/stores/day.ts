import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { addDays, subDays, parseISO } from 'date-fns'
import { api } from '../api'
import { isoToLocalMinutes, localDateKey } from '../composables/useFormat'
import type { EntryRange } from '../composables/useEntryModal'
import { useSettingsStore } from './settings'
import type {
  ActivityBlock,
  CalendarEvent,
  SuggestedEntry,
  TimeEntry,
  WindowSummaryItem,
} from '../schemas'

export const useDayStore = defineStore('day', () => {
  const selectedDate = ref(localDateKey(new Date()))
  const currentDate = ref(localDateKey(new Date()))
  const activityBlocks = ref<ActivityBlock[]>([])
  const timeEntries = ref<TimeEntry[]>([])
  const windowSummary = ref<WindowSummaryItem[]>([])
  const rawSuggestions = ref<SuggestedEntry[]>([])
  const calendarEvents = ref<CalendarEvent[]>([])
  const loading = ref(false)
  const loadError = ref<string | null>(null)

  const isViewingToday = computed(() => selectedDate.value === currentDate.value)

  async function loadDay(date?: string, silent = false) {
    if (date) {
      selectedDate.value = date
    }
    if (!silent) {
      loading.value = true
    }
    try {
      const [blocks, entries, winSummary, suggestions] = await Promise.all([
        api.getActivityForDay(selectedDate.value),
        api.getTimeEntriesForDay(selectedDate.value),
        api.getWindowSummaryForDay(selectedDate.value),
        api.getSuggestedEntriesForDay(selectedDate.value),
      ])
      activityBlocks.value = blocks
      timeEntries.value = entries
      windowSummary.value = winSummary
      rawSuggestions.value = suggestions

      // Load calendar events (fire-and-forget — don't block on failure)
      api
        .getCalendarEvents(selectedDate.value)
        .then((events) => {
          calendarEvents.value = events
        })
        .catch(() => {
          calendarEvents.value = []
        })
      loadError.value = null

      // Auto-accept suggestions if enabled
      const settingsStore = useSettingsStore()

      if (settingsStore.settings.autoAcceptSuggested && suggestions.length > 0) {
        const nowMinutes = new Date().getHours() * 60 + new Date().getMinutes()
        // Ranges accepted in this pass count as "existing" so two overlapping
        // suggestions cannot both become entries.
        const claimed: { start: number; end: number }[] = []
        let created = false

        for (const s of suggestions) {
          const startMin = isoToMinutes(s.startedAt)
          const endMin = isoToMinutes(s.endedAt)

          if (endMin <= startMin) {
            continue
          }

          // Only auto-create entries that don't overlap existing ones
          const overlapsEntry = entries.some(
            (e) => e.startMinutes < endMin && e.endMinutes > startMin,
          )
          const overlapsClaimed = claimed.some((c) => c.start < endMin && c.end > startMin)

          if (overlapsEntry || overlapsClaimed) {
            continue
          }
          // Don't auto-create entries that end in the future (still in progress)
          if (endMin > nowMinutes + 5) {
            continue
          }

          try {
            await api.createTimeEntry(
              selectedDate.value,
              s.projectId,
              Math.round(startMin),
              Math.round(endMin),
              '',
            )
            claimed.push({ start: startMin, end: endMin })
            created = true
          } catch (e) {
            console.error('[timesheeps] auto-accept failed:', e)
          }
        }

        // Reload to pick up newly created entries; `suggestedEntries` filters
        // out anything that now overlaps, so no re-fetch is needed.
        if (created) {
          timeEntries.value = await api.getTimeEntriesForDay(selectedDate.value)
        }
      }
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      loadError.value = msg
      console.error('[timesheeps] loadDay failed:', err)
    } finally {
      // Only the caller that raised the flag may lower it — silent background
      // refreshes must not clear a spinner raised by a real navigation.
      if (!silent) {
        loading.value = false
      }
    }
  }

  function nextDay() {
    loadDay(localDateKey(addDays(parseISO(selectedDate.value), 1)))
  }

  function prevDay() {
    loadDay(localDateKey(subDays(parseISO(selectedDate.value), 1)))
  }

  function goToday() {
    loadDay(localDateKey(new Date()))
  }

  function refreshCurrentDate() {
    const today = localDateKey(new Date())

    if (currentDate.value !== today) {
      const wasViewingToday = selectedDate.value === currentDate.value
      currentDate.value = today

      if (wasViewingToday) {
        loadDay(today)
      }
    }
  }

  /**
   * Create one entry per range. A Window Activity row can aggregate several
   * separate sessions, so each becomes its own entry — a single entry spanning
   * the gaps between them would count untracked time as work.
   */
  async function createEntries(projectId: number, ranges: EntryRange[], note: string) {
    const created = await Promise.all(
      ranges.map((r) =>
        api.createTimeEntry(selectedDate.value, projectId, r.startMinutes, r.endMinutes, note),
      ),
    )
    timeEntries.value = [...timeEntries.value, ...created].sort(
      (a, b) => a.startMinutes - b.startMinutes,
    )
    return created
  }

  async function createEntry(
    projectId: number,
    startMinutes: number,
    endMinutes: number,
    note: string,
  ) {
    const [entry] = await createEntries(projectId, [{ startMinutes, endMinutes }], note)
    return entry
  }

  async function updateEntry(
    id: number,
    projectId: number,
    startMinutes: number,
    endMinutes: number,
    note: string,
  ) {
    await api.updateTimeEntry(id, projectId, startMinutes, endMinutes, note)
    const idx = timeEntries.value.findIndex((e) => e.id === id)

    if (idx >= 0) {
      timeEntries.value[idx] = {
        ...timeEntries.value[idx],
        projectId,
        startMinutes,
        endMinutes,
        note,
      }
      timeEntries.value = [...timeEntries.value].sort((a, b) => a.startMinutes - b.startMinutes)
    }
  }

  async function deleteEntry(id: number) {
    await api.deleteTimeEntry(id)
    timeEntries.value = timeEntries.value.filter((e) => e.id !== id)
  }

  const summary = computed(() => {
    const map = new Map<number, number>()

    for (const e of timeEntries.value) {
      map.set(e.projectId, (map.get(e.projectId) ?? 0) + (e.endMinutes - e.startMinutes))
    }

    return map
  })

  /** UTC ISO timestamp → local minutes since midnight. */
  const isoToMinutes = isoToLocalMinutes

  const suggestedEntries = computed(() => {
    return rawSuggestions.value
      .map((s) => ({
        projectId: s.projectId,
        startMinutes: isoToMinutes(s.startedAt),
        endMinutes: isoToMinutes(s.endedAt),
      }))
      .filter((s) => s.endMinutes > s.startMinutes)
      .filter(
        (s) =>
          !timeEntries.value.some(
            (e) => e.startMinutes < s.endMinutes && e.endMinutes > s.startMinutes,
          ),
      )
  })

  return {
    selectedDate,
    activityBlocks,
    timeEntries,
    windowSummary,
    calendarEvents,
    suggestedEntries,
    loading,
    loadError,
    isViewingToday,
    loadDay,
    nextDay,
    prevDay,
    goToday,
    refreshCurrentDate,
    createEntry,
    createEntries,
    updateEntry,
    deleteEntry,
    summary,
  }
})
