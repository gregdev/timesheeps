import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { format, addDays, subDays, parseISO } from 'date-fns'
import { api } from '../api'
import type { ActivityBlock, SuggestedEntry, TimeEntry, WindowSummaryItem } from '../schemas'

export const useDayStore = defineStore('day', () => {
  const selectedDate = ref(format(new Date(), 'yyyy-MM-dd'))
  const currentDate = ref(format(new Date(), 'yyyy-MM-dd'))
  const activityBlocks = ref<ActivityBlock[]>([])
  const timeEntries = ref<TimeEntry[]>([])
  const windowSummary = ref<WindowSummaryItem[]>([])
  const rawSuggestions = ref<SuggestedEntry[]>([])
  const loading = ref(false)
  const loadError = ref<string | null>(null)

  const isViewingToday = computed(() => selectedDate.value === currentDate.value)

  async function loadDay(date?: string, silent = false) {
    if (date) selectedDate.value = date
    if (!silent) loading.value = true
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
      loadError.value = null
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      loadError.value = msg
      console.error('[timesheeps] loadDay failed:', err)
    } finally {
      loading.value = false
    }
  }

  function nextDay() {
    loadDay(format(addDays(parseISO(selectedDate.value), 1), 'yyyy-MM-dd'))
  }
  function prevDay() {
    loadDay(format(subDays(parseISO(selectedDate.value), 1), 'yyyy-MM-dd'))
  }
  function goToday() {
    loadDay(format(new Date(), 'yyyy-MM-dd'))
  }

  function refreshCurrentDate() {
    const today = format(new Date(), 'yyyy-MM-dd')
    if (currentDate.value !== today) {
      const wasViewingToday = selectedDate.value === currentDate.value
      currentDate.value = today
      if (wasViewingToday) loadDay(today)
    }
  }

  async function createEntry(
    projectId: number,
    startMinutes: number,
    endMinutes: number,
    note: string,
  ) {
    const entry = await api.createTimeEntry(
      selectedDate.value,
      projectId,
      startMinutes,
      endMinutes,
      note,
    )
    timeEntries.value = [...timeEntries.value, entry].sort(
      (a, b) => a.startMinutes - b.startMinutes,
    )
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

  function isoToMinutes(iso: string): number {
    const d = new Date(iso)
    return d.getHours() * 60 + d.getMinutes() + d.getSeconds() / 60
  }

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
    updateEntry,
    deleteEntry,
    summary,
  }
})
