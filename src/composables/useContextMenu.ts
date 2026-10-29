import { ref } from 'vue'

export interface ContextMenuItem {
  label: string
  action: () => void
  danger?: boolean
  /**
   * Rendered greyed out and inert. Used for bulk actions that do not apply to
   * the current selection (e.g. Merge across two projects) — better than
   * offering the action and refusing it after the click.
   */
  disabled?: boolean
}

// Module-level singleton — one menu open at a time across the whole app
const visible = ref(false)
const x = ref(0)
const y = ref(0)
const items = ref<ContextMenuItem[]>([])

export function useContextMenu() {
  function open(e: MouseEvent, menuItems: ContextMenuItem[]) {
    e.preventDefault()
    x.value = e.clientX
    y.value = e.clientY
    items.value = menuItems
    visible.value = true
  }

  function close() {
    visible.value = false
  }

  return { visible, x, y, items, open, close }
}
