import { createI18n } from 'vue-i18n'

const messages = {
  zh: {
    app: { title: 'BootKeeper', subtitle: 'Windows 自启动管理' },
    nav: { all: '全部', registry: '注册表', folder: '启动文件夹', task: '计划任务' },
    table: {
      name: '名称',
      category: '类别',
      command: '命令',
      location: '位置',
      signature: '签名',
      risk: '风险',
      status: '状态',
      actions: '操作',
    },
    risk: { low: '低', medium: '中', high: '高' },
    sig: { none: '未签名', valid: '有效', invalid: '无效', unknown: '未知' },
    status: { enabled: '启用', disabled: '已禁用' },
    action: {
      enable: '启用',
      disable: '禁用',
      remove: '删除',
      refresh: '刷新',
      analyzing: '分析中…',
    },
    empty: '暂无启动项',
    error: '加载失败',
  },
  en: {
    app: { title: 'BootKeeper', subtitle: 'Windows autostart manager' },
    nav: { all: 'All', registry: 'Registry', folder: 'Startup folder', task: 'Scheduled tasks' },
    table: {
      name: 'Name',
      category: 'Category',
      command: 'Command',
      location: 'Location',
      signature: 'Signature',
      risk: 'Risk',
      status: 'Status',
      actions: 'Actions',
    },
    risk: { low: 'Low', medium: 'Medium', high: 'High' },
    sig: { none: 'Unsigned', valid: 'Valid', invalid: 'Invalid', unknown: 'Unknown' },
    status: { enabled: 'Enabled', disabled: 'Disabled' },
    action: {
      enable: 'Enable',
      disable: 'Disable',
      remove: 'Remove',
      refresh: 'Refresh',
      analyzing: 'Analyzing…',
    },
    empty: 'No startup items',
    error: 'Failed to load',
  },
}

export const i18n = createI18n({
  legacy: false,
  locale: 'zh',
  fallbackLocale: 'en',
  messages,
})
