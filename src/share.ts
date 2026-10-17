import { Build, Hero, Item } from "./api";

/** Share codes start with this, so an old code can still be told apart once the format changes. */
const PREFIX = "BCMA1.";

/** What a share code carries: ids instead of class names keep it short. */
interface Packed {
  h: number;
  i: number[];
  u?: number;
  s?: number;
  a?: number[];
  t: number[];
  m?: [number, number][];
  c?: number;
}

const toBase64Url = (text: string) => btoa(text).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");
const fromBase64Url = (code: string) => atob(code.replace(/-/g, "+").replace(/_/g, "/"));

/** The build as a code to paste somewhere: hero, purchase order, souls, ability upgrades, imbues and toggled effects. */
export function shareCode(build: Build, heroes: Hero[], items: Map<string, Item>): string {
  const id = (c: string) => items.get(c)?.id;
  const ids = (list: string[]) => list.map(id).filter((x): x is number => x !== undefined);
  const packed: Packed = { h: heroes.find((h) => h.class_name === build.hero)?.id ?? 0, i: ids(build.purchases), t: build.ability_tiers };
  if (build.upto !== null) packed.u = build.upto;
  if (build.souls !== null) packed.s = build.souls;
  if (build.active.length) packed.a = ids(build.active);
  const imbues = Object.entries(build.imbues).flatMap(([c, index]) => (id(c) !== undefined ? [[id(c)!, index] as [number, number]] : []));
  if (imbues.length) packed.m = imbues;
  const copied = heroes.find((h) => h.class_name === build.copied_ult)?.id;
  if (copied !== undefined) packed.c = copied;
  return PREFIX + toBase64Url(JSON.stringify(packed));
}

/** The build a share code stands for, or an error that says what is wrong with it. */
export function readShareCode(code: string, heroes: Hero[], items: Item[]): Partial<Build> {
  const text = code.trim();
  if (!text.startsWith(PREFIX)) throw new Error("Not a build code: it should start with " + PREFIX);
  let packed: Packed;
  try {
    packed = JSON.parse(fromBase64Url(text.slice(PREFIX.length)));
  } catch {
    throw new Error("The build code is damaged: copy it again in full");
  }
  const hero = heroes.find((h) => h.id === packed.h);
  if (!hero) throw new Error("The code's hero is not in the game data: use Update data");
  const byId = new Map(items.map((i) => [i.id, i.class_name]));
  const classes = (list: number[] = []) => list.map((x) => byId.get(x)).filter((x): x is string => x !== undefined);
  const purchases = classes(packed.i);
  return {
    hero: hero.class_name,
    purchases,
    upto: packed.u !== undefined && packed.u < purchases.length ? packed.u : null,
    souls: packed.s ?? null,
    active: classes(packed.a),
    ability_tiers: Array.isArray(packed.t) ? packed.t.map((t) => Math.max(0, Math.min(3, Number(t) || 0))) : [0, 0, 0, 0],
    imbues: Object.fromEntries((packed.m ?? []).flatMap(([x, index]) => (byId.has(x) ? [[byId.get(x)!, index]] : []))),
    copied_ult: heroes.find((h) => h.id === packed.c)?.class_name ?? null,
  };
}
