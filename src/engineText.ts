import { LANG, NAMES } from "./i18n";
import { PREFIXES, SENTENCES, WORDS } from "./locales/engine-tr";
import GAME from "./locales/game-tr.json";
import NAME_ROWS from "./locales/names-tr.json";

/**
 * The engine writes its sentences in English, and also reads some of them back, so it keeps writing English. Its
 * answers are put into the app's language here, on their way in: every text in an answer is matched against the
 * English sentence templates the engine writes, and the matching Turkish template is filled with the parts.
 *
 * A template is the engine's sentence with `{name}` for each part that changes. A part named `n`, `n1`, `n2`… is a
 * number and is written the Turkish way; any other part is put into Turkish on its own when it is a known word or
 * sentence, and kept as it is otherwise (hero and item names).
 *
 * Hero, item and ability names have a language of their own (NAMES): with Turkish names, the game's Turkish name takes
 * the place of the English one, in the game data, in the engine's answers and inside its sentences, whatever the
 * app's language. With an English app and Turkish names, the templates fill the English sentence again.
 */

/** Sentences in Turkish. */
const TEXT = LANG === "tr";

/** The game's Turkish names: by class name, and by English name for the names inside sentences. */
const byClass = new Map<string, string>();
const byName = new Map<string, string>();
if (NAMES === "tr") {
  for (const [cls, en, tr] of NAME_ROWS as [string, string, string][]) {
    byClass.set(cls, tr);
    if (!byName.has(en)) byName.set(en, tr);
  }
}

interface Rule {
  re: RegExp;
  names: string[];
  tr: string;
  /** Length of the template's fixed text: more specific templates are tried first. */
  weight: number;
}

const NUMBER = "[−+-]?\\d+(?:,\\d{3})*(?:\\.\\d+)?";

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

function compile(en: string, tr: string): Rule {
  const names: string[] = [];
  let pattern = "^";
  let weight = 0;
  const parts = en.split(/(\{\w+\})/);
  for (const [at, part] of parts.entries()) {
    const name = /^\{(\w+)\}$/.exec(part)?.[1];
    if (name) {
      names.push(name);
      // A part never runs over the end of a sentence.
      pattern += /^n\d*$/.test(name) ? `(${NUMBER})` : at === parts.length - 2 && parts[at + 1] === "" ? "((?:(?!\\. ).)+?)(?<!\\.)" : "((?:(?!\\. ).)+?)";
    } else {
      pattern += escape(part);
      weight += part.length;
    }
  }
  return { re: new RegExp(pattern + "$", "s"), names, tr, weight };
}

const exact = new Map<string, string>();
/** Rules by the first word of their fixed text, so a text is only tried against templates that can match. */
const byWord = new Map<string, Rule[]>();
const open: Rule[] = [];

function firstWord(s: string): string {
  return /^\S+/.exec(s)?.[0] ?? "";
}

function build() {
  if (TEXT) for (const [en, tr] of Object.entries(GAME as Record<string, string>)) exact.set(en, tr);
  for (const [en, tr] of SENTENCES) {
    if (!en.includes("{")) {
      if (TEXT) exact.set(en, tr);
      continue;
    }
    // With English sentences the template fills the English again, with the names inside put into Turkish.
    const rule = compile(en, TEXT ? tr : en);
    // Only a first word without a part in it can pick the template out.
    const head = /^\S+/.exec(en)?.[0] ?? "";
    const word = head.includes("{") ? "" : head;
    if (!word) open.push(rule);
    else byWord.set(word, [...(byWord.get(word) ?? []), rule]);
  }
  for (const list of [...byWord.values(), open]) list.sort((a, b) => b.weight - a.weight);
}

if (TEXT || NAMES === "tr") build();

/** Texts the templates did not cover in this run: the audit lists them. */
export const missed = new Map<string, number>();

/** A number as Turkish writes it: a comma for the decimal point, a dot between thousands. */
function number(s: string): string {
  if (!TEXT) return s;
  const sign = s.match(/^[−+-]/)?.[0] ?? "";
  const plain = s.slice(sign.length).replace(/,/g, "");
  const [whole, frac] = plain.split(".");
  const grouped = whole.length > 3 ? whole.replace(/\B(?=(\d{3})+(?!\d))/g, ".") : whole;
  return sign + grouped + (frac ? "," + frac : "");
}

