<script setup lang="ts">
  /**
   * Editor for a single rule: a named group of AND-ed conditions.
   * Conditions are combined with AND; separate rules are OR-ed together and
   * evaluated in precedence order (see MatchRuleList).
   */
  import { computed, ref, watch } from 'vue'
  import { useSettingsStore } from '../stores/settings'
  import type { MatchCondition, MatchField, MatchOperator, ProjectMatchRule } from '../schemas'
  import ConditionValueInput from './ConditionValueInput.vue'

  const props = defineProps<{
    projectId: number
    /** Existing rule to edit, or null to create a new one. */
    rule: ProjectMatchRule | null
    /** Rules already on this project, used to seed a sensible default name. */
    existingCount: number
  }>()

  const emit = defineEmits<{ (e: 'saved'): void; (e: 'cancel'): void }>()

  const settingsStore = useSettingsStore()

  const name = ref('')
  const conditions = ref<MatchCondition[]>([])
  const saving = ref(false)
  const error = ref<string | null>(null)

  const FIELD_LABELS: Record<MatchField, string> = {
    app_name: 'App name',
    window_title: 'Window title',
  }

  const OPERATOR_LABELS: Record<MatchOperator, string> = {
    contains: 'contains',
    equals: 'is exactly',
    starts_with: 'starts with',
    ends_with: 'ends with',
  }

  const operators: MatchOperator[] = ['contains', 'equals', 'starts_with', 'ends_with']

  function reset() {
    name.value = props.rule?.name ?? ''
    conditions.value = props.rule
      ? props.rule.conditions.map((c) => ({ ...c }))
      : [{ field: 'window_title', operator: 'contains', value: '', negate: false }]
    error.value = null
  }

  reset()
  watch(() => props.rule, reset)
  watch(() => props.projectId, reset)

  /** First positive app-name value, used to narrow title suggestions. */
  const appContext = computed(() => {
    const appCondition = conditions.value.find((c) => c.field === 'app_name' && !c.negate)
    return appCondition?.value.trim() || undefined
  })

  const canSave = computed(
    () => conditions.value.some((c) => c.value.trim().length > 0) && !saving.value,
  )

  function addCondition() {
    conditions.value.push({ field: 'window_title', operator: 'contains', value: '', negate: false })
  }

  function removeCondition(index: number) {
    conditions.value.splice(index, 1)
  }

  function defaultName(): string {
    const first = conditions.value.find((c) => c.value.trim())
    const base = first ? first.value.trim() : 'Rule'
    return props.existingCount > 0 ? `${base} (${props.existingCount + 1})` : base
  }

  async function save() {
    const cleaned = conditions.value
      .map((c) => ({ ...c, value: c.value.trim() }))
      .filter((c) => c.value.length > 0)

    if (cleaned.length === 0) {
      error.value = 'Add at least one condition with a value.'
      return
    }

    saving.value = true
    error.value = null

    try {
      const finalName = name.value.trim() || defaultName()

      if (props.rule) {
        await settingsStore.updateMatchRule(props.rule.id, finalName, cleaned)
      } else {
        await settingsStore.createMatchRule(props.projectId, finalName, cleaned)
      }

      emit('saved')
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      saving.value = false
    }
  }
</script>

<template>
  <div class="rule-editor">
    <div class="editor-header">
      <input
        v-model="name"
        class="name-input"
        placeholder="Rule name (optional)"
        spellcheck="false"
      />
    </div>

    <div
      v-for="(condition, index) in conditions"
      :key="index"
      class="condition-row"
    >
      <span v-if="index > 0" class="and-badge">AND</span>
      <span v-else class="and-badge and-badge--spacer" />

      <select v-model="condition.field" class="field-select">
        <option value="app_name">{{ FIELD_LABELS.app_name }}</option>
        <option value="window_title">{{ FIELD_LABELS.window_title }}</option>
      </select>

      <select v-model="condition.operator" class="op-select">
        <option v-for="op in operators" :key="op" :value="op">{{ OPERATOR_LABELS[op] }}</option>
      </select>

      <ConditionValueInput
        v-model="condition.value"
        :field="condition.field"
        :app-name="appContext"
        placeholder="e.g. Jira"
      />

      <label class="negate-toggle" :title="condition.negate ? 'Condition is inverted' : 'Invert condition'">
        <input v-model="condition.negate" type="checkbox" />
        <span>is not</span>
      </label>

      <button
        class="btn-ghost danger sm"
        :disabled="conditions.length === 1"
        title="Remove condition"
        @click="removeCondition(index)"
      >
        ×
      </button>
    </div>

    <p class="hint">
      All conditions must match (AND). Separate rules are checked top-down —
      the <strong>first</strong> match wins.
    </p>

    <div v-if="error" class="error">{{ error }}</div>

    <div class="editor-actions">
      <button class="btn-secondary" @click="addCondition">+ Condition</button>
      <div class="spacer" />
      <button class="btn-secondary" @click="emit('cancel')">Cancel</button>
      <button class="btn-primary" :disabled="!canSave" @click="save">
        {{ rule ? 'Save rule' : 'Add rule' }}
      </button>
    </div>
  </div>
</template>

<style scoped>
  .rule-editor {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .editor-header {
    display: flex;
    gap: var(--space-2);
  }

  .name-input {
    font-weight: 500;
  }

  .condition-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
  }

  .and-badge {
    flex-shrink: 0;
    width: 34px;
    text-align: center;
    font-size: var(--text-xs);
    font-weight: 700;
    color: var(--text-faint);
    letter-spacing: 0.04em;
  }

  .and-badge--spacer {
    visibility: hidden;
  }

  .field-select {
    width: 120px;
    flex-shrink: 0;
  }

  .op-select {
    width: 118px;
    flex-shrink: 0;
  }

  .negate-toggle {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
    cursor: pointer;
    white-space: nowrap;
  }

  .negate-toggle input {
    width: auto;
    margin: 0;
  }

  .hint {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .error {
    font-size: var(--text-xs);
    color: var(--danger);
  }

  .editor-actions {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin-top: var(--space-1);
  }

  .spacer {
    flex: 1;
  }

  .sm {
    padding: 0 var(--space-2);
    font-size: var(--text-base);
    line-height: 1.2;
  }
</style>
