/**
 * Tauri 2 接入：系统毛玻璃（Windows Acrylic / Mica / Blur，macOS vibrancy）。
 * 只给 Tauri 应用用（依赖 @tauri-apps/api），复制到项目的 src/coolove-ui/ 即可。
 *
 * 前提：
 *   - tauri.conf.json 的窗口 "transparent": true（macOS 另需 "macOSPrivateApi": true）
 *   - capabilities 里加权限 "core:window:allow-set-effects"（clearEffects 用的也是它）
 *
 * 用法：
 *   const ok = await applyNativeGlass(settings.nativeGlass)          // 'off' | 'auto' | 'acrylic' | 'mica' | 'blur'
 *   applyCooloveTheme({ ...theme, glass: ok ? 'native' : 'css' })  // 成功才切到 native，失败自动退回 CSS 模糊
 *
 * 系统效果按矩形窗口绘制，CSS 圆角剪不掉它；这里会把 --native-radius 写成系统窗口的圆角，
 * coolove-ui 在 data-glass="native" 时用它做窗口圆角。
 */
import { getCurrentWindow, Effect, EffectState } from '@tauri-apps/api/window'

export type NativeGlassMode = 'off' | 'auto' | 'acrylic' | 'mica' | 'blur'
export type NativeGlassPlatform = 'windows11' | 'windows10' | 'macos' | 'other'

export const NATIVE_GLASS_OPTIONS: ReadonlyArray<{ value: NativeGlassMode; label: string; hint: string }> = [
  { value: 'off', label: '关闭', hint: '只用 CSS 半透明，背后的桌面不模糊' },
  { value: 'auto', label: '自动', hint: 'Windows 11 用 Acrylic，Windows 10 用 Blur，macOS 用系统毛玻璃' },
  { value: 'acrylic', label: 'Acrylic', hint: '透出并模糊窗口后面的内容；Windows 10 与 Windows 11 早期版本拖动窗口时可能卡顿' },
  { value: 'mica', label: 'Mica', hint: '仅 Windows 11：取桌面壁纸的颜色，流畅但看不到后面的窗口' },
  { value: 'blur', label: 'Blur', hint: 'Windows 10 上流畅；Windows 11 22H2 起拖动可能卡顿' }
]

/** 判断系统。Windows 11 靠 userAgentData 的 platformVersion（13 起为 Windows 11），取不到时按 Windows 10 处理。 */
export async function detectNativeGlassPlatform(): Promise<NativeGlassPlatform> {
  const nav = navigator as Navigator & {
    userAgentData?: { platform?: string; getHighEntropyValues?: (hints: string[]) => Promise<{ platformVersion?: string }> }
  }
  const ua = navigator.userAgent
  if (/Macintosh|Mac OS X/.test(ua)) return 'macos'
  if (!/Windows/.test(ua)) return 'other'
  try {
    const info = await nav.userAgentData?.getHighEntropyValues?.(['platformVersion'])
    const major = Number(info?.platformVersion?.split('.')[0])
    if (major >= 13) return 'windows11'
  } catch {
    // 拿不到版本时保守地按 Windows 10 处理
  }
  return 'windows10'
}

/** 各平台的系统窗口圆角（px） */
export function nativeRadius(platform: NativeGlassPlatform): number {
  return platform === 'macos' ? 10 : platform === 'windows11' ? 8 : 0
}

export function pickEffect(mode: NativeGlassMode, platform: NativeGlassPlatform): Effect | null {
  if (mode === 'off' || platform === 'other') return null
  if (platform === 'macos') return Effect.UnderWindowBackground
  if (mode === 'mica') return platform === 'windows11' ? Effect.Mica : null
  if (mode === 'acrylic') return Effect.Acrylic
  if (mode === 'blur') return Effect.Blur
  return platform === 'windows11' ? Effect.Acrylic : Effect.Blur
}

/**
 * 打开或关闭系统毛玻璃。返回 true 表示已经打开，可以把主题切到 glass: 'native'；
 * 关闭、平台不支持或调用失败都返回 false（并清掉已有效果），保持 CSS 模糊。
 */
export async function applyNativeGlass(mode: NativeGlassMode, el: HTMLElement = document.documentElement): Promise<boolean> {
  const off = () => el.style.removeProperty('--native-radius')
  let win: ReturnType<typeof getCurrentWindow>
  try {
    win = getCurrentWindow()
  } catch {
    // 不在 Tauri 窗口里（浏览器预览、测试环境）
    off()
    return false
  }
  const platform = await detectNativeGlassPlatform()
  const effect = pickEffect(mode, platform)
  if (!effect) {
    await win.clearEffects().catch(() => {})
    off()
    return false
  }
  const radius = nativeRadius(platform)
  try {
    await win.setEffects({ effects: [effect], state: EffectState.Active, radius })
    el.style.setProperty('--native-radius', `${radius}px`)
    return true
  } catch (error) {
    console.warn('[coolove-ui] 系统毛玻璃不可用，退回 CSS 模糊：', error)
    await win.clearEffects().catch(() => {})
    off()
    return false
  }
}
