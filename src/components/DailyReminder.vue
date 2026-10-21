<script setup lang="ts">
  import { computed, onMounted, onUnmounted } from 'vue'
  import { format, parseISO } from 'date-fns'
  import type { DailyReminderEvent, DailyReminderProject } from '../schemas'
  import { formatSeconds } from '../composables/useFormat'

  const props = defineProps<{ event: DailyReminderEvent }>()
  const emit = defineEmits<{
    (e: 'dismiss'): void
    (e: 'open'): void
  }>()

  const dayLabel = computed(() => format(parseISO(props.event.date), 'EEEE d MMMM'))
  const logged = computed(() => formatSeconds(props.event.loggedSecs))
  const tracked = computed(() => formatSeconds(props.event.trackedSecs))
  const nothingYet = computed(() => props.event.trackedSecs === 0 && props.event.loggedSecs === 0)

  /** The backend already sorts by tracked time; the prompt stays compact. */
  const rows = computed(() => props.event.projects.slice(0, 6))
  const hiddenCount = computed(() => Math.max(0, props.event.projects.length - rows.value.length))

  /** Logged time is what the timesheet shows, so it wins when both exist. */
  function rowValue(p: DailyReminderProject): string {
    return formatSeconds(p.loggedSecs > 0 ? p.loggedSecs : p.trackedSecs)
  }

  function rowKind(p: DailyReminderProject): string {
    return p.loggedSecs > 0 ? 'logged' : 'tracked'
  }

  function onDocKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      emit('dismiss')
    }
  }

  onMounted(() => document.addEventListener('keydown', onDocKeydown))
  onUnmounted(() => document.removeEventListener('keydown', onDocKeydown))
</script>

<template>
  <div class="reminder-backdrop" @mousedown.self="emit('dismiss')">
    <div class="reminder" role="dialog" aria-modal="true" aria-labelledby="reminder-title">
      <div class="reminder-header">
        <span id="reminder-title">🗓 Time to check your timesheet</span>
        <button class="btn-ghost close-btn" aria-label="Dismiss" @click="emit('dismiss')">✕</button>
      </div>

      <div class="reminder-body">
        <p class="reminder-date">{{ dayLabel }}</p>

        <div class="totals">
          <div class="total">
            <span class="total-value">{{ logged }}</span>
            <span class="total-label">logged</span>
          </div>
          <div class="total">
            <span class="total-value">{{ tracked }}</span>
            <span class="total-label">tracked</span>
          </div>
        </div>

        <ul v-if="rows.length > 0" class="project-list">
          <li v-for="p in rows" :key="p.projectId" class="project-row">
            <span class="color-dot" :style="{ background: p.color || 'var(--border-strong)' }" />
            <span class="project-name">{{ p.projectName }}</span>
            <span class="project-value">{{ rowValue(p) }}</span>
            <span class="project-kind">{{ rowKind(p) }}</span>
          </li>
        </ul>

        <p v-else-if="nothingYet" class="empty">Nothing recorded today yet.</p>

        <p v-if="hiddenCount > 0" class="more">
          +{{ hiddenCount }} more project{{ hiddenCount === 1 ? '' : 's' }}
        </p>

        <p class="hint">You can turn this reminder off, or change its time, in Settings.</p>
      </div>

      <div class="reminder-footer">
        <button class="btn-secondary" @click="emit('dismiss')">Dismiss</button>
        <button class="btn-primary" @click="emit('open')">Open timeline</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
  .reminder-backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 35%);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 300;
    animation: backdrop-in 0.15s ease;
  }

  .reminder {
    background: var(--surface);
    border-radius: 10px;
    box-shadow: var(--shadow-md);
    width: 360px;
    max-width: calc(100vw - 2 * var(--space-4));
    display: flex;
    flex-direction: column;
    overflow: hidden;
    animation: reminder-in 0.15s ease 0.05s both;
  }

  @keyframes backdrop-in {
    from {
      opacity: 0;
    }

    to {
      opacity: 1;
    }
  }

  @keyframes reminder-in {
    from {
      opacity: 0;
      transform: scale(0.96) translateY(-6px);
    }

    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .reminder-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-2);
    padding: 14px var(--space-4) 10px;
    font-weight: 600;
    font-size: var(--text-sm);
    border-bottom: 1px solid var(--border);
  }

  .close-btn {
    font-size: 14px;
  }

  .reminder-body {
    padding: 14px var(--space-4);
  }

  .reminder-date {
    font-size: var(--text-xs);
    color: var(--text-muted);
    margin-bottom: var(--space-3);
  }

  .totals {
    display: flex;
    gap: var(--space-4);
    margin-bottom: var(--space-3);
  }

  .total {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .total-value {
    font-size: var(--text-xl);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    line-height: 1.1;
  }

  .total-label {
    font-size: var(--text-xs);
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .project-list {
    list-style: none;
    margin: 0 0 var(--space-2);
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 5px;
    border-top: 1px solid var(--border);
    padding-top: var(--space-3);
  }

  .project-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-xs);
  }

  .color-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .project-name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .project-value {
    font-variant-numeric: tabular-nums;
    font-weight: 500;
  }

  .project-kind {
    color: var(--text-faint);
    min-width: 42px;
  }

  .empty,
  .more,
  .hint {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .empty {
    margin-bottom: var(--space-2);
  }

  .more {
    margin-top: 2px;
  }

  .hint {
    margin-top: var(--space-3);
    color: var(--text-faint);
  }

  .reminder-footer {
    display: flex;
    justify-content: flex-end;
    gap: var(--space-2);
    padding: 10px var(--space-4) 14px;
    border-top: 1px solid var(--border);
  }
</style>
