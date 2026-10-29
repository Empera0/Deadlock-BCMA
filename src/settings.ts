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
  theme: "theme",
  density: "density",
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
  [SETTINGS.theme]: "obsidian",
  [SETTINGS.density]: "compact",
};

/** The ranks, by their badge tier (badge = tier × 10 + subrank 1–6); the names are the game's own, shown as they are (not translated where shown). */
export const TIERS = ["Initiate", "Seeker", "Alchemist", "Arcanist", "Ritualist", "Emissary", "Archon", "Oracle", "Phantom", "Ascendant", "Eternus"];

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

/** Puts the chosen theme and density on the page; called at start and whenever either changes. */
export function applyLook() {
  const root = document.documentElement;
  root.dataset.theme = readSetting(SETTINGS.theme);
  root.dataset.density = readSetting(SETTINGS.density);
}

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
  if (key === SETTINGS.theme || key === SETTINGS.density) applyLook();
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
