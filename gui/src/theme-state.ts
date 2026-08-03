// Reactive MD3 theme state shared by main.ts (loader) and Root.vue (provider).
import { ref, computed } from 'vue'
import { zhCN, enUS, dateZhCN, dateEnUS } from 'naive-ui'
import { themeFromSeed, themeOverrides, accentFromAbgr, SEED_FALLBACK } from './theme'

export const themeSeed = ref<number>(SEED_FALLBACK)
export const locale = ref(zhCN)
export const dateLocale = ref(dateZhCN)
export const overrides = computed(() => themeOverrides(themeFromSeed(themeSeed.value)))

export function applyAccent(v: number | null) {
  if (v != null && v !== 0) themeSeed.value = accentFromAbgr(v)
}

export function setLocale(l: 'zh' | 'en') {
  locale.value = l === 'en' ? enUS : zhCN
  dateLocale.value = l === 'en' ? dateEnUS : dateZhCN
}
