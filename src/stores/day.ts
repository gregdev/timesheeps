import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { format, addDays, subDays, isToday, parseISO } from 'date-fns'
import { api } from '../api'
import type { ActivityBlock, TimeEntry } from '../schemas'

export const useDayStore = defineStore('day', () => {
  const selectedDate = ref(format(new Date(), 'yyyy-MM-dd'))
  const activityBlocks = ref<ActivityBlock[]>([])
  const timeEntries = ref<TimeEntry[]>([])
  const loading = ref(false)

  const isViewingToday = computed(() => isToday(parseISO(selectedDate.value)))

  async function loadDay(date?: string) {
    if (date) selectedDate.value = date
    loading.value = true
    try {
      const [blocks, entries] = await Promise.all([
        api.getActivityForDay(selectedDate.value),
        api.getTimeEntriesForDay(selectedDate.value),
      ])
      activityBlocks.value = blocks
      timeEntries.value = entries
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

  async function createEntry(projectId: number, startMinutes: number, endMinutes: number, note: string) {
    const entry = await api.createTimeEntry(selectedDate.value, projectId, startMinutes, endMinutes, note)
    timeEntries.value = [...timeEntries.value, entry].sort((a, b) => a.startMinutes - b.startMinutes)
    return entry
  }

  async function updateEntry(id: number, projectId: number, startMinutes: number, endMinutes: number, note: string) {
    await api.updateTimeEntry(id, projectId, startMinutes, endMinutes, note)
    const idx = timeEntries.value.findIndex(e => e.id === id)
    if (idx >= 0) {
      timeEntries.value[idx] = { ...timeEntries.value[idx], projectId, startMinutes, endMinutes, note }
      timeEntries.value = [...timeEntries.value].sort((a, b) => a.startMinutes - b.startMinutes)
    }
  }

  async function deleteEntry(id: number) {
    await api.deleteTimeEntry(id)
    timeEntries.value = timeEntries.value.filter(e => e.id !== id)
  }

  const summary = computed(() => {
    const map = new Map<number, number>()
    for (const e of timeEntries.value) {
      map.set(e.projectId, (map.get(e.projectId) ?? 0) + (e.endMinutes - e.startMinutes))
    }
    return map
  })

  return {
    selectedDate, activityBlocks, timeEntries, loading, isViewingToday,
    loadDay, nextDay, prevDay, goToday,
    createEntry, updateEntry, deleteEntry,
    summary,
  }
})
