<script setup lang="ts">
  import type { ActivityBlock } from '../schemas'
  import { useContextMenu } from '../composables/useContextMenu'
  import { useAppColour } from '../composables/useAppColour'

  const props = defineProps<{ block: ActivityBlock }>()

  const emit = defineEmits<{
    (e: 'track-to-project', block: ActivityBlock): void
    (e: 'create-ignore-rule', appName: string): void
  }>()

  const { open: openMenu } = useContextMenu()
  const { appColour: appColor } = useAppColour()

  function formatDuration(secs: number): string {
    const h = Math.floor(secs / 3600)
    const m = Math.floor((secs % 3600) / 60)
    if (h === 0) return `${m}m`
    if (m === 0) return `${h}h`
    return `${h}h ${m}m`
  }

  function formatTimeRange(startIso: string, endIso: string): string {
    const s = new Date(startIso)
    const e = new Date(endIso)
    const fmt = (d: Date) =>
      `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`
    return `${fmt(s)} – ${fmt(e)}`
  }

  function onContextMenu(e: MouseEvent) {
    openMenu(e, [
      {
        label: 'Track to project…',
        action: () => emit('track-to-project', props.block),
      },
      {
        label: 'Create ignore rule',
        action: () => emit('create-ignore-rule', props.block.appName),
      },
    ])
  }
</script>

<template>
  <li class="match-item" @contextmenu="onContextMenu">
    <span class="match-time">
      {{ formatTimeRange(block.startedAt, block.endedAt) }}
    </span>
    <span class="match-app" :style="{ color: appColor(block.appName) }">
      {{ block.appName }}
    </span>
    <span class="match-title">{{ block.windowTitle }}</span>
    <span class="match-dur">{{ formatDuration(block.durationSecs) }}</span>
  </li>
</template>

<style scoped>
  .match-item {
    display: grid;
    grid-template-columns: 110px auto 1fr auto;
    align-items: center;
    gap: 10px;
    padding: 5px 8px;
    border-radius: var(--radius);
    font-size: 12px;
    transition: background 0.12s;
  }

  .match-item:hover {
    background: color-mix(in srgb, var(--border) 40%, transparent);
  }

  .match-time {
    color: var(--text-muted);
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .match-app {
    font-weight: 600;
    white-space: nowrap;
  }

  .match-title {
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .match-dur {
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
</style>
