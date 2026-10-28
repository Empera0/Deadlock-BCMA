import { useState } from "react";
import { fmt, MatchPlayer, MatchReport, MatchSample } from "../api";
import { clock, LineChart } from "./MatchCharts";
import { engineText } from "../engineText";
import { LOCALE, t } from "../i18n";

interface Metric {
  key: string;
  label: string;
  value: (s: MatchSample) => number;
}

interface Props {
  report: MatchReport;
  metrics: Metric[];
  /** The two players shown first. */
  first: number;
  second: number;
}

interface Row {
  label: string;
  a: number;
  b: number;
  /** False when the smaller number is the better one. */
  more?: boolean;
  format?: (v: number) => string;
  note?: string;
}

const COLORS = ["#f2b84b", "#4fc3f7"];

function Rows({ title, rows }: { title: string; rows: Row[] }) {
  const shown = rows.filter((r) => r.a !== 0 || r.b !== 0);
  if (shown.length === 0) return null;
  return (
    <div className="group">
      <h3>{title}</h3>
      {shown.map((r) => {
        const top = Math.max(Math.abs(r.a), Math.abs(r.b), 1e-9);
        const format = r.format ?? ((v: number) => fmt(v, 0));
        const better = r.a === r.b ? -1 : (r.a > r.b) === (r.more ?? true) ? 0 : 1;
        return (
          <div key={r.label} className="versus-row" title={r.note}>
            <b className={better === 0 ? "lead" : ""} style={better === 0 ? { color: COLORS[0] } : undefined}>
              {format(r.a)}
            </b>
            <span className="versus-bar left">
              <i style={{ width: `${(Math.abs(r.a) / top) * 100}%`, background: COLORS[0], opacity: better === 0 ? 1 : 0.4 }} />
            </span>
            <span className="versus-label">{r.label}</span>
            <span className="versus-bar">
              <i style={{ width: `${(Math.abs(r.b) / top) * 100}%`, background: COLORS[1], opacity: better === 1 ? 1 : 0.4 }} />
            </span>
            <b className={better === 1 ? "lead" : ""} style={better === 1 ? { color: COLORS[1] } : undefined}>
              {format(r.b)}
            </b>
          </div>
        );
      })}
    </div>
  );
}

function Picker({ report, value, onChange, color }: { report: MatchReport; value: number; onChange: (slot: number) => void; color: string }) {
  return (
    <div className="player-picker">
      {report.players.map((p) => (
        <button key={p.slot} className={p.slot === value ? "on" : ""} onClick={() => onChange(p.slot)} title={p.hero_name}>
          <img src={p.hero_icon} alt={p.hero_name} style={{ borderColor: p.slot === value ? color : "transparent" }} />
        </button>
      ))}
    </div>
  );
}

const pct = (part: number, whole: number) => (whole > 0 ? (part / whole) * 100 : 0);
const percent = (v: number) => `${fmt(v, 1)}%`;

