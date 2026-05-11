<template>
  <div class="app-shell">
    <header class="title-bar" @mousedown="handleDrag">
      <div class="title-left">
        <span class="app-title">C盘清理助手</span>
      </div>
      <div class="window-actions">
        <button class="window-btn" type="button" title="设置" @click.stop="settingsOpen = true">
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 15.2a3.2 3.2 0 1 0 0-6.4 3.2 3.2 0 0 0 0 6.4Z" />
            <path d="M19.4 15a1.6 1.6 0 0 0 .3 1.8l.1.1a2 2 0 0 1-2.8 2.8l-.1-.1a1.6 1.6 0 0 0-1.8-.3 1.6 1.6 0 0 0-1 1.5V21a2 2 0 0 1-4 0v-.2a1.6 1.6 0 0 0-1-1.5 1.6 1.6 0 0 0-1.8.3l-.1.1a2 2 0 0 1-2.8-2.8l.1-.1a1.6 1.6 0 0 0 .3-1.8 1.6 1.6 0 0 0-1.5-1H3a2 2 0 0 1 0-4h.2a1.6 1.6 0 0 0 1.5-1 1.6 1.6 0 0 0-.3-1.8l-.1-.1a2 2 0 0 1 2.8-2.8l.1.1a1.6 1.6 0 0 0 1.8.3h.1a1.6 1.6 0 0 0 1-1.5V3a2 2 0 0 1 4 0v.2a1.6 1.6 0 0 0 1 1.5h.1a1.6 1.6 0 0 0 1.8-.3l.1-.1a2 2 0 0 1 2.8 2.8l-.1.1a1.6 1.6 0 0 0-.3 1.8v.1a1.6 1.6 0 0 0 1.5 1h.2a2 2 0 0 1 0 4h-.2a1.6 1.6 0 0 0-1.5 1Z" />
          </svg>
        </button>
        <button class="window-btn" type="button" title="最小化" @click.stop="minimizeWindow">
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round">
            <line x1="5" y1="12" x2="19" y2="12" />
          </svg>
        </button>
        <button class="window-btn close-window" type="button" title="关闭" @click.stop="closeWindow">
          <svg viewBox="0 0 24 24" width="13" height="13" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round">
            <line x1="18" y1="6" x2="6" y2="18" />
            <line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>
    </header>

    <main class="workspace">
      <section class="top-panel">
        <div class="brand-mark">
          <img src="/icon.png" alt="" />
        </div>
        <div class="hero-copy">
          <h1>C盘清理助手</h1>
          <p>{{ status }}</p>
        </div>
      </section>

      <section class="total-panel">
        <div class="total-head">
          <span>C 盘概览</span>
          <em>{{ systemDriveLabel }}</em>
        </div>
        <div class="disk-stats">
          <span class="disk-stat">
            <small>总容量</small>
            <strong>{{ formatBytesOrDash(driveSpace?.totalBytes) }}</strong>
          </span>
          <span class="disk-stat">
            <small>当前剩余</small>
            <strong>{{ formatBytesOrDash(driveSpace?.freeBytes) }}</strong>
          </span>
          <span class="disk-stat">
            <small>可清理</small>
            <strong>{{ formatBytes(totalDiscoverableBytes) }}</strong>
          </span>
        </div>
        <small>{{ progressText }}</small>
        <div class="progress-track" :class="{ active: busy }">
          <div class="progress-fill" :style="{ width: `${activeProgress}%` }" />
        </div>
      </section>

      <section class="group-stack" aria-label="清理分组">
        <button
          v-for="group in groups"
          :key="group.id"
          class="group-card"
          :class="[group.tone, `level-${group.level}`, { selected: selectedGroupIds.has(group.id), disabled: !group.selectable || group.count === 0 }]"
          type="button"
          :disabled="busy || !group.selectable || group.count === 0"
          @click="toggleGroup(group.id)"
        >
          <span class="group-dot" />
          <span class="group-main">
            <span class="group-title-line">
              <b>{{ group.title }}</b>
              <i class="level-badge">{{ levelLabel(group.level) }}</i>
            </span>
            <small>{{ group.description }}</small>
          </span>
          <span class="group-stat">
            <strong>{{ formatBytes(group.bytes) }}</strong>
            <small>{{ group.count }} 项</small>
          </span>
        </button>
      </section>

      <section v-if="report" class="report-panel">
        <span>最近清理</span>
        <strong>{{ formatBytes(report.freedBytes) }}</strong>
        <small>{{ reportSummary }}</small>
      </section>

      <footer class="action-bar">
        <button class="primary-btn" type="button" :disabled="busy" @click="scan()">
          {{ phase === 'scan' ? `扫描 ${scanProgress}%` : '扫描' }}
        </button>
        <button class="danger-btn" type="button" :disabled="busy || selectedItems.length === 0" @click="cleanup">
          {{ phase === 'clean' ? `清理 ${cleanupProgress}%` : '清理' }}
        </button>
      </footer>
    </main>

    <div v-if="settingsOpen" class="modal-backdrop" @mousedown.self="settingsOpen = false">
      <section class="settings-modal" role="dialog" aria-modal="true" aria-labelledby="settings-title">
        <div class="modal-header">
          <div>
            <h2 id="settings-title">设置</h2>
            <p>主题偏好会保存到本机。</p>
          </div>
          <button class="icon-close" type="button" title="关闭" @click="settingsOpen = false">
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round">
              <line x1="18" y1="6" x2="6" y2="18" />
              <line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>

        <div class="setting-block">
          <label>外观</label>
          <div class="segmented">
            <button
              v-for="option in themeOptions"
              :key="option.value"
              type="button"
              :class="{ active: themeMode === option.value }"
              @click="setTheme(option.value)"
            >
              {{ option.label }}
            </button>
          </div>
        </div>
      </section>
    </div>

    <div v-if="confirmDialog" class="modal-backdrop" @mousedown.self="resolveConfirm(false)">
      <section class="confirm-modal" role="dialog" aria-modal="true" aria-labelledby="confirm-title">
        <div class="confirm-mark">
          <svg viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 9v4" />
            <path d="M12 17h.01" />
            <path d="M10.3 4.3 2.6 18a2 2 0 0 0 1.7 3h15.4a2 2 0 0 0 1.7-3L13.7 4.3a2 2 0 0 0-3.4 0Z" />
          </svg>
        </div>
        <div class="confirm-copy">
          <h2 id="confirm-title">{{ confirmDialog.title }}</h2>
          <p>{{ confirmDialog.message }}</p>
          <small>{{ confirmDialog.detail }}</small>
        </div>
        <div class="confirm-actions">
          <button class="modal-secondary" type="button" @click="resolveConfirm(false)">取消</button>
          <button class="modal-danger" type="button" @click="resolveConfirm(true)">确认清理</button>
        </div>
      </section>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

