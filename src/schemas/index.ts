import { z } from 'zod'

export const ActivityBlockSchema = z.object({
  appName: z.string(),
  windowTitle: z.string(),
  startedAt: z.string(),
  endedAt: z.string(),
  durationSecs: z.number(),
})
export type ActivityBlock = z.infer<typeof ActivityBlockSchema>

export const ProjectSchema = z.object({
  id: z.number(),
  name: z.string(),
  color: z.string(),
  archivedAt: z.string().nullable(),
  parentId: z.number().nullable(),
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

export const ProjectMatchRuleSchema = z.object({
  id: z.number(),
  projectId: z.number(),
  ruleType: FilterRuleTypeSchema,
  value: z.string(),
})
export type ProjectMatchRule = z.infer<typeof ProjectMatchRuleSchema>

export const SuggestedEntrySchema = z.object({
  projectId: z.number(),
  startedAt: z.string(),
  endedAt: z.string(),
})
export type SuggestedEntry = z.infer<typeof SuggestedEntrySchema>

export const SettingsSchema = z.object({
  minDurationSecs: z.number(),
  mergeGapSecs: z.number(),
  idleTimeoutSecs: z.number(),
  timelineStartHour: z.number(),
  timelineEndHour: z.number(),
  startOnLogin: z.boolean(),
  snapMinutes: z.number(),
  windowSummaryMinSecs: z.number(),
  titleSplitApps: z.array(z.string()),
  weekStartsOn: z.number(),
  payScheduleFrequency: z.enum(['weekly', 'fortnightly']),
  payScheduleAnchor: z.string(),
})
export type Settings = z.infer<typeof SettingsSchema>

export const IdleReturnEventSchema = z.object({
  idleSecs: z.number(),
  idleStartedAt: z.string(),
  idleEndedAt: z.string(),
})
export type IdleReturnEvent = z.infer<typeof IdleReturnEventSchema>

export const WindowSummaryItemSchema = z.object({
  appName: z.string(),
  windowTitle: z.string(),
  totalSecs: z.number(),
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
