import { MouseEvent, useEffect, useMemo, useState } from "react";
import { api, Build, FightSide, fmt, HistoryEntry, LocalScan, MatchPlayer, MatchPreset, MatchReport, MatchSample, MatchSummary } from "../api";
import { MatchDuels, MatchLanes, MatchMap, MatchTrend } from "./MatchExtras";
import { Bars, clock, FightStrip, LineChart, Marker, Series } from "./MatchCharts";
import MatchCompare from "./MatchCompare";
import DeathAnalysisPanel from "./DeathAnalysisPanel";

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
      {side.present.map((slot) => icon(slot, side.deaths.includes(slot) ? "died" : "", side.deaths.includes(slot) ? "in the fight, died" : "in the fight"))}
      {side.absent.map((a) => icon(a.slot, a.missed ? "away missed" : "away", `${a.doing}, ${fmt(a.distance, 0)} m away${a.missed ? " — the team needed the numbers" : ""}`))}
      {side.dead.map((slot) => icon(slot, "dead", "dead when the fight started"))}
    </span>
  );
}

interface Metric {
  key: string;
  label: string;
  value: (s: MatchSample) => number;
}

const METRICS: Metric[] = [
  { key: "souls", label: "Souls", value: (s) => s.net_worth },
  { key: "hero_damage", label: "Hero damage", value: (s) => s.hero_damage },
  { key: "objective_damage", label: "Objective damage", value: (s) => s.objective_damage },
  { key: "creep_damage", label: "Trooper damage", value: (s) => s.creep_damage },
  { key: "neutral_damage", label: "Neutral camp damage", value: (s) => s.neutral_damage },
  { key: "damage_taken", label: "Damage taken", value: (s) => s.damage_taken },
  { key: "mitigated", label: "Damage mitigated", value: (s) => s.mitigated },
  { key: "healing", label: "Healing", value: (s) => s.healing },
  { key: "kills", label: "Kills", value: (s) => s.kills },
  { key: "deaths", label: "Deaths", value: (s) => s.deaths },
  { key: "assists", label: "Assists", value: (s) => s.assists },
  { key: "last_hits", label: "Last hits", value: (s) => s.last_hits },
  { key: "denies", label: "Denies", value: (s) => s.denies },
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
const deathsText = (p: MatchPlayer) => (p.revives_at.length > 0 ? `${p.deaths} (+${p.revives_at.length} revived)` : String(p.deaths));

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
            {player.kills}/{player.deaths}/{player.assists} · {fmt(player.net_worth)} souls · level {player.level} · {TEAM_NAME[player.team]} · {player.role}
          </small>
          <Titles player={player} />
          <small>
            Scored most from {player.strengths.map((x) => `${x.name} (${fmt(x.amount, 1)})`).join(", ")}. Lost most in{" "}
            {player.gaps.map((x) => `${x.name} (−${fmt(x.amount, 1)})`).join(", ")}.
          </small>
        </div>
        <div className="open-against">
          <button onClick={() => onOpenBuild(player.build)}>Open final build in the builder</button>
          <button
            title="Opens the master builder with this match's teams and lane, and sets this player's purchases next to the build it suggests"
            onClick={() => onOpenMaster(masterPreset(report, player))}
          >
            Compare with the master builder
          </button>
          <span title="Opens the final build with that enemy's final health and resists as the target, so the builder shows what the build does to them">
            <small>against</small>
            {report.players
              .filter((o) => o.team !== player.team)
              .map((o) => {
                const last = o.timeline[o.timeline.length - 1];
                return (
                  <img
                    key={o.slot}
                    src={o.hero_icon}
                    alt={o.hero_name}
                    title={last ? `${o.hero_name}: ${fmt(last.game_health, 0)} health, ${fmt(last.bullet_resist, 0)}% bullet resist, ${fmt(last.spirit_resist, 0)}% spirit resist` : o.hero_name}
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
          <span>Hero damage</span>
          <b>{fmt(player.hero_damage, 0)}</b>
        </div>
        <div>
          <span>Objective damage</span>
          <b>{fmt(player.objective_damage, 0)}</b>
        </div>
        <div>
          <span>Souls per minute</span>
          <b>{fmt(player.net_worth / minutes, 0)}</b>
        </div>
        <div>
          <span>Damage taken</span>
          <b>{fmt(player.damage_taken, 0)}</b>
        </div>
        <div title="Healing kept for oneself / healing and barriers given to teammates">
          <span>Healing: self / teammates</span>
          <b>
            {fmt(player.self_healing, 0)} / {fmt(player.teammate_support, 0)}
          </b>
        </div>
        <div title={`${fmt(player.hero_hits, 0)} of ${fmt(player.hero_shots, 0)} shots at heroes hit`}>
          <span>Accuracy vs heroes</span>
          <b>{accuracy === null ? "–" : `${fmt(accuracy, 1)}%`}</b>
        </div>
        <div title={`${fmt(player.hero_headshots, 0)} of ${fmt(player.hero_hits, 0)} hits were headshots`}>
          <span>Headshot share</span>
          <b>{headshots === null ? "–" : `${fmt(headshots, 1)}%`}</b>
        </div>
        <div>
          <span>Last hits / denies</span>
          <b>
            {player.last_hits} / {player.denies}
          </b>
        </div>
        <div title="Team fights joined out of those the player was alive for">
          <span>Team fights joined</span>
          <b>
            {player.fights_joined} / {player.fights_possible}
          </b>
        </div>
        <div title="Team fights the player stayed away from while most of the enemy team was there and the own team did not have the numbers">
          <span>Fights left to the team</span>
          <b className={player.fights_missed > 0 ? "warn" : ""}>{player.fights_missed}</b>
        </div>
        <div title={player.objectives_taken.map((t) => `${clock(t.time)} ${t.name}`).join("\n") || "None"}>
          <span>Objectives taken</span>
          <b>{player.objectives_taken.length}</b>
        </div>
        <div title="Team fights the player was in that their team won (lost fewer players) and lost; even fights are left out">
          <span>Fights won / lost when there</span>
          <b>
            {player.fights_won} / {player.fights_lost}
          </b>
        </div>
        <div title="Team fights in which the player was the first of anyone to die">
          <span>Died first in a team fight</span>
          <b className={player.first_deaths >= 3 ? "warn" : ""}>{player.first_deaths}</b>
        </div>
        <div title="Late-game team fights the player was alive for but not in, whatever they were doing instead">
          <span>Late-game fights skipped</span>
          <b className={player.late_fights_skipped > 0 ? "warn" : ""}>{player.late_fights_skipped}</b>
        </div>
      </div>

      <div className="group">
        <h3>Macro habits</h3>
        <div className="tiles">
          <div title={`Lane ${player.lane}, against ${player.lane_opponents.map((o) => names.get(o) ?? "?").join(" and ") || "nobody"}`}>
            <span>Souls at {clock(player.lane_time)} vs lane opponent</span>
            <b>
              {fmt(player.lane_souls, 0)}{" "}
              <small className={player.lane_souls_diff < 0 ? "warn" : "good"}>{signed(player.lane_souls_diff)}</small>
            </b>
          </div>
          <div>
            <span>Laning: last hits / denies / K / D</span>
            <b>
              {player.lane_last_hits} / {player.lane_denies} / {player.lane_kills} / {player.lane_deaths}
            </b>
          </div>
          <div title={`${player.trooper_last_hits} of ${player.possible_last_hits} troopers that could have been last hit`}>
            <span>Trooper last hits taken</span>
            <b>{fmt(player.last_hit_pct, 0)}%</b>
          </div>
          <div title="Souls held without spending them, averaged over the whole match">
            <span>Average unspent souls</span>
            <b>{fmt(player.avg_unspent_souls, 0)}</b>
          </div>
          <div title="Ability points held without spending them, averaged over the whole match">
            <span>Average unspent ability points</span>
            <b>{fmt(player.avg_unspent_ap, 1)}</b>
          </div>
          <div>
            <span>Time dead</span>
            <b>
              {clock(player.dead_seconds)} <small>{fmt((player.dead_seconds / Math.max(report.duration_s, 1)) * 100, 1)}%</small>
            </b>
          </div>
          <div>
            <span>Souls lost on death</span>
            <b>{fmt(player.souls_lost_on_death, 0)}</b>
          </div>
        </div>
        <div className="tiles">
          <div title="Hero damage dealt to the lane opponents and taken from them up to 9:00">
            <span>Lane damage dealt / taken</span>
            <b>
              {fmt(player.lane_damage_dealt, 0)} / {fmt(player.lane_damage_taken, 0)}{" "}
              <small className={player.lane_damage_dealt < player.lane_damage_taken ? "warn" : "good"}>
                {signed(player.lane_damage_dealt - player.lane_damage_taken)}
              </small>
            </b>
          </div>
          <div title="Kills on the lane opponents and deaths to them up to 9:00">
            <span>Against the lane opponents: K / D</span>
            <b>
              {player.lane_kills_on} / {player.lane_deaths_to}
            </b>
          </div>
          <div
            title={
              player.roams
                .map((r) => `${clock(r.start)} to ${r.name} for ${r.seconds}s: ${r.enemy_deaths} enemy death(s), ${r.kills} kill(s)${r.died ? ", died" : ""}`)
                .join("\n") || "Stayed in the lane"
            }
          >
            <span>Roams in the laning phase</span>
            <b>
              {player.roams.length}{" "}
              {player.roams.length > 0 && <small>{player.roams.filter((r) => r.enemy_deaths > 0).length} with an enemy death</small>}
            </b>
          </div>
          <div title="Team fights won by two or more players that the team took nothing from, after which this player went to neutral camps">
            <span>Won a fight, then farmed</span>
            <b className={player.wins_farmed > 0 ? "warn" : ""}>{player.wins_farmed}</b>
          </div>
        </div>
        {player.roams.map((r) => (
          <div key={r.start} className={"insight " + (r.died ? "bad" : r.enemy_deaths > 0 ? "good" : "")}>
            <strong>
              {clock(r.start)} · roamed to {r.name} for {r.seconds}s
            </strong>
            <span>
              {r.enemy_deaths > 0 ? `${r.enemy_deaths} enemy death(s) there, ${r.kills} by ${player.hero_name}` : "No enemy died there"}
              {r.died ? `; ${player.hero_name} died` : ""}.
            </span>
          </div>
        ))}
        <div className="time-split-row">
          <TimeSplit player={player} duration={report.duration_s} />
        </div>
        <div className="rank-legend">
          {player.time_split.map((t, i) => (
            <span key={t.name}>
              <i style={{ background: SPLIT_COLORS[i] }} />
              {t.name} {clock(t.amount)} ({fmt((t.amount / Math.max(report.duration_s, 1)) * 100, 0)}%)
            </span>
          ))}
        </div>
      </div>

      <div className="group">
        <h3>How the build fit the match</h3>
        {player.insights.map((i, n) => (
          <div key={n} className={"insight " + i.tone}>
            <strong>{i.title}</strong>
            <span>{i.text}</span>
          </div>
        ))}
      </div>

      {player.item_reasons.length > 0 && (
        <div className="group">
          <h3>Whom the items were bought against</h3>
          <div className="muted">
            Every item that answers enemies (a resist, a fire rate slow, healing reduction, resist reduction, an answer to an ability),
            held against the enemies at the moment it was bought. Up to 12:00 that is the lane
            {player.lane_opponents.length > 0
              ? ` (lane ${player.lane} against ${player.lane_opponents.map((slot) => names.get(slot) ?? "?").join(" and ")})`
              : ""}
            , with all the damage they had dealt so far; after that the whole enemy team, with the damage of the minutes before. Green: the
            item fit what the enemies were doing. Red: it did not.
          </div>
          <table className="match-table compare timing">
            <thead>
              <tr>
                <th className="left">Time</th>
                <th className="left">Item</th>
                <th className="left">Against</th>
                <th className="left">What it does</th>
                <th className="left wide">What the enemies were doing</th>
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
                  <td className="left">{r.lane ? "Lane" : "Enemy team"}</td>
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
          <h3>Build timing</h3>
          <div className="muted">
            When the match started calling for an item and when it came. The lane is held against the lane opponents only (rows marked
            Lane, up to 12:00); everything after it against the whole enemy team, with the damage dealt from the end of the lane on.
            Within 4 minutes is on time (green), 10 minutes or more, or never, is late (red). The first big items are held against the median of all twelve players. The Score column is what the
            entry adds to "Items bought against the enemy team" in the ranking: 1 on time, 0.6 a little late, 0.3 very late, 0 never;
            a dash means it does not count (a teammate covered it, or it is not an answer to the enemy).
            {player.counter_score !== null ? ` ${player.hero_name}'s average: ${fmt(player.counter_score, 0)}%.` : ""}
          </div>
          <table className="match-table compare timing">
            <thead>
              <tr>
                <th className="left">What</th>
                <th>Called for</th>
                <th>Bought</th>
                <th>Delay</th>
                <th>Score</th>
                <th className="left wide">Why</th>
              </tr>
            </thead>
            <tbody>
              {player.build_timing.map((t, i) => (
                <tr key={i}>
                  <td className="left">
                    <b className={"timing-" + t.tone}>{t.need}</b>
                    {t.item && <small> · {t.item}</small>}
                  </td>
                  <td>{t.needed_at !== null ? clock(t.needed_at) : "–"}</td>
                  <td>{t.bought_at !== null ? clock(t.bought_at) : "never"}</td>
                  <td>
                    {t.needed_at !== null && t.bought_at !== null ? (t.bought_at > t.needed_at ? `+${clock(t.bought_at - t.needed_at)}` : "in time") : "–"}
                  </td>
                  <td>{t.credit !== null ? fmt(t.credit, 1) : "–"}</td>
                  <td className="left wide muted">{t.text}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {player.build_timing.some((t) => t.advice.length > 0) && (
        <div className="group">
          <h3>Build suggestions</h3>
          <div className="muted">
            For every answer that came late or never: the item that would have covered it and a rough number for what it would have
            changed, taken from the damage actually dealt in the match over the time the need stood open. Resist numbers assume the
            same damage would have come in; resist reduction numbers assume it was on every target hit, so they are an upper bound.
          </div>
          {player.build_timing
            .filter((t) => t.advice.length > 0)
            .map((t, i) => (
              <div key={i} className="insight bad">
                <strong>
                  {t.need} · called for from {clock(t.needed_at ?? 0)} ·{" "}
                  {t.bought_at !== null ? `${t.item} only came at ${clock(t.bought_at)}` : "never bought"}
                </strong>
                {t.advice.map((a) => (
                  <span key={a.item} className="advice">
                    <img src={a.icon} alt="" loading="lazy" />
                    <b>{a.item}</b>
                    <small>{fmt(a.cost)} souls</small>
                    <small>{a.effect}</small>
                    <span>{a.item === t.item ? `Bought on time: ${a.note}.` : `${a.note.charAt(0).toUpperCase()}${a.note.slice(1)}.`}</span>
                  </span>
                ))}
                {t.spent_on.length > 0 && <span>Meanwhile the souls went to: {t.spent_on.join(", ")}.</span>}
              </div>
            ))}
        </div>
      )}

      {player.objectives_taken.length > 0 && (
        <div className="group">
          <h3>Objectives taken</h3>
          <div className="muted">
            Enemy objectives {player.hero_name} was at when they fell, and what the rest of the team was doing at that moment.
          </div>
          {player.objectives_taken.map((t) => {
            const fight = report.fights.find((f) => t.time + 10 >= f.start && t.time <= f.end + 20);
            const side = fight?.sides[player.team];
            const away = side?.absent.some((a) => a.slot === player.slot);
            const risky = Boolean(fight && away && fight.even);
            return (
              <div key={t.time + t.name} className={"insight " + (risky ? "bad" : fight && away ? "good" : "")}>
                <strong>
                  {clock(t.time)} · {t.name}
                  {risky ? " · the fight should have come first" : ""}
                </strong>
                <span>
                  {fight && side
                    ? away
                      ? `Taken while a team fight was going on elsewhere (${side.present.length} against ${fight.sides[1 - player.team].present.length}, started ${clock(fight.start)}); the team lost ${side.deaths.length} and killed ${fight.sides[1 - player.team].deaths.length}.${
                          risky
                            ? ` The match was even then (walkers ${fight.walkers[0]} v ${fight.walkers[1]}, souls ${fmt((fight.team_souls[0] ?? 0) / 1000, 0)}k v ${fmt((fight.team_souls[1] ?? 0) / 1000, 0)}k), so a lost fight would have cost more than this objective gained.`
                            : fight.late
                              ? " The team was clearly ahead or behind at that point, so this counts as a trade."
                              : ""
                        }`
                      : `Taken out of a team fight ${player.hero_name} was part of (started ${clock(fight.start)}).`
                    : "No team fight was going on at the time."}
                </span>
              </div>
            );
          })}
        </div>
      )}

      {player.enabled.length > 0 && (
        <div className="group">
          <h3>Damage enabled for teammates</h3>
          <div className="muted">
            An estimate of the extra damage teammates dealt because {player.hero_name} lowered the enemies' resists or made them take more
            damage: about <b>{fmt(player.enabled_total, 0)}</b> in total. The game does not record when a debuff was on someone, so it is
            counted as on a target while {player.hero_name} was fighting that target and for its duration afterwards.
          </div>
          {player.enabled.map((e) => (
            <div key={e.source} className="insight good">
              <strong>
                {e.source} · {e.effect} · +{fmt(e.bonus, 0)} damage for teammates
              </strong>
              <span>
                Teammates dealt {fmt(e.amplified, 0)} damage of that kind while it was on the target; {fmt(e.bonus, 0)} of it exists only
                because of the debuff. {e.by_teammate.map((m) => `${m.name} +${fmt(m.amount, 0)}`).join(", ")}.
                {e.needs ? ` Needs ${e.needs} to apply; the estimate assumes it was applied whenever ${player.hero_name} was fighting, so read it as an upper figure.` : ""}
              </span>
            </div>
          ))}
        </div>
      )}

      {report.deaths.some((d) => d.slot === player.slot) && (
        <div className="group">
          <h3>Deaths</h3>
          <div className="muted">
            Who was around a second before each death and where it happened. Caught: outside a team fight, no teammate within 40 m,
            against two or more enemies or more than 50 m into the enemy's side. The last column is what the team then went into while{" "}
            {player.hero_name} was still dead.
          </div>
          <DeathAnalysisPanel key={`${report.match_id}-${player.slot}`} matchId={report.match_id} slot={player.slot} hero={player.hero_name} />
          <table className="match-table compare timing">
            <thead>
              <tr>
                <th className="left">Time</th>
                <th className="left">Killed by</th>
                <th>Numbers</th>
                <th>Nearest teammate</th>
                <th>Where</th>
                <th className="left">Kind</th>
                <th className="left wide">The team went on without {player.hero_name} into</th>
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
                    <td>{d.nearest_ally_m !== null ? `${fmt(d.nearest_ally_m, 0)} m` : "none alive"}</td>
                    <td>{d.depth_m >= 0 ? `${fmt(d.depth_m, 0)} m enemy side` : `${fmt(-d.depth_m, 0)} m own side`}</td>
                    <td className="left">
                      {d.in_fight ? "team fight" : d.caught ? <b className="timing-bad">caught alone</b> : d.alone ? "alone" : "skirmish"}
                    </td>
                    <td className="left wide">{d.cost.length > 0 ? <b className="timing-bad">{d.cost.join(" and ")}</b> : "–"}</td>
                  </tr>
                ))}
            </tbody>
          </table>
        </div>
      )}

      {report.mid_boss.length > 0 && (
        <div className="group">
          <h3>Mid boss</h3>
          {report.mid_boss.map((b) => {
            const side = b.sides[player.team];
            const away = side.absent.find((a) => a.slot === player.slot);
            const there = side.present.includes(player.slot);
            const mine = b.team_claimed === player.team;
            return (
              <div key={b.time} className={"insight " + (away?.missed ? "bad" : there ? "good" : "")}>
                <strong>
                  {clock(b.time)} · {mine ? (b.stolen ? "rejuvenator stolen for the team" : "rejuvenator taken") : b.stolen ? "rejuvenator stolen by the enemy" : "rejuvenator lost"}
                  {b.even ? " · match even" : ""}
                </strong>
                <span>
                  {there
                    ? `${player.hero_name} was at the pit${side.deaths.includes(player.slot) ? " and died there" : ""} (${side.present.length} against ${b.sides[1 - player.team].present.length}).`
                    : away
                      ? `${player.hero_name} was ${away.doing}, ${fmt(away.distance, 0)} m away, with ${side.present.length} teammates against ${b.sides[1 - player.team].present.length} at the pit.${away.missed ? " Both teams were there: the team needed the numbers." : ""}`
                      : `${player.hero_name} was dead.`}
                </span>
              </div>
            );
          })}
        </div>
      )}

      {missed.length > 0 && (
        <div className="group">
          <h3>Team fights without {player.hero_name}</h3>
          <div className="muted">
            Fights with most of the enemy team in them, where the own team did not have the numbers and {player.hero_name} was alive but not
            there. Being at an objective when it falls is a trade and does not count, except in a late-game fight with the match
            still even; merely hitting troopers or an objective that does not fall always counts in the late game.
          </div>
          {missed.map((m) => (
            <div key={m.start} className="insight bad">
              <strong>
                {clock(m.start)} · {m.own} against {m.enemy}
                {m.late ? " · late game" : ""}
                {m.even ? ` · match even (walkers ${m.walkers[0]} v ${m.walkers[1]}, souls ${fmt((m.teamSouls[0] ?? 0) / 1000, 0)}k v ${fmt((m.teamSouls[1] ?? 0) / 1000, 0)}k)` : ""}
              </strong>
              <span>
                {player.hero_name} was {m.doing}, {fmt(m.distance, 0)} m away. The team lost {m.lost} and killed {m.killed}.
              </span>
            </div>
          ))}
        </div>
      )}

      <div className="group">
        <h3>Kills and deaths</h3>
        <FightStrip
          duration={report.duration_s}
          kills={player.kills_at.map((k) => ({ time: k.time, label: names.get(k.other) ?? "?" }))}
          deaths={player.deaths_at.map((d) => ({ time: d.time, label: names.get(d.other) ?? "?" }))}
          revives={player.revives_at}
        />
        {player.revives_at.length > 0 && (
          <div className="muted">
            Orange marks: {player.hero_name} went down and a revive brought them back ({player.revives_at.map(clock).join(", ")}). The game
            records no death for these; the ranking counts them.
          </div>
        )}
      </div>

      <div className="match-columns">
        <div className="group">
          <h3>Where the souls came from</h3>
          <Bars rows={player.souls.map((s, i) => ({ key: s.name, name: s.name, amount: s.amount, color: PART_COLORS[i % PART_COLORS.length] }))} />
        </div>
        <div className="group">
          <h3>Where the damage went</h3>
          <Bars
            rows={[
              { key: "hero", name: "Enemy heroes", amount: player.hero_damage, color: PART_COLORS[0] },
              { key: "objective", name: "Objectives", amount: player.objective_damage, color: PART_COLORS[1] },
              { key: "creep", name: "Lane troopers", amount: player.creep_damage, color: PART_COLORS[5] },
              { key: "neutral", name: "Neutral camps", amount: player.neutral_damage, color: PART_COLORS[6] },
            ]}
          />
        </div>
      </div>

      <div className="group">
        <h3>Souls by source over time</h3>
        <LineChart times={times} series={soulSeries} height={220} />
      </div>

      <div className="group">
        <h3>Map play</h3>
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
        <h3>Purchases</h3>
        {player.unknown_items > 0 && (
          <div className="muted">{player.unknown_items} purchase(s) are not in the calculator's data. Use Update data.</div>
        )}
        <div className="match-items">
          {player.purchases.map((p, i) => (
            <div
              key={i}
              className={"match-item " + p.category + (p.sold_at !== null ? " sold" : "")}
              title={p.sold_at !== null ? `Sold or upgraded at ${clock(p.sold_at)}` : `${fmt(p.cost)} souls`}
            >
              <img src={p.icon} alt="" loading="lazy" />
              <span>{p.name}</span>
              <small>{clock(p.time)}</small>
            </div>
          ))}
        </div>
      </div>

      <div className="group">
        <h3>Ability order</h3>
        <div className="match-items">
          {player.abilities.map((a, i) => (
            <div key={i} className="match-item spirit" title={a.tier === 0 ? "Unlocked" : `Upgrade ${a.tier}`}>
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
          <h3>Damage to enemy heroes by source</h3>
          <Bars rows={player.damage_sources.map((s) => ({ key: s.name, name: s.name, amount: s.amount }))} />
        </div>
        <div className="group">
          <h3>Healing by source</h3>
          <Bars rows={player.heal_sources.map((s) => ({ key: s.name, name: s.name, amount: s.amount, color: "var(--vitality)" }))} />
        </div>
      </div>

      <div className="group">
        <h3>Calculator vs game</h3>
        <div className="muted">
          What the game recorded at each snapshot next to what the calculator gets for the build owned at that moment (permanent pickups
          included). Differences usually mean a temporary effect was active in the game: a buff, a conditional item, stolen stats.
        </div>
        <table className="match-table compare">
          <thead>
            <tr>
              <th>Time</th>
              <th>Level</th>
              <th>Souls</th>
              <th>Health (game)</th>
              <th>Health (calc)</th>
              <th>Spirit (game)</th>
              <th>Spirit (calc)</th>
              <th>Weapon % (game)</th>
              <th>Weapon % (calc)</th>
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
                  <button className="ghost small" title="Open the build as it was at this moment" onClick={() => onOpenBuild(s.build)}>
                    Open
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
        label: "Souls from " + name.toLowerCase(),
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
        label: `${TEAM_NAME[1 - o.team]} destroyed ${TEAM_NAME[o.team]}'s ${o.name}${o.lane ? ` (lane ${o.lane})` : ""}`,
      })),
    ...report.mid_boss.map((b) => ({ time: b.time, color: "#e0e0e0", label: b.stolen ? `Mid boss killed by ${TEAM_NAME[b.team_killed]}, ${TEAM_NAME[b.team_claimed]} stole the rejuvenator` : `Mid boss killed, ${TEAM_NAME[b.team_claimed]} took the rejuvenator` })),
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
        Match {report.match_id} · {clock(report.duration_s)} · {new Date(report.start_time * 1000).toLocaleString("en-US")} ·{" "}
        <b style={{ color: TEAM_COLOR[report.winning_team] }}>{TEAM_NAME[report.winning_team]} won</b>
      </div>
      <div className="match-nav">
      <div className="side-tabs match-sections">
        <button className={section === "overview" ? "on" : ""} onClick={() => setSection("overview")}>
          Overview
        </button>
        <button className={section === "lanes" ? "on" : ""} onClick={() => setSection("lanes")}>
          Lanes
        </button>
        <button className={section === "graphs" ? "on" : ""} onClick={() => setSection("graphs")}>
          Graphs
        </button>
        <button className={section === "player" ? "on" : ""} onClick={() => setSection("player")}>
          Player{player ? `: ${player.hero_name}` : ""}
        </button>
        <button className={section === "compare" ? "on" : ""} onClick={() => setSection("compare")}>
          Compare two players
        </button>
        <button className={section === "duels" ? "on" : ""} onClick={() => setSection("duels")}>
          Duels
        </button>
        <button className={section === "map" ? "on" : ""} onClick={() => setSection("map")}>
          Map
        </button>
      </div>
      <JumpBar watch={`${report.match_id}-${section}-${selected}`} />
      </div>

      {section === "overview" && (
        <>
          {report.story.length > 0 && (
            <div className="group">
              <h3>The match in short</h3>
              <div className="muted">
                Written from the numbers below: the lanes at 9:00, the soul lead between the game's snapshots, the fight or mid boss after
                which the lead moved the most toward the winner within three minutes, the winner's best-ranked player, and what the losing
                team's numbers point at.
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
            <h3>Player ranking</h3>
            <div className="muted">
              Every player scored out of 100 from their share of the best value in the match. What each part is worth depends on the
              player's role: absorbing damage counts most for a frontliner, healing teammates and setting up their damage for a support,
              kills without deaths for an assassin, damage, objectives and souls for a marksman, damage and softened targets for a
              caster. Hover a role to see what decided it, and a colored part to see what it measures and its maximum for that player.
              Click a player for their details.
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
                      <small className="role-tag" title={`Scored as: ${p.role.toLowerCase()} (${p.role_why})`}>
                        {p.role}
                      </small>
                    </strong>
                    {p.player_name && <small className="player-name">{p.player_name}</small>}
                    <small>
                      {p.kills}/{deathsText(p)}/{p.assists} · {fmt(p.net_worth)} souls
                      {p.game_mvp_rank !== null ? ` · game MVP #${p.game_mvp_rank}` : ""}
                    </small>
                    <Titles player={p} />
                  </span>
                  <span className="rank-bar">
                    {r.parts.map((part, j) => (
                      <i
                        key={part.label}
                        style={{ width: `${part.points}%`, background: PART_COLORS[j % PART_COLORS.length] }}
                        title={`${part.label}: ${fmt(part.value, 0)} → ${fmt(part.points, 1)} of ${fmt(part.max_points, 1)} points`}
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
                    {TEAM_NAME[team]} {report.winning_team === team ? "· Victory" : ""}
                  </th>
                  <th>K / D / A</th>
                  <th>Souls</th>
                  <th>Level</th>
                  <th>Hero damage</th>
                  <th>Objective damage</th>
                  <th>Taken</th>
                  <th>Healing</th>
                  <th>Last hits</th>
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
              <h3>Titles</h3>
              <div className="muted">
                One title for whoever alone leads the match in each thing; a tie gives none. Red ones are for leading in something you
                would rather not. Under each of the top three: the three parts of the score that player earned the most points from, which
                is what the ranking rests on.
              </div>
              <div className="title-grid">
                {report.ranking.flatMap((r) => {
                  const p = bySlot.get(r.slot);
                  return (p?.titles ?? []).map((t) => (
                    <div key={t.name} className={"title-card " + t.tone} onClick={() => p && open(p.slot)}>
                      <b>{t.name}</b>
                      <span>
                        <img src={p?.hero_icon} alt="" loading="lazy" />
                        <strong style={{ color: TEAM_COLOR[p?.team ?? 0] }}>{p?.hero_name}</strong>
                      </span>
                      <small>
                        {t.what} · {t.value}
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
                      #{i + 1} {p.hero_name} ({p.role.toLowerCase()}, {fmt(r.score, 1)} points)
                    </strong>
                    <span>
                      Scored most from {p.strengths.map((x) => `${x.name} (${fmt(x.amount, 1)})`).join(", ")}; lost most in{" "}
                      {p.gaps.map((x) => `${x.name} (−${fmt(x.amount, 1)})`).join(", ")}.
                    </span>
                  </div>
                );
              })}
            </div>
          )}

          {report.lanes.length > 0 && (
            <div className="group">
              <h3>Lanes</h3>
              <div className="muted">
                The laning phase up to 9:00, lane by lane. Players are placed by where they stood in the first minutes, not by the lane
                the game assigned, so lane swaps show up correctly. Kills and damage are between the lane opponents only. A lane is won
                with a soul lead of 5% or more.
              </div>
              <div className="lanes">
                {report.lanes.map((lane) => (
                  <div key={lane.lane} className="lane-card">
                    <div className="lane-head">
                      <b>{lane.name}</b>
                      <small>lane {lane.lane}</small>
                      <span style={lane.winner !== null ? { color: TEAM_COLOR[lane.winner] } : undefined}>
                        {lane.winner !== null ? `${TEAM_NAME[lane.winner]} won` : "even"}
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
                        <span title="Souls at 9:00">
                          <b>{fmt(side.souls / 1000, 1)}k</b>
                        </span>
                        <span title="Last hits / denies at 9:00">
                          {side.last_hits} / {side.denies}
                        </span>
                        <span title="Kills on the lane opponents">{side.kills} K</span>
                        <span title="Hero damage dealt to the lane opponents">{fmt(side.damage, 0)} dmg</span>
                        <span title="When this side's guardian fell" className={side.guardian_lost_at !== null ? "" : "muted"}>
                          {side.guardian_lost_at !== null ? `guardian ${clock(side.guardian_lost_at)}` : "guardian held"}
                        </span>
                      </div>
                    ))}
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="group">
            <h3>Macro habits</h3>
            <div className="muted">
              In ranking order; lane lead and unspent souls also count toward the score. Lane lead is souls ahead of the average lane opponent at the end of the laning phase. The bar shows how the
              match time was spent: fighting heroes, troopers and objectives, neutral camps, out of combat, dead (hover for the times).
            </div>
            <table className="match-table compare macro">
              <thead>
                <tr>
                  <th className="left">Player</th>
                  <th title="Souls ahead of the average lane opponent at the end of the laning phase">Lane lead</th>
                  <th title="Trooper last hits out of the troopers that could have been last hit">Last hits</th>
                  <th title="Souls left unspent, averaged over the match">Unspent</th>
                  <th title="Time spent waiting to respawn">Dead</th>
                  <th title="Souls lost on death">Lost on death</th>
                  <th title="Team fights joined out of those the player was alive for">Fights</th>
                  <th title="Enemy objectives the player was at when they fell">Objectives</th>
                  <th title="Items the match called for that came 10 minutes or more late, or never; hover for which">Late items</th>
                  <th title="Deaths outside a team fight with no teammate within 40 m, against two or more enemies or deep on the enemy's side">Caught</th>
                  <th title="Deaths outside a team fight after which a team fight or a mid boss began while the player was still dead">Left short</th>
                  <th title="Estimated extra damage teammates dealt thanks to this player's resist reductions">Enabled</th>
                  <th className="left wide">Time spent</th>
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
                      <td title={p.objectives_taken.map((t) => `${clock(t.time)} ${t.name}`).join("\n")}>{p.objectives_taken.length}</td>
                      <td title={p.build_timing.filter((t) => t.tone === "bad").map((t) => `${t.need}: ${t.text}`).join("\n")}>
                        {p.build_timing.filter((t) => t.tone === "bad").length || "–"}
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
            <h3>Team fights</h3>
            <div className="muted">
              Found from the players' positions: at least five players, two or more per side, fighting in the same place. Dimmed heroes were
              alive but elsewhere (hover for what they were doing), a red outline marks one the team needed, greyed-out ones were dead. The
              kill count is colored for the team that lost fewer players. In
              fights marked late (from 25:00, or once a base objective has fallen) every living player who stayed away is marked, unless
              they were at an enemy objective when it fell. In fights also marked even (the teams within one walker of each other, or
              within 5% in souls before builds are full) not even that excuses it: losing the fight would cost more than the objective.
            </div>
            <table className="match-table compare fights">
              <thead>
                <tr>
                  <th className="left narrow">Time</th>
                  <th className="left narrow">Length</th>
                  <th className="left" style={{ color: TEAM_COLOR[0] }}>
                    {TEAM_NAME[0]}
                  </th>
                  <th className="narrow">Players</th>
                  <th className="narrow">Kills</th>
                  <th title="Souls of the players present on each side when the fight started, in thousands">Souls there</th>
                  <th className="left">First to die</th>
                  <th className="left" style={{ color: TEAM_COLOR[1] }}>
                    {TEAM_NAME[1]}
                  </th>
                  <th className="left">Objectives</th>
                  <th className="left" title="For a fight one team won by two or more players: what it took in the fight and the 90 seconds after, and which of the survivors went to neutral camps instead">
                    After the win
                  </th>
                </tr>
              </thead>
              <tbody>
                {report.fights.map((f) => (
                  <tr key={f.start}>
                    <td className="left narrow">
                      {clock(f.start)}
                      {f.late && <small className="late-tag">late</small>}
                      {f.even && (
                        <small
                          className="late-tag even"
                          title={`The match was even: walkers ${f.walkers[0]} v ${f.walkers[1]}, souls ${fmt((f.team_souls[0] ?? 0) / 1000, 0)}k v ${fmt((f.team_souls[1] ?? 0) / 1000, 0)}k`}
                        >
                          even
                        </small>
                      )}
                    </td>
                    <td className="left narrow">{f.end - f.start}s</td>
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
                          <span style={{ color: TEAM_COLOR[f.winner] }}>{[...f.follow.taken, ...(f.follow.boss ? ["Mid boss"] : [])].join(", ")}</span>
                        ) : (
                          <span className="fight-team">
                            <b className="warn">nothing</b>
                            {f.follow.farming.map((slot) => {
                              const p = bySlot.get(slot);
                              return p ? <img key={slot} src={p.hero_icon} alt={p.hero_name} title={`${p.hero_name} went to neutral camps`} loading="lazy" /> : null;
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
            {report.fights.length === 0 && <div className="muted">No team fights found (or the match has no position data).</div>}
          </div>

          {report.mid_boss.length > 0 && (
            <div className="group">
              <h3>Mid boss and rejuvenator</h3>
              <div className="muted">
                Who was at the pit in the boss's last 30 seconds, read from the players' positions. Dimmed heroes were alive but elsewhere
                (hover for what they were doing); a red outline marks one who was needed because both teams showed up, a dashed outline one
                who died in the fight over it; greyed-out heroes were dead. Below each kill: what the team with the rejuvenator made of it
                while it lasted.
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
                          <span style={{ color: TEAM_COLOR[b.team_killed] }}>{TEAM_NAME[b.team_killed]}</span> killed the boss,{" "}
                          <span style={{ color: TEAM_COLOR[own] }}>{TEAM_NAME[own]}</span> stole the rejuvenator
                        </>
                      ) : (
                        <>
                          <span style={{ color: TEAM_COLOR[own] }}>{TEAM_NAME[own]}</span> killed the boss and took the rejuvenator
                        </>
                      )}
                      {b.late && <small className="late-tag">late</small>}
                      {b.even && <small className="late-tag even">even</small>}
                      {b.sides.some((side) => side.present.length === 0) && <small className="late-tag quiet">uncontested</small>}
                    </strong>
                    <span>
                      Walkers {b.walkers[0]} v {b.walkers[1]}, souls {fmt((b.team_souls[0] ?? 0) / 1000, 0)}k v {fmt((b.team_souls[1] ?? 0) / 1000, 0)}k
                      when it fell.
                    </span>
                    {[0, 1].map((team) => (
                      <span key={team} className="pit-side">
                        <b style={{ color: TEAM_COLOR[team] }}>{TEAM_NAME[team]}</b>
                        <FightTeam side={b.sides[team]} players={bySlot} />
                        <small>
                          {b.sides[team].present.length} at the pit
                          {b.sides[team].deaths.length > 0 ? `, ${b.sides[team].deaths.length} died` : ""}
                          {b.sides[team].absent.length > 0 ? `, ${b.sides[team].absent.length} elsewhere` : ""}
                          {b.sides[team].dead.length > 0 ? `, ${b.sides[team].dead.length} dead` : ""}
                        </small>
                      </span>
                    ))}
                    <span>
                      In the next {clock(b.window_s)} {TEAM_NAME[own]}{" "}
                      {paid ? `took ${b.taken.map((t) => `${t.name} (${clock(t.time)})`).join(", ")}` : "took no objective"}
                      {b.lost.length > 0 ? ` and lost ${b.lost.map((t) => `${t.name} (${clock(t.time)})`).join(", ")}` : ""}; kills {b.kills[own]} against{" "}
                      {b.kills[other]}; soul lead {b.lead_change >= 0 ? "+" : "−"}
                      {fmt(Math.abs(b.lead_change), 0)}.
                    </span>
                  </div>
                );
              })}
            </div>
          )}

          {report.deaths.some((d) => d.cost.length > 0) && (
            <div className="group">
              <h3>Deaths that left the team short</h3>
              <div className="muted">
                Deaths outside a team fight after which a team fight or a mid boss began while the player was still waiting to respawn.
                Caught means no teammate within 40 m, against two or more enemies or more than 50 m past the middle of the map on the
                enemy's side.
              </div>
              {report.deaths
                .filter((d) => d.cost.length > 0)
                .map((d) => {
                  const p = bySlot.get(d.slot);
                  return (
                    <div key={d.time + "-" + d.slot} className={"insight " + (d.caught ? "bad" : "")}>
                      <strong>
                        {clock(d.time)} · <span style={{ color: TEAM_COLOR[p?.team ?? 0] }}>{names.get(d.slot)}</span> killed by {names.get(d.killer) ?? "?"}
                        {d.caught && <small className="late-tag">caught</small>}
                      </strong>
                      <span>
                        {d.enemies} {d.enemies === 1 ? "enemy" : "enemies"} against {d.allies + 1};{" "}
                        {d.nearest_ally_m !== null ? `nearest teammate ${fmt(d.nearest_ally_m, 0)} m away` : "no teammate alive"};{" "}
                        {d.depth_m >= 0 ? `${fmt(d.depth_m, 0)} m into the enemy's side` : `${fmt(-d.depth_m, 0)} m on the own side`}.
                      </span>
                      <span>
                        {TEAM_NAME[p?.team ?? 0]} went into {d.cost.join(" and ")} without {names.get(d.slot)}.
                      </span>
                    </div>
                  );
                })}
            </div>
          )}

          <div className="group">
            <h3>Objectives</h3>
            <div className="muted">
              The heroes shown were at the objective in its last 20 seconds: they are the ones who took it. A player credited this way is not
              blamed for a team fight that was going on at the same time.
            </div>
            <table className="match-table compare">
              <tbody>
                {[
                  ...report.objectives
                    .filter((o) => o.destroyed_at !== null)
                    .map((o) => ({
                      time: o.destroyed_at as number,
                      team: 1 - o.team,
                      text: `destroyed ${TEAM_NAME[o.team]}'s ${o.name}${o.lane ? ` (lane ${o.lane})` : ""}`,
                      takers: o.takers,
                      players: o.player_damage + o.trooper_damage > 0 ? (o.player_damage / (o.player_damage + o.trooper_damage)) * 100 : null,
                    })),
                  ...report.mid_boss.map((b) => ({
                    time: b.time,
                    team: b.team_claimed,
                    text: b.stolen ? `stole the rejuvenator (${TEAM_NAME[b.team_killed]} killed the mid boss)` : "took the rejuvenator from the mid boss",
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
                            return p ? <img key={slot} src={p.hero_icon} alt={p.hero_name} title={`${p.hero_name} was there when it fell`} loading="lazy" /> : null;
                          })}
                        </span>
                        {e.takers.length === 0 && e.players !== null && <span className="muted">no player near: troopers finished it</span>}
                      </td>
                      <td title="Share of all the damage it took over the match that came from players; the rest was troopers">
                        {e.players !== null ? `${fmt(e.players, 0)}% by players` : ""}
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
            Show
            <select value={metric.key} onChange={(e) => setMetricKey(e.target.value)}>
              {metrics.map((m) => (
                <option key={m.key} value={m.key}>
                  {m.label}
                </option>
              ))}
            </select>
          </label>
          <div className="group">
            <h3>{metric.label}: every player over time</h3>
            <div className="muted">Dashed lines mark destroyed objectives and the mid boss. Click a name to highlight that player.</div>
            <LineChart times={times} series={playerSeries} markers={markers} height={320} onPick={pick} />
          </div>
          <div className="group">
            <h3>
              {metric.label}: {TEAM_NAME[0]} lead over {TEAM_NAME[1]}
            </h3>
            <LineChart
              times={times}
              markers={markers}
              height={200}
              series={[{ key: "lead", name: `${TEAM_NAME[0]} minus ${TEAM_NAME[1]}`, color: "#e0e0e0", values: totals[0].map((v, i) => v - totals[1][i]) }]}
            />
          </div>
          <div className="match-columns">
            <div className="group">
              <h3>{metric.label}: team totals</h3>
              <LineChart
                times={times}
                height={220}
                series={[0, 1].map((team) => ({ key: team, name: TEAM_NAME[team], color: TEAM_COLOR[team], values: totals[team] }))}
              />
            </div>
            <div className="group">
              <h3>{metric.label}: end of match</h3>
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
          <p className="muted">Pick a player in the overview.</p>
        ))}
    </>
  );
}

export default function MatchView({ onOpenBuild, onOpenMaster }: Props) {
  const [matchId, setMatchId] = useState("");
  const [accountId, setAccountId] = useState("");
  const [report, setReport] = useState<MatchReport | null>(null);
  const [history, setHistory] = useState<HistoryEntry[]>([]);
  const [local, setLocal] = useState<LocalScan | null>(null);
  const [trend, setTrend] = useState<{ rows: MatchSummary[]; skipped: number } | null>(null);
  const [showTrend, setShowTrend] = useState(false);
  const [status, setStatus] = useState("");
  const [busy, setBusy] = useState(false);

  const loadMatch = async (id: string) => {
    const value = Number(id.trim());
    if (!Number.isInteger(value) || value <= 0) {
      setStatus("Enter a match ID (numbers only).");
      return;
    }
    setBusy(true);
    setStatus("Loading match…");
    try {
      setReport(await api.fetchMatch(value));
      setMatchId(String(value));
      setStatus("");
    } catch (e) {
      setStatus(String(e));
    } finally {
      setBusy(false);
    }
  };

  const loadHistory = async () => {
    const id = accountId.trim();
    if (!/^\d+$/.test(id)) {
      setStatus("Enter a Steam account ID (numbers only).");
      return;
    }
    setBusy(true);
    setStatus("Loading match history…");
    try {
      setHistory(await api.matchHistory(id));
      setStatus("");
    } catch (e) {
      setStatus(String(e));
    } finally {
      setBusy(false);
    }
  };

  const analyze = async () => {
    const id = accountId.trim();
    if (!/^\d+$/.test(id)) {
      setStatus("Enter a Steam account ID (numbers only).");
      return;
    }
    setBusy(true);
    setStatus("Analyzing the last 10 matches… the first time this downloads each of them.");
    try {
      setTrend(await api.analyzeHistory(id, 10));
      setShowTrend(true);
      setStatus("");
    } catch (e) {
      setStatus(String(e));
    } finally {
      setBusy(false);
    }
  };

  const findLocal = async () => {
    setBusy(true);
    setStatus("Reading Steam's cache…");
    try {
      setLocal(await api.findLocalMatches());
      setStatus("");
    } catch (e) {
      setStatus(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="match-view">
      <div className="match-bar">
        <label>
          Match ID
          <input value={matchId} onChange={(e) => setMatchId(e.target.value)} onKeyDown={(e) => e.key === "Enter" && loadMatch(matchId)} />
        </label>
        <button disabled={busy} onClick={() => loadMatch(matchId)}>
          Load match
        </button>
        <label title="Your SteamID (the long 7656… number or the short account ID). Lists your recent matches.">
          Steam account ID
          <input value={accountId} onChange={(e) => setAccountId(e.target.value)} onKeyDown={(e) => e.key === "Enter" && loadHistory()} />
        </label>
        <button disabled={busy} onClick={loadHistory}>
          Recent matches
        </button>
        <button
          disabled={busy}
          onClick={analyze}
          title="Analyzes this account's last 10 matches and shows the averages, where points are lost and every match side by side."
        >
          Analyze last 10
        </button>
        {trend && report && (
          <button className="ghost" onClick={() => setShowTrend(!showTrend)}>
            {showTrend ? "Back to the match" : "Back to the 10 matches"}
          </button>
        )}
        <button
          disabled={busy}
          onClick={findLocal}
          title="Reads Steam's cache on this computer for the matches the game has opened. Nothing is sent anywhere."
        >
          Find matches on this PC
        </button>
        {status && <span className="status">{status}</span>}
      </div>

      <div className="match-body">
        {local && (
          <div className="match-history">
            <h3>Matches on this PC</h3>
            {local.matches.map((m) => (
              <button
                key={m.match_id}
                className={"history-row" + (report?.match_id === m.match_id ? " on" : "")}
                onClick={() => loadMatch(String(m.match_id))}
              >
                <span>
                  <strong>Match {m.match_id}</strong>
                  <small>{m.has_replay ? "replay downloaded" : "details opened"}</small>
                </span>
                <small>{m.seen_at > 0 ? new Date(m.seen_at * 1000).toLocaleDateString("en-US") : ""}</small>
              </button>
            ))}
            {local.matches.length === 0 && (
              <p className="muted">
                {local.found
                  ? `No Deadlock matches among the ${fmt(local.files)} files in ${local.folder}. The game puts a match there when you open its details or download its replay in the game; do that once and search again.`
                  : `Steam's cache folder was not found (looked in ${local.folder}).`}
              </p>
            )}
            {local.matches.length > 0 && <p className="muted">From {local.folder}</p>}
          </div>
        )}
        {history.length > 0 && (
          <div className="match-history">
            <h3>Recent matches</h3>
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
                    {h.kills}/{h.deaths}/{h.assists} · {fmt(h.net_worth)} souls · {clock(h.duration_s)}
                  </small>
                </span>
                <small>
                  {h.won ? "Win" : "Loss"}
                  <br />
                  {new Date(h.start_time * 1000).toLocaleDateString("en-US")}
                </small>
              </button>
            ))}
          </div>
        )}

        <div className="match-main" onClick={toggleDetails}>
          {trend && (showTrend || !report) ? (
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
              Enter a match ID to see the player ranking, graphs of how the match went, every player's purchases, soul sources, damage
              sources and how the calculator compares with the game. The match ID is shown in the game's match history and in the watch tab.
            </p>
          )}
        </div>
      </div>
    </div>
  );
}
