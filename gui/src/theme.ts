// Material Design 3 dynamic theming for Naive UI.
//
// Reads the Windows accent color (DWM AccentColor, ABGR) via a Tauri command,
// then builds a Material 3 tonal palette with material-color-utilities and
// maps it onto Naive UI's theme overrides (seed -> primary palette).
//
// Light scheme: primary tone ~40, surface tone ~98, neutral text ~10
// Dark scheme:  primary tone ~80, surface tone ~6,  neutral text ~90

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

/** Build Naive UI theme overrides from a Material 3 source color (light). */
export function themeFromSeed(seed: number): Md3Theme {
  const scheme = themeFromSourceColor(seed)
  const p = scheme.palettes.primary as TonalPalette
  const n = scheme.palettes.neutral as TonalPalette
  const nv = scheme.palettes.neutralVariant as TonalPalette
  const t = scheme.palettes.tertiary as TonalPalette

  return {
    seed,
    primary: argbToCss(p.tone(40)),
    primaryHover: argbToCss(p.tone(50)),
    primaryPressed: argbToCss(p.tone(30)),
    info: argbToCss(p.tone(40)),
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

/** Dark variant — same seed, inverted surface tones. */
export function themeFromSeedDark(seed: number): Md3Theme {
  const scheme = themeFromSourceColor(seed)
  const p = scheme.palettes.primary as TonalPalette
  const n = scheme.palettes.neutral as TonalPalette
  const nv = scheme.palettes.neutralVariant as TonalPalette
  const t = scheme.palettes.tertiary as TonalPalette

  return {
    seed,
    primary: argbToCss(p.tone(80)),
    primaryHover: argbToCss(p.tone(70)),
    primaryPressed: argbToCss(p.tone(90)),
    info: argbToCss(p.tone(80)),
    success: argbToCss(p.tone(60)),
    warning: argbToCss(t.tone(60)),
    error: argbToCss(p.tone(60)),
    bodyBg: argbToCss(n.tone(6)),
    cardBg: argbToCss(n.tone(10)),
    border: argbToCss(nv.tone(30)),
    text: argbToCss(n.tone(90)),
    textSub: argbToCss(nv.tone(80)),
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
    Button: { borderRadiusMedium: '8px' },
    DataTable: {
      thColor: t.cardBg,
      tdColor: t.cardBg,
      borderColor: t.border,
    },
  }
}

export function accentFromAbgr(v: number): number {
  const r = (v >> 16) & 0xff
  const g = (v >> 8) & 0xff
  const b = v & 0xff
  return (0xff << 24) | (r << 16) | (g << 8) | b
}

export { SEED_FALLBACK, argbFromHex }
