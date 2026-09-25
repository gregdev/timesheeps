import { ref } from 'vue'
import { api } from '../api'

/**
 * Resolved app icons as PNG data URIs, keyed by app name. Module-level so every
 * component shares one cache and each app is only fetched once per session.
 * `null` means "asked and there is no icon" — the UI falls back to an avatar.
 */
const icons = new Map<string, string | null>()
const pending = new Map<string, Promise<string | null>>()
const version = ref(0)

async function fetchIcon(appName: string): Promise<string | null> {
  const existing = pending.get(appName)

  if (existing) {
    return existing
  }

  const promise = api
    .getAppIcon(appName)
    .catch(() => null)
    .then((icon) => {
      icons.set(appName, icon)
      pending.delete(appName)
      version.value++
      return icon
    })

  pending.set(appName, promise)
  return promise
}

export function useAppIcon() {
  /** Cached icon for an app, or null when not resolved / not available. */
  function appIcon(appName: string): string | null {
    return icons.get(appName) ?? null
  }

  /** Kick off extraction if it has not been requested yet. Safe to call repeatedly. */
  function ensureIcon(appName: string): void {
    if (appName && !icons.has(appName) && !pending.has(appName)) {
      void fetchIcon(appName)
    }
  }

  /** Two-character label used by the avatar fallback. */
  function appInitials(appName: string): string {
    const cleaned = appName.replace(/[^\p{L}\p{N} ]+/gu, ' ').trim()

    if (!cleaned) {
      return '?'
    }

    const words = cleaned.split(/\s+/)

    if (words.length === 1) {
      return words[0].slice(0, 2).toUpperCase()
    }

    return (words[0][0] + words[1][0]).toUpperCase()
  }

  return { appIcon, ensureIcon, appInitials, iconVersion: version }
}
