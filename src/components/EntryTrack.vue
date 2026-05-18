<script setup lang="ts">
  import { ref, computed, onUnmounted } from 'vue'
  import type { TimeEntry } from '../schemas'
  import { useTimeline } from '../composables/useTimeline'
  import TimeBlockItem from './TimeBlockItem.vue'
  import SuggestedBlockItem from './SuggestedBlockItem.vue'

  defineProps<{
    entries: TimeEntry[]
    suggestions: { projectId: number; startMinutes: number; endMinutes: number }[]
  }>()
  const emit = defineEmits<{
    (e: 'request-create', startMinutes: number, endMinutes: number): void
    (e: 'edit', entry: TimeEntry): void
    (e: 'accept-suggestion', projectId: number, startMinutes: number, endMinutes: number): void
  }>()

  const { totalHeight, minuteToY, yToMinute, snapMinutes, clampMin, minutesToTime } = useTimeline()
  const trackRef = ref<HTMLElement>()

  // Hover state
  const hoverY = ref(0)
  const isHovering = ref(false)

  const hoverTime = computed(() => {
    const raw = yToMinute(hoverY.value)
    return minutesToTime(snapMinutes(Math.round(raw)))
  })

  function onTrackMousemove(e: MouseEvent) {
    if (!trackRef.value) return
    const rect = trackRef.value.getBoundingClientRect()
    hoverY.value = e.clientY - rect.top
    isHovering.value = true
  }

  function onTrackMouseleave() {
    isHovering.value = false
  }

  // Drag-to-create state
  const isDragging = ref(false)
  const hasMoved = ref(false)
  const dragStartMin = ref(0)
  const dragCurrentMin = ref(0)

  const previewTop = computed(() => {
    if (!isDragging.value || !hasMoved.value) return 0
    return minuteToY(Math.min(dragStartMin.value, dragCurrentMin.value))
  })
  const previewHeight = computed(() => {
    if (!isDragging.value || !hasMoved.value) return 0
    return Math.abs(minuteToY(dragCurrentMin.value) - minuteToY(dragStartMin.value))
  })

  const dragTopMin = computed(() => Math.min(dragStartMin.value, dragCurrentMin.value))
  const dragBottomMin = computed(() => Math.max(dragStartMin.value, dragCurrentMin.value))
  const dragTopY = computed(() => minuteToY(dragTopMin.value))
  const dragBottomY = computed(() => minuteToY(dragBottomMin.value))

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
    hasMoved.value = false

    document.addEventListener('mousemove', onDocMousemove)
    document.addEventListener('mouseup', onDocMouseup)
  }

  function onDocMousemove(e: MouseEvent) {
    if (!isDragging.value || !trackRef.value) return
    hasMoved.value = true
    const rect = trackRef.value.getBoundingClientRect()
    const relY = e.clientY - rect.top
    dragCurrentMin.value = snapMinutes(clampMin(yToMinute(relY)))
  }

  function onDocMouseup() {
    document.removeEventListener('mousemove', onDocMousemove)
    document.removeEventListener('mouseup', onDocMouseup)
    if (!isDragging.value) return
    isDragging.value = false
    hasMoved.value = false

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
    @mousemove="onTrackMousemove"
    @mouseleave="onTrackMouseleave"
  >
    <template v-if="isHovering && !hasMoved">
      <div class="hover-line" :style="{ top: hoverY + 'px' }" />
      <div class="hover-tooltip" :style="{ top: hoverY - 22 + 'px' }">{{ hoverTime }}</div>
    </template>

    <template v-if="isDragging && hasMoved && previewHeight <= 4">
      <div class="hover-line" :style="{ top: dragTopY + 'px' }" />
      <div class="hover-tooltip" :style="{ top: dragTopY - 22 + 'px' }">
        {{ minutesToTime(dragTopMin) }}
      </div>
    </template>

    <div
      v-if="isDragging && hasMoved && previewHeight > 4"
      class="drag-preview"
      :style="{ top: previewTop + 'px', height: previewHeight + 'px' }"
    />

    <template v-if="isDragging && hasMoved && previewHeight > 4">
      <div class="hover-tooltip" :style="{ top: dragTopY - 22 + 'px' }">
        {{ minutesToTime(dragTopMin) }}
      </div>
      <div
        v-if="dragBottomMin !== dragTopMin"
        class="hover-tooltip"
        :style="{ top: dragBottomY + 4 + 'px' }"
      >
        {{ minutesToTime(dragBottomMin) }}
      </div>
    </template>
    <TimeBlockItem
      v-for="entry in entries"
      :key="entry.id"
      :entry="entry"
      @edit="emit('edit', $event)"
    />
    <SuggestedBlockItem
      v-for="(s, i) in suggestions"
      :key="'s' + i"
      :project-id="s.projectId"
      :start-minutes="s.startMinutes"
      :end-minutes="s.endMinutes"
      @accept="emit('accept-suggestion', $event, s.startMinutes, s.endMinutes)"
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

  .hover-line {
    position: absolute;
    left: 0;
    right: 0;
    height: 1px;
    background: var(--primary);
    opacity: 0.7;
    pointer-events: none;
    z-index: 6;
  }

  .hover-tooltip {
    position: absolute;
    right: 6px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 4px;
    padding: 2px 6px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text);
    pointer-events: none;
    z-index: 7;
    white-space: nowrap;
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.15);
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