type RiskLevel = 'low' | 'medium' | 'high'
type ThemeMode = 'system' | 'dark' | 'light'
type CleanupLevel = 'light' | 'standard' | 'deep'
type GroupId =
  | 'systemTemp'
  | 'appCache'
  | 'windowsOld'
  | 'recycleBin'
  | 'systemCache'
  | 'largeFiles'
  | 'oldDownloads'
  | 'systemBackups'

interface CleanupItem {
  id: string
  path: string
  sizeBytes: number
  category: string
  risk: RiskLevel
  defaultSelected: boolean
  reason: string
}

interface CleanupPlan {
  items: CleanupItem[]
  suggestions: CleanupItem[]
  systemBackups: CleanupItem[]
  skipped: string[]
  reclaimableBytes: number
  suggestedBytes: number
}

interface ScanResult {
  systemDrive: string
  driveSpace?: DriveSpace | null
  plan: CleanupPlan
}

interface DriveSpace {
  totalBytes: number
  freeBytes: number
  availableBytes: number
}

interface CleanupReport {
  freedBytes: number
  deletedCount: number
  skippedCount: number
  lockedCount: number
  permissionFailedCount: number
  failedCount: number
  errors: string[]
  logPath?: string
}

interface ProgressPayload {
  percent: number
  phase: string
  currentPath: string
}

interface SummaryGroup {
  id: GroupId
  title: string
  description: string
  tone: string
  level: CleanupLevel
  items: CleanupItem[]
  bytes: number
  count: number
  selectable: boolean
  defaultSelected: boolean
}

interface ConfirmDialog {
  title: string
  message: string
  detail: string
  resolve: (confirmed: boolean) => void
}

interface ScanRunOptions {
  preserveReport?: boolean
  statusText?: string
  finalStatus?: (result: ScanResult) => string
}

const themeOptions: Array<{ value: ThemeMode; label: string }> = [
  { value: 'system', label: '跟随系统' },
  { value: 'dark', label: '深色' },
  { value: 'light', label: '浅色' }
]

const emptyPlan: CleanupPlan = {
  items: [],
  suggestions: [],
  systemBackups: [],
  skipped: [],
  reclaimableBytes: 0,
  suggestedBytes: 0
}

