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

/**
 * How long a suggestion must be quiet before it may be auto-accepted, in
 * minutes.
 *
 * The poller keeps a row *open* while a window stays focused: every tick moves
 * `ended_at` forward, so an active session's suggestion always ends "now".
 * Auto-accepting that snapshot writes an entry for however much had been
 * recorded at that instant (often a single minute), and every later refresh
 * then skips the grown suggestion because it overlaps the entry just created —
 * leaving a permanent 1-minute entry for a 20-minute session. Waiting for the
 * block to stop moving means the entry covers the whole session.
 *
 * Comfortably larger than the 20s poll interval and the 30s UI refresh.
 */
const AUTO_ACCEPT_SETTLE_MINUTES = 2

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
  // `loadDay` can be in flight twice (the 30s interval plus the poller's
  // `activity-updated` event). Two overlapping passes would both see the same
  // suggestion as unclaimed and each create an entry for it.
  let autoAccepting = false

  const isViewingToday = computed(() => selectedDate.value === currentDate.value)

  // ── Undo / redo ─────────────────────────────────────────────────────────────
  //
  // Snapshot-based rather than command-based. A day holds a few dozen entries,
  // so copying the list is cheap, and composites like Merge (one update plus N
  // deletes) then undo as a single step for free — no per-operation inverse to
  // write or keep correct.
  //
  // The subtlety is identity: undoing a delete has to *re-create* the row, and
  // the backend issues a new id. Restore therefore matches by id first and falls
  // back to matching by content, so stale ids in an older snapshot still line up
  // with the rows that replaced them.

  interface HistorySnapshot {
    date: string
    entries: TimeEntry[]
  }

  /** How many steps each direction remembers. */
  const HISTORY_LIMIT = 50

  const undoStack = ref<HistorySnapshot[]>([])
  const redoStack = ref<HistorySnapshot[]>([])
  /**
   * True while `restore` is writing. Stops the restore's own API calls from
   * being mistaken for user edits (and thus re-recorded).
   */
  let applyingHistory = false

  const canUndo = computed(() => undoStack.value.length > 0)
  const canRedo = computed(() => redoStack.value.length > 0)

  function snapshot(): HistorySnapshot {
    return {
      date: selectedDate.value,
      entries: timeEntries.value.map((e) => ({ ...e })),
    }
  }

  function sameEntry(a: TimeEntry, b: TimeEntry): boolean {
    return (
      a.projectId === b.projectId &&
      a.startMinutes === b.startMinutes &&
      a.endMinutes === b.endMinutes &&
      a.note === b.note
    )
  }

  function sameEntries(a: TimeEntry[], b: TimeEntry[]): boolean {
    if (a.length !== b.length) {
      return false
    }

    const byId = new Map(b.map((e) => [e.id, e]))

    return a.every((e) => {
      const other = byId.get(e.id)

      return other !== undefined && sameEntry(e, other)
    })
  }

  /** Content identity, used when ids differ because a row was re-created. */
  function contentKey(e: TimeEntry): string {
    return `${e.projectId}|${e.startMinutes}|${e.endMinutes}|${e.note}`
  }

  /**
   * Run a mutation, recording the pre-state as one undo step if it changed
   * anything.
   *
   * Recording is deliberately *after* the fact: a mutation that throws leaves
   * both stacks untouched, and a no-op (a merge that turned out to be refused,
   * an update that set the same values) does not add a step the user would have
   * to press undo twice to get past.
   */
  async function withHistory<T>(mutate: () => Promise<T>): Promise<T> {
    if (applyingHistory) {
      return mutate()
    }

    const before = snapshot()
    const result = await mutate()

    if (!sameEntries(before.entries, timeEntries.value)) {
      undoStack.value = [...undoStack.value, before].slice(-HISTORY_LIMIT)
      redoStack.value = []
    }

    return result
  }

  /**
   * Drive the entry list back to `target`, touching only what differs.
   *
   * Deletes run first so an update can never be briefly applied on top of a row
   * that is on its way out.
   */
  async function restore(target: HistorySnapshot): Promise<void> {
    applyingHistory = true

    try {
      if (target.date !== selectedDate.value) {
        // History belongs to the day it was captured on; writing these rows onto
        // another date would be worse than refusing.
        return
      }

      const current = timeEntries.value.map((e) => ({ ...e }))
      const matched = new Set<number>()
      const pairs: { want: TimeEntry; have: TimeEntry | null }[] = []

      // Pass 1 — same id, so the row survived and can be updated in place.
      const byId = new Map(current.map((e) => [e.id, e]))

      for (const want of target.entries) {
        const have = byId.get(want.id)

        if (have && !matched.has(have.id)) {
          matched.add(have.id)
          pairs.push({ want, have })
        } else {
          pairs.push({ want, have: null })
        }
      }

      // Pass 2 — same content under a different id (an undone delete, whose
      // re-created row got a fresh id).
      const orphaned = current.filter((e) => !matched.has(e.id))

      for (const pair of pairs) {
        if (pair.have) {
          continue
        }

        const idx = orphaned.findIndex((e) => contentKey(e) === contentKey(pair.want))

        if (idx >= 0) {
          const have = orphaned.splice(idx, 1)[0]
          matched.add(have.id)
          pair.have = have
        }
      }

      const toDelete = current.filter((e) => !matched.has(e.id)).map((e) => e.id)
      const toUpdate = pairs.filter(
        (p): p is { want: TimeEntry; have: TimeEntry } =>
          p.have !== null && !sameEntry(p.have, p.want),
      )
      const toCreate = pairs.filter((p) => p.have === null).map((p) => p.want)

      await Promise.all(toDelete.map((id) => api.deleteTimeEntry(id)))

      for (const { want, have } of toUpdate) {
        await api.updateTimeEntry(
          have.id,
          want.projectId,
          want.startMinutes,
          want.endMinutes,
          want.note,
        )
      }

      for (const want of toCreate) {
        await api.createTimeEntry(
          target.date,
          want.projectId,
          want.startMinutes,
          want.endMinutes,
          want.note,
        )
      }

      // Re-read rather than trusting the local reconstruction: re-created rows
      // have ids only the backend knows, and every later diff depends on them.
      timeEntries.value = await api.getTimeEntriesForDay(selectedDate.value)
    } finally {
      applyingHistory = false
    }
  }

  /** @returns whether anything was undone, so a caller can avoid eating the keystroke. */
  async function undo(): Promise<boolean> {
    const target = undoStack.value[undoStack.value.length - 1]

    if (!target || applyingHistory || target.date !== selectedDate.value) {
      return false
    }

    undoStack.value = undoStack.value.slice(0, -1)
    redoStack.value = [...redoStack.value, snapshot()].slice(-HISTORY_LIMIT)
    await restore(target)

    return true
  }

  async function redo(): Promise<boolean> {
    const target = redoStack.value[redoStack.value.length - 1]

    if (!target || applyingHistory || target.date !== selectedDate.value) {
      return false
    }

    redoStack.value = redoStack.value.slice(0, -1)
    undoStack.value = [...undoStack.value, snapshot()].slice(-HISTORY_LIMIT)
    await restore(target)

    return true
  }

  function clearHistory() {
    undoStack.value = []
    redoStack.value = []
  }

  async function loadDay(date?: string, silent = false) {
    // A different day has entirely different entries, so steps captured on the
    // previous one are meaningless — and restoring them would write rows onto a
    // date they never belonged to.
    if (date && date !== selectedDate.value) {
      selectedDate.value = date
      clearHistory()
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

      if (settingsStore.settings.autoAcceptSuggested && suggestions.length > 0 && !autoAccepting) {
        autoAccepting = true

        try {
          const now = new Date()
          const nowMinutes = now.getHours() * 60 + now.getMinutes() + now.getSeconds() / 60
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

            // Still being recorded — the poller extends this block's end on
            // every tick, so its length is not final yet. Accepting it now
            // would freeze the sliver recorded so far into an entry that then
            // blocks the real span as an "overlap" on every later refresh.
            // A suggestion ending in the future (clock skew) also lands here.
            if (nowMinutes - endMin < AUTO_ACCEPT_SETTLE_MINUTES) {
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
        } finally {
          autoAccepting = false
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
    return withHistory(async () => {
      const created = await Promise.all(
        ranges.map((r) =>
          api.createTimeEntry(selectedDate.value, projectId, r.startMinutes, r.endMinutes, note),
        ),
      )
      timeEntries.value = [...timeEntries.value, ...created].sort(
        (a, b) => a.startMinutes - b.startMinutes,
      )

      return created
    })
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

  /** Raw update, with no history step — for composites that record their own. */
  async function applyUpdate(
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

  async function updateEntry(
    id: number,
    projectId: number,
    startMinutes: number,
    endMinutes: number,
    note: string,
  ) {
    await withHistory(() => applyUpdate(id, projectId, startMinutes, endMinutes, note))
  }

  /** Raw delete, with no history step — for composites that record their own. */
  async function removeEntries(ids: number[]) {
    if (ids.length === 0) {
      return
    }

    await Promise.all(ids.map((id) => api.deleteTimeEntry(id)))
    timeEntries.value = timeEntries.value.filter((e) => !ids.includes(e.id))
  }

  /**
   * Delete several entries at once. Confirmation belongs to the caller — this
   * is the same irreversible operation as `deleteEntry`, just repeated as one
   * undo step.
   */
  async function deleteEntries(ids: number[]) {
    await withHistory(() => removeEntries(ids))
  }

  async function deleteEntry(id: number) {
    await deleteEntries([id])
  }

  /**
   * Fold entry `id` into the entry immediately before it, keeping the earlier
   * entry's identity and extending its end.
   *
   * Deliberately strict: the two must be *adjacent* in time (no third entry
   * between them) and belong to the same project. Merging across a different
   * project's entry would create an overlap and bill that work here; merging
   * across a long gap would count untracked time as work.
   *
   * Returns false when the pair does not qualify, so a caller that judged the
   * menu item visible on stale data does not silently change anything.
   */
  async function mergeWithPrevious(id: number): Promise<boolean> {
    const sorted = [...timeEntries.value].sort((a, b) => a.startMinutes - b.startMinutes)
    const idx = sorted.findIndex((e) => e.id === id)

    if (idx <= 0) {
      return false
    }

    const target = sorted[idx]
    const previous = sorted[idx - 1]

    if (previous.projectId !== target.projectId || previous.endMinutes > target.startMinutes) {
      return false
    }

    // Validated before recording, so a refused merge leaves no undo step.
    await withHistory(async () => {
      // Update before deleting: if the update is rejected (the backend refuses
      // an inverted range) the later entry is still there to try again with.
      await applyUpdate(
        previous.id,
        previous.projectId,
        previous.startMinutes,
        Math.max(previous.endMinutes, target.endMinutes),
        previous.note || target.note,
      )
      await removeEntries([target.id])
    })

    return true
  }

  /**
   * Why merging `ids` would be wrong, or null when it is safe.
   *
   * Shared with the header bar so the Merge button is disabled with a reason
   * instead of failing after the click.
   */
  function mergeBlocker(ids: number[]): string | null {
    const selected = timeEntries.value.filter((e) => ids.includes(e.id))

    if (selected.length < 2) {
      return 'Select at least two entries to merge them.'
    }

    if (new Set(selected.map((e) => e.projectId)).size > 1) {
      return 'The selected entries belong to more than one project.'
    }

    const start = Math.min(...selected.map((e) => e.startMinutes))
    const end = Math.max(...selected.map((e) => e.endMinutes))

    if (end <= start) {
      return 'The selection has no duration — edit the entry first.'
    }

    // A span is fine across *gaps*, but crossing an entry that stays out of the
    // selection would leave the survivor overlapping it.
    const blocker = timeEntries.value.find(
      (e) => !ids.includes(e.id) && e.startMinutes < end && e.endMinutes > start,
    )

    return blocker
      ? `The merged span would cover another entry (${formatEntryRange(blocker)}).`
      : null
  }

  function formatEntryRange(entry: TimeEntry): string {
    const clock = (m: number) =>
      `${String(Math.floor(m / 60)).padStart(2, '0')}:${String(m % 60).padStart(2, '0')}`

    return `${clock(entry.startMinutes)}–${clock(entry.endMinutes)}`
  }

  /**
   * Fold several entries into one, spanning the earliest start to the latest
   * end and keeping the earliest entry's id.
   *
   * The gaps inside the span become counted time, which is the point of the
   * action — the user is asserting they were working throughout. Anything that
   * makes that assertion unsafe is refused by `mergeBlocker` instead of being
   * silently trimmed.
   */
  async function mergeEntries(ids: number[]): Promise<{ ok: boolean; reason?: string }> {
    const blocker = mergeBlocker(ids)

    if (blocker) {
      return { ok: false, reason: blocker }
    }

    const selected = timeEntries.value
      .filter((e) => ids.includes(e.id))
      .sort((a, b) => a.startMinutes - b.startMinutes)

    const survivor = selected[0]
    const start = survivor.startMinutes
    const end = Math.max(...selected.map((e) => e.endMinutes))
    // Keep the first non-blank note; the survivor's own note wins when set.
    const note = selected.find((e) => e.note.trim())?.note ?? ''

    await withHistory(async () => {
      await applyUpdate(survivor.id, survivor.projectId, start, end, note)
      await removeEntries(selected.slice(1).map((e) => e.id))
    })

    return { ok: true }
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
    deleteEntries,
    mergeWithPrevious,
    mergeBlocker,
    mergeEntries,
    canUndo,
    canRedo,
    undo,
    redo,
    clearHistory,
    summary,
  }
})
