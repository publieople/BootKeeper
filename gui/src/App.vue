<script setup lang="ts">
import { ref, computed, onMounted, h } from 'vue'
import {
  NButton, NDataTable, NTag, NSpace, NEmpty, NModal, NForm, NFormItem,
  NInput, NSelect, NRadioGroup, NRadio, NIcon,
} from 'naive-ui'
import { Refresh, Add as AddIcon, Search as SearchIcon, Sunny as SunIcon, Moon as MoonIcon } from '@vicons/ionicons5'
import { useI18n } from 'vue-i18n'
import type { DataTableColumns, FormInst, FormRules } from 'naive-ui'
import { invoke } from '@tauri-apps/api/core'
import { dark, toggleDark } from './theme-state'

interface StartupItem {
  id: string
  category: 'registry_run' | 'startup_folder' | 'scheduled_task' | 'service'
  name: string
  command: string
  location: string
  signature: 'none' | 'valid' | 'invalid' | 'unknown'
  publisher: string | null
  risk: 'low' | 'medium' | 'high'
  enabled: boolean
}

interface SnapshotMeta {
  id: string
  created_at: string
  operation: string
  entry_count: number
}

const items = ref<StartupItem[]>([])
const snapshots = ref<SnapshotMeta[]>([])
const loading = ref(true)
const currentTab = ref('all')
const { t } = useI18n()
const search = ref('')
const actionError = ref('')

// Add-entry dialog state
const showAdd = ref(false)
const addForm = ref({
  category: 'registry_run' as 'registry_run' | 'startup_folder',
  name: '',
  command: '',
  location: 'HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run',
})
const addFormRef = ref<FormInst | null>(null)

const addRules: FormRules = {
  name: { required: true, message: 'name required', trigger: ['input', 'blur'] },
  command: { required: true, message: 'command required', trigger: ['input', 'blur'] },
}

async function fetchItems() {
  loading.value = true
  try {
    const raw = await invoke<StartupItem[]>('list_items')
    items.value = raw ?? []
    console.log(
      'fetchItems:', items.value.length, 'total,',
      items.value.filter((i) => i.name.endsWith('.disabled')).length, 'disabled'
    )
  } catch (e) {
    console.error('list_items failed:', e)
  } finally {
    loading.value = false
  }
}

async function fetchSnapshots() {
  try {
    snapshots.value = (await invoke<SnapshotMeta[]>('list_snapshots')) ?? []
  } catch (e) {
    console.error('list_snapshots failed:', e)
  }
}

onMounted(() => {
  fetchItems()
  fetchSnapshots()
    console.log("items count:", items.value.length, "disabled:", items.value.filter(function(i) { return i.name.endsWith(".disabled") }).map(function(i) { return i.name }))
  })


const filtered = computed(() => {
  let list = currentTab.value === 'all'
    ? items.value
    : items.value.filter((i) => i.category === currentTab.value)
  const q = search.value.toLowerCase()
  if (q) list = list.filter((i) => i.name.toLowerCase().includes(q) || i.command.toLowerCase().includes(q))
  return list
})

const categoryLabels: Record<string, string> = {
  all: 'nav.all',
  registry_run: 'nav.registry',
  startup_folder: 'nav.folder',
  scheduled_task: 'nav.task',
  service: 'nav.service',
}

const tabs = ['all', 'registry_run', 'startup_folder', 'scheduled_task', 'service', 'snapshots'] as const

function riskTag(risk: StartupItem['risk']) {
  const map = { low: 'success', medium: 'warning', high: 'error' } as const
  return map[risk]
}

function sigType(sig: StartupItem['signature']) {
  if (sig === 'valid') return 'success'
  if (sig === 'invalid') return 'error'
  return 'default'
}

// Dim disabled rows in the DataTable.
function rowProps(row: StartupItem) {
  return row.enabled ? {} : { style: { opacity: '0.55' } }
}

async function doAction(action: 'enable' | 'disable' | 'remove', item: StartupItem) {
  actionError.value = ''
  try {
    const res = await invoke<any>('run_write_action', { action, id: item.id })
    console.log('write result:', res)
    await fetchItems()
    await fetchSnapshots()
  } catch (e) {
    const msg = typeof e === 'string' ? e : JSON.stringify(e)
    actionError.value = `${action} ${item.name}: ${msg}`
    console.error('write action failed:', e)
  }
}

