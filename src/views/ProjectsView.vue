<script setup lang="ts">
  /**
   * Top-level Projects page: project management on the left, match-rule
   * authoring and the global precedence list on the right.
   */
  import { computed, onMounted, ref } from 'vue'
  import { useProjectsStore } from '../stores/projects'
  import { useSettingsStore } from '../stores/settings'
  import ProjectList from '../components/ProjectList.vue'
  import MatchRuleEditor from '../components/MatchRuleEditor.vue'
  import MatchRuleList from '../components/MatchRuleList.vue'
  import type { ProjectMatchRule } from '../schemas'

  const projectsStore = useProjectsStore()
  const settingsStore = useSettingsStore()

  const selectedProjectId = ref<number | null>(null)
  const editingRule = ref<ProjectMatchRule | null>(null)
  const showEditor = ref(false)

  const selectedProject = computed(() =>
    selectedProjectId.value ? projectsStore.byId(selectedProjectId.value) : undefined,
  )

  const projectRules = computed(() =>
    selectedProjectId.value
      ? settingsStore.orderedMatchRules.filter((r) => r.projectId === selectedProjectId.value)
      : [],
  )

  const statsLoading = ref(false)

  async function refreshStats() {
    statsLoading.value = true

    try {
      await settingsStore.refreshRuleStats()
    } finally {
      statsLoading.value = false
    }
  }

  function selectProject(id: number) {
    selectedProjectId.value = id
    editingRule.value = null
    showEditor.value = false
  }

  function startNewRule() {
    editingRule.value = null
    showEditor.value = true
  }

  function startEditRule(rule: ProjectMatchRule) {
    selectedProjectId.value = rule.projectId
    editingRule.value = rule
    showEditor.value = true
  }

  function cancelEditor() {
    showEditor.value = false
    editingRule.value = null
  }

  async function onSaved() {
    cancelEditor()
    await refreshStats()
  }

  async function onChanged() {
    // Project deleted → drop the selection so the editor does not point at it.
    if (selectedProjectId.value && !projectsStore.byId(selectedProjectId.value)) {
      selectedProjectId.value = null
      showEditor.value = false
      editingRule.value = null
    }

    await refreshStats()
  }

  onMounted(async () => {
    if (projectsStore.projects.length === 0) {
      await projectsStore.load()
    }
    if (!selectedProjectId.value && projectsStore.active.length > 0) {
      selectedProjectId.value = projectsStore.active[0].id
    }

    void settingsStore.loadKnownApps()
    await refreshStats()
  })
</script>

<template>
  <div class="projects-view">
    <div class="view-scroll">
      <div class="col col--projects">
        <ProjectList
          :selected-id="selectedProjectId"
          @select="selectProject"
          @changed="onChanged"
        />
      </div>

      <div class="col col--rules">
        <section class="panel">
          <div class="section-header">
            <h2>
              Auto-match rules
              <span v-if="selectedProject" class="project-tag">
                <span class="color-dot" :style="{ background: selectedProject.color }" />
                {{ selectedProject.name }}
              </span>
            </h2>
            <button
              v-if="selectedProjectId && !showEditor"
              class="btn-primary"
              @click="startNewRule"
            >
              + New rule
            </button>
          </div>

          <p class="hint">
            Activity matching a rule becomes a suggestion on the timeline. A rule is one or more
            conditions combined with AND — click a suggestion to accept it as a time entry.
          </p>

          <div v-if="!selectedProjectId" class="empty">Select a project to manage its rules.</div>

          <template v-else>
            <div class="rule-summary">
              <div v-if="projectRules.length === 0" class="empty">
                No rules on this project yet.
              </div>
              <div v-for="rule in projectRules" :key="rule.id" class="summary-row">
                <code class="summary-conditions">
                  {{
                    rule.conditions
                      .map((c) => `${c.field === 'app_name' ? 'app' : 'title'} ${c.value}`)
                      .join(' AND ')
                  }}
                </code>
                <button class="btn-ghost" @click="startEditRule(rule)">Edit</button>
              </div>
            </div>

            <MatchRuleEditor
              v-if="showEditor"
              :project-id="selectedProjectId"
              :rule="editingRule"
              :existing-count="projectRules.length"
              @saved="onSaved"
              @cancel="cancelEditor"
            />
          </template>
        </section>

        <section class="panel">
          <MatchRuleList @edit="startEditRule" />
        </section>
      </div>
    </div>
  </div>
</template>

<style scoped>
  .projects-view {
    flex: 1;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .view-scroll {
    flex: 1;
    overflow-y: auto;
    display: grid;
    grid-template-columns: minmax(320px, 1fr) minmax(420px, 1.4fr);
    gap: var(--space-6);
    padding: var(--space-4) var(--space-5);
    align-items: start;
  }

  @media (width <= 1100px) {
    .view-scroll {
      grid-template-columns: 1fr;
    }
  }

  .col {
    display: flex;
    flex-direction: column;
    gap: var(--space-4);
    min-width: 0;
  }

  .panel {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
    padding: var(--space-4);
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 8px;
  }

  .section-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-3);
    padding-bottom: var(--space-2);
    border-bottom: 1px solid var(--border);
  }

  .section-header h2 {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    margin: 0;
    font-size: var(--text-lg);
    font-weight: 600;
  }

  .project-tag {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px var(--space-2);
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: var(--text-xs);
    font-weight: 500;
  }

  .color-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .hint {
    margin: 0;
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .empty {
    padding: var(--space-2) 0;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .rule-summary {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
  }

  .summary-row {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    font-size: var(--text-xs);
  }

  .summary-conditions {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: monospace;
    color: var(--text-muted);
  }
</style>
