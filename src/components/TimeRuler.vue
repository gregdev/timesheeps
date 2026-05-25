<script setup lang="ts">
  import { useTimeline } from '../composables/useTimeline'

  const { totalHeight, hours, minuteToY } = useTimeline()
</script>

<template>
  <div class="ruler" :style="{ height: totalHeight + 'px' }">
    <div
      v-for="hour in hours"
      :key="hour"
      class="hour-label"
      :style="{ top: minuteToY(hour * 60) + 'px' }"
    >
      {{ hour === 0 ? '12a' : hour < 12 ? hour + 'a' : hour === 12 ? '12p' : hour - 12 + 'p' }}
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
