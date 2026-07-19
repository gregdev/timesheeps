<script setup lang="ts">
  import { computed } from 'vue'
  import type { ActivityBlock } from '../schemas'
  import { useTimeline } from '../composables/useTimeline'
  import { useEntryModal } from '../composables/useEntryModal'
  import { useContextMenu } from '../composables/useContextMenu'
  import { useSettingsStore } from '../stores/settings'
  import { useDayStore } from '../stores/day'
  import { useAppColour } from '../composables/useAppColour'

  const props = defineProps<{ block: ActivityBlock }>()
  const { minuteToY, isoToMinutes, formatDuration, minutesToTime } = useTimeline()
  const { pendingCreate } = useEntryModal()
  const { open: openMenu } = useContextMenu()
  const settingsStore = useSettingsStore()
  const dayStore = useDayStore()
  const { appColour } = useAppColour()

  const startMin = computed(() => isoToMinutes(props.block.startedAt))
  const endMin = computed(() => isoToMinutes(props.block.endedAt))
  const top = computed(() => minuteToY(startMin.value))
  const height = computed(() => Math.max(minuteToY(endMin.value) - top.value, 4))
  const durationMin = computed(() => Math.round(props.block.durationSecs / 60))
  const color = computed(() => appColour(props.block.appName))

  const tooltip = computed(
    () =>
      `${props.block.appName}\n${props.block.windowTitle}\n${minutesToTime(Math.round(startMin.value))} – ${minutesToTime(Math.round(endMin.value))} (${formatDuration(durationMin.value)})`,
  )

  function onContextMenu(e: MouseEvent) {
    const appName = props.block.appName
    const startM = Math.round(startMin.value)
    const endM = Math.round(endMin.value)

    openMenu(e, [
      {
        label: 'Track to project…',
        action: () => {
          pendingCreate.value = {
            startMinutes: startM,
            endMinutes: endM,
            note: appName,
            autoTrackAppName: appName,
          }
        },
      },
      {
        label: 'Auto-track to project…',
        action: () => {
          pendingCreate.value = {
            startMinutes: startM,
            endMinutes: endM,
            note: appName,
            autoTrackAppName: appName,
            autoTrackEnabled: true,
          }
        },
      },
      {
        label: 'Create ignore rule',
        action: async () => {
          await settingsStore.createRule('app_name', appName)
          await dayStore.loadDay(undefined, true)
        },
      },
    ])
  }
</script>

<template>
  <div
    class="activity-block"
    :style="{ top: top + 'px', height: height + 'px', '--color': color }"
    :data-tooltip="tooltip"
    :data-tooltip-color="color"
    @contextmenu="onContextMenu"
  >
    <span v-if="height > 18" class="app-name">{{ block.appName }}</span>
    <span v-if="height > 34" class="title">{{ block.windowTitle }}</span>
  </div>
</template>

<style scoped>
  .activity-block {
    position: absolute;
    left: 3px;
    right: 3px;
    border-radius: 4px;
    background: color-mix(in srgb, var(--color) 18%, transparent);
    border-left: 3px solid var(--color);
    overflow: hidden;
    padding: 2px var(--space-1);
    cursor: context-menu;
    transition: filter 0.15s;
  }

  .activity-block:hover {
    filter: brightness(1.07);
  }

  .activity-leave-active {
    transition: opacity 0.2s ease;
    pointer-events: none;
  }

  .activity-leave-to {
    opacity: 0;
  }

  .activity-enter-active {
    transition: opacity 0.3s ease;
    transition-delay: calc(0.2s + min(calc(var(--i, 0) * 20ms), 280ms));
  }

  .activity-enter-from {
    opacity: 0;
  }

  .app-name {
    display: block;
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--color);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.3;
  }

  .title {
    display: block;
    font-size: var(--text-xs);
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.3;
  }
</style>
