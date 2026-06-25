<script setup lang="ts">
  import { ref, computed, onMounted, onUnmounted } from 'vue'
  import { useTimerStore } from '../stores/timer'
  import { useProjectsStore } from '../stores/projects'
  import { invoke } from '@tauri-apps/api/core'

  const timer = useTimerStore()
  const projectsStore = useProjectsStore()

  const selectedProjectId = ref<number | null>(null)
  const note = ref('')
  const view = ref<'idle' | 'running'>('idle')

  const activeProjects = computed(() => projectsStore.projects.filter((p) => !p.archivedAt))

  // Sync view with timer state
  function syncView() {
    if (timer.isActive) {
      view.value = 'running'
    } else {
      view.value = 'idle'
    }
  }

  onMounted(async () => {
    syncView()

    // Close popup on Escape
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        invoke('hide_main_window')
      }
    }
    document.addEventListener('keydown', onKey)
    onUnmounted(() => document.removeEventListener('keydown', onKey))
  })

  // Watch timer state changes from tick events
  import { watch } from 'vue'
  watch(() => timer.state.status, syncView)

  async function handleStart() {
    if (selectedProjectId.value === null) return
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

  function formatMs(ms: number): string {
    const totalSeconds = Math.floor(ms / 1000)
    const hours = Math.floor(totalSeconds / 3600)
    const minutes = Math.floor((totalSeconds % 3600) / 60)
    const seconds = totalSeconds % 60
    if (hours > 0) {
      return `${hours}:${String(minutes).padStart(2, '0')}:${String(seconds).padStart(2, '0')}`
    }
    return `${minutes}:${String(seconds).padStart(2, '0')}`
  }

  function handleDismiss() {
    invoke('hide_main_window')
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
        <button class="btn btn--sm btn--ghost" @click="handleDismiss">Cancel</button>
        <button
          class="btn btn--sm btn--primary"
          :disabled="selectedProjectId === null"
          @click="handleStart"
        >
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
        {{ formatMs(timer.state.elapsedMs) }}
      </div>

      <p v-if="timer.state.note" class="popup-note">{{ timer.state.note }}</p>

      <div class="popup-actions">
        <button v-if="timer.isRunning" class="btn btn--sm" @click="handlePause">Pause</button>
        <button v-if="timer.isPaused" class="btn btn--sm btn--primary" @click="handleResume">
          Resume
        </button>
        <button class="btn btn--sm btn--danger" @click="handleStop">Stop</button>
      </div>
    </template>
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

  .btn--ghost {
    background: transparent;
    border: 1px solid transparent;
    color: var(--text-muted);
  }

  .btn--ghost:hover {
    background: var(--surface-2);
  }
</style>
