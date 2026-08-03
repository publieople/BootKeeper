<script setup lang="ts">
import { computed } from 'vue'
import { NConfigProvider, NMessageProvider } from 'naive-ui'
import { overrides, locale, dateLocale, themeSeed } from './theme-state'
import { themeFromSeed } from './theme'
import App from './App.vue'

const cssVars = computed(() => {
  const t = themeFromSeed(themeSeed.value)
  return {
    '--bk-primary': t.primary,
    '--bk-body-bg': t.bodyBg,
    '--bk-card-bg': t.cardBg,
    '--bk-border': t.border,
    '--bk-text': t.text,
    '--bk-text-sub': t.textSub,
  }
})
</script>

<template>
  <div :style="cssVars" style="height: 100%">
    <NConfigProvider :theme-overrides="overrides" :locale="locale" :date-locale="dateLocale">
      <NMessageProvider>
        <App />
      </NMessageProvider>
    </NConfigProvider>
  </div>
</template>