/** A part of a sentence: a known word, a sentence of its own, a list of such, or a name kept as it is. */
function part(s: string, depth: number): string {
  const named = byName.get(s);
  if (named !== undefined) return named;
  // "Violet's Abstract Landscape": a hero and one of its abilities.
  const owned = byName.size ? /^(.+)'s (.+)$/.exec(s) : null;
  if (owned && byName.has(owned[1]) && byName.has(owned[2])) return `${byName.get(owned[1])}: ${byName.get(owned[2])}`;
  const word = TEXT ? WORDS[s] : undefined;
  if (word !== undefined) return word;
  // A time in seconds, "4.1s".
  const seconds = TEXT ? /^([−+-]?\d+(?:\.\d+)?)s$/.exec(s) : null;
  if (seconds) return `${number(seconds[1])} sn`;
  const template = match(s, depth + 1);
  if (template !== null) return template;
  for (const sep of [", ", " and ", " or ", " + ", "; "]) {
    if (!s.includes(sep)) continue;
    const pieces = s.split(sep);
    const done = pieces.map((p) => part(p, depth + 1));
    if (done.some((d, i) => d !== pieces[i])) return done.join(TEXT && sep === " and " ? " ve " : TEXT && sep === " or " ? " veya " : sep);
  }
  return text(s, depth + 1, false);
}

function apply(rule: Rule, m: RegExpExecArray, depth: number): string {
  return rule.tr.replace(/\{(\w+)\}/g, (all, name: string) => {
    const at = rule.names.indexOf(name);
    if (at < 0) return all;
    const value = m[at + 1];
    return /^n\d*$/.test(name) ? number(value) : part(value, depth);
  });
}

/** A template written with a capital first letter also covers the text that starts small, and the other way round. */
function match(s: string, depth: number): string | null {
  const found = exactly(s, depth);
  if (found !== null) return found;
  // Only for sentences: a single word may be a key ("weapon", "spirit") that must stay as it is.
  if (!s.includes(" ")) return null;
  const first = s.charAt(0);
  const swapped = first === first.toLowerCase() ? first.toUpperCase() : first.toLowerCase();
  if (swapped === first) return null;
  const other = exactly(swapped + s.slice(1), depth);
  if (other === null) return null;
  const head = other.charAt(0);
  return (swapped === swapped.toUpperCase() ? head.toLocaleLowerCase("tr") : head.toLocaleUpperCase("tr")) + other.slice(1);
}

function exactly(s: string, depth: number): string | null {
  const known = exact.get(s);
  if (known !== undefined) return known;
  for (const rule of [...(byWord.get(firstWord(s)) ?? []), ...open]) {
    const m = rule.re.exec(s);
    if (m) return apply(rule, m, depth);
  }
  return null;
}

/** Where a text is cut into pieces when no template covers it whole, from the largest pieces to the smallest. */
const SPLITS = [/(\n)/, /((?<=[.!?])\s+)/, /(\s+·\s+)/, /((?<=;)\s+)/, /((?<=:)\s+)/, /(, )/];

/** One text of the engine's in Turkish, or as it is when no template covers it. */
function text(s: string, depth = 0, record = true, level = 0): string {
  if (depth > 24 || !/[A-Za-z]{2}/.test(s)) return s;
  // A name, also as a label ("Haze:").
  const named = byName.get(s) ?? (s.endsWith(":") && byName.has(s.slice(0, -1)) ? byName.get(s.slice(0, -1)) + ":" : undefined);
  if (named !== undefined) return named;
  const whole = match(s, depth);
  if (whole !== null) return whole;
  // A sentence that ends with a full stop the template does not have, or the other way round.
  const end = /[.;:]$/.exec(s)?.[0];
  if (end) {
    const bare = match(s.slice(0, -1), depth);
    if (bare !== null) return bare.endsWith(end) ? bare : bare + end;
  } else {
    const dotted = match(s + ".", depth);
    if (dotted !== null) return dotted.replace(/\.$/, "");
  }
  const stats = statList(s);
  if (stats !== null) return stats;
  // Several sentences, or several parts, in one text: each on its own.
  for (let i = level; i < SPLITS.length; i++) {
    const pieces = s.split(SPLITS[i]);
    if (pieces.length < 2) continue;
    const done = pieces.map((p, j) => (j % 2 === 1 ? p : text(p, depth + 1, false, i + 1)));
    if (done.some((d, j) => d !== pieces[j])) {
      if (record) pieces.forEach((p, j) => j % 2 === 0 && done[j] === p && note(p));
      return done.join("");
    }
  }
  if (record) note(s);
  return s;
}

