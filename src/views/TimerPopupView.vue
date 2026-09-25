<script setup lang="ts">
  import { ref, computed, onMounted, onUnmounted } from 'vue'
  import { invoke } from '@tauri-apps/api/core'
  import { useTimerStore } from '../stores/timer'
  import { useProjectsStore } from '../stores/projects'
  import { formatElapsed } from '../composables/useFormat'

  const timer = useTimerStore()
  const projectsStore = useProjectsStore()

  const selectedProjectId = ref<number | null>(null)
  const note = ref('')

  const activeProjects = computed(() => projectsStore.projects.filter((p) => !p.archivedAt))

  // Close popup on Escape
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      handleDismiss()
    }
  }

  function handleDismiss() {
    void invoke('hide_main_window')
  }

  function handleOpenApp() {
    void invoke('show_main_window')
  }

  onMounted(() => document.addEventListener('keydown', onKey))
  onUnmounted(() => document.removeEventListener('keydown', onKey))

  async function handleStart() {
    if (selectedProjectId.value === null) {
      return
    }

    await timer.start(selectedProjectId.value, note.value.trim())
    note.value = ''
  }

  async function handlePause() {
    await timer.pause()
  }

  async function handleResume() {
    await timer.resume()
  }

  async function handleStop() {
    await timer.stop()
    note.value = ''
  }
</script>

<template>
  <div class="popup-shell">
    <button class="popup-close" title="Close (Esc)" @click="handleDismiss">&times;</button>
    <!-- Idle state: pick project + start -->
    <template v-if="!timer.isActive">
      <div class="popup-header">Start Timer</div>

      <select
        v-model="selectedProjectId"
        class="popup-select"
        autofocus
        @keydown.enter="handleStart"
        @keydown.escape="handleDismiss"
      >
        <option :value="null" disabled>Select project…</option>
        <option v-for="p in activeProjects" :key="p.id" :value="p.id">
          {{ p.name }}
        </option>
      </select>

      <input
        v-model="note"
        type="text"
        class="popup-input"
        placeholder="Note (optional)"
        @keydown.enter="handleStart"
        @keydown.escape="handleDismiss"
      />

      <div class="popup-actions">
        <button class="btn-ghost sm" @click="handleDismiss">Cancel</button>
        <button class="btn-primary sm" :disabled="selectedProjectId === null" @click="handleStart">
          Start
        </button>
      </div>
    </template>

    <!-- Running/paused state -->
    <template v-else>
      <div class="popup-header popup-header--active">
        <span
          class="popup-project-dot"
          :style="{ background: timer.state.projectColor || '#6366f1' }"
        />
        <span class="popup-project-name">
          {{ timer.state.projectName || 'Untracked' }}
        </span>
      </div>

      <div class="popup-time" :class="{ 'popup-time--paused': timer.isPaused }">
        {{ formatElapsed(timer.state.elapsedMs) }}
      </div>

      <p v-if="timer.state.note" class="popup-note">{{ timer.state.note }}</p>

      <div class="popup-actions">
        <button v-if="timer.isRunning" class="btn-secondary sm" @click="handlePause">Pause</button>
        <button v-if="timer.isPaused" class="btn-primary sm" @click="handleResume">Resume</button>
        <button class="btn-danger sm" @click="handleStop">Stop</button>
      </div>
    </template>

    <!-- Always available: leave the popup for the full app window -->
    <button class="popup-open-app" title="Open Timesheeps" @click="handleOpenApp">
      <svg
        width="12"
        height="12"
        viewBox="0 0 16 16"
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
        stroke-linejoin="round"
        aria-hidden="true"
      >
        <rect x="1.5" y="2.5" width="13" height="11" rx="1.5" />
        <path d="M1.5 6h13" />
      </svg>
      <span>Open full app</span>
    </button>
  </div>
</template>

<style scoped>
  .popup-shell {
    position: relative;
    padding: 16px;
    user-select: none;
    display: flex;
    flex-direction: column;
    gap: 10px;
    font-family: var(--font);
    font-size: var(--text-sm);
    color: var(--text);
    background: var(--surface);
    border-radius: 8px;
    min-width: 240px;
  }

  .popup-close {
    position: absolute;
    top: 6px;
    right: 8px;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    border: none;
    background: transparent;
    color: var(--text-muted);
    font-size: 18px;
    line-height: 1;
    cursor: pointer;
    border-radius: 4px;
  }

  .popup-close:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  .popup-header {
    font-size: var(--text-xs);
    font-weight: 700;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .popup-header--active {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text);
    text-transform: none;
    letter-spacing: 0;
    font-size: var(--text-sm);
  }

  .popup-project-dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .popup-project-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .popup-time {
    font-size: 2rem;
    font-weight: 800;
    font-variant-numeric: tabular-nums;
    letter-spacing: -1px;
    color: var(--success);
    text-align: center;
    line-height: 1;
  }

  .popup-time--paused {
    color: var(--warning);
  }

  .popup-note {
    font-size: var(--text-xs);
    color: var(--text-muted);
    margin: 0;
    font-style: italic;
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .popup-select,
  .popup-input {
    width: 100%;
    padding: 6px 10px;
    border: 1px solid var(--border);
    border-radius: 6px;
    background: var(--bg);
    color: var(--text);
    font-family: var(--font);
    font-size: var(--text-sm);
  }

  .popup-select:focus,
  .popup-input:focus {
    outline: none;
    border-color: var(--primary);
    box-shadow: 0 0 0 2px rgb(122 158 128 / 25%);
  }

  .popup-actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
  }

  .popup-open-app {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    width: 100%;
    padding: var(--space-1) var(--space-3);
    border: 1px solid var(--border);
    border-top-color: var(--border);
    border-radius: 6px;
    background: transparent;
    color: var(--text-muted);
    font-family: var(--font);
    font-size: var(--text-xs);
    cursor: pointer;
  }

  .popup-open-app:hover {
    background: var(--surface-2);
    color: var(--text);
  }

  /* Compact actions — the global button styles are sized for page forms. */
  .sm {
    padding: var(--space-1) var(--space-3);
    font-size: var(--text-xs);
  }
</style>
