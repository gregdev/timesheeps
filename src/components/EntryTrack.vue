<script setup lang="ts">
  import { ref, computed, watch, onUnmounted } from 'vue'
  import type { TimeEntry } from '../schemas'
  import { useTimeline } from '../composables/useTimeline'
  import { useContextMenu } from '../composables/useContextMenu'
  import type { ContextMenuItem } from '../composables/useContextMenu'
  import { useEntrySelection } from '../composables/useEntrySelection'
  import { useDayStore } from '../stores/day'
  import TimeBlockItem from './TimeBlockItem.vue'
  import SuggestedBlockItem from './SuggestedBlockItem.vue'

  const props = defineProps<{
    entries: TimeEntry[]
    suggestions: { projectId: number; startMinutes: number; endMinutes: number }[]
  }>()
  const emit = defineEmits<{
    (e: 'request-create', startMinutes: number, endMinutes: number): void
    (e: 'edit', entry: TimeEntry): void
    (e: 'accept-suggestion', projectId: number, startMinutes: number, endMinutes: number): void
    (e: 'merge-selection'): void
    (e: 'delete-selection'): void
    (e: 'delete-entry', entry: TimeEntry): void
  }>()

  const dayStore = useDayStore()
  const { visible: menuVisible, open: openMenu } = useContextMenu()
  const { anchorId, selectedIds, isSelected, toggle, setAnchor, selectRange, clear, prune } =
    useEntrySelection()

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
    if (!trackRef.value) {
      return
    }

    const rect = trackRef.value.getBoundingClientRect()
    hoverY.value = e.clientY - rect.top
    isHovering.value = true
  }

  function onTrackMouseleave() {
    isHovering.value = false
  }

  /**
   * The entry `entry` may be folded into, or null.
   *
   * Only the entry immediately before it qualifies, and only when it is in the
   * same project and does not overlap it. The merge itself re-checks this in
   * `dayStore.mergeWithPrevious` — the entries can change under the open menu.
   */
  function mergeTarget(entry: TimeEntry): TimeEntry | null {
    const sorted = [...props.entries].sort((a, b) => a.startMinutes - b.startMinutes)
    const idx = sorted.findIndex((e) => e.id === entry.id)

    if (idx <= 0) {
      return null
    }

    const previous = sorted[idx - 1]

    return previous.projectId === entry.projectId && previous.endMinutes <= entry.startMinutes
      ? previous
      : null
  }

  /**
   * True when the current selection exists only to show which entry an open
   * context menu belongs to, and so should be dropped when that menu closes.
   *
   * Deliberately a plain variable, not a ref: it is bookkeeping for the watcher
   * below and must never trigger a re-render on its own.
   */
  let contextMenuSelection = false

  /**
   * Right-click menu for one entry, which becomes a bulk menu when the entry is
   * part of a multi-selection.
   *
   * Bulk actions are emitted rather than performed here: the timeline owns the
   * selection-clearing and the error reporting, and duplicating the merge
   * plumbing in two components is how the two paths drift apart.
   */
  function onEntryContextMenu(entry: TimeEntry, e: MouseEvent) {
    if (selectedIds.value.length > 1 && isSelected(entry.id)) {
      const ids = [...selectedIds.value]
      const count = ids.length
      const blocker = dayStore.mergeBlocker(ids)

      // The selection was built deliberately (or by an earlier menu), so the
      // bulk menu must not discard it on close.
      contextMenuSelection = false

      openMenu(e, [
        {
          label: `Merge ${count} entries`,
          // Disabled, with the reason spelled out, rather than offering an
          // action that would be refused after the click.
          disabled: blocker !== null,
          action: () => emit('merge-selection'),
        },
        {
          label: `Delete ${count} entries`,
          danger: true,
          action: () => emit('delete-selection'),
        },
        { label: 'Clear selection', action: clear },
      ])

      return
    }

    // Highlight the entry the menu belongs to, so it is obvious which one the
    // actions will hit. An entry that was already selected keeps its selection
    // afterwards; one selected *by* this right-click does not — otherwise every
    // right-click would leave a selection lying around.
    if (!isSelected(entry.id)) {
      selectRange([entry.id])
      setAnchor(entry.id)
      contextMenuSelection = true
    } else {
      contextMenuSelection = false
    }

    const previous = mergeTarget(entry)
    const items: ContextMenuItem[] = [{ label: 'Edit entry…', action: () => emit('edit', entry) }]

    if (previous) {
      items.push({
        label: 'Merge with previous entry',
        action: () => {
          void dayStore.mergeWithPrevious(entry.id)
        },
      })
    }

    items.push({
      label: 'Delete entry',
      danger: true,
      action: () => emit('delete-entry', entry),
    })

    openMenu(e, items)
  }

  /**
   * Drop the right-click highlight once the menu closes.
   *
   * `flush: 'sync'` matters: the menu closes on mousedown, and the very next
   * event may be a contextmenu on a different entry. A deferred callback would
   * land after that new selection and wipe it out.
   */
  watch(
    menuVisible,
    (open) => {
      if (open || !contextMenuSelection) {
        return
      }

      contextMenuSelection = false
      clear()
    },
    { flush: 'sync' },
  )

  /**
   * One click handler for every entry, whether it landed on the body or on a
   * resize handle.
   *
   *   ctrl/cmd+click → add or remove just this entry
   *   shift+click    → select everything between the anchor and this entry
   *   plain click    → select just this entry
   *
   * A single click never opens the editor; that is a double-click now, so a
   * stray click cannot interrupt what the user was doing. Every branch selects
   * *something*, which is why the anchor is set even for a plain click.
   */
  function onSelectEntry(entry: TimeEntry, e: MouseEvent) {
    if (e.ctrlKey || e.metaKey) {
      toggle(entry.id)
      return
    }

    if (e.shiftKey) {
      const ordered = [...props.entries].sort((a, b) => a.startMinutes - b.startMinutes)
      const to = ordered.findIndex((x) => x.id === entry.id)
      const from = anchorId.value === null ? -1 : ordered.findIndex((x) => x.id === anchorId.value)

      if (to < 0) {
        return
      }

      if (from < 0) {
        selectRange([entry.id])
        setAnchor(entry.id)
        return
      }

      const [lo, hi] = from < to ? [from, to] : [to, from]
      selectRange(ordered.slice(lo, hi + 1).map((x) => x.id))
      return
    }

    selectRange([entry.id])
    setAnchor(entry.id)
  }

  // Entries disappear on delete, on a day change and when a suggestion or edit
  // replaces them; a stale id would leave "3 selected" over nothing.
  watch(
    () => props.entries,
    (entries) => prune(entries.map((e) => e.id)),
    { deep: true, immediate: true },
  )

  // Drag-to-create state
  const isDragging = ref(false)
  const hasMoved = ref(false)
  const dragStartMin = ref(0)
  const dragCurrentMin = ref(0)

  const previewTop = computed(() => {
    if (!isDragging.value || !hasMoved.value) {
      return 0
    }

    return minuteToY(Math.min(dragStartMin.value, dragCurrentMin.value))
  })
  const previewHeight = computed(() => {
    if (!isDragging.value || !hasMoved.value) {
      return 0
    }

    return Math.abs(minuteToY(dragCurrentMin.value) - minuteToY(dragStartMin.value))
  })

  const dragTopMin = computed(() => Math.min(dragStartMin.value, dragCurrentMin.value))
  const dragBottomMin = computed(() => Math.max(dragStartMin.value, dragCurrentMin.value))
  const dragTopY = computed(() => minuteToY(dragTopMin.value))
  const dragBottomY = computed(() => minuteToY(dragBottomMin.value))

  function onTrackMousedown(e: MouseEvent) {
    if (e.button !== 0) {
      return
    }
    if ((e.target as HTMLElement).closest('.time-block')) {
      return
    }
    if (!trackRef.value) {
      return
    }

    // Pressing empty track starts a create-drag, so it also dismisses a
    // selection — otherwise the header keeps offering Merge while the user
    // works somewhere else.
    if (!e.ctrlKey && !e.metaKey) {
      clear()
    }

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
    if (!isDragging.value || !trackRef.value) {
      return
    }

    hasMoved.value = true
    const rect = trackRef.value.getBoundingClientRect()
    const relY = e.clientY - rect.top
    dragCurrentMin.value = snapMinutes(clampMin(yToMinute(relY)))
  }

  function onDocMouseup() {
    document.removeEventListener('mousemove', onDocMousemove)
    document.removeEventListener('mouseup', onDocMouseup)

    if (!isDragging.value) {
      return
    }

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
      :selected="isSelected(entry.id)"
      @edit="emit('edit', $event)"
      @select="onSelectEntry"
      @context-menu="onEntryContextMenu"
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
    padding: 2px var(--space-2);
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text);
    pointer-events: none;
    z-index: 7;
    white-space: nowrap;
    box-shadow: 0 1px 4px rgb(0 0 0 / 15%);
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
