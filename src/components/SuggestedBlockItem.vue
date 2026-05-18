<script setup lang="ts">
  import { computed } from 'vue'
  import { useProjectsStore } from '../stores/projects'
  import { useTimeline } from '../composables/useTimeline'

  const props = defineProps<{
    projectId: number
    startMinutes: number
    endMinutes: number
  }>()

  const emit = defineEmits<{
    (e: 'accept', projectId: number, startMinutes: number, endMinutes: number): void
  }>()

  const projectsStore = useProjectsStore()
  const { minuteToY } = useTimeline()

  const project = computed(() => projectsStore.byId(props.projectId))
  const color = computed(() => project.value?.color ?? '#6366f1')
  const top = computed(() => minuteToY(props.startMinutes))
  const height = computed(() => Math.max(minuteToY(props.endMinutes) - top.value, 4))
</script>

<template>
  <div
    class="suggested-block"
    :style="{ top: top + 'px', height: height + 'px', '--color': color }"
    :title="`Suggested: ${project?.name ?? 'Unknown'} — click to accept`"
    @click.stop="emit('accept', projectId, startMinutes, endMinutes)"
  >
    <span v-if="height > 22" class="project-name">{{ project?.name ?? '?' }}</span>
    <span v-if="height > 14" class="accept-hint">+ Accept</span>
  </div>
</template>

<style scoped>
  .suggested-block {
    position: absolute;
    left: 4px;
    right: 4px;
    border-radius: 5px;
    background: color-mix(in srgb, var(--color) 10%, transparent);
    border: 2px dashed var(--color);
    opacity: 0.75;
    display: flex;
    flex-direction: column;
    justify-content: center;
    padding: 2px 6px;
    overflow: hidden;
    cursor: pointer;
    z-index: 4;
    transition:
      opacity 0.15s,
      background 0.15s;
    user-select: none;
  }

  .suggested-block:hover {
    opacity: 1;
    background: color-mix(in srgb, var(--color) 18%, transparent);
  }

  .project-name {
    font-size: 11px;
    font-weight: 600;
    color: var(--color);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    line-height: 1.3;
  }

  .accept-hint {
    font-size: 10px;
    color: var(--color);
    opacity: 0.7;
    white-space: nowrap;
  }
</style>
