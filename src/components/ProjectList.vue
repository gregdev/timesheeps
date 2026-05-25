<script setup lang="ts">
  import { ref, computed } from 'vue'
  import { useProjectsStore } from '../stores/projects'
  import { useSettingsStore } from '../stores/settings'
  import type { FilterRuleType } from '../schemas'

  const store = useProjectsStore()
  const settingsStore = useSettingsStore()

  const editing = ref<{ id: number; name: string; color: string; parentId: number | null } | null>(
    null,
  )
  const creating = ref(false)
  const newName = ref('')
  const newColor = ref('#6366f1')
  const newParentId = ref<number | null>(null)

  const newRuleType = ref<FilterRuleType>('title_pattern')
  const newRuleValue = ref('')

  const editingRules = computed(() =>
    editing.value
      ? settingsStore.projectMatchRules.filter((r) => r.projectId === editing.value!.id)
      : [],
  )

  async function addRule() {
    if (!newRuleValue.value.trim() || !editing.value) return
    await settingsStore.createMatchRule(
      editing.value.id,
      newRuleType.value,
      newRuleValue.value.trim(),
    )
    newRuleValue.value = ''
  }

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

  async function submitCreate() {
    if (!newName.value.trim()) {
      return
    }

    await store.create(newName.value.trim(), newColor.value, newParentId.value)
    newName.value = ''
    newColor.value = '#6366f1'
    newParentId.value = null
    creating.value = false
  }

  function cancelCreate() {
    creating.value = false
    newParentId.value = null
  }

  function cancelEdit() {
    editing.value = null
    newRuleValue.value = ''
  }

  async function submitEdit() {
    if (!editing.value || !editing.value.name.trim()) {
      return
    }

    await store.update(
      editing.value.id,
      editing.value.name.trim(),
      editing.value.color,
      editing.value.parentId,
    )

    editing.value = null
    newRuleValue.value = ''
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

      <div class="form-actions">
        <button class="btn-secondary" @click="cancelCreate">Cancel</button>

        <button class="btn-primary" :disabled="!newName.trim()" @click="submitCreate">Add</button>
      </div>
    </div>

    <!-- Tree list -->
    <div class="list">
      <div v-if="treeRows.length === 0" class="empty">No projects yet.</div>

      <div
        v-for="{ project: p, depth } in treeRows"
        :key="p.id"
        class="project-row"
        :class="{ archived: !!p.archivedAt, child: depth === 1 }"
      >
        <template v-if="editing?.id === p.id">
          <div class="form-row" style="flex: 1; flex-wrap: wrap; gap: 8px">
            <input
              v-model="editing.name"
              style="flex: 1; min-width: 120px"
              autofocus
              @keyup.enter="submitEdit"
            />

            <div class="swatches inline">
              <button
                v-for="c in PRESET_COLORS"
                :key="c"
                class="swatch"
                :style="{ background: c, outline: editing.color === c ? `3px solid ${c}` : 'none' }"
                @click="editing.color = c"
              />
            </div>

            <!-- Parent select only for projects that have no active children -->
            <select v-if="!hasActiveChildren(p.id)" v-model="editing.parentId" style="width: 100%">
              <option :value="null">None — standalone</option>
              <option v-for="parent in parentOptions" :key="parent.id" :value="parent.id">
                {{ parent.name }}
              </option>
            </select>

            <!-- Match rules -->
            <div class="match-rules-section">
              <div class="match-rules-label">Auto-match rules</div>
              <div class="match-add-row">
                <select v-model="newRuleType" class="rule-type-select">
                  <option value="title_pattern">Title contains</option>
                  <option value="app_name">App name is</option>
                </select>
                <input
                  v-model="newRuleValue"
                  placeholder="e.g. Jira"
                  class="rule-value-input"
                  @keyup.enter="addRule"
                />
                <button class="btn-secondary" :disabled="!newRuleValue.trim()" @click="addRule">
                  Add
                </button>
              </div>
              <div v-if="editingRules.length === 0" class="match-empty">No rules yet.</div>
              <div v-for="rule in editingRules" :key="rule.id" class="match-rule-row">
                <span class="rule-badge">
                  {{ rule.ruleType === 'title_pattern' ? 'title' : 'app' }}
                </span>
                <code class="rule-val">{{ rule.value }}</code>
                <button class="btn-ghost danger sm" @click="settingsStore.deleteMatchRule(rule.id)">
                  ×
                </button>
              </div>
            </div>
          </div>

          <button class="btn-secondary" @click="cancelEdit">Cancel</button>
          <button class="btn-primary" @click="submitEdit">Save</button>
        </template>

        <template v-else>
          <span v-if="depth === 1" class="child-indent">↳</span>
          <span class="dot" :style="{ background: p.color }" />
          <span class="p-name" :class="{ 'text-muted': !!p.archivedAt }">
            {{ p.name }}
            <span v-if="p.archivedAt" class="archived-tag">archived</span>
          </span>

          <div class="row-actions">
            <button
              v-if="!p.archivedAt"
              class="btn-ghost"
              @click="editing = { id: p.id, name: p.name, color: p.color, parentId: p.parentId }"
            >
              Edit
            </button>

            <button v-if="!p.archivedAt" class="btn-ghost danger" @click="store.archive(p.id)">
              Archive
            </button>
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

  .match-rules-section {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding-top: var(--space-2);
    border-top: 1px solid var(--border);
  }

  .match-rules-label {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .match-add-row {
    display: flex;
    gap: var(--space-2);
    align-items: center;
  }

  .rule-type-select {
    width: 130px;
    flex-shrink: 0;
  }

  .rule-value-input {
    flex: 1;
    min-width: 0;
  }

  .match-empty {
    font-size: var(--text-xs);
    color: var(--text-faint);
  }

  .match-rule-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-xs);
  }

  .rule-badge {
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 3px;
    padding: 1px var(--space-1);
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .rule-val {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .btn-ghost.sm {
    padding: 1px 6px;
    font-size: var(--text-sm);
    line-height: 1;
  }
</style>
