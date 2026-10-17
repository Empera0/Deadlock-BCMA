import { useEffect, useMemo, useState } from "react";
import { api, Build, EMPTY_MAP, fmt, MasterBuild, PowerPoint } from "../api";

/** Ability upgrade tiers the master builder assumes in each phase. */
const PHASE_TIERS = [
  [0, 0, 0, 0],
  [1, 1, 1, 0],
  [2, 2, 1, 1],
  [3, 3, 3, 3],
];
const STEP = 1000;
/** A step that raises the fight damage by this share or more is a power spike. */
const SPIKE = 0.15;

interface Point extends PowerPoint {
  bought: string[];
  spike: boolean;
}

/** The master build at each step of souls: what is bought by then, less what was sold before. */
function buildsAlong(master: MasterBuild): { souls: number; build: Build; bought: string[] }[] {
  const end = master.phases[master.phases.length - 1]?.souls_to ?? 40000;
  const out: { souls: number; build: Build; bought: string[] }[] = [];
  let prev: string[] = [];
  for (let souls = STEP; souls <= end; souls += STEP) {
    const phase = Math.max(0, master.phases.findIndex((p) => souls <= p.souls_to));
    const sold = new Set(master.phases.filter((p) => p.souls_from < souls).flatMap((p) => p.sold));
    let spent = 0;
    const owned = master.items.filter((x) => (spent += x.paid) <= souls && !sold.has(x.name));
    const purchases = owned.map((x) => x.item);
    out.push({
      souls,
      bought: owned.filter((x) => !prev.includes(x.item)).map((x) => x.name),
      build: {
        hero: master.hero,
        purchases,
        upto: null,
        souls,
        active: [],
        ability_tiers: PHASE_TIERS[phase] ?? [0, 0, 0, 0],
        imbues: Object.fromEntries(Object.entries(master.imbues).filter(([k]) => purchases.includes(k))),
        copied_ult: null,
        target: { health: 2000, bullet_resist: 0, spirit_resist: 0 },
        map: EMPTY_MAP,
      },
    });
    prev = purchases;
  }
  return out;
}

const W = 340;
const H = 150;
const PAD = { left: 44, right: 10, top: 10, bottom: 24 };

/** One measure along the souls: a single line, the spikes marked, a crosshair with the step's numbers. */
function Chart({
  title,
  points,
  value,
  color,
  hover,
  onHover,
  spikes,
}: {
  title: string;
  points: Point[];
  value: (p: Point) => number;
  color: string;
  hover: number | null;
  onHover: (i: number | null) => void;
  spikes?: boolean;
}) {
  const max = Math.max(...points.map(value), 1);
  const maxSouls = points[points.length - 1]?.souls ?? 1;
  const x = (souls: number) => PAD.left + ((W - PAD.left - PAD.right) * souls) / maxSouls;
  const y = (v: number) => H - PAD.bottom - ((H - PAD.top - PAD.bottom) * v) / max;
  const path = points.map((p, i) => `${i ? "L" : "M"}${x(p.souls).toFixed(1)},${y(value(p)).toFixed(1)}`).join("");
  const ticks = [0, 0.5, 1].map((f) => f * max);
  const h = hover !== null ? points[hover] : null;
  return (
    <figure className="power-chart">
      <figcaption>{title}</figcaption>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        role="img"
        aria-label={`${title} by souls`}
        onMouseLeave={() => onHover(null)}
        onMouseMove={(e) => {
          const r = e.currentTarget.getBoundingClientRect();
          const souls = (((e.clientX - r.left) / r.width) * W - PAD.left) / (W - PAD.left - PAD.right) * maxSouls;
          const i = Math.round(souls / STEP) - 1;
          onHover(Math.max(0, Math.min(points.length - 1, i)));
        }}
      >
        {ticks.map((t) => (
          <g key={t}>
            <line x1={PAD.left} x2={W - PAD.right} y1={y(t)} y2={y(t)} className="grid" />
            <text x={PAD.left - 6} y={y(t) + 3} textAnchor="end" className="tick">
              {fmt(t, 0)}
            </text>
          </g>
        ))}
        {[0, 10000, 20000, 30000, 40000]
          .filter((s) => s <= maxSouls)
          .map((s) => (
            <text key={s} x={x(s)} y={H - 6} textAnchor="middle" className="tick">
              {s / 1000}k
            </text>
          ))}
        <path d={path} fill="none" stroke={color} strokeWidth={2} strokeLinejoin="round" />
        {points.map((p, i) =>
          spikes && p.spike ? <circle key={i} cx={x(p.souls)} cy={y(value(p))} r={4} fill={color} stroke="var(--panel)" strokeWidth={2} /> : null,
        )}
        {h && (
          <>
            <line x1={x(h.souls)} x2={x(h.souls)} y1={PAD.top} y2={H - PAD.bottom} className="crosshair" />
            <circle cx={x(h.souls)} cy={y(value(h))} r={4} fill={color} stroke="var(--panel)" strokeWidth={2} />
          </>
        )}
        <rect x={PAD.left} y={PAD.top} width={W - PAD.left - PAD.right} height={H - PAD.top - PAD.bottom} fill="transparent" />
      </svg>
    </figure>
  );
}

