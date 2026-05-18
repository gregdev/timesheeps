<script setup lang="ts">
  import { ref, watch, nextTick, onUnmounted } from 'vue'
  import { useContextMenu } from '../composables/useContextMenu'

  const { visible, x, y, items, close } = useContextMenu()

  const menuRef = ref<HTMLElement>()
  const finalX = ref(0)
  const finalY = ref(0)

  watch(visible, async (v: boolean) => {
    if (v) {
      await nextTick()
      if (menuRef.value) {
        const w = menuRef.value.offsetWidth
        const h = menuRef.value.offsetHeight
        finalX.value = Math.min(x.value, window.innerWidth - w - 8)
        finalY.value = Math.min(y.value, window.innerHeight - h - 8)
      } else {
        finalX.value = x.value
        finalY.value = y.value
      }
    }
  })

  function onItemClick(action: () => void) {
    action()
    close()
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') close()
  }

  function onMousedown(e: MouseEvent) {
    if (menuRef.value && !menuRef.value.contains(e.target as Node)) close()
  }

  watch(visible, (v: boolean) => {
    if (v) {
      document.addEventListener('mousedown', onMousedown)
      document.addEventListener('keydown', onKeydown)
    } else {
      document.removeEventListener('mousedown', onMousedown)
      document.removeEventListener('keydown', onKeydown)
    }
  })

  onUnmounted(() => {
    document.removeEventListener('mousedown', onMousedown)
    document.removeEventListener('keydown', onKeydown)
  })
</script>

<template>
  <Teleport to="body">
    <ul
      v-if="visible"
      ref="menuRef"
      class="context-menu"
      :style="{ left: finalX + 'px', top: finalY + 'px' }"
      role="menu"
    >
      <li
        v-for="item in items"
        :key="item.label"
        class="context-menu-item"
        :class="{ danger: item.danger }"
        role="menuitem"
        @mousedown.prevent="onItemClick(item.action)"
      >
        {{ item.label }}
      </li>
    </ul>
  </Teleport>
</template>

<style scoped>
  .context-menu {
    position: fixed;
    z-index: 9999;
    list-style: none;
    margin: 0;
    padding: 4px 0;
    min-width: 180px;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 6px;
    box-shadow:
      0 4px 16px rgb(0 0 0 / 18%),
      0 1px 4px rgb(0 0 0 / 10%);
    animation: ctx-in 0.12s cubic-bezier(0.16, 1, 0.3, 1);
  }

  @keyframes ctx-in {
    from {
      opacity: 0;
      transform: scale(0.97) translateY(-4px);
    }

    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }

  .context-menu-item {
    padding: 7px 14px;
    font-size: 13px;
    color: var(--text);
    cursor: default;
    user-select: none;
    white-space: nowrap;
    transition:
      background 0.1s,
      color 0.1s;
  }

  .context-menu-item:hover {
    background: color-mix(in srgb, var(--primary) 12%, transparent);
    color: var(--primary);
  }

  .context-menu-item.danger {
    color: #ef4444;
  }

  .context-menu-item.danger:hover {
    background: color-mix(in srgb, #ef4444 12%, transparent);
    color: #ef4444;
  }
</style>
