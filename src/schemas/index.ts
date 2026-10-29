import { z } from 'zod'

export const ActivityBlockSchema = z.object({
  appName: z.string(),
  windowTitle: z.string(),
  /** Every distinct title seen inside this merged block; match rules test all of them. */
  windowTitles: z.array(z.string()),
  startedAt: z.string(),
  endedAt: z.string(),
  durationSecs: z.number(),
  /**
   * Owning window handle. This — not the title — is what identifies the window the
   * Window Activity panel groups by, since one window cycles through many titles.
   * 0 for legacy rows recorded before the handle was captured.
   */
  windowId: z.number(),
})
export type ActivityBlock = z.infer<typeof ActivityBlockSchema>

export const ProjectSchema = z.object({
  id: z.number(),
  name: z.string(),
  color: z.string(),
  archivedAt: z.string().nullable(),
  parentId: z.number().nullable(),
  /**
   * Optional regex used to pull a sub-group key (e.g. a Jira ticket such as
   * `ELMS-5813`) out of a window title, so this project's time can be broken
   * down further. A match rule's own pattern overrides it. Null means none.
   */
  subGroupPattern: z.string().nullable(),
})
export type Project = z.infer<typeof ProjectSchema>

export const TimeEntrySchema = z.object({
  id: z.number(),
  date: z.string(),
  projectId: z.number(),
  startMinutes: z.number(),
  endMinutes: z.number(),
  note: z.string(),
})
export type TimeEntry = z.infer<typeof TimeEntrySchema>

export const FilterRuleTypeSchema = z.enum(['title_pattern', 'app_name'])
export type FilterRuleType = z.infer<typeof FilterRuleTypeSchema>

export const FilterRuleSchema = z.object({
  id: z.number(),
  ruleType: FilterRuleTypeSchema,
  value: z.string(),
})
export type FilterRule = z.infer<typeof FilterRuleSchema>

export const MatchFieldSchema = z.enum(['app_name', 'window_title'])
export type MatchField = z.infer<typeof MatchFieldSchema>

export const MatchOperatorSchema = z.enum(['contains', 'equals', 'starts_with', 'ends_with'])
export type MatchOperator = z.infer<typeof MatchOperatorSchema>

export const MatchConditionSchema = z.object({
  field: MatchFieldSchema,
  operator: MatchOperatorSchema,
  value: z.string(),
  /** "is not" — the condition asserts that no title/app name matches. */
  negate: z.boolean(),
})
export type MatchCondition = z.infer<typeof MatchConditionSchema>

/** A named group of AND-ed conditions. Rules are OR-ed and evaluated in `position` order. */
export const ProjectMatchRuleSchema = z.object({
  id: z.number(),
  projectId: z.number(),
  name: z.string(),
  position: z.number(),
  conditions: z.array(MatchConditionSchema),
  /** Overrides the owning project's pattern for the blocks this rule claims. */
  subGroupPattern: z.string().nullable(),
})
export type ProjectMatchRule = z.infer<typeof ProjectMatchRuleSchema>

/** Effectiveness of a rule over a recent window, used to surface dead rules. */
export const RuleStatSchema = z.object({
  ruleId: z.number(),
  projectId: z.number(),
  hits: z.number(),
  matchedSecs: z.number(),
  lastMatchedAt: z.string().nullable(),
  /** Projects whose rules also matched the same activity. */
  overlapsWith: z.array(z.number()),
})
export type RuleStat = z.infer<typeof RuleStatSchema>

/** An app seen in recorded activity, for the search-as-you-type value picker. */
export const KnownAppSchema = z.object({
  appName: z.string(),
  totalSecs: z.number(),
  lastSeen: z.string(),
  exePath: z.string(),
  sampleTitles: z.array(z.string()),
})
export type KnownApp = z.infer<typeof KnownAppSchema>

export const SuggestedEntrySchema = z.object({
  projectId: z.number(),
  ruleId: z.number(),
  startedAt: z.string(),
  endedAt: z.string(),
})
export type SuggestedEntry = z.infer<typeof SuggestedEntrySchema>

