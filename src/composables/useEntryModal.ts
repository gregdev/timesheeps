import { ref } from 'vue'
import type { TimeEntry } from '../schemas'

export interface PendingCreate {
  startMinutes: number
  endMinutes: number
  note: string
  projectId?: number | null
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
