<script setup lang="ts">
  import { ref, computed, nextTick, onMounted, onUnmounted, watch } from 'vue'
  import type { TimeEntry } from '../schemas'
  import { useDayStore } from '../stores/day'
  import { useSettingsStore } from '../stores/settings'
  import { useTimeline } from '../composables/useTimeline'
  import { useEntryModal, type EntryRange } from '../composables/useEntryModal'
  import { useContextMenu } from '../composables/useContextMenu'
  import { useEntrySelection } from '../composables/useEntrySelection'
  import { useProjectsStore } from '../stores/projects'
  import TimeRuler from './TimeRuler.vue'
  import ActivityBlockItem from './ActivityBlockItem.vue'
  import EntryTrack from './EntryTrack.vue'
  import ProjectPickerModal from './ProjectPickerModal.vue'
  import ConfirmDialog from './ConfirmDialog.vue'

  defineOptions({ inheritAttrs: false })

  const dayStore = useDayStore()
  const settingsStore = useSettingsStore()
  const projectsStore = useProjectsStore()
  const {
    totalHeight,
    hours,
    minuteToY,
    yToMinute,
    zoomBy,
    isoToMinutes,
    minutesToTime,
    formatDuration,
  } = useTimeline()
  const { pendingCreate, editingEntry } = useEntryModal()
  const { open: openMenu } = useContextMenu()
  const { selectedIds, selectionCount, clear: clearSelection } = useEntrySelection()

  const scrollRef = ref<HTMLElement>()

  // ---- selection ----
  /**
   * Why the current selection cannot be merged, or null.
   *
   * Computed here rather than tracked as an error: the header disables the
   * button and shows the reason on hover, so nothing is attempted and rejected.
   */
  const mergeIssue = computed(() => dayStore.mergeBlocker(selectedIds.value))
  const mergeError = ref<string | null>(null)
  let mergeErrorTimer: ReturnType<typeof setTimeout> | null = null

  async function onMergeSelected() {
    const ids = [...selectedIds.value]
    const result = await dayStore.mergeEntries(ids)

    if (result.ok) {
      clearSelection()
      return
    }

    mergeError.value = result.reason ?? 'Could not merge the selected entries.'

    if (mergeErrorTimer) {
      clearTimeout(mergeErrorTimer)
    }

    mergeErrorTimer = setTimeout(() => {
      mergeError.value = null
    }, 4000)
  }

  function onSelectionKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && selectionCount.value > 0) {
      clearSelection()
      return
    }

    if (e.key !== 'Delete') {
      return
    }

    // Never steal Delete from a text field (the timer note, the picker's
    // project search) and never stack a prompt on top of an open modal.
    const target = e.target as HTMLElement | null
    const typing =
      target !== null &&
      (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable)

    if (typing || deletePrompt.value || editingEntry.value || pendingCreate.value) {
      return
    }

    if (selectionCount.value === 0) {
      return
    }

    e.preventDefault()
    promptDeleteEntries([...selectedIds.value])
  }

  // ---- deletion ----
  interface DeletePrompt {
    ids: number[]
    title: string
    message: string
    chips: string[]
  }

  const deletePrompt = ref<DeletePrompt | null>(null)

  function chipsFor(entries: TimeEntry[]): string[] {
    const shown = entries.slice(0, 6).map((e) => {
      const project = projectsStore.byId(e.projectId)?.name ?? 'No project'

      return `${project} ${minutesToTime(e.startMinutes)}–${minutesToTime(e.endMinutes)}`
    })

    if (entries.length > shown.length) {
      shown.push(`+${entries.length - shown.length} more`)
    }

    return shown
  }

  /** Open the confirmation for an explicit set of ids. */
  function promptDeleteEntries(ids: number[]) {
    const entries = dayStore.timeEntries
      .filter((e) => ids.includes(e.id))
      .sort((a, b) => a.startMinutes - b.startMinutes)

    if (entries.length === 0) {
      return
    }

    const totalMin = entries.reduce((sum, e) => sum + (e.endMinutes - e.startMinutes), 0)
    const what = entries.length === 1 ? '1 time entry' : `${entries.length} time entries`

    deletePrompt.value = {
      ids: entries.map((e) => e.id),
      title: entries.length === 1 ? 'Delete time entry?' : `Delete ${entries.length} entries?`,
      message: `${what} · ${formatDuration(totalMin)} will be removed from this day.`,
      chips: chipsFor(entries),
    }
  }

  /** Open the confirmation for the current selection. */
  function onDeleteSelected() {
    promptDeleteEntries([...selectedIds.value])
  }

  /**
   * Close without deleting.
   *
   * Escape/✕/backdrop (`dismiss`) and the Cancel button (`cancel`) are the same
   * outcome here — but they are kept as separate events so a prompt where the
   * secondary button *does* something cannot be dismissed into that action.
   */
  function closeDeletePrompt() {
    deletePrompt.value = null
  }

  function onDeleteEntry(entry: TimeEntry) {
    promptDeleteEntries([entry.id])
  }

  async function confirmDelete() {
    const ids = deletePrompt.value?.ids ?? []
    deletePrompt.value = null

    if (ids.length === 0) {
      return
    }

    await dayStore.deleteEntries(ids)
    clearSelection()
  }

  // ---- zoom ----
  /**
   * Ctrl+wheel zooms around the pointer.
   *
   * `preventDefault` is essential: WebView2 otherwise treats ctrl+wheel as a
   * page zoom and scales the whole UI out from under the timeline. The minute
   * under the cursor is captured before the change and scrolled back to
   * afterwards, so the view does not jump.
   */
  async function onWheel(e: WheelEvent) {
    if (!e.ctrlKey && !e.metaKey) {
      return
    }

    const scroller = scrollRef.value

    if (!scroller) {
      return
    }

    e.preventDefault()

    const pointerOffset = e.clientY - scroller.getBoundingClientRect().top
    const minute = yToMinute(scroller.scrollTop + pointerOffset)

    zoomBy(e.deltaY < 0 ? 1.15 : 1 / 1.15)

    // The content is taller than the viewport, so the new scrollTop must be
    // applied after the height has been laid out or it clamps to the old one.
    await nextTick()
    scroller.scrollTop = minuteToY(minute) - pointerOffset
  }

  // ---- column resize ----
  const splitRatio = ref(settingsStore.settings.timelineColSplitPct / 100)
  const isResizing = ref(false)

  const activityPct = computed(() => Math.round(splitRatio.value * 100))

  // Settings load asynchronously, so the persisted split can arrive after this
  // component mounts (and can also be changed from the Settings page).
  watch(
    () => settingsStore.settings.timelineColSplitPct,
    (pct) => {
      if (!isResizing.value) {
        splitRatio.value = pct / 100
      }
    },
  )

  function onResizeStart(e: MouseEvent) {
    if (e.button !== 0) {
      return
    }

    e.preventDefault()
    isResizing.value = true
    document.body.style.cursor = 'col-resize'
    document.body.style.userSelect = 'none'
    document.addEventListener('mousemove', onResizeMove)
    document.addEventListener('mouseup', onResizeEnd)
  }

  function onResizeMove(e: MouseEvent) {
    if (!scrollRef.value) {
      return
    }

    const body = scrollRef.value.querySelector('.timeline-body') as HTMLElement

    if (!body) {
      return
    }

    const rect = body.getBoundingClientRect()
    const rulerW = 44 // .ruler-spacer width
    const handleW = 6 // .track-divider width
    const available = rect.width - rulerW - handleW

    if (available <= 0) {
      return
    }

    let ratio = (e.clientX - rect.left - rulerW) / available
    ratio = Math.max(0.15, Math.min(0.85, ratio))
    splitRatio.value = ratio
  }

  function onResizeEnd() {
    isResizing.value = false
    document.body.style.cursor = ''
    document.body.style.userSelect = ''
    document.removeEventListener('mousemove', onResizeMove)
    document.removeEventListener('mouseup', onResizeEnd)
    // persist
    settingsStore.save({
      ...settingsStore.settings,
      timelineColSplitPct: Math.round(splitRatio.value * 100),
    })
  }

  function onHandleContextMenu(e: MouseEvent) {
    openMenu(e, [
      {
        label: 'Reset to default (50/50)',
        action: () => {
          splitRatio.value = 0.5
          settingsStore.save({
            ...settingsStore.settings,
            timelineColSplitPct: 50,
          })
        },
      },
    ])
  }

  onUnmounted(() => {
    document.body.style.cursor = ''
    document.body.style.userSelect = ''
    document.removeEventListener('mousemove', onResizeMove)
    document.removeEventListener('mouseup', onResizeEnd)
  })

  function onRequestCreate(start: number, end: number) {
    pendingCreate.value = { startMinutes: start, endMinutes: end, note: '' }
  }

  function onAcceptSuggestion(projectId: number, startMinutes: number, endMinutes: number) {
    pendingCreate.value = { startMinutes, endMinutes, note: '', projectId }
  }

  function onEditEntry(entry: TimeEntry) {
    editingEntry.value = entry
  }

  async function onModalSave(
    projectId: number,
    ranges: EntryRange[],
    note: string,
    autoTrack: boolean,
  ) {
    if (editingEntry.value) {
      // Editing always targets one existing entry, so only one range can apply.
      const [range] = ranges

      await dayStore.updateEntry(
        editingEntry.value.id,
        projectId,
        range.startMinutes,
        range.endMinutes,
        note,
      )
      editingEntry.value = null
    } else if (pendingCreate.value) {
      const autoTrackAppName = pendingCreate.value.autoTrackAppName

      // Close the picker *before* any follow-up prompt. The auto-track conflict
      // dialog is a sibling modal, and leaving `pendingCreate` set would render
      // both at once with independent Escape/focus handling.
      pendingCreate.value = null

      await dayStore.createEntries(projectId, ranges, note)

      // Create auto-track rule if toggle was enabled
      if (autoTrack && autoTrackAppName) {
        await createAutoTrackRule(projectId, autoTrackAppName)
      }
    }
  }

  /**
   * Add an "app name is X" rule for the project, but never silently stack up a
   * second rule when another project already claims that app — the older rule
   * would keep winning and the new one would look broken.
   */
  async function createAutoTrackRule(projectId: number, appName: string) {
    const conflict = settingsStore.findAppNameConflict(appName, projectId)

    if (!conflict) {
      await settingsStore.createMatchRule(projectId, appName, buildAppNameConditions(appName))
      return
    }

    const choice = await askAppConflict({
      appName,
      conflictId: conflict.id,
      conflictName: conflict.name,
      conflictProjectName: projectsStore.byId(conflict.projectId)?.name ?? 'another project',
      projectName: projectsStore.byId(projectId)?.name ?? 'this project',
    })

    // Dismissed (✕ / Escape / backdrop) or dismissed via the secondary button:
    // check which, because the secondary button DOES create a rule.
    if (choice === null) {
      return
    }

    if (choice === 'reassign') {
      await settingsStore.updateMatchRule(
        conflict.id,
        conflict.name,
        buildAppNameConditions(appName),
      )
      await settingsStore.reorderMatchRules([
        conflict.id,
        ...settingsStore.orderedMatchRules.map((r) => r.id).filter((id) => id !== conflict.id),
      ])
    } else {
      // Give the new rule the project it was actually created for, then trust
      // priority to resolve the overlap.
      await settingsStore.createMatchRule(projectId, appName, buildAppNameConditions(appName))
    }

    await settingsStore.refreshRuleStats()
  }

  // ---- auto-track rule conflict ----
  type AppConflictChoice = 'reassign' | 'add-below'

  interface AppConflictPrompt {
    appName: string
    conflictId: number
    conflictName: string
    conflictProjectName: string
    projectName: string
  }

  const appConflict = ref<AppConflictPrompt | null>(null)
  /** Resolver for the in-flight `askAppConflict`, if any. */
  let settleAppConflict: ((choice: AppConflictChoice | null) => void) | null = null

  /**
   * Ask which way to resolve a duplicate app rule, as a promise.
   *
   * The dialog is rendered declaratively like every other modal; the promise is
   * what lets `createAutoTrackRule` stay a straight-line async function instead
   * of a state machine.
   *
   * Resolves `null` when the user dismissed the prompt without choosing.
   */
  function askAppConflict(prompt: AppConflictPrompt): Promise<AppConflictChoice | null> {
    return new Promise((resolve) => {
      settleAppConflict = resolve
      appConflict.value = prompt
    })
  }

  function resolveAppConflict(choice: AppConflictChoice | null) {
    appConflict.value = null
    const settle = settleAppConflict
    settleAppConflict = null
    settle?.(choice)
  }

  function buildAppNameConditions(appName: string) {
    return [
      { field: 'app_name' as const, operator: 'equals' as const, value: appName, negate: false },
    ]
  }

  async function onModalDelete(id: number) {
    await dayStore.deleteEntry(id)
    editingEntry.value = null
  }

  function onModalCancel() {
    pendingCreate.value = null
    editingEntry.value = null
  }

  /** Whichever entry the picker is editing/creating — it covers both flows. */
  const modalTarget = computed(() => pendingCreate.value ?? editingEntry.value)

  /**
   * The ranges the picker will save. A Window Activity row can cover several
   * separate sessions; every other entry point is a single range.
   */
  const modalRanges = computed<EntryRange[]>(() => {
    if (editingEntry.value) {
      return [
        {
          startMinutes: editingEntry.value.startMinutes,
          endMinutes: editingEntry.value.endMinutes,
        },
      ]
    }

    const pending = pendingCreate.value

    if (!pending) {
      return []
    }

    return pending.ranges?.length
      ? pending.ranges
      : [{ startMinutes: pending.startMinutes, endMinutes: pending.endMinutes }]
  })

  const modalProjectId = computed<number | null>(() => modalTarget.value?.projectId ?? null)

  onMounted(() => {
    // Scroll to 8am by default
    if (scrollRef.value) {
      scrollRef.value.scrollTop = minuteToY(8 * 60) - 40
    }

    // Registered by hand so it is non-passive: `preventDefault` on a passive
    // wheel listener is ignored, and without it the webview page-zooms.
    scrollRef.value?.addEventListener('wheel', onWheel, { passive: false })
    document.addEventListener('keydown', onSelectionKeydown)
  })

  onUnmounted(() => {
    scrollRef.value?.removeEventListener('wheel', onWheel)
    document.removeEventListener('keydown', onSelectionKeydown)

    if (mergeErrorTimer) {
      clearTimeout(mergeErrorTimer)
    }
  })

  // A different day has entirely different entries, so a selection cannot carry
  // over — the ids would either vanish or point at unrelated rows.
  watch(
    () => dayStore.selectedDate,
    () => {
      clearSelection()

      if (scrollRef.value) {
        scrollRef.value.scrollTop = minuteToY(8 * 60) - 40
      }
    },
  )
