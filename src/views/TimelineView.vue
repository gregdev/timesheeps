<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import DayNav from '../components/DayNav.vue'
import TimelineCanvas from '../components/TimelineCanvas.vue'
import { useDayStore } from '../stores/day'
import { useProjectsStore } from '../stores/projects'
import { useTimeline } from '../composables/useTimeline'

const dayStore = useDayStore()
const projectsStore = useProjectsStore()
const { formatDuration } = useTimeline()

const summaryItems = computed(() =>
  [...dayStore.summary.entries()].map(([projectId, mins]) => ({
    project: projectsStore.byId(projectId),
    mins,
  })).sort((a, b) => b.mins - a.mins)
)

const totalMins = computed(() =>
  summaryItems.value.reduce((sum, item) => sum + item.mins, 0)
)

// Auto-refresh activity when viewing today
let refreshTimer: ReturnType<typeof setInterval> | null = null
onMounted(() => {
  refreshTimer = setInterval(() => {
    if (dayStore.isViewingToday) dayStore.loadDay()
  }, 30_000)
})
onUnmounted(() => {
  if (refreshTimer) clearInterval(refreshTimer)
})
</script>

<template>
  <div class="timeline-view">
    <DayNav />

    <div v-if="dayStore.loading" class="loading">Loading…</div>

    <TimelineCanvas v-else />

    <!-- Day summary bar -->
    <div v-if="summaryItems.length > 0" class="summary-bar">
      <div v-for="item in summaryItems" :key="item.project?.id" class="summary-item">
        <span class="dot" :style="{ background: item.project?.color ?? '#6366f1' }" />
        <span class="proj-name">{{ item.project?.name ?? 'Unknown' }}</span>
        <span class="proj-time">{{ formatDuration(item.mins) }}</span>
      </div>
      <div class="summary-total">Total: {{ formatDuration(totalMins) }}</div>
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

.loading {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
}

.summary-bar {
  display: flex;
  align-items: center;
  gap: 16px;
  padding: 6px 16px;
  border-top: 1px solid var(--border);
  background: var(--surface);
  flex-shrink: 0;
  overflow-x: auto;
  flex-wrap: nowrap;
}

.summary-item {
  display: flex;
  align-items: center;
  gap: 5px;
  flex-shrink: 0;
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.proj-name {
  font-size: 12px;
  color: var(--text-muted);
}

.proj-time {
  font-size: 12px;
  font-weight: 600;
}

.summary-total {
  margin-left: auto;
  font-size: 12px;
  font-weight: 600;
  color: var(--text-muted);
  flex-shrink: 0;
}
</style>
