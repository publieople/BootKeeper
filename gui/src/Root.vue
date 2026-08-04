<script setup lang="ts">
import { NConfigProvider, NMessageProvider } from 'naive-ui'
import { overrides, nTheme, locale, dateLocale } from './theme-state'
import App from './App.vue'
</script>

<template>
  <NConfigProvider :theme-overrides="overrides" :theme="nTheme" :locale="locale" :date-locale="dateLocale">
    <NMessageProvider>
      <App />
    </NMessageProvider>
  </NConfigProvider>
</template>