</script>

<template>
  <div class="timeline-canvas" v-bind="$attrs">
    <!-- Column headers -->
    <div class="col-headers">
      <div class="ruler-spacer" />
      <div class="col-header" :style="{ flex: `0 0 ${activityPct}%` }">Activity</div>
      <div class="resize-spacer" />
      <div class="col-header entry-header" style="flex: 1 1 0%">
        <template v-if="selectionCount > 0">
          <span class="selection-label">{{ selectionCount }} selected</span>
          <button
            class="selection-btn"
            :disabled="mergeIssue !== null"
            :title="mergeIssue ?? 'Merge the selected entries into one block'"
            @click="onMergeSelected"
          >
            Merge
          </button>
          <button class="selection-btn" @click="clearSelection">Clear</button>
          <span v-if="mergeError" class="selection-error">{{ mergeError }}</span>
        </template>
        <template v-else>My Time</template>
      </div>
    </div>

    <!-- Scrollable timeline body -->
    <div ref="scrollRef" class="scroll-area">
      <div class="timeline-body" :style="{ height: totalHeight + 'px' }">
        <!-- Hour grid lines -->
        <div class="grid-lines">
          <div
            v-for="hour in hours"
            :key="hour"
            class="grid-line"
            :style="{ top: minuteToY(hour * 60) + 'px' }"
          />
        </div>

        <!-- Time ruler -->
        <TimeRuler />

        <!-- Activity track (left, read-only) -->
        <div
          class="activity-track"
          :style="{ height: totalHeight + 'px', flex: `0 0 ${activityPct}%` }"
        >
          <TransitionGroup name="activity">
            <ActivityBlockItem
              v-for="(block, i) in dayStore.activityBlocks"
              :key="block.startedAt"
              :style="{ '--i': i }"
              :block="block"
            />
          </TransitionGroup>

          <!-- Calendar event markers -->
          <div
            v-for="event in dayStore.calendarEvents"
            :key="event.startAt"
            class="calendar-event-marker"
            :style="{
              top: minuteToY(isoToMinutes(event.startAt)) + 'px',
              height:
                Math.max(
                  4,
                  minuteToY(isoToMinutes(event.endAt)) - minuteToY(isoToMinutes(event.startAt)),
                ) + 'px',
            }"
            :title="`${event.subject}${event.location ? ' @ ' + event.location : ''}${event.isTeamsMeeting ? ' (Teams)' : ''}`"
          >
            <span class="cal-event-label">{{ event.subject }}</span>
          </div>

          <div v-if="!dayStore.loading && dayStore.activityBlocks.length === 0" class="empty-track">
            No activity recorded
          </div>
        </div>

        <!-- Resize handle between columns -->
        <div
          class="track-divider"
          :class="{ resizing: isResizing }"
          @mousedown="onResizeStart"
          @contextmenu="onHandleContextMenu"
        />

        <!-- Entry track (right, user drags to create) -->
        <EntryTrack
          :entries="dayStore.timeEntries"
          :suggestions="dayStore.suggestedEntries"
          @request-create="onRequestCreate"
          @edit="onEditEntry"
          @accept-suggestion="onAcceptSuggestion"
          @merge-selection="onMergeSelected"
          @delete-selection="onDeleteSelected"
          @delete-entry="onDeleteEntry"
        />
      </div>
    </div>
  </div>

  <ProjectPickerModal
    v-if="modalTarget"
    :initial-ranges="modalRanges"
    :initial-project-id="modalProjectId"
    :initial-note="modalTarget.note"
    :entry-id="editingEntry?.id ?? null"
    :auto-track-app-name="pendingCreate?.autoTrackAppName"
    :initial-auto-track="pendingCreate?.autoTrackEnabled"
    @save="onModalSave"
    @delete="onModalDelete"
    @cancel="onModalCancel"
  />

  <ConfirmDialog
    v-if="deletePrompt"
    :title="deletePrompt.title"
    :message="deletePrompt.message"
    :chips="deletePrompt.chips"
    confirm-label="Delete"
    @confirm="confirmDelete"
    @cancel="closeDeletePrompt"
    @dismiss="closeDeletePrompt"
  />

  <ConfirmDialog
    v-if="appConflict"
    title="App is already tracked"
    :message="
      `“${appConflict.appName}” is already matched to ${appConflict.conflictProjectName} by the rule ` +
      `“${appConflict.conflictName}”. Reassign that rule to ${appConflict.projectName}, or keep it and ` +
      `add ${appConflict.projectName}'s rule below it in priority order?`
    "
    confirm-label="Reassign rule"
    cancel-label="Add below it"
    link-label="Cancel"
    :danger="false"
    hint="Closing this creates no rule."
    @confirm="resolveAppConflict('reassign')"
    @cancel="resolveAppConflict('add-below')"
    @dismiss="resolveAppConflict(null)"
  />