export const SettingsSchema = z.object({
  minDurationSecs: z.number(),
  mergeGapSecs: z.number(),
  /**
   * Largest gap between two matched blocks that still becomes one suggested
   * entry. Kept well above `mergeGapSecs` because every window switch leaves a
   * poll interval of dead air. `0` restores merge-on-contact only.
   */
  suggestionGapSecs: z.number(),
  idleTimeoutSecs: z.number(),
  timelineStartHour: z.number(),
  timelineEndHour: z.number(),
  startOnLogin: z.boolean(),
  snapMinutes: z.number(),
  windowSummaryMinSecs: z.number(),
  titleSplitApps: z.array(z.string()),
  titleGroupApps: z.array(z.string()),
  weekStartsOn: z.number(),
  payScheduleFrequency: z.enum(['weekly', 'fortnightly']),
  payScheduleAnchor: z.string(),
  timelineColSplitPct: z.number(),
  layoutWindowSummaryWidth: z.number(),
  layoutProjectSummaryWidth: z.number(),
  autoAcceptSuggested: z.boolean(),
  /** When true, a prompt appears once a day at `dailyReminderTime`. */
  dailyReminderEnabled: z.boolean(),
  /** Local time of day (`HH:MM`, 24-hour) for the daily reminder. */
  dailyReminderTime: z.string(),
  /**
   * When false, the MCP server refuses to create or change anything, including
   * projects, match rules and time entries. Claude cannot change this itself.
   */
  mcpAllowWrites: z.boolean(),
})
export type Settings = z.infer<typeof SettingsSchema>

export const IdleReturnEventSchema = z.object({
  idleSecs: z.number(),
  idleStartedAt: z.string(),
  idleEndedAt: z.string(),
})
export type IdleReturnEvent = z.infer<typeof IdleReturnEventSchema>

/** One project row in the daily timesheet reminder. */
export const DailyReminderProjectSchema = z.object({
  projectId: z.number(),
  projectName: z.string(),
  color: z.string(),
  /** Merged activity attributed to the project today. */
  trackedSecs: z.number(),
  /** Time entries logged against the project today. */
  loggedSecs: z.number(),
})
export type DailyReminderProject = z.infer<typeof DailyReminderProjectSchema>

/**
 * Payload of the `daily-reminder` event emitted by the backend scheduler.
 * Totals come from the same pipeline the timeline uses, not raw activity rows.
 */
export const DailyReminderEventSchema = z.object({
  /** Local `YYYY-MM-DD`. */
  date: z.string(),
  trackedSecs: z.number(),
  loggedSecs: z.number(),
  projects: z.array(DailyReminderProjectSchema),
})
export type DailyReminderEvent = z.infer<typeof DailyReminderEventSchema>

export const WindowSummaryItemSchema = z.object({
  appName: z.string(),
  windowTitle: z.string(),
  totalSecs: z.number(),
  /** Grouping handle, or 0 when the row was grouped by title / extracted key. */
  windowId: z.number(),
})
export type WindowSummaryItem = z.infer<typeof WindowSummaryItemSchema>

export const DaySearchResultSchema = z.object({
  date: z.string(),
  allBlocks: z.array(ActivityBlockSchema),
  matchedBlocks: z.array(ActivityBlockSchema),
  totalMatchedSecs: z.number(),
})
export type DaySearchResult = z.infer<typeof DaySearchResultSchema>

export const SearchResultsSchema = z.object({
  days: z.array(DaySearchResultSchema),
  noteMatches: z.array(TimeEntrySchema),
})
export type SearchResults = z.infer<typeof SearchResultsSchema>

// ── Timer ────────────────────────────────────────────────────────────────────

export const TimerStatusSchema = z.enum(['stopped', 'running', 'paused'])
export type TimerStatus = z.infer<typeof TimerStatusSchema>

export const TimerStateSchema = z.object({
  status: TimerStatusSchema,
  projectId: z.number().nullable(),
  projectName: z.string().nullable(),
  projectColor: z.string().nullable(),
  note: z.string(),
  startedAt: z.string().nullable(),
  accumulatedMs: z.number(),
  pausedAt: z.string().nullable(),
  elapsedMs: z.number(),
})
export type TimerState = z.infer<typeof TimerStateSchema>

// ── Microsoft 365 / Calendar ─────────────────────────────────────────────────

export const CalendarEventSchema = z.object({
  subject: z.string(),
  startAt: z.string(),
  endAt: z.string(),
  isAllDay: z.boolean(),
  organizer: z.string(),
  location: z.string(),
  isTeamsMeeting: z.boolean(),
})
export type CalendarEvent = z.infer<typeof CalendarEventSchema>

export const M365StatusSchema = z.object({
  connected: z.boolean(),
  accountName: z.string(),
})
export type M365Status = z.infer<typeof M365StatusSchema>
