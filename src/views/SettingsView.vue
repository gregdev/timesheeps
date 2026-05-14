<script setup lang="ts">
import { ref, watch } from 'vue'
import { useSettingsStore } from '../stores/settings'
import { useDayStore } from '../stores/day'
import ProjectList from '../components/ProjectList.vue'
import FilterRuleList from '../components/FilterRuleList.vue'
import type { Settings } from '../schemas'

const settingsStore = useSettingsStore()
const dayStore = useDayStore()

const form = ref<Settings>({ ...settingsStore.settings })
const saved = ref(false)

watch(() => settingsStore.settings, (s: Settings) => { form.value = { ...s } }, { deep: true })

async function saveSettings() {
  await settingsStore.save({ ...form.value })
  // Reload day to apply new filter/merge settings
  await dayStore.loadDay()
  saved.value = true
  setTimeout(() => { saved.value = false }, 2000)
}


</script>

<template>
  <div class="settings-view">
    <div class="settings-scroll">

      <section class="settings-section">
        <h2>Timeline</h2>
        <div class="settings-grid">
          <div class="form-group">
            <label>Timeline start</label>
            <select v-model.number="form.timelineStartHour">
              <option v-for="h in Array.from({length: 13}, (_, i) => i)" :key="h" :value="h">{{ h }}:00</option>
            </select>
          </div>
          <div class="form-group">
            <label>Timeline end</label>
            <select v-model.number="form.timelineEndHour">
              <option v-for="h in Array.from({length: 9}, (_, i) => i + 16)" :key="h" :value="h">{{ h }}:00</option>
            </select>
          </div>
        </div>
      </section>

      <section class="settings-section">
        <h2>Activity Tracking</h2>
        <div class="settings-grid">
          <div class="form-group">
            <label>Minimum duration shown</label>
            <select v-model.number="form.minDurationSecs">
              <option :value="10">10 seconds</option>
              <option :value="30">30 seconds</option>
              <option :value="60">1 minute</option>
              <option :value="120">2 minutes</option>
              <option :value="300">5 minutes</option>
            </select>
            <p class="field-hint">Events shorter than this are hidden.</p>
          </div>
          <div class="form-group">
            <label>Merge gap</label>
            <select v-model.number="form.mergeGapSecs">
              <option :value="30">30 seconds</option>
              <option :value="60">1 minute</option>
              <option :value="120">2 minutes</option>
              <option :value="300">5 minutes</option>
            </select>
            <p class="field-hint">Consecutive same-app events within this gap are merged.</p>
          </div>
          <div class="form-group">
            <label>Idle timeout</label>
            <select v-model.number="form.idleTimeoutSecs">
              <option :value="120">2 minutes</option>
              <option :value="300">5 minutes</option>
              <option :value="600">10 minutes</option>
              <option :value="900">15 minutes</option>
            </select>
            <p class="field-hint">Recording pauses after this much inactivity.</p>
          </div>
        </div>
        <div class="save-row">
          <button class="btn-primary" @click="saveSettings">Save settings</button>
          <span v-if="saved" class="saved-msg">✓ Saved</span>
        </div>
      </section>

      <section class="settings-section">
        <ProjectList />
      </section>

      <section class="settings-section">
        <FilterRuleList />
      </section>

    </div>
  </div>
</template>

<style scoped>
.settings-view {
  flex: 1;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.settings-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 20px;
  display: flex;
  flex-direction: column;
  gap: 28px;
  max-width: 680px;
}

.settings-section {
  display: flex;
  flex-direction: column;
  gap: 14px;
}

.settings-section h2 {
  font-size: 15px;
  font-weight: 600;
  padding-bottom: 10px;
  border-bottom: 1px solid var(--border);
}

.settings-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: 14px;
}

.field-hint {
  font-size: 11px;
  color: var(--text-faint);
  margin-top: 4px;
}

.save-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.saved-msg {
  font-size: 12px;
  color: #22c55e;
  font-weight: 500;
}
</style>
