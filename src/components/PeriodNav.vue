<script setup lang="ts">
  defineProps<{
    label: string
    isCurrent: boolean
    currentLabel: string
    labelMinWidth?: string
  }>()

  defineEmits<{
    prev: []
    next: []
    current: []
  }>()
</script>

<template>
  <div class="period-nav">
    <button class="btn-ghost nav-arrow" @click="$emit('prev')">‹</button>

    <span class="nav-label" :class="{ current: isCurrent }" :style="{ minWidth: labelMinWidth }">
      {{ label }}
    </span>

    <button class="btn-ghost nav-arrow" @click="$emit('next')">›</button>

    <button v-if="!isCurrent" class="btn-secondary jump-btn" @click="$emit('current')">
      {{ currentLabel }}
    </button>

    <slot />
  </div>
</template>

<style scoped>
  .period-nav {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    padding: var(--space-2) var(--space-4);
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    flex-shrink: 0;
  }

  .nav-arrow {
    font-size: var(--text-xl);
    line-height: 1;
    width: var(--space-7);
    height: var(--space-7);
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .nav-label {
    font-size: var(--text-sm);
    font-weight: 600;
    color: var(--text);
    text-align: center;
  }

  .nav-label.current {
    color: var(--text);
  }

  .jump-btn {
    margin-left: var(--space-1);
    font-size: var(--text-xs);
    padding: var(--space-1) 10px;
  }
</style>