export default function MatchCompare({ report, metrics, first, second }: Props) {
  const [slotA, setSlotA] = useState(first);
  const [slotB, setSlotB] = useState(second);
  const [metricKey, setMetricKey] = useState("souls");
  const a = report.players.find((p) => p.slot === slotA) ?? report.players[0];
  const b = report.players.find((p) => p.slot === slotB) ?? report.players[1] ?? report.players[0];
  const rankA = report.ranking.find((r) => r.slot === a.slot);
  const rankB = report.ranking.find((r) => r.slot === b.slot);
  const minutes = Math.max(report.duration_s / 60, 1);
  const metric = metrics.find((m) => m.key === metricKey) ?? metrics[0];
  const named = (list: { name: string; amount: number }[], name: string) => list.find((s) => s.name === name)?.amount ?? 0;
  const union = (x: { name: string }[], y: { name: string }[]) => [...new Set([...x, ...y].map((s) => s.name))];
  const points = (rankA?.parts ?? []).map((part, i) => ({
    label: part.label,
    a: part.points,
    b: rankB?.parts[i]?.points ?? 0,
    valueA: part.value,
    valueB: rankB?.parts[i]?.value ?? 0,
    maxA: part.max_points,
    maxB: rankB?.parts[i]?.max_points ?? 0,
  }));
  const gaps = points.map((p) => ({ ...p, gap: p.a - p.b })).sort((x, y) => Math.abs(y.gap) - Math.abs(x.gap));
  const lead = (rankA?.score ?? 0) - (rankB?.score ?? 0);
  const ahead = lead >= 0 ? a : b;
  const behind = lead >= 0 ? b : a;
  const sign = lead >= 0 ? 1 : -1;
  const reasons = (who: number) =>
    gaps
      .filter((g) => g.gap * sign * who > 0.3)
      .slice(0, 4)
      .map((g) => `${g.label.toLocaleLowerCase(LOCALE)} (+${fmt(Math.abs(g.gap), 1)})`)
      .join(", ");

  const side = (p: MatchPlayer, color: string) => (
    <div className="versus-head" style={{ borderColor: color }}>
      <img src={p.hero_icon} alt="" />
      <div>
        <strong style={{ color }}>
          {p.hero_name}{" "}
          <small className="role-tag" title={t("Scored as: {role} ({why})", { role: p.role.toLocaleLowerCase(LOCALE), why: p.role_why })}>
            {p.role}
          </small>
        </strong>
        {p.player_name && <small className="player-name">{p.player_name}</small>}
        <small>
          {p.kills}/{p.deaths}/{p.assists} · {t("{n} souls", { n: fmt(p.net_worth) })} ·{" "}
          <b className={`place-text place-${Math.min(report.ranking.findIndex((r) => r.slot === p.slot) + 1, 4)}`}>
            {t("#{n} in the ranking", { n: report.ranking.findIndex((r) => r.slot === p.slot) + 1 })}
          </b>
        </small>
      </div>
    </div>
  );
  const purchases = (p: MatchPlayer) => (
    <div className="match-items">
      {p.purchases
        .filter((x) => x.sold_at === null)
        .map((x, i) => (
          <div key={i} className={"match-item " + x.category} title={t("{n} souls", { n: fmt(x.cost) })}>
            <img src={x.icon} alt="" loading="lazy" />
            <span>{x.name}</span>
            <small>{clock(x.time)}</small>
          </div>
        ))}
    </div>
  );

  return (
    <div className="versus">
      <div className="match-columns">
        <div>
          <Picker report={report} value={a.slot} onChange={setSlotA} color={COLORS[0]} />
          {side(a, COLORS[0])}
        </div>
        <div>
          <Picker report={report} value={b.slot} onChange={setSlotB} color={COLORS[1]} />
          {side(b, COLORS[1])}
        </div>
      </div>

      {a.slot !== b.slot && (
        <div className="insight">
          <strong>
            {t("{a} scores {n} points more than {b} ({x} against {y})", {
              a: ahead.hero_name,
              n: fmt(Math.abs(lead), 1),
              b: behind.hero_name,
              x: fmt(Math.max(rankA?.score ?? 0, rankB?.score ?? 0), 1),
              y: fmt(Math.min(rankA?.score ?? 0, rankB?.score ?? 0), 1),
            })}
          </strong>
          <span>
            {t("{hero} gains most from: {what}.", { hero: ahead.hero_name, what: reasons(1) || t("nothing in particular") })}
          </span>
          <span>
            {t("{hero} is ahead in: {what}.", { hero: behind.hero_name, what: reasons(-1) || t("nothing") })}
          </span>
        </div>
      )}

      <Rows
        title={t("Score, part by part (points)")}
        rows={[
          { label: t("Total score"), a: rankA?.score ?? 0, b: rankB?.score ?? 0, format: (v) => fmt(v, 1) },
          ...points.map((p) => ({
            label:
              Math.abs(p.maxA - p.maxB) < 0.05
                ? `${p.label} (${t("max {n}", { n: fmt(p.maxA, 1) })})`
                : `${p.label} (${t("max {n}", { n: `${fmt(p.maxA, 1)} / ${fmt(p.maxB, 1)}` })})`,
            a: p.a,
            b: p.b,
            format: (v: number) => fmt(v, 1),
            note: `${a.hero_name}: ${fmt(p.valueA, 0)} · ${b.hero_name}: ${fmt(p.valueB, 0)}`,
          })),
        ]}
      />

      <Rows
        title={t("Fighting")}
        rows={[
          { label: t("Kills"), a: a.kills, b: b.kills },
          { label: t("Assists"), a: a.assists, b: b.assists },
          { label: t("Deaths"), a: a.deaths, b: b.deaths, more: false },
          { label: t("Went down but revived"), a: a.revives_at.length, b: b.revives_at.length, more: false },
          { label: t("Close calls (10% health or less, survived)"), a: a.close_calls, b: b.close_calls, more: false },
          { label: t("Team fights joined"), a: a.fights_joined, b: b.fights_joined },
          { label: t("Team fights joined %"), a: pct(a.fights_joined, a.fights_possible), b: pct(b.fights_joined, b.fights_possible), format: percent },
          { label: t("Fights left to the team"), a: a.fights_missed, b: b.fights_missed, more: false },
          { label: t("Late-game fights skipped"), a: a.late_fights_skipped, b: b.late_fights_skipped, more: false },
          { label: t("Team fights won when there"), a: a.fights_won, b: b.fights_won },
          { label: t("Team fights lost when there"), a: a.fights_lost, b: b.fights_lost, more: false },
          { label: t("Died first in a team fight"), a: a.first_deaths, b: b.first_deaths, more: false },
          { label: t("Lane: damage dealt to the lane opponents"), a: a.lane_damage_dealt, b: b.lane_damage_dealt },
          { label: t("Lane: damage taken from the lane opponents"), a: a.lane_damage_taken, b: b.lane_damage_taken, more: false },
          { label: t("Won a fight, then farmed"), a: a.wins_farmed, b: b.wins_farmed, more: false },
          { label: t("Caught alone outside a fight"), a: a.caught_deaths, b: b.caught_deaths, more: false },
          { label: t("Deaths that left the team short for a fight"), a: a.costly_deaths, b: b.costly_deaths, more: false },
          { label: t("Hero damage"), a: a.hero_damage, b: b.hero_damage },
          { label: t("Hero damage per minute"), a: a.hero_damage / minutes, b: b.hero_damage / minutes },
          { label: t("Damage enabled for teammates (estimate)"), a: a.enabled_total, b: b.enabled_total },
          { label: t("Objective damage"), a: a.objective_damage, b: b.objective_damage },
          { label: t("Objectives taken (there when they fell)"), a: a.objectives_taken.length, b: b.objectives_taken.length },
          { label: t("Damage taken"), a: a.damage_taken, b: b.damage_taken },
          { label: t("Damage mitigated"), a: a.mitigated, b: b.mitigated },
          { label: t("Self healing"), a: a.self_healing, b: b.self_healing },
          { label: t("Healing and barriers for teammates"), a: a.teammate_support, b: b.teammate_support },
          { label: t("Accuracy vs heroes"), a: pct(a.hero_hits, a.hero_shots), b: pct(b.hero_hits, b.hero_shots), format: percent },
          { label: t("Headshot share"), a: pct(a.hero_headshots, a.hero_hits), b: pct(b.hero_headshots, b.hero_hits), format: percent },
        ]}
      />

      <Rows
        title={t("Souls")}
        rows={[
          { label: t("Souls"), a: a.net_worth, b: b.net_worth },
          { label: t("Souls per minute"), a: a.net_worth / minutes, b: b.net_worth / minutes },
          ...union(a.souls, b.souls).map((name) => ({ label: t("From {source}", { source: name.toLocaleLowerCase(LOCALE) }), a: named(a.souls, name), b: named(b.souls, name) })),
          { label: t("Trooper damage"), a: a.creep_damage, b: b.creep_damage },
          { label: t("Neutral camp damage"), a: a.neutral_damage, b: b.neutral_damage },
        ]}
      />

      <Rows
        title={t("Macro habits")}
        rows={[
          { label: t("Souls at the end of laning ({time})", { time: clock(a.lane_time) }), a: a.lane_souls, b: b.lane_souls },
          { label: t("Souls ahead of the lane opponent"), a: a.lane_souls_diff, b: b.lane_souls_diff, format: (v) => `${v > 0 ? "+" : ""}${fmt(v, 0)}` },
          { label: t("Laning last hits"), a: a.lane_last_hits, b: b.lane_last_hits },
          { label: t("Laning denies"), a: a.lane_denies, b: b.lane_denies },
          { label: t("Trooper last hits taken"), a: a.last_hit_pct, b: b.last_hit_pct, format: percent },
          { label: t("Average unspent souls"), a: a.avg_unspent_souls, b: b.avg_unspent_souls, more: false },
          { label: t("Average unspent ability points"), a: a.avg_unspent_ap, b: b.avg_unspent_ap, more: false, format: (v) => fmt(v, 1) },
          { label: t("Time dead (seconds)"), a: a.dead_seconds, b: b.dead_seconds, more: false },
          { label: t("Souls lost on death"), a: a.souls_lost_on_death, b: b.souls_lost_on_death, more: false },
          ...a.time_split
            .filter((part) => part.name !== engineText("Dead"))
            .map((part, i) => ({
              label: t("{name} (share of the match)", { name: part.name }),
              a: pct(part.amount, report.duration_s),
              b: pct(b.time_split[i]?.amount ?? 0, report.duration_s),
              format: percent,
              more: part.name !== engineText("Out of combat"),
            })),
        ]}
      />

      <Rows
        title={t("Map play")}
        rows={union(a.macro_stats, b.macro_stats).map((name) => ({
          label: name,
          a: named(a.macro_stats, name),
          b: named(b.macro_stats, name),
          more: name !== engineText("Parries missed"),
        }))}
      />

      <div className="group">
        <h3>{t("Over time")}</h3>
        <label className="field inline">
          {t("Show")}
          <select value={metric.key} onChange={(e) => setMetricKey(e.target.value)}>
            {metrics.map((m) => (
              <option key={m.key} value={m.key}>
                {m.label}
              </option>
            ))}
          </select>
        </label>
        <LineChart
          times={[0, ...a.timeline.map((s) => s.time)]}
          height={240}
          series={[
            { key: "a", name: a.hero_name, color: COLORS[0], values: [0, ...a.timeline.map(metric.value)] },
            { key: "b", name: b.hero_name, color: COLORS[1], values: [0, ...b.timeline.map(metric.value)] },
          ]}
        />
      </div>

      <div className="match-columns">
        <div className="group">
          <h3 style={{ color: COLORS[0] }}>{t("{hero}: final items", { hero: a.hero_name })}</h3>
          {purchases(a)}
          {a.insights.map((i, n) => (
            <div key={n} className={"insight " + i.tone}>
              <strong>{i.title}</strong>
              <span>{i.text}</span>
            </div>
          ))}
        </div>
        <div className="group">
          <h3 style={{ color: COLORS[1] }}>{t("{hero}: final items", { hero: b.hero_name })}</h3>
          {purchases(b)}
          {b.insights.map((i, n) => (
            <div key={n} className={"insight " + i.tone}>
              <strong>{i.title}</strong>
              <span>{i.text}</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
