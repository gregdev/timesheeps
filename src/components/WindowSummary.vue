<script setup lang="ts">
  import { computed } from 'vue'
  import { useDayStore } from '../stores/day'
  import { useTimeline } from '../composables/useTimeline'
  import { useEntryModal, type EntryRange } from '../composables/useEntryModal'
  import { useContextMenu } from '../composables/useContextMenu'
  import { useSettingsStore } from '../stores/settings'
  import { useAppColour } from '../composables/useAppColour'
  import { cleanWindowTitle, formatSeconds } from '../composables/useFormat'
  import type { WindowSummaryItem } from '../schemas'

  const dayStore = useDayStore()
  const { isoToMinutes } = useTimeline()
  const { pendingCreate } = useEntryModal()
  const { open: openMenu } = useContextMenu()
  const settingsStore = useSettingsStore()
  const { appColour: appColor } = useAppColour()

  /**
   * The blocks a summary row aggregates, as one range per session.
   *
   * The row was grouped by whichever key `get_window_summary_for_date` chose, so the
   * match has to follow the same rule: title for title-split apps, the extracted key
   * for grouped apps, and the window handle otherwise. Matching on `windowTitle`
   * alone picked an arbitrary set of blocks, because a block's displayed title is
   * its *last* title while the row shows its *longest* — two different things that
   * happen to coincide on some days and not others.
   */
  function sessionsFor(item: WindowSummaryItem): EntryRange[] {
    const name = item.appName.toLowerCase()
    const splitByTitle = settingsStore.settings.titleSplitApps.some((a) => a.toLowerCase() === name)
    const groupedByTitle = settingsStore.settings.titleGroupApps.some(
      (a) => a.toLowerCase() === name,
    )

    const matched = dayStore.activityBlocks.filter((b) => {
      if (b.appName !== item.appName) {
        return false
      }
      if (splitByTitle) {
        return b.windowTitle === item.windowTitle
      }
      if (groupedByTitle) {
        // The row's title for these apps is a key extracted *from* the raw title
        // (a project name), so it is never an exact block title.
        return b.windowTitles.some((t) => t.includes(item.windowTitle))
      }

      // Rows recorded before the handle was captured fall back to grouping by title.
      return item.windowId !== 0 ? b.windowId === item.windowId : b.windowTitle === item.windowTitle
    })

    // Blocks are already merged into sessions, so each is one range — and they stay
    // separate entries, never a single span swallowing the gaps between them.
    return matched
      .sort((a, b) => a.startedAt.localeCompare(b.startedAt))
      .map((b) => ({
        startMinutes: Math.round(isoToMinutes(b.startedAt)),
        endMinutes: Math.round(isoToMinutes(b.endedAt)),
      }))
      .filter((r) => r.endMinutes > r.startMinutes)
  }

  function onItemContextMenu(e: MouseEvent, item: WindowSummaryItem) {
    const alreadySplit = settingsStore.settings.titleSplitApps.some(
      (a) => a.toLowerCase() === item.appName.toLowerCase(),
    )
    const alreadyGrouped = settingsStore.settings.titleGroupApps.some(
      (a) => a.toLowerCase() === item.appName.toLowerCase(),
    )

    openMenu(e, [
      {
        label: 'Track to project…',
        action: () => {
          const ranges = sessionsFor(item)

          if (ranges.length === 0) {
            window.alert(
              `No sessions to track for "${item.windowTitle}".\n\n` +
                'Its visits on this day are all shorter than the minimum block length in ' +
                'Settings, so there is nothing to create entries from.',
            )
            return
          }

          pendingCreate.value = {
            startMinutes: ranges[0].startMinutes,
            endMinutes: ranges[0].endMinutes,
            ranges,
            note: item.appName,
          }
        },
      },
      {
        label: alreadySplit ? '✓ Split by tab title' : 'Split by tab title',
        action: async () => {
          await settingsStore.addToTitleSplitApps(item.appName)
          await dayStore.loadDay(undefined, true)
        },
      },
      {
        label: alreadyGrouped ? '✓ Group by project name' : 'Group by project name',
        action: async () => {
          await settingsStore.addToTitleGroupApps(item.appName)
          await dayStore.loadDay(undefined, true)
        },
      },
      {
        label: 'Create ignore rule',
        action: async () => {
          await settingsStore.createRule('app_name', item.appName)
          await dayStore.loadDay(undefined, true)
        },
      },
    ])
  }

  const totalSecs = computed(() =>
    dayStore.windowSummary.reduce((s: number, i: WindowSummaryItem) => s + i.totalSecs, 0),
  )

  /** Bars are relative to the busiest window, which is not necessarily the first. */
  const maxItemSecs = computed(() =>
    dayStore.windowSummary.reduce((max, i) => Math.max(max, i.totalSecs), 1),
  )
</script>

<template>
  <aside class="window-summary">
    <div class="ws-header">
      <span class="ws-title">Window Activity</span>
      <span class="ws-total">{{ formatSeconds(totalSecs) }} total</span>
    </div>

    <div v-if="dayStore.windowSummary.length === 0" class="ws-empty">No activity recorded yet</div>

    <ul v-else class="ws-list">
      <li
        v-for="item in dayStore.windowSummary"
        :key="item.appName + item.windowTitle"
        class="ws-item"
        data-tooltip="Right-click for actions"
        @contextmenu="onItemContextMenu($event, item)"
      >
        <div class="ws-bar-wrap">
          <div
            class="ws-bar"
            :style="{
              width: (item.totalSecs / maxItemSecs) * 100 + '%',
              background: appColor(item.appName),
            }"
          />
        </div>
        <div class="ws-labels">
          <span class="ws-app" :style="{ color: appColor(item.appName) }">{{ item.appName }}</span>
          <span v-if="cleanWindowTitle(item.appName, item.windowTitle)" class="ws-window">
            {{ cleanWindowTitle(item.appName, item.windowTitle) }}
          </span>
        </div>
        <span class="ws-dur">{{ formatSeconds(item.totalSecs) }}</span>
      </li>
    </ul>
  </aside>
</template>

<style scoped>
  .window-summary {
    width: 220px;
    flex-shrink: 0;
    border-left: 1px solid var(--border);
    background: var(--surface);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .ws-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: var(--space-2) var(--space-4) var(--space-2);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .ws-title {
    font-size: var(--text-xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .ws-total {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .ws-empty {
    padding: var(--space-5) var(--space-4);
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .ws-list {
    list-style: none;
    margin: 0;
    padding: var(--space-2) 0;
    overflow-y: auto;
    flex: 1;
  }

  .ws-item {
    display: grid;
    grid-template-columns: 1fr auto;
    grid-template-rows: auto auto;
    gap: 0 var(--space-2);
    padding: var(--space-2) var(--space-4);
    transition: background 0.12s;
  }

  .ws-item:hover {
    background: color-mix(in srgb, var(--border) 40%, transparent);
  }

  .ws-bar-wrap {
    grid-column: 1 / -1;
    height: 3px;
    background: var(--border);
    border-radius: 2px;
    margin-bottom: var(--space-1);
    overflow: hidden;
  }

  .ws-bar {
    height: 100%;
    border-radius: 2px;
    opacity: 0.7;
    transition: width 0.3s ease;
  }

  .ws-labels {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }

  .ws-app {
    font-size: var(--text-xs);
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ws-window {
    font-size: var(--text-xs);
    color: var(--text-muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ws-dur {
    font-size: var(--text-xs);
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    align-self: center;
  }
</style>
