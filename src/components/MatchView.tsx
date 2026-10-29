import { MouseEvent, useEffect, useMemo, useState } from "react";
import { api, Build, DeathTrend, FightSide, fmt, HistoryEntry, LocalScan, MatchPlayer, MatchPreset, MatchReport, MatchSample, MatchSummary } from "../api";
import { MatchDuels, MatchLanes, MatchMap, MatchTrend } from "./MatchExtras";
import { Bars, clock, FightStrip, LineChart, Marker, Series } from "./MatchCharts";
import MatchCompare from "./MatchCompare";
import DeathAnalysisPanel, { DeathTrendView } from "./DeathAnalysisPanel";
import ItemTimingPanel from "./ItemTimingPanel";
import ReplayPanel from "./ReplayPanel";
import { readSetting, SETTINGS, useSetting } from "../settings";
import ErrorNote from "./ErrorNote";
import { dateTime, date, t, tn } from "../i18n";
import { LOCALE } from "../i18n";

interface Props {
  /** Opens a build from the match in the builder. */
  onOpenBuild: (build: Build, target?: Build["target"]) => void;
  /** Opens a player of the match in the master builder, teams and purchases included. */
  onOpenMaster: (preset: MatchPreset) => void;
}

/** A player of the match as the master builder takes it: both teams, the lane, and every purchase with the
 * player's net worth at the time (read between the match's snapshots). */
function masterPreset(report: MatchReport, player: MatchPlayer): MatchPreset {
  const points = [{ time: 0, net_worth: 0 }, ...player.timeline.map((s) => ({ time: s.time, net_worth: s.net_worth }))];
  const worthAt = (t: number) => {
    const after = points.findIndex((p) => p.time >= t);
    if (after <= 0) return after === 0 ? points[0].net_worth : points[points.length - 1].net_worth;
    const a = points[after - 1];
    const b = points[after];
    return a.net_worth + ((b.net_worth - a.net_worth) * (t - a.time)) / Math.max(b.time - a.time, 1);
  };
  const hero = (slot: number) => report.players.find((o) => o.slot === slot)?.hero;
  return {
    key: `${report.match_id}-${player.slot}`,
    match_id: report.match_id,
    player_name: player.player_name || player.hero_name,
    hero: player.hero,
    allies: report.players.filter((o) => o.team === player.team && o.slot !== player.slot).map((o) => o.hero),
    enemies: report.players.filter((o) => o.team !== player.team).map((o) => o.hero),
    lane: player.lane_opponents.map(hero).filter((h): h is string => !!h),
    net_worth: player.net_worth,
    won: player.won,
    bought: player.purchases.map((p) => ({ item: p.item, net_worth: Math.round(worthAt(p.time)), time: p.time })),
  };
}

const TEAM_NAME = ["Hidden King", "Archmother"];
const TEAM_COLOR = ["#f2b84b", "#4fc3f7"];
/** One color per player: warm for the first team, cool for the second. */
const PLAYER_COLORS = [
  ["#f2b84b", "#e8873a", "#e2574c", "#f08fb0", "#c9c227", "#b98a5a"],
  ["#4fc3f7", "#6f7fe0", "#26a69a", "#a184e0", "#81c784", "#9fb4c4"],
];
const PART_COLORS = [
  "#e2574c", "#f2b84b", "#f08fb0", "#c9c227", "#4fc3f7", "#9fb4c4", "#81c784", "#26a69a",
  "#5ce9a9", "#d98ad0", "#b98a5a", "#7fd6c2", "#e8873a", "#6f7fe0", "#a184e0", "#c3cad3", "#f6e58d", "#8fd14f",
  "#ff9aa2", "#5d8aa8", "#d4a373",
];

/** The heroes of one team in a fight: there, away (and why), or dead when it started. */
function FightTeam({ side, players }: { side: FightSide; players: Map<number, MatchPlayer> }) {
  const icon = (slot: number, className: string, title: string) => {
    const p = players.get(slot);
    return p ? <img key={slot} className={className} src={p.hero_icon} alt={p.hero_name} title={`${p.hero_name}: ${title}`} loading="lazy" /> : null;
  };
  return (
    <span className="fight-team">
      {side.present.map((slot) => icon(slot, side.deaths.includes(slot) ? "died" : "", side.deaths.includes(slot) ? t("in the fight, died") : t("in the fight")))}
      {side.absent.map((a) => icon(a.slot, a.missed ? "away missed" : "away", `${t("{doing}, {n} m away", { doing: a.doing, n: fmt(a.distance, 0) })}${a.missed ? ` — ${t("the team needed the numbers")}` : ""}`))}
      {side.dead.map((slot) => icon(slot, "dead", t("dead when the fight started")))}
    </span>
  );
}

interface Metric {
  key: string;
  label: string;
  value: (s: MatchSample) => number;
}

const METRICS: Metric[] = [
  { key: "souls", label: t("Souls"), value: (s) => s.net_worth },
  { key: "hero_damage", label: t("Hero damage"), value: (s) => s.hero_damage },
  { key: "objective_damage", label: t("Objective damage"), value: (s) => s.objective_damage },
  { key: "creep_damage", label: t("Trooper damage"), value: (s) => s.creep_damage },
  { key: "neutral_damage", label: t("Neutral camp damage"), value: (s) => s.neutral_damage },
  { key: "damage_taken", label: t("Damage taken"), value: (s) => s.damage_taken },
  { key: "mitigated", label: t("Damage mitigated"), value: (s) => s.mitigated },
  { key: "healing", label: t("Healing"), value: (s) => s.healing },
  { key: "kills", label: t("Kills"), value: (s) => s.kills },
  { key: "deaths", label: t("Deaths"), value: (s) => s.deaths },
  { key: "assists", label: t("Assists"), value: (s) => s.assists },
  { key: "last_hits", label: t("Last hits"), value: (s) => s.last_hits },
  { key: "denies", label: t("Denies"), value: (s) => s.denies },
];

const SPLIT_COLORS = ["#e2574c", "#f2b84b", "#26a69a", "#6b6f7a", "#2b2b2b"];
const signed = (v: number) => `${v > 0 ? "+" : ""}${fmt(v, 0)}`;

/** One bar showing how a player's match time was spent. */
/** Buttons that jump to the headings of whatever the match screen is showing. */
function JumpBar({ watch }: { watch: string }) {
  const [titles, setTitles] = useState<string[]>([]);
  const headings = () => [...document.querySelectorAll<HTMLElement>(".match-main .group > h3")];
  useEffect(() => {
    // The heading's own text, without the counters some headings carry.
    const found = headings().map((h) => (h.childNodes[0]?.textContent ?? h.textContent ?? "").trim());
    setTitles((before) => (before.join("|") === found.join("|") ? before : found));
  }, [watch]);
  if (titles.length < 3) return null;
  return (
    <div className="jump-bar">
      {titles.map((title, i) => (
        <button key={i + title} onClick={() => headings()[i]?.parentElement?.scrollIntoView({ block: "start", behavior: "smooth" })}>
          {title}
        </button>
      ))}
    </div>
  );
}

/** Clicks the match screen handles in one place: a heading shows or hides its explanation, a table row its full text. */
function toggleDetails(e: MouseEvent<HTMLElement>) {
  const target = e.target as HTMLElement;
  if (target.closest("button, a, input, select")) return;
  const heading = target.closest(".group > h3");
  if (heading?.nextElementSibling?.classList.contains("muted")) {
    heading.parentElement?.classList.toggle("help-open");
    return;
  }
  target.closest(".match-table.timing tbody tr")?.classList.toggle("open");
}

/** The titles a player earned in the match, as small plates. */
function Titles({ player }: { player: MatchPlayer }) {
  if (player.titles.length === 0) return null;
  return (
    <span className="titles">
      {player.titles.map((t) => (
        <i key={t.name} className={"title " + t.tone} title={`${t.what}: ${t.value}`}>
          {t.name}
        </i>
      ))}
    </span>
  );
}

function TimeSplit({ player, duration }: { player: MatchPlayer; duration: number }) {
  return (
    <span className="time-split" title={player.time_split.map((t) => `${t.name}: ${clock(t.amount)}`).join(" · ")}>
      {player.time_split.map((t, i) => (
        <i key={t.name} style={{ width: `${(t.amount / Math.max(duration, 1)) * 100}%`, background: SPLIT_COLORS[i] }} />
      ))}
    </span>
  );
}

/** Deaths, with the times a revive undid one shown next to them. */
const deathsText = (p: MatchPlayer) => (p.revives_at.length > 0 ? t("{n} (+{m} revived)", { n: p.deaths, m: p.revives_at.length }) : String(p.deaths));

/** Difference between the calculator and the game, as a class for coloring. */
function gap(game: number, calc: number | null): string {
  if (calc === null) return "";
  const off = Math.abs(calc - game) / Math.max(Math.abs(game), 1);
  return off < 0.01 || Math.abs(calc - game) < 1.5 ? "match" : off < 0.05 ? "near" : "off";
}

