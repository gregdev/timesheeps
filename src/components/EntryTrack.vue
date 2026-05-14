<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue'
import type { TimeEntry } from '../schemas'
import { useTimeline } from '../composables/useTimeline'
import TimeBlockItem from './TimeBlockItem.vue'

defineProps<{ entries: TimeEntry[] }>()
const emit = defineEmits<{
  (e: 'request-create', startMinutes: number, endMinutes: number): void
  (e: 'edit', entry: TimeEntry): void
}>()

const { totalHeight, minuteToY, yToMinute, snapMinutes, clampMin } = useTimeline()
const trackRef = ref<HTMLElement>()

// Drag-to-create state
const isDragging = ref(false)
const dragStartMin = ref(0)
const dragCurrentMin = ref(0)

const previewTop = computed(() => {
  if (!isDragging.value) return 0
  return minuteToY(Math.min(dragStartMin.value, dragCurrentMin.value))
})
const previewHeight = computed(() => {
  if (!isDragging.value) return 0
  return Math.abs(minuteToY(dragCurrentMin.value) - minuteToY(dragStartMin.value))
})

function onTrackMousedown(e: MouseEvent) {
  if (e.button !== 0) return
  if ((e.target as HTMLElement).closest('.time-block')) return
  if (!trackRef.value) return

  const rect = trackRef.value.getBoundingClientRect()
  const relY = e.clientY - rect.top
  const rawMin = yToMinute(relY)
  dragStartMin.value = snapMinutes(clampMin(rawMin))
  dragCurrentMin.value = dragStartMin.value
  isDragging.value = true

  document.addEventListener('mousemove', onDocMousemove)
  document.addEventListener('mouseup', onDocMouseup)
}

function onDocMousemove(e: MouseEvent) {
  if (!isDragging.value || !trackRef.value) return
  const rect = trackRef.value.getBoundingClientRect()
  const relY = e.clientY - rect.top
  dragCurrentMin.value = snapMinutes(clampMin(yToMinute(relY)))
}

function onDocMouseup() {
  document.removeEventListener('mousemove', onDocMousemove)
  document.removeEventListener('mouseup', onDocMouseup)
  if (!isDragging.value) return
  isDragging.value = false

  const start = Math.min(dragStartMin.value, dragCurrentMin.value)
  const end = Math.max(dragStartMin.value, dragCurrentMin.value)
  if (end - start >= 5) {
    emit('request-create', start, end)
  }
}

onUnmounted(() => {
  document.removeEventListener('mousemove', onDocMousemove)
  document.removeEventListener('mouseup', onDocMouseup)
})
</script>

<template>
  <div
    ref="trackRef"
    class="entry-track"
    :style="{ height: totalHeight + 'px' }"
    @mousedown="onTrackMousedown"
  >
    <div
      v-if="isDragging && previewHeight > 4"
      class="drag-preview"
      :style="{ top: previewTop + 'px', height: previewHeight + 'px' }"
    />
    <TimeBlockItem
      v-for="entry in entries"
      :key="entry.id"
      :entry="entry"
      @edit="emit('edit', $event)"
    />
  </div>
</template>

<style scoped>
.entry-track {
  position: relative;
  flex: 1;
  cursor: crosshair;
  background: transparent;
}

.drag-preview {
  position: absolute;
  left: 4px;
  right: 4px;
  border-radius: 5px;
  background: color-mix(in srgb, var(--primary) 18%, transparent);
  border: 2px dashed var(--primary);
  pointer-events: none;
  z-index: 5;
}
</style>
