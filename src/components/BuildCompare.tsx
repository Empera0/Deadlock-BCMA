import { useEffect, useMemo, useState } from "react";
import { api, Build, fmt, Hero, Item, PowerPoint, Sheet } from "../api";
import { percent, t } from "../i18n";
import ErrorNote from "./ErrorNote";

const COLORS = ["var(--accent)", "#4fc3f7"];
/** Stats listed side by side: the ones that differ most, this many. */
const STATS_SHOWN = 16;
const CURRENT = "";

interface Curve {
  /** One point per purchase: the souls spent by then and the build's numbers. */
  points: (PowerPoint & { item: string })[];
  sheet: Sheet | null;
}

/** The build after each of its purchases. */
const steps = (b: Build) => b.purchases.map((_, i) => ({ ...b, upto: i + 1, souls: null }));

/** Where `b` passes `a` (or falls behind it) along the souls, read between their points. */
function crossings(a: Curve["points"], b: Curve["points"], value: (p: PowerPoint) => number): { souls: number; ahead: 0 | 1 }[] {
  const at = (pts: Curve["points"], souls: number) => {
    const i = pts.findIndex((p) => p.souls >= souls);
    if (i <= 0) return value(pts[Math.max(i, 0)] ?? pts[pts.length - 1]);
    const [p, q] = [pts[i - 1], pts[i]];
    return value(p) + ((value(q) - value(p)) * (souls - p.souls)) / Math.max(q.souls - p.souls, 1);
  };
  const end = Math.min(a[a.length - 1]?.souls ?? 0, b[b.length - 1]?.souls ?? 0);
  const out: { souls: number; ahead: 0 | 1 }[] = [];
  let last: 0 | 1 | null = null;
  for (let souls = 1000; souls <= end; souls += 500) {
    const lead: 0 | 1 = at(a, souls) >= at(b, souls) ? 0 : 1;
    if (last !== null && lead !== last) out.push({ souls, ahead: lead });
    last = lead;
  }
  return out;
}

function Chart({ title, curves, value, names }: { title: string; curves: Curve[]; value: (p: PowerPoint) => number; names: string[] }) {
  const W = 420;
  const H = 180;
  const P = { l: 48, r: 10, t: 10, b: 24 };
  const all = curves.flatMap((c) => c.points);
  const maxX = Math.max(...all.map((p) => p.souls), 1);
  const maxY = Math.max(...all.map(value), 1);
  const x = (s: number) => P.l + ((W - P.l - P.r) * s) / maxX;
  const y = (v: number) => H - P.b - ((H - P.t - P.b) * v) / maxY;
  return (
    <figure className="power-chart">
      <figcaption>{title}</figcaption>
      <svg viewBox={`0 0 ${W} ${H}`} role="img" aria-label={title}>
        {[0, 0.5, 1].map((f) => (
          <g key={f}>
            <line x1={P.l} x2={W - P.r} y1={y(f * maxY)} y2={y(f * maxY)} className="grid" />
            <text x={P.l - 6} y={y(f * maxY) + 3} textAnchor="end" className="tick">
              {fmt(f * maxY, 0)}
            </text>
          </g>
        ))}
        {[0, 10000, 20000, 30000, 40000, 50000]
          .filter((s) => s <= maxX)
          .map((s) => (
            <text key={s} x={x(s)} y={H - 6} textAnchor="middle" className="tick">
              {s / 1000}k
            </text>
          ))}
        {curves.map((c, i) => (
          <g key={i}>
            <path d={c.points.map((p, j) => `${j ? "L" : "M"}${x(p.souls).toFixed(1)},${y(value(p)).toFixed(1)}`).join("")} fill="none" stroke={COLORS[i]} strokeWidth={2} />
            {c.points.map((p, j) => (
              <circle key={j} cx={x(p.souls)} cy={y(value(p))} r={3} fill={COLORS[i]}>
                <title>
                  {names[i]} · {p.item} · {t("{n} souls", { n: fmt(p.souls) })} · {fmt(value(p), 0)}
                </title>
              </circle>
            ))}
          </g>
        ))}
      </svg>
    </figure>
  );
}

/** Two builds side by side: how their fight damage and effective health grow with the souls spent, where one
 *  passes the other, and their numbers once complete. */
