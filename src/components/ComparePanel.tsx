import { useEffect, useMemo, useState } from "react";
import { api, Build, fmt, Hero, Item, PowerPoint, Sheet, Stat } from "../api";
import { readShareCode } from "../share";

const STEP = 1000;

/** The build at each step of souls: its purchases in order, as far as the souls pay for them. */
function along(build: Build, items: Map<string, Item>, end: number): Build[] {
  const out: Build[] = [];
  for (let souls = STEP; souls <= end; souls += STEP) {
    let spent = 0;
    const purchases = build.purchases.filter((c) => (spent += items.get(c)?.cost ?? 0) <= souls);
    out.push({ ...build, souls, purchases, upto: null, active: build.active.filter((a) => purchases.includes(a)) });
  }
  return out;
}

const W = 340;
const H = 140;
const PAD = { left: 44, right: 10, top: 10, bottom: 22 };

/** Two lines over the souls: this build and the other. */
function TwoLines({ title, a, b, souls }: { title: string; a: number[]; b: number[]; souls: number[] }) {
  const max = Math.max(1, ...a, ...b);
  const end = souls[souls.length - 1] ?? 1;
  const x = (s: number) => PAD.left + ((W - PAD.left - PAD.right) * s) / end;
  const y = (v: number) => H - PAD.bottom - ((H - PAD.top - PAD.bottom) * v) / max;
  const path = (vals: number[]) => vals.map((v, i) => `${i ? "L" : "M"}${x(souls[i]).toFixed(1)},${y(v).toFixed(1)}`).join("");
  return (
    <figure className="power-chart">
      <figcaption>{title}</figcaption>
      <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label={`${title} of both builds by souls`}>
        {[0, 0.5, 1].map((f) => (
          <g key={f}>
            <line x1={PAD.left} x2={W - PAD.right} y1={y(f * max)} y2={y(f * max)} className="grid" />
            <text x={PAD.left - 6} y={y(f * max) + 3} textAnchor="end" className="tick">
              {fmt(f * max, 0)}
            </text>
          </g>
        ))}
        {[0, 10000, 20000, 30000, 40000]
          .filter((s) => s <= end)
          .map((s) => (
            <text key={s} x={x(s)} y={H - 5} textAnchor="middle" className="tick">
              {s / 1000}k
            </text>
          ))}
        <path d={path(a)} fill="none" stroke="var(--accent)" strokeWidth={2} />
        <path d={path(b)} fill="none" stroke="var(--spirit)" strokeWidth={2} strokeDasharray="5 3" />
      </svg>
    </figure>
  );
}

