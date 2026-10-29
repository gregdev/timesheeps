<script setup lang="ts">
  import { ref, computed, onUnmounted, watch } from 'vue'
  import type { TimeEntry } from '../schemas'
  import { useTimeline } from '../composables/useTimeline'
  import { useProjectsStore } from '../stores/projects'
  import { useDayStore } from '../stores/day'

  const props = defineProps<{ entry: TimeEntry; selected?: boolean }>()
  const emit = defineEmits<{
    (e: 'edit', entry: TimeEntry): void
    (e: 'select', entry: TimeEntry, event: MouseEvent): void
    (e: 'context-menu', entry: TimeEntry, event: MouseEvent): void
  }>()

  const { minuteToY, startMin, endMin, snapMinutes, hourHeight, formatDuration, minutesToTime } =
    useTimeline()
  const projectsStore = useProjectsStore()
  const dayStore = useDayStore()

  const project = computed(() => projectsStore.byId(props.entry.projectId))
  const color = computed(() => project.value?.color ?? '#6366f1')
  const durationMin = computed(() => localEnd.value - localStart.value)

  const localStart = ref(props.entry.startMinutes)
  const localEnd = ref(props.entry.endMinutes)

  const top = computed(() => minuteToY(localStart.value))
  const height = computed(() => Math.max(minuteToY(localEnd.value) - top.value, 8))

  /**
   * Shrink the resize handles on short blocks so they keep a clickable body.
   * A fixed 6px per handle swallowed a 6-minute entry whole at 100% zoom.
   */
  const handleHeight = computed(() => Math.max(2, Math.min(6, Math.floor(height.value / 4))))

  /**
   * Hover tooltip: what this block actually is, then how to change it.
   *
   * The block body only shows these lines when it is tall enough, so a short
   * entry would otherwise be unidentifiable without opening the editor — which
   * now needs a double-click.
   */
  const tooltip = computed(() => {
    const lines = [
      project.value?.name ?? '(no project)',
      `${minutesToTime(localStart.value)} – ${minutesToTime(localEnd.value)} · ${formatDuration(durationMin.value)}`,
    ]
    const note = props.entry.note.trim()

    if (note) {
      lines.push(note)
    }

    lines.push('Double-click to edit · drag to move · ctrl/shift+click to select')

    return lines.join('\n')
  })

  // Drag state
  type DragEdge = 'top' | 'bottom' | 'move'
  let dragEdge: DragEdge | null = null
  let dragOriginY = 0
  let dragOriginStart = 0
  let dragOriginEnd = 0
  let didMove = false
  /**
   * A drag still produces a trailing `click` event on the block body once the
   * mouse is released, which would pop the edit modal straight after a move or
   * resize. Swallow exactly that one click.
   */
  let suppressClick = false
  /**
   * Whether the press started on the body rather than on a resize handle.
   * Only a body press produces a trailing click to swallow — setting the flag
   * for a handle press would eat the *next* body click instead.
   */
  let pressStartedOnBody = false

  // Keep the local copy in step with the store. Without this an entry edited
  // through the modal (or replaced by the 30s activity refresh) keeps rendering
  // at its old position until the component happens to be re-created.
  watch(
    () => [props.entry.startMinutes, props.entry.endMinutes] as const,
    ([start, end]) => {
      if (dragEdge === null) {
        localStart.value = start
        localEnd.value = end
      }
    },
  )

  function startDrag(edge: DragEdge, e: MouseEvent) {
    // Only the primary button drags. Without this a right-click's mousedown
    // started a "move" drag whose mouseup was treated as a click, so
    // right-clicking an entry opened the editor — on top of the context menu.
    if (e.button !== 0) {
      return
    }

    e.preventDefault()
    e.stopPropagation()
    dragEdge = edge
    dragOriginY = e.clientY
    dragOriginStart = localStart.value
    dragOriginEnd = localEnd.value
    didMove = false
    suppressClick = false
    pressStartedOnBody = edge === 'move'
    document.addEventListener('mousemove', onMove)
    document.addEventListener('mouseup', onUp)
  }

  function onMove(e: MouseEvent) {
    const deltaY = e.clientY - dragOriginY
    const deltaMins = snapMinutes((deltaY / hourHeight.value) * 60)
    const duration = dragOriginEnd - dragOriginStart

    if (deltaMins !== 0) {
      didMove = true
    }

    if (dragEdge === 'bottom') {
      // `minEnd` is the hard floor: an entry must never end before it starts.
      // The timeline-end clamp below has to respect it — a block that already
      // sits at or past `timelineEndHour` (an auto-accepted suggestion after
      // 22:00, say) otherwise collapses its end down to the timeline end and
      // writes a negative duration.
      const minEnd = dragOriginStart + 5

      localEnd.value = Math.min(
        Math.max(dragOriginEnd + deltaMins, minEnd),
        Math.max(endMin.value, minEnd),
      )
    } else if (dragEdge === 'top') {
      // Same invariant as the bottom edge: never let the start overtake the end.
      // A block that already sits above `timelineStartHour` would otherwise be
      // pushed down to the timeline start and end up inverted.
      const maxStart = dragOriginEnd - 5

      localStart.value = Math.max(
        Math.min(dragOriginStart + deltaMins, maxStart),
        Math.min(startMin.value, maxStart),
      )
    } else {
      // Moving keeps the duration intact, so this cannot invert — but it must
      // not be able to leave the block somewhere unreachable either.
      let ns = dragOriginStart + deltaMins
      ns = Math.max(ns, startMin.value)
      ns = Math.min(ns, Math.max(endMin.value - duration, startMin.value))
      localStart.value = ns
      localEnd.value = ns + duration
    }
  }

  async function onUp(e: MouseEvent) {
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('mouseup', onUp)
    dragEdge = null

    if (didMove) {
      // Any real drag consumes the trailing click, even when it lands back on
      // the original time range.
      suppressClick = pressStartedOnBody

      if (
        localStart.value !== props.entry.startMinutes ||
        localEnd.value !== props.entry.endMinutes
      ) {
        await dayStore.updateEntry(
          props.entry.id,
          props.entry.projectId,
          localStart.value,
          localEnd.value,
          props.entry.note,
        )
      }

      return
    }

    // A press with no movement is a click, whichever part of the block it landed
    // on. Routing the handles through the same path is what makes a 2-minute
    // entry — all handle, no body — usable with ctrl/shift.
    suppressClick = pressStartedOnBody
    emit('select', props.entry, e)
  }

  function onClick(e: MouseEvent) {
    if (suppressClick) {
      suppressClick = false
      return
    }

    // Whether this adds to the selection or replaces it depends on the
    // modifiers and on what is already selected, which only the parent knows.
    emit('select', props.entry, e)
  }

  /**
   * A double-click is the only way to open the editor with the mouse, so it is
   * handled on the whole block — a 2-minute entry is all resize handle, and its
   * body is too small to be a reliable double-click target.
   */
  function onDblClick(e: MouseEvent) {
    e.stopPropagation()
    emit('edit', props.entry)
  }

  /**
   * The menu *items* live in the parent: whether "merge with previous" applies
   * depends on the neighbouring entries, which this component never sees.
   */
  function onContextMenu(e: MouseEvent) {
    e.preventDefault()
    e.stopPropagation()
    emit('context-menu', props.entry, e)
  }

  onUnmounted(() => {
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('mouseup', onUp)
  })
</script>

<template>
  <div
    class="time-block"
    :class="{ selected }"
    :style="{ top: top + 'px', height: height + 'px', '--color': color }"
    :data-tooltip="tooltip"
    :data-tooltip-color="color"
    @contextmenu="onContextMenu"
    @dblclick="onDblClick"
  >
    <div
      class="handle handle-top"
      :style="{ height: handleHeight + 'px' }"
      @mousedown.stop="startDrag('top', $event)"
    />
    <div class="block-body" @mousedown.stop="startDrag('move', $event)" @click.stop="onClick">
      <span v-if="height > 22" class="project-name">{{ project?.name ?? '(no project)' }}</span>
      <span v-if="height > 38" class="duration">
        {{ minutesToTime(localStart) }} – {{ minutesToTime(localEnd) }} ·
        {{ formatDuration(durationMin) }}
      </span>
      <span v-if="height > 54 && entry.note" class="note">{{ entry.note }}</span>
    </div>
    <div
      class="handle handle-bottom"
      :style="{ height: handleHeight + 'px' }"
      @mousedown.stop="startDrag('bottom', $event)"
    />
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

  .time-block.selected {
    z-index: 12;
    box-shadow:
      0 0 0 2px var(--surface),
      0 0 0 4px var(--primary);
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
