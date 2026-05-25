<script setup lang="ts">
  import { computed } from 'vue'
  import type { ActivityBlock } from '../schemas'
  import { useAppColour } from '../composables/useAppColour'

  const { appColour: appColor } = useAppColour()

  const props = defineProps<{
    allBlocks: ActivityBlock[]
    matchedBlocks: ActivityBlock[]
    startHour: number
    endHour: number
  }>()

  const startMin = computed(() => props.startHour * 60)
  const totalMin = computed(() => (props.endHour - props.startHour) * 60)

  function isoToMin(iso: string): number {
    const d = new Date(iso)
    return d.getHours() * 60 + d.getMinutes() + d.getSeconds() / 60
  }

  function leftPct(iso: string): number {
    const min = isoToMin(iso)
    return Math.max(0, Math.min(100, ((min - startMin.value) / totalMin.value) * 100))
  }

  function widthPct(startIso: string, endIso: string): number {
    const s = isoToMin(startIso)
    const e = isoToMin(endIso)
    const left = leftPct(startIso)
    return Math.max(0.3, Math.min(100 - left, ((e - s) / totalMin.value) * 100))
  }

  // Hour tick positions for the hour grid
  const hourTicks = computed(() => {
    const ticks: number[] = []
    for (let h = props.startHour + 1; h < props.endHour; h++) {
      ticks.push(((h * 60 - startMin.value) / totalMin.value) * 100)
    }
    return ticks
  })
</script>

<template>
  <div class="sdt-wrap">
    <!-- Hour grid lines -->
    <div v-for="tick in hourTicks" :key="tick" class="sdt-tick" :style="{ left: tick + '%' }" />
    <!-- All blocks (dimmed background) -->
    <div
      v-for="block in allBlocks"
      :key="block.startedAt + '-bg'"
      class="sdt-block sdt-block-dim"
      :style="{
        left: leftPct(block.startedAt) + '%',
        width: widthPct(block.startedAt, block.endedAt) + '%',
        '--color': appColor(block.appName),
      }"
    />
    <!-- Matched blocks (highlighted) -->
    <div
      v-for="block in matchedBlocks"
      :key="block.startedAt + '-hi'"
      class="sdt-block sdt-block-match"
      :style="{
        left: leftPct(block.startedAt) + '%',
        width: widthPct(block.startedAt, block.endedAt) + '%',
        '--color': appColor(block.appName),
      }"
    />
  </div>
</template>

<style scoped>
  .sdt-wrap {
    position: relative;
    height: 24px;
    background: var(--surface-2);
    border-radius: 4px;
    overflow: hidden;
    margin: 6px 0 8px;
  }

  .sdt-tick {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--border);
    opacity: 0.5;
  }

  .sdt-block {
    position: absolute;
    top: 3px;
    bottom: 3px;
    border-radius: 2px;
    min-width: 2px;
  }

  .sdt-block-dim {
    background: color-mix(in srgb, var(--color) 20%, transparent);
  }

  .sdt-block-match {
    background: var(--color);
    opacity: 0.85;
  }
</style>
