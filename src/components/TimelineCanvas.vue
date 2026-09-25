<script setup lang="ts">
  import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
  import type { TimeEntry } from '../schemas'
  import { useDayStore } from '../stores/day'
  import { useSettingsStore } from '../stores/settings'
  import { useTimeline } from '../composables/useTimeline'
  import { useEntryModal, type EntryRange } from '../composables/useEntryModal'
  import { useContextMenu } from '../composables/useContextMenu'
  import TimeRuler from './TimeRuler.vue'
  import ActivityBlockItem from './ActivityBlockItem.vue'
  import EntryTrack from './EntryTrack.vue'
  import ProjectPickerModal from './ProjectPickerModal.vue'

  defineOptions({ inheritAttrs: false })

  const dayStore = useDayStore()
  const settingsStore = useSettingsStore()
  const { totalHeight, hours, minuteToY, isoToMinutes } = useTimeline()
  const { pendingCreate, editingEntry } = useEntryModal()
  const { open: openMenu } = useContextMenu()

  const scrollRef = ref<HTMLElement>()

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
      await dayStore.createEntries(projectId, ranges, note)

      // Create auto-track rule if toggle was enabled
      if (autoTrack && pendingCreate.value.autoTrackAppName) {
        await createAutoTrackRule(projectId, pendingCreate.value.autoTrackAppName)
      }

      pendingCreate.value = null
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

    const choice = window.confirm(
      `"${appName}" is already matched to a different project by the rule "${conflict.name}".\n\n` +
        `OK — reassign that rule to this project instead.\n` +
        `Cancel — keep it and add this project's rule below it in priority order.`,
    )

    if (choice) {
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
  })

  // Reset scroll to 8am when navigating to a different day
  watch(
    () => dayStore.selectedDate,
    () => {
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
      <div class="col-header entry-header" style="flex: 1 1 0%">My Time</div>
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
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }

  .ruler-spacer {
    width: 44px;
    flex-shrink: 0;
  }

  .col-header {
    flex: 1;
    padding: 6px 8px;
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-faint);
    border-left: 1px solid var(--border);
  }

  .entry-header {
    border-right: none;
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
