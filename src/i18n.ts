import { TR } from "./locales/tr";

/** The languages the app is written in. English is the source: every text is written in English in the code and
 *  looked up in the other language's dictionary by that English text. */
export type Lang = "en" | "tr";

export const LANGUAGES: [Lang, string][] = [
  ["en", "English"],
  ["tr", "Türkçe"],
];

const KEY = "language";

function readLang(): Lang {
  try {
    return localStorage.getItem(KEY) === "tr" ? "tr" : "en";
  } catch {
    return "en";
  }
}

/** The language of this run of the app. Changing it starts the window again, so every screen and every sentence the
 *  engine writes follows it at once. */
export const LANG: Lang = readLang();

/** The locale numbers and dates are written in. */
export const LOCALE = LANG === "tr" ? "tr-TR" : "en-US";

export function setLang(lang: Lang) {
  try {
    localStorage.setItem(KEY, lang);
  } catch {
    /* Not kept: the language holds for this run only. */
  }
  window.location.reload();
}

/** A text in the app's language, with `{name}` filled from `vars`. A text missing from the dictionary stays in
 *  English; `npm run i18n` lists any such text. */
export function t(en: string, vars?: Record<string, string | number>): string {
  const text = LANG === "en" ? en : (TR[en] ?? en);
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (all, name: string) => (name in vars ? String(vars[name]) : all));
}

/** A text with a count: the singular or the plural English text, looked up and filled with `n` and `vars`. */
export function tn(n: number, one: string, many: string, vars?: Record<string, string | number>): string {
  return t(n === 1 ? one : many, { n, ...vars });
}

/** A share as the language writes it: "45%" in English, "%45" in Turkish. */
export function percent(n: string | number): string {
  return LANG === "tr" ? `%${n}` : `${n}%`;
}

/** A date as the language writes it. */
export function date(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleDateString(LOCALE);
}

export function dateTime(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString(LOCALE);
}
