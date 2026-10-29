import { computed, ref } from 'vue'

/**
 * Multi-select for time entries, shared by the entry track and the header bar.
 *
 * Module-level like `useContextMenu`: the track owns the clicks while the
 * header owns the bulk actions, and threading the list through props in both
 * directions buys nothing.
 */
const selectedIds = ref<number[]>([])
/** Last entry clicked — the fixed end of a shift+click range. */
const anchorId = ref<number | null>(null)

export function useEntrySelection() {
  const selectionCount = computed(() => selectedIds.value.length)

  function isSelected(id: number): boolean {
    return selectedIds.value.includes(id)
  }

  function toggle(id: number) {
    selectedIds.value = isSelected(id)
      ? selectedIds.value.filter((i) => i !== id)
      : [...selectedIds.value, id]
    anchorId.value = id
  }

  /**
   * Move the shift+click anchor without changing what is selected.
   *
   * A plain click has to set this even though it also clears the selection and
   * opens the editor: otherwise the *next* shift+click has no anchor, and the
   * range gesture silently turns into another editor popup.
   */
  function setAnchor(id: number | null) {
    anchorId.value = id
  }

  function selectRange(ids: number[]) {
    selectedIds.value = [...ids]
  }

  function clear() {
    if (selectedIds.value.length === 0 && anchorId.value === null) {
      return
    }

    selectedIds.value = []
    anchorId.value = null
  }

  /**
   * Drop ids that no longer exist.
   *
   * Entries vanish on delete, on a day change and on conflict resolution, and a
   * stale id would keep the header claiming "3 selected" with nothing
   * highlighted.
   */
  function prune(existing: number[]) {
    const next = selectedIds.value.filter((id) => existing.includes(id))

    if (next.length !== selectedIds.value.length) {
      selectedIds.value = next
    }

    if (anchorId.value !== null && !existing.includes(anchorId.value)) {
      anchorId.value = next.length ? next[0] : null
    }
  }

  return {
    selectedIds,
    anchorId,
    selectionCount,
    isSelected,
    toggle,
    setAnchor,
    selectRange,
    clear,
    prune,
  }
}