/** How the master build's fight damage and effective health grow with souls, with its power spikes. */
export default function PowerCurve({ master }: { master: MasterBuild }) {
  const steps = useMemo(() => buildsAlong(master), [master]);
  const [points, setPoints] = useState<Point[] | null>(null);
  const [hover, setHover] = useState<number | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    let stale = false;
    setPoints(null);
    api
      .powerCurve(steps.map((s) => s.build))
      .then((rows) => {
        if (stale) return;
        const out: Point[] = [];
        rows.forEach((r, i) => r && out.push({ ...r, souls: steps[i].souls, bought: steps[i].bought, spike: false }));
        // A spike is a step that raises the fight damage by 15% or more and by over twice the usual step.
        const rises = out.slice(1).map((p, i) => p.fight_damage - out[i].fight_damage);
        const usual = [...rises].sort((a, b) => a - b)[Math.floor(rises.length / 2)] ?? 0;
        out.forEach((p, i) => {
          const rise = i > 0 ? p.fight_damage - out[i - 1].fight_damage : 0;
          p.spike = i > 0 && rise >= out[i - 1].fight_damage * SPIKE && rise > 2 * usual;
        });
        setPoints(out);
      })
      .catch((e) => !stale && setError(String(e)));
    return () => {
      stale = true;
    };
  }, [steps]);

  if (error) return <div className="error">{error}</div>;
  if (!points) return <p className="muted">Working out the power curve…</p>;
  const h = hover !== null ? points[hover] : null;
  const spikes = points.filter((p) => p.spike);
  return (
    <div className="group power-curve">
      <h3>
        Power curve <small className="muted">· every {fmt(STEP)} souls · dots mark spikes: +{SPIKE * 100}% fight damage or more in one step</small>
      </h3>
      <div className="power-charts">
        <Chart title="Fight damage" points={points} value={(p) => p.fight_damage} color="var(--accent)" hover={hover} onHover={setHover} spikes />
        <Chart title="Effective health" points={points} value={(p) => p.effective_health} color="var(--vitality)" hover={hover} onHover={setHover} />
      </div>
      <div className="power-readout">
        {h ? (
          <>
            <b>{fmt(h.souls)} souls</b> · {fmt(h.fight_damage, 0)} fight damage · {fmt(h.effective_health, 0)} effective health
            {h.bought.length > 0 && <span className="muted"> · bought: {h.bought.join(", ")}</span>}
          </>
        ) : (
          <span className="muted">
            {spikes.length
              ? `Spikes at ${spikes.map((s) => `${fmt(s.souls)} (${s.bought.join(", ") || "level"})`).join(" · ")}`
              : "No step stands out: the build grows evenly."}
          </span>
        )}
      </div>
      <p className="muted">
        Fight damage is every ability and active item once and six seconds of shooting, against a target without resists; effective health is max
        health over what the resists let through.
      </p>
    </div>
  );
}
