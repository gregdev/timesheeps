<script setup lang="ts">
  import { ref, onMounted } from 'vue'
  import { api } from '../api'

  const show = ref(false)
  const requested = ref(false)

  onMounted(async () => {
    try {
      const granted = await api.checkScreenRecordingPermission()
      show.value = !granted
    } catch {
      // Not running inside Tauri (e.g. pnpm dev) or not on macOS — hide banner
    }
  })

  async function requestPermission() {
    await api.requestScreenRecordingPermission()
    requested.value = true
  }
</script>

<template>
  <div v-if="show" class="permission-banner">
    <span class="banner-icon">⚠</span>

    <span class="banner-text">
      <template v-if="!requested">
        Timesheeps needs
        <strong>Screen Recording</strong>
        permission to track active windows.
        <button class="banner-btn" @click="requestPermission">Grant permission</button>
      </template>
      <template v-else>
        Open
        <strong>System Settings → Privacy &amp; Security → Screen Recording</strong>
        and enable Timesheeps, then restart the app.
      </template>
    </span>

    <button class="banner-dismiss" title="Dismiss" @click="show = false">×</button>
  </div>
</template>

<style scoped>
  .permission-banner {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-2) var(--space-4);
    background: color-mix(in srgb, var(--warning, #f59e0b) 15%, transparent);
    border-bottom: 1px solid color-mix(in srgb, var(--warning, #f59e0b) 40%, transparent);
    font-size: var(--text-sm);
    flex-shrink: 0;
  }

  .banner-icon {
    color: var(--warning, #f59e0b);
    flex-shrink: 0;
  }

  .banner-text {
    flex: 1;
    color: var(--text);
  }

  .banner-btn {
    margin-left: var(--space-2);
    padding: 2px var(--space-3);
    background: var(--warning, #f59e0b);
    color: #000;
    border: none;
    border-radius: var(--radius);
    font-size: var(--text-xs);
    font-weight: 600;
    cursor: pointer;
  }

  .banner-btn:hover {
    opacity: 0.85;
  }

  .banner-dismiss {
    flex-shrink: 0;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    font-size: var(--text-lg);
    line-height: 1;
    padding: 0 var(--space-1);
  }

  .banner-dismiss:hover {
    color: var(--text);
  }
</style>
