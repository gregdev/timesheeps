<script setup lang="ts">
  import { computed, ref } from 'vue'
  import { format, parseISO } from 'date-fns'
  import { api } from '../api'
  import { minutesToClock } from '../composables/useFormat'
  import { useProjectsStore } from '../stores/projects'
  import type { DayReview, MeetingCandidate, PeriodReview, UnclaimedSuggestion } from '../schemas'

  const props = defineProps<{
    review: PeriodReview | null
    projectName: (id: number) => string
    projectColor: (id: number) => string
    /** Show the gap rows. On a warm-up period they can be noisy. */
    showGaps?: boolean
  }>()

  const emit = defineEmits<{
    navigate: [date: string]
    changed: []
  }>()

  const projectsStore = useProjectsStore()
  const busy = ref<string | null>(null)

  /**
   * Per-meeting project choice, seeded from the remembered mapping.
   *
   * Kept in the panel rather than in the store because it is transient UI state:
   * the durable answer is the mapping the Rust side stores, keyed by series.
   */
  const meetingProject = ref<Map<string, number>>(new Map())

  const activeProjects = computed(() => projectsStore.active)

  const flagged = computed(() => props.review?.days.filter((d) => d.needsAttention) ?? [])
  const cleanDays = computed(() => (props.review?.days.length ?? 0) - flagged.value.length)

  function fmtDay(date: string): string {
    return format(parseISO(date), 'EEE d MMM')
  }

  function tracked(day: DayReview): string {
    return day.trackedSecs >= 60 ? clockish(day.trackedSecs) : 'nothing'
  }

  function clockish(secs: number): string {
    const mins = Math.round(secs / 60)
    return mins >= 60 ? `${Math.floor(mins / 60)}h${mins % 60 ? ` ${mins % 60}m` : ''}` : `${mins}m`
  }

  function selectedProject(m: MeetingCandidate): number {
    return meetingProject.value.get(m.iCalUid) ?? m.projectId ?? 0
  }

  function setMeetingProject(iCalUid: string, event: Event) {
    const value = Number((event.target as HTMLSelectElement).value)
    const next = new Map(meetingProject.value)

    if (value > 0) {
      next.set(iCalUid, value)
    } else {
      next.delete(iCalUid)
    }

    meetingProject.value = next
  }

  /**
   * Log a meeting to the chosen project.
   *
   * A meeting is normally one continuous block, so this is a single entry — and
   * the chair of a meeting that ran long will fix the end by hand like any other
   * entry. The project choice is remembered for the series, so next week's
   * standup arrives pre-filled.
   */
  async function logMeeting(date: string, m: MeetingCandidate) {
    const projectId = selectedProject(m)

    if (projectId <= 0) {
      return
    }

    busy.value = `meeting:${date}:${m.startMinutes}`

    try {
      await api.setMeetingProject(m.iCalUid, projectId)
      await api.createTimeEntry(date, projectId, m.startMinutes, m.endMinutes, m.subject)
      emit('changed')
    } catch (err) {
      console.error('[timesheeps] logging a meeting failed:', err)
    } finally {
      busy.value = null
    }
  }

  async function hideMeeting(m: MeetingCandidate) {
    busy.value = `hide:${m.iCalUid}`

    try {
      await api.hideMeeting(m.iCalUid)
      emit('changed')
    } catch (err) {
      console.error('[timesheeps] hiding a meeting failed:', err)
    } finally {
      busy.value = null
    }
  }

  async function logSuggestion(date: string, s: UnclaimedSuggestion) {
    busy.value = `${date}:${s.startMinutes}`

    try {
      await api.createTimeEntry(date, s.projectId, s.startMinutes, s.endMinutes, '')
      emit('changed')
    } catch (err) {
      console.error('[timesheeps] logging a suggestion failed:', err)
    } finally {
      busy.value = null
    }
  }

  async function acceptAll(day: DayReview) {
    busy.value = day.date

    try {
      await api.acceptSuggestionsForDay(day.date)
      emit('changed')
    } catch (err) {
      console.error('[timesheeps] accepting suggestions failed:', err)
    } finally {
      busy.value = null
    }
  }

  async function saveNote(date: string, event: Event) {
    const note = (event.target as HTMLInputElement).value
    busy.value = `note:${date}`

    try {
      await api.setDayNote(date, note)
      emit('changed')
    } catch (err) {
      console.error('[timesheeps] saving the day note failed:', err)
    } finally {
      busy.value = null
    }
  }
