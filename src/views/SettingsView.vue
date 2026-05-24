<script setup lang="ts">
  import { ref, watch } from 'vue'
  import { useSettingsStore } from '../stores/settings'
  import { useDayStore } from '../stores/day'
  import ProjectList from '../components/ProjectList.vue'
  import FilterRuleList from '../components/FilterRuleList.vue'
  import type { Settings } from '../schemas'
  import { api } from '../api'

  const settingsStore = useSettingsStore()
  const dayStore = useDayStore()

  const form = ref<Settings>({
    ...settingsStore.settings,
    titleSplitApps: [...settingsStore.settings.titleSplitApps],
  })
  const saved = ref(false)
  const newSplitApp = ref('')

  function addSplitApp() {
    const v = newSplitApp.value.trim()

    if (!v || form.value.titleSplitApps.some((a: string) => a.toLowerCase() === v.toLowerCase())) {
      return
    }

    form.value.titleSplitApps = [...form.value.titleSplitApps, v]
    newSplitApp.value = ''
  }

  watch(
    () => settingsStore.settings,
    (s: Settings) => {
      form.value = { ...s, titleSplitApps: [...s.titleSplitApps] }
    },
    { deep: true },
  )

  async function saveSettings() {
    await settingsStore.save({ ...form.value })
    // Reload day to apply new filter/merge settings
    await dayStore.loadDay()
    saved.value = true
    setTimeout(() => {
      saved.value = false
    }, 2000)
  }

  type ClaudeStatus = 'idle' | 'loading' | 'success' | 'error'
  const claudeStatus = ref<ClaudeStatus>('idle')
  const claudeError = ref('')

  async function setupClaude() {
    claudeStatus.value = 'loading'
    claudeError.value = ''

    try {
      await api.setupClaudeMcp()
      claudeStatus.value = 'success'
    } catch (e: unknown) {
      claudeStatus.value = 'error'
      claudeError.value = e instanceof Error ? e.message : String(e)
    }
  }
</script>