function note(s: string) {
  if (/[A-Z\s]/.test(s) && /[a-z]{2}/.test(s)) missed.set(s, (missed.get(s) ?? 0) + 1);
}

const STAT = /([A-Za-z][A-Za-z'’. ]*?) (×?[−+-]?\d[\d,]*(?:\.\d+)?(?:%|s|m)?(?: scaling|\/stack| \/ meter)?)(?=, | [A-Z]|$)/g;

/** Game values listed the way the engine writes ability upgrades and their numbers: "Damage +40, Cooldown −2s".
 *  The labels are the game's own, put into Turkish one by one. */
function statList(s: string): string | null {
  let out = "";
  let last = 0;
  let changed = false;
  STAT.lastIndex = 0;
  for (let m = STAT.exec(s); m; m = STAT.exec(s)) {
    const gap = s.slice(last, m.index);
    if (gap !== "" && gap !== ", " && gap !== " ") return null;
    const label = exact.get(m[1]) ?? WORDS[m[1]];
    if (label !== undefined) changed = true;
    const value = m[2]
      .replace(/[−+-]?\d[\d,]*(?:\.\d+)?/, number)
      .replace(/ scaling$/, " ölçekleme")
      .replace(/\/stack$/, "/yığın")
      .replace(/ \/ meter$/, " / metre");
    out += gap + (label ?? m[1]) + " " + value;
    last = STAT.lastIndex;
  }
  return changed && last === s.length ? out : null;
}

/** Fields that hold names and keys rather than sentences: left as they are. */
const KEEP = new Set(["hero", "item", "class_name", "icon", "hero_icon", "key", "purchases", "active", "owned", "imbues", "player_name", "build", "image", "path", "url"]);

/** The engine's answer in the app's language: every text in it, at any depth. */
export function translateDeep<T>(value: T): T {
  if (!TEXT && NAMES !== "tr") return value;
  return deep(value) as T;
}

function deep(v: unknown): unknown {
  if (typeof v === "string") return text(v);
  if (Array.isArray(v)) return v.map(deep);
  if (v && typeof v === "object") {
    const out: Record<string, unknown> = {};
    // The name of something the game defines (an item, a hero, an ability) stays as the game's data has it.
    const defined = typeof (v as { class_name?: unknown }).class_name === "string" ? (v as { class_name: string }).class_name : null;
    for (const [k, x] of Object.entries(v)) {
      if (defined !== null && k === "name") out[k] = byClass.get(defined) ?? x;
      else out[k] = KEEP.has(k) ? x : deep(x);
    }
    return out;
  }
  return v;
}

/** One text of the engine's in the app's language. */
export function engineText(s: string): string {
  return TEXT || NAMES === "tr" ? text(s) : s;
}

/** The English an error was put into Turkish from: errors are sorted by their English words. */
const errors = new Map<string, string>();

/** An error of the backend in the app's language, its English kept for `englishOf`. */
export function engineError(s: string): string {
  const done = engineText(s);
  if (done !== s) errors.set(done, s);
  return done;
}

export function englishOf(s: string): string {
  return errors.get(s) ?? s;
}

/** Whether an engine sentence, in either language, starts the way the English one does. */
export function startsLike(s: string, english: string): boolean {
  return s.startsWith(english) || (LANG === "tr" && PREFIXES[english] !== undefined && s.startsWith(PREFIXES[english]));
}