</script>

<template>
  <section v-if="review" class="review-panel">
    <header class="rv-header">
      <span class="rv-title">Needs attention</span>
      <span class="rv-count">
        <template v-if="flagged.length === 0">all {{ cleanDays }} days logged</template>
        <template v-else>
          {{ flagged.length }} day{{ flagged.length === 1 ? '' : 's' }}
          <template v-if="cleanDays > 0">· {{ cleanDays }} clean</template>
        </template>
      </span>
    </header>

    <p v-if="flagged.length === 0" class="rv-clean">
      Nothing outstanding in this period. Copy it and go.
    </p>

    <ul v-else class="rv-list">
      <li v-for="day in flagged" :key="day.date" class="rv-day">
        <button class="rv-day-head" @click="emit('navigate', day.date)">
          <span class="rv-date">{{ fmtDay(day.date) }}</span>
          <span class="rv-totals">
            {{ tracked(day) }} tracked · {{ clockish(day.loggedSecs) }} logged
            <template v-if="day.expectedMinutes > 0">
              · target {{ clockish(day.expectedMinutes * 60) }}
            </template>
          </span>
        </button>

        <div class="rv-chips">
          <span v-for="reason in day.attention" :key="reason" class="rv-chip">{{ reason }}</span>
        </div>

        <div v-if="showGaps !== false && day.gaps.length" class="rv-gaps">
          <span
            v-for="gap in day.gaps"
            :key="`${gap.startMinutes}-${gap.endMinutes}`"
            class="rv-gap"
          >
            {{ minutesToClock(gap.startMinutes) }}–{{ minutesToClock(gap.endMinutes) }} untracked
          </span>
        </div>

        <div v-if="day.meetings.length" class="rv-meetings">
          <div v-for="m in day.meetings" :key="`${m.iCalUid}-${m.startMinutes}`" class="rv-meeting">
            <span class="rv-meeting-icon" :title="m.isTeamsMeeting ? 'Teams meeting' : 'Meeting'">
              ◷
            </span>
            <span
              class="rv-meeting-subject"
              :title="m.organizer ? `Organised by ${m.organizer}` : ''"
            >
              {{ m.subject }}
            </span>
            <span class="rv-sug-time">
              {{ minutesToClock(m.startMinutes) }}–{{ minutesToClock(m.endMinutes) }}
            </span>

            <select
              class="rv-meeting-project"
              :value="selectedProject(m)"
              :disabled="busy === `meeting:${day.date}:${m.startMinutes}`"
              @change="setMeetingProject(m.iCalUid, $event)"
            >
              <option :value="0">Choose project…</option>
              <option v-for="p in activeProjects" :key="p.id" :value="p.id">{{ p.name }}</option>
            </select>

            <button
              class="rv-meeting-log"
              :disabled="
                selectedProject(m) <= 0 || busy === `meeting:${day.date}:${m.startMinutes}`
              "
              @click="logMeeting(day.date, m)"
            >
              Log
            </button>

            <button
              class="rv-meeting-hide"
              :disabled="busy === `hide:${m.iCalUid}`"
              title="Never offer this meeting series again"
              @click="hideMeeting(m)"
            >
              ✕
            </button>
          </div>
        </div>

        <div v-if="day.suggestions.length" class="rv-suggestions">
          <button
            v-for="s in day.suggestions"
            :key="`${s.startMinutes}-${s.endMinutes}-${s.projectId}`"
            class="rv-suggestion"
            :disabled="busy === `${day.date}:${s.startMinutes}`"
            @click="logSuggestion(day.date, s)"
          >
            <span class="rv-dot" :style="{ background: projectColor(s.projectId) }" />
            <span class="rv-sug-name">{{ projectName(s.projectId) }}</span>
            <span class="rv-sug-time">
              {{ minutesToClock(s.startMinutes) }}–{{ minutesToClock(s.endMinutes) }}
            </span>
            <span class="rv-sug-action">Log</span>
          </button>

          <button
            v-if="day.suggestions.length > 1"
            class="rv-accept-all"
            :disabled="busy === day.date"
            @click="acceptAll(day)"
          >
            Log all {{ day.suggestions.length }}
          </button>
        </div>

        <input
          class="rv-note"
          :value="day.note"
          placeholder="What did you work on? Saved when you leave the field."
          :disabled="busy === `note:${day.date}`"
          @change="saveNote(day.date, $event)"
        />
      </li>
    </ul>
  </section>
