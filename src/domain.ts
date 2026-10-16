export type Protection = "safe" | "review" | "blocked";
export interface Item {
  id: string;
  name: string;
  path: string;
  source: string;
  category: string;
  evidence: string;
  impact: string;
  protection: Protection;
  protectionReason: string;
  sizeBytes: number;
  fileCount: number;
  isDir: boolean;
  modifiedMs: number;
  permanentAllowed: boolean;
  complete: boolean;
}
export interface Registration {
  path: string;
  label: string;
  kind: "output" | "unknown" | "comfyui";
}
export interface Inventory {
  scanId: string;
  items: Item[];
  warnings: string[];
  cancelled: boolean;
  drives: { path: string; totalBytes: number; freeBytes: number }[];
  recycleBytes: number;
}
export interface Progress {
  current: string;
  completed: number;
  total: number;
}
export interface CleanupReport {
  recycledBytes: number;
  deletedBytes: number;
  completedIds: string[];
  errors: string[];
}
export const categories: Record<string, string> = {
  cache: "缓存与日志",
  temp: "临时文件",
  dependency: "环境与依赖",
  model: "模型与权重",
  chat: "聊天与历史",
  output: "生成成果",
  download: "下载内容",
  large: "大文件",
  system: "系统与更新",
  state: "应用状态",
  unknown: "未知内容",
};
export function formatBytes(bytes: number): string {
  if (!Number.isFinite(bytes) || bytes < 0) return "—";
  if (bytes === 0) return "0 B";
  const unit = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), 4);
  return `${(bytes / 1024 ** unit).toFixed(unit ? 1 : 0)} ${["B", "KiB", "MiB", "GiB", "TiB"][unit]}`;
}
export function matchesSearch(item: Item, search: string): boolean {
  const haystack =
    `${item.name} ${item.path} ${item.source} ${categories[item.category]}`.toLocaleLowerCase();
  return search
    .trim()
    .toLocaleLowerCase()
    .split(/\s+/)
    .every((term) => {
      const escaped = term
        .split("*")
        .map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
        .join(".*");
      return new RegExp(escaped, "u").test(haystack);
    });
}
const normalized = (path: string) =>
  path.startsWith("/")
    ? path.replace(/\/+$/, "")
    : path.replace(/\//g, "\\").replace(/\\+$/, "").toLocaleLowerCase();
export function containsPath(parent: string, child: string) {
  const p = normalized(parent);
  const c = normalized(child);
  return p === c || c.startsWith(`${p}${parent.startsWith("/") ? "/" : "\\"}`);
}

export function validRegistrationPath(path: string, macOS: boolean) {
  return (
    (macOS
      ? path.startsWith("/") && path.length > 1
      : /^[A-Za-z]:[\\/]/.test(path) && path.length > 3) &&
    !path.split(/[\\/]/).includes("..") &&
    !path.includes("\0")
  );
}
export function mergeQueue(queue: Item[], incoming: Item[]): Item[] {
  const result = [...queue];
  for (const item of incoming) {
    if (
      item.protection === "blocked" ||
      !item.complete ||
      result.some((entry) => containsPath(entry.path, item.path))
    )
      continue;
    for (let i = result.length - 1; i >= 0; i--)
      if (containsPath(item.path, result[i].path)) result.splice(i, 1);
    result.push(item);
  }
  return result;
}
export function selectRange(
  ids: string[],
  current: Set<string>,
  anchor: string | null,
  target: string,
  extend: boolean,
  toggle: boolean,
): Set<string> {
  if (extend && anchor && ids.includes(anchor)) {
    const [from, to] = [ids.indexOf(anchor), ids.indexOf(target)].sort(
      (a, b) => a - b,
    );
    return new Set([...current, ...ids.slice(from, to + 1)]);
  }
  if (toggle) {
    const next = new Set(current);
    if (next.has(target)) next.delete(target);
    else next.add(target);
    return next;
  }
  return new Set([target]);
}
