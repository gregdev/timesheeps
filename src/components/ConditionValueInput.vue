<script setup lang="ts">
  /**
   * Search-as-you-type value input for a match condition.
   *
   * Values are suggested from real recorded activity rather than typed blind —
   * that is what stops rules like "sdg-buyflow" being written when the actual
   * window title says "soa-buyflow". Free text is still allowed.
   */
  import { computed, nextTick, ref, watch } from 'vue'
  import { useSettingsStore } from '../stores/settings'
  import { formatSeconds } from '../composables/useFormat'
  import type { MatchField } from '../schemas'
  import AppIcon from './AppIcon.vue'

  const props = defineProps<{
    modelValue: string
    field: MatchField
    /** App the condition is anchored to, used to narrow title suggestions. */
    appName?: string
    placeholder?: string
  }>()

  const emit = defineEmits<{ (e: 'update:modelValue', value: string): void }>()

  const settingsStore = useSettingsStore()

  const open = ref(false)
  const highlighted = ref(-1)
  const inputRef = ref<HTMLInputElement>()
  let blurTimer: ReturnType<typeof setTimeout> | undefined

  interface Option {
    value: string
    appName: string
    seconds: number
  }

  const options = computed<Option[]>(() => {
    const query = props.modelValue.trim().toLowerCase()

    if (props.field === 'app_name') {
      return settingsStore.knownApps
        .filter((a) => !query || a.appName.toLowerCase().includes(query))
        .slice(0, 40)
        .map((a) => ({ value: a.appName, appName: a.appName, seconds: a.totalSecs }))
    }

    // Title suggestions: prefer the app the rule is anchored to.
    const source = props.appName
      ? settingsStore.knownApps.filter(
          (a) => a.appName.toLowerCase() === props.appName?.toLowerCase(),
        )
      : settingsStore.knownApps

    const seen = new Set<string>()
    const out: Option[] = []

    for (const app of source) {
      for (const title of app.sampleTitles) {
        if (seen.has(title) || (query && !title.toLowerCase().includes(query))) {
          continue
        }

        seen.add(title)
        out.push({ value: title, appName: app.appName, seconds: app.totalSecs })
      }
    }

    return out.slice(0, 40)
  })

  function choose(option: Option) {
    emit('update:modelValue', option.value)
    open.value = false
    highlighted.value = -1
  }

  function onFocus() {
    if (blurTimer) {
      clearTimeout(blurTimer)
    }

    open.value = true
  }

  function onBlur() {
    // Delay so a click on an option lands before the list closes.
    blurTimer = setTimeout(() => {
      open.value = false
      highlighted.value = -1
    }, 150)
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault()

      if (!open.value) {
        open.value = true
        return
      }

      highlighted.value = Math.min(highlighted.value + 1, options.value.length - 1)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      highlighted.value = Math.max(highlighted.value - 1, -1)
    } else if (e.key === 'Enter') {
      const option = options.value[highlighted.value]

      if (option) {
        e.preventDefault()
        choose(option)
      }
    } else if (e.key === 'Escape') {
      open.value = false
      highlighted.value = -1
    }
  }

  watch(
    () => props.field,
    async () => {
      highlighted.value = -1
      await nextTick()

      if (props.field === 'app_name' || props.field === 'window_title') {
        void settingsStore.loadKnownApps()
      }
    },
  )
</script>

<template>
  <div class="value-input">
    <input
      ref="inputRef"
      :value="modelValue"
      :placeholder="placeholder"
      spellcheck="false"
      autocomplete="off"
      @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
      @focus="onFocus"
      @blur="onBlur"
      @keydown="onKeydown"
    />

    <ul v-if="open && options.length > 0" class="value-options">
      <li
        v-for="(option, i) in options"
        :key="option.value + i"
        class="value-option"
        :class="{ highlighted: highlighted === i }"
        @mousedown.prevent="choose(option)"
      >
        <AppIcon v-if="field === 'app_name'" :app-name="option.value" :size="16" />
        <span class="value-text">{{ option.value }}</span>
        <span class="value-meta">{{ formatSeconds(option.seconds) }}</span>
      </li>
    </ul>
  </div>
</template>

<style scoped>
  .value-input {
    position: relative;
    flex: 1;
    min-width: 0;
  }

  .value-options {
    position: absolute;
    top: calc(100% + 2px);
    left: 0;
    right: 0;
    z-index: 60;
    max-height: 220px;
    overflow-y: auto;
    margin: 0;
    padding: var(--space-1);
    list-style: none;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    box-shadow: var(--shadow-md);
  }

  .value-option {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: var(--space-1) var(--space-2);
    border-radius: 4px;
    font-size: var(--text-sm);
    cursor: pointer;
  }

  .value-option.highlighted {
    background: var(--surface-2);
  }

  .value-text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .value-meta {
    color: var(--text-faint);
    font-size: var(--text-xs);
    flex-shrink: 0;
  }
</style>
