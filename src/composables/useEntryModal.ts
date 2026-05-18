import { ref } from 'vue'
import type { TimeEntry } from '../schemas'

export interface PendingCreate {
  startMinutes: number
  endMinutes: number
  note: string
  projectId?: number | null
}

// Module-level singleton — shared across all component instances
const pendingCreate = ref<PendingCreate | null>(null)
const editingEntry = ref<TimeEntry | null>(null)

export function useEntryModal() {
  return { pendingCreate, editingEntry }
}
