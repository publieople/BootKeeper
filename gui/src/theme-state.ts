// Reactive MD3 theme state shared by main.ts (loader) and Root.vue (provider).
import { ref, computed } from 'vue'
import { zhCN, enUS, dateZhCN, dateEnUS, darkTheme } from 'naive-ui'
import { themeFromSeed, themeFromSeedDark, themeOverrides, accentFromAbgr, SEED_FALLBACK } from './theme'

export const themeSeed = ref<number>(SEED_FALLBACK)
export const locale = ref(zhCN)
export const dateLocale = ref(dateZhCN)
export const dark = ref(true) // ponytail: default dark, toggle in header

// System preference override — call once on mount.
export function initDarkMode() {
  const mq = window.matchMedia('(prefers-color-scheme: dark)')
  // respect saved preference, then system, then fall to our default
  const saved = localStorage.getItem('bk-theme')
  if (saved === 'light') dark.value = false
  else if (saved === 'dark') dark.value = true
  else dark.value = mq.matches
  mq.addEventListener('change', (e) => {
    if (!localStorage.getItem('bk-theme')) dark.value = e.matches
  })
}

export function toggleDark() {
  dark.value = !dark.value
  localStorage.setItem('bk-theme', dark.value ? 'dark' : 'light')
}

const md3Light = computed(() => themeFromSeed(themeSeed.value))
const md3Dark = computed(() => themeFromSeedDark(themeSeed.value))
const md3 = computed(() => dark.value ? md3Dark.value : md3Light.value)

export const overrides = computed(() => themeOverrides(md3.value))
export const nTheme = computed(() => (dark.value ? darkTheme : null))
export const cssVars = computed(() => ({
  '--bk-primary': md3.value.primary,
  '--bk-body-bg': md3.value.bodyBg,
  '--bk-card-bg': md3.value.cardBg,
  '--bk-border': md3.value.border,
  '--bk-text': md3.value.text,
  '--bk-text-sub': md3.value.textSub,
}))

export function applyAccent(v: number | null) {
  if (v != null && v !== 0) themeSeed.value = accentFromAbgr(v)
}

export function setLocale(l: 'zh' | 'en') {
  locale.value = l === 'en' ? enUS : zhCN
  dateLocale.value = l === 'en' ? dateEnUS : dateZhCN
}
