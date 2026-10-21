<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import Icon from "./components/Icon.vue";
import Modal from "./components/Modal.vue";
import {
  categories,
  formatBytes,
  matchesSearch,
  mergeQueue,
  selectRange,
  validRegistrationPath,
  type Item,
  type Inventory,
  type Registration,
  type CleanupReport,
  type Progress,
} from "./domain";
import { demoInventory } from "./demo";
import { themes, useTheme } from "./useTheme";
import { NATIVE_GLASS_OPTIONS } from "./coolove-ui/nativeGlass";

const native = "__TAURI_INTERNALS__" in window;
const macOS = navigator.platform.includes("Mac");
const { theme, glass, glassActive, tint, dispose } = useTheme();
const glassHint = computed(
  () => NATIVE_GLASS_OPTIONS.find((o) => o.value === glass.value)?.hint ?? "",
);
const inventory = ref<Inventory | null>(native ? null : demoInventory());
const registry = ref(new Map<string, Item>());
const phase = ref<"idle" | "scan" | "clean" | "browse">("idle");
const busy = computed(() => phase.value !== "idle");
const progress = ref<Progress>({ current: "", completed: 0, total: 0 });
const notice = ref("");
const error = ref("");
const report = ref<CleanupReport | null>(null);
const view = ref("all");
const category = ref("");
const search = ref("");
const source = ref("");
const minSize = ref(0);
const sort = ref("size");
const page = ref(1);
const pageSize = 40;
const selected = ref(new Set<string>());
const anchor = ref<string | null>(null);
const focused = ref<Item | null>(null);
const queue = ref<Item[]>([]);
const reviewed = ref(new Set<string>());
const browsing = ref<{ parent: Item; items: Item[] }[]>([]);
const modal = ref<
  "settings" | "review" | "confirm" | "preview" | "recycle" | null
>(null);
const reviewItems = ref<Item[]>([]);
const reviewChecks = ref(new Set<string>());
const mode = ref<"recycle" | "permanent">("recycle");
const confirmed = ref(false);
const recycleConfirmation = ref("");
const previewText = ref("");
const settingsTab = ref("roots");
const registration = ref<Registration>({ label: "", path: "", kind: "output" });
function readRegistrations(): Registration[] {
  try {
    const value = JSON.parse(
      localStorage.getItem("disk-cleaner.roots") || "[]",
    );
    return Array.isArray(value)
      ? value
          .filter(
            (v) =>
              typeof v.path === "string" &&
              typeof v.label === "string" &&
              ["output", "unknown", "comfyui"].includes(v.kind),
          )
          .slice(0, 32)
      : [];
  } catch {
    return [];
  }
}
const registrations = ref<Registration[]>(readRegistrations());
watch(
  registrations,
  (value) => {
    try {
      localStorage.setItem("disk-cleaner.roots", JSON.stringify(value));
    } catch {
      error.value = "无法保存目录设置";
    }
  },
  { deep: true },
);
function registerItems(items: Item[]) {
  for (const item of items) registry.value.set(item.id, item);
}
if (inventory.value) registerItems(inventory.value.items);
const allItems = computed(() => inventory.value?.items || []);
const currentItems = computed(() =>
  view.value === "queue"
    ? queue.value
    : browsing.value.length
      ? browsing.value[browsing.value.length - 1].items
      : allItems.value,
);
const sources = computed(() =>
  [...new Set(allItems.value.map((i) => i.source))].sort(),
);
const filtered = computed(() =>
  currentItems.value
    .filter((item) => {
      if (view.value === "safe" && item.protection !== "safe") return false;
      if (view.value === "protected" && item.protection === "safe")
        return false;
      return (
        (!category.value || item.category === category.value) &&
        (!source.value || item.source === source.value) &&
        item.sizeBytes >= minSize.value &&
        matchesSearch(item, search.value)
      );
    })
    .sort((a, b) =>
      sort.value === "name"
        ? a.name.localeCompare(b.name, "zh-CN")
        : sort.value === "date"
          ? b.modifiedMs - a.modifiedMs
          : b.sizeBytes - a.sizeBytes,
    ),
);
const pageCount = computed(() =>
  Math.max(1, Math.ceil(filtered.value.length / pageSize)),
);
const visible = computed(() =>
  filtered.value.slice((page.value - 1) * pageSize, page.value * pageSize),
);
const selectedItems = computed(() =>
  [...selected.value]
    .map((id) => registry.value.get(id))
    .filter((i): i is Item => !!i),
);
const actionable = computed(() =>
  selectedItems.value.filter((i) => i.protection !== "blocked"),
);
const total = computed(() =>
  allItems.value.reduce((sum, i) => sum + i.sizeBytes, 0),
);
const safeBytes = computed(() =>
  allItems.value
    .filter((i) => i.protection === "safe")
    .reduce((sum, i) => sum + i.sizeBytes, 0),
);
const protectedBytes = computed(() => total.value - safeBytes.value);
const queueBytes = computed(() =>
  queue.value.reduce((sum, i) => sum + i.sizeBytes, 0),
);
const segments = computed(() =>
  Object.entries(categories)
    .map(([id, label], index) => ({
      id,
      label,
      index,
      size: allItems.value
        .filter((i) => i.category === id)
        .reduce((s, i) => s + i.sizeBytes, 0),
    }))
    .filter((s) => s.size > 0),
);
const canPermanent = computed(
  () => queue.value.length > 0 && queue.value.every((i) => i.permanentAllowed),
);
const title = computed(() =>
  category.value
    ? categories[category.value]
    : {
        all: "文件总览",
        safe: "可清理候选",
        protected: "受保护资料",
        queue: "清理清单",
        recycle: "回收站",
      }[view.value] || "文件总览",
);
const status = (item: Item) =>
  item.protection === "blocked"
    ? "禁止处理"
    : reviewed.value.has(item.id)
      ? "已审核"
      : item.protection === "review"
        ? "默认保护"
        : "可审核清理";
