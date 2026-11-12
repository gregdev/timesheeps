<script setup lang="ts">
  import { onMounted, onUnmounted, ref } from 'vue'
  import { listen } from '@tauri-apps/api/event'
  import { api } from '../api'
  import type { UpdateInfo } from '../schemas'

  /**
   * "A new version is available" strip.
   *
   * The backend checks a few times a day and emits `update-available`; this only
   * ever reports. Installing is the user's click, because a restart throws away
   * an in-flight stopwatch (`TimerManager` is memory-only) and this app is left
   * running for weeks at a time.
   *
   * A dismissal is remembered per version in localStorage, so a banner that has
   * been waved away does not reappear on the next six-hourly check — but a
   * *newer* release still surfaces.
   */

  const DISMISSED_KEY = 'updateDismissedVersion'

  const update = ref<UpdateInfo | null>(null)
  const installing = ref(false)
  const timerRunning = ref(false)
  const error = ref('')

  function dismissedVersion(): string {
    return localStorage.getItem(DISMISSED_KEY) ?? ''
  }

  function offer(info: UpdateInfo | null) {
    if (!info || info.version === dismissedVersion()) {
      return
    }

    // Never replace an offer the user is already looking at with the same one.
    if (update.value?.version === info.version) {
      return
    }

    update.value = info
    timerRunning.value = false
    error.value = ''
  }

  function dismiss() {
    if (update.value) {
      localStorage.setItem(DISMISSED_KEY, update.value.version)
    }

    update.value = null
    timerRunning.value = false
  }

  async function install(force = false) {
    installing.value = true
    error.value = ''

    try {
      await api.installUpdate(force)
      // On success the process restarts and this promise never settles; anything
      // below is the failure path.
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)

      if (msg.includes('TIMER_RUNNING')) {
        // Not an error — the app is asking whether to sacrifice the stopwatch.
        timerRunning.value = true
      } else {
        error.value = msg
      }

      installing.value = false
    }
  }

  let unlisten: (() => void) | null = null

  onMounted(async () => {
    // Read the last known state first: the background check may have run before
    // this view existed, in which case the event has already been and gone.
    api.getUpdateInfo().then(offer).catch(() => {})

    try {
      unlisten = await listen<UpdateInfo>('update-available', (event) => offer(event.payload))
    } catch {
      // Not running under Tauri (plain `pnpm dev`) — nothing to listen to.
    }
  })

  onUnmounted(() => unlisten?.())
</script>

<template>
  <div v-if="update" class="update-banner">
    <span class="ub-icon">↑</span>

    <template v-if="timerRunning">
      <span class="ub-text">
        A timer is running. Installing restarts the app and discards it.
      </span>
      <button class="ub-btn ub-btn--primary" :disabled="installing" @click="install(true)">
        {{ installing ? 'Installing…' : 'Restart anyway' }}
      </button>
      <button class="ub-btn" :disabled="installing" @click="timerRunning = false">
        Stop the timer first
      </button>
    </template>

    <template v-else>
      <span class="ub-text">
        Version <strong>{{ update.version }}</strong> is available
        <span class="ub-from">(you have {{ update.currentVersion }})</span>.
        <span v-if="error" class="ub-error">{{ error }}</span>
      </span>
      <button class="ub-btn ub-btn--primary" :disabled="installing" @click="install()">
        {{ installing ? 'Installing…' : 'Install & restart' }}
      </button>
    </template>

    <button class="ub-close" :disabled="installing" title="Not now" @click="dismiss">✕</button>
  </div>
</template>

<style scoped>
  .update-banner {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-3);
    font-size: var(--text-xs);
    color: var(--text);
    background: color-mix(in srgb, var(--primary) 12%, var(--surface));
    border-bottom: 1px solid color-mix(in srgb, var(--primary) 35%, var(--border));
    flex-shrink: 0;
  }

  .ub-icon {
    color: var(--primary);
    font-weight: 700;
  }

  .ub-text {
    flex: 1;
    min-width: 0;
  }

  .ub-from {
    color: var(--text-muted);
  }

  .ub-error {
    color: var(--danger, #ef4444);
  }

  .ub-btn {
    font-size: var(--text-xs);
    padding: 2px var(--space-2);
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text);
    cursor: pointer;
    white-space: nowrap;
  }

  .ub-btn--primary {
    border-color: var(--primary);
    background: var(--primary);
    color: #fff;
    font-weight: 600;
  }

  .ub-close {
    font-size: var(--text-xs);
    padding: 2px 4px;
    border: none;
    background: none;
    color: var(--text-muted);
    cursor: pointer;
  }

  .ub-btn:disabled,
  .ub-close:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .ub-close:hover:not(:disabled) {
    color: var(--text);
  }
</style>
