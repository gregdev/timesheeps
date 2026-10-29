<script setup lang="ts">
  import { nextTick, onMounted, onUnmounted, ref } from 'vue'

  /**
   * Modal prompt for destructive or ambiguous actions.
   *
   * Deliberately styled to match `ProjectPickerModal` rather than using
   * `window.confirm`: the native box cannot be themed, looks nothing like the
   * rest of the app under a dark OS theme, and blocks the whole webview.
   *
   * It has **three** outcomes, which is why it is not a boolean:
   *
   *   confirm → the primary button
   *   cancel  → the secondary button (which may itself do real work)
   *   dismiss → ✕, Escape or a backdrop click; the user wants neither
   *             action, so a caller must treat it as "do nothing"
   *
   * Collapsing `dismiss` into `cancel` would silently run the secondary action
   * whenever someone pressed Escape.
   *
   * Focus starts on the secondary button, not on the primary one — Enter on a
   * freshly opened "Delete?" should not delete anything.
   */
  const props = withDefaults(
    defineProps<{
      title: string
      message: string
      /** Small chips detailing exactly what is about to go, e.g. "21:00–21:40". */
      chips?: string[]
      confirmLabel?: string
      cancelLabel?: string
      /**
       * When set, a link-styled "cancel" is rendered on the left of the footer.
       *
       * Needed for prompts whose secondary button *does* something ("Add below
       * it"): without it there is no visible way to back out, and the only exit
       * is the ✕ or Escape. It emits `dismiss`, exactly like the ✕ — the link is
       * a clearer label for the same "do nothing" outcome, not a third action.
       */
      linkLabel?: string | null
      /** Renders the primary button in the danger colour. */
      danger?: boolean
      /** Footnote under the body; `null` hides it. */
      hint?: string | null
    }>(),
    {
      chips: () => [],
      confirmLabel: 'Delete',
      cancelLabel: 'Cancel',
      linkLabel: null,
      danger: true,
      hint: 'This cannot be undone.',
    },
  )

  const emit = defineEmits<{
    (e: 'confirm'): void
    (e: 'cancel'): void
    (e: 'dismiss'): void
  }>()

  const cancelRef = ref<HTMLButtonElement>()
  const linkRef = ref<HTMLButtonElement>()

  onMounted(async () => {
    // Registered on `document` in the capture phase so Escape closes the dialog
    // instead of reaching the timeline's own "clear selection" handler.
    document.addEventListener('keydown', onKeydown, true)
    await nextTick()
    // Focus the option that changes nothing, so a reflexive Enter can never run
    // a write: the left-hand link when there is one, else the secondary button.
    ;(linkRef.value ?? cancelRef.value)?.focus()
  })

  onUnmounted(() => {
    document.removeEventListener('keydown', onKeydown, true)
  })

  function onKeydown(e: KeyboardEvent) {
    if (e.key !== 'Escape') {
      // Enter is left to the focused button, so the initial "focus the secondary
      // button" above actually means something: a reflexive Enter cancels rather
      // than deleting. Tab, then Enter, still reaches the primary action.
      return
    }

    e.preventDefault()
    e.stopPropagation()
    emit('dismiss')
  }
</script>

<template>
  <div class="modal-backdrop" @mousedown.self="emit('dismiss')">
    <div class="modal" role="alertdialog" aria-modal="true" :aria-label="props.title">
      <div class="modal-header">
        <span>{{ props.title }}</span>
        <button class="btn-ghost close-btn" aria-label="Close" @click="emit('dismiss')">✕</button>
      </div>

      <div class="modal-body">
        <p class="message">{{ props.message }}</p>

        <div v-if="props.chips.length" class="chips">
          <span v-for="chip in props.chips" :key="chip" class="chip">{{ chip }}</span>
        </div>

        <p v-if="props.hint" class="hint">{{ props.hint }}</p>
      </div>

      <div class="modal-footer">
        <button v-if="props.linkLabel" ref="linkRef" class="link-btn" @click="emit('dismiss')">
          {{ props.linkLabel }}
        </button>
        <div class="spacer" />
        <button ref="cancelRef" class="btn-secondary" @click="emit('cancel')">
          {{ props.cancelLabel }}
        </button>
        <button
          class="confirm-btn"
          :class="{ 'btn-danger': props.danger, 'btn-primary': !props.danger }"
          @click="emit('confirm')"
        >
          {{ props.confirmLabel }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
  /* Kept in step with ProjectPickerModal so the two read as one family. */
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 35%);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 120;
    animation: backdrop-in 0.15s ease;
  }

  .modal {
    background: var(--surface);
    border-radius: 10px;
    box-shadow: var(--shadow-md);
    width: 340px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: modal-in 0.15s ease 0.05s both;
  }

  @keyframes backdrop-in {
    from {
      opacity: 0;
    }

    to {
      opacity: 1;
    }
  }

  @keyframes modal-in {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(-6px);
    }

    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .modal-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px var(--space-4) 10px;
    font-weight: 600;
    font-size: var(--text-sm);
    border-bottom: 1px solid var(--border);
  }

  .close-btn {
    font-size: 14px;
  }

  .modal-body {
    padding: 14px var(--space-4);
  }

  .message {
    font-size: var(--text-sm);
    margin: 0;
    line-height: 1.5;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    margin-top: 10px;
  }

  .chip {
    font-size: var(--text-xs);
    font-variant-numeric: tabular-nums;
    background: var(--surface-2);
    color: var(--text-muted);
    padding: 2px var(--space-2);
    border-radius: 10px;
    white-space: nowrap;
  }

  .hint {
    font-size: var(--text-xs);
    color: var(--text-muted);
    margin: 10px 0 0;
  }

  .modal-footer {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 10px var(--space-4) 14px;
    border-top: 1px solid var(--border);
  }

  .spacer {
    flex: 1;
  }

  /**
   * A text link rather than a button: it is the "do nothing" escape hatch, and
   * dressing it as a third button would give three actions equal visual weight
   * when only two of them do anything.
   */
  .link-btn {
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    font: inherit;
    font-size: var(--text-sm);
    color: var(--text-muted);
    text-decoration: underline;
    text-underline-offset: 2px;
    cursor: pointer;
    min-height: 0;
  }

  .link-btn:hover {
    color: var(--primary);
  }

  .link-btn:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
    border-radius: 2px;
  }
</style>
