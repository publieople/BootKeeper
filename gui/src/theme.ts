// Material Design 3 dynamic theming for Naive UI.
//
// Reads the Windows accent color (DWM AccentColor, ABGR) via a Tauri command,
// then builds a Material 3 tonal palette with material-color-utilities and
// maps it onto Naive UI's theme overrides (seed -> primary palette).

import type { GlobalThemeOverrides } from 'naive-ui'
import { argbFromHex, hexFromArgb, themeFromSourceColor, TonalPalette } from '@material/material-color-utilities'

const SEED_FALLBACK = 0xff18a058 // BootKeeper green

export interface Md3Theme {
  seed: number
  primary: string
  primaryHover: string
  primaryPressed: string
  info: string
  success: string
  warning: string
  error: string
  bodyBg: string
  cardBg: string
  border: string
  text: string
  textSub: string
}

function argbToCss(a: number): string {
  return hexFromArgb(a)
}

/** Build Naive UI theme overrides from a Material 3 source color. */
export function themeFromSeed(seed: number): Md3Theme {
  const scheme = themeFromSourceColor(seed)

  // Material 3 tonal palette: primary/tertiary + neutral surfaces.
  const p = scheme.palettes.primary as TonalPalette
  const n = scheme.palettes.neutral as TonalPalette
  const nv = scheme.palettes.neutralVariant as TonalPalette
  const t = scheme.palettes.tertiary as TonalPalette

  // Naive UI works on a light theme with overrides.
  const primary = argbToCss(p.tone(40))
  const primaryHover = argbToCss(p.tone(50))
  const primaryPressed = argbToCss(p.tone(30))

  return {
    seed,
    primary,
    primaryHover,
    primaryPressed,
    info: primary,
    success: argbToCss(p.tone(60)),
    warning: argbToCss(t.tone(60)),
    error: argbToCss(p.tone(80)),
    bodyBg: argbToCss(n.tone(98)),
    cardBg: argbToCss(n.tone(99)),
    border: argbToCss(nv.tone(90)),
    text: argbToCss(n.tone(10)),
    textSub: argbToCss(nv.tone(50)),
  }
}

export function themeOverrides(t: Md3Theme): GlobalThemeOverrides {
  return {
    common: {
      primaryColor: t.primary,
      primaryColorHover: t.primaryHover,
      primaryColorPressed: t.primaryPressed,
      infoColor: t.info,
      successColor: t.success,
      warningColor: t.warning,
      errorColor: t.error,
      bodyColor: t.bodyBg,
      cardColor: t.cardBg,
      borderColor: t.border,
      textColorBase: t.text,
      textColor1: t.text,
      textColor2: t.textSub,
      fontSize: '14px',
      borderRadius: '8px',
    },
    Button: {
      borderRadiusMedium: '8px',
    },
    DataTable: {
      thColor: t.cardBg,
      tdColor: t.cardBg,
      borderColor: t.border,
    },
  }
}

/** Extract ABGR accent from DWM AccentColor value (0xAABBGGRR). */
export function accentFromAbgr(v: number): number {
  const r = (v >> 16) & 0xff
  const g = (v >> 8) & 0xff
  const b = v & 0xff
  // material-color-utilities wants 0xAARRGGBB
  return (0xff << 24) | (r << 16) | (g << 8) | b
}

export { SEED_FALLBACK, argbFromHex }
