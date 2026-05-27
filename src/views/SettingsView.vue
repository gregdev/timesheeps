<script setup lang="ts">
  import { ref, watch, nextTick, onMounted } from 'vue'
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
    titleGroupApps: [...settingsStore.settings.titleGroupApps],
  })
  const newSplitApp = ref('')
  const newGroupApp = ref('')

  function addSplitApp() {
    const v = newSplitApp.value.trim()

    if (!v || form.value.titleSplitApps.some((a: string) => a.toLowerCase() === v.toLowerCase())) {
      return
    }

    form.value.titleSplitApps = [...form.value.titleSplitApps, v]
    newSplitApp.value = ''
    trackField('titleSplitApps')
  }

  function removeSplitApp(app: string) {
    form.value.titleSplitApps = form.value.titleSplitApps.filter((a) => a !== app)
    trackField('titleSplitApps')
  }

  function addGroupApp() {
    const v = newGroupApp.value.trim()

    if (!v || form.value.titleGroupApps.some((a: string) => a.toLowerCase() === v.toLowerCase())) {
      return
    }

    form.value.titleGroupApps = [...form.value.titleGroupApps, v]
    newGroupApp.value = ''
    trackField('titleGroupApps')
  }

  function removeGroupApp(app: string) {
    form.value.titleGroupApps = form.value.titleGroupApps.filter((a) => a !== app)
    trackField('titleGroupApps')
  }

  // Sync form when store changes externally
  let updatingFromStore = false
  watch(
    () => settingsStore.settings,
    async (s: Settings) => {
      updatingFromStore = true
      form.value = {
        ...s,
        titleSplitApps: [...s.titleSplitApps],
        titleGroupApps: [...s.titleGroupApps],
      }
      await nextTick()
      updatingFromStore = false
    },
    { deep: true },
  )

  // Auto-save on any form change (debounced)
  const savedField = ref<string | null>(null)
  let pendingField = ''
  let saveTimer: ReturnType<typeof setTimeout> | null = null
  let fieldTimer: ReturnType<typeof setTimeout> | null = null

  function trackField(field: string) {
    pendingField = field
  }

  function showSaved(field: string) {
    if (fieldTimer) {
      clearTimeout(fieldTimer)
    }

    savedField.value = field
    fieldTimer = setTimeout(() => {
      savedField.value = null
    }, 2000)
  }

  watch(
    form,
    async (s) => {
      if (updatingFromStore) {
        return
      }
      if (saveTimer) {
        clearTimeout(saveTimer)
      }

      saveTimer = setTimeout(async () => {
        await settingsStore.save({ ...s })
        await dayStore.loadDay()
        showSaved(pendingField)
      }, 400)
    },
    { deep: true },
  )

  watch(
    () => settingsStore.colourScheme,
    () => showSaved('colourScheme'),
  )

  type ClaudeStatus = 'idle' | 'loading' | 'success' | 'already' | 'error'
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

  onMounted(async () => {
    try {
      if (await api.checkClaudeMcp()) {
        claudeStatus.value = 'already'
      }
    } catch {
      // silently ignore — detection is best-effort
    }
  })
</script>