function PlayerDetail({
  report,
  player,
  color,
  names,
  onOpenBuild,
  onOpenMaster,
}: {
  report: MatchReport;
  player: MatchPlayer;
  color: string;
  names: Map<number, string>;
  onOpenBuild: (build: Build, target?: Build["target"]) => void;
  onOpenMaster: Props["onOpenMaster"];
}) {
  const accuracy = player.hero_shots > 0 ? (player.hero_hits / player.hero_shots) * 100 : null;
  const headshots = player.hero_hits > 0 ? (player.hero_headshots / player.hero_hits) * 100 : null;
  const minutes = Math.max(report.duration_s / 60, 1);
  const times = [0, ...player.timeline.map((s) => s.time)];
  const missed = report.fights.flatMap((f) => {
    const side = f.sides[player.team];
    const away = side?.absent.find((a) => a.slot === player.slot && a.missed);
    const other = f.sides[1 - player.team];
    return away && other
      ? [{ start: f.start, late: f.late, even: f.even, walkers: f.walkers, teamSouls: f.team_souls, own: side.present.length, enemy: other.present.length, lost: side.deaths.length, killed: other.deaths.length, ...away }]
      : [];
  });
  const soulNames = player.souls.map((s) => s.name);
  const soulSeries: Series[] = soulNames.map((name, i) => ({
    key: name,
    name,
    color: PART_COLORS[i % PART_COLORS.length],
    values: [0, ...player.timeline.map((s) => s.souls.find((x) => x.name === name)?.amount ?? 0)],
  }));
  return (
    <div className="match-detail">
      <div className="match-detail-head">
        <img src={player.hero_icon} alt="" />
        <div>
          <strong style={{ color }}>{player.hero_name}</strong>
          {player.player_name && <small className="player-name">{player.player_name}</small>}
          <small>
            {player.kills}/{player.deaths}/{player.assists} · {t("{n} souls", { n: fmt(player.net_worth) })} · {t("level {n}", { n: player.level })} · {TEAM_NAME[player.team]} ·{" "}
            {player.role}
          </small>
          <Titles player={player} />
          <small>
            {t("Scored most from {best}. Lost most in {worst}.", {
              best: player.strengths.map((x) => `${x.name} (${fmt(x.amount, 1)})`).join(", "),
              worst: player.gaps.map((x) => `${x.name} (−${fmt(x.amount, 1)})`).join(", "),
            })}
          </small>
        </div>
        <div className="open-against">
          <button onClick={() => onOpenBuild(player.build)}>{t("Open final build in the builder")}</button>
          <button
            title={t("Opens the master builder with this match's teams and lane, and sets this player's purchases next to the build it suggests")}
            onClick={() => onOpenMaster(masterPreset(report, player))}
          >
            {t("Compare with the master builder")}
          </button>
          <span title={t("Opens the final build with that enemy's final health and resists as the target, so the builder shows what the build does to them")}>
            <small>{t("against")}</small>
            {report.players
              .filter((o) => o.team !== player.team)
              .map((o) => {
                const last = o.timeline[o.timeline.length - 1];
                return (
                  <img
                    key={o.slot}
                    src={o.hero_icon}
                    alt={o.hero_name}
                    title={
                      last
                        ? `${o.hero_name}: ${t("{h} health, {b}% bullet resist, {s}% spirit resist", { h: fmt(last.game_health, 0), b: fmt(last.bullet_resist, 0), s: fmt(last.spirit_resist, 0) })}`
                        : o.hero_name
                    }
                    onClick={() =>
                      last &&
                      onOpenBuild(player.build, { health: Math.round(last.game_health), bullet_resist: Math.round(last.bullet_resist), spirit_resist: Math.round(last.spirit_resist) })
                    }
                  />
                );
              })}
          </span>
        </div>
      </div>

      <div className="tiles">
        <div>
          <span>{t("Hero damage")}</span>
          <b>{fmt(player.hero_damage, 0)}</b>
        </div>
        <div>
          <span>{t("Objective damage")}</span>
          <b>{fmt(player.objective_damage, 0)}</b>
        </div>
        <div>
          <span>{t("Souls per minute")}</span>
          <b>{fmt(player.net_worth / minutes, 0)}</b>
        </div>
        <div>
          <span>{t("Damage taken")}</span>
          <b>{fmt(player.damage_taken, 0)}</b>
        </div>
        <div title={t("Healing kept for oneself / healing and barriers given to teammates")}>
          <span>{t("Healing: self / teammates")}</span>
          <b>
            {fmt(player.self_healing, 0)} / {fmt(player.teammate_support, 0)}
          </b>
        </div>
        <div title={t("{a} of {b} shots at heroes hit", { a: fmt(player.hero_hits, 0), b: fmt(player.hero_shots, 0) })}>
          <span>{t("Accuracy vs heroes")}</span>
          <b>{accuracy === null ? "–" : `${fmt(accuracy, 1)}%`}</b>
        </div>
        <div title={t("{a} of {b} hits were headshots", { a: fmt(player.hero_headshots, 0), b: fmt(player.hero_hits, 0) })}>
          <span>{t("Headshot share")}</span>
          <b>{headshots === null ? "–" : `${fmt(headshots, 1)}%`}</b>
        </div>
        <div>
          <span>{t("Last hits / denies")}</span>
          <b>
            {player.last_hits} / {player.denies}
          </b>
        </div>
        <div title={t("Team fights joined out of those the player was alive for")}>
          <span>{t("Team fights joined")}</span>
          <b>
            {player.fights_joined} / {player.fights_possible}
          </b>
        </div>
        <div title={t("Team fights the player stayed away from while most of the enemy team was there and the own team did not have the numbers")}>
          <span>{t("Fights left to the team")}</span>
          <b className={player.fights_missed > 0 ? "warn" : ""}>{player.fights_missed}</b>
        </div>
        <div title={player.objectives_taken.map((o) => `${clock(o.time)} ${o.name}`).join("\n") || t("None")}>
          <span>{t("Objectives taken")}</span>
          <b>{player.objectives_taken.length}</b>
        </div>
        <div title={t("Team fights the player was in that their team won (lost fewer players) and lost; even fights are left out")}>
          <span>{t("Fights won / lost when there")}</span>
          <b>
            {player.fights_won} / {player.fights_lost}
          </b>
        </div>
        <div title={t("Team fights in which the player was the first of anyone to die")}>
          <span>{t("Died first in a team fight")}</span>
          <b className={player.first_deaths >= 3 ? "warn" : ""}>{player.first_deaths}</b>
        </div>
        <div title={t("Late-game team fights the player was alive for but not in, whatever they were doing instead")}>
          <span>{t("Late-game fights skipped")}</span>
          <b className={player.late_fights_skipped > 0 ? "warn" : ""}>{player.late_fights_skipped}</b>
        </div>
      </div>

      <div className="group">
        <h3>{t("Macro habits")}</h3>
        <div className="tiles">
          <div
            title={t("Lane {n}, against {who}", { n: player.lane, who: player.lane_opponents.map((o) => names.get(o) ?? "?").join(t(" and ")) || t("nobody") })}
          >
            <span>{t("Souls at {time} vs lane opponent", { time: clock(player.lane_time) })}</span>
            <b>
              {fmt(player.lane_souls, 0)}{" "}
              <small className={player.lane_souls_diff < 0 ? "warn" : "good"}>{signed(player.lane_souls_diff)}</small>
            </b>
          </div>
          <div>
            <span>{t("Laning: last hits / denies / K / D")}</span>
            <b>
              {player.lane_last_hits} / {player.lane_denies} / {player.lane_kills} / {player.lane_deaths}
            </b>
          </div>
          <div title={t("{a} of {b} troopers that could have been last hit", { a: player.trooper_last_hits, b: player.possible_last_hits })}>
            <span>{t("Trooper last hits taken")}</span>
            <b>{fmt(player.last_hit_pct, 0)}%</b>
          </div>
          <div title={t("Souls held without spending them, averaged over the whole match")}>
            <span>{t("Average unspent souls")}</span>
            <b>{fmt(player.avg_unspent_souls, 0)}</b>
          </div>
          <div title={t("Ability points held without spending them, averaged over the whole match")}>
            <span>{t("Average unspent ability points")}</span>
            <b>{fmt(player.avg_unspent_ap, 1)}</b>
          </div>
          <div>
            <span>{t("Time dead")}</span>
            <b>
              {clock(player.dead_seconds)} <small>{fmt((player.dead_seconds / Math.max(report.duration_s, 1)) * 100, 1)}%</small>
            </b>
          </div>
          <div>
            <span>{t("Souls lost on death")}</span>
            <b>{fmt(player.souls_lost_on_death, 0)}</b>
          </div>
        </div>
        <div className="tiles">
          <div title={t("Hero damage dealt to the lane opponents and taken from them up to 9:00")}>
            <span>{t("Lane damage dealt / taken")}</span>
            <b>
              {fmt(player.lane_damage_dealt, 0)} / {fmt(player.lane_damage_taken, 0)}{" "}
              <small className={player.lane_damage_dealt < player.lane_damage_taken ? "warn" : "good"}>
                {signed(player.lane_damage_dealt - player.lane_damage_taken)}
              </small>
            </b>
          </div>
          <div title={t("Kills on the lane opponents and deaths to them up to 9:00")}>
            <span>{t("Against the lane opponents: K / D")}</span>
            <b>
              {player.lane_kills_on} / {player.lane_deaths_to}
            </b>
          </div>
          <div
            title={
              player.roams
                .map(
                  (r) =>
                    `${clock(r.start)} ${t("to {where} for {n}s: {d} enemy death(s), {k} kill(s)", { where: r.name, n: r.seconds, d: r.enemy_deaths, k: r.kills })}${r.died ? `, ${t("died")}` : ""}`,
                )
                .join("\n") || t("Stayed in the lane")
            }
          >
            <span>{t("Roams in the laning phase")}</span>
            <b>
              {player.roams.length}{" "}
              {player.roams.length > 0 && <small>{t("{n} with an enemy death", { n: player.roams.filter((r) => r.enemy_deaths > 0).length })}</small>}
            </b>
          </div>
          <div title={t("Team fights won by two or more players that the team took nothing from, after which this player went to neutral camps")}>
            <span>{t("Won a fight, then farmed")}</span>
            <b className={player.wins_farmed > 0 ? "warn" : ""}>{player.wins_farmed}</b>
          </div>
        </div>
        {player.roams.map((r) => (
          <div key={r.start} className={"insight " + (r.died ? "bad" : r.enemy_deaths > 0 ? "good" : "")}>
            <strong>
              {clock(r.start)} · {t("roamed to {where} for {n}s", { where: r.name, n: r.seconds })}
            </strong>
            <span>
              {r.enemy_deaths > 0 ? t("{n} enemy death(s) there, {k} by {hero}", { n: r.enemy_deaths, k: r.kills, hero: player.hero_name }) : t("No enemy died there")}
              {r.died ? `; ${t("{hero} died", { hero: player.hero_name })}` : ""}.
            </span>
          </div>
        ))}
        <div className="time-split-row">
          <TimeSplit player={player} duration={report.duration_s} />
        </div>
        <div className="rank-legend">
          {player.time_split.map((part, i) => (
            <span key={part.name}>
              <i style={{ background: SPLIT_COLORS[i] }} />
              {part.name} {clock(part.amount)} ({fmt((part.amount / Math.max(report.duration_s, 1)) * 100, 0)}%)
            </span>
          ))}
        </div>
      </div>

      <div className="group">
        <h3>{t("How the build fit the match")}</h3>
        {player.insights.map((i, n) => (
          <div key={n} className={"insight " + i.tone}>
            <strong>{i.title}</strong>
            <span>{i.text}</span>
          </div>
        ))}
      </div>

      {player.item_reasons.length > 0 && (
        <div className="group">
          <h3>{t("Whom the items were bought against")}</h3>
          <div className="muted">
            {t(
              "Every item that answers enemies (a resist, a fire rate slow, healing reduction, resist reduction, an answer to an ability), held against the enemies at the moment it was bought. Up to 12:00 that is the lane{who}, with all the damage they had dealt so far; after that the whole enemy team, with the damage of the minutes before. Green: the item fit what the enemies were doing. Red: it did not.",
              {
                who:
                  player.lane_opponents.length > 0
                    ? ` ${t("(lane {n} against {who})", { n: player.lane, who: player.lane_opponents.map((slot) => names.get(slot) ?? "?").join(t(" and ")) })}`
                    : "",
              },
            )}
          </div>
          <table className="match-table compare timing">
            <thead>
              <tr>
                <th className="left">{t("Time")}</th>
                <th className="left">{t("Item")}</th>
                <th className="left">{t("Against")}</th>
                <th className="left">{t("What it does")}</th>
                <th className="left wide">{t("What the enemies were doing")}</th>
              </tr>
            </thead>
            <tbody>
              {player.item_reasons.map((r, i) => (
                <tr key={i}>
                  <td className="left">{clock(r.time)}</td>
                  <td className="left">
                    <span className="reason-item">
                      <img src={r.icon} alt="" loading="lazy" />
                      <b className={"timing-" + r.tone}>{r.item}</b>
                    </span>
                  </td>
                  <td className="left">{r.lane ? t("Lane") : t("Enemy team")}</td>
                  <td className="left">{r.effect}</td>
                  <td className="left wide muted">{r.text}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {player.build_timing.length > 0 && (
        <div className="group">
          <h3>{t("Build timing")}</h3>
          <div className="muted">
            {t(
              'When the match started calling for an item and when it came. The lane is held against the lane opponents only (rows marked Lane, up to 12:00); everything after it against the whole enemy team, with the damage dealt from the end of the lane on. Within 4 minutes is on time (green), 10 minutes or more, or never, is late (red). The first big items are held against the median of all twelve players. The Score column is what the entry adds to "Items bought against the enemy team" in the ranking: 1 on time, 0.6 a little late, 0.3 very late, 0 never; a dash means it does not count (a teammate covered it, or it is not an answer to the enemy).',
            )}
            {player.counter_score !== null ? ` ${t("{hero}'s average: {n}%.", { hero: player.hero_name, n: fmt(player.counter_score, 0) })}` : ""}
          </div>
          <table className="match-table compare timing">
            <thead>
              <tr>
                <th className="left">{t("What")}</th>
                <th>{t("Called for")}</th>
                <th>{t("Bought")}</th>
                <th>{t("Delay")}</th>
                <th>{t("Score")}</th>
                <th className="left wide">{t("Why")}</th>
              </tr>
            </thead>
            <tbody>
              {player.build_timing.map((row, i) => (
                <tr key={i}>
                  <td className="left">
                    <b className={"timing-" + row.tone}>{row.need}</b>
                    {row.item && <small> · {row.item}</small>}
                  </td>
                  <td>{row.needed_at !== null ? clock(row.needed_at) : "–"}</td>
                  <td>{row.bought_at !== null ? clock(row.bought_at) : t("never")}</td>
                  <td>
                    {row.needed_at !== null && row.bought_at !== null ? (row.bought_at > row.needed_at ? `+${clock(row.bought_at - row.needed_at)}` : t("in time")) : "–"}
                  </td>
                  <td>{row.credit !== null ? fmt(row.credit, 1) : "–"}</td>
                  <td className="left wide muted">{row.text}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <ItemTimingPanel key={`timing-${report.match_id}-${player.slot}`} matchId={report.match_id} slot={player.slot} hero={player.hero_name} />

      {player.build_timing.some((row) => row.advice.length > 0) && (
        <div className="group">
          <h3>{t("Build suggestions")}</h3>
          <div className="muted">
            {t("For every answer that came late or never: the item that would have covered it and a rough number for what it would have changed, taken from the damage actually dealt in the match over the time the need stood open. Resist numbers assume the same damage would have come in; resist reduction numbers assume it was on every target hit, so they are an upper bound.")}
          </div>
          {player.build_timing
            .filter((row) => row.advice.length > 0)
            .map((row, i) => (
              <div key={i} className="insight bad">
                <strong>
                  {row.need} · {t("called for from {time}", { time: clock(row.needed_at ?? 0) })} ·{" "}
                  {row.bought_at !== null ? t("{item} only came at {time}", { item: row.item ?? "", time: clock(row.bought_at) }) : t("never bought")}
                </strong>
                {row.advice.map((a) => (
                  <span key={a.item} className="advice">
                    <img src={a.icon} alt="" loading="lazy" />
                    <b>{a.item}</b>
                    <small>{t("{n} souls", { n: fmt(a.cost) })}</small>
                    <small>{a.effect}</small>
                    <span>{a.item === row.item ? t("Bought on time: {note}.", { note: a.note }) : `${a.note.charAt(0).toLocaleUpperCase(LOCALE)}${a.note.slice(1)}.`}</span>
                  </span>
                ))}
                {row.spent_on.length > 0 && <span>{t("Meanwhile the souls went to: {items}.", { items: row.spent_on.join(", ") })}</span>}
              </div>
            ))}
        </div>
      )}

      {player.objectives_taken.length > 0 && (
        <div className="group">
          <h3>{t("Objectives taken")}</h3>
          <div className="muted">
            {t("Enemy objectives {hero} was at when they fell, and what the rest of the team was doing at that moment.", { hero: player.hero_name })}
          </div>
          {player.objectives_taken.map((o) => {
            const fight = report.fights.find((f) => o.time + 10 >= f.start && o.time <= f.end + 20);
            const side = fight?.sides[player.team];
            const away = side?.absent.some((a) => a.slot === player.slot);
            const risky = Boolean(fight && away && fight.even);
            return (
              <div key={o.time + o.name} className={"insight " + (risky ? "bad" : fight && away ? "good" : "")}>
                <strong>
                  {clock(o.time)} · {o.name}
                  {risky ? ` · ${t("the fight should have come first")}` : ""}
                </strong>
                <span>
                  {fight && side
                    ? away
                      ? `${t("Taken while a team fight was going on elsewhere ({own} against {other}, started {time}); the team lost {lost} and killed {killed}.", {
                          own: side.present.length,
                          other: fight.sides[1 - player.team].present.length,
                          time: clock(fight.start),
                          lost: side.deaths.length,
                          killed: fight.sides[1 - player.team].deaths.length,
                        })}${
                          risky
                            ? ` ${t("The match was even then (walkers {w1} v {w2}, souls {s1}k v {s2}k), so a lost fight would have cost more than this objective gained.", {
                                w1: fight.walkers[0],
                                w2: fight.walkers[1],
                                s1: fmt((fight.team_souls[0] ?? 0) / 1000, 0),
                                s2: fmt((fight.team_souls[1] ?? 0) / 1000, 0),
                              })}`
                            : fight.late
                              ? ` ${t("The team was clearly ahead or behind at that point, so this counts as a trade.")}`
                              : ""
                        }`
                      : t("Taken out of a team fight {hero} was part of (started {time}).", { hero: player.hero_name, time: clock(fight.start) })
                    : t("No team fight was going on at the time.")}
                </span>
              </div>
            );
          })}
        </div>
      )}

      {player.enabled.length > 0 && (
        <div className="group">
          <h3>{t("Damage enabled for teammates")}</h3>
          <div className="muted">
            {t(
              "An estimate of the extra damage teammates dealt because {hero} lowered the enemies' resists or made them take more damage: about {n} in total. The game does not record when a debuff was on someone, so it is counted as on a target while {hero} was fighting that target and for its duration afterwards.",
              { hero: player.hero_name, n: fmt(player.enabled_total, 0) },
            )}
          </div>
          {player.enabled.map((e) => (
            <div key={e.source} className="insight good">
              <strong>
                {e.source} · {e.effect} · {t("+{n} damage for teammates", { n: fmt(e.bonus, 0) })}
              </strong>
              <span>
                {t("Teammates dealt {a} damage of that kind while it was on the target; {b} of it exists only because of the debuff.", { a: fmt(e.amplified, 0), b: fmt(e.bonus, 0) })}{" "}
                {e.by_teammate.map((m) => `${m.name} +${fmt(m.amount, 0)}`).join(", ")}.
                {e.needs
                  ? ` ${t("Needs {what} to apply; the estimate assumes it was applied whenever {hero} was fighting, so read it as an upper figure.", { what: e.needs, hero: player.hero_name })}`
                  : ""}
              </span>
            </div>
          ))}
        </div>
      )}

      {report.deaths.some((d) => d.slot === player.slot) && (
        <div className="group">
          <h3>{t("Deaths")}</h3>
          <div className="muted">
            {t(
              "Who was around a second before each death and where it happened. Caught: outside a team fight, no teammate within 40 m, against two or more enemies or more than 50 m into the enemy's side. A death in your own lane before the later of its two guardians falls is the lane's (a gank, a partner gone back) and never counts as caught. The last column is what the team then went into while {hero} was still dead.",
              { hero: player.hero_name },
            )}
          </div>
          <DeathAnalysisPanel key={`${report.match_id}-${player.slot}`} matchId={report.match_id} slot={player.slot} hero={player.hero_name} />
          <table className="match-table compare timing">
            <thead>
              <tr>
                <th className="left">{t("Time")}</th>
                <th className="left">{t("Killed by")}</th>
                <th>{t("Numbers")}</th>
                <th>{t("Nearest teammate")}</th>
                <th>{t("Where")}</th>
                <th className="left">{t("Kind")}</th>
                <th className="left wide">{t("The team went on without {hero} into", { hero: player.hero_name })}</th>
              </tr>
            </thead>
            <tbody>
              {report.deaths
                .filter((d) => d.slot === player.slot)
                .map((d) => (
                  <tr key={d.time}>
                    <td className="left">{clock(d.time)}</td>
                    <td className="left">{names.get(d.killer) ?? "?"}</td>
                    <td>
                      {d.allies + 1} v {d.enemies}
                    </td>
                    <td>{d.nearest_ally_m !== null ? `${fmt(d.nearest_ally_m, 0)} m` : t("none alive")}</td>
                    <td>{d.depth_m >= 0 ? t("{n} m enemy side", { n: fmt(d.depth_m, 0) }) : t("{n} m own side", { n: fmt(-d.depth_m, 0) })}</td>
                    <td className="left">
                      {d.in_fight ? (
                        t("team fight")
                      ) : d.caught ? (
                        <b className="timing-bad">{t("caught alone")}</b>
                      ) : d.in_lane && d.alone && d.enemies >= 2 ? (
                        t("outnumbered in lane")
                      ) : d.in_lane ? (
                        t("in lane")
                      ) : d.alone ? (
                        t("alone")
                      ) : (
                        t("skirmish")
                      )}
                    </td>
                    <td className="left wide">{d.cost.length > 0 ? <b className="timing-bad">{d.cost.join(t(" and "))}</b> : "–"}</td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>
      )}

      <ReplayPanel key={`replay-${report.match_id}-${player.slot}`} matchId={report.match_id} slot={player.slot} hero={player.hero_name} />

      {report.mid_boss.length > 0 && (
        <div className="group">
          <h3>{t("Mid boss")}</h3>
          {report.mid_boss.map((b) => {
            const side = b.sides[player.team];
            const away = side.absent.find((a) => a.slot === player.slot);
            const there = side.present.includes(player.slot);
            const mine = b.team_claimed === player.team;
            return (
              <div key={b.time} className={"insight " + (away?.missed ? "bad" : there ? "good" : "")}>
                <strong>
                  {clock(b.time)} ·{" "}
                  {mine
                    ? b.stolen
                      ? t("rejuvenator stolen for the team")
                      : t("rejuvenator taken")
                    : b.stolen
                      ? t("rejuvenator stolen by the enemy")
                      : t("rejuvenator lost")}
                  {b.even ? ` · ${t("match even")}` : ""}
                </strong>
                <span>
                  {there
                    ? t(side.deaths.includes(player.slot) ? "{hero} was at the pit and died there ({own} against {other})." : "{hero} was at the pit ({own} against {other}).", {
                        hero: player.hero_name,
                        own: side.present.length,
                        other: b.sides[1 - player.team].present.length,
                      })
                    : away
                      ? `${t("{hero} was {doing}, {n} m away, with {own} teammates against {other} at the pit.", {
                          hero: player.hero_name,
                          doing: away.doing,
                          n: fmt(away.distance, 0),
                          own: side.present.length,
                          other: b.sides[1 - player.team].present.length,
                        })}${away.missed ? ` ${t("Both teams were there: the team needed the numbers.")}` : ""}`
                      : t("{hero} was dead.", { hero: player.hero_name })}
                </span>
              </div>
            );
          })}
        </div>
      )}

      {missed.length > 0 && (
        <div className="group">
          <h3>{t("Team fights without {hero}", { hero: player.hero_name })}</h3>
          <div className="muted">
            {t(
              "Fights with most of the enemy team in them, where the own team did not have the numbers and {hero} was alive but not there. Being at an objective when it falls is a trade and does not count, except in a late-game fight with the match still even; merely hitting troopers or an objective that does not fall always counts in the late game.",
              { hero: player.hero_name },
            )}
          </div>
          {missed.map((m) => (
            <div key={m.start} className="insight bad">
              <strong>
                {clock(m.start)} · {t("{own} against {other}", { own: m.own, other: m.enemy })}
                {m.late ? ` · ${t("late game")}` : ""}
                {m.even
                  ? ` · ${t("match even (walkers {w1} v {w2}, souls {s1}k v {s2}k)", {
                      w1: m.walkers[0],
                      w2: m.walkers[1],
                      s1: fmt((m.teamSouls[0] ?? 0) / 1000, 0),
                      s2: fmt((m.teamSouls[1] ?? 0) / 1000, 0),
                    })}`
                  : ""}
              </strong>
              <span>
                {t("{hero} was {doing}, {n} m away. The team lost {lost} and killed {killed}.", { hero: player.hero_name, doing: m.doing, n: fmt(m.distance, 0), lost: m.lost, killed: m.killed })}
              </span>
            </div>
          ))}
        </div>
      )}

      <div className="group">
        <h3>{t("Kills and deaths")}</h3>
        <FightStrip
          duration={report.duration_s}
          kills={player.kills_at.map((k) => ({ time: k.time, label: names.get(k.other) ?? "?" }))}
          deaths={player.deaths_at.map((d) => ({ time: d.time, label: names.get(d.other) ?? "?" }))}
          revives={player.revives_at}
        />
        {player.revives_at.length > 0 && (
          <div className="muted">
            {t("Orange marks: {hero} went down and a revive brought them back ({times}). The game records no death for these; the ranking counts them.", {
              hero: player.hero_name,
              times: player.revives_at.map(clock).join(", "),
            })}
          </div>
        )}
      </div>

      <div className="match-columns">
        <div className="group">
          <h3>{t("Where the souls came from")}</h3>
          <Bars rows={player.souls.map((s, i) => ({ key: s.name, name: s.name, amount: s.amount, color: PART_COLORS[i % PART_COLORS.length] }))} />
        </div>
        <div className="group">
          <h3>{t("Where the damage went")}</h3>
          <Bars
            rows={[
              { key: "hero", name: t("Enemy heroes"), amount: player.hero_damage, color: PART_COLORS[0] },
              { key: "objective", name: t("Objectives"), amount: player.objective_damage, color: PART_COLORS[1] },
              { key: "creep", name: t("Lane troopers"), amount: player.creep_damage, color: PART_COLORS[5] },
              { key: "neutral", name: t("Neutral camps"), amount: player.neutral_damage, color: PART_COLORS[6] },
            ]}
          />
        </div>
      </div>

      <div className="group">
        <h3>{t("Souls by source over time")}</h3>
        <LineChart times={times} series={soulSeries} height={220} />
      </div>

      <div className="group">
        <h3>{t("Map play")}</h3>
        <div className="tiles small">
          {player.macro_stats.map((s) => (
            <div key={s.name}>
              <span>{s.name}</span>
              <b>{fmt(s.amount, 0)}</b>
            </div>
          ))}
        </div>
      </div>

      <div className="group">
        <h3>{t("Purchases")}</h3>
        {player.unknown_items > 0 && (
          <div className="muted">{t("{n} purchase(s) are not in the calculator's data. Use Update data.", { n: player.unknown_items })}</div>
        )}
        <div className="match-items">
          {player.purchases.map((p, i) => (
            <div
              key={i}
              className={"match-item " + p.category + (p.sold_at !== null ? " sold" : "")}
              title={p.sold_at !== null ? t("Sold or upgraded at {time}", { time: clock(p.sold_at) }) : t("{n} souls", { n: fmt(p.cost) })}
            >
              <img src={p.icon} alt="" loading="lazy" />
              <span>{p.name}</span>
              <small>{clock(p.time)}</small>
            </div>
          ))}
        </div>
      </div>

      <div className="group">
        <h3>{t("Ability order")}</h3>
        <div className="match-items">
          {player.abilities.map((a, i) => (
            <div key={i} className="match-item spirit" title={a.tier === 0 ? t("Unlocked") : t("Upgrade {n}", { n: a.tier })}>
              <img src={a.icon} alt="" loading="lazy" />
              <span>
                {a.name} {a.tier === 0 ? "" : `T${a.tier}`}
              </span>
              <small>{clock(a.time)}</small>
            </div>
          ))}
        </div>
      </div>

      <div className="match-columns">
        <div className="group">
          <h3>{t("Damage to enemy heroes by source")}</h3>
          <Bars rows={player.damage_sources.map((s) => ({ key: s.name, name: s.name, amount: s.amount }))} />
        </div>
        <div className="group">
          <h3>{t("Healing by source")}</h3>
          <Bars rows={player.heal_sources.map((s) => ({ key: s.name, name: s.name, amount: s.amount, color: "var(--vitality)" }))} />
        </div>
      </div>

      <div className="group">
        <h3>{t("Calculator vs game")}</h3>
        <div className="muted">
          {t("What the game recorded at each snapshot next to what the calculator gets for the build owned at that moment (permanent pickups included). Differences usually mean a temporary effect was active in the game: a buff, a conditional item, stolen stats.")}
        </div>
        <table className="match-table compare">
          <thead>
            <tr>
              <th>{t("Time")}</th>
              <th>{t("Level")}</th>
              <th>{t("Souls")}</th>
              <th>{t("Health (game)")}</th>
              <th>{t("Health (calc)")}</th>
              <th>{t("Spirit (game)")}</th>
              <th>{t("Spirit (calc)")}</th>
              <th>{t("Weapon % (game)")}</th>
              <th>{t("Weapon % (calc)")}</th>
              <th />
            </tr>
          </thead>
          <tbody>
            {player.timeline.map((s) => (
              <tr key={s.time}>
                <td>{clock(s.time)}</td>
                <td>{s.level}</td>
                <td>{fmt(s.net_worth)}</td>
                <td>{fmt(s.game_health, 0)}</td>
                <td className={gap(s.game_health, s.calc_health)}>{s.calc_health === null ? "–" : fmt(s.calc_health, 0)}</td>
                <td>{fmt(s.game_spirit, 0)}</td>
                <td className={gap(s.game_spirit, s.calc_spirit)}>{s.calc_spirit === null ? "–" : fmt(s.calc_spirit, 0)}</td>
                <td>{fmt(s.game_weapon, 0)}</td>
                <td className={gap(s.game_weapon, s.calc_weapon)}>{s.calc_weapon === null ? "–" : fmt(s.calc_weapon, 0)}</td>
                <td>
                  <button className="ghost small" title={t("Open the build as it was at this moment")} onClick={() => onOpenBuild(s.build)}>
                    {t("Open")}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}

function Report({
  report,
  onOpenBuild,
  onOpenMaster,
}: {
  report: MatchReport;
  onOpenBuild: (build: Build, target?: Build["target"]) => void;
  onOpenMaster: Props["onOpenMaster"];
}) {
  const [section, setSection] = useState<"overview" | "lanes" | "graphs" | "player" | "compare" | "duels" | "map">("overview");
  const [selected, setSelected] = useState<number | null>(null);
  const [metricKey, setMetricKey] = useState("souls");

  const colors = useMemo(() => {
    const map = new Map<number, string>();
    for (const team of [0, 1]) {
      report.players.filter((p) => p.team === team).forEach((p, i) => map.set(p.slot, PLAYER_COLORS[team][i % 6]));
    }
    return map;
  }, [report]);
  const names = useMemo(() => new Map(report.players.map((p) => [p.slot, p.hero_name])), [report]);
  const bySlot = useMemo(() => new Map(report.players.map((p) => [p.slot, p])), [report]);

  // Soul sources can be charted too, one metric per source.
  const metrics = useMemo(() => {
    const sources = new Set<string>();
    for (const p of report.players) for (const s of p.souls) sources.add(s.name);
    return [
      ...METRICS,
      ...[...sources].map((name) => ({
        key: "souls:" + name,
        label: t("Souls from {source}", { source: name.toLocaleLowerCase(LOCALE) }),
        value: (s: MatchSample) => s.souls.find((x) => x.name === name)?.amount ?? 0,
      })),
    ];
  }, [report]);
  const metric = metrics.find((m) => m.key === metricKey) ?? metrics[0];

  const times = [0, ...(report.players[0]?.timeline.map((s) => s.time) ?? [])];
  const playerSeries: Series[] = report.players.map((p) => ({
    key: p.slot,
    name: p.hero_name,
    color: colors.get(p.slot) ?? "#888",
    values: [0, ...p.timeline.map(metric.value)],
    bold: p.slot === selected,
  }));
  const teamTotal = (team: number) =>
    times.map((_, i) => report.players.filter((p) => p.team === team).reduce((a, p) => a + (playerSeries.find((s) => s.key === p.slot)?.values[i] ?? 0), 0));
  const totals = [teamTotal(0), teamTotal(1)];
  const markers: Marker[] = [
    ...report.objectives
      .filter((o) => o.destroyed_at !== null)
      .map((o) => ({
        time: o.destroyed_at as number,
        // The marker takes the color of the team that destroyed it.
        color: TEAM_COLOR[1 - o.team],
        label: `${TEAM_NAME[1 - o.team]} ${o.lane ? t("destroyed {team}'s {name} (lane {n})", { team: TEAM_NAME[o.team], name: o.name, n: o.lane }) : t("destroyed {team}'s {name}", { team: TEAM_NAME[o.team], name: o.name })}`,
      })),
    ...report.mid_boss.map((b) => ({ time: b.time, color: "#e0e0e0", label: b.stolen ? t("Mid boss killed by {a}, {b} stole the rejuvenator", { a: TEAM_NAME[b.team_killed], b: TEAM_NAME[b.team_claimed] }) : t("Mid boss killed, {team} took the rejuvenator", { team: TEAM_NAME[b.team_claimed] }) })),
  ];

  const pick = (slot: string | number) => setSelected(Number(slot));
  const open = (slot: number) => {
    setSelected(slot);
    setSection("player");
  };
  const player = selected !== null ? bySlot.get(selected) ?? null : null;
  const partLabels = report.ranking[0]?.parts.map((p) => p.label) ?? [];

  return (
    <>
      <div className="match-title">
        <b>{t("Match {id}", { id: report.match_id })}</b>
        <span>
          {clock(report.duration_s)} · {dateTime(report.start_time)}
        </span>
        <span className="match-winner" style={{ ["--team" as string]: TEAM_COLOR[report.winning_team] }}>
          {t("{team} won", { team: TEAM_NAME[report.winning_team] })}
        </span>
      </div>
      <div className="match-nav">
      <div className="side-tabs match-sections">
        <button className={section === "overview" ? "on" : ""} onClick={() => setSection("overview")}>
          {t("Overview")}
        </button>
        <button className={section === "lanes" ? "on" : ""} onClick={() => setSection("lanes")}>
          {t("Lanes")}
        </button>
        <button className={section === "graphs" ? "on" : ""} onClick={() => setSection("graphs")}>
          {t("Graphs")}
        </button>
        <button className={section === "player" ? "on" : ""} onClick={() => setSection("player")}>
          {player ? t("Player: {hero}", { hero: player.hero_name }) : t("Player")}
        </button>
        <button className={section === "compare" ? "on" : ""} onClick={() => setSection("compare")}>
          {t("Compare two players")}
        </button>
        <button className={section === "duels" ? "on" : ""} onClick={() => setSection("duels")}>
          {t("Duels")}
        </button>
        <button className={section === "map" ? "on" : ""} onClick={() => setSection("map")}>
          {t("Map")}
        </button>
      </div>
      <JumpBar watch={`${report.match_id}-${section}-${selected}`} />
      </div>

      {section === "overview" && (
        <>
          {report.story.length > 0 && (
            <div className="group">
              <h3>{t("The match in short")}</h3>
              <div className="muted">
                {t("Written from the numbers below: the lanes at 9:00, the soul lead between the game's snapshots, the fight or mid boss after which the lead moved the most toward the winner within three minutes, the winner's best-ranked player, and what the losing team's numbers point at.")}
              </div>
              <div className="story">
                {report.story.map((line) => (
                  <div key={line.title} className={"insight " + line.tone}>
                    <strong>{line.title}</strong>
                    <span>{line.text}</span>
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="group">
            <h3>{t("Player ranking")}</h3>
            <div className="muted">
              {t("Every player scored out of 100 from their share of the best value in the match. What each part is worth depends on the player's role: absorbing damage counts most for a frontliner, healing teammates and setting up their damage for a support, kills without deaths for an assassin, damage, objectives and souls for a marksman, damage and softened targets for a caster. Hover a role to see what decided it, and a colored part to see what it measures and its maximum for that player. Click a player for their details.")}
            </div>
            <div className="rank-legend">
              {partLabels.map((label, i) => (
                <span key={label}>
                  <i style={{ background: PART_COLORS[i % PART_COLORS.length] }} />
                  {label}
                </span>
              ))}
            </div>
            {report.ranking.map((r, i) => {
              const p = bySlot.get(r.slot);
              if (!p) return null;
              return (
                <div key={r.slot} className={`rank-row place-${Math.min(i + 1, 4)}` + (r.slot === selected ? " on" : "")} onClick={() => open(r.slot)}>
                  <span className="rank-place">{i === 0 ? "MVP" : i + 1}</span>
                  <img src={p.hero_icon} alt="" loading="lazy" />
                  <span className="rank-name">
                    <strong style={{ color: TEAM_COLOR[p.team] }}>
                      {p.hero_name}{" "}
                      <small className="role-tag" title={t("Scored as: {role} ({why})", { role: p.role.toLocaleLowerCase(LOCALE), why: p.role_why })}>
                        {p.role}
                      </small>
                    </strong>
                    {p.player_name && <small className="player-name">{p.player_name}</small>}
                    <small>
                      {p.kills}/{deathsText(p)}/{p.assists} · {t("{n} souls", { n: fmt(p.net_worth) })}
                      {p.game_mvp_rank !== null ? ` · ${t("game MVP #{n}", { n: p.game_mvp_rank })}` : ""}
                    </small>
                    <Titles player={p} />
                  </span>
                  <span className="rank-bar">
                    {r.parts.map((part, j) => (
                      <i
                        key={part.label}
                        style={{ width: `${part.points}%`, background: PART_COLORS[j % PART_COLORS.length] }}
                        title={`${part.label}: ${fmt(part.value, 0)} → ${t("{a} of {b} points", { a: fmt(part.points, 1), b: fmt(part.max_points, 1) })}`}
                      />
                    ))}
                  </span>
                  <b className="rank-score">{fmt(r.score, 1)}</b>
                </div>
              );
            })}
          </div>

          {[0, 1].map((team) => (
            <table key={team} className="match-table">
              <thead>
                <tr>
                  <th className="left" style={{ color: TEAM_COLOR[team] }}>
                    {TEAM_NAME[team]} {report.winning_team === team ? `· ${t("Victory")}` : ""}
                  </th>
                  <th>{t("K / D / A")}</th>
                  <th>{t("Souls")}</th>
                  <th>{t("Level")}</th>
                  <th>{t("Hero damage")}</th>
                  <th>{t("Objective damage")}</th>
                  <th>{t("Taken")}</th>
                  <th>{t("Healing")}</th>
                  <th>{t("Last hits")}</th>
                </tr>
              </thead>
              <tbody>
                {report.players
                  .filter((p) => p.team === team)
                  .map((p) => (
                    <tr key={p.slot} className={p.slot === selected ? "on" : ""} onClick={() => open(p.slot)}>
                      <td className="left">
                        <img src={p.hero_icon} alt="" loading="lazy" />
                        <span className="hero-cell">
                          {p.hero_name}
                          {p.player_name && <small className="player-name">{p.player_name}</small>}
                        </span>
                      </td>
                      <td>
                        {p.kills} / {deathsText(p)} / {p.assists}
                      </td>
                      <td>{fmt(p.net_worth)}</td>
                      <td>{p.level}</td>
                      <td>{fmt(p.hero_damage, 0)}</td>
                      <td>{fmt(p.objective_damage, 0)}</td>
                      <td>{fmt(p.damage_taken, 0)}</td>
                      <td>{fmt(p.healing, 0)}</td>
                      <td>{p.last_hits}</td>
                    </tr>
                  ))}
              </tbody>
            </table>
          ))}

          {report.players.some((p) => p.titles.length > 0) && (
            <div className="group">
              <h3>{t("Titles")}</h3>
              <div className="muted">
                {t("One title for whoever alone leads the match in each thing; a tie gives none. Red ones are for leading in something you would rather not. Under each of the top three: the three parts of the score that player earned the most points from, which is what the ranking rests on.")}
              </div>
              <div className="title-grid">
                {report.ranking.flatMap((r) => {
                  const p = bySlot.get(r.slot);
                  return (p?.titles ?? []).map((title) => (
                    <div key={title.name} className={"title-card " + title.tone} onClick={() => p && open(p.slot)}>
                      <b>{title.name}</b>
                      <span>
                        <img src={p?.hero_icon} alt="" loading="lazy" />
                        <strong style={{ color: TEAM_COLOR[p?.team ?? 0] }}>{p?.hero_name}</strong>
                      </span>
                      <small>
                        {title.what} · {title.value}
                      </small>
                    </div>
                  ));
                })}
              </div>
              {report.ranking.slice(0, 3).map((r, i) => {
                const p = bySlot.get(r.slot);
                if (!p) return null;
                return (
                  <div key={r.slot} className="insight good">
                    <strong>
                      #{i + 1} {p.hero_name} ({p.role.toLocaleLowerCase(LOCALE)}, {t("{n} points", { n: fmt(r.score, 1) })})
                    </strong>
                    <span>
                      {t("Scored most from {best}; lost most in {worst}.", {
                        best: p.strengths.map((x) => `${x.name} (${fmt(x.amount, 1)})`).join(", "),
                        worst: p.gaps.map((x) => `${x.name} (−${fmt(x.amount, 1)})`).join(", "),
                      })}
                    </span>
                  </div>
                );
              })}
            </div>
          )}

          {report.lanes.length > 0 && (
            <div className="group">
              <h3>{t("Lanes")}</h3>
              <div className="muted">
                {t("The laning phase up to 9:00, lane by lane. Players are placed by where they stood in the first minutes, not by the lane the game assigned, so lane swaps show up correctly. Kills and damage are between the lane opponents only. A lane is won with a soul lead of 5% or more.")}
              </div>
              <div className="lanes">
                {report.lanes.map((lane) => (
                  <div key={lane.lane} className="lane-card">
                    <div className="lane-head">
                      <b>{lane.name}</b>
                      <small>{t("lane {n}", { n: lane.lane })}</small>
                      <span style={lane.winner !== null ? { color: TEAM_COLOR[lane.winner] } : undefined}>
                        {lane.winner !== null ? t("{team} won", { team: TEAM_NAME[lane.winner] }) : t("even")}
                      </span>
                    </div>
                    {lane.sides.map((side, team) => (
                      <div key={team} className={"lane-side" + (lane.winner === team ? " won" : "")} style={{ borderColor: TEAM_COLOR[team] }}>
                        <span className="fight-team">
                          {side.players.map((slot) => {
                            const p = bySlot.get(slot);
                            return p ? <img key={slot} src={p.hero_icon} alt={p.hero_name} title={p.hero_name} loading="lazy" onClick={() => open(slot)} /> : null;
                          })}
                        </span>
                        <span title={t("Souls at 9:00")}>
                          <b>{fmt(side.souls / 1000, 1)}k</b>
                        </span>
                        <span title={t("Last hits / denies at 9:00")}>
                          {side.last_hits} / {side.denies}
                        </span>
                        <span title={t("Kills on the lane opponents")}>{t("{n} K", { n: side.kills })}</span>
                        <span title={t("Hero damage dealt to the lane opponents")}>{t("{n} dmg", { n: fmt(side.damage, 0) })}</span>
                        <span title={t("When this side's guardian fell")} className={side.guardian_lost_at !== null ? "" : "muted"}>
                          {side.guardian_lost_at !== null ? t("guardian {time}", { time: clock(side.guardian_lost_at) }) : t("guardian held")}
                        </span>
                      </div>
                    ))}
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="group">
            <h3>{t("Macro habits")}</h3>
            <div className="muted">
              {t("In ranking order; lane lead and unspent souls also count toward the score. Lane lead is souls ahead of the average lane opponent at the end of the laning phase. The bar shows how the match time was spent: fighting heroes, troopers and objectives, neutral camps, out of combat, dead (hover for the times).")}
            </div>
            <table className="match-table compare macro">
              <thead>
                <tr>
                  <th className="left">{t("Player")}</th>
                  <th title={t("Souls ahead of the average lane opponent at the end of the laning phase")}>{t("Lane lead")}</th>
                  <th title={t("Trooper last hits out of the troopers that could have been last hit")}>{t("Last hits")}</th>
                  <th title={t("Souls left unspent, averaged over the match")}>{t("Unspent")}</th>
                  <th title={t("Time spent waiting to respawn")}>{t("Dead")}</th>
                  <th title={t("Souls lost on death")}>{t("Lost on death")}</th>
                  <th title={t("Team fights joined out of those the player was alive for")}>{t("Fights")}</th>
                  <th title={t("Enemy objectives the player was at when they fell")}>{t("Objectives")}</th>
                  <th title={t("Items the match called for that came 10 minutes or more late, or never; hover for which")}>{t("Late items")}</th>
                  <th title={t("Deaths outside a team fight with no teammate within 40 m, against two or more enemies or deep on the enemy's side")}>{t("Caught")}</th>
                  <th title={t("Deaths outside a team fight after which a team fight or a mid boss began while the player was still dead")}>{t("Left short")}</th>
                  <th title={t("Estimated extra damage teammates dealt thanks to this player's resist reductions")}>{t("Enabled")}</th>
                  <th className="left wide">{t("Time spent")}</th>
                </tr>
              </thead>
              <tbody>
                {report.ranking.map((r) => {
                  const p = bySlot.get(r.slot);
                  if (!p) return null;
                  return (
                    <tr key={p.slot} className={p.slot === selected ? "on" : ""} onClick={() => open(p.slot)}>
                      <td className="left">
                        <img src={p.hero_icon} alt="" loading="lazy" />
                        <span style={{ color: TEAM_COLOR[p.team] }}>{p.hero_name}</span>
                      </td>
                      <td className={p.lane_souls_diff < 0 ? "off" : "match"}>{signed(p.lane_souls_diff)}</td>
                      <td>{fmt(p.last_hit_pct, 0)}%</td>
                      <td>{fmt(p.avg_unspent_souls, 0)}</td>
                      <td>{clock(p.dead_seconds)}</td>
                      <td>{fmt(p.souls_lost_on_death, 0)}</td>
                      <td>
                        {p.fights_joined}/{p.fights_possible}
                      </td>
                      <td title={p.objectives_taken.map((o) => `${clock(o.time)} ${o.name}`).join("\n")}>{p.objectives_taken.length}</td>
                      <td title={p.build_timing.filter((row) => row.tone === "bad").map((row) => `${row.need}: ${row.text}`).join("\n")}>
                        {p.build_timing.filter((row) => row.tone === "bad").length || "–"}
                      </td>
                      <td>{p.caught_deaths || "–"}</td>
                      <td>{p.costly_deaths ? <b className="warn">{p.costly_deaths}</b> : "–"}</td>
                      <td>{p.enabled_total > 0 ? `+${fmt(p.enabled_total, 0)}` : "–"}</td>
                      <td className="left wide">
                        <TimeSplit player={p} duration={report.duration_s} />
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          <div className="group">
            <h3>{t("Team fights")}</h3>
            <div className="muted">
              {t("Found from the players' positions: at least five players, two or more per side, fighting in the same place. Dimmed heroes were alive but elsewhere (hover for what they were doing), a red outline marks one the team needed, greyed-out ones were dead. The kill count is colored for the team that lost fewer players. In fights marked late (from 25:00, or once a base objective has fallen) every living player who stayed away is marked, unless they were at an enemy objective when it fell. In fights also marked even (the teams within one walker of each other, or within 5% in souls before builds are full) not even that excuses it: losing the fight would cost more than the objective.")}
            </div>
            <table className="match-table compare fights">
              <thead>
                <tr>
                  <th className="left narrow">{t("Time")}</th>
                  <th className="left narrow">{t("Length")}</th>
                  <th className="left" style={{ color: TEAM_COLOR[0] }}>
                    {TEAM_NAME[0]}
                  </th>
                  <th className="narrow">{t("Players")}</th>
                  <th className="narrow">{t("Kills")}</th>
                  <th title={t("Souls of the players present on each side when the fight started, in thousands")}>{t("Souls there")}</th>
                  <th className="left">{t("First to die")}</th>
                  <th className="left" style={{ color: TEAM_COLOR[1] }}>
                    {TEAM_NAME[1]}
                  </th>
                  <th className="left">{t("Objectives")}</th>
                  <th className="left" title={t("For a fight one team won by two or more players: what it took in the fight and the 90 seconds after, and which of the survivors went to neutral camps instead")}>
                    {t("After the win")}
                  </th>
                </tr>
              </thead>
              <tbody>
                {report.fights.map((f) => (
                  <tr key={f.start}>
                    <td className="left narrow">
                      {clock(f.start)}
                      {f.late && <small className="late-tag">{t("late")}</small>}
                      {f.even && (
                        <small
                          className="late-tag even"
                          title={t("The match was even: walkers {w1} v {w2}, souls {s1}k v {s2}k", {
                            w1: f.walkers[0],
                            w2: f.walkers[1],
                            s1: fmt((f.team_souls[0] ?? 0) / 1000, 0),
                            s2: fmt((f.team_souls[1] ?? 0) / 1000, 0),
                          })}
                        >
                          {t("even")}
                        </small>
                      )}
                    </td>
                    <td className="left narrow">{t("{n}s", { n: f.end - f.start })}</td>
                    <td className="left">
                      <FightTeam side={f.sides[0]} players={bySlot} />
                    </td>
                    <td className="narrow">
                      {f.sides[0].present.length} v {f.sides[1].present.length}
                    </td>
                    <td className="narrow">
                      <b style={f.winner !== null ? { color: TEAM_COLOR[f.winner] } : undefined}>
                        {f.sides[1].deaths.length} : {f.sides[0].deaths.length}
                      </b>
                    </td>
                    <td>
                      {fmt((f.souls[0] ?? 0) / 1000, 0)}k v {fmt((f.souls[1] ?? 0) / 1000, 0)}k
                    </td>
                    <td className="left">
                      {f.kills[0] ? (
                        <span style={{ color: TEAM_COLOR[bySlot.get(f.kills[0].victim)?.team ?? 0] }}>
                          {names.get(f.kills[0].victim)} <small>{clock(f.kills[0].time)}</small>
                        </span>
                      ) : (
                        "–"
                      )}
                    </td>
                    <td className="left">
                      <FightTeam side={f.sides[1]} players={bySlot} />
                    </td>
                    <td className="left">
                      {f.objectives.map(([team, name], i) => (
                        <span key={i} style={{ color: TEAM_COLOR[team] }}>
                          {name}
                          {i < f.objectives.length - 1 ? ", " : ""}
                        </span>
                      ))}
                    </td>
                    <td className="left">
                      {f.follow && f.winner !== null ? (
                        f.follow.taken.length > 0 || f.follow.boss ? (
                          <span style={{ color: TEAM_COLOR[f.winner] }}>{[...f.follow.taken, ...(f.follow.boss ? [t("Mid boss")] : [])].join(", ")}</span>
                        ) : (
                          <span className="fight-team">
                            <b className="warn">{t("nothing")}</b>
                            {f.follow.farming.map((slot) => {
                              const p = bySlot.get(slot);
                              return p ? <img key={slot} src={p.hero_icon} alt={p.hero_name} title={t("{hero} went to neutral camps", { hero: p.hero_name })} loading="lazy" /> : null;
                            })}
                          </span>
                        )
                      ) : (
                        ""
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
            {report.fights.length === 0 && <div className="muted">{t("No team fights found (or the match has no position data).")}</div>}
          </div>

          {report.mid_boss.length > 0 && (
            <div className="group">
              <h3>{t("Mid boss and rejuvenator")}</h3>
              <div className="muted">
                {t("Who was at the pit in the boss's last 30 seconds, read from the players' positions. Dimmed heroes were alive but elsewhere (hover for what they were doing); a red outline marks one who was needed because both teams showed up, a dashed outline one who died in the fight over it; greyed-out heroes were dead. Below each kill: what the team with the rejuvenator made of it while it lasted.")}
              </div>
              {report.mid_boss.map((b) => {
                const own = b.team_claimed;
                const other = 1 - own;
                const paid = b.taken.length > 0;
                return (
                  <div key={b.time} className={"insight " + (paid ? "good" : b.lost.length > 0 ? "bad" : "")}>
                    <strong>
                      {clock(b.time)} ·{" "}
                      {b.stolen ? (
                        <>
                          <span style={{ color: TEAM_COLOR[b.team_killed] }}>{TEAM_NAME[b.team_killed]}</span> {t("killed the boss,")}{" "}
                          <span style={{ color: TEAM_COLOR[own] }}>{TEAM_NAME[own]}</span> {t("stole the rejuvenator")}
                        </>
                      ) : (
                        <>
                          <span style={{ color: TEAM_COLOR[own] }}>{TEAM_NAME[own]}</span> {t("killed the boss and took the rejuvenator")}
                        </>
                      )}
                      {b.late && <small className="late-tag">{t("late")}</small>}
                      {b.even && <small className="late-tag even">{t("even")}</small>}
                      {b.sides.some((side) => side.present.length === 0) && <small className="late-tag quiet">{t("uncontested")}</small>}
                    </strong>
                    <span>
                      {t("Walkers {w1} v {w2}, souls {s1}k v {s2}k when it fell.", {
                        w1: b.walkers[0],
                        w2: b.walkers[1],
                        s1: fmt((b.team_souls[0] ?? 0) / 1000, 0),
                        s2: fmt((b.team_souls[1] ?? 0) / 1000, 0),
                      })}
                    </span>
                    {[0, 1].map((team) => (
                      <span key={team} className="pit-side">
                        <b style={{ color: TEAM_COLOR[team] }}>{TEAM_NAME[team]}</b>
                        <FightTeam side={b.sides[team]} players={bySlot} />
                        <small>
                          {t("{n} at the pit", { n: b.sides[team].present.length })}
                          {b.sides[team].deaths.length > 0 ? `, ${t("{n} died", { n: b.sides[team].deaths.length })}` : ""}
                          {b.sides[team].absent.length > 0 ? `, ${t("{n} elsewhere", { n: b.sides[team].absent.length })}` : ""}
                          {b.sides[team].dead.length > 0 ? `, ${t("{n} dead", { n: b.sides[team].dead.length })}` : ""}
                        </small>
                      </span>
                    ))}
                    <span>
                      {t("In the next {time} {team} {what}; kills {a} against {b}; soul lead {lead}.", {
                        time: clock(b.window_s),
                        team: TEAM_NAME[own],
                        what: `${
                          paid ? t("took {list}", { list: b.taken.map((o) => `${o.name} (${clock(o.time)})`).join(", ") }) : t("took no objective")
                        }${b.lost.length > 0 ? ` ${t("and lost {list}", { list: b.lost.map((o) => `${o.name} (${clock(o.time)})`).join(", ") })}` : ""}`,
                        a: b.kills[own],
                        b: b.kills[other],
                        lead: `${b.lead_change >= 0 ? "+" : "−"}${fmt(Math.abs(b.lead_change), 0)}`,
                      })}
                    </span>
                  </div>
                );
              })}
            </div>
          )}

          {report.deaths.some((d) => d.cost.length > 0) && (
            <div className="group">
              <h3>{t("Deaths that left the team short")}</h3>
              <div className="muted">
                {t("Deaths outside a team fight after which a team fight or a mid boss began while the player was still waiting to respawn. Caught means no teammate within 40 m, against two or more enemies or more than 50 m past the middle of the map on the enemy's side.")}
              </div>
              {report.deaths
                .filter((d) => d.cost.length > 0)
                .map((d) => {
                  const p = bySlot.get(d.slot);
                  return (
                    <div key={d.time + "-" + d.slot} className={"insight " + (d.caught ? "bad" : "")}>
                      <strong>
                        {clock(d.time)} · <span style={{ color: TEAM_COLOR[p?.team ?? 0] }}>{names.get(d.slot)}</span> {t("killed by {hero}", { hero: names.get(d.killer) ?? "?" })}
                        {d.caught && <small className="late-tag">{t("caught")}</small>}
                      </strong>
                      <span>
                        {tn(d.enemies, "{n} enemy against {m}", "{n} enemies against {m}", { m: d.allies + 1 })};{" "}
                        {d.nearest_ally_m !== null ? t("nearest teammate {n} m away", { n: fmt(d.nearest_ally_m, 0) }) : t("no teammate alive")};{" "}
                        {d.depth_m >= 0 ? t("{n} m into the enemy's side", { n: fmt(d.depth_m, 0) }) : t("{n} m on the own side", { n: fmt(-d.depth_m, 0) })}.
                      </span>
                      <span>
                        {t("{team} went into {what} without {hero}.", { team: TEAM_NAME[p?.team ?? 0], what: d.cost.join(t(" and ")), hero: names.get(d.slot) ?? "?" })}
                      </span>
                    </div>
                  );
                })}
            </div>
          )}

          <div className="group">
            <h3>{t("Objectives")}</h3>
            <div className="muted">
              {t("The heroes shown were at the objective in its last 20 seconds: they are the ones who took it. A player credited this way is not blamed for a team fight that was going on at the same time.")}
            </div>
            <table className="match-table compare">
              <tbody>
                {[
                  ...report.objectives
                    .filter((o) => o.destroyed_at !== null)
                    .map((o) => ({
                      time: o.destroyed_at as number,
                      team: 1 - o.team,
                      text: o.lane
                        ? t("destroyed {team}'s {name} (lane {n})", { team: TEAM_NAME[o.team], name: o.name, n: o.lane })
                        : t("destroyed {team}'s {name}", { team: TEAM_NAME[o.team], name: o.name }),
                      takers: o.takers,
                      players: o.player_damage + o.trooper_damage > 0 ? (o.player_damage / (o.player_damage + o.trooper_damage)) * 100 : null,
                    })),
                  ...report.mid_boss.map((b) => ({
                    time: b.time,
                    team: b.team_claimed,
                    text: b.stolen ? t("stole the rejuvenator ({team} killed the mid boss)", { team: TEAM_NAME[b.team_killed] }) : t("took the rejuvenator from the mid boss"),
                    takers: [] as number[],
                    players: null as number | null,
                  })),
                ]
                  .sort((a, b) => a.time - b.time)
                  .map((e, i) => (
                    <tr key={i}>
                      <td className="left narrow">{clock(e.time)}</td>
                      <td className="left">
                        <b style={{ color: TEAM_COLOR[e.team] }}>{TEAM_NAME[e.team]}</b> {e.text}
                      </td>
                      <td className="left">
                        <span className="fight-team">
                          {e.takers.map((slot) => {
                            const p = bySlot.get(slot);
                            return p ? <img key={slot} src={p.hero_icon} alt={p.hero_name} title={t("{hero} was there when it fell", { hero: p.hero_name })} loading="lazy" /> : null;
                          })}
                        </span>
                        {e.takers.length === 0 && e.players !== null && <span className="muted">{t("no player near: troopers finished it")}</span>}
                      </td>
                      <td title={t("Share of all the damage it took over the match that came from players; the rest was troopers")}>
                        {e.players !== null ? t("{n}% by players", { n: fmt(e.players, 0) }) : ""}
                      </td>
                    </tr>
                  ))}
              </tbody>
            </table>
          </div>
        </>
      )}

      {section === "graphs" && (
        <>
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
          <div className="group">
            <h3>{t("{metric}: every player over time", { metric: metric.label })}</h3>
            <div className="muted">{t("Dashed lines mark destroyed objectives and the mid boss. Click a name to highlight that player.")}</div>
            <LineChart times={times} series={playerSeries} markers={markers} height={320} onPick={pick} />
          </div>
          <div className="group">
            <h3>
              {t("{metric}: {a} lead over {b}", { metric: metric.label, a: TEAM_NAME[0], b: TEAM_NAME[1] })}
            </h3>
            <LineChart
              times={times}
              markers={markers}
              height={200}
              series={[{ key: "lead", name: t("{a} minus {b}", { a: TEAM_NAME[0], b: TEAM_NAME[1] }), color: "#e0e0e0", values: totals[0].map((v, i) => v - totals[1][i]) }]}
            />
          </div>
          <div className="match-columns">
            <div className="group">
              <h3>{t("{metric}: team totals", { metric: metric.label })}</h3>
              <LineChart
                times={times}
                height={220}
                series={[0, 1].map((team) => ({ key: team, name: TEAM_NAME[team], color: TEAM_COLOR[team], values: totals[team] }))}
              />
            </div>
            <div className="group">
              <h3>{t("{metric}: end of match", { metric: metric.label })}</h3>
              <Bars
                onPick={pick}
                rows={report.players.map((p) => ({
                  key: p.slot,
                  name: p.hero_name,
                  amount: playerSeries.find((s) => s.key === p.slot)?.values.slice(-1)[0] ?? 0,
                  color: colors.get(p.slot),
                  bold: p.slot === selected,
                }))}
              />
            </div>
          </div>
        </>
      )}

      {section === "lanes" && <MatchLanes report={report} onOpen={open} />}
      {section === "duels" && <MatchDuels report={report} />}
      {section === "map" && <MatchMap report={report} />}

      {section === "compare" && (
        <MatchCompare
          report={report}
          metrics={metrics}
          first={selected ?? report.ranking[0]?.slot ?? report.players[0].slot}
          second={report.ranking.find((r) => r.slot !== (selected ?? report.ranking[0]?.slot))?.slot ?? report.players[0].slot}
        />
      )}

      {section === "player" &&
        (player ? (
          <>
            <div className="player-picker">
              {report.players.map((p) => (
                <button key={p.slot} className={p.slot === selected ? "on" : ""} onClick={() => setSelected(p.slot)} title={p.hero_name}>
                  <img src={p.hero_icon} alt={p.hero_name} style={{ borderColor: TEAM_COLOR[p.team] }} />
                </button>
              ))}
            </div>
            <PlayerDetail report={report} player={player} color={colors.get(player.slot) ?? "inherit"} names={names} onOpenBuild={onOpenBuild} onOpenMaster={onOpenMaster} />
          </>
        ) : (
          <p className="muted">{t("Pick a player in the overview.")}</p>
        ))}
    </>
  );
}

export default function MatchView({ onOpenBuild, onOpenMaster }: Props) {
  const [matchId, setMatchId] = useState("");
  // Starts on the player's own account from Settings; another player's typed here is looked at without
  // changing it.
  const [mine] = useSetting(SETTINGS.account);
  const [accountId, setAccountId] = useState(() => readSetting(SETTINGS.account));
  useEffect(() => setAccountId(mine), [mine]);
  const [deaths, setDeaths] = useState<DeathTrend | null>(null);
  const [showDeaths, setShowDeaths] = useState(false);
  const [report, setReport] = useState<MatchReport | null>(null);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [local, setLocal] = useState<LocalScan | null>(null);
  const [trend, setTrend] = useState<{ rows: MatchSummary[]; skipped: number } | null>(null);
  const [showTrend, setShowTrend] = useState(false);
  const [status, setStatus] = useState("");
  // The last thing that failed, with what tries it again.
  const [failure, setFailure] = useState<{ error: unknown; retry: () => void } | null>(null);
  const [busy, setBusy] = useState(false);

  const loadMatch = async (id: string) => {
    const value = Number(id.trim());
    if (!Number.isInteger(value) || value <= 0) {
      setStatus(t("Enter a match ID (numbers only)."));
      return;
    }
    setBusy(true);
    setFailure(null);
    setStatus(t("Loading match…"));
    try {
      setReport(await api.fetchMatch(value));
      setMatchId(String(value));
      setStatus("");
    } catch (e) {
      setStatus("");
      setFailure({ error: e, retry: () => loadMatch(id) });
    } finally {
      setBusy(false);
    }
  };

  const loadHistory = async () => {
    const id = accountId.trim();
    if (!/^\d+$/.test(id)) {
      setStatus(t("Enter a Steam account ID (numbers only)."));
      return;
    }
    setBusy(true);
    setFailure(null);
    setStatus(t("Loading match history…"));
    try {
      setHistory(await api.matchHistory(id));
      setStatus("");
    } catch (e) {
      setStatus("");
      setFailure({ error: e, retry: loadHistory });
    } finally {
      setBusy(false);
    }
  };

  const analyze = async () => {
    const id = accountId.trim();
    if (!/^\d+$/.test(id)) {
      setStatus(t("Enter a Steam account ID (numbers only)."));
      return;
    }
    setBusy(true);
    setFailure(null);
    setStatus(t("Analyzing the last 10 matches… the first time this downloads each of them."));
    try {
      setTrend(await api.analyzeHistory(id, 10));
      setShowTrend(true);
      setShowDeaths(false);
      setStatus("");
    } catch (e) {
      setStatus("");
      setFailure({ error: e, retry: analyze });
    } finally {
      setBusy(false);
    }
  };

  const whyDie = async () => {
    const id = accountId.trim();
    if (!/^\d+$/.test(id)) {
      setStatus(t("Enter a Steam account ID (numbers only)."));
      return;
    }
    setBusy(true);
    setFailure(null);
    setStatus(t("Studying the deaths of the last 10 matches… the first time this downloads each of them."));
    try {
      setDeaths(await api.deathTrend(id, 10));
      setShowDeaths(true);
      setShowTrend(false);
      setStatus("");
    } catch (e) {
      setStatus("");
      setFailure({ error: e, retry: whyDie });
    } finally {
      setBusy(false);
    }
  };

  const findLocal = async () => {
    setBusy(true);
    setFailure(null);
    setStatus(t("Reading Steam's cache…"));
    try {
      setLocal(await api.findLocalMatches());
      setStatus("");
    } catch (e) {
      setStatus("");
      setFailure({ error: e, retry: findLocal });
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="match-view">
      <div className="match-bar">
        <label>
          {t("Match ID")}
          <input value={matchId} onChange={(e) => setMatchId(e.target.value)} onKeyDown={(e) => e.key === "Enter" && loadMatch(matchId)} />
        </label>
        <button disabled={busy} onClick={() => loadMatch(matchId)}>
          {t("Load match")}
        </button>
        <label title={t("Your SteamID (the long 7656… number or the short account ID). Lists your recent matches.")}>
          {t("Steam account ID")}
          <input value={accountId} onChange={(e) => setAccountId(e.target.value)} onKeyDown={(e) => e.key === "Enter" && loadHistory()} />
        </label>
        {mine.trim() && accountId.trim() !== mine.trim() && (
          <button className="ghost small" onClick={() => setAccountId(mine)} title={t("Back to your own account, from Settings")}>
            {t("My account")}
          </button>
        )}
        <button disabled={busy} onClick={loadHistory}>
          {t("Recent matches")}
        </button>
        <button
          disabled={busy}
          onClick={analyze}
          title={t("Analyzes this account's last 10 matches and shows the averages, where points are lost and every match side by side.")}
        >
          {t("Analyze last 10")}
        </button>
        <button disabled={busy} onClick={whyDie} title={t("Studies every death of this account's last 10 matches: who kills you, with what, and what would have helped.")}>
          {t("Why do I die?")}
        </button>
        {deaths && (report || trend) && (
          <button className="ghost" onClick={() => setShowDeaths(!showDeaths)}>
            {showDeaths ? t("Hide the deaths") : t("Show the deaths")}
          </button>
        )}
        {trend && report && (
          <button className="ghost" onClick={() => setShowTrend(!showTrend)}>
            {showTrend ? t("Back to the match") : t("Back to the 10 matches")}
          </button>
        )}
        <button
          disabled={busy}
          onClick={findLocal}
          title={t("Reads Steam's cache on this computer for the matches the game has opened. Nothing is sent anywhere.")}
        >
          {t("Find matches on this PC")}
        </button>
        {status && <span className="status">{status}</span>}
      </div>
      {failure && <ErrorNote error={failure.error} onRetry={failure.retry} />}

      <div className="match-body">
        {local && (
          <div className="match-history">
            <h3>{t("Matches on this PC")}</h3>
            {local.matches.map((m) => (
              <button
                key={m.match_id}
                className={"history-row" + (report?.match_id === m.match_id ? " on" : "")}
                onClick={() => loadMatch(String(m.match_id))}
              >
                <span>
                  <strong>{t("Match {id}", { id: m.match_id })}</strong>
                  <small>{m.has_replay ? t("replay downloaded") : t("details opened")}</small>
                </span>
                <small>{m.seen_at > 0 ? date(m.seen_at) : ""}</small>
              </button>
            ))}
            {local.matches.length === 0 && (
              <p className="muted">
                {local.found
                  ? t(
                      "No Deadlock matches among the {n} files in {folder}. The game puts a match there when you open its details or download its replay in the game; do that once and search again.",
                      { n: fmt(local.files), folder: local.folder },
                    )
                  : t("Steam's cache folder was not found (looked in {folder}).", { folder: local.folder })}
              </p>
            )}
            {local.matches.length > 0 && <p className="muted">{t("From {folder}", { folder: local.folder })}</p>}
          </div>
        )}
        {history.length > 0 && (
          <div className="match-history">
            <h3>{t("Recent matches")}</h3>
            {history.map((h) => (
              <button
                key={h.match_id}
                className={"history-row" + (h.won ? " won" : " lost") + (report?.match_id === h.match_id ? " on" : "")}
                onClick={() => loadMatch(String(h.match_id))}
              >
                <img src={h.hero_icon} alt="" loading="lazy" />
                <span>
                  <strong>{h.hero_name}</strong>
                  <small>
                    {h.kills}/{h.deaths}/{h.assists} · {t("{n} souls", { n: fmt(h.net_worth) })} · {clock(h.duration_s)}
                  </small>
                </span>
                <small>
                  {h.won ? t("Win") : t("Loss")}
                  <br />
                  {date(h.start_time)}
                </small>
              </button>
            ))}
          </div>
        )}

        <div className="match-main" onClick={toggleDetails}>
          {deaths && showDeaths ? (
            <DeathTrendView
              trend={deaths}
              onOpen={(id) => {
                setShowDeaths(false);
                loadMatch(String(id));
              }}
            />
          ) : trend && (showTrend || !report) ? (
            <MatchTrend
              rows={trend.rows}
              skipped={trend.skipped}
              onOpen={(id) => {
                setShowTrend(false);
                loadMatch(String(id));
              }}
            />
          ) : report ? (
            // A new match starts from the overview with nobody selected.
            <Report key={report.match_id} report={report} onOpenBuild={onOpenBuild} onOpenMaster={onOpenMaster} />
          ) : (
            <p className="muted">
              {t("Enter a match ID to see the player ranking, graphs of how the match went, every player's purchases, soul sources, damage sources and how the calculator compares with the game. The match ID is shown in the game's match history and in the watch tab.")}
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