const countCategory = (id: string) =>
  allItems.value.filter((i) => i.category === id).length;
watch([search, source, minSize, category, view], () => {
  page.value = 1;
});
watch(pageCount, (count) => {
  page.value = Math.min(page.value, count);
});
function navigate(next: string, nextCategory = "") {
  view.value = next;
  category.value = nextCategory;
  source.value = "";
  search.value = "";
  minSize.value = 0;
  browsing.value = [];
  selected.value = new Set();
  focused.value = null;
}
function select(item: Item, event: MouseEvent | KeyboardEvent, toggle = false) {
  selected.value = selectRange(
    filtered.value.map((i) => i.id),
    selected.value,
    anchor.value,
    item.id,
    event.shiftKey,
    toggle || event.ctrlKey || event.metaKey,
  );
  if (!event.shiftKey) anchor.value = item.id;
  focused.value = item;
}
function togglePage() {
  const next = new Set(selected.value);
  const all = visible.value.every((i) => next.has(i.id));
  for (const item of visible.value) {
    if (all) next.delete(item.id);
    else next.add(item.id);
  }
  selected.value = next;
}
async function command(name: string, args?: Record<string, unknown>) {
  if (!native) {
    notice.value = "演示模式：此操作需要桌面程序。";
    return;
  }
  try {
    await invoke(name, args);
  } catch (e) {
    error.value = String(e);
  }
}
async function scan() {
  if (busy.value) return;
  if (!native) {
    inventory.value = demoInventory();
    registry.value.clear();
    registerItems(inventory.value.items);
    queue.value = [];
    reviewed.value.clear();
    navigate("all");
    notice.value = "已重新加载演示数据，没有读取本机文件。";
    return;
  }
  phase.value = "scan";
  error.value = "";
  notice.value = "";
  queue.value = [];
  reviewed.value.clear();
  selected.value.clear();
  browsing.value = [];
  focused.value = null;
  progress.value = { current: "正在查找已知目录…", completed: 0, total: 1 };
  try {
    inventory.value = await invoke<Inventory>("scan_inventory", {
      options: { registrations: registrations.value },
    });
    registry.value.clear();
    registerItems(inventory.value.items);
    notice.value = inventory.value.cancelled
      ? "扫描已停止；结果仅供查看，请重新完成扫描后清理。"
      : `扫描完成，共发现 ${inventory.value.items.length} 项。没有自动选择任何文件。`;
  } catch (e) {
    error.value = String(e);
  } finally {
    phase.value = "idle";
  }
}
function addToQueue(items = actionable.value) {
  const pending = items.filter(
    (i) => i.protection === "review" && !reviewed.value.has(i.id),
  );
  if (pending.length) {
    reviewItems.value = items;
    reviewChecks.value = new Set();
    modal.value = "review";
    return;
  }
  queue.value = mergeQueue(queue.value, items);
  notice.value = `清理清单共 ${queue.value.length} 项，可继续跨页添加。`;
}
function acceptReview() {
  for (const item of reviewItems.value)
    if (reviewChecks.value.has(item.id)) reviewed.value.add(item.id);
  queue.value = mergeQueue(
    queue.value,
    reviewItems.value.filter(
      (i) => i.protection === "safe" || reviewed.value.has(i.id),
    ),
  );
  modal.value = null;
  notice.value = `已审核并加入清单，共 ${queue.value.length} 项。`;
}
function openConfirm() {
  mode.value = "recycle";
  confirmed.value = false;
  modal.value = "confirm";
}
async function cleanup() {
  if (!native || !confirmed.value || busy.value || !queue.value.length) return;
  phase.value = "clean";
  error.value = "";
  try {
    report.value = await invoke<CleanupReport>("cleanup_inventory", {
      request: {
        scanId: inventory.value?.scanId,
        ids: queue.value.map((i) => i.id),
        reviewedIds: [...reviewed.value],
        mode: mode.value,
      },
    });
    modal.value = null;
    phase.value = "idle";
    await scan();
    notice.value = `处理结束：${report.value.completedIds.length} 项成功，${report.value.errors.length} 项失败。`;
  } catch (e) {
    error.value = String(e);
    modal.value = null;
  } finally {
    phase.value = "idle";
  }
}
async function browse(item: Item) {
  if (busy.value) return;
  if (!native) {
    notice.value = "演示模式不读取目录；请在桌面程序中浏览真实文件。";
    return;
  }
  phase.value = "browse";
  error.value = "";
  try {
    const items = await invoke<Item[]>("browse_item", { id: item.id });
    registerItems(items);
    browsing.value.push({ parent: item, items });
    selected.value.clear();
    search.value = "";
    source.value = "";
    category.value = "";
    page.value = 1;
  } catch (e) {
    error.value = String(e);
  } finally {
    phase.value = "idle";
  }
}
async function preview(item: Item) {
  if (!native) {
    previewText.value =
      "这是演示预览。桌面版只读取前 64 KiB 文本，内容保留在本机。";
    modal.value = "preview";
    return;
  }
  phase.value = "browse";
  try {
    previewText.value = await invoke<string>("preview_item", { id: item.id });
    modal.value = "preview";
  } catch (e) {
    error.value = String(e);
  } finally {
    phase.value = "idle";
  }
}
function saveRegistration() {
  const value = {
    ...registration.value,
    path: registration.value.path.trim(),
    label: registration.value.label.trim(),
  };
  if (
    !validRegistrationPath(value.path, macOS) ||
    !value.label ||
    value.label.length > 100 ||
    value.path.split(/[\\/]/).includes("..")
  ) {
    error.value = macOS
      ? "请输入具体的绝对目录和来源名称，例如 /Users/me/ComfyUI。"
      : "请输入具体的 Windows 绝对目录和来源名称，例如 D:\\ComfyUI。";
    return;
  }
  if (registrations.value.length >= 32) {
    error.value = "最多登记 32 个目录";
    return;
  }
  if (
    registrations.value.some((r) =>
      macOS
        ? r.path === value.path
        : r.path.toLowerCase() === value.path.toLowerCase(),
    )
  ) {
    error.value = "此目录已经登记";
    return;
  }
  registrations.value.push(value);
  registration.value = { label: "", path: "", kind: "output" };
  notice.value = "目录已登记，下次扫描生效。";
}
async function emptyRecycle() {
  if (!native || recycleConfirmation.value !== "清空回收站") return;
  phase.value = "clean";
  try {
    await invoke("empty_recycle_bin", {
      confirmation: recycleConfirmation.value,
    });
    modal.value = null;
    phase.value = "idle";
    await scan();
  } catch (e) {
    error.value = String(e);
    modal.value = null;
  } finally {
    phase.value = "idle";
  }
}
const unlisten: UnlistenFn[] = [];
onMounted(async () => {
  if (!native) return;
  for (const event of ["scan-progress", "cleanup-progress"]) {
    try {
      unlisten.push(
        await listen<Progress>(event, (e) => {
          progress.value = e.payload;
        }),
      );
    } catch (e) {
      error.value = `无法接收进度：${e}`;
    }
  }
});
onUnmounted(() => {
  unlisten.forEach((fn) => fn());
  dispose();
});
</script>

