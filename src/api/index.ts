import { invoke } from '@tauri-apps/api/core'
import { z } from 'zod'
import {
  ActivityBlockSchema,
  FilterRuleSchema,
  ProjectMatchRuleSchema,
  ProjectSchema,
  SearchResultsSchema,
  SettingsSchema,
  SuggestedEntrySchema,
  TimeEntrySchema,
  TimerStateSchema,
  WindowSummaryItemSchema,
  type FilterRuleType,
  type Settings,
  type TimerState,
} from '../schemas'

async function callArray<T>(
  schema: z.ZodType<T>,
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T[]> {
  const result = await invoke(cmd, args)
  return z.array(schema).parse(result)
}

async function callOne<T>(
  schema: z.ZodType<T>,
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const result = await invoke(cmd, args)
  return schema.parse(result)
}

export const api = {
  getActivityForDay: (date: string) =>
    callArray(ActivityBlockSchema, 'get_activity_for_day', { date }),

  getWindowSummaryForDay: (date: string) =>
    callArray(WindowSummaryItemSchema, 'get_window_summary_for_day', { date }),
  getProjects: () => callArray(ProjectSchema, 'get_projects'),
  createProject: (name: string, color: string, parentId: number | null = null) =>
    callOne(ProjectSchema, 'create_project', { payload: { name, color, parentId } }),
  updateProject: (id: number, name: string, color: string, parentId: number | null = null) =>
    invoke<void>('update_project', { payload: { id, name, color, parentId } }),
  archiveProject: (id: number) => invoke<void>('archive_project', { id }),

  getTimeEntriesForDay: (date: string) =>
    callArray(TimeEntrySchema, 'get_time_entries_for_day', { date }),
  createTimeEntry: (
    date: string,
    projectId: number,
    startMinutes: number,
    endMinutes: number,
    note: string,
  ) =>
    callOne(TimeEntrySchema, 'create_time_entry', {
      payload: { date, projectId, startMinutes, endMinutes, note },
    }),
  updateTimeEntry: (
    id: number,
    projectId: number,
    startMinutes: number,
    endMinutes: number,
    note: string,
  ) =>
    invoke<void>('update_time_entry', {
      payload: { id, projectId, startMinutes, endMinutes, note },
    }),
  deleteTimeEntry: (id: number) => invoke<void>('delete_time_entry', { id }),

  getSettings: () => callOne(SettingsSchema, 'get_settings'),
  saveSettings: (settings: Settings) => invoke<void>('save_settings', { settings }),
  checkClaudeMcp: () => invoke<boolean>('check_claude_mcp'),
  setupClaudeMcp: () => invoke<void>('setup_claude_mcp'),
  checkScreenRecordingPermission: () => invoke<boolean>('check_screen_recording_permission'),
  requestScreenRecordingPermission: () => invoke<void>('request_screen_recording_permission'),

  getFilterRules: () => callArray(FilterRuleSchema, 'get_filter_rules'),
  createFilterRule: (ruleType: FilterRuleType, value: string) =>
    callOne(FilterRuleSchema, 'create_filter_rule', { payload: { ruleType, value } }),
  deleteFilterRule: (id: number) => invoke<void>('delete_filter_rule', { id }),

  getProjectMatchRules: () => callArray(ProjectMatchRuleSchema, 'get_project_match_rules'),
  createProjectMatchRule: (projectId: number, ruleType: FilterRuleType, value: string) =>
    callOne(ProjectMatchRuleSchema, 'create_project_match_rule', {
      payload: { projectId, ruleType, value },
    }),
  deleteProjectMatchRule: (id: number) => invoke<void>('delete_project_match_rule', { id }),
  getSuggestedEntriesForDay: (date: string) =>
    callArray(SuggestedEntrySchema, 'get_suggested_entries_for_day', { date }),

  search: (query: string) => callOne(SearchResultsSchema, 'search', { query }),

  // ── Timer ───────────────────────────────────────────────────────────────
  startTimer: (projectId: number, note: string): Promise<TimerState> =>
    callOne(TimerStateSchema, 'start_timer', { projectId, note }),
  pauseTimer: (): Promise<TimerState> => callOne(TimerStateSchema, 'pause_timer'),
  resumeTimer: (): Promise<TimerState> => callOne(TimerStateSchema, 'resume_timer'),
  stopTimer: (): Promise<TimerState> => callOne(TimerStateSchema, 'stop_timer'),
  getTimerState: (): Promise<TimerState> => callOne(TimerStateSchema, 'get_timer_state'),
}
