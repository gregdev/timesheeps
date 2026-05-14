<script setup lang="ts">
import { ref, computed } from 'vue'
import { useProjectsStore } from '../stores/projects'
import { useTimeline } from '../composables/useTimeline'

const props = defineProps<{
  initialStart: number
  initialEnd: number
  initialProjectId: number | null
  initialNote: string
  entryId: number | null
}>()

const emit = defineEmits<{
  (e: 'save', projectId: number, startMinutes: number, endMinutes: number, note: string): void
  (e: 'delete', id: number): void
  (e: 'cancel'): void
}>()

const projectsStore = useProjectsStore()
const { minutesToTime, formatDuration } = useTimeline()

const selectedProjectId = ref<number | null>(props.initialProjectId ?? projectsStore.active[0]?.id ?? null)
const note = ref(props.initialNote)
const startMin = ref(props.initialStart)
const endMin = ref(props.initialEnd)

const durationMin = computed(() => endMin.value - startMin.value)
const isEditing = computed(() => props.entryId !== null)

function save() {
  if (!selectedProjectId.value) return
  emit('save', selectedProjectId.value, startMin.value, endMin.value, note.value)
}

function onDelete() {
  if (props.entryId) emit('delete', props.entryId)
}
</script>

<template>
  <div class="modal-backdrop" @mousedown.self="emit('cancel')">
    <div class="modal">
      <div class="modal-header">
        <span>{{ isEditing ? 'Edit time entry' : 'New time entry' }}</span>
        <button class="btn-ghost close-btn" @click="emit('cancel')">✕</button>
      </div>

      <div class="modal-body">
        <div class="time-range">
          {{ minutesToTime(startMin) }} – {{ minutesToTime(endMin) }}
          <span class="duration-badge">{{ formatDuration(durationMin) }}</span>
        </div>

        <div class="form-group">
          <label>Project</label>
          <select v-model="selectedProjectId">
            <option v-for="p in projectsStore.active" :key="p.id" :value="p.id">
              {{ p.name }}
            </option>
          </select>
          <p v-if="projectsStore.active.length === 0" class="hint">
            No projects yet — add one in Settings first.
          </p>
        </div>

        <div class="form-group">
          <label>Note (optional)</label>
          <textarea v-model="note" placeholder="What were you working on?" rows="2" />
        </div>
      </div>

      <div class="modal-footer">
        <button v-if="isEditing" class="btn-danger" @click="onDelete">Delete</button>
        <div class="spacer" />
        <button class="btn-secondary" @click="emit('cancel')">Cancel</button>
        <button class="btn-primary" :disabled="!selectedProjectId" @click="save">
          {{ isEditing ? 'Save' : 'Add' }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  background: rgba(0,0,0,0.35);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}

.modal {
  background: var(--surface);
  border-radius: 10px;
  box-shadow: var(--shadow-md);
  width: 340px;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 16px 10px;
  font-weight: 600;
  font-size: 14px;
  border-bottom: 1px solid var(--border);
}

.close-btn { font-size: 14px; }

.modal-body { padding: 14px 16px; }

.time-range {
  font-size: 15px;
  font-weight: 600;
  margin-bottom: 14px;
  display: flex;
  align-items: center;
  gap: 8px;
}

.duration-badge {
  font-size: 12px;
  font-weight: 500;
  background: var(--surface-2);
  color: var(--text-muted);
  padding: 2px 8px;
  border-radius: 10px;
}

.hint {
  font-size: 11px;
  color: var(--text-muted);
  margin-top: 4px;
}

.modal-footer {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 16px 14px;
  border-top: 1px solid var(--border);
}

.spacer { flex: 1; }
</style>