<template>
  <div class="app-shell cl-window">
    <header
      class="app-titlebar cl-titlebar"
      @mousedown="
        (e) => {
          if (e.button === 0 && !(e.target as HTMLElement).closest('button'))
            command('start_drag');
        }
      "
      @dblclick="command('maximize_window')"
    >
      <span class="brand-symbol"><img src="/app-icon.png" alt="" /></span
      ><b>磁盘管家</b><span class="titlebar-divider"></span
      ><span class="titlebar-caption">让每一份空间，都清清楚楚</span>
      <div class="cl-titlebar__spacer"></div>
      <span v-if="!native" class="cl-badge cl-badge--warning"
        >演示模式 · 非实测</span
      ><span v-else class="local-label"
        ><span class="status-dot"></span>本机运行</span
      >
      <div class="window-actions">
        <button
          aria-label="隐藏到托盘"
          title="隐藏到托盘"
          @click="command('hide_to_tray')"
        >
          <Icon name="tray" :size="15" />
        </button>
        <button aria-label="最小化" @click="command('minimize_window')">
          <Icon name="minus" :size="14" /></button
        ><button aria-label="最大化" @click="command('maximize_window')">
          <Icon name="maximize" :size="13" /></button
        ><button
          aria-label="关闭窗口"
          class="window-close"
          @click="command('close_window')"
        >
          <Icon name="close" :size="16" />
        </button>
      </div>
    </header>
    <div class="cl-desktop">
      <aside class="cl-sidebar">
        <div class="sidebar-brand">
          <span class="brand-symbol brand-symbol--lg"
            ><img src="/app-icon.png" alt=""
          /></span>
          <div>
            <strong>Disk Cleaner</strong><small>AI 与系统空间管理</small>
          </div>
        </div>
        <nav class="cl-nav" aria-label="主要导航">
          <button
            v-for="nav in [
              { id: 'all', name: '文件总览', icon: 'grid' },
              { id: 'safe', name: '可清理候选', icon: 'spark' },
              { id: 'protected', name: '受保护资料', icon: 'shield' },
              { id: 'queue', name: '清理清单', icon: 'list' },
            ]"
            :key="nav.id"
            class="cl-nav__item"
            :aria-current="view === nav.id && !category ? 'page' : undefined"
            @click="navigate(nav.id)"
          >
            <Icon :name="nav.icon" /><span>{{ nav.name }}</span
            ><small v-if="nav.id === 'queue'" class="nav-count">{{
              queue.length
            }}</small>
          </button>
        </nav>
        <div>
          <p class="cl-sidebar__label">内容分类</p>
          <nav class="cl-nav category-nav" aria-label="内容分类">
            <button
              v-for="(label, id) in categories"
              :key="id"
              class="cl-nav__item"
              :aria-current="category === id ? 'page' : undefined"
              @click="navigate('all', id)"
            >
              <span class="category-dot" :class="`category-${id}`"></span
              ><span>{{ label }}</span
              ><small>{{ countCategory(id) }}</small>
            </button>
          </nav>
        </div>
        <div class="sidebar-disks">
          <p class="cl-sidebar__label">本地磁盘</p>
          <div
            v-for="drive in inventory?.drives"
            :key="drive.path"
            class="drive-item"
          >
            <div>
              <Icon name="disk" :size="15" /><b>{{ drive.path }}</b
              ><span>{{ formatBytes(drive.freeBytes) }} 可用</span>
            </div>
            <div class="drive-track">
              <i
                :style="{
                  width: `${(1 - drive.freeBytes / drive.totalBytes) * 100}%`,
                }"
              ></i>
            </div>
          </div>
          <p v-if="!inventory" class="sidebar-note">扫描后查看磁盘容量</p>
        </div>
        <div class="sidebar-bottom">
          <button
            class="cl-nav__item"
            :aria-current="view === 'recycle' ? 'page' : undefined"
            @click="navigate('recycle')"
          >
            <Icon name="trash" /><span>回收站</span
            ><small>{{
              formatBytes(inventory?.recycleBytes || 0)
            }}</small></button
          ><button
            class="cl-nav__item"
            @click="
              settingsTab = 'appearance';
              modal = 'settings';
            "
          >
            <Icon name="settings" /><span>外观与扫描设置</span>
          </button>
          <div class="sidebar-footnote">
            <Icon name="shield" :size="13" />本地扫描 · 默认保护资料
          </div>
        </div>
      </aside>
      <main class="cl-workspace">
        <section class="workspace-heading">
          <div>
            <div class="eyebrow">STORAGE WORKSPACE</div>
            <h1>
              {{ title
              }}<span v-if="view === 'queue'" class="heading-count">{{
                queue.length
              }}</span>
            </h1>
            <p>先看清占用，再决定如何处理。</p>
          </div>
          <div class="heading-actions">
            <button
              class="cl-btn cl-btn--outline"
              :disabled="busy"
              @click="
                settingsTab = 'roots';
                modal = 'settings';
              "
            >
              <Icon name="plus" />登记目录</button
            ><button
              v-if="phase === 'scan'"
              class="cl-btn"
              @click="command('cancel_scan')"
            >
              停止扫描</button
            ><button
              class="cl-btn cl-btn--primary"
              :disabled="busy"
              @click="scan"
            >
              <Icon name="refresh" />{{
                busy ? "正在处理…" : inventory ? "重新扫描" : "开始扫描"
              }}
            </button>
          </div>
        </section>
        <div v-if="!native" class="demo-banner">
          <Icon
            name="info"
            :size="14"
          />当前展示模拟数据，没有扫描或删除本机文件。真实操作请使用桌面版。
        </div>
        <div v-if="busy" class="scan-status" role="status">
          <div>
            <span>{{
              phase === "scan"
                ? "正在扫描"
                : phase === "clean"
                  ? "正在处理"
                  : "正在读取目录"
            }}</span
            ><small>{{ progress.completed }} / {{ progress.total }}</small>
          </div>
          <progress
            :value="phase === 'browse' ? undefined : progress.completed"
            :max="progress.total || 1"
          ></progress>
          <p>{{ progress.current }}</p>
        </div>
        <div v-if="error" class="message message--error" role="alert">
          <span>{{ error }}</span
          ><button aria-label="关闭错误提示" @click="error = ''">
            <Icon name="close" :size="14" />
          </button>
        </div>
        <div v-else-if="notice" class="message" role="status">
          <span>{{ notice }}</span
          ><button aria-label="关闭提示" @click="notice = ''">
            <Icon name="close" :size="14" />
          </button>
        </div>
        <details v-if="report" class="report">
          <summary>
            最近处理：移入回收站 {{ formatBytes(report.recycledBytes) }} ·
            永久删除文件逻辑大小 {{ formatBytes(report.deletedBytes) }} · 失败
            {{ report.errors.length }} 项
          </summary>
          <p>
            移入后可在系统回收站或 Finder
            废纸篓中还原；清空后才释放空间。逻辑大小不等于实际磁盘释放量。
          </p>
          <p v-for="(msg, index) in report.errors" :key="index">{{ msg }}</p>
        </details>
        <template v-if="view === 'recycle'">
          <div class="recycle-panel">
            <span class="recycle-icon"><Icon name="trash" :size="42" /></span>
            <h2>给文件留一条退路</h2>
            <p>
              回收站中约有
              <strong>{{ formatBytes(inventory?.recycleBytes || 0) }}</strong>
              内容。
            </p>
            <p>
              <template v-if="macOS"
                >仅统计当前用户的废纸篓，不含外置卷；请在 Finder
                中还原或清倒。</template
              ><template v-else
                >移入回收站不代表空间已释放。你可以先还原需要的文件，<br />确认不再需要后，再清空全部本地磁盘的回收站。</template
              >
            </p>
            <div class="recycle-actions">
              <button
                class="cl-btn cl-btn--primary"
                @click="command('open_system', { target: 'recycle' })"
              >
                打开{{ macOS ? "Finder 废纸篓" : "Windows 回收站"
                }}<Icon name="external" /></button
              ><button
                v-if="!macOS"
                class="cl-btn cl-btn--danger"
                :disabled="!native || busy"
                @click="
                  recycleConfirmation = '';
                  modal = 'recycle';
                "
              >
                清空回收站…
              </button>
            </div>
          </div>
        </template>
        <template v-else>
          <section
            v-if="view !== 'queue' && !browsing.length"
            class="overview"
            aria-label="空间概览"
          >
            <div class="overview-stats">
              <div>
                <span>已发现内容</span
                ><strong>{{
                  inventory ? formatBytes(total) : "等待扫描"
                }}</strong
                ><small>{{ allItems.length }} 个文件或目录</small>
              </div>
              <div>
                <span><i class="mini-dot dot-safe"></i>可清理候选</span
                ><strong>{{ formatBytes(safeBytes) }}</strong
                ><small>缓存、日志及临时内容</small>
              </div>
              <div>
                <span><i class="mini-dot dot-protected"></i>受保护资料</span
                ><strong>{{ formatBytes(protectedBytes) }}</strong
                ><small>需审核或通过应用管理</small>
              </div>
              <div class="overview-tip">
                <Icon name="shield" :size="24" />
                <p>模型与成果有价值<br /><b>默认保护，由你决定</b></p>
              </div>
            </div>
            <div
              v-if="segments.length"
              class="storage-meter"
              aria-label="按内容分类的容量分布"
            >
              <button
                v-for="segment in segments"
                :key="segment.id"
                :class="`category-${segment.id}`"
                :style="{ flexGrow: segment.size }"
                :title="`${segment.label} ${formatBytes(segment.size)}`"
                :aria-label="`筛选${segment.label}`"
                @click="navigate('all', segment.id)"
              ></button>
            </div>
            <div class="storage-legend">
              <button
                v-for="segment in segments.slice(0, 5)"
                :key="segment.id"
                @click="navigate('all', segment.id)"
              >
                <i class="mini-dot" :class="`category-${segment.id}`"></i
                >{{ segment.label
                }}<span>{{ formatBytes(segment.size) }}</span></button
              ><span class="logical-size">逻辑大小，非可释放空间</span>
            </div>
          </section>
          <div v-if="browsing.length" class="breadcrumbs">
            <button
              @click="
                browsing = [];
                page = 1;
              "
            >
              全部扫描结果</button
            ><template v-for="(level, index) in browsing" :key="level.parent.id"
              ><Icon name="chevron" :size="12" /><button
                @click="
                  browsing = browsing.slice(0, index + 1);
                  page = 1;
                "
              >
                {{ level.parent.name }}
              </button></template
            >
          </div>
          <div class="cl-toolbar filters">
            <label class="search-box"
              ><Icon name="search" :size="16" /><input
                v-model="search"
                aria-label="搜索文件"
                placeholder="搜索名称、路径或来源…"
              /><kbd>＊</kbd></label
            ><select v-model="source" class="cl-select" aria-label="来源筛选">
              <option value="">全部来源</option>
              <option v-for="s in sources" :key="s">{{ s }}</option></select
            ><select v-model="minSize" class="cl-select" aria-label="大小筛选">
              <option :value="0">不限大小</option>
              <option :value="100 * 1024 ** 2">大于 100 MiB</option>
              <option :value="1024 ** 3">大于 1 GiB</option>
              <option :value="10 * 1024 ** 3">大于 10 GiB</option></select
            ><select v-model="sort" class="cl-select" aria-label="排序方式">
              <option value="size">大小 ↓</option>
              <option value="name">名称 A–Z</option>
              <option value="date">最近修改</option>
            </select>
          </div>
          <div class="list-meta">
            <span
              >{{ filtered.length }} 项结果<span v-if="selected.size">
                · 已选 {{ selected.size }} 项</span
              ></span
            ><span>Ctrl 多选 · Shift 连选 · 双击浏览</span>
          </div>
          <div class="cl-split">
            <div class="inventory-pane">
              <div class="table-scroll">
                <table
                  v-if="visible.length"
                  class="cl-table cl-table--files inventory-table"
                  aria-label="文件清单"
                >
                  <colgroup>
                    <col style="width: 38px" />
                    <col />
                    <col style="width: 90px" />
                    <col style="width: 112px" />
                    <col style="width: 105px" />
                  </colgroup>
                  <thead>
                    <tr>
                      <th>
                        <input
                          type="checkbox"
                          :checked="
                            visible.length > 0 &&
                            visible.every((i) => selected.has(i.id))
                          "
                          :indeterminate="
                            visible.some((i) => selected.has(i.id)) &&
                            !visible.every((i) => selected.has(i.id))
                          "
                          aria-label="选择本页"
                          @change="togglePage"
                        />
                      </th>
                      <th>名称 / 路径</th>
                      <th
                        class="is-num"
                        :aria-sort="sort === 'size' ? 'descending' : 'none'"
                      >
                        大小
                      </th>
                      <th>来源</th>
                      <th>{{ view === "queue" ? "清单" : "保护状态" }}</th>
                    </tr>
                  </thead>
                  <tbody>
                    <tr
                      v-for="item in visible"
                      :key="item.id"
                      tabindex="0"
                      :aria-selected="selected.has(item.id)"
                      @click="select(item, $event)"
                      @keydown.space.prevent="select(item, $event, true)"
                      @keydown.enter.prevent="focused = item"
                      @dblclick="item.isDir ? browse(item) : preview(item)"
                    >
                      <td>
                        <input
                          type="checkbox"
                          :checked="selected.has(item.id)"
                          :aria-label="`选择 ${item.name}`"
                          @click.stop="select(item, $event, true)"
                        />
                      </td>
                      <td>
                        <div class="cl-file">
                          <span
                            class="file-symbol"
                            :class="`file-${item.category}`"
                            ><Icon
                              :name="item.isDir ? 'folder' : 'file'"
                              :size="19" /></span
                          ><span class="cl-file__body"
                            ><span class="cl-file__name">{{ item.name }}</span
                            ><span class="cl-file__path" :title="item.path">{{
                              item.path
                            }}</span></span
                          >
                        </div>
                      </td>
                      <td class="is-num item-size">
                        {{
                          item.complete
                            ? formatBytes(item.sizeBytes)
                            : "未完整统计"
                        }}
                      </td>
                      <td>
                        <span class="source-label">{{ item.source }}</span
                        ><small class="cell-subtitle">{{
                          categories[item.category]
                        }}</small>
                      </td>
                      <td>
                        <button
                          v-if="view === 'queue'"
                          class="cl-btn cl-btn--ghost cl-btn--sm"
                          :disabled="busy"
                          @click.stop="
                            queue = queue.filter((i) => i.id !== item.id)
                          "
                        >
                          移出清单</button
                        ><span
                          v-else
                          class="protection-label"
                          :class="`protection-${item.protection}`"
                          ><Icon
                            v-if="item.protection !== 'safe'"
                            name="lock"
                            :size="11"
                          />{{ status(item) }}</span
                        >
                      </td>
                    </tr>
                  </tbody>
                </table>
                <div v-else class="empty-state">
                  <span class="empty-icon"
                    ><Icon :name="inventory ? 'search' : 'disk'" :size="34"
                  /></span>
                  <h2>
                    {{
                      !inventory
                        ? "先看看空间用在了哪里"
                        : view === "queue"
                          ? "清理清单还是空的"
                          : "没有匹配的文件"
                    }}
                  </h2>
                  <p>
                    {{
                      !inventory
                        ? "扫描 AI 工具、系统临时文件和下载目录。"
                        : view === "queue"
                          ? "在列表中选择内容，审核后加入这里统一处理。"
                          : "尝试调整来源、大小或搜索条件。"
                    }}
                  </p>
                  <button
                    v-if="!inventory"
                    class="cl-btn cl-btn--primary"
                    :disabled="busy"
                    @click="scan"
                  >
                    开始扫描</button
                  ><button
                    v-else
                    class="cl-btn cl-btn--outline"
                    @click="navigate('all')"
                  >
                    查看全部内容
                  </button>
                </div>
              </div>
              <div class="pagination">
                <span
                  >{{ filtered.length ? (page - 1) * pageSize + 1 : 0 }}–{{
                    Math.min(page * pageSize, filtered.length)
                  }}
                  / {{ filtered.length }}</span
                >
                <div>
                  <button
                    class="cl-btn cl-btn--ghost cl-btn--sm"
                    :disabled="page <= 1"
                    @click="page--"
                  >
                    上一页</button
                  ><span>{{ page }} / {{ pageCount }}</span
                  ><button
                    class="cl-btn cl-btn--ghost cl-btn--sm"
                    :disabled="page >= pageCount"
                    @click="page++"
                  >
                    下一页
                  </button>
                </div>
              </div>
            </div>
            <aside class="cl-inspector" aria-label="文件详情">
              <template v-if="focused"
                ><div class="inspector-kicker">
                  文件详情 <span>{{ categories[focused.category] }}</span>
                </div>
                <span class="detail-symbol" :class="`file-${focused.category}`"
                  ><Icon :name="focused.isDir ? 'folder' : 'file'" :size="30"
                /></span>
                <h2>{{ focused.name }}</h2>
                <div class="detail-size">
                  {{ formatBytes(focused.sizeBytes)
                  }}<small>{{ focused.fileCount }} 个文件</small>
                </div>
                <p class="detail-path">{{ focused.path }}</p>
                <div class="detail-section">
                  <h3><Icon name="info" :size="14" />来源证据</h3>
                  <p>{{ focused.evidence }}</p>
                </div>
                <div class="detail-section">
                  <h3><Icon name="shield" :size="14" />处理建议与影响</h3>
                  <p>{{ focused.impact }}</p>
                  <div
                    class="protection-box"
                    :class="`protection-${focused.protection}`"
                  >
                    <Icon name="lock" :size="14" /><span>{{
                      focused.protectionReason
                    }}</span>
                  </div>
                </div>
                <div class="detail-section detail-meta">
                  <span>最近修改</span
                  ><b>{{
                    focused.modifiedMs
                      ? new Date(focused.modifiedMs).toLocaleString("zh-CN", {
                          hour12: false,
                        })
                      : "未知"
                  }}</b>
                </div>
                <div class="detail-actions">
                  <button
                    v-if="focused.isDir"
                    class="cl-btn cl-btn--outline"
                    :disabled="busy"
                    @click="browse(focused)"
                  >
                    <Icon name="folder" />浏览目录</button
                  ><button
                    v-else
                    class="cl-btn cl-btn--outline"
                    :disabled="busy || focused.protection === 'blocked'"
                    @click="preview(focused)"
                  >
                    内容预览</button
                  ><button
                    class="cl-btn cl-btn--ghost"
                    :disabled="busy"
                    @click="command('reveal_item', { id: focused.id })"
                  >
                    打开位置<Icon name="external" :size="14" /></button
                  ><button
                    v-if="focused.category === 'system'"
                    class="cl-btn cl-btn--outline"
                    @click="command('open_system', { target: 'storage' })"
                  >
                    Windows 存储设置</button
                  ><button
                    class="cl-btn cl-btn--soft"
                    :disabled="busy || focused.protection === 'blocked'"
                    @click="addToQueue([focused])"
                  >
                    {{
                      focused.protection === "review" &&
                      !reviewed.has(focused.id)
                        ? "审核保护并加入"
                        : "加入清理清单"
                    }}<Icon name="plus" :size="14" />
                  </button></div
              ></template>
              <div v-else class="inspector-empty">
                <Icon name="shield" :size="36" />
                <h3>了解文件，再做决定</h3>
                <p>选择一项内容，查看来源证据、删除影响和保护原因。</p>
                <span>聊天、项目、模型和成果<br />默认保护</span>
              </div>
            </aside>
          </div>
          <div class="selection-bar" v-if="selected.size && view !== 'queue'">
            <span
              >已选 <b>{{ selected.size }}</b> 项 ·
              {{
                formatBytes(selectedItems.reduce((s, i) => s + i.sizeBytes, 0))
              }}<small v-if="actionable.length < selected.size">
                · {{ selected.size - actionable.length }} 项禁止处理</small
              ></span
            ><button
              class="cl-btn cl-btn--ghost cl-btn--sm"
              @click="selected.clear()"
            >
              取消选择</button
            ><button
              class="cl-btn cl-btn--soft cl-btn--sm"
              :disabled="busy || !actionable.length"
              @click="addToQueue()"
            >
              加入清理清单<Icon name="plus" />
            </button>
          </div>
          <div class="cl-queuebar">
            <span class="queue-icon"
              ><Icon name="list" :size="20" /><i v-if="queue.length"></i
            ></span>
            <div class="cl-queuebar__summary">
              <strong
                >待处理 {{ queue.length }} 项<span>{{
                  formatBytes(queueBytes)
                }}</span></strong
              ><small>默认移入回收站，清空后才释放空间</small>
            </div>
            <button
              class="cl-btn cl-btn--ghost"
              :disabled="!queue.length || busy"
              @click="navigate('queue')"
            >
              查看清单</button
            ><button
              class="cl-btn cl-btn--primary"
              :disabled="!queue.length || busy"
              @click="openConfirm"
            >
              预览并处理<Icon name="arrow" />
            </button>
          </div>
        </template>
        <details v-if="inventory?.warnings.length" class="scan-warnings">
          <summary>
            {{ inventory.warnings.length }} 条扫描提示（部分目录未能读取）
          </summary>
          <p v-for="(warning, i) in inventory.warnings" :key="i">
            {{ warning }}
          </p>
        </details>
      </main>
    </div>
    <footer class="app-status">
      <span
        ><span class="status-dot"></span
        >{{
          native ? "本地处理，不上传文件内容" : "演示数据 · 所有删除操作已禁用"
        }}</span
      ><span>Coolove UI 0.7 · Disk Cleaner</span>
    </footer>

    <Modal
      v-if="modal === 'settings'"
      title="外观与扫描设置"
      wide
      @close="modal = null"
    >
      <p v-if="error" class="cl-alert cl-alert--danger" role="alert">
        {{ error }}
      </p>
      <div class="settings-tabs">
        <button
          class="cl-btn"
          :class="settingsTab === 'roots' ? 'cl-btn--soft' : 'cl-btn--ghost'"
          @click="settingsTab = 'roots'"
        >
          扫描目录</button
        ><button
          class="cl-btn"
          :class="
            settingsTab === 'appearance' ? 'cl-btn--soft' : 'cl-btn--ghost'
          "
          @click="settingsTab = 'appearance'"
        >
          外观主题
        </button>
      </div>
      <template v-if="settingsTab === 'roots'"
        ><p class="settings-copy">
          自动识别 Codex、Claude Code、Cursor、Windsurf、VS Code、Ollama、ComfyUI、NVIDIA 缓存、Hugging Face、ModelScope
          等工具的常见目录，并纳入系统临时文件、下载和应用缓存。自定义安装路径可在下方登记。
        </p>
        <div class="cl-alert">
          人工登记不证明文件作者。成果与未知内容默认保护；ComfyUI 根目录会按
          output、models、temp 分开盘点。
        </div>
        <div class="registration-list">
          <div v-for="(root, i) in registrations" :key="root.path">
            <span
              ><b>{{ root.label }}</b
              ><small>{{ root.path }}</small></span
            ><button
              class="cl-btn cl-btn--ghost cl-btn--sm"
              :disabled="busy"
              @click="registrations.splice(i, 1)"
            >
              移除
            </button>
          </div>
        </div>
        <form class="registration-form" @submit.prevent="saveRegistration">
          <label
            >来源名称<input
              v-model="registration.label"
              class="cl-input"
              placeholder="例如：我的 API 绘图脚本"
              maxlength="100"
              required /></label
          ><label
            >目录类型<select v-model="registration.kind" class="cl-select">
              <option value="output">API / 生成成果</option>
              <option value="comfyui">ComfyUI 安装根目录</option>
              <option value="unknown">普通目录（来源未知）</option>
            </select></label
          ><label class="full-width"
            >本地绝对路径<input
              v-model="registration.path"
              class="cl-input"
              :placeholder="macOS ? '/Users/me/AI/outputs' : 'D:\\AI\\outputs'"
              required /></label
          ><button
            class="cl-btn cl-btn--primary"
            :disabled="busy"
            type="submit"
          >
            <Icon name="plus" />登记目录
          </button>
        </form>
        <p class="settings-copy">
          设置保存在本机，下次扫描生效。只扫描列出的规则目录，不进行整盘作者推断。每个目录最多统计
          20 万项，扫描不完整的内容不能清理。
        </p></template
      >
      <template v-else
        ><p class="settings-copy">
          选择适合你的工作空间。切换外观会保留当前清单。
        </p>
        <div class="theme-grid">
          <button
            v-for="t in themes"
            :key="t.id"
            :style="{ '--preview-bg': t.bg, '--preview-accent': t.accent }"
            :aria-pressed="theme === t.id"
            @click="theme = t.id"
          >
            <span class="theme-preview"><i></i><i></i><i></i></span
            ><span
              >{{ t.name }}<Icon v-if="theme === t.id" name="check" :size="15"
            /></span>
          </button>
        </div>
        <label class="system-theme"
          ><input
            type="checkbox"
            :checked="theme === 'system'"
            @change="theme = theme === 'system' ? 'light' : 'system'"
          />跟随系统外观</label
        >
        <div class="native-glass">
          <label
            >系统毛玻璃（实验）<select
              v-model="glass"
              class="cl-select"
              :disabled="!native"
            >
              <option
                v-for="o in NATIVE_GLASS_OPTIONS"
                :key="o.value"
                :value="o.value"
              >
                {{ o.label }}
              </option>
            </select></label
          >
          <label v-if="glassActive"
            >色调浓度 {{ Math.round(tint * 100) }}%<input
              v-model.number="tint"
              type="range"
              min="0.4"
              max="0.95"
              step="0.01"
          /></label>
          <small>{{
            !native
              ? "仅桌面版可用：由系统模糊窗口后面的桌面。"
              : glass !== "off" && !glassActive
                ? "当前系统不支持所选效果，已保持普通外观。"
                : glassHint
          }}</small>
        </div></template
      >
      <template #footer
        ><button class="cl-btn cl-btn--primary" @click="modal = null">
          完成
        </button></template
      >
    </Modal>
    <Modal
      v-if="modal === 'review'"
      title="审核受保护资料"
      wide
      @close="modal = null"
      ><p class="settings-copy">
        逐项阅读删除影响。只将你明确审核的内容加入清理清单。
      </p>
      <div
        v-for="item in reviewItems.filter(
          (i) => i.protection === 'review' && !reviewed.has(i.id),
        )"
        :key="item.id"
        class="review-item"
      >
        <label
          ><input
            type="checkbox"
            :checked="reviewChecks.has(item.id)"
            @change="
              reviewChecks.has(item.id)
                ? reviewChecks.delete(item.id)
                : reviewChecks.add(item.id)
            "
          /><b>{{ item.name }}</b
          ><span>{{ formatBytes(item.sizeBytes) }}</span></label
        >
        <p>{{ item.path }}</p>
        <p>{{ item.impact }}</p>
      </div>
      <template #footer
        ><button class="cl-btn" @click="modal = null">取消</button
        ><button
          class="cl-btn cl-btn--primary"
          :disabled="!reviewChecks.size"
          @click="acceptReview"
        >
          审核并加入 {{ reviewChecks.size }} 项
        </button></template
      ></Modal
    >
    <Modal
      v-if="modal === 'confirm'"
      title="核对清理清单"
      wide
      :busy="busy"
      @close="modal = null"
      ><div class="confirm-summary">
        <Icon name="trash" :size="25" />
        <div>
          <strong>{{ queue.length }} 项 · {{ formatBytes(queueBytes) }}</strong>
          <p>
            {{
              mode === "recycle"
                ? "先进入系统回收站 / 废纸篓，仍占用磁盘空间，可以还原。"
                : "永久删除无法从回收站还原，仅允许已知临时内容。"
            }}
          </p>
        </div>
      </div>
      <div class="confirm-list">
        <div v-for="item in queue" :key="item.id">
          <b
            >{{ item.name }}<span>{{ formatBytes(item.sizeBytes) }}</span></b
          ><small>{{ item.path }}</small>
          <p>{{ item.impact }}</p>
        </div>
      </div>
      <label class="mode-field"
        >处理方式<select
          v-model="mode"
          class="cl-select"
          :disabled="busy"
          @change="confirmed = false"
        >
          <option value="recycle">移入系统回收站 / 废纸篓（推荐）</option>
          <option value="permanent" :disabled="!canPermanent">
            永久删除（仅限临时文件）
          </option>
        </select></label
      ><label class="confirm-check"
        ><input
          v-model="confirmed"
          type="checkbox"
          :disabled="busy"
        />我已核对以上路径与影响，并关闭相关应用</label
      >
      <p v-if="!native" class="cl-alert">演示模式仅供预览，不执行删除。</p>
      <template #footer
        ><button class="cl-btn" :disabled="busy" @click="modal = null">
          返回检查</button
        ><button
          class="cl-btn"
          :class="
            mode === 'permanent' ? 'cl-btn--danger-solid' : 'cl-btn--primary'
          "
          :disabled="!confirmed || busy || !native"
          @click="cleanup"
        >
          {{
            busy
              ? "处理中…"
              : mode === "recycle"
                ? "确认移入回收站"
                : "确认永久删除"
          }}
        </button></template
      ></Modal
    >
    <Modal
      v-if="modal === 'preview'"
      title="文本预览 · 仅本地读取"
      wide
      @close="modal = null"
    >
      <pre class="text-preview">{{ previewText }}</pre>
    </Modal>
    <Modal
      v-if="modal === 'recycle'"
      title="清空全部磁盘的回收站"
      :busy="busy"
      @close="modal = null"
      ><p>
        此操作会永久删除回收站中现有的全部内容，包括其他应用移入的文件。无法通过回收站还原。
      </p>
      <label class="recycle-confirm-label"
        >请输入“清空回收站”确认<input
          v-model="recycleConfirmation"
          class="cl-input"
          placeholder="清空回收站" /></label
      ><template #footer
        ><button class="cl-btn" :disabled="busy" @click="modal = null">
          取消</button
        ><button
          class="cl-btn cl-btn--danger-solid"
          :disabled="recycleConfirmation !== '清空回收站' || busy || !native"
          @click="emptyRecycle"
        >
          永久清空
        </button></template
      ></Modal
    >
  </div>
</template>
