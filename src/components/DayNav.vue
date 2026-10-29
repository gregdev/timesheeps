<script setup lang="ts">
  import { computed } from 'vue'
  import { differenceInCalendarDays, format, parseISO } from 'date-fns'
  import { VueDatePicker } from '@vuepic/vue-datepicker'
  import '@vuepic/vue-datepicker/dist/main.css'
  import { usePreferredDark } from '@vueuse/core'
  import { useDayStore } from '../stores/day'
  import { useSettingsStore } from '../stores/settings'
  import { localDateKey } from '../composables/useFormat'

  const dayStore = useDayStore()
  const settingsStore = useSettingsStore()
  const isDark = usePreferredDark()

  const displayDate = computed(() => format(parseISO(dayStore.selectedDate), 'EEEE, MMMM d, yyyy'))

  /**
   * How far the selected day is from today, in days (negative = in the past).
   *
   * Rendered as a separate badge between the date and the button, never inside
   * the button: "Back to Today (6 days ago)" reads as though *today* was six
   * days ago. On its own it clearly describes the date being shown.
   */
  const daysFromToday = computed(() => {
    const selected = parseISO(dayStore.selectedDate)
    const today = parseISO(localDateKey(new Date()))

    return differenceInCalendarDays(selected, today)
  })

  const relativeLabel = computed(() => {
    const n = Math.abs(daysFromToday.value)

    if (n === 0) {
      return null
    }

    return `${n} day${n === 1 ? '' : 's'} ${daysFromToday.value < 0 ? 'ago' : 'ahead'}`
  })

  const backToTodayTooltip = computed(() =>
    relativeLabel.value
      ? `Jump to today (T) — this day is ${relativeLabel.value}`
      : 'Jump to today (T)',
  )

  function onDateSelect(val: string | null) {
    if (val) {
      dayStore.loadDay(val)
    }
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

    <!--
      The distance sits between the date and the button, describing the *date*
      rather than the action. Reading "Wednesday, October 21, 2026 · 8 days ago
      [Back to Today]" leaves no doubt which day was eight days ago.
    -->
    <span v-if="relativeLabel" class="relative-badge">{{ relativeLabel }}</span>

    <button
      v-if="!dayStore.isViewingToday"
      class="btn-secondary back-to-today"
      :data-tooltip="backToTodayTooltip"
      @click="dayStore.goToday"
    >
      Back to Today
    </button>
  </div>
</template>

<style scoped>
  .day-nav {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-4);
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    flex-shrink: 0;
  }

  .arrow {
    font-size: var(--text-xl);
    line-height: 1;
    width: var(--space-7);
    height: var(--space-7);
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
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
    cursor: pointer;
    padding: var(--space-1) 10px;
    border-radius: var(--radius);
    display: flex;
    align-items: center;
    gap: var(--space-2);
    min-width: 15rem;
    justify-content: center;
  }

  .date-label:hover {
    background: var(--surface-2);
  }

  .today-badge {
    font-size: var(--text-xs);
    font-weight: 600;
    background: var(--primary);
    color: #fff;
    padding: 2px var(--space-2);
    border-radius: 10px;
  }

  .back-to-today {
    font-size: var(--text-xs);
    padding: var(--space-1) var(--space-3);
    white-space: nowrap;
  }

  .relative-badge {
    font-size: var(--text-xs);
    color: var(--text-faint);
    white-space: nowrap;
    margin-inline: var(--space-2);
    padding: 2px var(--space-2);
    border-radius: 10px;
    background: var(--surface-2);
    font-variant-numeric: tabular-nums;
  }
</style>
