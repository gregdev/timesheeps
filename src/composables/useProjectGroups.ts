import { computed, type Ref } from 'vue'
import { useProjectsStore } from '../stores/projects'
import type { Project, TimeEntry } from '../schemas'

export interface ProjectGroupChild {
  project: Project
  mins: number
}

export interface ProjectGroup {
  project: Project
  /** Minutes logged directly on this project. */
  ownMins: number
  /** `ownMins` plus every child's minutes. */
  totalMins: number
  children: ProjectGroupChild[]
}

/**
 * Roll per-project minute totals up into parent/child groups, biggest first.
 *
 * Shared by the day, week and pay-period summaries: a child's minutes count
 * towards its parent's total, an orphaned child (archived parent) stands on its
 * own, and groups are ordered by total minutes.
 */
export function groupProjectMinutes(totals: Map<number, number>): ProjectGroup[] {
  const projectsStore = useProjectsStore()
  const map = new Map<number, ProjectGroup>()

  const groupFor = (project: Project): ProjectGroup => {
    let group = map.get(project.id)

    if (!group) {
      group = { project, ownMins: 0, totalMins: 0, children: [] }
      map.set(project.id, group)
    }

    return group
  }

  for (const [projectId, mins] of totals) {
    const project = projectsStore.byId(projectId)

    if (!project) {
      continue
    }

    const parent =
      project.parentId === null || project.parentId === undefined
        ? undefined
        : projectsStore.byId(project.parentId)

    if (parent) {
      groupFor(parent).children.push({ project, mins })
    } else {
      // Standalone project, or a child whose parent was deleted/archived away.
      groupFor(project).ownMins += mins
    }
  }

  return [...map.values()]
    .map((group) => ({
      ...group,
      totalMins: group.ownMins + group.children.reduce((sum, c) => sum + c.mins, 0),
      children: [...group.children].sort((a, b) => b.mins - a.mins),
    }))
    .sort((a, b) => b.totalMins - a.totalMins)
}

/** Per-project minute totals from a list of time entries. */
export function minutesByProject(entries: TimeEntry[]): Map<number, number> {
  const totals = new Map<number, number>()

  for (const entry of entries) {
    const mins = entry.endMinutes - entry.startMinutes
    totals.set(entry.projectId, (totals.get(entry.projectId) ?? 0) + mins)
  }

  return totals
}

/**
 * Parent/child groups for a day (from the day store's summary map) — the shape
 * both `ProjectSummary` and `PeriodSummary` render.
 */
export function useProjectGroups(totals: Ref<Map<number, number>>) {
  const groups = computed(() => groupProjectMinutes(totals.value))
  const totalMins = computed(() => groups.value.reduce((sum, g) => sum + g.totalMins, 0))
  const maxMins = computed(() => groups.value[0]?.totalMins ?? 1)

  return { groups, totalMins, maxMins }
}
