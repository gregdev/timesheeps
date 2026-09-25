<script setup lang="ts">
  /**
   * The global rule precedence list. Rules are checked top to bottom across all
   * projects and the first match wins, so the order shown here *is* the
   * behaviour. Hit counts make a rule that never matches impossible to miss.
   */
  import { computed, ref } from 'vue'
  import { useSettingsStore } from '../stores/settings'
  import { useProjectsStore } from '../stores/projects'
  import { formatSeconds } from '../composables/useFormat'
  import type { MatchCondition, ProjectMatchRule } from '../schemas'

  const emit = defineEmits<{ (e: 'edit', rule: ProjectMatchRule): void }>()

  const settingsStore = useSettingsStore()
  const projectsStore = useProjectsStore()

  const draggingId = ref<number | null>(null)
  const confirmingDelete = ref<number | null>(null)

  const rules = computed(() => settingsStore.orderedMatchRules)

  async function deleteRule(id: number) {
    await settingsStore.deleteMatchRule(id)
    confirmingDelete.value = null
  }

  function projectName(id: number): string {
    const p = projectsStore.byId(id)

    if (!p) {
      return '(deleted)'
    }
    if (p.parentId) {
      const parent = projectsStore.byId(p.parentId)
      return parent ? `${parent.name} › ${p.name}` : p.name
    }

    return p.name
  }

  function projectColour(id: number): string {
    return projectsStore.byId(id)?.color ?? '#6366f1'
  }

  const OPERATOR_LABELS: Record<string, string> = {
    contains: 'contains',
    equals: 'is',
    starts_with: 'starts with',
    ends_with: 'ends with',
  }

  function conditionText(condition: MatchCondition): string {
    const field = condition.field === 'app_name' ? 'app' : 'title'
    const not = condition.negate ? 'not ' : ''
    return `${field} ${not}${OPERATOR_LABELS[condition.operator]} "${condition.value}"`
  }

  function conditionSummary(rule: ProjectMatchRule): string {
    if (rule.conditions.length === 0) {
      return 'no conditions — never matches'
    }

    return rule.conditions.map(conditionText).join('  AND  ')
  }

  function statFor(ruleId: number) {
    return settingsStore.statsFor(ruleId)
  }

  function overlapNames(ruleId: number): string[] {
    return (statFor(ruleId)?.overlapsWith ?? []).map(projectName)
  }

  // ── Drag to reorder ────────────────────────────────────────────────────
  function onDragStart(rule: ProjectMatchRule) {
    draggingId.value = rule.id
  }

  function onDrop(target: ProjectMatchRule) {
    const sourceId = draggingId.value
    draggingId.value = null

    if (sourceId === null || sourceId === target.id) {
      return
    }

    const ids = rules.value.map((r) => r.id).filter((id) => id !== sourceId)
    const targetIndex = ids.indexOf(target.id)
    ids.splice(targetIndex, 0, sourceId)
    void settingsStore.reorderMatchRules(ids)
  }

  async function moveUp(rule: ProjectMatchRule) {
    const ids = rules.value.map((r) => r.id)
    const index = ids.indexOf(rule.id)

    if (index <= 0) {
      return
    }

    ids.splice(index - 1, 0, ids.splice(index, 1)[0])
    await settingsStore.reorderMatchRules(ids)
  }

  async function moveDown(rule: ProjectMatchRule) {
    const ids = rules.value.map((r) => r.id)
    const index = ids.indexOf(rule.id)

    if (index < 0 || index >= ids.length - 1) {
      return
    }

    ids.splice(index + 1, 0, ids.splice(index, 1)[0])
    await settingsStore.reorderMatchRules(ids)
  }
</script>

