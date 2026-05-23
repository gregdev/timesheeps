<script setup lang="ts">
  import { ref, onMounted, watch } from 'vue'
  import type { TimeEntry } from '../schemas'
  import { useDayStore } from '../stores/day'
  import { useTimeline } from '../composables/useTimeline'
  import { useEntryModal } from '../composables/useEntryModal'
  import TimeRuler from './TimeRuler.vue'
  import ActivityBlockItem from './ActivityBlockItem.vue'
  import EntryTrack from './EntryTrack.vue'
  import ProjectPickerModal from './ProjectPickerModal.vue'

  defineOptions({ inheritAttrs: false })

  const dayStore = useDayStore()
  const { totalHeight, hours, minuteToY } = useTimeline()
  const { pendingCreate, editingEntry } = useEntryModal()

  const scrollRef = ref<HTMLElement>()

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
    startMinutes: number,
    endMinutes: number,
    note: string,
  ) {
    if (editingEntry.value) {
      await dayStore.updateEntry(editingEntry.value.id, projectId, startMinutes, endMinutes, note)
      editingEntry.value = null
    } else if (pendingCreate.value) {
      await dayStore.createEntry(projectId, startMinutes, endMinutes, note)
      pendingCreate.value = null
    }
  }

  async function onModalDelete(id: number) {
    await dayStore.deleteEntry(id)
    editingEntry.value = null
  }

  function onModalCancel() {
    pendingCreate.value = null
    editingEntry.value = null
  }

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
      <div class="col-header">Activity</div>
      <div class="col-header entry-header">My Time</div>
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
        <div class="activity-track" :style="{ height: totalHeight + 'px' }">
          <TransitionGroup name="activity">
            <ActivityBlockItem
              v-for="(block, i) in dayStore.activityBlocks"
              :key="block.startedAt"
              :style="{ '--i': i }"
              :block="block"
            />
          </TransitionGroup>
          <div v-if="!dayStore.loading && dayStore.activityBlocks.length === 0" class="empty-track">
            No activity recorded
          </div>
        </div>

        <!-- Divider -->
        <div class="track-divider" />

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
    v-if="pendingCreate || editingEntry"
    :initial-start="(pendingCreate ?? editingEntry)!.startMinutes"
    :initial-end="(pendingCreate ?? editingEntry)!.endMinutes"
    :initial-project-id="editingEntry?.projectId ?? pendingCreate?.projectId ?? null"
    :initial-note="editingEntry?.note ?? pendingCreate?.note ?? ''"
    :entry-id="editingEntry?.id ?? null"
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
    width: 1px;
    background: var(--border);
    flex-shrink: 0;
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
</style>
