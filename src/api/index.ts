import { invoke } from '@tauri-apps/api/core'
import { z } from 'zod'
import {
  ActivityBlockSchema,
  FilterRuleSchema,
  ProjectSchema,
  SettingsSchema,
  TimeEntrySchema,
  type FilterRuleType,
  type Settings,
} from '../schemas'

async function callArray<T>(schema: z.ZodType<T>, cmd: string, args?: Record<string, unknown>): Promise<T[]> {
  const result = await invoke(cmd, args)
  return z.array(schema).parse(result)
}

async function callOne<T>(schema: z.ZodType<T>, cmd: string, args?: Record<string, unknown>): Promise<T> {
  const result = await invoke(cmd, args)
  return schema.parse(result)
}

export const api = {
  getActivityForDay: (date: string) =>
    callArray(ActivityBlockSchema, 'get_activity_for_day', { date }),

  getProjects: () =>
    callArray(ProjectSchema, 'get_projects'),
  createProject: (name: string, color: string) =>
    callOne(ProjectSchema, 'create_project', { payload: { name, color } }),
  updateProject: (id: number, name: string, color: string) =>
    invoke<void>('update_project', { payload: { id, name, color } }),
  archiveProject: (id: number) =>
    invoke<void>('archive_project', { id }),

  getTimeEntriesForDay: (date: string) =>
    callArray(TimeEntrySchema, 'get_time_entries_for_day', { date }),
  createTimeEntry: (date: string, projectId: number, startMinutes: number, endMinutes: number, note: string) =>
    callOne(TimeEntrySchema, 'create_time_entry', { payload: { date, projectId, startMinutes, endMinutes, note } }),
  updateTimeEntry: (id: number, projectId: number, startMinutes: number, endMinutes: number, note: string) =>
    invoke<void>('update_time_entry', { payload: { id, projectId, startMinutes, endMinutes, note } }),
  deleteTimeEntry: (id: number) =>
    invoke<void>('delete_time_entry', { id }),

  getSettings: () =>
    callOne(SettingsSchema, 'get_settings'),
  saveSettings: (settings: Settings) =>
    invoke<void>('save_settings', { settings }),

  getFilterRules: () =>
    callArray(FilterRuleSchema, 'get_filter_rules'),
  createFilterRule: (ruleType: FilterRuleType, value: string) =>
    callOne(FilterRuleSchema, 'create_filter_rule', { payload: { ruleType, value } }),
  deleteFilterRule: (id: number) =>
    invoke<void>('delete_filter_rule', { id }),
}
