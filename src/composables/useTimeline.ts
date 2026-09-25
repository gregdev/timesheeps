import { computed } from 'vue'
import { useSettingsStore } from '../stores/settings'
import { formatMinutes, isoToLocalMinutes, minutesToClock } from './useFormat'

export const HOUR_HEIGHT = 160 // px per hour

export function useTimeline() {
  const settingsStore = useSettingsStore()

  const startMin = computed(() => settingsStore.settings.timelineStartHour * 60)
  const endMin = computed(() => settingsStore.settings.timelineEndHour * 60)
  const totalHeight = computed(() => ((endMin.value - startMin.value) / 60) * HOUR_HEIGHT)
  const hours = computed(() => {
    const result: number[] = []

    for (
      let h = settingsStore.settings.timelineStartHour;
      h <= settingsStore.settings.timelineEndHour;
      h++
    ) {
      result.push(h)
    }

    return result
  })

  function minuteToY(min: number): number {
    return ((min - startMin.value) / 60) * HOUR_HEIGHT
  }

  function yToMinute(y: number): number {
    return (y / HOUR_HEIGHT) * 60 + startMin.value
  }

  function snapMinutes(min: number, snap = settingsStore.settings.snapMinutes): number {
    return Math.round(min / snap) * snap
  }

  function clampMin(min: number): number {
    return Math.max(startMin.value, Math.min(endMin.value, min))
  }

  // These three live in `useFormat` so components with no timeline context
  // (the window summary, match-rule stats) format values identically.
  const formatDuration = formatMinutes
  const minutesToTime = minutesToClock
  const isoToMinutes = isoToLocalMinutes

  return {
    startMin,
    endMin,
    totalHeight,
    hours,
    minuteToY,
    yToMinute,
    snapMinutes,
    clampMin,
    formatDuration,
    minutesToTime,
    isoToMinutes,
  }
}
