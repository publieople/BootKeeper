import { createApp } from 'vue'
import Root from './Root.vue'
import { i18n } from './i18n'
import { NMessageProvider, NConfigProvider } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { applyAccent, setLocale, initDarkMode } from './theme-state'

// Load Windows accent color -> MD3 seed (falls back to green).
invoke<number | null>('get_accent_color')
  .then(applyAccent)
  .catch(() => {})

setLocale(i18n.global.locale.value === 'en' ? 'en' : 'zh')
initDarkMode()

const app = createApp(Root)
app.use(i18n)
app.component('NMessageProvider', NMessageProvider)
app.component('NConfigProvider', NConfigProvider)
app.mount('#app')
