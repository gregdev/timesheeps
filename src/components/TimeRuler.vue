<script setup lang="ts">
  import { computed } from 'vue'
  import { useTimeline } from '../composables/useTimeline'
  import { useContextMenu } from '../composables/useContextMenu'

  const {
    totalHeight,
    hours,
    minuteToY,
    zoomIn,
    zoomOut,
    resetZoom,
    zoomPercent,
    isExpanded,
    preferredStartMin,
    preferredEndMin,
    startMin,
    endMin,
    minutesToTime,
  } = useTimeline()
  const { open: openMenu } = useContextMenu()

  /**
   * The window can be wider than the Settings hours when the day contains work
   * outside them. Say so here, rather than leaving the user to wonder why the
   * timeline starts at 5am.
   */
  const rulerTitle = computed(() => {
    const base = `${zoomPercent.value}% · ctrl+scroll to zoom`

    if (!isExpanded.value) {
      return base
    }

    return (
      `${base}\nShowing ${minutesToTime(startMin.value)}–${minutesToTime(endMin.value)} ` +
      `to fit recorded time (Settings: ${minutesToTime(preferredStartMin.value)}–` +
      `${minutesToTime(preferredEndMin.value)})`
    )
  })

  /**
   * `24` is the end-of-day tick, not 12pm — normalising to 0 keeps midnight
   * reading as "12a" at both edges of an expanded window.
   */
  function hourLabel(hour: number): string {
    const h = hour % 24

    if (h === 0) {
      return '12a'
    }

    return h < 12 ? `${h}a` : h === 12 ? '12p' : `${h - 12}p`
  }

  /**
   * The ruler is the natural home for zoom: it is the part of the timeline that
   * already means "time". Ctrl+wheel works anywhere over the timeline body, and
   * the always-visible badge lives in the column header (which does not scroll,
   * unlike this element — it is as tall as the day).
   */
  function onContextMenu(e: MouseEvent) {
    openMenu(e, [
      { label: `Zoom in (${zoomPercent.value}%)`, action: zoomIn },
      { label: 'Zoom out', action: zoomOut },
      { label: 'Reset zoom', action: resetZoom },
    ])
  }
</script>

<template>
  <div
    class="ruler"
    :style="{ height: totalHeight + 'px' }"
    :title="rulerTitle"
    @contextmenu="onContextMenu"
  >
    <div
      v-for="hour in hours"
      :key="hour"
      class="hour-label"
      :style="{ top: minuteToY(hour * 60) + 'px' }"
    >
      {{ hourLabel(hour) }}
    </div>
  </div>
</template>

<style scoped>
  .ruler {
    position: relative;
    width: 44px;
    flex-shrink: 0;
    user-select: none;
  }

  .hour-label {
    position: absolute;
    right: 8px;
    transform: translateY(-50%);
    font-size: 11px;
    color: var(--text-faint);
    font-weight: 500;
    white-space: nowrap;
  }
</style>
