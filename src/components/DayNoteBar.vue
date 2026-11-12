<script setup lang="ts">
  import { ref, watch, onUnmounted } from 'vue'
  import { useDayStore } from '../stores/day'

  /**
   * A single line for "what did I actually do today".
   *
   * Window titles record which app was in front; they never record what the
   * work was. Two weeks later that distinction is the whole problem, so the
   * note is deliberately always visible and one keystroke away rather than
   * hidden behind a menu.
   *
   * Saves are debounced while typing and flushed on blur. The date the edit
   * started on is captured, so switching days mid-sentence cannot write the
   * text onto the wrong day.
   */

  const dayStore = useDayStore()
  const local = ref(dayStore.dayNote)

  let timer: ReturnType<typeof setTimeout> | null = null
  let pendingDate: string | null = null

  watch(
    () => dayStore.dayNote,
    (note) => {
      // Only pull from the store when there is nothing pending locally, or the
      // in-flight keystrokes would be overwritten by a background refresh.
      if (pendingDate === null && note !== local.value) {
        local.value = note
      }
    },
  )

  watch(
    () => dayStore.selectedDate,
    async () => {
      await flush()
      local.value = dayStore.dayNote
    },
  )

  async function flush(): Promise<void> {
    if (timer) {
      clearTimeout(timer)
      timer = null
    }
    if (pendingDate === null) {
      return
    }

    const date = pendingDate
    pendingDate = null
    await dayStore.saveDayNote(local.value, date)
  }

  function onInput() {
    pendingDate = dayStore.selectedDate

    if (timer) {
      clearTimeout(timer)
    }

    timer = setTimeout(flush, 700)
  }

  onUnmounted(() => {
    // Do not lose the last sentence when navigating away from the view.
    void flush()
  })
</script>

<template>
  <div class="day-note-bar">
    <span class="dn-label" title="What you worked on today, in your own words">Note</span>
    <input
      v-model="local"
      class="dn-input"
      type="text"
      placeholder="What did you work on today? Saved as you type…"
      maxlength="500"
      @input="onInput"
      @blur="flush"
    />
  </div>
</template>

<style scoped>
  .day-note-bar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 0 var(--space-3);
    height: 32px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }

  .dn-label {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .dn-input {
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    color: var(--text);
    font-size: var(--text-xs);
    padding: var(--space-1) 0;
    outline: none;
  }

  .dn-input::placeholder {
    color: var(--text-faint, var(--text-muted));
    opacity: 0.7;
  }

  .dn-input:focus::placeholder {
    opacity: 0.4;
  }
</style>