<template>
  <div class="rule-list">
    <div class="section-header">
      <h3>Rule priority</h3>
      <div class="stats-controls">
        <span class="stats-window">last {{ settingsStore.statsWindowDays }} days</span>
        <button class="btn-ghost" title="Recompute hit counts" @click="settingsStore.refreshRuleStats()">
          ↻
        </button>
      </div>
    </div>

    <p class="hint">
      Checked top to bottom across all projects — the first match wins. Drag a rule to change its
      priority.
    </p>

    <div v-if="rules.length === 0" class="empty">
      No match rules yet. Add one under a project to get activity suggestions.
    </div>

    <div v-else class="list">
      <div
        v-for="(rule, index) in rules"
        :key="rule.id"
        class="rule-row"
        :class="{ dragging: draggingId === rule.id }"
        draggable="true"
        @dragstart="onDragStart(rule)"
        @dragover.prevent
        @drop="onDrop(rule)"
        @dragend="draggingId = null"
      >
        <span class="drag-handle" title="Drag to reprioritise">⠿</span>
        <span class="rank">#{{ index + 1 }}</span>

        <span class="project-chip" :style="{ borderColor: projectColour(rule.projectId) }">
          <span class="color-dot" :style="{ background: projectColour(rule.projectId) }" />
          {{ projectName(rule.projectId) }}
        </span>

        <div class="rule-body">
          <div class="rule-name">{{ rule.name || '(unnamed rule)' }}</div>
          <code class="rule-conditions">{{ conditionSummary(rule) }}</code>
        </div>

        <div class="rule-stats">
          <template v-if="statFor(rule.id)">
            <span class="hit-count" :class="{ zero: statFor(rule.id)!.hits === 0 }">
              {{ statFor(rule.id)!.hits }} hits
            </span>
            <span class="hit-mins">{{ formatSeconds(statFor(rule.id)!.matchedSecs) }}</span>
          </template>
          <span v-else class="hit-mins">—</span>
        </div>

        <div class="rule-flags">
          <span
            v-if="statFor(rule.id) && statFor(rule.id)!.hits === 0"
            class="flag flag--warn"
            title="This rule matched nothing in the stats window — check its value"
          >
            never matches
          </span>
          <span
            v-if="overlapNames(rule.id).length > 0"
            class="flag flag--conflict"
            :title="`Also matched by: ${overlapNames(rule.id).join(', ')}`"
          >
            overlaps {{ overlapNames(rule.id).join(', ') }}
          </span>
        </div>

        <div class="row-actions">
          <template v-if="confirmingDelete === rule.id">
            <span class="confirm-text">Delete rule?</span>
            <button class="btn-ghost danger" @click="deleteRule(rule.id)">Delete</button>
            <button class="btn-ghost" @click="confirmingDelete = null">Cancel</button>
          </template>

          <template v-else>
            <button class="btn-ghost" title="Move up" @click="moveUp(rule)">↑</button>
            <button class="btn-ghost" title="Move down" @click="moveDown(rule)">↓</button>
            <button
              class="btn-ghost"
              title="Move to top"
              @click="settingsStore.promoteMatchRule(rule.id)"
            >
              ⇧
            </button>
            <button class="btn-ghost" @click="emit('edit', rule)">Edit</button>
            <button
              class="btn-ghost danger"
              title="Delete rule"
              @click="confirmingDelete = rule.id"
            >
              ×
            </button>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
  .rule-list {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--border);
  }

  .section-header h3 {
    margin: 0;
    font-size: var(--text-lg);
    font-weight: 600;
  }

  .stats-controls {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .stats-window {
    font-size: var(--text-xs);
    color: var(--text-faint);
  }

  .hint {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .empty {
    padding: var(--space-3) 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .rule-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    font-size: var(--text-sm);
  }

  .rule-row.dragging {
    opacity: 0.4;
  }

  .drag-handle {
    cursor: grab;
    color: var(--text-faint);
    flex-shrink: 0;
  }

  .rank {
    flex-shrink: 0;
    width: 28px;
    color: var(--text-faint);
    font-variant-numeric: tabular-nums;
    font-size: var(--text-xs);
  }

  .project-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
    padding: 2px var(--space-2);
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: var(--text-xs);
    white-space: nowrap;
  }

  .color-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .rule-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .rule-name {
    font-weight: 500;
  }

  .rule-conditions {
    font-family: monospace;
    font-size: var(--text-xs);
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .rule-stats {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    flex-shrink: 0;
    min-width: 64px;
    font-size: var(--text-xs);
  }

  .hit-count {
    color: var(--text);
    font-weight: 600;
  }

  .hit-count.zero {
    color: var(--warning);
  }

  .hit-mins {
    color: var(--text-faint);
  }

  .rule-flags {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    flex-shrink: 0;
  }

  .flag {
    font-size: var(--text-xs);
    padding: 1px 6px;
    border-radius: 999px;
    white-space: nowrap;
  }

  .flag--warn {
    background: color-mix(in srgb, var(--warning) 18%, transparent);
    color: var(--warning);
  }

  .flag--conflict {
    background: color-mix(in srgb, var(--danger) 16%, transparent);
    color: var(--danger);
  }

  .row-actions {
    display: flex;
    align-items: center;
    gap: 2px;
    flex-shrink: 0;
  }

  .confirm-text {
    font-size: var(--text-xs);
    color: var(--danger);
    white-space: nowrap;
  }
</style>
