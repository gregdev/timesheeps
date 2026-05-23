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
  import { useTimeline } from '../composables/useTimeline'

  const dayStore = useDayStore()
  const projectsStore = useProjectsStore()
  const { formatDuration, minutesToTime } = useTimeline()
  const copied = ref(false)

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
      <WindowSummary />
      <ProjectSummary />
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
    gap: 4px;
  }

  .copy-btn {
    font-size: 12px;
    padding: 4px 10px;
    white-space: nowrap;
  }

  .main-area {
    display: flex;
    flex: 1;
    overflow: hidden;
    position: relative;
  }

  .timeline-col {
    flex: 1;
    min-width: 0;
  }

  .load-error {
    position: absolute;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #ef4444;
    padding: 16px;
    text-align: center;
    white-space: pre-wrap;
    background: var(--bg);
    z-index: 20;
  }
</style>
