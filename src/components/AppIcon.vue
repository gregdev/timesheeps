<script setup lang="ts">
  import { computed, onMounted, watch } from 'vue'
  import { useAppColour } from '../composables/useAppColour'
  import { useAppIcon } from '../composables/useAppIcon'

  const props = withDefaults(defineProps<{ appName: string; size?: number }>(), { size: 20 })

  const { appColour } = useAppColour()
  const { appIcon, ensureIcon, appInitials, iconVersion } = useAppIcon()

  const icon = computed(() => {
    // Touch the version so this recomputes when a fetch lands.
    void iconVersion.value
    return appIcon(props.appName)
  })

  const boxStyle = computed(() => ({
    width: `${props.size}px`,
    height: `${props.size}px`,
    fontSize: `${Math.round(props.size * 0.42)}px`,
  }))

  onMounted(() => ensureIcon(props.appName))
  watch(
    () => props.appName,
    (name) => ensureIcon(name),
  )
</script>

<template>
  <img v-if="icon" :src="icon" :style="boxStyle" class="app-icon" :alt="appName" />
  <span
    v-else
    class="app-avatar"
    :style="{ ...boxStyle, background: appColour(appName) }"
    aria-hidden="true"
  >
    {{ appInitials(appName) }}
  </span>
</template>

<style scoped>
  .app-icon {
    border-radius: 4px;
    object-fit: contain;
    flex-shrink: 0;
    user-select: none;
    -webkit-user-drag: none;
  }

  .app-avatar {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 4px;
    color: #fff;
    font-weight: 700;
    letter-spacing: 0.02em;
    flex-shrink: 0;
    user-select: none;
  }
</style>
