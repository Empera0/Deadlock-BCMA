import { useEffect, useState } from "react";

/** Everything the player sets once and the screens read: kept on this computer, the same for every screen. */
export const SETTINGS = {
  /** The player's own Steam account ID. */
  account: "account-id",
  /** The ranks the Draft screen reads win rates at: the lowest average badge, and the highest ("" for no limit). */
  draftBadge: "draft-badge",
  draftMaxBadge: "draft-max-badge",
  /** The ranks the Build generator reads what players build at: "" for the top of the ladder, else as the Draft's. */
  buildBadge: "build-badge",
  buildMaxBadge: "build-max-badge",
  /** The Master builder's focus and damage to start with. */
  masterFocus: "master-focus",
  masterDamage: "master-damage",
  /** Build for the player's match as soon as it shows in the console log. */
  watchLive: "watch-live",
  /** Share finished matches with deadlock-api. */
  shareMatches: "share-matches",
  /** The look: a theme of THEMES, and compact or comfortable spacing. */
  /** Show how the last match went once it ends. */
  postMatch: "post-match",
  /** Follow the match being played on its own, and go to the Live screen when one starts. */
  liveTrack: "live-track",
  liveSwitch: "live-switch",
  /** Show the lane note when a match starts. */
  laneNote: "lane-note",
  theme: "theme",
  density: "density",
  /** The colors of the Custom theme, as JSON (`CustomColors`). */
  customTheme: "custom-theme",
  /** The size of everything in percent, corners (sharp, normal, round), font, and whether controls animate. */
  fontScale: "font-scale",
  corners: "corners",
  font: "font",
  motion: "motion",
} as const;

export const DEFAULTS: Record<string, string> = {
  [SETTINGS.account]: "",
  [SETTINGS.draftBadge]: "101",
  [SETTINGS.draftMaxBadge]: "",
  [SETTINGS.buildBadge]: "",
  [SETTINGS.buildMaxBadge]: "",
  [SETTINGS.masterFocus]: "auto",
  [SETTINGS.masterDamage]: "auto",
  [SETTINGS.watchLive]: "0",
  [SETTINGS.shareMatches]: "1",
  [SETTINGS.postMatch]: "1",
  [SETTINGS.laneNote]: "1",
  [SETTINGS.liveTrack]: "1",
  [SETTINGS.liveSwitch]: "1",
  [SETTINGS.theme]: "obsidian",
  [SETTINGS.density]: "compact",
  [SETTINGS.customTheme]: "",
  [SETTINGS.fontScale]: "100",
  [SETTINGS.corners]: "normal",
  [SETTINGS.font]: "default",
  [SETTINGS.motion]: "1",
};

/** The ranks, by their badge tier (badge = tier × 10 + subrank 1–6); the names are the game's own, shown as they are (not translated where shown). */
export const TIERS = ["Initiate", "Seeker", "Acolyte", "Sentinel", "Mystic", "Ritualist", "Emissary", "Oracle", "Phantom", "Ascendant", "Eternus"];

/** The lowest badge of a tier, and the highest. */
export const tierFrom = (i: number) => (i + 1) * 10 + 1;
export const tierTo = (i: number) => (i + 1) * 10 + 6;

/** A rank range setting as the backend takes it: the lowest badge (null for the top of the ladder) and the highest. */
export function rankRange(fromKey: string, toKey: string): { minBadge: number | null; maxBadge: number | null } {
  const from = readSetting(fromKey);
  const to = readSetting(toKey);
  return { minBadge: from === "" ? null : Number(from), maxBadge: from === "" || to === "" ? null : Number(to) };
}

/** The themes, with the colors their swatch shows: background, panel, text, accent; the names translated where shown. */
export const THEMES: [string, string, [string, string, string, string]][] = [
  ["obsidian", "Obsidian", ["#0b0b0d", "#121215", "#ececef", "#e6cf9c"]],
  ["graphite", "Graphite", ["#17181b", "#1e1f23", "#e8eaed", "#8ab4f8"]],
  ["midnight", "Midnight", ["#0b1020", "#10172a", "#e4ebf7", "#5fd4e6"]],
  ["moss", "Moss", ["#0e1311", "#131a17", "#e5ede8", "#7fd6a4"]],
  ["plum", "Plum", ["#131016", "#19151d", "#efe8f2", "#f0a8d0"]],
  ["paper", "Paper", ["#f4f3ef", "#ffffff", "#1d1c1a", "#a46a12"]],
];