async function submitAdd() {
  try {
    await addFormRef.value?.validate()
  } catch {
    return
  }
  try {
    await invoke('add_item', {
      category: addForm.value.category,
      name: addForm.value.name,
      command: addForm.value.command,
      location: addForm.value.category === 'startup_folder' ? 'user_startup' : addForm.value.location,
    })
    showAdd.value = false
    addForm.value.name = ''
    addForm.value.command = ''
    await fetchItems()
    await fetchSnapshots()
  } catch (e) {
    console.error('add failed:', e)
  }
}

async function restoreSnapshot(snap: SnapshotMeta) {
  try {
    await invoke<any>('restore_item', { snapshotId: snap.id, itemId: '' })
    await fetchItems()
    await fetchSnapshots()
  } catch (e) {
    console.error('restore failed:', e)
  }
}

const columns: DataTableColumns<StartupItem> = [
  { title: () => t('table.name'), key: 'name', sorter: (a, b) => a.name.localeCompare(b.name), width: 160 },
  {
    title: () => t('table.category'),
    key: 'category',
    width: 110,
    render: (row) => h(NTag, { size: 'small' }, () => t(categoryLabels[row.category])),
  },
  { title: () => t('table.command'), key: 'command', ellipsis: { tooltip: true } },
  {
    title: () => t('table.signature'),
    key: 'signature',
    width: 100,
    render: (row) =>
      h(NTag, { size: 'small', type: sigType(row.signature) }, () => t(`sig.${row.signature}`)),
  },
  {
    title: () => t('table.risk'),
    key: 'risk',
    width: 80,
    render: (row) => h(NTag, { size: 'small', type: riskTag(row.risk) }, () => t(`risk.${row.risk}`)),
  },
  {
    title: () => t('table.status'),
    key: 'enabled',
    width: 90,
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

const snapshotColumns: DataTableColumns<SnapshotMeta> = [
  { title: () => t('table.snap_time'), key: 'created_at', width: 200 },
  { title: () => t('table.snap_op'), key: 'operation', width: 120 },
  { title: () => t('table.snap_count'), key: 'entry_count', width: 90 },
  {
    title: () => t('table.snap_actions'),
    key: 'actions',
    width: 120,
    render: (row) =>
      h(NButton, { size: 'small', type: 'primary', onClick: () => restoreSnapshot(row) }, () =>
        t('action.restore')),
  },
]
</script>

<template>
  <div class="app">
    <header class="app-header">
      <div>
        <h1>{{ t('app.title') }}
          <span style="font-size:14px;opacity:0.5;margin-left:8px">
            {{ items.length }} {{ dark ? '●' : '○' }}
          </span>
        </h1>
        <p class="subtitle">{{ t('app.subtitle') }}</p>
      </div>
      <NSpace>
        <NButton size="small" @click="toggleDark" quaternary>
          <template #icon><NIcon><component :is="dark ? SunIcon : MoonIcon" /></NIcon></template>
        </NButton>
        <NButton size="small" type="primary" @click="showAdd = true">
          <template #icon><NIcon><AddIcon /></NIcon></template>
          {{ t('action.add') }}
        </NButton>
        <NButton size="small" @click="fetchItems" :loading="loading">
          <template #icon><NIcon><Refresh /></NIcon></template>
          {{ t('action.refresh') }}
        </NButton>
      </NSpace>
    </header>

    <nav class="tabs">
      <button
        v-for="tab in tabs"
        :key="tab"
        :class="['tab', { active: currentTab === tab }]"
        @click="currentTab = tab"
      >
        {{ t(categoryLabels[tab] ?? `nav.${tab}`) }}
      </button>
    </nav>

    <NInput
      v-if="currentTab !== 'snapshots'"
      v-model:value="search"
      :placeholder="t('search.placeholder')"
      clearable
      size="small"
      style="margin-bottom: 10px"
    >
      <template #prefix><NIcon><SearchIcon /></NIcon></template>
    </NInput>

    <main>
      <div v-if="actionError" class="action-error">{{ actionError }}</div>
      <div v-if="currentTab === 'snapshots'">
        <p class="retention">{{ t('snapshot_retention') }}</p>
        <NDataTable
          :columns="snapshotColumns"
          :data="snapshots"
          :row-key="(row: SnapshotMeta) => row.id"
        >
          <template #empty>
            <NEmpty :description="t('empty_snapshots')" />
          </template>
        </NDataTable>
      </div>
      <NDataTable
        v-else
        :columns="columns"
        :data="filtered"
        :loading="loading"
        :bordered="true"
        :row-key="(row: StartupItem) => row.id"
        :row-props="rowProps"
      >
        <template #empty>
          <NEmpty :description="t('empty')" />
        </template>
      </NDataTable>
    </main>

    <NModal v-model:show="showAdd" preset="card" :title="t('action.add')" style="width: 480px">
      <NForm ref="addFormRef" :model="addForm" :rules="addRules" label-placement="top">
        <NFormItem :label="t('table.category')" path="category">
          <NRadioGroup v-model:value="addForm.category">
            <NRadio value="registry_run">{{ t('nav.registry') }}</NRadio>
            <NRadio value="startup_folder">{{ t('nav.folder') }}</NRadio>
          </NRadioGroup>
        </NFormItem>
        <NFormItem :label="t('table.name')" path="name">
          <NInput v-model:value="addForm.name" placeholder="MyApp" />
        </NFormItem>
        <NFormItem
          :label="addForm.category === 'registry_run' ? t('table.command') : t('add.source_file')"
          path="command"
        >
          <NInput v-model:value="addForm.command" placeholder="C:\Path\to\app.exe" />
        </NFormItem>
        <NFormItem v-if="addForm.category === 'registry_run'" :label="t('table.location')" path="location">
          <NSelect
            v-model:value="addForm.location"
            :options="[
              { label: 'HKCU\\...\\Run', value: 'HKCU\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run' },
              { label: 'HKLM\\...\\Run', value: 'HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run' },
            ]"
          />
        </NFormItem>
      </NForm>
      <template #footer>
        <NSpace justify="end">
          <NButton @click="showAdd = false">{{ t('action.cancel') }}</NButton>
          <NButton type="primary" @click="submitAdd">{{ t('action.add') }}</NButton>
        </NSpace>
      </template>
    </NModal>
  </div>
</template>

<style>
* { box-sizing: border-box; margin: 0; padding: 0; }
html, body, #app { height: 100%; }
body {
  font-family: -apple-system, 'Segoe UI', 'Microsoft YaHei', sans-serif;
  background: var(--bk-body-bg, #f5f5f5);
  color: var(--bk-text, #1f1f1f);
}
.app { max-width: 1100px; margin: 0 auto; padding: 24px; }
.app-header {
  display: flex; justify-content: space-between; align-items: center;
  margin-bottom: 16px;
}
.app-header h1 { font-size: 24px; }
.subtitle { color: var(--bk-text-sub, #666); font-size: 13px; margin-top: 2px; }
.tabs { display: flex; gap: 8px; margin-bottom: 16px; flex-wrap: wrap; }
.tab {
  padding: 6px 14px; border: 1px solid var(--bk-border, #ddd); border-radius: 999px;
  background: var(--bk-card-bg, #fff); cursor: pointer; font-size: 13px;
  color: var(--bk-text-sub, #555); transition: all .15s;
}
.tab:hover { border-color: var(--bk-primary, #18a058); color: var(--bk-primary, #18a058); }
.tab.active {
  background: var(--bk-primary, #18a058); border-color: var(--bk-primary, #18a058); color: #fff;
}
main {
  background: var(--bk-card-bg, #fff); border-radius: 12px; padding: 8px;
  box-shadow: 0 1px 3px rgba(0,0,0,.08);
}
.retention { padding: 8px 12px; color: var(--bk-text-sub, #888); font-size: 12px; }
.action-error {
  padding: 10px 12px; margin-bottom: 12px; border-radius: 8px;
  background: var(--bk-error-bg, rgba(192,48,48,0.12)); color: #c03030; font-size: 13px;
  border: 1px solid rgba(192,48,48,0.25); white-space: pre-wrap; word-break: break-all;
}
</style>
