import { useState } from "react";
import { fmt } from "../api";

export const clock = (seconds: number) => `${Math.floor(seconds / 60)}:${String(Math.round(seconds % 60)).padStart(2, "0")}`;

export interface Series {
  key: string | number;
  name: string;
  color: string;
  values: number[];
  bold?: boolean;
}

export interface Marker {
  time: number;
  label: string;
  color: string;
}

interface LineChartProps {
  /** Seconds; one entry per value of every series. */
  times: number[];
  series: Series[];
  markers?: Marker[];
  height?: number;
  format?: (value: number) => string;
  /** Clicking a legend entry or line. */
  onPick?: (key: string | number) => void;
}

const W = 1000;
const PAD = { left: 62, right: 14, top: 12, bottom: 26 };

/** A plain SVG line chart: time on the x axis, one line per series, values under the cursor in the legend. */
export function LineChart({ times, series, markers = [], height = 260, format = (v) => fmt(v, 0), onPick }: LineChartProps) {
  const [hover, setHover] = useState<number | null>(null);
  const all = series.flatMap((s) => s.values);
  const min = Math.min(0, ...all);
  const max = Math.max(1, ...all);
  const end = Math.max(...times, 1);
  const x = (t: number) => PAD.left + (t / end) * (W - PAD.left - PAD.right);
  const y = (v: number) => PAD.top + (1 - (v - min) / (max - min)) * (height - PAD.top - PAD.bottom);
  const lines = [0, 1, 2, 3, 4].map((i) => min + ((max - min) * i) / 4);
  const minutes: number[] = [];
  for (let t = 0; t <= end; t += 300) minutes.push(t);
  const shown = hover ?? times.length - 1;
  const legend = [...series].sort((a, b) => (b.values[shown] ?? 0) - (a.values[shown] ?? 0));

  return (
    <div className="chart">
      <svg
        viewBox={`0 0 ${W} ${height}`}
        onMouseLeave={() => setHover(null)}
        onMouseMove={(e) => {
          const box = e.currentTarget.getBoundingClientRect();
          const px = ((e.clientX - box.left) / box.width) * W;
          let best = 0;
          times.forEach((t, i) => {
            if (Math.abs(x(t) - px) < Math.abs(x(times[best]) - px)) best = i;
          });
          setHover(best);
        }}
      >
        {lines.map((v) => (
          <g key={v}>
            <line className="chart-grid" x1={PAD.left} x2={W - PAD.right} y1={y(v)} y2={y(v)} />
            <text className="chart-label" x={PAD.left - 8} y={y(v) + 4} textAnchor="end">
              {format(v)}
            </text>
          </g>
        ))}
        {min < 0 && <line className="chart-zero" x1={PAD.left} x2={W - PAD.right} y1={y(0)} y2={y(0)} />}
        {minutes.map((t) => (
          <text key={t} className="chart-label" x={x(t)} y={height - 8} textAnchor="middle">
            {clock(t)}
          </text>
        ))}
        {markers.map((m, i) => (
          <g key={i}>
            <line x1={x(m.time)} x2={x(m.time)} y1={PAD.top} y2={height - PAD.bottom} stroke={m.color} strokeDasharray="3 4" opacity={0.7} />
            <circle cx={x(m.time)} cy={PAD.top + 4} r={4} fill={m.color}>
              <title>
                {clock(m.time)} · {m.label}
              </title>
            </circle>
          </g>
        ))}
        {series.map((s) => (
          <path
            key={s.key}
            d={s.values.map((v, i) => `${i === 0 ? "M" : "L"}${x(times[i]).toFixed(1)},${y(v).toFixed(1)}`).join(" ")}
            fill="none"
            stroke={s.color}
            strokeWidth={s.bold ? 3.5 : 1.6}
            opacity={series.some((o) => o.bold) && !s.bold ? 0.45 : 1}
            onClick={() => onPick?.(s.key)}
          />
        ))}
        {hover !== null && (
          <>
            <line className="chart-cursor" x1={x(times[hover])} x2={x(times[hover])} y1={PAD.top} y2={height - PAD.bottom} />
            {series.map((s) => (
              <circle key={s.key} cx={x(times[hover])} cy={y(s.values[hover] ?? 0)} r={s.bold ? 4.5 : 3} fill={s.color} />
            ))}
          </>
        )}
      </svg>
      <div className="chart-legend">
        <span className="muted">{clock(times[shown] ?? 0)}</span>
        {legend.map((s) => (
          <button key={s.key} className={"legend-entry" + (s.bold ? " on" : "")} onClick={() => onPick?.(s.key)}>
            <i style={{ background: s.color }} />
            {s.name} <b>{format(s.values[shown] ?? 0)}</b>
          </button>
        ))}
      </div>
    </div>
  );
}

