import { ref, watch } from "vue";
import { applyCooloveTheme } from "./coolove-ui/useCooloveTheme";
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
function savedTheme() {
  try {
    return localStorage.getItem("disk-cleaner.theme") || "system";
  } catch {
    return "system";
  }
}
export function useTheme() {
  const theme = ref(savedTheme());
  const media = matchMedia("(prefers-color-scheme: dark)");
  const apply = () => {
    const choice =
      themes.find((t) => t.id === theme.value) || themes[media.matches ? 1 : 0];
    applyCooloveTheme({
      theme: choice.mode,
      opacity: 1,
      accent: choice.accent,
    });
    document.documentElement.style.setProperty(
      "--base-rgb",
      [1, 3, 5].map((i) => parseInt(choice.bg.slice(i, i + 2), 16)).join(" "),
    );
    document.documentElement.style.setProperty("--solid-bg", choice.bg);
    try {
      localStorage.setItem("disk-cleaner.theme", theme.value);
    } catch {
      /* private browsing */
    }
  };
  watch(theme, apply, { immediate: true });
  media.addEventListener("change", apply);
  return { theme, dispose: () => media.removeEventListener("change", apply) };
}
