<script setup lang="ts">
  import { computed } from 'vue'
  import { format, parseISO } from 'date-fns'
  import { VueDatePicker } from '@vuepic/vue-datepicker'
  import '@vuepic/vue-datepicker/dist/main.css'
  import { usePreferredDark } from '@vueuse/core'
  import { useDayStore } from '../stores/day'
  import { useSettingsStore } from '../stores/settings'

  const dayStore = useDayStore()
  const settingsStore = useSettingsStore()
  const isDark = usePreferredDark()

  const displayDate = computed(() => format(parseISO(dayStore.selectedDate), 'EEEE, MMMM d, yyyy'))

  function onDateSelect(val: string | null) {
    if (val) dayStore.loadDay(val)
  }
</script>

<template>
  <div class="day-nav">
    <button class="btn-ghost arrow" data-tooltip="Previous day (←)" @click="dayStore.prevDay">
      ‹
    </button>
    <VueDatePicker
      :model-value="dayStore.selectedDate"
      model-type="yyyy-MM-dd"
      :auto-apply="true"
      :enable-time-picker="false"
      :dark="isDark"
      :week-start="settingsStore.settings.weekStartsOn"
      :clearable="false"
      class="dp-nav"
      @update:model-value="onDateSelect"
    >
      <template #trigger>
        <button class="date-label" :class="{ today: dayStore.isViewingToday }">
          {{ displayDate }}
          <span v-if="dayStore.isViewingToday" class="today-badge">Today</span>
        </button>
      </template>
    </VueDatePicker>
    <button class="btn-ghost arrow" data-tooltip="Next day (→)" @click="dayStore.nextDay">›</button>
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

  .dp-nav {
    width: auto;
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

  .date-label:hover {
    background: var(--surface-2);
  }

  .today-badge {
    font-size: 11px;
    font-weight: 600;
    background: var(--primary);
    color: #fff;
    padding: 2px 7px;
    border-radius: 10px;
  }
</style>
