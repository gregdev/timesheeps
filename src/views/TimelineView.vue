<script setup lang="ts">
  import { onMounted, onUnmounted, ref } from 'vue'
  import { listen } from '@tauri-apps/api/event'
  import { format, parseISO } from 'date-fns'
  import DayNav from '../components/DayNav.vue'
  import TimelineCanvas from '../components/TimelineCanvas.vue'
  import WindowSummary from '../components/WindowSummary.vue'
  import ProjectSummary from '../components/ProjectSummary.vue'
  import { useDayStore } from '../stores/day'
  import { useProjectsStore } from '../stores/projects'
  import { useSettingsStore } from '../stores/settings'
  import { useTimeline } from '../composables/useTimeline'
  import { useContextMenu } from '../composables/useContextMenu'

  const dayStore = useDayStore()
  const projectsStore = useProjectsStore()
  const settingsStore = useSettingsStore()
  const { formatDuration, minutesToTime } = useTimeline()
  const { open: openMenu } = useContextMenu()
  const copied = ref(false)

  // ---- panel resize ----
  const windowSummaryWidth = ref(settingsStore.settings.layoutWindowSummaryWidth)
  const projectSummaryWidth = ref(settingsStore.settings.layoutProjectSummaryWidth)
  const resizingHandle = ref<'ws' | 'ps' | null>(null)

  function onResizeStart(handle: 'ws' | 'ps', e: MouseEvent) {
    if (e.button !== 0) return
    e.preventDefault()
    resizingHandle.value = handle
    document.body.style.cursor = 'col-resize'
    document.body.style.userSelect = 'none'
    document.addEventListener('mousemove', onResizeMove)
    document.addEventListener('mouseup', onResizeEnd)
  }

  function onResizeMove(e: MouseEvent) {
    if (resizingHandle.value === 'ws') {
      // Resize timeline-canvas ↔ window-summary; project-summary unchanged
      const area = document.querySelector('.main-area') as HTMLElement | null
      if (!area) return
      const rect = area.getBoundingClientRect()
      // windowSummaryWidth = distance from right edge, minus project-summary and handle widths
      const psW = projectSummaryWidth.value
      const handleW = 6
      const newWsW = rect.right - e.clientX - psW - handleW
      windowSummaryWidth.value = Math.max(100, Math.min(500, Math.round(newWsW)))
    } else if (resizingHandle.value === 'ps') {
      const area = document.querySelector('.main-area') as HTMLElement | null
      if (!area) return
      const rect = area.getBoundingClientRect()
      const newPsW = rect.right - e.clientX
      projectSummaryWidth.value = Math.max(100, Math.min(500, Math.round(newPsW)))
    }
  }

  function onResizeEnd() {
    resizingHandle.value = null
    document.body.style.cursor = ''
    document.body.style.userSelect = ''
    document.removeEventListener('mousemove', onResizeMove)
    document.removeEventListener('mouseup', onResizeEnd)
    // persist
    settingsStore.save({
      ...settingsStore.settings,
      layoutWindowSummaryWidth: windowSummaryWidth.value,
      layoutProjectSummaryWidth: projectSummaryWidth.value,
    })
  }

  function onHandleContextMenu(handle: 'ws' | 'ps', e: MouseEvent) {
    const label =
      handle === 'ws'
        ? 'Reset Window Summary to default (220px)'
        : 'Reset Project Summary to default (220px)'
    openMenu(e, [
      {
        label,
        action: () => {
          if (handle === 'ws') {
            windowSummaryWidth.value = 220
          } else {
            projectSummaryWidth.value = 220
          }
          settingsStore.save({
            ...settingsStore.settings,
            layoutWindowSummaryWidth: windowSummaryWidth.value,
            layoutProjectSummaryWidth: projectSummaryWidth.value,
          })
        },
      },
    ])
  }

  function copyDay() {
    const date = format(parseISO(dayStore.selectedDate), 'EEEE, MMMM d, yyyy')
    const lines: string[] = [date, '']
    const entries = dayStore.timeEntries

    if (entries.length === 0) {
      lines.push('No time logged.')
    } else {
      for (const entry of entries) {
        const project = projectsStore.projects.find((p) => p.id === entry.projectId)
        const name = project?.name ?? 'Unknown'
        const start = minutesToTime(entry.startMinutes)
        const end = minutesToTime(entry.endMinutes)
        const dur = formatDuration(entry.endMinutes - entry.startMinutes)
        const note = entry.note ? `  ${entry.note}` : ''
        lines.push(`${name}  ${start} – ${end}  ${dur}${note}`)
      }

      const total = entries.reduce((sum, e) => sum + (e.endMinutes - e.startMinutes), 0)
      lines.push('')
      lines.push(`Total: ${formatDuration(total)}`)
    }

    navigator.clipboard.writeText(lines.join('\n'))
    copied.value = true
    setTimeout(() => {
      copied.value = false
    }, 2000)
  }

  // Auto-refresh activity when viewing today
  let refreshTimer: ReturnType<typeof setInterval> | null = null
  let unlistenActivityUpdated: (() => void) | null = null

  onMounted(async () => {
    refreshTimer = setInterval(() => {
      if (dayStore.isViewingToday) {
        dayStore.loadDay(undefined, true)
      }
    }, 30_000)

    // Also refresh immediately whenever the Rust poller writes new activity
    unlistenActivityUpdated = await listen('activity-updated', () => {
      if (dayStore.isViewingToday) {
        dayStore.loadDay(undefined, true)
      }
    })
  })

  onUnmounted(() => {
    if (refreshTimer) {
      clearInterval(refreshTimer)
    }
    if (unlistenActivityUpdated) {
      unlistenActivityUpdated()
    }
    document.body.style.cursor = ''
    document.body.style.userSelect = ''
    document.removeEventListener('mousemove', onResizeMove)
    document.removeEventListener('mouseup', onResizeEnd)
  })
