/**
 * Shared formatting helpers.
 *
 * Durations show up in three different units across the app — minutes (timeline
 * entries), seconds (activity totals from the Rust side) and milliseconds (the
 * live timer). Routing them all through one place keeps "1h 5m" spelled the
 * same everywhere instead of being re-implemented per component.
 */

import { format, parseISO } from 'date-fns'

/**
 * `yyyy-MM-dd` key for a local calendar day.
 *
 * Never use `Date#toISOString().slice(0, 10)` for day keys: that yields the
 * *UTC* date, which is the previous day for anyone west of Greenwich late in
 * the evening. Every date the backend stores and queries is a local day.
 */
export function localDateKey(value: Date | string): string {
  return format(typeof value === 'string' ? parseISO(value) : value, 'yyyy-MM-dd')
}

/** `90` → `"1h 30m"`, `120` → `"2h"`, `45` → `"45m"`. */
export function formatMinutes(totalMin: number): string {
  const h = Math.floor(totalMin / 60)
  const m = Math.floor(totalMin % 60)

  if (h === 0) {
    return `${m}m`
  }
  if (m === 0) {
    return `${h}h`
  }

  return `${h}h ${m}m`
}

/** Same as {@link formatMinutes} but for second-based totals. */
export function formatSeconds(totalSecs: number): string {
  return formatMinutes(Math.floor(totalSecs / 60))
}

/** Stopwatch display: `"1:05:09"` with hours, `"05:09"` without. */
export function formatElapsed(ms: number): string {
  const totalSeconds = Math.max(0, Math.floor(ms / 1000))
  const hours = Math.floor(totalSeconds / 3600)
  const minutes = Math.floor((totalSeconds % 3600) / 60)
  const seconds = totalSeconds % 60
  const mm = String(minutes).padStart(2, '0')
  const ss = String(seconds).padStart(2, '0')

  return hours > 0 ? `${hours}:${mm}:${ss}` : `${mm}:${ss}`
}

/** Minutes since local midnight → `"09:30"`. */
export function minutesToClock(min: number): string {
  const h = Math.floor(min / 60)
  const m = Math.floor(min % 60)

  return `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}`
}

/**
 * Local minutes since midnight for an RFC3339/ISO timestamp, with the seconds
 * kept as a fraction so short blocks still land in the right place on the
 * timeline. Backend timestamps are UTC; the browser converts them to local time.
 */
export function isoToLocalMinutes(iso: string): number {
  const d = new Date(iso)

  return d.getHours() * 60 + d.getMinutes() + d.getSeconds() / 60
}

/**
 * Strip the trailing app-name segment from a window title so the summary panels
 * do not repeat what the app column already says:
 *
 *   "project — file.ts — Visual Studio Code"  →  "project — file.ts"
 *   "Visual Studio Code"                      →  "" (nothing left to show)
 */
export function cleanWindowTitle(appName: string, title: string): string {
  const parts = title.split(' \u2014 ')
  const app = appName.toLowerCase()

  if (parts.length > 1 && parts[parts.length - 1].trim().toLowerCase() === app) {
    parts.pop()
  }

  const cleaned = parts.join(' \u2014 ').trim()

  return cleaned.toLowerCase() === app ? '' : cleaned
}
