import type { Ref } from 'vue'
import type { TimeEntry } from '../schemas'
import { formatMinutes } from './useFormat'

export interface PeriodDayData {
  date: string
  entries: TimeEntry[]
  hasActivity: boolean
}

export function usePeriodGrid(dayData: Ref<Map<string, PeriodDayData>>) {
  function projectDayMinutes(projectId: number, date: string): number {
    const data = dayData.value.get(date)

    if (!data) {
      return 0
    }

    return data.entries
      .filter((e) => e.projectId === projectId)
      .reduce((sum, e) => sum + (e.endMinutes - e.startMinutes), 0)
  }

  function dayTotalMinutes(date: string): number {
    const data = dayData.value.get(date)

    if (!data) {
      return 0
    }

    return data.entries.reduce((sum, e) => sum + (e.endMinutes - e.startMinutes), 0)
  }

  function hasUnlogged(date: string): boolean {
    const data = dayData.value.get(date)

    if (!data) {
      return false
    }

    return data.hasActivity && dayTotalMinutes(date) === 0
  }

  function fmtMin(min: number): string {
    if (min === 0) {
      return ''
    }

    return formatMinutes(min)
  }

  /** Every entry across the loaded range, in day order. */
  function entriesInRange(): TimeEntry[] {
    const entries: TimeEntry[] = []

    for (const data of dayData.value.values()) {
      entries.push(...data.entries)
    }

    return entries
  }

  /** Ids of the projects that have at least one entry in the loaded range. */
  function usedProjectIds(): Set<number> {
    const ids = new Set<number>()

    for (const data of dayData.value.values()) {
      for (const entry of data.entries) {
        ids.add(entry.projectId)
      }
    }

    return ids
  }

  return {
    projectDayMinutes,
    dayTotalMinutes,
    hasUnlogged,
    fmtMin,
    entriesInRange,
    usedProjectIds,
  }
}