</script>

<template>
  <div class="timeline-view">
    <div class="day-nav-area">
      <DayNav />
      <div class="day-actions">
        <button
          class="btn-ghost copy-btn"
          :title="copied ? 'Copied!' : 'Copy day summary (T = today, ←/→ = navigate)'"
          @click="copyDay"
        >
          {{ copied ? '✓ Copied' : 'Copy' }}
        </button>
      </div>
    </div>

    <div class="main-area">
      <TimelineCanvas class="timeline-col" />
      <div v-if="dayStore.loadError" class="load-error">
        Error loading data: {{ dayStore.loadError }}
      </div>
      <div
        class="panel-handle"
        :class="{ resizing: resizingHandle === 'ws' }"
        @mousedown="onResizeStart('ws', $event)"
        @contextmenu="onHandleContextMenu('ws', $event)"
      />
      <WindowSummary :style="{ width: windowSummaryWidth + 'px' }" />
      <div
        class="panel-handle"
        :class="{ resizing: resizingHandle === 'ps' }"
        @mousedown="onResizeStart('ps', $event)"
        @contextmenu="onHandleContextMenu('ps', $event)"
      />
      <ProjectSummary :style="{ width: projectSummaryWidth + 'px' }" />
    </div>
  </div>
</template>

<style scoped>
  .timeline-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    overflow: hidden;
  }

  .day-nav-area {
    position: relative;
    flex-shrink: 0;
  }

  .day-actions {
    position: absolute;
    top: 0;
    right: 10px;
    bottom: 0;
    display: flex;
    align-items: center;
    gap: var(--space-1);
  }

  .copy-btn {
    font-size: var(--text-xs);
    padding: var(--space-1) var(--space-2);
    white-space: nowrap;
  }

  .main-area {
    display: flex;
    flex: 1;
    overflow: hidden;
    position: relative;
  }

  .timeline-col {
    flex: 1 1 0%;
    min-width: 200px;
  }

  .panel-handle {
    width: 6px;
    flex-shrink: 0;
    cursor: col-resize;
    background: var(--border);
    position: relative;
    transition: background 0.15s;
    user-select: none;
  }

  .panel-handle::after {
    content: '';
    position: absolute;
    inset: 0 2px;
    background: transparent;
    border-radius: 2px;
    transition: background 0.15s;
  }

  .panel-handle:hover::after,
  .panel-handle.resizing::after {
    background: var(--primary);
    opacity: 0.4;
  }

  .load-error {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--danger);
    padding: var(--space-4);
    text-align: center;
    white-space: pre-wrap;
    background: var(--bg);
    z-index: 20;
  }
</style>
