<script setup lang="ts">
import { computed } from 'vue'
import type { IdleReturnEvent } from '../schemas'

const props = defineProps<{ event: IdleReturnEvent }>()
const emit = defineEmits<{ (e: 'dismiss'): void }>()

const mins = computed(() => Math.round(props.event.idleSecs / 60))

function fmt(iso: string): string {
  const d = new Date(iso)
  return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}
</script>

<template>
  <div class="idle-prompt">
    <div class="idle-icon">💤</div>
    <div class="idle-body">
      <strong>Away for {{ mins }} min</strong>
      <span>{{ fmt(event.idleStartedAt) }} – {{ fmt(event.idleEndedAt) }}</span>
    </div>
    <button class="btn-ghost dismiss" @click="emit('dismiss')">✕</button>
  </div>
</template>

<style scoped>
.idle-prompt {
  position: fixed;
  bottom: 20px;
  right: 20px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 10px;
  box-shadow: var(--shadow-md);
  padding: 12px 14px;
  display: flex;
  align-items: center;
  gap: 10px;
  z-index: 200;
  max-width: 280px;
}

.idle-icon { font-size: 20px; flex-shrink: 0; }

.idle-body {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 2px;
  font-size: 12px;
}
.idle-body strong { font-size: 13px; }
.idle-body span { color: var(--text-muted); }

.dismiss { padding: 2px 6px; }
</style>
