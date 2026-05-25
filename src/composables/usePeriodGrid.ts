import type { Ref } from 'vue'
import type { TimeEntry } from '../schemas'

export interface PeriodDayData {
  date: string
  entries: TimeEntry[]
  hasActivity: boolean
}

function formatDuration(totalMin: number): string {
  const h = Math.floor(totalMin / 60)
  const m = totalMin % 60
  if (h === 0) return `${m}m`
  if (m === 0) return `${h}h`
  return `${h}h ${m}m`
}

export function usePeriodGrid(dayData: Ref<Map<string, PeriodDayData>>) {
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

  return { projectDayMinutes, dayTotalMinutes, hasUnlogged, fmtMin }
}
