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
        <button
          class="btn-primary"
          :disabled="updateStatus === 'checking'"
          @click="checkForUpdates"
        >
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
    padding: var(--space-8);
  }

  .about-card {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-2);
  }

  .app-icon {
    width: 96px;
    height: 96px;
    border-radius: 20px;
    margin-bottom: var(--space-2);
    user-select: none;
    -webkit-user-drag: none;
  }

  .app-name {
    font-size: var(--text-3xl);
    font-weight: 700;
    margin: 0;
    color: var(--text);
  }

  .app-author {
    position: relative;
    top: -8px;
    font-size: var(--text-lg);
    color: var(--text-muted);
    margin: 0;

    a {
      color: inherit;
    }
  }

  .app-version {
    font-size: var(--text-sm);
    color: var(--text);
    margin: 0;
  }

  .update-section {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-3);
    margin-top: var(--space-4);
  }

  .status-msg {
    font-size: var(--text-sm);
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
    color: var(--danger);
    max-width: 320px;
    text-align: center;
  }
</style>