/** This build against another: a saved one or one from a share code, stat by stat, in a fight and over the souls. */
export default function ComparePanel({ heroes, items, build, sheet }: { heroes: Hero[]; items: Map<string, Item>; build: Build; sheet: Sheet }) {
  const [saved, setSaved] = useState<string[]>([]);
  const [other, setOther] = useState<Build | null>(null);
  const [otherName, setOtherName] = useState("");
  const [code, setCode] = useState("");
  const [otherSheet, setOtherSheet] = useState<Sheet | null>(null);
  const [fight, setFight] = useState<(PowerPoint | null)[] | null>(null);
  const [curve, setCurve] = useState<{ souls: number[]; a: PowerPoint[]; b: PowerPoint[] } | null>(null);
  const [error, setError] = useState("");

  useEffect(() => {
    api.listBuilds().then(setSaved).catch(() => setSaved([]));
  }, []);

  useEffect(() => {
    if (!other) return;
    let stale = false;
    setError("");
    const end = Math.max(build.souls ?? sheet.souls, other.souls ?? 0, 10_000);
    const stepsA = along(build, items, end);
    const stepsB = along(other, items, end);
    Promise.all([api.compute(other), api.powerCurve([build, other]), api.powerCurve([...stepsA, ...stepsB])])
      .then(([s, now, all]) => {
        if (stale) return;
        setOtherSheet(s);
        setFight(now);
        const pts = (list: (PowerPoint | null)[]) => list.map((p) => p ?? { souls: 0, fight_damage: 0, effective_health: 0 });
        setCurve({ souls: stepsA.map((b) => b.souls ?? 0), a: pts(all.slice(0, stepsA.length)), b: pts(all.slice(stepsA.length)) });
      })
      .catch((e) => !stale && setError(String(e)));
    return () => {
      stale = true;
    };
  }, [other, build, sheet.souls, items]);

  const loadSaved = (name: string) => {
    if (!name) return;
    api
      .loadBuild(name)
      .then((b) => {
        setOther({ ...build, ...b });
        setOtherName(name);
      })
      .catch((e) => setError(String(e)));
  };
  const loadCode = () => {
    try {
      setOther({ ...build, ...readShareCode(code, heroes, [...items.values()]) });
      setOtherName("Build from the code");
      setError("");
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const rows = useMemo(() => {
    if (!otherSheet) return [];
    const theirs = new Map(otherSheet.stats.map((s) => [s.key, s]));
    return sheet.stats
      .filter((s) => s.group !== "target" && s.group !== "other")
      .map((s): [Stat, Stat | undefined] => [s, theirs.get(s.key)])
      .filter(([a, b]) => (a.value ?? 0) !== 0 || (b?.value ?? 0) !== 0);
  }, [sheet, otherSheet]);
  const heroName = (cls: string) => heroes.find((h) => h.class_name === cls)?.name ?? cls;
  const diff = (a: number | null, b: number | null | undefined) => {
    if (a === null || b === null || b === undefined) return null;
    return b - a;
  };

  return (
    <aside className="panel compare-panel">
      <div className="group">
        <h3>Compare with another build</h3>
        <div className="fields">
          <label className="field wide">
            A saved build
            <select value="" onChange={(e) => loadSaved(e.target.value)}>
              <option value="">Choose…</option>
              {saved.map((s) => (
                <option key={s}>{s}</option>
              ))}
            </select>
          </label>
          <label className="field wide">
            Or a build code
            <input placeholder="BCMA1.…" value={code} onChange={(e) => setCode(e.target.value)} onKeyDown={(e) => e.key === "Enter" && loadCode()} />
          </label>
          <button onClick={loadCode} disabled={!code.trim()}>
            Load
          </button>
        </div>
        {error && <div className="error">{error}</div>}
        {!other && <p className="muted">Pick a saved build or paste a friend's build code to set it against this one.</p>}
      </div>

      {other && otherSheet && (
        <>
          <div className="group">
            <h3>
              <span className="compare-key a">This build</span> {heroName(build.hero)} · {fmt(sheet.souls)} souls
              <span className="compare-key b">{otherName}</span> {heroName(other.hero)} · {fmt(otherSheet.souls)} souls
            </h3>
            {fight && fight[0] && fight[1] && (
              <div className="tiles small">
                <div>
                  <span>Fight damage</span>
                  <b>
                    {fmt(fight[0].fight_damage, 0)} / {fmt(fight[1].fight_damage, 0)}
                  </b>
                </div>
                <div>
                  <span>Effective health</span>
                  <b>
                    {fmt(fight[0].effective_health, 0)} / {fmt(fight[1].effective_health, 0)}
                  </b>
                </div>
                <div>
                  <span>Souls spent</span>
                  <b>
                    {fmt(sheet.spent)} / {fmt(otherSheet.spent)}
                  </b>
                </div>
              </div>
            )}
            <table className="match-table compare-stats">
              <thead>
                <tr>
                  <th className="left">Stat</th>
                  <th>This</th>
                  <th>Other</th>
                  <th>Other − this</th>
                </tr>
              </thead>
              <tbody>
                {rows.map(([a, b]) => {
                  const d = diff(a.value, b?.value);
                  return (
                    <tr key={a.key}>
                      <td className="left">{a.label}</td>
                      <td>
                        {fmt(a.value, 1)}
                        {a.unit}
                      </td>
                      <td>
                        {fmt(b?.value, 1)}
                        {b?.unit}
                      </td>
                      <td className={d === null || Math.abs(d) < 1e-6 ? "muted" : d > 0 ? "up" : "down"}>
                        {d === null ? "–" : `${d > 0 ? "+" : ""}${fmt(d, 1)}`}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          {curve && (
            <div className="group">
              <h3>Over the souls</h3>
              <div className="power-charts">
                <TwoLines title="Fight damage" souls={curve.souls} a={curve.a.map((p) => p.fight_damage)} b={curve.b.map((p) => p.fight_damage)} />
                <TwoLines title="Effective health" souls={curve.souls} a={curve.a.map((p) => p.effective_health)} b={curve.b.map((p) => p.effective_health)} />
              </div>
              <p className="muted">
                Solid: this build; dashed: the other. Each build's items are bought in its own order as the souls allow, with its own ability
                upgrades throughout. Fight damage is every ability and active item once and six seconds of shooting.
              </p>
            </div>
          )}
        </>
      )}
    </aside>
  );
}
