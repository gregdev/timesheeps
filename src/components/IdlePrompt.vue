<script setup lang="ts">
  import { computed } from 'vue'
  import type { IdleReturnEvent } from '../schemas'
  import { useTimeline } from '../composables/useTimeline'

  const props = defineProps<{ event: IdleReturnEvent }>()
  const emit = defineEmits<{ (e: 'dismiss'): void }>()

  const { formatDuration } = useTimeline()
  const duration = computed(() => formatDuration(Math.round(props.event.idleSecs / 60)))

  function fmt(iso: string): string {
    const d = new Date(iso)
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
  }
</script>

<template>
  <div class="idle-prompt">
    <div class="idle-icon">💤</div>
    <div class="idle-body">
      <strong>Away for {{ duration }}</strong>
      <span>{{ fmt(event.idleStartedAt) }} – {{ fmt(event.idleEndedAt) }}</span>
    </div>
    <button class="btn-ghost dismiss" @click="emit('dismiss')">✕</button>
  </div>
</template>

<style scoped>
  .idle-prompt {
    position: fixed;
    bottom: var(--space-5);
    right: var(--space-5);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: var(--shadow-md);
    padding: var(--space-3) 14px;
    display: flex;
    align-items: center;
    gap: 10px;
    z-index: 200;
    max-width: 280px;
  }

  .idle-icon {
    font-size: 20px;
    flex-shrink: 0;
  }

  .idle-body {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--text-xs);
  }

  .idle-body strong {
    font-size: var(--text-sm);
  }

  .idle-body span {
    color: var(--text-muted);
  }

  .dismiss {
    padding: 2px 6px;
  }
</style>