const plan = ref<CleanupPlan>(emptyPlan)
const systemDriveLabel = ref('C:\\')
const driveSpace = ref<DriveSpace | null>(null)
const selectedGroupList = ref<GroupId[]>([])
const status = ref('扫描后按等级卡片选择清理项')
const busy = ref(false)
const phase = ref<'idle' | 'scan' | 'clean'>('idle')
const report = ref<CleanupReport | null>(null)
const scanProgress = ref(0)
const cleanupProgress = ref(0)
const progressPhase = ref('准备')
const progressCurrent = ref('')
const confirmDialog = ref<ConfirmDialog | null>(null)
const settingsOpen = ref(false)
const themeMode = ref<ThemeMode>('system')
let unlistenScan: UnlistenFn | null = null
let unlistenCleanup: UnlistenFn | null = null
let mediaQuery: MediaQueryList | null = null

const selectedGroupIds = computed(() => new Set(selectedGroupList.value))
const totalDiscoverableBytes = computed(() => plan.value.reclaimableBytes + plan.value.suggestedBytes)
const activeProgress = computed(() => (phase.value === 'clean' ? cleanupProgress.value : scanProgress.value))
const reportSummary = computed(() => {
  if (!report.value) return ''
  const parts = [`成功 ${report.value.deletedCount} 项`]
  if (report.value.skippedCount > 0) parts.push(`跳过 ${report.value.skippedCount} 项`)
  if (report.value.lockedCount > 0) parts.push(`占用 ${report.value.lockedCount} 项`)
  if (report.value.permissionFailedCount > 0) parts.push(`权限失败 ${report.value.permissionFailedCount} 项`)
  if (report.value.failedCount > 0) parts.push(`失败 ${report.value.failedCount} 项`)
  return parts.join('，')
})
const progressText = computed(() => {
  if (phase.value === 'scan') return `${translatePhase(progressPhase.value)} ${scanProgress.value}% · ${progressCurrent.value || '系统盘'}`
  if (phase.value === 'clean') return `${translatePhase(progressPhase.value)} ${cleanupProgress.value}% · ${progressCurrent.value || '准备清理'}`
  if (scanProgress.value === 100) return '轻度默认勾选；标准、深度可自行选择'
  return '轻度、标准、深度已拆成卡片'
})

const groups = computed<SummaryGroup[]>(() => {
  const systemTemp = itemsByCategory(plan.value.items, 'Low risk cache')
  const appCache = itemsByCategory(plan.value.items, 'Enhanced cache')
  const windowsOld = itemsByCategory(plan.value.items, 'Windows.old')
  const recycleBin = itemsByCategory(plan.value.items, 'Recycle Bin')
  const systemCache = itemsByCategory(plan.value.items, 'System update cache')
  const largeFiles = itemsByCategory(plan.value.suggestions, 'Large file suggestion')
  const oldDownloads = itemsByCategories(plan.value.suggestions, ['Old download suggestion', 'Download item suggestion'])
  const backups = plan.value.systemBackups ?? []

  return [
    {
      id: 'systemTemp',
      title: '系统临时',
      description: 'Temp、崩溃转储、缩略图缓存',
      tone: 'tone-clean',
      level: 'light',
      items: systemTemp,
      bytes: sumBytes(systemTemp),
      count: systemTemp.length,
      selectable: true,
      defaultSelected: true
    },
    {
      id: 'appCache',
      title: '应用缓存',
      description: '浏览器、开发工具、常见软件缓存',
      tone: 'tone-confirm',
      level: 'standard',
      items: appCache,
      bytes: sumBytes(appCache),
      count: appCache.length,
      selectable: true,
      defaultSelected: false
    },
    {
      id: 'windowsOld',
      title: 'Windows.old',
      description: '旧系统目录，确认无需回退后清理',
      tone: 'tone-confirm',
      level: 'standard',
      items: windowsOld,
      bytes: sumBytes(windowsOld),
      count: windowsOld.length,
      selectable: true,
      defaultSelected: false
    },
    {
      id: 'recycleBin',
      title: '回收站',
      description: '清空所有磁盘回收站，确认无须恢复后清理',
      tone: 'tone-confirm',
      level: 'standard',
      items: recycleBin,
      bytes: sumBytes(recycleBin),
      count: recycleBin.length,
      selectable: true,
      defaultSelected: false
    },
    {
      id: 'systemCache',
      title: '系统缓存',
      description: 'Windows 更新、错误报告、着色器缓存',
      tone: 'tone-confirm',
      level: 'standard',
      items: systemCache,
      bytes: sumBytes(systemCache),
      count: systemCache.length,
      selectable: true,
      defaultSelected: false
    },
    {
      id: 'largeFiles',
      title: '大文件',
      description: '200 MB 以上候选，需要确认',
      tone: 'tone-caution',
      level: 'deep',
      items: largeFiles,
      bytes: sumBytes(largeFiles),
      count: largeFiles.length,
      selectable: true,
      defaultSelected: false
    },
    {
      id: 'oldDownloads',
      title: '下载内容',
      description: '下载目录顶层文件和文件夹，确认后清理',
      tone: 'tone-caution',
      level: 'deep',
      items: oldDownloads,
      bytes: sumBytes(oldDownloads),
      count: oldDownloads.length,
      selectable: true,
      defaultSelected: false
    },
    {
      id: 'systemBackups',
      title: '系统备份点',
      description: backups.length > 0 ? '还原点/卷影副本，单独列出不自动删除' : '未发现可枚举的系统还原点',
      tone: 'tone-backup',
      level: 'deep',
      items: backups,
      bytes: 0,
      count: backups.length,
      selectable: false,
      defaultSelected: false
    }
  ]
})

