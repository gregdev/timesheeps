<script setup lang="ts">
  import { ref, computed } from 'vue'
  import { useSettingsStore } from '../stores/settings'
  import { useProjectsStore } from '../stores/projects'
  import type { FilterRuleType } from '../schemas'

  const settingsStore = useSettingsStore()
  const projectsStore = useProjectsStore()

  const newProjectId = ref<number | null>(null)
  const newType = ref<FilterRuleType>('title_pattern')
  const newValue = ref('')

  const activeProjects = computed(() => projectsStore.projects.filter((p) => !p.archivedAt))

  async function addRule() {
    if (!newValue.value.trim() || newProjectId.value === null) return
    await settingsStore.createMatchRule(newProjectId.value, newType.value, newValue.value.trim())
    newValue.value = ''
  }

  function projectName(id: number) {
    const p = projectsStore.byId(id)
    if (!p) return '(deleted)'
    if (p.parentId) {
      const parent = projectsStore.byId(p.parentId)
      return parent ? `${parent.name} › ${p.name}` : p.name
    }
    return p.name
  }

  function projectColor(id: number) {
    return projectsStore.byId(id)?.color ?? '#6366f1'
  }
</script>

<template>
  <div class="match-rules">
    <div class="section-header">
      <h3>Project Match Rules</h3>
    </div>
    <p class="hint">
      Activity matching these rules will be suggested on the timeline. Click a suggestion to accept
      it as a time entry.
    </p>

    <!-- Add rule form -->
    <div class="add-form">
      <select v-model="newProjectId" style="flex: 1; min-width: 120px">
        <option :value="null" disabled>Select project…</option>
        <option v-for="p in activeProjects" :key="p.id" :value="p.id">
          {{ projectName(p.id) }}
        </option>
      </select>
      <select v-model="newType" style="width: 140px; flex-shrink: 0">
        <option value="title_pattern">Title contains</option>
        <option value="app_name">App name is</option>
      </select>
      <input
        v-model="newValue"
        placeholder="e.g. Jira"
        style="flex: 1; min-width: 100px"
        @keyup.enter="addRule"
      />
      <button
        class="btn-primary"
        :disabled="!newValue.trim() || newProjectId === null"
        @click="addRule"
      >
        Add
      </button>
    </div>

    <!-- Rules list -->
    <div class="list">
      <div v-if="settingsStore.projectMatchRules.length === 0" class="empty">
        No match rules yet.
      </div>
      <div v-for="rule in settingsStore.projectMatchRules" :key="rule.id" class="rule-row">
        <span class="color-dot" :style="{ background: projectColor(rule.projectId) }" />
        <span class="rule-project">{{ projectName(rule.projectId) }}</span>
        <span class="rule-type">
          {{ rule.ruleType === 'title_pattern' ? 'Title contains' : 'App name is' }}
        </span>
        <code class="rule-value">{{ rule.value }}</code>
        <button class="btn-ghost danger" @click="settingsStore.deleteMatchRule(rule.id)">
          Remove
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
  .match-rules {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .section-header h3 {
    font-size: var(--text-sm);
    font-weight: 600;
  }

  .hint {
    font-size: var(--text-xs);
    color: var(--text-muted);
    margin: 0;
  }

  .add-form {
    display: flex;
    gap: var(--space-2);
    align-items: center;
    flex-wrap: wrap;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .empty {
    font-size: var(--text-sm);
    color: var(--text-muted);
    padding: var(--space-2) 0;
  }

  .rule-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 7px 10px;
    border-radius: var(--radius);
    background: var(--surface);
    border: 1px solid var(--border);
    font-size: var(--text-sm);
  }

  .color-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .rule-project {
    font-weight: 600;
    flex-shrink: 0;
  }

  .rule-type {
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .rule-value {
    flex: 1;
    background: var(--surface-2);
    padding: 1px 6px;
    border-radius: 3px;
    font-size: var(--text-xs);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .btn-ghost.danger {
    color: var(--danger);
    transition:
      background 0.1s,
      color 0.1s;
    flex-shrink: 0;
  }

  .btn-ghost.danger:hover {
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }
</style>
