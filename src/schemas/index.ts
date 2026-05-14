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

export const SettingsSchema = z.object({
  minDurationSecs: z.number(),
  mergeGapSecs: z.number(),
  idleTimeoutSecs: z.number(),
  timelineStartHour: z.number(),
  timelineEndHour: z.number(),
})
export type Settings = z.infer<typeof SettingsSchema>

export const IdleReturnEventSchema = z.object({
  idleSecs: z.number(),
  idleStartedAt: z.string(),
  idleEndedAt: z.string(),
})
export type IdleReturnEvent = z.infer<typeof IdleReturnEventSchema>