const selectedItems = computed(() =>
  groups.value
    .filter(group => group.selectable && selectedGroupIds.value.has(group.id))
    .flatMap(group => group.items)
)

onMounted(async () => {
  themeMode.value = loadTheme()
  mediaQuery = window.matchMedia('(prefers-color-scheme: dark)')
  mediaQuery.addEventListener('change', handleSystemThemeChange)
  applyTheme()

  unlistenScan = await listen<ProgressPayload>('scan-progress', event => {
    scanProgress.value = Math.max(scanProgress.value, clampPercent(event.payload.percent))
    progressPhase.value = event.payload.phase
    progressCurrent.value = compactPath(event.payload.currentPath)
  })
  unlistenCleanup = await listen<ProgressPayload>('cleanup-progress', event => {
    cleanupProgress.value = Math.max(cleanupProgress.value, clampPercent(event.payload.percent))
    progressPhase.value = event.payload.phase
    progressCurrent.value = compactPath(event.payload.currentPath)
  })
})

onUnmounted(() => {
  unlistenScan?.()
  unlistenCleanup?.()
  mediaQuery?.removeEventListener('change', handleSystemThemeChange)
})

async function handleDrag(event: MouseEvent) {
  if ((event.target as HTMLElement).closest('button')) return
  await invoke('start_drag')
}

async function closeWindow() {
  await invoke('close_window')
}

async function minimizeWindow() {
  await invoke('minimize_window')
}

async function scan(options: ScanRunOptions = {}) {
  busy.value = true
  phase.value = 'scan'
  status.value = options.statusText ?? '正在扫描系统盘...'
  if (!options.preserveReport) report.value = null
  plan.value = emptyPlan
  selectedGroupList.value = []
  scanProgress.value = 0
  progressPhase.value = 'Preparing scan'
  progressCurrent.value = ''

  try {
    const result = await invoke<ScanResult>('scan_system_drive', {
      options: { strength: 'deep' }
    })
    systemDriveLabel.value = result.systemDrive
    driveSpace.value = result.driveSpace ?? null
    plan.value = normalizePlan(result.plan)
    selectedGroupList.value = groups.value
      .filter(group => group.defaultSelected && group.count > 0)
      .map(group => group.id)
    scanProgress.value = 100
    status.value = options.finalStatus?.(result) ?? `扫描完成：${result.systemDrive}`
  } catch (error) {
    status.value = `扫描失败：${String(error)}`
  } finally {
    busy.value = false
    phase.value = 'idle'
  }
}

async function cleanup() {
  const selectedNames = groups.value
    .filter(group => selectedGroupIds.value.has(group.id))
    .map(group => group.title)
    .join('、')
  const confirmed = await requestConfirm({
    title: '确认永久清理',
    message: `将永久删除「${selectedNames}」中的 ${selectedItems.value.length} 个项目。`,
    detail: '此操作不进入回收站；被占用或失败的文件会跳过并写入日志。系统备份点不会在这里删除。'
  })
  if (!confirmed) return

  busy.value = true
  phase.value = 'clean'
  status.value = '正在清理...'
  cleanupProgress.value = 0
  progressPhase.value = 'Preparing cleanup'
  progressCurrent.value = ''

  try {
    const result = await invoke<CleanupReport>('cleanup_selected', {
      itemIds: selectedItems.value.map(item => item.id)
    })
    report.value = result
    cleanupProgress.value = 100
    selectedGroupList.value = []
    await scan({
      preserveReport: true,
      statusText: '清理完成，正在重新扫描...',
      finalStatus: scanResult => cleanupFinishedStatus(result, scanResult)
    })
  } catch (error) {
    status.value = `清理失败：${String(error)}`
  } finally {
    busy.value = false
    phase.value = 'idle'
  }
}

