<script setup lang="ts">
  import { ref, computed, onMounted, onUnmounted } from 'vue'
  import { useTimerStore } from '../stores/timer'
  import { useProjectsStore } from '../stores/projects'

  const timer = useTimerStore()
  const projectsStore = useProjectsStore()

  const expanded = ref(false)
  const selectedProjectId = ref<number | null>(null)
  const note = ref('')

  // Filter to active (non-archived) projects for the dropdown
  const activeProjects = computed(() => projectsStore.projects.filter((p) => !p.archivedAt))

  function toggleExpand() {
    if (timer.isActive) {
      // If already running, just toggle
      expanded.value = !expanded.value
    } else {
      expanded.value = !expanded.value
    }
  }

  async function handleStart() {
    if (selectedProjectId.value === null) {
      return
    }

    await timer.start(selectedProjectId.value, note.value.trim())
    note.value = ''
    expanded.value = false
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

  function formatMs(ms: number): string {
    const totalSeconds = Math.floor(ms / 1000)
    const hours = Math.floor(totalSeconds / 3600)
    const minutes = Math.floor((totalSeconds % 3600) / 60)
    const seconds = totalSeconds % 60

    if (hours > 0) {
      return `${hours}h ${String(minutes).padStart(2, '0')}m`
    }

    return `${minutes}:${String(seconds).padStart(2, '0')}`
  }

  // Close expanded panel on Escape
  function onKeyDown(e: KeyboardEvent) {
    if (e.key === 'Escape' && expanded.value) {
      expanded.value = false
    }
  }

  onMounted(() => {
    document.addEventListener('keydown', onKeyDown)
  })

  onUnmounted(() => {
    document.removeEventListener('keydown', onKeyDown)
  })
</script>

<template>
  <div
    class="timer-widget"
    :class="{
      'timer--running': timer.isRunning,
      'timer--paused': timer.isPaused,
      'timer--expanded': expanded,
    }"
  >
    <!-- Collapsed display -->
    <button
      class="timer-btn"
      :title="
        timer.isActive
          ? `${timer.elapsedFormatted} — ${timer.state.projectName || 'No project'}`
          : 'Start timer'
      "
      @click="toggleExpand"
    >
      <span v-if="timer.isRunning" class="timer-dot timer-dot--running" />
      <span v-else-if="timer.isPaused" class="timer-dot timer-dot--paused" />

      <template v-if="timer.isActive">
        <span class="timer-elapsed">{{ formatMs(timer.state.elapsedMs) }}</span>
        <span v-if="timer.state.projectName" class="timer-project-label">
          {{ timer.state.projectName }}
        </span>
      </template>
      <template v-else>
        <span class="timer-label">Timer</span>
      </template>
    </button>

    <!-- Expanded panel -->
    <div v-if="expanded && !timer.isActive" class="timer-panel">
      <div class="timer-panel-header">Start Timer</div>

      <select v-model="selectedProjectId" class="timer-project-select">
        <option :value="null" disabled>Select project…</option>
        <option v-for="p in activeProjects" :key="p.id" :value="p.id">
          {{ p.name }}
        </option>
      </select>

      <input
        v-model="note"
        type="text"
        class="timer-note-input"
        placeholder="Note (optional)"
        @keydown.enter="handleStart"
      />

      <div class="timer-panel-actions">
        <button class="btn btn--sm" @click="expanded = false">Cancel</button>
        <button
          class="btn btn--sm btn--primary"
          :disabled="selectedProjectId === null"
          @click="handleStart"
        >
          Start
        </button>
      </div>
    </div>

    <!-- Expanded running panel -->
    <div v-if="expanded && timer.isRunning" class="timer-panel timer-panel--running">
      <div class="timer-panel-header">
        <span class="timer-panel-project">
          {{ timer.state.projectName || 'No project' }}
        </span>
        <span class="timer-panel-time">{{ timer.elapsedFormatted }}</span>
      </div>
      <p v-if="timer.state.note" class="timer-panel-note">{{ timer.state.note }}</p>
      <div class="timer-panel-actions">
        <button class="btn btn--sm" @click="handlePause">Pause</button>
        <button class="btn btn--sm btn--danger" @click="handleStop">Stop</button>
      </div>
    </div>

    <!-- Expanded paused panel -->
    <div v-if="expanded && timer.isPaused" class="timer-panel timer-panel--paused">
      <div class="timer-panel-header">
        <span class="timer-panel-project">
          {{ timer.state.projectName || 'No project' }}
        </span>
        <span class="timer-panel-time timer-panel-time--paused">
          {{ timer.elapsedFormatted }}
        </span>
      </div>
      <p class="timer-panel-status">Paused</p>
      <div class="timer-panel-actions">
        <button class="btn btn--sm" @click="handleStop">Stop</button>
        <button class="btn btn--sm btn--primary" @click="handleResume">Resume</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
  .timer-widget {
    position: relative;
    margin-left: auto;
    display: flex;
    align-items: center;
  }

  .timer-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text);
    font-family: var(--font);
    font-size: var(--text-sm);
    cursor: pointer;
    white-space: nowrap;
    transition:
      border-color 0.15s,
      box-shadow 0.15s;
  }

  .timer-btn:hover {
    border-color: var(--border-strong);
  }

  .timer--running .timer-btn {
    border-color: var(--success);
    box-shadow: 0 0 0 1px var(--success);
  }

  .timer--paused .timer-btn {
    border-color: var(--warning);
  }

  .timer-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .timer-dot--running {
    background: var(--success);
    animation: timer-pulse 1.5s ease-in-out infinite;
  }

  .timer-dot--paused {
    background: var(--warning);
  }

  @keyframes timer-pulse {
    0%,
    100% {
      opacity: 1;
    }

    50% {
      opacity: 0.4;
    }
  }

  .timer-elapsed {
    font-variant-numeric: tabular-nums;
    font-weight: 700;
    min-width: 48px;
  }

  .timer-project-label {
    color: var(--text-muted);
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .timer-label {
    color: var(--text-muted);
  }

  /* ── Panel ────────────────────────────────────────────────────────────── */

  .timer-panel {
    position: absolute;
    top: calc(100% + 6px);
    right: 0;
    width: 260px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow-md);
    padding: var(--space-3);
    z-index: 100;
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }

  .timer-panel-header {
    font-size: var(--text-sm);
    font-weight: 700;
    color: var(--text);
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .timer-panel--running .timer-panel-header {
    color: var(--success);
  }

  .timer-panel--paused .timer-panel-header {
    color: var(--warning);
  }

  .timer-panel-time {
    font-variant-numeric: tabular-nums;
    font-size: var(--text-lg);
    font-weight: 800;
  }

  .timer-panel-time--paused {
    color: var(--warning);
  }

  .timer-panel-project {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .timer-panel-note {
    font-size: var(--text-xs);
    color: var(--text-muted);
    margin: 0;
    font-style: italic;
  }

  .timer-panel-status {
    font-size: var(--text-xs);
    color: var(--warning);
    margin: 0;
    font-weight: 600;
  }

  .timer-project-select,
  .timer-note-input {
    width: 100%;
    padding: 4px 8px;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: var(--bg);
    color: var(--text);
    font-family: var(--font);
    font-size: var(--text-sm);
  }

  .timer-project-select:focus,
  .timer-note-input:focus {
    outline: none;
    border-color: var(--primary);
  }

  .timer-panel-actions {
    display: flex;
    gap: var(--space-2);
    justify-content: flex-end;
  }
</style>
