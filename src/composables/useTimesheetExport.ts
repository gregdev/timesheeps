import { format, parseISO } from 'date-fns'
import { formatMinutes, minutesToClock } from './useFormat'
import type { PeriodReview } from '../schemas'

/**
 * Clipboard formats for a whole pay period — the literal last step of filling
 * in a timesheet.
 *
 * `copyDay` on the timeline answers "what did I do today", which is not the
 * problem: the problem is ten days of work that has to be retyped into Harvest.
 * Two shapes are offered because both are useful: readable text for pasting into
 * a timesheet description, and TSV for pasting into a spreadsheet.
 */

type ProjectName = (id: number) => string

interface DayRow {
  date: string
  entries: PeriodReview['days'][number]['entries']
  note: string
}

function daysWithContent(review: PeriodReview): DayRow[] {
  return review.days.filter((d) => d.entries.length > 0 || d.note.trim() !== '')
}

function dayTotal(day: DayRow): number {
  return day.entries.reduce((sum, e) => sum + (e.endMinutes - e.startMinutes), 0)
}

/** Project totals across the range, largest first. */
export function projectTotals(
  review: PeriodReview,
  projectName: ProjectName,
): { id: number; name: string; minutes: number }[] {
  const totals = new Map<number, number>()

  for (const day of review.days) {
    for (const entry of day.entries) {
      totals.set(
        entry.projectId,
        (totals.get(entry.projectId) ?? 0) + (entry.endMinutes - entry.startMinutes),
      )
    }
  }

  return [...totals.entries()]
    .map(([id, minutes]) => ({ id, name: projectName(id), minutes }))
    .sort((a, b) => b.minutes - a.minutes)
}

/** Readable block: one section per day, then a project roll-up. */
export function periodText(review: PeriodReview, projectName: ProjectName): string {
  const lines: string[] = []
  const start = format(parseISO(review.startDate), 'EEE d MMM')
  const end = format(parseISO(review.endDate), 'EEE d MMM yyyy')

  lines.push(`Timesheet — ${start} – ${end}`, '')

  const days = daysWithContent(review)

  if (days.length === 0) {
    lines.push('No time logged in this period.')
    return lines.join('\n')
  }

  // Project names are padded so the columns line up in a monospace paste.
  const width = Math.max(
    ...days.flatMap((d) => d.entries.map((e) => projectName(e.projectId).length)),
    4,
  )

  for (const day of days) {
    lines.push(`${format(parseISO(day.date), 'EEE d MMM')}  (${formatMinutes(dayTotal(day))})`)

    for (const entry of day.entries) {
      const name = projectName(entry.projectId).padEnd(width)
      const range = `${minutesToClock(entry.startMinutes)}–${minutesToClock(entry.endMinutes)}`
      const duration = formatMinutes(entry.endMinutes - entry.startMinutes)
      const note = entry.note.trim() ? `  ${entry.note.trim()}` : ''
      lines.push(`  ${range}  ${name}  ${duration.padStart(6)}${note}`)
    }

    if (day.note.trim()) {
      lines.push(`  Note: ${day.note.trim()}`)
    }

    lines.push('')
  }

  lines.push(`Period total: ${formatMinutes(Math.round(review.totalLoggedSecs / 60))}`)

  for (const p of projectTotals(review, projectName)) {
    lines.push(`  ${p.name.padEnd(width)}  ${formatMinutes(p.minutes).padStart(6)}`)
  }

  return lines.join('\n')
}

/** Spreadsheet block: `date, project, start, end, hours, note`. */
export function periodTsv(review: PeriodReview, projectName: ProjectName): string {
  const rows: string[] = ['date\tproject\tstart\tend\thours\tnote']

  for (const day of review.days) {
    for (const entry of day.entries) {
      const hours = ((entry.endMinutes - entry.startMinutes) / 60).toFixed(2)
      rows.push(
        [
          day.date,
          projectName(entry.projectId),
          minutesToClock(entry.startMinutes),
          minutesToClock(entry.endMinutes),
          hours,
          entry.note.trim(),
        ].join('\t'),
      )
    }
  }

  return rows.join('\n')
}
