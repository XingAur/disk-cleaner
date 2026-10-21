import type { Inventory, Item } from "./domain";
const gib = 1024 ** 3;
const samples = [
  [
    "qwen2.5:14b",
    "Ollama",
    "model",
    8.4,
    "review",
    "C:\\Users\\Demo\\.ollama\\models\\manifests\\registry.ollama.ai\\library\\qwen2.5\\14b",
  ],
  [
    "llama3.2:3b",
    "Ollama",
    "model",
    1.9,
    "review",
    "C:\\Users\\Demo\\.ollama\\models\\manifests\\registry.ollama.ai\\library\\llama3.2\\3b",
  ],
  [
    "Cache",
    "Windsurf",
    "cache",
    0.64,
    "safe",
    "C:\\Users\\Demo\\AppData\\Roaming\\Windsurf\\Cache",
  ],
  ["output", "ComfyUI", "output", 3.6, "review", "D:\\ComfyUI\\output"],
  [
    "Cache",
    "Cursor",
    "cache",
    1.8,
    "safe",
    "C:\\Users\\Demo\\AppData\\Roaming\\Cursor\\Cache",
  ],
  [
    "hub",
    "Hugging Face",
    "model",
    5.2,
    "review",
    "C:\\Users\\Demo\\.cache\\huggingface\\hub",
  ],
  [
    "sessions",
    "Codex",
    "chat",
    0.84,
    "review",
    "C:\\Users\\Demo\\.codex\\sessions",
  ],
  [
    "_cacache",
    "Node.js / npm",
    "dependency",
    1.2,
    "safe",
    "C:\\Users\\Demo\\AppData\\Local\\npm-cache\\_cacache",
  ],
  [
    "render-preview",
    "ComfyUI",
    "temp",
    0.46,
    "safe",
    "D:\\ComfyUI\\temp\\render-preview",
  ],
  [
    "projects",
    "Claude Code",
    "chat",
    0.32,
    "review",
    "C:\\Users\\Demo\\.claude\\projects",
  ],
  [
    "design-assets.zip",
    "未知",
    "download",
    0.72,
    "review",
    "C:\\Users\\Demo\\Downloads\\design-assets.zip",
  ],
  [
    "workspaceStorage",
    "Cursor",
    "state",
    0.18,
    "blocked",
    "C:\\Users\\Demo\\AppData\\Roaming\\Cursor\\User\\workspaceStorage",
  ],
  [
    "Download",
    "Windows",
    "system",
    0.63,
    "blocked",
    "C:\\Windows\\SoftwareDistribution\\Download",
  ],
  [
    "debug",
    "Claude Code",
    "cache",
    0.12,
    "safe",
    "C:\\Users\\Demo\\.claude\\debug",
  ],
] as const;
export function demoInventory(): Inventory {
  return {
    scanId: "demo",
    cancelled: false,
    warnings: [],
    recycleBytes: 1.3 * gib,
    drives: [
      { path: "C:\\", totalBytes: 476 * gib, freeBytes: 86 * gib },
      { path: "D:\\", totalBytes: 953 * gib, freeBytes: 412 * gib },
    ],
    items: samples.map(
      ([name, source, category, size, protection, path], i): Item => ({
        id: `demo-${i}`,
        name,
        source,
        category,
        sizeBytes: size * gib,
        protection,
        path,
        evidence:
          source === "未知"
            ? "来源未知 · 仅根据下载目录分类"
            : `已知目录规则 · ${source}（仅目录关联）`,
        impact:
          source === "Ollama"
            ? `卸载模型 ${name}：删除它的清单和只被它使用的数据块，与 ollama rm 一致；与其他模型共用的数据块会保留。`
            : category === "model"
              ? "删除后需要重新下载模型，依赖该模型的任务将无法运行。建议先在应用内按模型卸载。"
              : protection === "safe"
                ? "关闭相关应用后可以重建缓存；首次启动或下载可能变慢。"
                : "可能包含会话、项目状态或生成成果，请先检查内容并备份。",
        protectionReason:
          protection === "blocked"
            ? "包含数据库或系统运行状态，请在应用内管理"
            : protection === "review"
              ? "默认保护，请先核对内容与删除影响"
              : "已知缓存或临时目录",
        fileCount: 24 + i * 47,
        isDir: !name.endsWith(".zip") && source !== "Ollama",
        modifiedMs: Date.now() - i * 86400000,
        permanentAllowed: category === "temp",
        complete: true,
      }),
    ),
  };
}
