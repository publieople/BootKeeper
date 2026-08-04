<script setup lang="ts">
import { NConfigProvider, NMessageProvider } from 'naive-ui'
import { overrides, nTheme, locale, dateLocale, cssVars } from './theme-state'
import App from './App.vue'
</script>

<template>
  <div :style="cssVars" style="height: 100%">
    <NConfigProvider :theme-overrides="overrides" :theme="nTheme" :locale="locale" :date-locale="dateLocale">
      <NMessageProvider>
        <App />
      </NMessageProvider>
    </NConfigProvider>
  </div>
</template>