<template>
  <div class="settings-view">
    <div class="settings-scroll">
      <div class="settings-col">
        <section class="settings-section">
          <h2>Timeline</h2>

          <div class="settings-grid">
            <div class="form-group">
              <label>Timeline start</label>

              <select v-model.number="form.timelineStartHour">
                <option v-for="h in Array.from({ length: 13 }, (_, i) => i)" :key="h" :value="h">
                  {{ h }}:00
                </option>
              </select>
            </div>

            <div class="form-group">
              <label>Timeline end</label>

              <select v-model.number="form.timelineEndHour">
                <option
                  v-for="h in Array.from({ length: 9 }, (_, i) => i + 16)"
                  :key="h"
                  :value="h"
                >
                  {{ h }}:00
                </option>
              </select>
            </div>

            <div class="form-group">
              <label>Week starts on</label>

              <select v-model.number="form.weekStartsOn">
                <option :value="1">Monday</option>
                <option :value="0">Sunday</option>
              </select>
            </div>
          </div>
        </section>

        <section class="settings-section">
          <h2>Pay Schedule</h2>
          <div class="settings-grid">
            <div class="form-group">
              <label>Pay frequency</label>
              <select v-model="form.payScheduleFrequency">
                <option value="weekly">Weekly</option>
                <option value="fortnightly">Fortnightly</option>
              </select>
            </div>
            <div class="form-group">
              <label>Pay period start date</label>
              <input v-model="form.payScheduleAnchor" type="date" />
              <p class="field-hint">
                A known pay period start date — used to calculate all past and future periods.
              </p>
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
              <label>Window summary minimum</label>

              <select v-model.number="form.windowSummaryMinSecs">
                <option :value="30">30 seconds</option>
                <option :value="60">1 minute</option>
                <option :value="120">2 minutes</option>
                <option :value="300">5 minutes</option>
                <option :value="600">10 minutes</option>
              </select>

              <p class="field-hint">
                Windows with less total time than this are hidden from the activity summary panel.
              </p>
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
            <div class="form-group">
              <label>Time snap</label>
              <select v-model.number="form.snapMinutes">
                <option :value="1">1 minute</option>
                <option :value="5">5 minutes</option>
                <option :value="10">10 minutes</option>
                <option :value="15">15 minutes</option>
              </select>
              <p class="field-hint">Rounding increment for dragging and hover tooltip.</p>
            </div>
          </div>
          <div class="form-group form-group--inline">
            <label class="checkbox-label">
              <input v-model="form.startOnLogin" type="checkbox" />
              Start Timesheeps when Windows starts
            </label>
          </div>

          <div class="form-group form-group--full">
            <label>Split Window Activity by tab title</label>
            <p class="field-hint">
              For these apps, each distinct window title gets its own row instead of being grouped
              by window. Useful for browsers.
            </p>
            <div class="split-apps">
              <span v-for="app in form.titleSplitApps" :key="app" class="split-chip">
                {{ app }}
                <button
                  class="chip-remove"
                  @click="form.titleSplitApps = form.titleSplitApps.filter((a) => a !== app)"
                >
                  ×
                </button>
              </span>
              <div class="split-add">
                <input
                  v-model="newSplitApp"
                  placeholder="App name…"
                  style="width: 120px"
                  @keyup.enter="addSplitApp"
                />
                <button class="btn-primary" :disabled="!newSplitApp.trim()" @click="addSplitApp">
                  Add
                </button>
              </div>
            </div>
          </div>

          <div class="save-row">
            <button class="btn-primary" @click="saveSettings">Save settings</button>
            <span v-if="saved" class="saved-msg">✓ Saved</span>
          </div>
        </section>

        <section class="settings-section">
          <h2>Claude AI</h2>
          <p class="field-hint">
            Ask Claude "what did I work on today?" and it will query your activity data directly.
          </p>
          <div class="claude-row">
            <button class="btn-primary" :disabled="claudeStatus === 'loading'" @click="setupClaude">
              {{ claudeStatus === 'loading' ? 'Configuring…' : 'Set up Claude MCP' }}
            </button>
            <span v-if="claudeStatus === 'success'" class="saved-msg">
              ✓ Configured — restart Claude Desktop to apply
            </span>
            <span v-if="claudeStatus === 'error'" class="error-msg">{{ claudeError }}</span>
          </div>
          <p class="field-hint" style="margin-top: 0.5rem">
            Also enable developer mode in Claude:
            <strong>Help → Troubleshoot → Enable Developer Mode</strong>
          </p>
        </section>
      </div>

      <section class="settings-section settings-section--card">
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
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 48px;
    align-items: start;
  }

  .settings-col {
    display: flex;
    flex-direction: column;
    gap: 28px;
  }

  .settings-section {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .settings-section--card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: 16px;
  }

  .settings-section h2 {
    font-size: 18px;
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
    font-size: 12px;
    color: var(--text-faint);
    margin-top: 4px;
  }

  .form-group--inline {
    grid-column: 1 / -1;
  }

  .form-group--full {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .split-apps {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }

  .split-chip {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    padding: 3px 8px 3px 10px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 20px;
    font-size: 12px;
  }

  .chip-remove {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 14px;
    height: 14px;
    border: none;
    background: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: 14px;
    padding: 0;
    line-height: 1;
  }

  .chip-remove:hover {
    color: var(--danger);
  }

  .split-add {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .checkbox-label {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    cursor: pointer;
  }

  .checkbox-label input[type='checkbox'] {
    width: 15px;
    height: 15px;
    cursor: pointer;
  }

  .save-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .claude-row {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-wrap: wrap;
  }

  .error-msg {
    font-size: 12px;
    color: var(--danger);
    font-weight: 500;
  }

  .saved-msg {
    font-size: 12px;
    color: #22c55e;
    font-weight: 500;
    animation: fade-in 0.2s ease;
  }

  @keyframes fade-in {
    from {
      opacity: 0;
      transform: translateY(2px);
    }

    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
</style>
