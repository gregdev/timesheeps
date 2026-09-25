import { ref } from 'vue'
import type { TimeEntry } from '../schemas'

/** One entry's worth of time. A single track action can produce several. */
export interface EntryRange {
  startMinutes: number
  endMinutes: number
}

export interface PendingCreate {
  startMinutes: number
  endMinutes: number
  note: string
  projectId?: number | null
  /**
   * Every session this action covers, when it covers more than one. A Window
   * Activity row aggregates sessions across a whole day, so tracking it creates
   * one entry per session rather than a single entry that spans the gaps between
   * them. Falls back to `startMinutes`/`endMinutes` when absent.
   */
  ranges?: EntryRange[]
  /** If set, the modal shows an auto-track toggle using this app name */
  autoTrackAppName?: string
  /** Whether the auto-track toggle starts checked */
  autoTrackEnabled?: boolean
}

// Module-level singleton — shared across all component instances
const pendingCreate = ref<PendingCreate | null>(null)
const editingEntry = ref<TimeEntry | null>(null)

export function useEntryModal() {
  return { pendingCreate, editingEntry }
}
