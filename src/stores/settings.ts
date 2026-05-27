import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { api } from '../api'
import type { FilterRule, FilterRuleType, ProjectMatchRule, Settings } from '../schemas'

export type ColourScheme = 'system' | 'light' | 'dark'

function applyColourScheme(scheme: ColourScheme) {
  if (scheme === 'system') {
    document.documentElement.removeAttribute('data-theme')
  } else {
    document.documentElement.setAttribute('data-theme', scheme)
  }
}

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
    titleGroupApps: ['Code'],
    weekStartsOn: 1,
    payScheduleFrequency: 'weekly',
    payScheduleAnchor: new Date().toISOString().slice(0, 10),
  })
  const colourScheme = ref<ColourScheme>(
    (localStorage.getItem('colourScheme') as ColourScheme | null) ?? 'system',
  )

  // Apply immediately on store creation
  applyColourScheme(colourScheme.value)

  watch(colourScheme, (scheme) => {
    localStorage.setItem('colourScheme', scheme)
    applyColourScheme(scheme)
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
    colourScheme,
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
