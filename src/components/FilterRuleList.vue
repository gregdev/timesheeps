<script setup lang="ts">
import { ref } from 'vue'
import { useSettingsStore } from '../stores/settings'
import type { FilterRuleType } from '../schemas'

const store = useSettingsStore()

const newType = ref<FilterRuleType>('title_pattern')
const newValue = ref('')

async function addRule() {
  if (!newValue.value.trim()) return
  await store.createRule(newType.value, newValue.value.trim())
  newValue.value = ''
}
</script>

<template>
  <div class="filter-rules">
    <div class="section-header">
      <h3>Ignore Rules</h3>
    </div>
    <p class="hint">Activity matching these rules is hidden from the timeline.</p>

    <!-- Add rule form -->
    <div class="add-form">
      <select v-model="newType" style="width: 140px; flex-shrink: 0">
        <option value="title_pattern">Title contains</option>
        <option value="app_name">App name is</option>
      </select>
      <input v-model="newValue" placeholder="e.g. login" @keyup.enter="addRule" />
      <button class="btn-primary" @click="addRule" :disabled="!newValue.trim()">Add</button>
    </div>

    <!-- Rules list -->
    <div class="list">
      <div v-if="store.filterRules.length === 0" class="empty">No ignore rules. All activity is shown.</div>
      <div v-for="rule in store.filterRules" :key="rule.id" class="rule-row">
        <span class="rule-type">
          {{ rule.ruleType === 'title_pattern' ? 'Title contains' : 'App name is' }}
        </span>
        <code class="rule-value">{{ rule.value }}</code>
        <button class="btn-ghost danger" @click="store.deleteRule(rule.id)">Remove</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.filter-rules { display: flex; flex-direction: column; gap: 12px; }
.section-header h3 { font-size: 14px; font-weight: 600; }
.hint { font-size: 12px; color: var(--text-muted); margin-top: -6px; }

.add-form { display: flex; gap: 8px; align-items: center; }

.list { display: flex; flex-direction: column; gap: 4px; }
.empty { font-size: 13px; color: var(--text-muted); }

.rule-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 10px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: var(--radius);
}

.rule-type { font-size: 12px; color: var(--text-muted); flex-shrink: 0; }
.rule-value {
  flex: 1;
  font-family: monospace;
  font-size: 12px;
  background: var(--surface-2);
  padding: 2px 6px;
  border-radius: 4px;
}

.btn-ghost.danger { color: var(--danger); }
.btn-ghost.danger:hover { background: color-mix(in srgb, var(--danger) 10%, transparent); }
</style>
