<script setup lang="ts">
  import { ref, computed, watch, onMounted, onUnmounted, nextTick } from 'vue'
  import { useProjectsStore } from '../stores/projects'
  import { useTimeline } from '../composables/useTimeline'
  import type { Project } from '../schemas'

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

  const initialProject = props.initialProjectId ? projectsStore.byId(props.initialProjectId) : null
  const query = ref(initialProject?.name ?? '')
  const selectedProjectId = ref<number | null>(props.initialProjectId ?? null)
  const note = ref(props.initialNote)
  const startMin = ref(props.initialStart)
  const endMin = ref(props.initialEnd)
  const showDropdown = ref(false)
  const highlightedIndex = ref(-1)
  const inputRef = ref<HTMLInputElement>()

  onMounted(() => {
    nextTick(() => inputRef.value?.focus())
  })

  const durationMin = computed(() => endMin.value - startMin.value)
  const isEditing = computed(() => props.entryId !== null)

  const filtered = computed(() => {
    const q = query.value.trim().toLowerCase()

    if (!q) {
      // Grouped order: roots then their children
      const result: { project: Project; depth: 0 | 1 }[] = []
      for (const root of projectsStore.roots) {
        result.push({ project: root, depth: 0 })
        for (const child of projectsStore.childrenOf(root.id)) {
          result.push({ project: child, depth: 1 })
        }
      }
      // Orphan children (parent archived) shown standalone
      const inResult = new Set(result.map((r) => r.project.id))
      for (const p of projectsStore.active) {
        if (!inResult.has(p.id)) result.push({ project: p, depth: 0 })
      }
      return result
    }

    // Search: match own name OR parent name
    return projectsStore.active
      .filter((p) => {
        if (p.name.toLowerCase().includes(q)) return true
        if (p.parentId !== null) {
          return projectsStore.byId(p.parentId)?.name.toLowerCase().includes(q) ?? false
        }
        return false
      })
      .map((p) => ({ project: p, depth: 0 as const }))
  })

  const exactMatch = computed(() =>
    projectsStore.active.find((p) => p.name.toLowerCase() === query.value.trim().toLowerCase()),
  )

  const showCreate = computed(() => query.value.trim().length > 0 && !exactMatch.value)

  const canSave = computed(() => selectedProjectId.value !== null || query.value.trim().length > 0)

  watch(query, (val) => {
    const match = projectsStore.active.find(
      (p) => p.name.toLowerCase() === val.trim().toLowerCase(),
    )
    selectedProjectId.value = match?.id ?? null
    highlightedIndex.value = -1
  })

  function onFocus() {
    showDropdown.value = true
  }

  function onBlur() {
    setTimeout(() => {
      showDropdown.value = false
    }, 150)
  }

  function selectProject(p: Project) {
    selectedProjectId.value = p.id
    query.value = p.name
    showDropdown.value = false
  }

  async function createAndSelect() {
    const name = query.value.trim()

    if (!name) {
      return
    }

    const p = await projectsStore.create(name, '#6366f1')
    selectProject(p)
  }

  function onKeydown(e: KeyboardEvent) {
    const total = filtered.value.length + (showCreate.value ? 1 : 0)

    if (e.key === 'ArrowDown') {
      e.preventDefault()
      showDropdown.value = true
      highlightedIndex.value = Math.min(highlightedIndex.value + 1, total - 1)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      highlightedIndex.value = Math.max(highlightedIndex.value - 1, -1)
    } else if (e.key === 'Enter') {
      e.preventDefault()

      if (highlightedIndex.value >= 0 && highlightedIndex.value < filtered.value.length) {
        e.stopPropagation()
        selectProject(filtered.value[highlightedIndex.value].project)
      } else if (showCreate.value) {
        e.stopPropagation()
        createAndSelect()
      }
    } else if (e.key === 'Escape') {
      showDropdown.value = false
      emit('cancel')
    }
  }

  async function save() {
    let projectId = selectedProjectId.value

    if (!projectId && query.value.trim()) {
      const p = await projectsStore.create(query.value.trim(), '#6366f1')
      projectId = p.id
    }

    if (!projectId) {
      return
    }

    emit('save', projectId, startMin.value, endMin.value, note.value)
  }

  function onDelete() {
    if (props.entryId) {
      emit('delete', props.entryId)
    }
  }

  function onDocKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      emit('cancel')
    } else if (e.key === 'Enter') {
      const isCtrlEnter = e.ctrlKey
      const targetIsTextarea = e.target instanceof HTMLTextAreaElement
      if ((isCtrlEnter || !targetIsTextarea) && canSave.value) {
        e.preventDefault()
        save()
      }
    }
  }

  onMounted(() => document.addEventListener('keydown', onDocKeydown))
  onUnmounted(() => document.removeEventListener('keydown', onDocKeydown))
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
          <div class="autocomplete">
            <input
              ref="inputRef"
              v-model="query"
              type="text"
              placeholder="Type to search or create..."
              autocomplete="off"
              @focus="onFocus"
              @blur="onBlur"
              @keydown="onKeydown"
            />
            <div v-if="showDropdown && (filtered.length > 0 || showCreate)" class="dropdown">
              <div
                v-for="({ project: p, depth }, i) in filtered"
                :key="p.id"
                class="dropdown-item"
                :class="{ highlighted: highlightedIndex === i, child: depth === 1 }"
                @mousedown.prevent="selectProject(p)"
              >
                <span class="color-dot" :style="{ background: p.color }" />
                <span class="item-label">
                  <span v-if="depth === 1 && p.parentId" class="parent-hint">
                    {{ projectsStore.byId(p.parentId)?.name }} ›
                  </span>
                  {{ p.name }}
                </span>
              </div>
              <div
                v-if="showCreate"
                class="dropdown-item create-item"
                :class="{ highlighted: highlightedIndex === filtered.length }"
                @mousedown.prevent="createAndSelect"
              >
                + Create "{{ query.trim() }}"
              </div>
            </div>
          </div>
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
        <button class="btn-primary" :disabled="!canSave" @click="save">
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
    background: rgb(0 0 0 / 35%);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
    animation: backdrop-in 0.15s ease;
  }

  .modal {
    background: var(--surface);
    border-radius: 10px;
    box-shadow: var(--shadow-md);
    width: 340px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: modal-in 0.15s ease 0.05s both;
  }

  @keyframes backdrop-in {
    from {
      opacity: 0;
    }

    to {
      opacity: 1;
    }
  }

  @keyframes modal-in {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(-6px);
    }

    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
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

  .close-btn {
    font-size: 14px;
  }

  .modal-body {
    padding: 14px 16px;
  }

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

  .autocomplete {
    position: relative;
  }

  .autocomplete input {
    width: 100%;
    box-sizing: border-box;
  }

  .dropdown {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow: var(--shadow-md);
    z-index: 10;
    max-height: 180px;
    overflow-y: auto;
    margin-top: 2px;
    animation: dropdown-in 0.1s ease;
  }

  @keyframes dropdown-in {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }

    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .dropdown-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 7px 10px;
    cursor: pointer;
    font-size: 13px;
    transition: background 0.1s;
  }

  .dropdown-item:hover,
  .dropdown-item.highlighted {
    background: var(--surface-2);
  }

  .color-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .item-label {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .parent-hint {
    color: var(--text-faint);
    font-size: 11px;
    flex-shrink: 0;
  }

  .dropdown-item.child {
    padding-left: 24px;
  }

  .create-item {
    color: var(--primary);
  }

  .modal-footer {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 16px 14px;
    border-top: 1px solid var(--border);
  }

  .spacer {
    flex: 1;
  }
</style>