</template>

<style scoped>
  .review-panel {
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    flex-shrink: 0;
    overflow: hidden;
  }

  .rv-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-3);
    padding: var(--space-2) 14px;
    border-bottom: 1px solid var(--border);
  }

  .rv-title {
    font-size: var(--text-xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .rv-count {
    font-size: var(--text-xs);
    color: var(--text-muted);
  }

  .rv-clean {
    margin: 0;
    padding: var(--space-3) 14px;
    font-size: var(--text-sm);
    color: var(--text-muted);
  }

  .rv-list {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .rv-day {
    display: flex;
    flex-direction: column;
    gap: var(--space-1);
    padding: var(--space-3) 14px;
    border-bottom: 1px solid var(--border);
  }

  .rv-day:last-child {
    border-bottom: none;
  }

  .rv-day-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--space-3);
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    text-align: left;
    color: inherit;
    font: inherit;
  }

  .rv-date {
    font-size: var(--text-sm);
    font-weight: 600;
  }

  .rv-day-head:hover .rv-date {
    color: var(--primary);
    text-decoration: underline;
  }

  .rv-totals {
    font-size: var(--text-xs);
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .rv-chips,
  .rv-gaps {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
  }

  .rv-chip {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--warning, #f59e0b) 18%, transparent);
    color: var(--text);
    border: 1px solid color-mix(in srgb, var(--warning, #f59e0b) 40%, transparent);
    white-space: nowrap;
  }

  .rv-gap {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 999px;
    border: 1px dashed var(--border);
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .rv-meetings {
    display: flex;
    flex-direction: column;
    gap: 2px;
    margin-top: var(--space-1);
  }

  .rv-meeting {
    display: flex;
    align-items: center;
    gap: var(--space-1);
    font-size: var(--text-xs);
    padding: 2px 6px;
    border-radius: var(--radius);
    border: 1px solid color-mix(in srgb, var(--primary) 30%, var(--border));
    background: color-mix(in srgb, var(--primary) 6%, transparent);
  }

  .rv-meeting-icon {
    color: var(--primary);
    flex-shrink: 0;
  }

  .rv-meeting-subject {
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .rv-meeting-project {
    margin-left: auto;
    font-size: 10px;
    padding: 1px 2px;
    max-width: 130px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--surface);
    color: var(--text);
  }

  .rv-meeting-log {
    font-size: 10px;
    font-weight: 600;
    padding: 1px 6px;
    border-radius: var(--radius);
    border: 1px solid var(--primary);
    background: transparent;
    color: var(--primary);
    cursor: pointer;
  }

  .rv-meeting-hide {
    font-size: 10px;
    line-height: 1;
    padding: 1px 4px;
    border: none;
    background: none;
    color: var(--text-muted);
    cursor: pointer;
  }

  .rv-meeting-log:disabled,
  .rv-meeting-hide:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .rv-meeting-hide:hover:not(:disabled) {
    color: var(--text);
  }

  .rv-suggestions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-1);
    margin-top: var(--space-1);
  }

  .rv-suggestion {
    display: inline-flex;
    align-items: center;
    gap: var(--space-1);
    font-size: var(--text-xs);
    padding: 2px 6px;
    border-radius: var(--radius);
    border: 1px solid var(--border);
    background: var(--surface-2, transparent);
    color: var(--text);
    cursor: pointer;
  }

  .rv-suggestion:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .rv-suggestion:hover:not(:disabled) {
    border-color: var(--primary);
  }

  .rv-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .rv-sug-time {
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .rv-sug-action {
    color: var(--primary);
    font-weight: 600;
  }

  .rv-accept-all {
    font-size: var(--text-xs);
    padding: 2px 8px;
    border-radius: var(--radius);
    border: 1px solid var(--primary);
    background: transparent;
    color: var(--primary);
    cursor: pointer;
  }

  .rv-accept-all:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .rv-note {
    margin-top: var(--space-1);
    font-size: var(--text-xs);
    padding: var(--space-1) var(--space-2);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg, transparent);
    color: var(--text);
    width: 100%;
  }

  .rv-note::placeholder {
    color: var(--text-muted);
  }
</style>
