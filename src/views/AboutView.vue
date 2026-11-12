<script setup lang="ts">
  import { ref, onMounted } from 'vue'
  import { getVersion } from '@tauri-apps/api/app'
  import { openUrl } from '@tauri-apps/plugin-opener'
  import { api } from '../api'

  const RELEASES_URL = 'https://github.com/gregdev/timesheeps/releases/latest'

  const version = ref('0.0.0')
  const updateStatus = ref<'idle' | 'checking' | 'available' | 'up-to-date' | 'error'>('idle')
  const updateVersion = ref('')
  const errorMsg = ref('')
  const installing = ref(false)
  const timerRunning = ref(false)

  onMounted(async () => {
    try {
      version.value = await getVersion()
    } catch {
      // Running in browser/dev mode — version already set to fallback
    }
  })

  /**
   * The same check the background loop runs, so the two cannot disagree: an idle
   * app says "up to date" while a banner offers an install only if they share one
   * code path.
   */
  async function checkForUpdates() {
    updateStatus.value = 'checking'
    errorMsg.value = ''
    updateVersion.value = ''

    try {
      const update = await api.checkForUpdates()

      if (update) {
        updateStatus.value = 'available'
        updateVersion.value = update.version
      } else {
        updateStatus.value = 'up-to-date'
      }
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)
      updateStatus.value = 'error'
      // No published release yet, offline, or running in dev mode.
      errorMsg.value =
        msg.toLowerCase().includes('json') || msg.toLowerCase().includes('fetch')
          ? 'Could not reach the update server.'
          : msg
    }
  }

  async function installUpdate(force = false) {
    installing.value = true
    errorMsg.value = ''

    try {
      await api.installUpdate(force)
      // Success restarts the process, so nothing below runs.
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e)

      if (msg.includes('TIMER_RUNNING')) {
        timerRunning.value = true
      } else {
        errorMsg.value = msg
      }

      installing.value = false
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
          class="btn-primary update-btn"
          :disabled="updateStatus === 'checking' || installing"
          @click="checkForUpdates"
        >
          {{ updateStatus === 'checking' ? 'Checking…' : 'Check for updates' }}
        </button>

        <!--
          Result area with a reserved footprint.

          The card is vertically centred in `.about-view`, so anything that
          appears below the button grows the card and re-centres it — which
          yanks the button out from under the pointer mid-click. Giving this
          region the height of its tallest common state keeps the card's size,
          and therefore its position, identical in every state.
        -->
        <div class="update-result">
          <p v-if="updateStatus === 'up-to-date'" class="status-msg status-ok">
            You're up to date.
          </p>

          <template v-else-if="updateStatus === 'available'">
            <p v-if="timerRunning" class="status-msg status-new">
              A timer is running — installing restarts the app and discards it.
            </p>
            <p v-else class="status-msg status-new">Version {{ updateVersion }} is available.</p>

            <button
              class="btn-primary update-btn"
              :disabled="installing"
              @click="installUpdate(timerRunning)"
            >
              {{
                installing ? 'Installing…' : timerRunning ? 'Restart anyway' : 'Install & restart'
              }}
            </button>

            <p v-if="errorMsg" class="status-msg status-err">{{ errorMsg }}</p>

            <a href="#" class="release-link" @click.prevent="openUrl(RELEASES_URL)">
              Or download it from the releases page
            </a>
          </template>

          <p v-else-if="updateStatus === 'error'" class="status-msg status-err">
            {{ errorMsg || 'Could not check for updates.' }}
            <a href="#" @click.prevent="openUrl(RELEASES_URL)">Check releases page.</a>
          </p>
        </div>
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

  /*
   * One width for every state, so "Check for updates" ↔ "Checking…" and
   * "Install & restart" ↔ "Installing…" do not resize the button. Width changes
   * are less distracting than height changes, but they are equally avoidable.
   */
  .update-btn {
    min-width: 172px;
  }

  /*
   * Reserved footprint for the result, sized to the tallest *reachable* state:
   * status line + Install button + install-error line + release link.
   *
   * Measured, not guessed — the error line was the state that overflowed the
   * first attempt at this. `min-height` rather than `height` so a message that
   * wraps at a narrow window grows the area instead of spilling out of it.
   */
  .update-result {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--space-3);
    min-height: 140px;
  }

  .release-link {
    font-size: var(--text-sm);
    color: var(--text-muted);
    text-decoration: none;
  }

  .release-link:hover {
    color: var(--text);
    text-decoration: underline;
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

  .status-err {
    color: var(--danger);
    max-width: 320px;
    text-align: center;
  }
</style>
