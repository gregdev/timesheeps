<script setup lang="ts">
  import { computed } from 'vue'
  import { isToday, format } from 'date-fns'
  import type { Project } from '../schemas'

  const props = defineProps<{
    days: Date[]
    dayStrings: string[]
    projects: Project[]
    projectDayMinutes: (projectId: number, date: string) => number
    dayTotalMinutes: (date: string) => number
    hasUnlogged: (date: string) => boolean
    fmtMin: (min: number) => string
    emptyMessage: string
    gridColumns: string
  }>()

  const emit = defineEmits<{
    navigateTo: [date: string]
  }>()

  const rowStyle = computed(() => ({ gridTemplateColumns: props.gridColumns }))
</script>

<template>
  <div class="period-grid">
    <!-- Header row -->
    <div class="grid-row header-row" :style="rowStyle">
      <div class="cell label-cell" />

      <div
        v-for="(date, i) in dayStrings"
        :key="date"
        class="cell day-header"
        :class="{ today: isToday(days[i]) }"
        @click="emit('navigateTo', date)"
      >
        <span class="day-name">{{ format(days[i], 'EEE') }}</span>
        <span class="day-date">{{ format(days[i], 'M/d') }}</span>
        <span
          v-if="hasUnlogged(date)"
          class="unlogged-dot"
          title="Activity recorded but no time logged"
        >
          ●
        </span>
      </div>
    </div>

    <!-- Empty state -->
    <div v-if="projects.length === 0" class="grid-row empty-row" :style="rowStyle">
      <div class="cell empty-cell">{{ emptyMessage }}</div>
    </div>

    <!-- Project rows -->
    <div
      v-for="project in projects"
      :key="project.id"
      class="grid-row project-row"
      :style="rowStyle"
    >
      <div class="cell project-label">
        <span class="dot" :style="{ background: project.color }" />
        <span class="project-name">{{ project.name }}</span>
      </div>

      <div
        v-for="date in dayStrings"
        :key="date"
        class="cell entry-cell"
        :class="{ 'has-value': projectDayMinutes(project.id, date) > 0 }"
        @click="emit('navigateTo', date)"
      >
        {{ fmtMin(projectDayMinutes(project.id, date)) }}
      </div>
    </div>

    <!-- Total row -->
    <div class="grid-row total-row" :style="rowStyle">
      <div class="cell total-label">Total</div>

      <div
        v-for="date in dayStrings"
        :key="date"
        class="cell total-cell"
        :class="{ 'has-value': dayTotalMinutes(date) > 0 }"
        @click="emit('navigateTo', date)"
      >
        {{ fmtMin(dayTotalMinutes(date)) }}
      </div>
    </div>
  </div>
</template>

<style scoped>
  .period-grid {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    overflow: hidden;
    flex-shrink: 0;
    min-width: max-content;
  }

  .grid-row {
    display: grid;
  }

  .cell {
    padding: var(--space-2) 10px;
    font-size: var(--text-xs);
    border-right: 1px solid var(--border);
    border-bottom: 1px solid var(--border);
    min-width: 0;
  }

  .cell:last-child {
    border-right: none;
  }

  /* ── Header ─────────────────────────────────────────────────────────────── */

  .header-row .cell {
    background: var(--surface-2);
  }

  .day-header {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    cursor: pointer;
    transition: background 0.1s;
    user-select: none;
  }

  .day-header:hover {
    background: var(--surface);
  }

  .day-header.today {
    background: color-mix(in srgb, var(--primary) 8%, var(--surface-2));
  }

  .day-name {
    font-size: var(--text-xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
  }

  .day-header.today .day-name {
    color: var(--primary);
  }

  .day-date {
    font-size: var(--text-sm);
    font-weight: 500;
  }

  .day-header.today .day-date {
    color: var(--primary);
    font-weight: 700;
  }

  .unlogged-dot {
    font-size: 8px;
    color: var(--warning);
    line-height: 1;
    margin-top: 1px;
  }

  /* ── Empty state ─────────────────────────────────────────────────────────── */

  .empty-cell {
    grid-column: 1 / -1;
    background: var(--surface);
    color: var(--text-muted);
    text-align: center;
    padding: var(--space-5);
  }

  /* ── Project rows ────────────────────────────────────────────────────────── */

  .project-row .cell {
    background: var(--surface);
    transition: background 0.1s;
  }

  .total-row .cell {
    background: var(--surface-2);
    border-bottom: none;
  }

  /* hover must come after the base rules for both row types */
  .project-row:hover .cell {
    background: var(--surface-2);
  }

  .project-label {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-weight: 500;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .project-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-cell {
    text-align: right;
    cursor: pointer;
    color: var(--text-muted);
  }

  .entry-cell.has-value {
    color: var(--text);
    font-weight: 500;
  }

  /* ── Total row ───────────────────────────────────────────────────────────── */

  .total-label {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    display: flex;
    align-items: center;
  }

  .total-cell {
    text-align: right;
    cursor: pointer;
    font-weight: 600;
    color: var(--text-muted);
    transition: background 0.1s;
  }

  .total-cell:hover {
    background: color-mix(in srgb, var(--border) 40%, var(--surface-2));
  }

  .total-cell.has-value {
    color: var(--primary);
  }
</style>
