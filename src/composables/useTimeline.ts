import { computed, ref, watch } from 'vue'
import { useSettingsStore } from '../stores/settings'
import { useDayStore } from '../stores/day'
import { formatMinutes, isoToLocalMinutes, minutesToClock } from './useFormat'

/** Pixels per hour at 100% zoom. */
export const HOUR_HEIGHT = 160

/** Minutes in a day. The visible window can never exceed 0:00–24:00. */
const DAY_MINUTES = 24 * 60

/**
 * Zoom bounds. The low end still fits a 16-hour day in a laptop viewport; the
 * high end makes a 2-minute entry about 16px tall, which is the point — short
 * entries are otherwise a few pixels and impossible to hit.
 */
const MIN_HOUR_HEIGHT = 60
const MAX_HOUR_HEIGHT = 480
/** Multiplier per Ctrl+wheel notch. */
const ZOOM_STEP = 1.15
const ZOOM_STORAGE_KEY = 'timelineHourHeight'

function clampHourHeight(px: number): number {
  return Math.max(MIN_HOUR_HEIGHT, Math.min(MAX_HOUR_HEIGHT, px))
}

function initialHourHeight(): number {
  const stored = Number(localStorage.getItem(ZOOM_STORAGE_KEY))

  return Number.isFinite(stored) && stored > 0 ? clampHourHeight(stored) : HOUR_HEIGHT
}

/**
 * Pixels per hour, shared by every timeline component.
 *
 * Module-level on purpose: `useTimeline()` is called from half a dozen
 * components, and a per-call ref would let the ruler and the tracks disagree
 * about where 9am is. Stored in localStorage rather than the settings table
 * because it is view state — persisting it through `save_settings` would mean
 * touching the struct, `Default`, `get_settings` and `save_settings` for a
 * value the backend never reads.
 */
const hourHeight = ref(initialHourHeight())

watch(hourHeight, (px) => localStorage.setItem(ZOOM_STORAGE_KEY, String(px)))

export function useTimeline() {
  const settingsStore = useSettingsStore()
  const dayStore = useDayStore()

  /** The window the user asked for in Settings. */
  const preferredStartMin = computed(() => settingsStore.settings.timelineStartHour * 60)
  const preferredEndMin = computed(() => settingsStore.settings.timelineEndHour * 60)

  /**
   * The span the day's own data needs, rounded out to whole hours.
   *
   * `timeline_start_hour`/`timeline_end_hour` are a *preference*, not a bound:
   * an entry recorded at 23:00 outside them was not merely clipped, it was
   * never drawn at all, so it could not be selected, edited or deleted. Include
   * entries, suggestions and activity blocks — all three are drawn on the
   * timeline, so all three must be reachable.
   */
  const dataBounds = computed<{ start: number; end: number } | null>(() => {
    let earliest = Number.POSITIVE_INFINITY
    let latest = Number.NEGATIVE_INFINITY

    // Inverted ranges are ignored rather than trusted: a legacy row like
    // 22:17 → 22:00 must not drag the window backwards.
    for (const e of dayStore.timeEntries) {
      if (e.endMinutes > e.startMinutes) {
        earliest = Math.min(earliest, e.startMinutes)
        latest = Math.max(latest, e.endMinutes)
      }
    }

    for (const b of dayStore.activityBlocks) {
      earliest = Math.min(earliest, isoToLocalMinutes(b.startedAt))
      latest = Math.max(latest, isoToLocalMinutes(b.endedAt))
    }

    for (const s of dayStore.suggestedEntries) {
      earliest = Math.min(earliest, s.startMinutes)
      latest = Math.max(latest, s.endMinutes)
    }

    if (!Number.isFinite(earliest) || !Number.isFinite(latest) || latest <= earliest) {
      return null
    }

    return {
      start: Math.max(0, Math.floor(earliest / 60) * 60),
      end: Math.min(DAY_MINUTES, Math.ceil(latest / 60) * 60),
    }
  })

  /**
   * The visible window: the preferred hours, widened to fit whatever the day
   * actually contains. Only ever *adds* hours, so changing this cannot hide
   * something the preference already showed.
   */
  const startMin = computed(() =>
    dataBounds.value
      ? Math.min(preferredStartMin.value, dataBounds.value.start)
      : preferredStartMin.value,
  )
  const endMin = computed(() =>
    dataBounds.value
      ? Math.max(preferredEndMin.value, dataBounds.value.end)
      : preferredEndMin.value,
  )
  /** True when the window has been widened past the Settings value. */
  const isExpanded = computed(
    () => startMin.value < preferredStartMin.value || endMin.value > preferredEndMin.value,
  )

  const totalHeight = computed(() => ((endMin.value - startMin.value) / 60) * hourHeight.value)
  const hours = computed(() => {
    const result: number[] = []

    for (let h = startMin.value / 60; h <= endMin.value / 60; h++) {
      result.push(h)
    }

    return result
  })

  function minuteToY(min: number): number {
    return ((min - startMin.value) / 60) * hourHeight.value
  }

  function yToMinute(y: number): number {
    return (y / hourHeight.value) * 60 + startMin.value
  }

  function setHourHeight(px: number) {
    hourHeight.value = clampHourHeight(px)
  }

  function zoomBy(factor: number) {
    setHourHeight(hourHeight.value * factor)
  }

  function zoomIn() {
    zoomBy(ZOOM_STEP)
  }

  function zoomOut() {
    zoomBy(1 / ZOOM_STEP)
  }

  function resetZoom() {
    hourHeight.value = HOUR_HEIGHT
  }

  const isZoomed = computed(() => Math.abs(hourHeight.value - HOUR_HEIGHT) > 0.5)
  const zoomPercent = computed(() => Math.round((hourHeight.value / HOUR_HEIGHT) * 100))
  const canZoomIn = computed(() => hourHeight.value < MAX_HOUR_HEIGHT - 0.5)
  const canZoomOut = computed(() => hourHeight.value > MIN_HOUR_HEIGHT + 0.5)

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
    preferredStartMin,
    preferredEndMin,
    isExpanded,
    totalHeight,
    hours,
    minuteToY,
    yToMinute,
    snapMinutes,
    clampMin,
    hourHeight,
    setHourHeight,
    zoomBy,
    zoomIn,
    zoomOut,
    resetZoom,
    isZoomed,
    zoomPercent,
    canZoomIn,
    canZoomOut,
    formatDuration,
    minutesToTime,
    isoToMinutes,
  }
}
