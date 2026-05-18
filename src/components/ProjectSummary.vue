<script setup lang="ts">
  import { computed } from 'vue'
  import { useDayStore } from '../stores/day'
  import { useProjectsStore } from '../stores/projects'
  import { useTimeline } from '../composables/useTimeline'

  const dayStore = useDayStore()
  const projectsStore = useProjectsStore()
  const { formatDuration } = useTimeline()

  const groups = computed(() => {
    type Group = {
      project: NonNullable<ReturnType<typeof projectsStore.byId>>
      ownMins: number
      totalMins: number
      children: { project: NonNullable<ReturnType<typeof projectsStore.byId>>; mins: number }[]
    }
    const map = new Map<number, Group>()

    for (const [projectId, mins] of dayStore.summary.entries()) {
      const project = projectsStore.byId(projectId)
      if (!project) continue

      if (project.parentId !== null && project.parentId !== undefined) {
        const parent = projectsStore.byId(project.parentId)
        if (parent) {
          if (!map.has(parent.id)) {
            map.set(parent.id, { project: parent, ownMins: 0, totalMins: 0, children: [] })
          }
          map.get(parent.id)!.children.push({ project, mins })
        } else {
          // Orphan child (parent archived) — show standalone
          map.set(project.id, { project, ownMins: mins, totalMins: mins, children: [] })
        }
      } else {
        if (!map.has(project.id)) {
          map.set(project.id, { project, ownMins: mins, totalMins: 0, children: [] })
        } else {
          map.get(project.id)!.ownMins = mins
        }
      }
    }

    return [...map.values()]
      .map((g) => ({
        ...g,
        totalMins: g.ownMins + g.children.reduce((s, c) => s + c.mins, 0),
        children: [...g.children].sort((a, b) => b.mins - a.mins),
      }))
      .sort((a, b) => b.totalMins - a.totalMins)
  })

  const totalMins = computed(() => groups.value.reduce((s: number, g) => s + g.totalMins, 0))
  const maxMins = computed(() => groups.value[0]?.totalMins ?? 1)
</script>

<template>
  <aside class="project-summary">
    <div class="ps-header">
      <span class="ps-title">Project Time</span>
      <span class="ps-total">{{ totalMins > 0 ? formatDuration(totalMins) + ' total' : '' }}</span>
    </div>

    <div v-if="groups.length === 0" class="ps-empty">No time logged yet</div>

    <ul v-else class="ps-list">
      <li v-for="group in groups" :key="group.project.id" class="ps-group">
        <!-- Parent / standalone row -->
        <div class="ps-item">
          <div class="ps-bar-wrap">
            <div
              class="ps-bar"
              :style="{
                width: (group.totalMins / maxMins) * 100 + '%',
                background: group.project.color,
              }"
            />
          </div>
          <div class="ps-labels">
            <span class="ps-dot" :style="{ background: group.project.color }" />
            <span class="ps-name">{{ group.project.name }}</span>
          </div>
          <span class="ps-dur">{{ formatDuration(group.totalMins) }}</span>
        </div>

        <!-- Child rows -->
        <div v-for="child in group.children" :key="child.project.id" class="ps-item ps-child">
          <div class="ps-bar-wrap">
            <div
              class="ps-bar ps-bar-child"
              :style="{
                width: (child.mins / group.totalMins) * 100 + '%',
                background: group.project.color,
              }"
            />
          </div>
          <div class="ps-labels">
            <span class="ps-name ps-child-name">{{ child.project.name }}</span>
          </div>
          <span class="ps-dur ps-child-dur">{{ formatDuration(child.mins) }}</span>
        </div>
      </li>
    </ul>
  </aside>
</template>

<style scoped>
  .project-summary {
    width: 220px;
    flex-shrink: 0;
    border-left: 1px solid var(--border);
    background: var(--surface);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .ps-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    padding: 10px 14px 8px;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .ps-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .ps-total {
    font-size: 11px;
    color: var(--text-muted);
  }

  .ps-empty {
    padding: 20px 14px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .ps-list {
    list-style: none;
    margin: 0;
    padding: 6px 0;
    overflow-y: auto;
    flex: 1;
  }

  .ps-group {
    display: flex;
    flex-direction: column;
  }

  .ps-item {
    display: grid;
    grid-template-columns: 1fr auto;
    grid-template-rows: auto auto;
    gap: 0 8px;
    padding: 6px 14px;
    transition: background 0.12s;
  }

  .ps-item:hover {
    background: color-mix(in srgb, var(--border) 40%, transparent);
  }

  .ps-child {
    padding: 3px 14px 3px 26px;
  }

  .ps-bar-wrap {
    grid-column: 1 / -1;
    height: 3px;
    background: var(--border);
    border-radius: 2px;
    margin-bottom: 5px;
    overflow: hidden;
  }

  .ps-child .ps-bar-wrap {
    height: 2px;
    margin-bottom: 3px;
  }

  .ps-bar {
    height: 100%;
    border-radius: 2px;
    opacity: 0.7;
    transition: width 0.3s ease;
  }

  .ps-bar-child {
    opacity: 0.5;
  }

  .ps-labels {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .ps-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .ps-name {
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text);
  }

  .ps-child-name {
    font-size: 11px;
    font-weight: 400;
    color: var(--text-muted);
  }

  .ps-dur {
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    align-self: center;
  }

  .ps-child-dur {
    font-size: 11px;
    font-weight: 400;
    color: var(--text-muted);
  }
</style>
