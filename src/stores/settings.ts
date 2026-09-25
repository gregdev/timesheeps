import { defineStore } from 'pinia'
import { computed, ref, watch } from 'vue'
import { api } from '../api'
import { localDateKey } from '../composables/useFormat'
import type {
  FilterRule,
  FilterRuleType,
  KnownApp,
  M365Status,
  MatchCondition,
  ProjectMatchRule,
  RuleStat,
  Settings,
} from '../schemas'

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
    payScheduleAnchor: localDateKey(new Date()),
    timelineColSplitPct: 50,
    layoutWindowSummaryWidth: 220,
    layoutProjectSummaryWidth: 220,
    autoAcceptSuggested: false,
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
  const ruleStats = ref<RuleStat[]>([])
  const knownApps = ref<KnownApp[]>([])

  /** Days of history covered by `ruleStats`; persisted so the label is honest. */
  const statsWindowDays = ref<number>(
    Number(localStorage.getItem('ruleStatsWindowDays') ?? 30) || 30,
  )

  const m365Status = ref<M365Status>({ connected: false, accountName: '' })
  const m365Connecting = ref(false)
  const m365ClientId = ref('')

  async function load() {
    const [s, rules, matchRules, m365] = await Promise.all([
      api.getSettings(),
      api.getFilterRules(),
      api.getProjectMatchRules(),
      api.getM365Status().catch(() => ({ connected: false, accountName: '' }) as M365Status),
    ])
    settings.value = s
    filterRules.value = rules
    projectMatchRules.value = matchRules
    m365Status.value = m365
  }

  /** Rules in global precedence order; the backend already sorts by position. */
  const orderedMatchRules = computed(() =>
    [...projectMatchRules.value].sort((a, b) => a.position - b.position || a.id - b.id),
  )

  function statsFor(ruleId: number): RuleStat | undefined {
    return ruleStats.value.find((s) => s.ruleId === ruleId)
  }

  async function refreshRuleStats(days = statsWindowDays.value) {
    statsWindowDays.value = days
    localStorage.setItem('ruleStatsWindowDays', String(days))
    ruleStats.value = await api.getRuleStats(days)
  }

  async function loadKnownApps(days = 90) {
    if (knownApps.value.length > 0) {
      return knownApps.value
    }

    knownApps.value = await api.getKnownApps(days)
    return knownApps.value
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

  async function createMatchRule(projectId: number, name: string, conditions: MatchCondition[]) {
    const rule = await api.createProjectMatchRule(projectId, name, conditions)
    projectMatchRules.value = [...projectMatchRules.value, rule]
    return rule
  }

  async function updateMatchRule(id: number, name: string, conditions: MatchCondition[]) {
    await api.updateProjectMatchRule(id, name, conditions)
    const idx = projectMatchRules.value.findIndex((r) => r.id === id)

    if (idx >= 0) {
      projectMatchRules.value[idx] = { ...projectMatchRules.value[idx], name, conditions }
      projectMatchRules.value = [...projectMatchRules.value]
    }
  }

  async function deleteMatchRule(id: number) {
    await api.deleteProjectMatchRule(id)
    projectMatchRules.value = projectMatchRules.value.filter((r) => r.id !== id)
    ruleStats.value = ruleStats.value.filter((s) => s.ruleId !== id)
  }

  /** Persist a new global precedence order (ids already in their target order). */
  async function reorderMatchRules(orderedIds: number[]) {
    await api.reorderProjectMatchRules(orderedIds)
    orderedIds.forEach((id, index) => {
      const rule = projectMatchRules.value.find((r) => r.id === id)

      if (rule) {
        rule.position = index
      }
    })
    projectMatchRules.value = [...projectMatchRules.value]
  }

  /** Move one rule to the very top of the precedence order. */
  async function promoteMatchRule(id: number) {
    const ids = orderedMatchRules.value.map((r) => r.id).filter((rid) => rid !== id)
    await reorderMatchRules([id, ...ids])
  }

  /** Any other project that already claims this app name, if any. */
  function findAppNameConflict(appName: string, excludeProjectId?: number) {
    const target = appName.toLowerCase()
    return orderedMatchRules.value.find(
      (r) =>
        r.projectId !== excludeProjectId &&
        r.conditions.some(
          (c) => c.field === 'app_name' && !c.negate && c.value.trim().toLowerCase() === target,
        ),
    )
  }

  async function addToTitleSplitApps(appName: string) {
    if (settings.value.titleSplitApps.some((a) => a.toLowerCase() === appName.toLowerCase())) {
      return
    }

    const updated = {
      ...settings.value,
      titleSplitApps: [...settings.value.titleSplitApps, appName],
    }
    await save(updated)
  }

  async function addToTitleGroupApps(appName: string) {
    if (settings.value.titleGroupApps.some((a) => a.toLowerCase() === appName.toLowerCase())) {
      return
    }

    const updated = {
      ...settings.value,
      titleGroupApps: [...settings.value.titleGroupApps, appName],
    }
    await save(updated)
  }

  async function connectM365() {
    m365Connecting.value = true

    try {
      const clientId = m365ClientId.value.trim()

      if (!clientId) {
        throw new Error('Client ID is required')
      }

      const { url } = await api.startM365Login(clientId)
      window.open(url, '_blank')
      // Poll for connection status
      await pollM365Status()
    } finally {
      m365Connecting.value = false
    }
  }

  async function pollM365Status() {
    // Poll for up to 2 minutes for the OAuth callback to complete
    for (let i = 0; i < 24; i++) {
      await new Promise((r) => setTimeout(r, 5000))

      try {
        const status = await api.getM365Status()

        if (status.connected) {
          m365Status.value = status
          return
        }
      } catch {
        // Keep polling
      }
    }
  }

  async function refreshM365Status() {
    try {
      m365Status.value = await api.getM365Status()
    } catch {
      m365Status.value = { connected: false, accountName: '' }
    }
  }

  async function disconnectM365() {
    await api.disconnectM365()
    m365Status.value = { connected: false, accountName: '' }
  }

  return {
    settings,
    colourScheme,
    filterRules,
    projectMatchRules,
    orderedMatchRules,
    ruleStats,
    statsWindowDays,
    knownApps,
    m365Status,
    m365Connecting,
    m365ClientId,
    load,
    save,
    createRule,
    deleteRule,
    createMatchRule,
    updateMatchRule,
    deleteMatchRule,
    reorderMatchRules,
    promoteMatchRule,
    findAppNameConflict,
    statsFor,
    refreshRuleStats,
    loadKnownApps,
    addToTitleSplitApps,
    addToTitleGroupApps,
    connectM365,
    disconnectM365,
    refreshM365Status,
  }
})
