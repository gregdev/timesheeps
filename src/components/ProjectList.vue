<script setup lang="ts">
  import { ref, computed } from 'vue'
  import { useProjectsStore } from '../stores/projects'
  import { regexError } from '../composables/useRegex'

  const props = withDefaults(defineProps<{ selectedId?: number | null }>(), {
    selectedId: null,
  })

  const emit = defineEmits<{
    (e: 'select', id: number): void
    (e: 'changed'): void
  }>()

  const store = useProjectsStore()

  const editing = ref<{
    id: number
    name: string
    color: string
    parentId: number | null
    subGroupPattern: string | null
  } | null>(null)
  const creating = ref(false)
  const newName = ref('')
  const newColor = ref('#6366f1')
  const newParentId = ref<number | null>(null)
  /** Optional default ticket regex shared by this project's match rules. */
  const newSubGroupPattern = ref('')
  const confirmingDelete = ref<number | null>(null)

  const PRESET_COLORS = [
    '#6366f1',
    '#8b5cf6',
    '#ec4899',
    '#f43f5e',
    '#f97316',
    '#eab308',
    '#22c55e',
    '#10b981',
    '#06b6d4',
    '#3b82f6',
  ]

  // Build display tree: active roots with their active children, then archived
  const treeRows = computed(() => {
    const rows: { project: (typeof store.projects)[number]; depth: 0 | 1 }[] = []
    const all = store.projects

    const activeRoots = all.filter((p) => !p.archivedAt && !p.parentId)

    for (const root of activeRoots) {
      rows.push({ project: root, depth: 0 })
      const children = all.filter((p) => p.parentId === root.id && !p.archivedAt)

      for (const child of children) {
        rows.push({ project: child, depth: 1 })
      }
    }

    const archivedRoots = all.filter((p) => !!p.archivedAt && !p.parentId)

    for (const root of archivedRoots) {
      rows.push({ project: root, depth: 0 })
      const children = all.filter((p) => p.parentId === root.id && !!p.archivedAt)

      for (const child of children) {
        rows.push({ project: child, depth: 1 })
      }
    }

    return rows
  })

  // Only root active projects can be parents (no grandparents)
  const parentOptions = computed(() =>
    store.roots.filter((p) => (editing.value ? p.id !== editing.value.id : true)),
  )

  function hasActiveChildren(id: number) {
    return store.projects.some((p) => p.parentId === id && !p.archivedAt)
  }

  /**
   * Client-side regex check for the optional ticket pattern.
   * Rust stores a bad pattern without complaining and simply yields no
   * sub-groups, so an unvalidated typo would look like it worked.
   */
  const newPatternError = computed(() => regexError(newSubGroupPattern.value))
  const editPatternError = computed(() => regexError(editing.value?.subGroupPattern))

  async function submitCreate() {
    if (!newName.value.trim() || newPatternError.value) {
      return
    }

    const created = await store.create(
      newName.value.trim(),
      newColor.value,
      newParentId.value,
      newSubGroupPattern.value.trim() || null,
    )
    newName.value = ''
    newColor.value = '#6366f1'
    newParentId.value = null
    newSubGroupPattern.value = ''
    creating.value = false
    emit('changed')
    emit('select', created.id)
  }

  function cancelCreate() {
    creating.value = false
    newParentId.value = null
    newSubGroupPattern.value = ''
  }

  function cancelEdit() {
    editing.value = null
  }

  async function submitEdit() {
    if (!editing.value || !editing.value.name.trim() || editPatternError.value) {
      return
    }

    await store.update(
      editing.value.id,
      editing.value.name.trim(),
      editing.value.color,
      editing.value.parentId,
      editing.value.subGroupPattern?.trim() || null,
    )

    editing.value = null
    emit('changed')
  }

  async function doArchive(id: number) {
    await store.archive(id)
    emit('changed')
  }

  async function doUnarchive(id: number) {
    await store.unarchive(id)
    emit('changed')
  }

  async function doDelete(id: number) {
    await store.remove(id)
    confirmingDelete.value = null
    emit('changed')
  }
