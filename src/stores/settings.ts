import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '../api'
import type { FilterRule, FilterRuleType, ProjectMatchRule, Settings } from '../schemas'

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<Settings>({
    minDurationSecs: 300,
    mergeGapSecs: 120,
    idleTimeoutSecs: 300,
    timelineStartHour: 7,
    timelineEndHour: 22,
    startOnLogin: true,
    snapMinutes: 5,
    windowSummaryMinSecs: 30,
    titleSplitApps: [
      'Brave',
      'Chrome',
      'Firefox',
      'msedge',
      'Opera',
      'Vivaldi',
      'Arc',
      'Zen',
      'Chromium',
    ],
    weekStartsOn: 1,
    payScheduleFrequency: 'weekly',
    payScheduleAnchor: new Date().toISOString().slice(0, 10),
  })
  const filterRules = ref<FilterRule[]>([])
  const projectMatchRules = ref<ProjectMatchRule[]>([])

  async function load() {
    const [s, rules, matchRules] = await Promise.all([
      api.getSettings(),
      api.getFilterRules(),
      api.getProjectMatchRules(),
    ])
    settings.value = s
    filterRules.value = rules
    projectMatchRules.value = matchRules
  }

  async function save(s: Settings) {
    await api.saveSettings(s)
    settings.value = s
  }

  async function createRule(ruleType: FilterRuleType, value: string) {
    const rule = await api.createFilterRule(ruleType, value)
    filterRules.value = [...filterRules.value, rule]
  }

  async function deleteRule(id: number) {
    await api.deleteFilterRule(id)
    filterRules.value = filterRules.value.filter((r) => r.id !== id)
  }

  async function createMatchRule(projectId: number, ruleType: FilterRuleType, value: string) {
    const rule = await api.createProjectMatchRule(projectId, ruleType, value)
    projectMatchRules.value = [...projectMatchRules.value, rule]
  }

  async function deleteMatchRule(id: number) {
    await api.deleteProjectMatchRule(id)
    projectMatchRules.value = projectMatchRules.value.filter((r) => r.id !== id)
  }

  return {
    settings,
    filterRules,
    projectMatchRules,
    load,
    save,
    createRule,
    deleteRule,
    createMatchRule,
    deleteMatchRule,
  }
})
