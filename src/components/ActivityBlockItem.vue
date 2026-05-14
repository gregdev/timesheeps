<script setup lang="ts">
import { computed } from 'vue'
import type { ActivityBlock } from '../schemas'
import { useTimeline } from '../composables/useTimeline'

const props = defineProps<{ block: ActivityBlock }>()
const { minuteToY, isoToMinutes, formatDuration, minutesToTime } = useTimeline()

const startMin = computed(() => isoToMinutes(props.block.startedAt))
const endMin = computed(() => isoToMinutes(props.block.endedAt))
const top = computed(() => minuteToY(startMin.value))
const height = computed(() => Math.max(minuteToY(endMin.value) - top.value, 4))
const durationMin = computed(() => Math.round(props.block.durationSecs / 60))
const color = computed(() => appColor(props.block.appName))

function appColor(name: string): string {
  const palette = ['#6366f1','#8b5cf6','#ec4899','#f43f5e','#f97316','#eab308','#22c55e','#06b6d4','#3b82f6','#14b8a6']
  let h = 0
  for (const c of name) h = (h * 31 + c.charCodeAt(0)) >>> 0
  return palette[h % palette.length]
}

const tooltip = computed(() =>
  `${props.block.appName}\n${props.block.windowTitle}\n${minutesToTime(Math.round(startMin.value))} – ${minutesToTime(Math.round(endMin.value))} (${formatDuration(durationMin.value)})`
)
</script>

<template>
  <div
    class="activity-block"
    :style="{ top: top + 'px', height: height + 'px', '--color': color }"
    :title="tooltip"
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
  cursor: default;
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
