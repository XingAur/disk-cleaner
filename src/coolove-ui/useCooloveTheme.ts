/**
 * Vue 3 接入：把 Coolove UI 的主题参数应用到 <html>。
 * 不依赖 coolove-ui.js，逻辑与 CooloveUI.applyTheme 保持一致。
 *
 * 用法（以 stock-widget 的 settings store 为例）：
 *
 *   import '@/styles/coolove-ui.css'
 *   import { applyCooloveTheme } from '@/composables/useCooloveTheme'
 *
 *   function applySettings() {
 *     applyCooloveTheme({
 *       theme: settings.value.theme,
 *       opacity: settings.value.backgroundOpacity,
 *       customBg: settings.value.customBgColor,
 *       customFg: settings.value.customFontColor,
 *       fontFamily: settings.value.fontFamily
 *     })
 *   }
 *
 * 或在组件里：const { theme, update } = useCooloveTheme({ opacity: 0.85 })
 */
import { reactive, watch } from 'vue'

export type CooloveThemeMode = 'dark' | 'light' | 'custom'

export interface CooloveTheme {
  theme: CooloveThemeMode
  /** 窗口背景不透明度 0~1 */
  opacity: number
  /** 背景模糊 px */
  blur: number
  accent: string
  customBg: string
  customFg: string
  /** 圆角缩放系数，1 为原版 */
  radius: number
  density: 'comfortable' | 'compact'
  /** cn = 红涨绿跌，intl = 绿涨红跌 */
  trend: 'cn' | 'intl'
  /** solid = 浮层不透明（原版），glass = 浮层也半透明 */
  overlay: 'solid' | 'glass'
  /** css = 窗口自己做 CSS 模糊，native = 系统毛玻璃（配合 tauri/nativeGlass.ts） */
  glass: 'css' | 'native'
  fontFamily?: string
}

export const COOLOVE_DEFAULTS: CooloveTheme = {
  theme: 'dark',
  opacity: 0.85,
  blur: 26,
  accent: '#1d9bf0',
  customBg: '#1f2937',
  customFg: '#ffffff',
  radius: 1,
  density: 'comfortable',
  trend: 'cn',
  overlay: 'solid',
  glass: 'css'
}

function hexToTriplet(hex: string): string | null {
  let h = hex.replace('#', '')
  if (h.length === 3) h = h.replace(/./g, '$&$&')
  if (!/^[0-9a-fA-F]{6}$/.test(h)) return null
  return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16)).join(' ')
}

const clamp = (n: number, min: number, max: number) => Math.min(max, Math.max(min, n))

export function applyCooloveTheme(partial: Partial<CooloveTheme>, el: HTMLElement = document.documentElement): CooloveTheme {
  const t: CooloveTheme = { ...COOLOVE_DEFAULTS, ...partial }
  el.setAttribute('data-theme', t.theme)
  el.setAttribute('data-density', t.density)
  el.setAttribute('data-trend', t.trend)
  el.setAttribute('data-overlay', t.overlay)
  if (t.glass === 'native') el.setAttribute('data-glass', 'native')
  else el.removeAttribute('data-glass')

  const s = el.style
  s.setProperty('--bg-opacity', String(clamp(t.opacity, 0, 1)))
  s.setProperty('--glass-blur', `${clamp(t.blur, 0, 80)}px`)
  s.setProperty('--accent', t.accent)
  s.setProperty('--r', String(t.radius))
  if (t.fontFamily) s.setProperty('--font-family', t.fontFamily)
  else s.removeProperty('--font-family')

  if (t.theme === 'custom') {
    s.setProperty('--custom-rgb', hexToTriplet(t.customBg) ?? '31 41 55')
    s.setProperty('--custom-fg', t.customFg)
  } else {
    s.removeProperty('--custom-rgb')
    s.removeProperty('--custom-fg')
  }
  return t
}

/** 响应式版本：修改返回的 theme 对象即自动应用 */
export function useCooloveTheme(initial: Partial<CooloveTheme> = {}, el?: HTMLElement) {
  const theme = reactive<CooloveTheme>({ ...COOLOVE_DEFAULTS, ...initial })
  watch(theme, (v) => applyCooloveTheme(v, el), { deep: true, immediate: true })
  return {
    theme,
    update: (patch: Partial<CooloveTheme>) => Object.assign(theme, patch),
    reset: () => Object.assign(theme, COOLOVE_DEFAULTS)
  }
}
