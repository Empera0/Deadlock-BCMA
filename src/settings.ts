import { useEffect, useState } from "react";

/** Everything the player sets once and the screens read: kept on this computer, the same for every screen. */
export const SETTINGS = {
  /** The player's own Steam account ID. */
  account: "account-id",
  /** The ranks the Draft screen reads win rates at: the lowest average badge, and the highest ("" for no limit). */
  draftBadge: "draft-badge",
  draftMaxBadge: "draft-max-badge",
  /** The Master builder's focus and damage to start with. */
  masterFocus: "master-focus",
  masterDamage: "master-damage",
  /** Build for the player's match as soon as it shows in the console log. */
  watchLive: "watch-live",
  /** Share finished matches with deadlock-api. */
  shareMatches: "share-matches",
} as const;

export const DEFAULTS: Record<string, string> = {
  [SETTINGS.account]: "",
  [SETTINGS.draftBadge]: "101",
  [SETTINGS.draftMaxBadge]: "",
  [SETTINGS.masterFocus]: "auto",
  [SETTINGS.masterDamage]: "auto",
  [SETTINGS.watchLive]: "0",
  [SETTINGS.shareMatches]: "1",
};

/** The ranks, by their badge tier (badge = tier × 10 + subrank 1–6); the names are the game's own, shown as they are (not translated where shown). */
export const TIERS = ["Initiate", "Seeker", "Alchemist", "Arcanist", "Ritualist", "Emissary", "Archon", "Oracle", "Phantom", "Ascendant", "Eternus"];

/** The lowest badge of a tier, and the highest. */
export const tierFrom = (i: number) => (i + 1) * 10 + 1;
export const tierTo = (i: number) => (i + 1) * 10 + 6;

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
