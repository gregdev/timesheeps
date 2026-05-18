<script setup lang="ts">
  import { ref, onMounted, onUnmounted } from 'vue'
  import { useContextMenu } from '../composables/useContextMenu'

  const { visible: menuVisible } = useContextMenu()

  const visible = ref(false)
  const text = ref('')
  const x = ref(0)
  const y = ref(0)
  const OFFSET = 12

  function getTarget(e: MouseEvent) {
    return (e.target as HTMLElement).closest('[data-tooltip]') as HTMLElement | null
  }

  function position(e: MouseEvent) {
    const tx = Math.min(e.clientX + OFFSET, window.innerWidth - 272)
    const ty = Math.min(e.clientY + OFFSET, window.innerHeight - 80)
    x.value = tx
    y.value = ty
  }

  function onOver(e: MouseEvent) {
    const el = getTarget(e)
    if (!el || menuVisible.value) {
      visible.value = false
      return
    }
    text.value = el.dataset.tooltip ?? ''
    visible.value = true
    position(e)
  }

  function onOut(e: MouseEvent) {
    const el = getTarget(e)
    if (el) visible.value = false
  }

  function onMove(e: MouseEvent) {
    if (visible.value) position(e)
  }

  function onContextMenu() {
    visible.value = false
  }

  onMounted(() => {
    document.addEventListener('mouseover', onOver)
    document.addEventListener('mouseout', onOut)
    document.addEventListener('mousemove', onMove)
    document.addEventListener('contextmenu', onContextMenu)
  })
  onUnmounted(() => {
    document.removeEventListener('mouseover', onOver)
    document.removeEventListener('mouseout', onOut)
    document.removeEventListener('mousemove', onMove)
    document.removeEventListener('contextmenu', onContextMenu)
  })
</script>

<template>
  <Teleport to="body">
    <div v-if="visible && text" class="tooltip-overlay" :style="{ left: x + 'px', top: y + 'px' }">
      {{ text }}
    </div>
  </Teleport>
</template>

<style scoped>
  .tooltip-overlay {
    position: fixed;
    z-index: 9999;
    background: #1e293b;
    color: #f1f5f9;
    padding: 5px 10px;
    border-radius: 6px;
    font-size: 11.5px;
    font-family: var(--font);
    white-space: pre-line;
    line-height: 1.6;
    pointer-events: none;
    max-width: 260px;
    box-shadow: 0 4px 14px rgb(0 0 0 / 35%);
  }

  @media (prefers-color-scheme: dark) {
    .tooltip-overlay {
      background: #334155;
      box-shadow: 0 4px 14px rgb(0 0 0 / 60%);
    }
  }
</style>
