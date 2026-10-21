import { ref, watch } from "vue";
import { applyCooloveTheme } from "./coolove-ui/useCooloveTheme";
import {
  applyNativeGlass,
  type NativeGlassMode,
} from "./coolove-ui/nativeGlass";
export const themes = [
  {
    id: "light",
    name: "晴空白",
    mode: "light",
    accent: "#6554c0",
    bg: "#fafafb",
  },
  {
    id: "dark",
    name: "曜石黑",
    mode: "dark",
    accent: "#a896f4",
    bg: "#101217",
  },
  {
    id: "mint",
    name: "薄荷庭院",
    mode: "light",
    accent: "#178473",
    bg: "#f3faf7",
  },
  {
    id: "latte",
    name: "奶油拿铁",
    mode: "light",
    accent: "#9a6b40",
    bg: "#faf6ef",
  },
  {
    id: "rose",
    name: "樱花粉",
    mode: "light",
    accent: "#b35282",
    bg: "#fff6fa",
  },
  {
    id: "blue",
    name: "午夜蓝",
    mode: "dark",
    accent: "#75a8ee",
    bg: "#111c2d",
  },
  {
    id: "violet",
    name: "紫藤夜色",
    mode: "dark",
    accent: "#bc9ceb",
    bg: "#20192b",
  },
  {
    id: "forest",
    name: "深林绿",
    mode: "dark",
    accent: "#73c6a6",
    bg: "#14251f",
  },
] as const;
function saved(key: string, fallback: string) {
  try {
    return localStorage.getItem(key) || fallback;
  } catch {
    return fallback;
  }
}
function save(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* private browsing */
  }
}
const glassModes: NativeGlassMode[] = ["off", "auto", "acrylic", "mica", "blur"];
export function useTheme() {
  const theme = ref(saved("disk-cleaner.theme", "system"));
  const storedGlass = saved("disk-cleaner.glass", "off") as NativeGlassMode;
  /** 系统毛玻璃设置；只有桌面版可用 */
  const glass = ref<NativeGlassMode>(
    glassModes.includes(storedGlass) ? storedGlass : "off",
  );
  /** 系统毛玻璃已经打开（设置了且系统支持） */
  const glassActive = ref(false);
  /** 打开系统毛玻璃时窗口色调的不透明度 */
  const tint = ref(
    Math.min(0.95, Math.max(0.4, Number(saved("disk-cleaner.glass-tint", "0.72")) || 0.72)),
  );
  const media = matchMedia("(prefers-color-scheme: dark)");
  const apply = () => {
    const choice =
      themes.find((t) => t.id === theme.value) || themes[media.matches ? 1 : 0];
    applyCooloveTheme({
      theme: choice.mode,
      opacity: glassActive.value ? tint.value : 1,
      accent: choice.accent,
      glass: glassActive.value ? "native" : "css",
    });
    document.documentElement.style.setProperty(
      "--base-rgb",
      [1, 3, 5].map((i) => parseInt(choice.bg.slice(i, i + 2), 16)).join(" "),
    );
    document.documentElement.style.setProperty("--solid-bg", choice.bg);
    save("disk-cleaner.theme", theme.value);
  };
  const applyGlass = async () => {
    save("disk-cleaner.glass", glass.value);
    glassActive.value =
      "__TAURI_INTERNALS__" in window
        ? await applyNativeGlass(glass.value)
        : false;
    apply();
  };
  watch(theme, apply, { immediate: true });
  watch(glass, applyGlass, { immediate: true });
  watch(tint, () => {
    save("disk-cleaner.glass-tint", String(tint.value));
    apply();
  });
  media.addEventListener("change", apply);
  return {
    theme,
    glass,
    glassActive,
    tint,
    dispose: () => media.removeEventListener("change", apply),
  };
}