</template>

<style scoped>
  .timeline-canvas {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .col-headers {
    display: flex;
    flex-shrink: 0;

    /*
     * Fixed, not content-driven. The selection toolbar (11px buttons at 19.4px
     * tall) is taller than the bare "My Time" label, so a content-sized row grew
     * by ~4px the moment anything was selected — which shifted the whole
     * timeline body under the cursor. Sized for the taller of the two states.
     */
    height: 32px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }

  .ruler-spacer {
    width: 44px;
    flex-shrink: 0;
  }

  .col-header {
    flex: 1;

    /* Vertical centring replaces the old 6px padding, so the fixed height and
       the border-left both apply to the full row. */
    display: flex;
    align-items: center;
    padding: 0 8px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-faint);
    border-left: 1px solid var(--border);
  }

  .entry-header {
    border-right: none;
    gap: 8px;
    text-transform: none;
  }

  .selection-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--text);
  }

  .selection-btn {
    font-size: 11px;
    padding: 2px 8px;
    min-height: 0;
    line-height: 1.4;
  }

  .selection-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .selection-error {
    font-size: 11px;
    color: var(--danger, #dc2626);
    text-transform: none;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .resize-spacer {
    width: 6px;
    flex-shrink: 0;
  }

  .scroll-area {
    flex: 1;
    overflow: hidden auto;
    background: var(--bg);
  }

  .timeline-body {
    position: relative;
    display: flex;
    min-width: 0;
  }

  .grid-lines {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 0;
  }

  .grid-line {
    position: absolute;
    left: 0;
    right: 0;
    height: 1px;
    background: var(--grid-line);
  }

  .activity-track {
    position: relative;
    flex: 1;
    overflow: hidden;
    border-left: 1px solid var(--border);
    min-width: 0;
  }

  .track-divider {
    width: 6px;
    background: var(--border);
    flex-shrink: 0;
    cursor: col-resize;
    position: relative;
    transition: background 0.15s;
    user-select: none;
  }

  .track-divider::after {
    content: '';
    position: absolute;
    inset: 0 2px;
    background: transparent;
    border-radius: 2px;
    transition: background 0.15s;
  }

  .track-divider:hover::after,
  .track-divider.resizing::after {
    background: var(--primary);
    opacity: 0.4;
  }

  .empty-track {
    position: absolute;
    top: 60px;
    left: 50%;
    transform: translateX(-50%);
    font-size: 12px;
    color: var(--text-faint);
    white-space: nowrap;
  }

  .calendar-event-marker {
    position: absolute;
    left: 2px;
    right: 2px;
    background: color-mix(in srgb, #3b82f6 25%, transparent);
    border-left: 3px solid #3b82f6;
    border-radius: 2px;
    overflow: hidden;
    pointer-events: none;
    z-index: 2;
    min-height: 4px;
  }

  .cal-event-label {
    font-size: 10px;
    color: #1d4ed8;
    padding: 0 4px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    display: block;
    line-height: 1.3;
  }

  @media (prefers-color-scheme: dark) {
    .cal-event-label {
      color: #93c5fd;
    }
  }

  html[data-theme='dark'] .cal-event-label {
    color: #93c5fd;
  }

  html[data-theme='light'] .cal-event-label {
    color: #1d4ed8;
  }
</style>