/** The colors a theme is made from; the rest (lines, hover, muted text…) are worked out from them. */
export interface CustomColors {
  light: boolean;
  bg: string;
  panel: string;
  text: string;
  accent: string;
  /** The colors of weapon, vitality and spirit items and stats; empty for the theme's own. */
  weapon: string;
  vitality: string;
  spirit: string;
}

/** Themes made from a few colors, the way the Custom theme is. */
export const MADE_THEMES: [string, string, CustomColors][] = [
  ["crimson", "Crimson", { light: false, bg: "#120b0c", panel: "#1a1013", text: "#f2e6e7", accent: "#ff6b6b", weapon: "", vitality: "", spirit: "" }],
  ["ember", "Ember", { light: false, bg: "#120e0a", panel: "#1a140e", text: "#f3ebe2", accent: "#ff9f43", weapon: "", vitality: "", spirit: "" }],
  ["hidden-king", "Hidden King", { light: false, bg: "#100d07", panel: "#18130b", text: "#f4ecd9", accent: "#f2b84b", weapon: "", vitality: "", spirit: "" }],
  ["archmother", "Archmother", { light: false, bg: "#071216", panel: "#0c1b21", text: "#e2f2f6", accent: "#4fc3f7", weapon: "", vitality: "", spirit: "" }],
  ["violet", "Violet", { light: false, bg: "#0f0c1a", panel: "#161226", text: "#ece8f7", accent: "#a78bfa", weapon: "", vitality: "", spirit: "" }],
  ["nord", "Nord", { light: false, bg: "#2e3440", panel: "#353c4a", text: "#eceff4", accent: "#88c0d0", weapon: "", vitality: "", spirit: "" }],
  ["frost", "Frost", { light: true, bg: "#eef3f8", panel: "#ffffff", text: "#18202a", accent: "#2b6cb0", weapon: "", vitality: "", spirit: "" }],
  ["sand", "Sand", { light: true, bg: "#f3ece0", panel: "#fbf7ef", text: "#2a241b", accent: "#b4532a", weapon: "", vitality: "", spirit: "" }],
];

export const DEFAULT_CUSTOM: CustomColors = { light: false, bg: "#101014", panel: "#17171c", text: "#ececef", accent: "#e6cf9c", weapon: "", vitality: "", spirit: "" };

/** The Custom theme's colors as kept, or the starting ones. */
export function customColors(): CustomColors {
  try {
    const kept = JSON.parse(readSetting(SETTINGS.customTheme) || "null");
    return kept ? { ...DEFAULT_CUSTOM, ...kept } : DEFAULT_CUSTOM;
  } catch {
    return DEFAULT_CUSTOM;
  }
}

const hex = (c: string): [number, number, number] => {
  const h = c.replace("#", "");
  const full = h.length === 3 ? h.split("").map((x) => x + x).join("") : h.padEnd(6, "0");
  return [0, 2, 4].map((i) => parseInt(full.slice(i, i + 2), 16) || 0) as [number, number, number];
};
const toHex = (rgb: number[]) => "#" + rgb.map((v) => Math.round(Math.max(0, Math.min(255, v))).toString(16).padStart(2, "0")).join("");
/** `a` moved toward `b` by `share` (0-1). */
export const mix = (a: string, b: string, share: number) => {
  const [x, y] = [hex(a), hex(b)];
  return toHex(x.map((v, i) => v + (y[i] - v) * share));
};
const luminance = (c: string) => {
  const [r, g, b] = hex(c).map((v) => v / 255);
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
};

/** Every color token of a theme made from its few colors. */
export function themeTokens(c: CustomColors): Record<string, string> {
  const [r, g, b] = hex(c.text);
  const out: Record<string, string> = {
    "--bg": c.bg,
    "--bg-deep": mix(c.bg, c.light ? "#000000" : "#000000", c.light ? 0.04 : 0.3),
    "--panel": c.panel,
    "--panel-2": mix(c.panel, c.text, 0.06),
    "--line": mix(c.panel, c.text, 0.12),
    "--border": mix(c.panel, c.text, 0.09),
    "--line-strong": mix(c.panel, c.text, 0.18),
    "--line-hover": mix(c.panel, c.text, 0.27),
    "--hover": `rgba(${r}, ${g}, ${b}, 0.05)`,
    "--text": c.text,
    "--muted": mix(c.text, c.bg, 0.42),
    "--accent": c.accent,
    "--on-accent": luminance(c.accent) > 0.5 ? "#0b0b0d" : "#ffffff",
    "--card-base": mix(c.bg, c.panel, 0.5),
    "--card-text": c.text,
  };
  if (c.weapon) out["--weapon"] = c.weapon;
  if (c.vitality) out["--vitality"] = c.vitality;
  if (c.spirit) out["--spirit"] = c.spirit;
  return out;
}

