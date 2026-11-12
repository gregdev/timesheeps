import type { Ref } from 'vue'
import type { TimeEntry } from '../schemas'
import { formatMinutes } from './useFormat'

/**
 * How far tracked time may exceed logged time before a day counts as
 * unfinished. The old check only flagged a day with *nothing* logged, so six
 * tracked hours with one logged looked perfectly fine.
 */
export const UNLOGGED_TOLERANCE_MIN = 30

export interface PeriodDayData {
  date: string
  entries: TimeEntry[]
  hasActivity: boolean
  /** Minutes of merged activity attributed to any project. */
  trackedMinutes: number
  /** Weekday target set in Settings, or 0 when none is configured. */
  expectedMinutes: number
  /** The user's own note for the day. */
  note: string
  /** Plain-English reasons the day is unfinished (backend-computed). */
  attention: string[]
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

  function trackedDayMinutes(date: string): number {
    return dayData.value.get(date)?.trackedMinutes ?? 0
  }

  /**
   * True when there is recorded work that has not been logged. Compares
   * minutes rather than testing for zero entries, so a day with six hours
   * tracked and one hour logged is still visible as unfinished.
   */
  function hasUnlogged(date: string): boolean {
    const data = dayData.value.get(date)

    if (!data || !data.hasActivity) {
      return false
    }

    return data.trackedMinutes - dayTotalMinutes(date) >= UNLOGGED_TOLERANCE_MIN
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
    trackedDayMinutes,
    hasUnlogged,
    fmtMin,
    entriesInRange,
    usedProjectIds,
  }
}