</script>

<template>
  <div class="project-list">
    <div class="section-header">
      <h2>Projects</h2>
      <button class="btn-primary" @click="creating = true">+ New project</button>
    </div>
    <!-- Create form -->
    <div v-if="creating" class="edit-form">
      <div class="form-row">
        <div class="form-group" style="flex: 1">
          <label>Name</label>
          <input
            v-model="newName"
            placeholder="e.g. Sendgrid"
            autofocus
            @keyup.enter="submitCreate"
          />
        </div>

        <div class="form-group color-picker">
          <label>Color</label>
          <div class="swatches">
            <button
              v-for="c in PRESET_COLORS"
              :key="c"
              class="swatch"
              :style="{ background: c, outline: newColor === c ? `3px solid ${c}` : 'none' }"
              @click="newColor = c"
            />
          </div>
        </div>
      </div>

      <div class="form-group">
        <label>Parent project (optional)</label>

        <select v-model="newParentId">
          <option :value="null">None — standalone project</option>
          <option v-for="p in store.roots" :key="p.id" :value="p.id">{{ p.name }}</option>
        </select>
      </div>

      <div class="form-group">
        <label>Ticket pattern (optional)</label>
        <input
          v-model="newSubGroupPattern"
          placeholder="Regex, e.g. ELMS-\d+"
          spellcheck="false"
          @keyup.enter="submitCreate"
        />
        <p v-if="newPatternError" class="field-hint hint-error">
          Invalid regex: {{ newPatternError }}
        </p>
        <p v-else class="field-hint">
          Used to split this project's time by ticket. Match rules inherit it unless they define
          their own.
        </p>
      </div>

      <div class="form-actions">
        <button class="btn-secondary" @click="cancelCreate">Cancel</button>

        <button
          class="btn-primary"
          :disabled="!newName.trim() || !!newPatternError"
          @click="submitCreate"
        >
          Add
        </button>
      </div>
    </div>

    <!-- Tree list -->
    <div class="list">
      <div v-if="treeRows.length === 0" class="empty">No projects yet.</div>

      <div
        v-for="{ project: p, depth } in treeRows"
        :key="p.id"
        class="project-row"
        :class="{
          archived: !!p.archivedAt,
          child: depth === 1,
          selected: props.selectedId === p.id,
        }"
        @click="emit('select', p.id)"
      >
        <template v-if="editing?.id === p.id">
          <div class="form-row" style="flex: 1; flex-wrap: wrap; gap: 8px">
            <input
              v-model="editing.name"
              style="flex: 1; min-width: 120px"
              autofocus
              @click.stop
              @keyup.enter="submitEdit"
            />

            <div class="swatches inline" @click.stop>
              <button
                v-for="c in PRESET_COLORS"
                :key="c"
                class="swatch"
                :style="{ background: c, outline: editing.color === c ? `3px solid ${c}` : 'none' }"
                @click="editing.color = c"
              />
            </div>

            <!-- Parent select only for projects that have no active children -->
            <select
              v-if="!hasActiveChildren(p.id)"
              v-model="editing.parentId"
              style="width: 100%"
              @click.stop
            >
              <option :value="null">None — standalone</option>
              <option v-for="parent in parentOptions" :key="parent.id" :value="parent.id">
                {{ parent.name }}
              </option>
            </select>

            <div style="width: 100%" @click.stop>
              <input
                v-model="editing.subGroupPattern"
                placeholder="Ticket pattern (optional regex, e.g. ELMS-\d+)"
                spellcheck="false"
                style="width: 100%"
                @keyup.enter="submitEdit"
              />
              <p v-if="editPatternError" class="field-hint hint-error">
                Invalid regex: {{ editPatternError }}
              </p>
            </div>
          </div>

          <button class="btn-secondary" @click.stop="cancelEdit">Cancel</button>
          <button class="btn-primary" :disabled="!!editPatternError" @click.stop="submitEdit">
            Save
          </button>
        </template>

        <template v-else>
          <span v-if="depth === 1" class="child-indent">↳</span>
          <span class="dot" :style="{ background: p.color }" />
          <span class="p-name" :class="{ 'text-muted': !!p.archivedAt }">
            {{ p.name }}
            <span v-if="p.archivedAt" class="archived-tag">archived</span>
          </span>

          <span
            v-if="p.subGroupPattern"
            class="pattern-tag"
            :title="`Ticket pattern: ${p.subGroupPattern}`"
          >
            ticket
          </span>

          <div class="row-actions" @click.stop>
            <template v-if="confirmingDelete === p.id">
              <span class="confirm-text">Delete permanently?</span>
              <button class="btn-ghost danger" @click="doDelete(p.id)">Delete</button>
              <button class="btn-ghost" @click="confirmingDelete = null">Cancel</button>
            </template>

            <template v-else>
              <button
                v-if="!p.archivedAt"
                class="btn-ghost"
                @click="
                  editing = {
                    id: p.id,
                    name: p.name,
                    color: p.color,
                    parentId: p.parentId,
                    subGroupPattern: p.subGroupPattern,
                  }
                "
              >
                Edit
              </button>

              <button v-if="!p.archivedAt" class="btn-ghost" @click="doArchive(p.id)">
                Archive
              </button>

              <button v-else class="btn-ghost" @click="doUnarchive(p.id)">Restore</button>

              <button class="btn-ghost danger" @click="confirmingDelete = p.id">Delete</button>
            </template>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
  .project-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .hint-error {
    color: var(--danger);
  }

  .pattern-tag {
    flex-shrink: 0;
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-faint);
    border: 1px solid currentcolor;
    border-radius: 999px;
    padding: 0 6px;
    opacity: 0.75;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .edit-form {
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    padding: var(--space-3);
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .form-row {
    display: flex;
    gap: var(--space-3);
    align-items: flex-end;
    flex-wrap: wrap;
  }

  .color-picker {
    flex-shrink: 0;
  }

  .form-actions {
    display: flex;
    gap: var(--space-2);
    justify-content: flex-end;
  }

  .swatches {
    display: flex;
    gap: var(--space-1);
    flex-wrap: wrap;
  }

  .swatches.inline {
    align-self: center;
  }

  .swatch {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    border: 2px solid var(--surface);
    cursor: pointer;
    padding: 0;
    outline-offset: 2px;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .empty {
    font-size: var(--text-sm);
    color: var(--text-muted);
    padding: var(--space-3) 0;
  }

  .project-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: var(--space-2) 10px;
    border-radius: var(--radius);
    background: var(--surface);
    border: 1px solid var(--border);
    transition: background 0.12s;
  }

  .project-row.archived {
    opacity: 0.5;
  }

  .project-row.child {
    margin-left: 20px;
    background: var(--surface-2);
  }

  .child-indent {
    font-size: var(--text-xs);
    color: var(--text-faint);
    flex-shrink: 0;
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .p-name {
    flex: 1;
    font-size: var(--text-sm);
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .text-muted {
    color: var(--text-muted);
  }

  .archived-tag {
    font-size: var(--text-xs);
    background: var(--surface-2);
    color: var(--text-faint);
    padding: 1px var(--space-1);
    border-radius: 4px;
  }

  .row-actions {
    display: flex;
    gap: var(--space-1);
  }

  .btn-ghost.danger {
    color: var(--danger);
    transition:
      background 0.1s,
      color 0.1s;
  }

  .btn-ghost.danger:hover {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }

  .project-row.selected {
    background: var(--surface-2);
    border-color: var(--border-strong);
  }

  .confirm-text {
    font-size: var(--text-xs);
    color: var(--danger);
    align-self: center;
    white-space: nowrap;
  }
</style>
