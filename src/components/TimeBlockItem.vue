<script setup lang="ts">
  import { ref, computed, onUnmounted } from 'vue'
  import type { TimeEntry } from '../schemas'
  import { useTimeline, HOUR_HEIGHT } from '../composables/useTimeline'
  import { useProjectsStore } from '../stores/projects'
  import { useDayStore } from '../stores/day'

  const props = defineProps<{ entry: TimeEntry }>()
  const emit = defineEmits<{
    (e: 'edit', entry: TimeEntry): void
    (e: 'updated', id: number, startMinutes: number, endMinutes: number): void
  }>()

  const { minuteToY, startMin, endMin, snapMinutes, formatDuration, minutesToTime } = useTimeline()
  const projectsStore = useProjectsStore()
  const dayStore = useDayStore()

  const project = computed(() => projectsStore.byId(props.entry.projectId))
  const color = computed(() => project.value?.color ?? '#6366f1')
  const durationMin = computed(() => localEnd.value - localStart.value)

  const localStart = ref(props.entry.startMinutes)
  const localEnd = ref(props.entry.endMinutes)

  const top = computed(() => minuteToY(localStart.value))
  const height = computed(() => Math.max(minuteToY(localEnd.value) - top.value, 4))

  // Drag state
  type DragEdge = 'top' | 'bottom' | 'move'
  let dragEdge: DragEdge | null = null
  let dragOriginY = 0
  let dragOriginStart = 0
  let dragOriginEnd = 0

  function startDrag(edge: DragEdge, e: MouseEvent) {
    e.preventDefault()
    e.stopPropagation()
    dragEdge = edge
    dragOriginY = e.clientY
    dragOriginStart = localStart.value
    dragOriginEnd = localEnd.value
    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)
  }

  function onMove(e: MouseEvent) {
    const deltaY = e.clientY - dragOriginY
    const deltaMins = snapMinutes((deltaY / HOUR_HEIGHT) * 60)
    const duration = dragOriginEnd - dragOriginStart

    if (dragEdge === 'bottom') {
      localEnd.value = Math.max(dragOriginEnd + deltaMins, dragOriginStart + 5)
      localEnd.value = Math.min(localEnd.value, endMin.value)
    } else if (dragEdge === 'top') {
      localStart.value = Math.min(dragOriginStart + deltaMins, dragOriginEnd - 5)
      localStart.value = Math.max(localStart.value, startMin.value)
    } else {
      let ns = dragOriginStart + deltaMins
      ns = Math.max(ns, startMin.value)
      ns = Math.min(ns, endMin.value - duration)
      localStart.value = ns
      localEnd.value = ns + duration
    }
  }

  async function onUp() {
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('mouseup', onUp)
    const edge = dragEdge
    dragEdge = null
    if (
      localStart.value !== props.entry.startMinutes ||
      localEnd.value !== props.entry.endMinutes
    ) {
      // Only count as edit (not click) if position changed
      emit('updated', props.entry.id, localStart.value, localEnd.value)
      await dayStore.updateEntry(
        props.entry.id,
        props.entry.projectId,
        localStart.value,
        localEnd.value,
        props.entry.note,
      )
    } else if (edge === 'move') {
      // No movement = it was a click
      emit('edit', props.entry)
    }
  }

  function onClick() {
    if (dragEdge === null) emit('edit', props.entry)
  }

  onUnmounted(() => {
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('mouseup', onUp)
  })
</script>

<template>
  <div
    class="time-block"
    :style="{ top: top + 'px', height: height + 'px', '--color': color }"
    data-tooltip="Click to edit"
  >
    <div class="handle handle-top" @mousedown.stop="startDrag('top', $event)" />
    <div class="block-body" @mousedown.stop="startDrag('move', $event)" @click.stop="onClick">
      <span v-if="height > 22" class="project-name">{{ project?.name ?? '(no project)' }}</span>
      <span v-if="height > 38" class="duration">
        {{ minutesToTime(localStart) }} – {{ minutesToTime(localEnd) }} ·
        {{ formatDuration(durationMin) }}
      </span>
      <span v-if="height > 54 && entry.note" class="note">{{ entry.note }}</span>
    </div>
    <div class="handle handle-bottom" @mousedown.stop="startDrag('bottom', $event)" />
  </div>
</template>

<style scoped>
  .time-block {
    position: absolute;
    left: 4px;
    right: 4px;
    border-radius: 5px;
    background: color-mix(in srgb, var(--color) 22%, var(--surface));
    border: 2px solid var(--color);
    overflow: visible;
    display: flex;
    flex-direction: column;
    user-select: none;
    z-index: 10;
    transition: box-shadow 0.15s;
  }

  .time-block:hover {
    box-shadow: 0 2px 8px rgb(0 0 0 / 12%);
  }

  .handle {
    height: 6px;
    cursor: ns-resize;
    flex-shrink: 0;
    position: relative;
    z-index: 2;
  }

  .handle::after {
    content: '';
    position: absolute;
    left: 50%;
    top: 50%;
    transform: translate(-50%, -50%);
    width: 24px;
    height: 2px;
    background: var(--color);
    border-radius: 1px;
    opacity: 0.35;
    transition: opacity 0.15s;
  }

  .time-block:hover .handle::after {
    opacity: 0.7;
  }

  .block-body {
    flex: 1;
    padding: 2px var(--space-2);
    cursor: grab;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-height: 0;
  }

  .block-body:active {
    cursor: grabbing;
  }

  .project-name {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--color);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .duration {
    font-size: var(--text-xs);
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .note {
    font-size: var(--text-xs);
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
