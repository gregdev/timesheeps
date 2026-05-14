<script setup lang="ts">
import { computed } from 'vue'
import { format, parseISO } from 'date-fns'
import { useDayStore } from '../stores/day'

const dayStore = useDayStore()

const displayDate = computed(() => {
  const d = parseISO(dayStore.selectedDate)
  return format(d, 'EEEE, MMMM d, yyyy')
})
</script>

<template>
  <div class="day-nav">
    <button class="btn-ghost arrow" @click="dayStore.prevDay" title="Previous day">‹</button>
    <button class="date-label" @click="dayStore.goToday" :class="{ today: dayStore.isViewingToday }">
      {{ displayDate }}
      <span v-if="dayStore.isViewingToday" class="today-badge">Today</span>
    </button>
    <button class="btn-ghost arrow" @click="dayStore.nextDay" title="Next day">›</button>
  </div>
</template>

<style scoped>
.day-nav {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 8px 16px;
  border-bottom: 1px solid var(--border);
  background: var(--surface);
  flex-shrink: 0;
}

.arrow {
  font-size: 20px;
  line-height: 1;
  width: 28px;
  height: 28px;
  padding: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.date-label {
  background: transparent;
  border: none;
  font-size: 14px;
  font-weight: 600;
  color: var(--text);
  cursor: pointer;
  padding: 4px 10px;
  border-radius: var(--radius);
  display: flex;
  align-items: center;
  gap: 8px;
}
.date-label:hover { background: var(--surface-2); }

.today-badge {
  font-size: 11px;
  font-weight: 600;
  background: var(--primary);
  color: #fff;
  padding: 2px 7px;
  border-radius: 10px;
}
</style>
