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
    suggestionGapSecs: 300,
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
    dailyReminderEnabled: false,
    dailyReminderTime: '17:00',
    mcpAllowWrites: true,
    expectedMinutesPerDay: 0,
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
  /** The code the user types at microsoft.com/devicelogin, while connecting. */
  const m365DeviceCode = ref<string | null>(null)
  const m365VerificationUri = ref('')

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

  /**
   * Re-read only the match rules.
   *
   * The MCP server can add or change rules while the app is open and nothing
   * notifies the frontend, so the Projects page re-reads on window focus. Kept
   * separate from `load()` deliberately: `load()` also replaces `settings`, which
   * is mirrored into the Settings page's in-progress form and would clobber it.
   */
  async function loadMatchRules() {
    projectMatchRules.value = await api.getProjectMatchRules()
  }

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

  async function createMatchRule(
    projectId: number,
    name: string,
    conditions: MatchCondition[],
    subGroupPattern: string | null = null,
  ) {
    const rule = await api.createProjectMatchRule(projectId, name, conditions, subGroupPattern)
    projectMatchRules.value = [...projectMatchRules.value, rule]
    return rule
  }

  async function updateMatchRule(
    id: number,
    name: string,
    conditions: MatchCondition[],
    subGroupPattern: string | null = null,
  ) {
    await api.updateProjectMatchRule(id, name, conditions, subGroupPattern)
    const idx = projectMatchRules.value.findIndex((r) => r.id === id)

    if (idx >= 0) {
      projectMatchRules.value[idx] = {
        ...projectMatchRules.value[idx],
        name,
        conditions,
        subGroupPattern,
      }
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

  /**
   * Begin device-code sign-in.
   *
   * The user is shown a short code and a URL rather than being bounced through a
   * localhost redirect: no callback listener to be blocked by an endpoint agent,
   * and nothing to configure in Azure beyond "allow public client flows".
   */
  async function connectM365() {
    m365Connecting.value = true
    m365DeviceCode.value = null

    try {
      const prompt = await api.startM365Login(m365ClientId.value.trim() || undefined)
      m365DeviceCode.value = prompt.userCode
      m365VerificationUri.value = prompt.verificationUri
      window.open(prompt.verificationUri, '_blank')

      await pollM365Status(prompt.expiresIn)
    } finally {
      m365Connecting.value = false
      m365DeviceCode.value = null
    }
  }

  /**
   * Poll until Microsoft reports the code was used.
   *
   * The backend is doing the token exchange; this only watches the status. The
   * poll runs for as long as the code is valid, because the user may well be
   * typing it on their phone.
   */
  async function pollM365Status(expiresInSecs = 900) {
    const attempts = Math.ceil(expiresInSecs / 5)

    for (let i = 0; i < attempts; i++) {
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
    m365DeviceCode,
    m365VerificationUri,
    load,
    save,
    createRule,
    deleteRule,
    createMatchRule,
    loadMatchRules,
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
