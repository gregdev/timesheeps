import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { api } from '../api'
import type { Project } from '../schemas'

export const useProjectsStore = defineStore('projects', () => {
  const projects = ref<Project[]>([])

  async function load() {
    projects.value = await api.getProjects()
  }

  const active = computed(() => projects.value.filter((p) => !p.archivedAt))

  // Root projects: active projects with no parent
  const roots = computed(() => active.value.filter((p) => p.parentId === null))

  function byId(id: number): Project | undefined {
    return projects.value.find((p) => p.id === id)
  }

  function childrenOf(parentId: number): Project[] {
    return active.value.filter((p) => p.parentId === parentId)
  }

  async function create(
    name: string,
    color: string,
    parentId: number | null = null,
    subGroupPattern: string | null = null,
  ) {
    const p = await api.createProject(name, color, parentId, subGroupPattern)
    projects.value = [...projects.value, p]
    return p
  }

  async function update(
    id: number,
    name: string,
    color: string,
    parentId: number | null = null,
    subGroupPattern: string | null = null,
  ) {
    await api.updateProject(id, name, color, parentId, subGroupPattern)
    const idx = projects.value.findIndex((p) => p.id === id)

    if (idx >= 0) {
      projects.value[idx] = { ...projects.value[idx], name, color, parentId, subGroupPattern }
    }
  }

  async function archive(id: number) {
    await api.archiveProject(id)
    const idx = projects.value.findIndex((p) => p.id === id)

    if (idx >= 0) {
      projects.value[idx] = { ...projects.value[idx], archivedAt: new Date().toISOString() }
    }
  }

  async function unarchive(id: number) {
    await api.unarchiveProject(id)
    const idx = projects.value.findIndex((p) => p.id === id)

    if (idx >= 0) {
      projects.value[idx] = { ...projects.value[idx], archivedAt: null }
    }
  }

  /** Permanently deletes a project; its time entries and match rules go with it. */
  async function remove(id: number) {
    await api.deleteProject(id)
    projects.value = projects.value.filter((p) => p.id !== id)
  }

  return {
    projects,
    active,
    roots,
    load,
    byId,
    childrenOf,
    create,
    update,
    archive,
    unarchive,
    remove,
  }
})
