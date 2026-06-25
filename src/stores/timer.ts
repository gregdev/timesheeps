import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { format } from 'date-fns'
import { listen } from '@tauri-apps/api/event'
import { api } from '../api'
import { useDayStore } from './day'
import type { TimerState } from '../schemas'

export const useTimerStore = defineStore('timer', () => {
  // ── state ──────────────────────────────────────────────────────────────
  const state = ref<TimerState>({
    status: 'stopped',
    projectId: null,
    projectName: null,
    projectColor: null,
    note: '',
    startedAt: null,
    accumulatedMs: 0,
    pausedAt: null,
    elapsedMs: 0,
  })

  // ── computed ────────────────────────────────────────────────────────────
  const isRunning = computed(() => state.value.status === 'running')
  const isPaused = computed(() => state.value.status === 'paused')
  const isActive = computed(() => state.value.status !== 'stopped')

  const elapsedFormatted = computed(() => {
    const ms = state.value.elapsedMs
    const totalSeconds = Math.floor(ms / 1000)
    const hours = Math.floor(totalSeconds / 3600)
    const minutes = Math.floor((totalSeconds % 3600) / 60)
    const seconds = totalSeconds % 60

    if (hours > 0) {
      return `${hours}h ${String(minutes).padStart(2, '0')}m ${String(seconds).padStart(2, '0')}s`
    }
    return `${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
  })

  // ── actions ─────────────────────────────────────────────────────────────
  async function fetchState() {
    try {
      state.value = await api.getTimerState()
    } catch (e) {
      console.error('[timer] fetchState failed:', e)
    }
  }

  async function start(projectId: number, note: string) {
    try {
      state.value = await api.startTimer(projectId, note)
    } catch (e) {
      console.error('[timer] start failed:', e)
    }
  }

  async function pause() {
    try {
      state.value = await api.pauseTimer()
    } catch (e) {
      console.error('[timer] pause failed:', e)
    }
  }

  async function resume() {
    try {
      state.value = await api.resumeTimer()
    } catch (e) {
      console.error('[timer] resume failed:', e)
    }
  }

  async function stop() {
    try {
      const finalState = await api.stopTimer()

      if (
        finalState.projectId !== null &&
        finalState.elapsedMs >= 60000 &&
        finalState.startedAt
      ) {
        const startedAt = new Date(finalState.startedAt)
        const startDate = format(startedAt, 'yyyy-MM-dd')
        const startMinutes = startedAt.getHours() * 60 + startedAt.getMinutes()
        const elapsedMinutes = Math.round(finalState.elapsedMs / 60000)
        const endTotalMinutes = startMinutes + elapsedMinutes

        // Calculate end date/time: midnight = 1440 min per day
        const endDate = new Date(startedAt.getTime() + finalState.elapsedMs)
        const endDateStr = format(endDate, 'yyyy-MM-dd')

        const dayStore = useDayStore()
        const isViewingStart = dayStore.selectedDate === startDate
        const isViewingEnd = startDate !== endDateStr && dayStore.selectedDate === endDateStr

        if (startDate === endDateStr) {
          // Single entry — all on one day
          const entry = await api.createTimeEntry(
            startDate, finalState.projectId,
            startMinutes, endTotalMinutes, finalState.note,
          )
          if (isViewingStart) {
            dayStore.timeEntries = [
              ...dayStore.timeEntries, entry,
            ].sort((a, b) => a.startMinutes - b.startMinutes)
          }
        } else {
          // Split at midnight: entry 1 from start→midnight, entry 2 from midnight→end
          const minsToMidnight = 1440 - startMinutes
          const minsAfterMidnight = endTotalMinutes - 1440

          if (minsToMidnight >= 1) {
            const entry1 = await api.createTimeEntry(
              startDate, finalState.projectId,
              startMinutes, 1440, finalState.note,
            )
            if (isViewingStart) {
              dayStore.timeEntries = [
                ...dayStore.timeEntries, entry1,
              ].sort((a, b) => a.startMinutes - b.startMinutes)
            }
          }

          if (minsAfterMidnight >= 1) {
            const entry2 = await api.createTimeEntry(
              endDateStr, finalState.projectId,
              0, minsAfterMidnight, finalState.note,
            )
            if (isViewingEnd) {
              dayStore.timeEntries = [
                ...dayStore.timeEntries, entry2,
              ].sort((a, b) => a.startMinutes - b.startMinutes)
            }
          }
        }
      }

      state.value = finalState
    } catch (e) {
      console.error('[timer] stop failed:', e)
    }
  }

  // ── initialise ──────────────────────────────────────────────────────────
  // Called once on app mount: fetch current state + subscribe to ticks
  let unlisten: (() => void) | null = null

  async function init() {
    await fetchState()

    unlisten = await listen<TimerState>('timer:tick', (event) => {
      state.value = event.payload
    })
  }

  function destroy() {
    unlisten?.()
  }

  return {
    state,
    isRunning,
    isPaused,
    isActive,
    elapsedFormatted,
    fetchState,
    start,
    pause,
    resume,
    stop,
    init,
    destroy,
  }
})