function requestConfirm(options: Omit<ConfirmDialog, 'resolve'>) {
  return new Promise<boolean>(resolve => {
    confirmDialog.value = { ...options, resolve }
  })
}

function resolveConfirm(confirmed: boolean) {
  confirmDialog.value?.resolve(confirmed)
  confirmDialog.value = null
}

function toggleGroup(id: GroupId) {
  if (selectedGroupIds.value.has(id)) {
    selectedGroupList.value = selectedGroupList.value.filter(groupId => groupId !== id)
    return
  }
  selectedGroupList.value = [...selectedGroupList.value, id]
}

function setTheme(value: ThemeMode) {
  themeMode.value = value
  localStorage.setItem('disk-cleaner.theme', value)
  applyTheme()
}

function applyTheme() {
  const effective = themeMode.value === 'system' ? (mediaQuery?.matches ? 'dark' : 'light') : themeMode.value
  document.documentElement.dataset.theme = effective
}

function handleSystemThemeChange() {
  if (themeMode.value === 'system') applyTheme()
}

function loadTheme(): ThemeMode {
  const value = localStorage.getItem('disk-cleaner.theme')
  return value === 'dark' || value === 'light' || value === 'system' ? value : 'system'
}

function normalizePlan(value: CleanupPlan): CleanupPlan {
  return {
    ...emptyPlan,
    ...value,
    items: value.items ?? [],
    suggestions: value.suggestions ?? [],
    systemBackups: value.systemBackups ?? [],
    skipped: value.skipped ?? []
  }
}

function cleanupFinishedStatus(cleanupReport: CleanupReport, scanResult: ScanResult) {
  const skipped = cleanupReport.skippedCount > 0 ? `，跳过 ${cleanupReport.skippedCount} 项` : ''
  const failed = cleanupReport.failedCount > 0 ? `，失败 ${cleanupReport.failedCount} 项` : ''
  const free = scanResult.driveSpace ? `，当前剩余 ${formatBytes(scanResult.driveSpace.freeBytes)}` : ''
  return `清理完成：释放 ${formatBytes(cleanupReport.freedBytes)}${skipped}${failed}；已重新扫描 ${scanResult.systemDrive}${free}`
}

function itemsByCategory(items: CleanupItem[], category: string) {
  return items.filter(item => item.category === category)
}

function itemsByCategories(items: CleanupItem[], categories: string[]) {
  const categorySet = new Set(categories)
  return items.filter(item => categorySet.has(item.category))
}

function sumBytes(items: CleanupItem[]) {
  return items.reduce((total, item) => total + item.sizeBytes, 0)
}

function compactPath(path: string) {
  if (!path) return ''
  if (path === 'All Recycle Bins') return '所有磁盘回收站'
  const parts = path.split(/[\\/]/).filter(Boolean)
  return parts.length > 2 ? `${parts[0]}\\...\\${parts[parts.length - 1]}` : path
}

function translatePhase(value: string) {
  const map: Record<string, string> = {
    'Preparing scan': '准备扫描',
    Scanning: '扫描中',
    Scanned: '已扫描',
    Complete: '完成',
    'Preparing cleanup': '准备清理',
    'Emptying recycle bin': '清空回收站',
    'All Recycle Bins': '所有磁盘回收站',
    Deleting: '删除中',
    Deleted: '已删除',
    Skipped: '已跳过',
    Failed: '失败'
  }
  return map[value] ?? value
}

function levelLabel(level: CleanupLevel) {
  const map: Record<CleanupLevel, string> = {
    light: '轻度',
    standard: '标准',
    deep: '深度'
  }
  return map[level]
}

function clampPercent(percent: number) {
  return Math.min(100, Math.max(0, Math.round(percent)))
}

function formatBytes(bytes: number) {
  const units = ['B', 'KB', 'MB', 'GB', 'TB']
  let value = bytes
  let index = 0
  while (value >= 1024 && index < units.length - 1) {
    value /= 1024
    index += 1
  }
  return index === 0 ? `${bytes} ${units[index]}` : `${value.toFixed(1)} ${units[index]}`
}

function formatBytesOrDash(bytes?: number | null) {
  return typeof bytes === 'number' ? formatBytes(bytes) : '--'
}
</script>
