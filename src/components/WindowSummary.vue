<script setup lang="ts">
  import { computed } from 'vue'
  import { useDayStore } from '../stores/day'
  import { useTimeline } from '../composables/useTimeline'
  import { useEntryModal } from '../composables/useEntryModal'
  import { useContextMenu } from '../composables/useContextMenu'
  import { useSettingsStore } from '../stores/settings'
  import { useAppColour } from '../composables/useAppColour'
  import type { WindowSummaryItem } from '../schemas'

  const dayStore = useDayStore()
  const { isoToMinutes } = useTimeline()
  const { pendingCreate } = useEntryModal()
  const { open: openMenu } = useContextMenu()
  const settingsStore = useSettingsStore()
  const { appColour: appColor } = useAppColour()

  function onItemContextMenu(e: MouseEvent, item: WindowSummaryItem) {
    openMenu(e, [
      {
        label: 'Track to project…',
        action: () => {
          const matching = dayStore.activityBlocks
            .filter((b) => b.appName === item.appName && b.windowTitle === item.windowTitle)
            .sort((a, b) => a.startedAt.localeCompare(b.startedAt))
          const startMinutes =
            matching.length > 0 ? Math.round(isoToMinutes(matching[0].startedAt)) : 9 * 60
          const endMinutes =
            matching.length > 0
              ? Math.round(isoToMinutes(matching[matching.length - 1].endedAt))
              : 10 * 60
          pendingCreate.value = { startMinutes, endMinutes, note: item.appName }
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

  function formatDuration(totalSecs: number): string {
    const h = Math.floor(totalSecs / 3600)
    const m = Math.floor((totalSecs % 3600) / 60)
    if (h === 0) return `${m}m`
    if (m === 0) return `${h}h`
    return `${h}h ${m}m`
  }

  /** Strip trailing app name from common window title patterns like:
   *  "project — file — App Name"  →  "project — file"
   *  "App Name"                   →  "" (hide if same as appName)
   */
  function cleanTitle(appName: string, title: string): string {
    const parts = title.split(' \u2014 ')
    if (parts.length > 1 && parts[parts.length - 1].toLowerCase() === appName.toLowerCase()) {
      parts.pop()
    }
    const cleaned = parts.join(' — ')
    return cleaned.toLowerCase() === appName.toLowerCase() ? '' : cleaned
  }

  const totalSecs = computed(() =>
    dayStore.windowSummary.reduce((s: number, i: WindowSummaryItem) => s + i.totalSecs, 0),
  )
</script>

<template>
  <aside class="window-summary">
    <div class="ws-header">
      <span class="ws-title">Window Activity</span>
      <span class="ws-total">{{ formatDuration(totalSecs) }} total</span>
    </div>

    <div v-if="dayStore.windowSummary.length === 0" class="ws-empty">No activity recorded yet</div>

    <ul v-else class="ws-list">
      <li
        v-for="item in dayStore.windowSummary"
        :key="item.appName + item.windowTitle"
        class="ws-item"
        title="Right-click to create time entry"
        @contextmenu="onItemContextMenu($event, item)"
      >
        <div class="ws-bar-wrap">
          <div
            class="ws-bar"
            :style="{
              width: (item.totalSecs / (dayStore.windowSummary[0]?.totalSecs || 1)) * 100 + '%',
              background: appColor(item.appName),
            }"
          />
        </div>
        <div class="ws-labels">
          <span class="ws-app" :style="{ color: appColor(item.appName) }">{{ item.appName }}</span>
          <span v-if="cleanTitle(item.appName, item.windowTitle)" class="ws-window">
            {{ cleanTitle(item.appName, item.windowTitle) }}
          </span>
        </div>
        <span class="ws-dur">{{ formatDuration(item.totalSecs) }}</span>
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
