import { computed } from 'vue'
import { useSettingsStore } from '../stores/settings'

export const HOUR_HEIGHT = 80 // px per hour

export function useTimeline() {
  const settingsStore = useSettingsStore()

  const startMin = computed(() => settingsStore.settings.timelineStartHour * 60)
  const endMin = computed(() => settingsStore.settings.timelineEndHour * 60)
  const totalHeight = computed(() => (endMin.value - startMin.value) / 60 * HOUR_HEIGHT)
  const hours = computed(() => {
    const result: number[] = []
    for (let h = settingsStore.settings.timelineStartHour; h <= settingsStore.settings.timelineEndHour; h++) {
      result.push(h)
    }
    return result
  })

  function minuteToY(min: number): number {
    return (min - startMin.value) / 60 * HOUR_HEIGHT
  }

  function yToMinute(y: number): number {
    return y / HOUR_HEIGHT * 60 + startMin.value
  }

  function snapMinutes(min: number, snap = 5): number {
    return Math.round(min / snap) * snap
  }

  function clampMin(min: number): number {
    return Math.max(startMin.value, Math.min(endMin.value, min))
  }

  function formatDuration(totalMin: number): string {
    const h = Math.floor(totalMin / 60)
    const m = totalMin % 60
    if (h === 0) return `${m}m`
    if (m === 0) return `${h}h`
    return `${h}h ${m}m`
  }

  function minutesToTime(min: number): string {
    const h = Math.floor(min / 60)
    const m = min % 60
    return `${h.toString().padStart(2, '0')}:${m.toString().padStart(2, '0')}`
  }

  function isoToMinutes(iso: string): number {
    const d = new Date(iso)
    return d.getHours() * 60 + d.getMinutes() + d.getSeconds() / 60
  }

  return {
    startMin, endMin, totalHeight, hours,
    minuteToY, yToMinute, snapMinutes, clampMin,
    formatDuration, minutesToTime, isoToMinutes,
  }
}
