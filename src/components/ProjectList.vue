<script setup lang="ts">
import { ref } from 'vue'
import { useProjectsStore } from '../stores/projects'

const store = useProjectsStore()

const editing = ref<{ id: number; name: string; color: string } | null>(null)
const creating = ref(false)
const newName = ref('')
const newColor = ref('#6366f1')

const PRESET_COLORS = [
  '#6366f1', '#8b5cf6', '#ec4899', '#f43f5e', '#f97316',
  '#eab308', '#22c55e', '#10b981', '#06b6d4', '#3b82f6',
]

async function submitCreate() {
  if (!newName.value.trim()) return
  await store.create(newName.value.trim(), newColor.value)
  newName.value = ''
  newColor.value = '#6366f1'
  creating.value = false
}

async function submitEdit() {
  if (!editing.value || !editing.value.name.trim()) return
  await store.update(editing.value.id, editing.value.name.trim(), editing.value.color)
  editing.value = null
}
</script>

<template>
  <div class="project-list">
    <div class="section-header">
      <h3>Projects</h3>
      <button class="btn-primary" @click="creating = true">+ New project</button>
    </div>

    <!-- Create form -->
    <div v-if="creating" class="edit-form">
      <div class="form-row">
        <div class="form-group" style="flex:1">
          <label>Name</label>
          <input v-model="newName" placeholder="e.g. Acme Corp" @keyup.enter="submitCreate" autofocus />
        </div>
        <div class="form-group color-picker">
          <label>Color</label>
          <div class="swatches">
            <button
              v-for="c in PRESET_COLORS" :key="c"
              class="swatch"
              :style="{ background: c, outline: newColor === c ? `3px solid ${c}` : 'none' }"
              @click="newColor = c"
            />
          </div>
        </div>
      </div>
      <div class="form-actions">
        <button class="btn-secondary" @click="creating = false">Cancel</button>
        <button class="btn-primary" @click="submitCreate" :disabled="!newName.trim()">Add</button>
      </div>
    </div>

    <!-- List -->
    <div class="list">
      <div v-if="store.projects.length === 0" class="empty">No projects yet.</div>
      <div v-for="p in store.projects" :key="p.id" class="project-row" :class="{ archived: !!p.archivedAt }">
        <template v-if="editing?.id === p.id">
          <div class="form-row" style="flex:1">
            <input v-model="editing.name" style="flex:1" @keyup.enter="submitEdit" autofocus />
            <div class="swatches inline">
              <button
                v-for="c in PRESET_COLORS" :key="c"
                class="swatch"
                :style="{ background: c, outline: editing.color === c ? `3px solid ${c}` : 'none' }"
                @click="editing.color = c"
              />
            </div>
          </div>
          <button class="btn-secondary" @click="editing = null">Cancel</button>
          <button class="btn-primary" @click="submitEdit">Save</button>
        </template>
        <template v-else>
          <span class="dot" :style="{ background: p.color }" />
          <span class="p-name" :class="{ 'text-muted': !!p.archivedAt }">
            {{ p.name }}
            <span v-if="p.archivedAt" class="archived-tag">archived</span>
          </span>
          <div class="row-actions">
            <button v-if="!p.archivedAt" class="btn-ghost" @click="editing = { id: p.id, name: p.name, color: p.color }">Edit</button>
            <button v-if="!p.archivedAt" class="btn-ghost danger" @click="store.archive(p.id)">Archive</button>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.project-list { display: flex; flex-direction: column; gap: 12px; }
.section-header { display: flex; align-items: center; justify-content: space-between; }
.section-header h3 { font-size: 14px; font-weight: 600; }

.edit-form {
  background: var(--surface-2);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 10px;
}

.form-row { display: flex; gap: 12px; align-items: flex-end; flex-wrap: wrap; }
.color-picker { flex-shrink: 0; }
.form-actions { display: flex; gap: 8px; justify-content: flex-end; }

.swatches { display: flex; gap: 5px; flex-wrap: wrap; }
.swatches.inline { align-self: center; }
.swatch {
  width: 20px; height: 20px;
  border-radius: 50%;
  border: 2px solid var(--surface);
  cursor: pointer;
  padding: 0;
  outline-offset: 2px;
}

.list { display: flex; flex-direction: column; gap: 1px; }
.empty { font-size: 13px; color: var(--text-muted); padding: 12px 0; }

.project-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
  border-radius: var(--radius);
  background: var(--surface);
  border: 1px solid var(--border);
}
.project-row.archived { opacity: 0.5; }

.dot { width: 10px; height: 10px; border-radius: 50%; flex-shrink: 0; }
.p-name { flex: 1; font-size: 13px; display: flex; align-items: center; gap: 6px; }
.text-muted { color: var(--text-muted); }
.archived-tag {
  font-size: 10px;
  background: var(--surface-2);
  color: var(--text-faint);
  padding: 1px 5px;
  border-radius: 4px;
}

.row-actions { display: flex; gap: 4px; }
.btn-ghost.danger { color: var(--danger); }
.btn-ghost.danger:hover { background: color-mix(in srgb, var(--danger) 10%, transparent); }
</style>
