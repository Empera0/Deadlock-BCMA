// Lists the engine's sentences the Turkish templates do not cover yet, most common first: the engine's answers
// are read from JSON files (`cargo run --release -p engine --example text_dump -- <dir> <match json>...` writes
// them) and put through the same translation the app uses.
// `node scripts/i18n-audit.mjs <answer json>... [--all]`
import { readFileSync } from "node:fs";
import { createServer } from "vite";

globalThis.localStorage = { getItem: () => "tr", setItem: () => {} };
const vite = await createServer({ server: { middlewareMode: true }, appType: "custom", logLevel: "error" });
const { translateDeep, missed } = await vite.ssrLoadModule("/src/engineText.ts");

const files = process.argv.slice(2).filter((a) => !a.startsWith("--"));
const all = process.argv.includes("--all");
let sample = null;
for (const f of files) {
  const v = translateDeep(JSON.parse(readFileSync(f, "utf8")));
  sample ??= v;
}
// Hero, item and ability names stay in English.
const data = JSON.parse(readFileSync("src-tauri/data/snapshot.json", "utf8"));
const names = new Set([...data.items.map((i) => i.name), ...data.heroes.flatMap((h) => [h.name, ...h.abilities.map((a) => a.name)])]);
// The same sentence with other numbers is one template.
const shapes = new Map();
for (const [s, n] of missed) {
  if (names.has(s.replace(/:$/, "")) || s.split(": ").every((x) => names.has(x.replace(/ \(.*\)$/, "")))) continue;
  const shape = s.replace(/[−+-]?\d[\d,]*(\.\d+)?/g, "{n}");
  const e = shapes.get(shape) ?? { n: 0, example: s };
  e.n += n;
  shapes.set(shape, e);
}
const rows = [...shapes].sort((a, b) => b[1].n - a[1].n);
for (const [shape, e] of all ? rows : rows.slice(0, 400)) console.log(`${String(e.n).padStart(6)}  ${shape}`);
console.log(`\n${rows.length} sentences not covered`);
await vite.close();