export default function BuildCompare({ current, heroes, items, onClose }: { current: Build; heroes: Hero[]; items: Map<string, Item>; onClose: () => void }) {
  const [saved, setSaved] = useState<string[]>([]);
  const [picks, setPicks] = useState<[string, string]>([CURRENT, ""]);
  const [builds, setBuilds] = useState<(Build | null)[]>([current, null]);
  const [curves, setCurves] = useState<(Curve | null)[]>([null, null]);
  const [error, setError] = useState<unknown>(null);

  useEffect(() => {
    api
      .listBuilds()
      .then((names) => {
        setSaved(names);
        if (names[0]) setPicks((p) => [p[0], p[1] || names[0]]);
      })
      .catch(setError);
  }, []);
  useEffect(() => {
    picks.forEach((name, i) => {
      const load = name === CURRENT ? Promise.resolve(current) : api.loadBuild(name);
      load.then((b) => setBuilds((all) => all.map((x, j) => (j === i ? b : x)))).catch(setError);
    });
  }, [picks, current]);
  useEffect(() => {
    let stale = false;
    setCurves([null, null]);
    builds.forEach((b, i) => {
      if (!b || b.purchases.length === 0) return;
      Promise.all([api.powerCurve(steps(b)), api.compute({ ...b, upto: null, souls: null })])
        .then(([rows, sheet]) => {
          if (stale) return;
          const points = rows.flatMap((r, j) => (r ? [{ ...r, item: items.get(b.purchases[j])?.name ?? b.purchases[j] }] : []));
          setCurves((all) => all.map((x, k) => (k === i ? { points, sheet } : x)));
        })
        .catch((e) => !stale && setError(e));
    });
    return () => {
      stale = true;
    };
  }, [builds, items]);

  const heroName = (b: Build | null) => heroes.find((h) => h.class_name === b?.hero)?.name ?? "";
  const names = picks.map((p, i) => `${p === CURRENT ? t("Current build") : p}${heroName(builds[i]) ? ` (${heroName(builds[i])})` : ""}`);
  const ready = curves[0] && curves[1] ? [curves[0], curves[1]] : null;
  const passes = useMemo(
    () => (ready ? { damage: crossings(ready[0].points, ready[1].points, (p) => p.fight_damage), health: crossings(ready[0].points, ready[1].points, (p) => p.effective_health) } : null),
    [ready],
  );
  const stats = useMemo(() => {
    if (!ready || !ready[0].sheet || !ready[1].sheet) return [];
    const other = new Map(ready[1].sheet.stats.map((s) => [s.key, s]));
    return ready[0].sheet.stats
      .flatMap((a) => {
        const b = other.get(a.key);
        if (!b || a.value === null || b.value === null) return [];
        const rel = Math.abs(a.value - b.value) / Math.max(Math.abs(a.value), Math.abs(b.value), 1e-9);
        return rel > 0.01 ? [{ label: a.label, unit: a.unit, a: a.value, b: b.value, rel }] : [];
      })
      .sort((x, y) => y.rel - x.rel)
      .slice(0, STATS_SHOWN);
  }, [ready]);
  const leadText = (list: { souls: number; ahead: 0 | 1 }[] | undefined) =>
    !list || list.length === 0
      ? t("No crossing: the same build leads all the way.")
      : list.map((c) => t("{name} ahead from {n} souls", { name: names[c.ahead], n: fmt(c.souls) })).join(" · ");
  const pick = (i: 0 | 1, name: string) => setPicks((p) => (i === 0 ? [name, p[1]] : [p[0], name]));

  return (
    <div className="overlay" onClick={onClose}>
      <div className="whats-new build-compare" role="dialog" aria-label={t("Compare builds")} onClick={(e) => e.stopPropagation()}>
        <div className="whats-new-head">
          <b>{t("Compare builds")}</b>
          <button className="ghost small" onClick={onClose}>
            {t("Close")}
          </button>
        </div>
        <div className="whats-new-body">
          <div className="fields">
            {([0, 1] as const).map((i) => (
              <label key={i} className="field">
                <span style={{ color: COLORS[i] }}>● {i === 0 ? t("First build") : t("Second build")}</span>
                <select value={picks[i]} onChange={(e) => pick(i, e.target.value)}>
                  <option value={CURRENT}>{t("Current build")}</option>
                  {saved.map((n) => (
                    <option key={n} value={n}>
                      {n}
                    </option>
                  ))}
                </select>
              </label>
            ))}
          </div>
          <ErrorNote error={error} />
          {saved.length === 0 && <p className="muted">{t("Save a build first: the current one is compared with a saved one, or two saved ones with each other.")}</p>}
          {builds.some((b) => b && b.purchases.length === 0) && <p className="muted">{t("A build without items has nothing to compare.")}</p>}
          {ready && passes && (
            <>
              <div className="power-charts">
                <Chart title={t("Fight damage")} curves={ready} value={(p) => p.fight_damage} names={names} />
                <Chart title={t("Effective health")} curves={ready} value={(p) => p.effective_health} names={names} />
              </div>
              <p>
                <b>{t("Fight damage")}:</b> {leadText(passes.damage)}
                <br />
                <b>{t("Effective health")}:</b> {leadText(passes.health)}
              </p>
              {stats.length > 0 && (
                <table className="match-table">
                  <thead>
                    <tr>
                      <th className="left">{t("Complete build")}</th>
                      <th style={{ color: COLORS[0] }}>{names[0]}</th>
                      <th style={{ color: COLORS[1] }}>{names[1]}</th>
                      <th>{t("Difference")}</th>
                    </tr>
                  </thead>
                  <tbody>
                    {stats.map((s) => (
                      <tr key={s.label}>
                        <td className="left">{s.label}</td>
                        <td className={s.a > s.b ? "good" : ""}>
                          {fmt(s.a, 1)}
                          {s.unit}
                        </td>
                        <td className={s.b > s.a ? "good" : ""}>
                          {fmt(s.b, 1)}
                          {s.unit}
                        </td>
                        <td className="muted">{percent(fmt(((s.b - s.a) / Math.max(Math.abs(s.a), 1e-9)) * 100, 0))}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
              <p className="muted">
                {t("Each dot is a purchase, at the souls spent by then. Fight damage is a team fight: every ability and active item once and six seconds of shooting. Effective health is max health over what the resists let through. Higher is not always better for every stat in the table: read cooldowns and reload times the other way.")}
              </p>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