/** The fonts that can be picked; `default` is the theme's own. */
export const FONTS: [string, string, string][] = [
  ["default", "Default", ""],
  ["segoe", "Segoe UI", '"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif'],
  ["barlow", "Barlow Condensed", '"Barlow Condensed", "Segoe UI", sans-serif'],
  ["serif", "Serif", 'Georgia, "Times New Roman", serif'],
  ["mono", "Monospace", 'Consolas, ui-monospace, monospace'],
];

/** Corner sizes per choice: extra small, small, normal, large. */
const CORNERS: Record<string, [number, number, number, number]> = {
  sharp: [0, 1, 2, 3],
  normal: [3, 6, 8, 12],
  round: [6, 10, 14, 20],
};

/** The tokens the look setters put on the page themselves, cleared before each change. */
let applied: string[] = [];

/** Puts the chosen theme, density, text size, corners, font and motion on the page; called at start and whenever
 *  one changes. Themes made from colors are set as tokens on the page, over the plain dark or light theme. */
export function applyLook() {
  const root = document.documentElement;
  const theme = readSetting(SETTINGS.theme);
  const made = theme === "custom" ? customColors() : MADE_THEMES.find(([key]) => key === theme)?.[2];
  for (const name of applied) root.style.removeProperty(name);
  const tokens: Record<string, string> = made ? themeTokens(made) : {};
  root.dataset.theme = made ? (made.light ? "paper" : "obsidian") : theme;
  root.dataset.density = readSetting(SETTINGS.density);
  root.dataset.motion = readSetting(SETTINGS.motion) === "0" ? "off" : "on";
  // The whole window is scaled, text and layout together: sizes are set in pixels throughout.
  const scale = Math.max(80, Math.min(140, Number(readSetting(SETTINGS.fontScale)) || 100)) / 100;
  root.style.zoom = scale === 1 ? "" : String(scale);
  const [xs, sm, r, lg] = CORNERS[readSetting(SETTINGS.corners)] ?? CORNERS.normal;
  Object.assign(tokens, { "--r-xs": `${xs}px`, "--r-sm": `${sm}px`, "--r": `${r}px`, "--r-lg": `${lg}px` });
  const font = FONTS.find(([key]) => key === readSetting(SETTINGS.font))?.[2];
  if (font) tokens["--font"] = font;
  for (const [name, value] of Object.entries(tokens)) root.style.setProperty(name, value);
  applied = Object.keys(tokens);
}

const LOOK_KEYS: string[] = [SETTINGS.theme, SETTINGS.density, SETTINGS.customTheme, SETTINGS.fontScale, SETTINGS.corners, SETTINGS.font, SETTINGS.motion];

const EVENT = "bcma-setting";

export function readSetting(key: string): string {
  try {
    return localStorage.getItem(key) ?? DEFAULTS[key] ?? "";
  } catch {
    return DEFAULTS[key] ?? "";
  }
}

export function writeSetting(key: string, value: string) {
  try {
    localStorage.setItem(key, value);
  } catch {
    /* Not kept: the setting holds for this session only. */
  }
  window.dispatchEvent(new CustomEvent(EVENT, { detail: { key, value } }));
  if (LOOK_KEYS.includes(key)) applyLook();
}

/** A setting, kept in step across every screen that reads it. */
export function useSetting(key: string): [string, (value: string) => void] {
  const [value, setValue] = useState(() => readSetting(key));
  useEffect(() => {
    const on = (e: Event) => {
      const d = (e as CustomEvent<{ key: string; value: string }>).detail;
      if (d.key === key) setValue(d.value);
    };
    window.addEventListener(EVENT, on);
    return () => window.removeEventListener(EVENT, on);
  }, [key]);
  return [value, (v: string) => writeSetting(key, v)];
}

/** A Steam account as the app wants it: the short account ID, from either the short or the long (7656…) form. */
export function accountFrom(text: string): string | null {
  const digits = text.trim();
  if (!/^\d+$/.test(digits)) return null;
  const n = BigInt(digits);
  const base = 76561197960265728n;
  return (n > base ? n - base : n).toString();
}
