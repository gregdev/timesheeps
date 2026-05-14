import { defineStore } from 'pinia'
import { ref } from 'vue'
import { api } from '../api'
import type { FilterRule, FilterRuleType, Settings } from '../schemas'

export const useSettingsStore = defineStore('settings', () => {
  const settings = ref<Settings>({
    minDurationSecs: 60,
    mergeGapSecs: 120,
    idleTimeoutSecs: 300,
    timelineStartHour: 7,
    timelineEndHour: 22,
  })
  const filterRules = ref<FilterRule[]>([])

  async function load() {
    const [s, rules] = await Promise.all([api.getSettings(), api.getFilterRules()])
    settings.value = s
    filterRules.value = rules
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
    filterRules.value = filterRules.value.filter(r => r.id !== id)
  }

  return { settings, filterRules, load, save, createRule, deleteRule }
})
