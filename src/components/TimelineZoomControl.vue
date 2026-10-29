<script setup lang="ts">
  import { useTimeline } from '../composables/useTimeline'

  const { zoomIn, zoomOut, resetZoom, isZoomed, zoomPercent, canZoomIn, canZoomOut } = useTimeline()

  /**
   * Zoom control for the day timeline.
   *
   * Lives in the view's toolbar rather than in the column headers: the header
   * row is laid out to the pixel (a 44px ruler gutter, then percentage-based
   * columns), so anything wider than the gutter overlaps the "Activity" title.
   *
   * Always visible, including at 100%: hiding it made the timeline's current
   * zoom level invisible, and an off-by-one ctrl+scroll left no obvious way back.
   * The label is a percentage of the 160 px/hour default rather than a raw
   * pixel figure. Clicking it resets to 100%.
   */
</script>

<template>
  <div
    class="zoom-control"
    :title="`Timeline zoom ${zoomPercent}% · ctrl+scroll over the timeline to zoom · click the value to reset`"
  >
    <button class="zoom-step" :disabled="!canZoomOut" aria-label="Zoom out" @click="zoomOut">
      −
    </button>
    <button
      class="zoom-value"
      :class="{ default: !isZoomed }"
      :title="isZoomed ? 'Reset to 100%' : 'Timeline is at 100%'"
      @click="resetZoom"
    >
      {{ zoomPercent }}%
    </button>
    <button class="zoom-step" :disabled="!canZoomIn" aria-label="Zoom in" @click="zoomIn">+</button>
  </div>
</template>

<style scoped>
  .zoom-control {
    display: flex;
    align-items: center;
    gap: 1px;
    padding: 0 var(--space-1);
    font-size: var(--text-xs);
    color: var(--text-muted);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    user-select: none;
  }

  .zoom-step,
  .zoom-value {
    background: none;
    border: none;
    margin: 0;
    font: inherit;
    font-size: var(--text-xs);
    color: inherit;
    cursor: pointer;
    min-height: 0;
    padding: 2px 4px;
    line-height: 1.4;
    border-radius: 4px;
  }

  .zoom-value {
    font-variant-numeric: tabular-nums;
    min-width: 40px;
    text-align: center;
  }

  /* At 100% there is nothing to reset, so the value should not look clickable. */
  .zoom-value.default {
    color: var(--text-faint);
  }

  .zoom-step:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .zoom-step:focus-visible,
  .zoom-value:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 1px;
  }

  .zoom-step:hover:not(:disabled),
  .zoom-value:hover {
    background: color-mix(in srgb, var(--primary) 12%, transparent);
    color: var(--primary);
  }
</style>
