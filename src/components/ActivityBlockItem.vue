<script setup lang="ts">
  import { computed } from 'vue'
  import type { ActivityBlock } from '../schemas'
  import { useTimeline } from '../composables/useTimeline'
  import { useEntryModal } from '../composables/useEntryModal'
  import { useContextMenu } from '../composables/useContextMenu'
  import { useSettingsStore } from '../stores/settings'
  import { useDayStore } from '../stores/day'

  const props = defineProps<{ block: ActivityBlock }>()
  const { minuteToY, isoToMinutes, formatDuration, minutesToTime } = useTimeline()
  const { pendingCreate } = useEntryModal()
  const { open: openMenu } = useContextMenu()
  const settingsStore = useSettingsStore()
  const dayStore = useDayStore()

  const startMin = computed(() => isoToMinutes(props.block.startedAt))
  const endMin = computed(() => isoToMinutes(props.block.endedAt))
  const top = computed(() => minuteToY(startMin.value))
  const height = computed(() => Math.max(minuteToY(endMin.value) - top.value, 4))
  const durationMin = computed(() => Math.round(props.block.durationSecs / 60))
  const color = computed(() => appColor(props.block.appName))

  function appColor(name: string): string {
    const palette = [
      '#6366f1',
      '#8b5cf6',
      '#ec4899',
      '#f43f5e',
      '#f97316',
      '#eab308',
      '#22c55e',
      '#06b6d4',
      '#3b82f6',
      '#14b8a6',
    ]
    let h = 0
    for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0
    return palette[h % palette.length]
  }

  const tooltip = computed(
    () =>
      `${props.block.appName}\n${props.block.windowTitle}\n${minutesToTime(Math.round(startMin.value))} – ${minutesToTime(Math.round(endMin.value))} (${formatDuration(durationMin.value)})`,
  )

  function onContextMenu(e: MouseEvent) {
    openMenu(e, [
      {
        label: 'Track to project…',
        action: () => {
          pendingCreate.value = {
            startMinutes: Math.round(startMin.value),
            endMinutes: Math.round(endMin.value),
            note: props.block.appName,
          }
        },
      },
      {
        label: 'Create ignore rule',
        action: async () => {
          await settingsStore.createRule('app_name', props.block.appName)
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
    padding: 2px 5px;
    cursor: context-menu;
    transition: filter 0.15s;
  }

  .activity-block:hover {
    filter: brightness(1.07);
  }

  .app-name {
    display: block;
    font-size: 11px;
    font-weight: 600;
    color: var(--color);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.3;
  }

  .title {
    display: block;
    font-size: 10px;
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.3;
  }
</style>
