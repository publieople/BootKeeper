<script setup lang="ts">
import { ref, computed, onMounted, h } from 'vue'
import { NButton, NDataTable, NTag, NSpace, NEmpty } from 'naive-ui'
import { Refresh } from '@vicons/ionicons5'
import { useI18n } from 'vue-i18n'
import type { DataTableColumns } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'

interface StartupItem {
  id: string
  category: 'registry_run' | 'startup_folder' | 'scheduled_task'
  name: string
  command: string
  location: string
  signature: 'none' | 'valid' | 'invalid' | 'unknown'
  publisher: string | null
  risk: 'low' | 'medium' | 'high'
  enabled: boolean
}

const items = ref<StartupItem[]>([])
const loading = ref(true)
const currentCategory = ref<'all' | 'registry_run' | 'startup_folder' | 'scheduled_task'>('all')
const { t } = useI18n()

async function fetchItems() {
  loading.value = true
  try {
    items.value = (await invoke<StartupItem[]>('list_items')) ?? []
  } catch (e) {
    console.error('list_items failed:', e)
  } finally {
    loading.value = false
  }
}

onMounted(fetchItems)

const filtered = computed(() =>
  currentCategory.value === 'all'
    ? items.value
    : items.value.filter((i) => i.category === currentCategory.value),
)

const categoryLabels: Record<string, string> = {
  all: 'nav.all',
  registry_run: 'nav.registry',
  startup_folder: 'nav.folder',
  scheduled_task: 'nav.task',
}

const tabs = ['all', 'registry_run', 'startup_folder', 'scheduled_task'] as const

function riskTag(risk: StartupItem['risk']) {
  const map = { low: 'success', medium: 'warning', high: 'error' } as const
  return map[risk]
}

function sigType(sig: StartupItem['signature']) {
  if (sig === 'valid') return 'success'
  if (sig === 'invalid') return 'error'
  return 'default'
}

async function doAction(action: 'enable' | 'disable' | 'remove', item: StartupItem) {
  try {
    await invoke('run_write_action', { action, id: item.id })
    await fetchItems()
  } catch (e) {
    console.error('write action failed:', e)
  }
}

const columns: DataTableColumns<StartupItem> = [
  { title: () => t('table.name'), key: 'name', sorter: (a, b) => a.name.localeCompare(b.name) },
  {
    title: () => t('table.category'),
    key: 'category',
    width: 130,
    render: (row) => h(NTag, { size: 'small' }, () => t(categoryLabels[row.category])),
  },
  { title: () => t('table.command'), key: 'command', ellipsis: { tooltip: true } },
  {
    title: () => t('table.signature'),
    key: 'signature',
    width: 110,
    render: (row) =>
      h(NTag, { size: 'small', type: sigType(row.signature) }, () => t(`sig.${row.signature}`)),
  },
  {
    title: () => t('table.risk'),
    key: 'risk',
    width: 90,
    render: (row) => h(NTag, { size: 'small', type: riskTag(row.risk) }, () => t(`risk.${row.risk}`)),
  },
  {
    title: () => t('table.status'),
    key: 'enabled',
    width: 100,
    render: (row) =>
      h(NTag, { size: 'small', type: row.enabled ? 'success' : 'default' }, () =>
        t(`status.${row.enabled ? 'enabled' : 'disabled'}`)),
  },
  {
    title: () => t('table.actions'),
    key: 'actions',
    width: 200,
    render: (row) =>
      h(NSpace, { size: 4 }, () => [
        row.enabled
          ? h(NButton, { size: 'small', onClick: () => doAction('disable', row) }, () => t('action.disable'))
          : h(NButton, { size: 'small', type: 'success', onClick: () => doAction('enable', row) }, () => t('action.enable')),
        h(NButton, { size: 'small', type: 'error', quaternary: true, onClick: () => doAction('remove', row) }, () => t('action.remove')),
      ]),
  },
]
</script>

<template>
  <div class="app">
    <header class="app-header">
      <div>
        <h1>{{ t('app.title') }}</h1>
        <p class="subtitle">{{ t('app.subtitle') }}</p>
      </div>
      <NSpace>
        <NButton size="small" @click="fetchItems" :loading="loading">
          <template #icon><Refresh /></template>
          {{ t('action.refresh') }}
        </NButton>
      </NSpace>
    </header>

    <nav class="tabs">
      <button
        v-for="tab in tabs"
        :key="tab"
        :class="['tab', { active: currentCategory === tab }]"
        @click="currentCategory = tab"
      >
        {{ t(categoryLabels[tab]) }}
      </button>
    </nav>

    <main>
      <NDataTable
        :columns="columns"
        :data="filtered"
        :loading="loading"
        :bordered="true"
        :row-key="(row: StartupItem) => row.id"
      >
        <template #empty>
          <NEmpty :description="t('empty')" />
        </template>
      </NDataTable>
    </main>
  </div>
</template>

<style>
* { box-sizing: border-box; margin: 0; padding: 0; }
html, body, #app { height: 100%; }
body {
  font-family: -apple-system, 'Segoe UI', 'Microsoft YaHei', sans-serif;
  background: #f5f5f5;
  color: #1f1f1f;
}
.app { max-width: 1100px; margin: 0 auto; padding: 24px; }
.app-header {
  display: flex; justify-content: space-between; align-items: center;
  margin-bottom: 16px;
}
.app-header h1 { font-size: 24px; }
.subtitle { color: #666; font-size: 13px; margin-top: 2px; }
.tabs { display: flex; gap: 8px; margin-bottom: 16px; }
.tab {
  padding: 6px 14px; border: 1px solid #ddd; border-radius: 999px;
  background: #fff; cursor: pointer; font-size: 13px; color: #555;
  transition: all .15s;
}
.tab:hover { border-color: #18a058; color: #18a058; }
.tab.active { background: #18a058; border-color: #18a058; color: #fff; }
main { background: #fff; border-radius: 12px; padding: 8px; box-shadow: 0 1px 3px rgba(0,0,0,.08); }
</style>