interface BarRow {
  key: string | number;
  name: string;
  amount: number;
  color?: string;
  bold?: boolean;
}

/** Horizontal bars, largest first, with each row's share of the total. */
export function Bars({ rows, onPick, share = true }: { rows: BarRow[]; onPick?: (key: string | number) => void; share?: boolean }) {
  const sorted = [...rows].sort((a, b) => b.amount - a.amount);
  const top = Math.max(...sorted.map((r) => r.amount), 1);
  const total = sorted.reduce((a, r) => a + r.amount, 0);
  return (
    <div>
      {sorted.map((r) => (
        <div key={r.key} className={"source" + (r.bold ? " on" : "") + (onPick ? " pick" : "")} onClick={() => onPick?.(r.key)}>
          <div className="source-head">
            <span>{r.name}</span>
            <b>
              {fmt(r.amount, 0)} {share && <small>{fmt((r.amount / Math.max(total, 1)) * 100, 1)}%</small>}
            </b>
          </div>
          <div className="source-bar">
            <div style={{ width: `${(r.amount / top) * 100}%`, background: r.color }} />
          </div>
        </div>
      ))}
    </div>
  );
}

/** Marks on a time axis: kills above the line, deaths below it. */
export function FightStrip({
  duration,
  kills,
  deaths,
  revives = [],
}: {
  duration: number;
  kills: { time: number; label: string }[];
  deaths: { time: number; label: string }[];
  /** Times a revive ability brought the player back. */
  revives?: number[];
}) {
  const height = 64;
  const x = (t: number) => PAD.left + (t / Math.max(duration, 1)) * (W - PAD.left - PAD.right);
  const minutes: number[] = [];
  for (let t = 0; t <= duration; t += 300) minutes.push(t);
  return (
    <div className="chart">
      <svg viewBox={`0 0 ${W} ${height}`}>
        <line className="chart-zero" x1={PAD.left} x2={W - PAD.right} y1={30} y2={30} />
        <text className="chart-label" x={PAD.left - 8} y={22} textAnchor="end">
          kills
        </text>
        <text className="chart-label" x={PAD.left - 8} y={44} textAnchor="end">
          deaths
        </text>
        {minutes.map((t) => (
          <text key={t} className="chart-label" x={x(t)} y={height - 4} textAnchor="middle">
            {clock(t)}
          </text>
        ))}
        {kills.map((k, i) => (
          <rect key={"k" + i} className="fight-kill" x={x(k.time) - 2.5} y={10} width={5} height={18} rx={2}>
            <title>
              {clock(k.time)} · killed {k.label}
            </title>
          </rect>
        ))}
        {revives.map((t, i) => (
          <rect key={"r" + i} className="fight-revive" x={x(t) - 2.5} y={32} width={5} height={18} rx={2}>
            <title>{clock(t)} · went down and revived</title>
          </rect>
        ))}
        {deaths.map((d, i) => (
          <rect key={"d" + i} className="fight-death" x={x(d.time) - 2.5} y={32} width={5} height={18} rx={2}>
            <title>
              {clock(d.time)} · killed by {d.label}
            </title>
          </rect>
        ))}
      </svg>
    </div>
  );
}
