<script setup lang="ts">
  import { computed } from 'vue'
  import { useTimeline } from '../composables/useTimeline'
  import { minutesByProject, useProjectGroups } from '../composables/useProjectGroups'
  import type { TimeEntry } from '../schemas'

  const props = defineProps<{ entries: TimeEntry[] }>()

  const { formatDuration } = useTimeline()

  const { groups, totalMins, maxMins } = useProjectGroups(
    computed(() => minutesByProject(props.entries)),
  )
</script>

<template>
  <div v-if="totalMins > 0" class="period-summary">
    <div class="ps-header">
      <span class="ps-title">Period Total</span>
      <span class="ps-grand-total">{{ formatDuration(totalMins) }}</span>
    </div>

    <ul class="ps-list">
      <li v-for="group in groups" :key="group.project.id" class="ps-group">
        <div class="ps-item">
          <div class="ps-bar-wrap">
            <div
              class="ps-bar"
              :style="{
                width: (group.totalMins / maxMins) * 100 + '%',
                background: group.project.color,
              }"
            />
          </div>
          <div class="ps-labels">
            <span class="ps-dot" :style="{ background: group.project.color }" />
            <span class="ps-name">{{ group.project.name }}</span>
          </div>
          <span class="ps-dur">{{ formatDuration(group.totalMins) }}</span>
        </div>

        <div v-for="child in group.children" :key="child.project.id" class="ps-item ps-child">
          <div class="ps-bar-wrap">
            <div
              class="ps-bar ps-bar-child"
              :style="{
                width: (child.mins / group.totalMins) * 100 + '%',
                background: group.project.color,
              }"
            />
          </div>
          <div class="ps-labels">
            <span class="ps-name ps-child-name">{{ child.project.name }}</span>
          </div>
          <span class="ps-dur ps-child-dur">{{ formatDuration(child.mins) }}</span>
        </div>
      </li>
    </ul>
  </div>
</template>

<style scoped>
  .period-summary {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    flex-shrink: 0;
  }

  .ps-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: var(--space-2) 14px;
    border-bottom: 1px solid var(--border);
  }

  .ps-title {
    font-size: var(--text-xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .ps-grand-total {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--primary);
  }

  .ps-list {
    list-style: none;
    margin: 0;
    padding: var(--space-1) 0;
    display: flex;
    flex-wrap: wrap;
  }

  .ps-group {
    display: flex;
    flex-direction: column;
    min-width: 160px;
    flex: 1;
  }

  .ps-item {
    display: grid;
    grid-template-columns: 1fr auto;
    grid-template-rows: auto auto;
    gap: 0 var(--space-2);
    padding: var(--space-2) var(--space-4);
    transition: background 0.12s;
  }

  .ps-item:hover {
    background: color-mix(in srgb, var(--border) 40%, transparent);
  }

  .ps-child {
    padding: var(--space-1) var(--space-4) var(--space-1) var(--space-7);
  }

  .ps-bar-wrap {
    grid-column: 1 / -1;
    height: 3px;
    background: var(--border);
    border-radius: 2px;
    margin-bottom: var(--space-1);
    overflow: hidden;
  }

  .ps-child .ps-bar-wrap {
    height: 2px;
    margin-bottom: var(--space-1);
  }

  .ps-bar {
    height: 100%;
    border-radius: 2px;
    opacity: 0.7;
    transition: width 0.3s ease;
  }

  .ps-bar-child {
    opacity: 0.5;
  }

  .ps-labels {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 0;
  }

  .ps-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .ps-name {
    font-size: var(--text-xs);
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ps-child-name {
    font-size: var(--text-xs);
    font-weight: 400;
    color: var(--text-muted);
  }

  .ps-dur {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    align-self: center;
  }

  .ps-child-dur {
    font-size: var(--text-xs);
    font-weight: 400;
    color: var(--text-muted);
  }
</style>
