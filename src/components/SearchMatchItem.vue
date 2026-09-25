<script setup lang="ts">
  import type { ActivityBlock } from '../schemas'
  import { useContextMenu } from '../composables/useContextMenu'
  import { useAppColour } from '../composables/useAppColour'
  import { formatSeconds, isoToLocalMinutes, minutesToClock } from '../composables/useFormat'

  const props = defineProps<{ block: ActivityBlock }>()

  const emit = defineEmits<{
    (e: 'track-to-project', block: ActivityBlock): void
    (e: 'create-ignore-rule', appName: string): void
    (e: 'delete', block: ActivityBlock): void
  }>()

  const { open: openMenu } = useContextMenu()
  const { appColour: appColor } = useAppColour()

  function formatTimeRange(startIso: string, endIso: string): string {
    const start = minutesToClock(Math.round(isoToLocalMinutes(startIso)))
    const end = minutesToClock(Math.round(isoToLocalMinutes(endIso)))

    return `${start} – ${end}`
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
      {
        label: 'Delete',
        danger: true,
        action: () => emit('delete', props.block),
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
    <span class="match-dur">{{ formatSeconds(block.durationSecs) }}</span>
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
