<script setup lang="ts">
  import { ref, onMounted } from 'vue'
  import { getVersion } from '@tauri-apps/api/app'
  import { check } from '@tauri-apps/plugin-updater'
  import { openUrl } from '@tauri-apps/plugin-opener'

  const RELEASES_URL = 'https://github.com/gregdev/timesheeps/releases/latest'

  const version = ref('0.0.0')
  const updateStatus = ref<'idle' | 'checking' | 'available' | 'up-to-date' | 'error'>('idle')
  const updateVersion = ref('')
  const errorMsg = ref('')

  onMounted(async () => {
    try {
      version.value = await getVersion()
    } catch {
      // Running in browser/dev mode — version already set to fallback
    }
  })

  async function checkForUpdates() {
    updateStatus.value = 'checking'
    errorMsg.value = ''
    updateVersion.value = ''

    try {
      const update = await check()

      if (update) {
        updateStatus.value = 'available'
        updateVersion.value = update.version
      } else {
        updateStatus.value = 'up-to-date'
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      updateStatus.value = 'error'
      // The updater endpoint doesn't exist yet (no published release) or we're in dev mode
      errorMsg.value =
        msg.toLowerCase().includes('json') || msg.toLowerCase().includes('fetch')
          ? 'Could not reach the update server.'
          : msg
    }
  }
</script>

<template>
  <div class="about-view">
    <div class="about-card">
      <img src="/app-icon.png" alt="Timesheeps icon" class="app-icon" />

      <h1 class="app-name">Timesheeps</h1>
      <p class="app-author">
        by
        <a href="#" @click.prevent="openUrl('https://gregsmith.au')">Greg Smith</a>
      </p>
      <p class="app-version">Version {{ version }}</p>

      <div class="update-section">
        <button class="btn-check" :disabled="updateStatus === 'checking'" @click="checkForUpdates">
          {{ updateStatus === 'checking' ? 'Checking…' : 'Check for updates' }}
        </button>

        <p v-if="updateStatus === 'up-to-date'" class="status-msg status-ok">You're up to date.</p>
        <p v-else-if="updateStatus === 'available'" class="status-msg status-new">
          Version {{ updateVersion }} is available. Download it from the
          <a href="#" @click.prevent="openUrl(RELEASES_URL)">releases page</a>
          .
        </p>
        <p v-else-if="updateStatus === 'error'" class="status-msg status-err">
          {{ errorMsg || 'Could not check for updates.' }}
          <a href="#" @click.prevent="openUrl(RELEASES_URL)">Check releases page.</a>
        </p>
      </div>
    </div>
  </div>
</template>

<style scoped>
  .about-view {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
    padding: 32px;
  }

  .about-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .app-icon {
    width: 96px;
    height: 96px;
    border-radius: 20px;
    margin-bottom: 8px;
    user-select: none;
    -webkit-user-drag: none;
  }

  .app-name {
    font-size: 22px;
    font-weight: 700;
    margin: 0;
    color: var(--text);
  }

  .app-author {
    position: relative;
    top: -8px;
    font-size: 14px;
    color: var(--text-muted);
    margin: 0;

    a {
      color: inherit;
    }
  }

  .app-version {
    font-size: 13px;
    color: var(--text);
    margin: 0;
  }

  .update-section {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    margin-top: 16px;
  }

  .btn-check {
    padding: 7px 20px;
    border-radius: var(--radius);
    background: var(--primary);
    color: #fff;
    font-size: 13px;
    font-weight: 500;
    border: none;
    cursor: pointer;
    transition: opacity 0.15s;
  }

  .btn-check:disabled {
    opacity: 0.55;
    cursor: default;
  }

  .btn-check:hover:not(:disabled) {
    opacity: 0.85;
  }

  .status-msg {
    font-size: 13px;
    margin: 0;
  }

  .status-ok {
    color: var(--text-muted);
  }

  .status-new {
    color: var(--text);
  }

  .status-new a {
    color: var(--primary);
    text-decoration: none;
  }

  .status-new a:hover {
    text-decoration: underline;
  }

  .status-err {
    color: var(--danger, #e53e3e);
    max-width: 320px;
    text-align: center;
  }
</style>