<template>
  <div class="settings-view">
    <div class="settings-scroll">
      <div class="settings-col">
        <section class="settings-section">
          <h2>Appearance</h2>

          <div class="settings-grid">
            <div class="form-group">
              <label>
                Colour scheme
                <Transition name="check">
                  <span v-if="savedField === 'colourScheme'" class="field-check">✓</span>
                </Transition>
              </label>

              <select v-model="settingsStore.colourScheme">
                <option value="system">System</option>
                <option value="light">Light</option>
                <option value="dark">Dark</option>
              </select>
            </div>
          </div>
        </section>

        <section class="settings-section">
          <h2>Timeline</h2>

          <div class="settings-grid">
            <div class="form-group">
              <label>
                Timeline start
                <Transition name="check">
                  <span v-if="savedField === 'timelineStartHour'" class="field-check">✓</span>
                </Transition>
              </label>

              <select
                v-model.number="form.timelineStartHour"
                @change="trackField('timelineStartHour')"
              >
                <option v-for="h in Array.from({ length: 13 }, (_, i) => i)" :key="h" :value="h">
                  {{ h }}:00
                </option>
              </select>
            </div>

            <div class="form-group">
              <label>
                Timeline end
                <Transition name="check">
                  <span v-if="savedField === 'timelineEndHour'" class="field-check">✓</span>
                </Transition>
              </label>

              <select v-model.number="form.timelineEndHour" @change="trackField('timelineEndHour')">
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
              <label>
                Week starts on
                <Transition name="check">
                  <span v-if="savedField === 'weekStartsOn'" class="field-check">✓</span>
                </Transition>
              </label>

              <select v-model.number="form.weekStartsOn" @change="trackField('weekStartsOn')">
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
              <label>
                Pay frequency
                <Transition name="check">
                  <span v-if="savedField === 'payScheduleFrequency'" class="field-check">✓</span>
                </Transition>
              </label>

              <select
                v-model="form.payScheduleFrequency"
                @change="trackField('payScheduleFrequency')"
              >
                <option value="weekly">Weekly</option>
                <option value="fortnightly">Fortnightly</option>
              </select>
            </div>

            <div class="form-group">
              <label>
                Pay period start date
                <Transition name="check">
                  <span v-if="savedField === 'payScheduleAnchor'" class="field-check">✓</span>
                </Transition>
              </label>

              <input
                v-model="form.payScheduleAnchor"
                type="date"
                @change="trackField('payScheduleAnchor')"
              />

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
              <label>
                Minimum duration shown
                <Transition name="check">
                  <span v-if="savedField === 'minDurationSecs'" class="field-check">✓</span>
                </Transition>
              </label>

              <select v-model.number="form.minDurationSecs" @change="trackField('minDurationSecs')">
                <option :value="10">10 seconds</option>
                <option :value="30">30 seconds</option>
                <option :value="60">1 minute</option>
                <option :value="120">2 minutes</option>
                <option :value="300">5 minutes</option>
              </select>

              <p class="field-hint">Events shorter than this are hidden.</p>
            </div>

            <div class="form-group">
              <label>
                Window summary minimum
                <Transition name="check">
                  <span v-if="savedField === 'windowSummaryMinSecs'" class="field-check">✓</span>
                </Transition>
              </label>

              <select
                v-model.number="form.windowSummaryMinSecs"
                @change="trackField('windowSummaryMinSecs')"
              >
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
              <label>
                Merge gap
                <Transition name="check">
                  <span v-if="savedField === 'mergeGapSecs'" class="field-check">✓</span>
                </Transition>
              </label>

              <select v-model.number="form.mergeGapSecs" @change="trackField('mergeGapSecs')">
                <option :value="30">30 seconds</option>
                <option :value="60">1 minute</option>
                <option :value="120">2 minutes</option>
                <option :value="300">5 minutes</option>
              </select>

              <p class="field-hint">Consecutive same-app events within this gap are merged.</p>
            </div>

            <div class="form-group">
              <label>
                Idle timeout
                <Transition name="check">
                  <span v-if="savedField === 'idleTimeoutSecs'" class="field-check">✓</span>
                </Transition>
              </label>

              <select v-model.number="form.idleTimeoutSecs" @change="trackField('idleTimeoutSecs')">
                <option :value="120">2 minutes</option>
                <option :value="300">5 minutes</option>
                <option :value="600">10 minutes</option>
                <option :value="900">15 minutes</option>
              </select>

              <p class="field-hint">Recording pauses after this much inactivity.</p>
            </div>

            <div class="form-group">
              <label>
                Time snap
                <Transition name="check">
                  <span v-if="savedField === 'snapMinutes'" class="field-check">✓</span>
                </Transition>
              </label>

              <select v-model.number="form.snapMinutes" @change="trackField('snapMinutes')">
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
              <input
                v-model="form.startOnLogin"
                type="checkbox"
                @change="trackField('startOnLogin')"
              />
              Start Timesheeps at login
              <Transition name="check">
                <span v-if="savedField === 'startOnLogin'" class="field-check">✓</span>
              </Transition>
            </label>
          </div>

          <div class="form-group form-group--full">
            <label>
              Split Window Activity by tab title
              <Transition name="check">
                <span v-if="savedField === 'titleSplitApps'" class="field-check">✓</span>
              </Transition>
            </label>

            <p class="field-hint">
              For these apps, each distinct window title gets its own row instead of being grouped
              by window. Useful for browsers.
            </p>

            <div class="split-apps">
              <span v-for="app in form.titleSplitApps" :key="app" class="split-chip">
                {{ app }}
                <button class="chip-remove" @click="removeSplitApp(app)">×</button>
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

          <div class="form-group form-group--full">
            <label>
              Group by project name in title
              <Transition name="check">
                <span v-if="savedField === 'titleGroupApps'" class="field-check">✓</span>
              </Transition>
            </label>

            <p class="field-hint">
              For these apps, closing and reopening the window (which gives a new window ID) still
              merges entries by extracting the project/workspace name from the title. Useful for
              IDEs like VS&nbsp;Code.
            </p>

            <div class="split-apps">
              <span v-for="app in form.titleGroupApps" :key="app" class="split-chip">
                {{ app }}
                <button class="chip-remove" @click="removeGroupApp(app)">×</button>
              </span>
              <div class="split-add">
                <input
                  v-model="newGroupApp"
                  placeholder="App name…"
                  style="width: 120px"
                  @keyup.enter="addGroupApp"
                />

                <button class="btn-primary" :disabled="!newGroupApp.trim()" @click="addGroupApp">
                  Add
                </button>
              </div>
            </div>
          </div>
        </section>

        <section class="settings-section">
          <h2>Claude AI</h2>

          <label>
            Claude Desktop app integration - ask Claude "what did I work on today?" and it will
            query your activity data directly.
          </label>

          <div class="claude-row">
            <button class="btn-primary" :disabled="claudeStatus === 'loading'" @click="setupClaude">
              {{
                claudeStatus === 'loading'
                  ? 'Configuring…'
                  : claudeStatus === 'already'
                    ? 'Reconfigure'
                    : 'Set up Claude MCP'
              }}
            </button>

            <span v-if="claudeStatus === 'already'" class="saved-msg">✓ Already configured</span>

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

  .field-check {
    display: inline-block;
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--success);
    margin-left: var(--space-1);
  }

  .check-enter-active {
    transition:
      opacity 0.15s ease,
      transform 0.15s ease;
  }

  .check-leave-active {
    transition:
      opacity 0.5s ease,
      transform 0.5s ease;
  }

  .check-enter-from,
  .check-leave-to {
    opacity: 0;
    transform: translateY(3px);
  }

  .settings-scroll {
    flex: 1;
    overflow-y: auto;
    padding: var(--space-5);
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: var(--space-12);
    align-items: start;
  }

  .settings-col {
    display: flex;
    flex-direction: column;
    gap: var(--space-7);
  }

  .settings-section {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
  }

  .settings-section--card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
    padding: var(--space-4);
  }

  .settings-section h2,
  .settings-section :deep(h2) {
    font-size: var(--text-lg);
    font-weight: 600;
    padding-bottom: var(--space-3);
    border-bottom: 1px solid var(--border);
  }

  .settings-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: var(--space-4);
  }

  .field-hint {
    font-size: var(--text-xs);
    color: var(--text-faint);
    margin-top: var(--space-1);
  }

  .form-group--inline {
    grid-column: 1 / -1;
  }

  .form-group--full {
    grid-column: 1 / -1;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .split-apps {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    align-items: center;
  }

  .split-chip {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    padding: 3px var(--space-2) 3px 10px;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: 20px;
    font-size: var(--text-xs);
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
    font-size: var(--text-sm);
    padding: 0;
    line-height: 1;
  }

  .chip-remove:hover {
    color: var(--danger);
  }

  .split-add {
    display: flex;
    gap: var(--space-2);
    align-items: center;
  }

  .checkbox-label {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-sm);
    cursor: pointer;
  }

  .checkbox-label input[type='checkbox'] {
    width: 15px;
    height: 15px;
    cursor: pointer;
  }

  .claude-row {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex-wrap: wrap;
  }

  .error-msg {
    font-size: var(--text-xs);
    color: var(--danger);
    font-weight: 500;
  }

  .saved-msg {
    font-size: var(--text-xs);
    color: var(--success);
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
