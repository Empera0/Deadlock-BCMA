import { useMemo, useState } from "react";
import { fmt, MatchPlayer, MatchReport, MatchSummary } from "../api";
import { Bars, clock } from "./MatchCharts";
import { date, t } from "../i18n";

const TEAM_NAME = ["Hidden King", "Archmother"];
const TEAM_COLOR = ["#f2b84b", "#4fc3f7"];
const PLAYER_COLORS = [
  ["#f2b84b", "#e8873a", "#e2574c", "#f08fb0", "#c9c227", "#b98a5a"],
  ["#4fc3f7", "#6f7fe0", "#26a69a", "#a184e0", "#81c784", "#9fb4c4"],
];

function useColors(report: MatchReport) {
  return useMemo(() => {
    const map = new Map<number, string>();
    for (const team of [0, 1]) {
      report.players.filter((p) => p.team === team).forEach((p, i) => map.set(p.slot, PLAYER_COLORS[team][i % 6]));
    }
    return map;
  }, [report]);
}

/** Who hurt and killed whom: one table per attacking team. */
export function MatchDuels({ report }: { report: MatchReport }) {
  const top = Math.max(...report.duels.map((d) => d.damage), 1);
  const cell = (from: number, to: number) => report.duels.find((d) => d.from === from && d.to === to);
  return (
    <>
      <div className="muted">
        {t("Hero damage each player dealt to each enemy, with kills in brackets. Read a row for what a player did, a column for what was done to a player. Brighter cells are more damage.")}
      </div>
      {[0, 1].map((team) => {
        const attackers = report.players.filter((p) => p.team === team);
        const targets = report.players.filter((p) => p.team !== team);
        return (
          <div key={team} className="group">
            <h3 style={{ color: TEAM_COLOR[team] }}>
              {t("{a} attacking {b}", { a: TEAM_NAME[team], b: TEAM_NAME[1 - team] })}
            </h3>
            <table className="match-table compare duels">
              <thead>
                <tr>
                  <th className="left">{t("Attacker ↓ · Target →")}</th>
                  {targets.map((target) => (
                    <th key={target.slot} title={target.player_name}>
                      <img src={target.hero_icon} alt="" loading="lazy" />
                      <br />
                      {target.hero_name}
                    </th>
                  ))}
                  <th>{t("Total")}</th>
                </tr>
              </thead>
              <tbody>
                {attackers.map((a) => (
                  <tr key={a.slot}>
                    <td className="left">
                      <img src={a.hero_icon} alt="" loading="lazy" />
                      {a.hero_name}
                    </td>
                    {targets.map((target) => {
                      const c = cell(a.slot, target.slot);
                      const damage = Math.max(c?.damage ?? 0, 0);
                      return (
                        <td
                          key={target.slot}
                          style={{ background: `rgba(226, 87, 76, ${(damage / top) * 0.75})` }}
                          title={`${a.hero_name} → ${target.hero_name}: ${t("{n} damage, {k} kill(s)", { n: fmt(damage, 0), k: c?.kills ?? 0 })}`}
                        >
                          {fmt(damage, 0)}
                          {c && c.kills > 0 ? <b> ({c.kills})</b> : ""}
                        </td>
                      );
                    })}
                    <td>
                      <b>{fmt(a.hero_damage, 0)}</b> ({a.kills})
                    </td>
                  </tr>
                ))}
                <tr className="totals">
                  <td className="left">{t("Taken from this team")}</td>
                  {targets.map((target) => (
                    <td key={target.slot}>
                      {fmt(
                        attackers.reduce((sum, a) => sum + (cell(a.slot, target.slot)?.damage ?? 0), 0),
                        0,
                      )}{" "}
                      ({target.deaths})
                    </td>
                  ))}
                  <td />
                </tr>
              </tbody>
            </table>
          </div>
        );
      })}
    </>
  );
}

/** The minimap with where players went, died and fought. */
export function MatchMap({ report }: { report: MatchReport }) {
  const colors = useColors(report);
  const minutes = Math.ceil(report.duration_s / 60);
  const [who, setWho] = useState<number | null>(null);
  const [from, setFrom] = useState(0);
  const [to, setTo] = useState(minutes);
  const [layers, setLayers] = useState({ paths: true, deaths: true, fights: true });
  const SIZE = 1000;
  const inWindow = (time: number) => time >= from * 60 && time <= to * 60;
  const names = new Map(report.players.map((p) => [p.slot, p.hero_name]));
  const shown = report.map.paths.filter((p) => who === null || p.slot === who);
  const toggle = (key: keyof typeof layers) => setLayers({ ...layers, [key]: !layers[key] });

  return (
    <div className="map-view">
      <div className="map-side">
        <div className="group">
          <h3>{t("Show")}</h3>
          <div className="player-picker wrap">
            <button className={who === null ? "on" : ""} onClick={() => setWho(null)}>
              {t("All")}
            </button>
            {report.players.map((p) => (
              <button key={p.slot} className={p.slot === who ? "on" : ""} onClick={() => setWho(p.slot)} title={p.hero_name}>
                <img src={p.hero_icon} alt={p.hero_name} style={{ borderColor: colors.get(p.slot) }} />
              </button>
            ))}
          </div>
          <label className="field inline">
            {t("From minute")}
            <input type="number" min={0} max={to} value={from} onChange={(e) => setFrom(Math.max(0, Math.min(to, Number(e.target.value) || 0)))} />
          </label>
          <label className="field inline">
            {t("To minute")}
            <input
              type="number"
              min={from}
              max={minutes}
              value={to}
              onChange={(e) => setTo(Math.max(from, Math.min(minutes, Number(e.target.value) || 0)))}
            />
          </label>
          {(["paths", "deaths", "fights"] as const).map((key) => (
            <label key={key} className="field inline">
              <input type="checkbox" checked={layers[key]} onChange={() => toggle(key)} />
              {key === "paths" ? t("Where they were") : key === "deaths" ? t("Deaths and kills") : t("Team fights")}
            </label>
          ))}
        </div>
        <div className="muted">
          {t(
            "Dots are a position every {n} seconds; where they pile up, the player spent time. A red cross is a death{kills}. Numbered circles are the team fights from the overview, in order. Hover a mark for the time. {team}'s base is at the bottom.",
            { n: report.map.step, kills: who !== null ? t(", a green ring a kill by {hero}", { hero: names.get(who) ?? "?" }) : "", team: TEAM_NAME[0] },
          )}
        </div>
      </div>
      <svg className="map" viewBox={`0 0 ${SIZE} ${SIZE}`}>
        <image href={report.map.image} width={SIZE} height={SIZE} opacity={0.75} />
        {layers.paths &&
          shown.map((path) => (
            <g key={path.slot} fill={colors.get(path.slot)} opacity={who === null ? 0.28 : 0.4}>
              {path.points
                .filter((p) => inWindow(p.time))
                .map((p) => (
                  <circle key={p.time} cx={p.left * SIZE} cy={p.top * SIZE} r={who === null ? 4 : 6} />
                ))}
            </g>
          ))}
        {layers.fights &&
          report.map.fights.map((f, i) =>
            inWindow(f.time) ? (
              <g key={i}>
                <circle cx={f.left * SIZE} cy={f.top * SIZE} r={26} fill="rgba(0,0,0,0.45)" stroke="#fff" strokeWidth={2}>
                  <title>{t("Team fight {n} at {time}", { n: i + 1, time: clock(f.time) })}</title>
                </circle>
                <text x={f.left * SIZE} y={f.top * SIZE + 8} textAnchor="middle" fill="#fff" fontSize={22} fontWeight={700} pointerEvents="none">
                  {i + 1}
                </text>
              </g>
            ) : null,
          )}
        {layers.deaths &&
          report.map.deaths
            .filter((d) => inWindow(d.time) && (who === null || d.victim === who || d.killer === who))
            .map((d, i) => {
              const x = d.left * SIZE;
              const y = d.top * SIZE;
              const kill = who !== null && d.killer === who;
              return kill ? (
                <circle key={i} cx={x} cy={y} r={11} fill="none" stroke="#7cb342" strokeWidth={4}>
                  <title>
                    {clock(d.time)} · {t("{a} killed {b}", { a: names.get(d.killer) ?? "?", b: names.get(d.victim) ?? "?" })}
                  </title>
                </circle>
              ) : (
                <g key={i} stroke="#ff5a52" strokeWidth={4}>
                  <title>
                    {clock(d.time)} · {t("{a} killed by {b}", { a: names.get(d.victim) ?? "?", b: names.get(d.killer) ?? "?" })}
                  </title>
                  <line x1={x - 9} y1={y - 9} x2={x + 9} y2={y + 9} />
                  <line x1={x - 9} y1={y + 9} x2={x + 9} y2={y - 9} />
                </g>
              );
            })}
      </svg>
    </div>
  );
}

const average = (values: number[]) => (values.length ? values.reduce((a, b) => a + b, 0) / values.length : 0);

/** A player's recent matches side by side: averages, where the points go missing, and each match. */
export function MatchTrend({ rows, skipped, onOpen }: { rows: MatchSummary[]; skipped: number; onOpen: (matchId: number) => void }) {
  if (rows.length === 0) {
    return (
      <p className="muted">
        {t("None of the recent matches could be analyzed")}
        {skipped > 0 ? ` ${t("({n} were not available)", { n: skipped })}` : ""}.
      </p>
    );
  }
  const wins = rows.filter((r) => r.won).length;
  const lanes = rows.filter((r) => r.lane_souls_diff > 0).length;
  const fightShare = average(rows.filter((r) => r.fights_possible > 0).map((r) => (r.fights_joined / r.fights_possible) * 100));
  // The average points earned in each part of the score, as a share of what the part is worth.
  const labels = rows[0].parts.map((p) => p.label);
  const parts = labels
    .map((label) => {
      const found = rows.map((r) => r.parts.find((p) => p.label === label)).filter((p) => p !== undefined);
      // What a part is worth depends on the role played in that match, so the maximum is averaged too.
      const max = average(found.map((p) => p.max_points)) || 1;
      const points = average(found.map((p) => p.points));
      return { label, points, max, lost: max - points };
    })
    .sort((a, b) => b.lost - a.lost);
  const heroes = [...new Set(rows.map((r) => r.hero_name))]
    .map((name) => {
      const games = rows.filter((r) => r.hero_name === name);
      return { name, icon: games[0].hero_icon, games: games.length, wins: games.filter((g) => g.won).length, score: average(games.map((g) => g.score)) };
    })
    .sort((a, b) => b.games - a.games);
  // Mistakes by the number of matches they showed up in, most frequent first.
  const needs = [...new Set(rows.flatMap((r) => r.late_needs))];
  const habits = [
    { label: t("Stayed out of a late-game team fight while the match was even"), games: rows.filter((r) => r.even_fights_skipped > 0), total: (r: MatchSummary) => r.even_fights_skipped },
    { label: t("Caught alone outside a team fight"), games: rows.filter((r) => r.caught_deaths > 0), total: (r: MatchSummary) => r.caught_deaths },
    { label: t("Died outside a fight and left the team short for a fight or a mid boss"), games: rows.filter((r) => r.costly_deaths > 0), total: (r: MatchSummary) => r.costly_deaths },
    { label: t("Alive but not at the pit for a mid boss"), games: rows.filter((r) => r.mid_boss_alive > r.mid_boss_present), total: (r: MatchSummary) => r.mid_boss_alive - r.mid_boss_present },
    { label: t("Lost the lane (behind the lane opponent in souls)"), games: rows.filter((r) => r.lane_souls_diff < 0), total: () => 1 },
    { label: t("Took more damage from the lane opponents than was dealt to them"), games: rows.filter((r) => r.lane_damage_taken > r.lane_damage_dealt), total: () => 1 },
    { label: t("Died to a lane opponent in the laning phase"), games: rows.filter((r) => r.lane_deaths_to > 0), total: (r: MatchSummary) => r.lane_deaths_to },
    { label: t("Roamed in the laning phase without an enemy dying there"), games: rows.filter((r) => r.roams > r.roams_paid), total: (r: MatchSummary) => r.roams - r.roams_paid },
    { label: t("Went to neutral camps after a clearly won fight the team took nothing from"), games: rows.filter((r) => r.wins_farmed > 0), total: (r: MatchSummary) => r.wins_farmed },
    ...needs.map((need) => ({
      label: t("{need}: bought 10 minutes or more late, or never", { need }),
      games: rows.filter((r) => r.late_needs.includes(need)),
      total: () => 1,
    })),
  ]
    .map((h) => ({ label: h.label, matches: h.games.length, times: h.games.reduce((sum, r) => sum + h.total(r), 0) }))
    .filter((h) => h.matches > 0)
    .sort((a, b) => b.matches - a.matches || b.times - a.times);
  // Titles by how often they were earned.
  const tally = (pick: (r: MatchSummary) => string[]) =>
    [...new Set(rows.flatMap(pick))]
      .map((name) => ({ name, times: rows.filter((r) => pick(r).includes(name)).length }))
      .sort((a, b) => b.times - a.times);
  const earned = [...tally((r) => r.titles).map((t) => ({ ...t, tone: "good" })), ...tally((r) => r.bad_titles).map((t) => ({ ...t, tone: "bad" }))];
  const roles = [...new Set(rows.map((r) => r.role))].map((role) => `${role} ×${rows.filter((r) => r.role === role).length}`).join(", ");
  const answered = rows.filter((r) => r.counter_score !== null).map((r) => r.counter_score as number);
  // Oldest first, so the bars read left to right in time.
  const inTime = [...rows].sort((a, b) => a.start_time - b.start_time);

  return (
    <>
      <div className="match-title">
        {t("{n} recent matches", { n: rows.length })}
        {skipped > 0 ? ` ${t("({n} more were not available from the match service)", { n: skipped })}` : ""}
      </div>
      <div className="tiles">
        <div>
          <span>{t("Wins")}</span>
          <b>
            {wins} / {rows.length} <small>{fmt((wins / rows.length) * 100, 0)}%</small>
          </b>
        </div>
        <div>
          <span>{t("Average score")}</span>
          <b>{fmt(average(rows.map((r) => r.score)), 1)}</b>
        </div>
        <div title={t("1 is the best of the twelve players")}>
          <span>{t("Average place")}</span>
          <b>{fmt(average(rows.map((r) => r.place)), 1)}</b>
        </div>
        <div>
          <span>{t("Lanes won")}</span>
          <b>
            {lanes} / {rows.length}
          </b>
        </div>
        <div>
          <span>{t("Team fights joined")}</span>
          <b>{fmt(fightShare, 0)}%</b>
        </div>
        <div>
          <span>{t("Late-game fights skipped per match")}</span>
          <b className={average(rows.map((r) => r.late_fights_skipped)) >= 1 ? "warn" : ""}>{fmt(average(rows.map((r) => r.late_fights_skipped)), 1)}</b>
        </div>
        <div>
          <span>{t("Average unspent souls")}</span>
          <b>{fmt(average(rows.map((r) => r.avg_unspent_souls)), 0)}</b>
        </div>
        <div>
          <span>{t("Time dead per match")}</span>
          <b>{clock(average(rows.map((r) => r.dead_seconds)))}</b>
        </div>
        <div title={t("Deaths outside a team fight with no teammate within 40 m, against two or more enemies or deep on the enemy's side")}>
          <span>{t("Caught alone per match")}</span>
          <b className={average(rows.map((r) => r.caught_deaths)) >= 1.5 ? "warn" : ""}>{fmt(average(rows.map((r) => r.caught_deaths)), 1)}</b>
        </div>
        <div title={t("How well the builds answered the enemies: resists, resist reduction, healing reduction and answers to abilities, bought and bought in time")}>
          <span>{t("Items against the enemy")}</span>
          <b>{answered.length > 0 ? `${fmt(average(answered), 0)}%` : "–"}</b>
        </div>
        <div>
          <span>{t("Roles played")}</span>
          <b>
            <small>{roles}</small>
          </b>
        </div>
      </div>

      {earned.length > 0 && (
        <div className="group">
          <h3>{t("Titles earned")}</h3>
          <div className="muted">{t("How often each title was earned over these matches: leading all twelve players in something.")}</div>
          <span className="titles">
            {earned.map((t) => (
              <i key={t.name} className={"title " + t.tone}>
                {t.name} ×{t.times}
              </i>
            ))}
          </span>
        </div>
      )}

      {habits.length > 0 && (
        <div className="group">
          <h3>{t("Recurring habits")}</h3>
          <div className="muted">
            {t("Mistakes by the number of matches they showed up in, most frequent first. One match can be chance; the same thing in half the matches or more (red) is a habit.")}
          </div>
          {habits.map((h) => (
            <div key={h.label} className="source" title={t("{a} time(s) over {b} match(es)", { a: h.times, b: h.matches })}>
              <div className="source-head">
                <span>{h.label}</span>
                <b className={h.matches * 2 >= rows.length ? "warn" : ""}>
                  {t("{a} of {b} matches", { a: h.matches, b: rows.length })}
                  {h.times > h.matches ? <small> · {t("{n} times", { n: h.times })}</small> : null}
                </b>
              </div>
              <div className="source-bar">
                <div style={{ width: `${(h.matches / rows.length) * 100}%`, background: h.matches * 2 >= rows.length ? "#e0736f" : "#b98a5a" }} />
              </div>
            </div>
          ))}
        </div>
      )}

      <div className="match-columns">
        <div className="group">
          <h3>{t("Where the points go missing")}</h3>
          <div className="muted">{t("Average points left on the table in each part of the score, largest first.")}</div>
          {parts.map((p) => (
            <div key={p.label} className="source" title={t("{a} of {b} points on average", { a: fmt(p.points, 1), b: fmt(p.max, 1) })}>
              <div className="source-head">
                <span>{p.label}</span>
                <b>
                  −{fmt(p.lost, 1)} <small>{t("of {n}", { n: fmt(p.max, 1) })}</small>
                </b>
              </div>
              <div className="source-bar">
                <div style={{ width: `${(p.lost / Math.max(p.max, 0.01)) * 100}%`, background: "#e0736f" }} />
              </div>
            </div>
          ))}
        </div>
        <div>
          <div className="group">
            <h3>{t("Score, match by match (oldest first)")}</h3>
            <div className="score-strip">
              {inTime.map((r) => (
                <button
                  key={r.match_id}
                  className={r.won ? "won" : "lost"}
                  style={{ height: `${Math.max(r.score, 4)}%` }}
                  title={`${r.hero_name} · ${t("{n} points", { n: fmt(r.score, 1) })} · ${t("place {n}", { n: r.place })} · ${r.won ? t("win") : t("loss")} · ${date(r.start_time)}`}
                  onClick={() => onOpen(r.match_id)}
                />
              ))}
            </div>
          </div>
          <div className="group">
            <h3>{t("By hero")}</h3>
            <Bars
              share={false}
              rows={heroes.map((h) => ({ key: h.name, name: `${h.name} · ${t("{a} game(s) · {b} won · average score", { a: h.games, b: h.wins })}`, amount: h.score }))}
            />
          </div>
        </div>
      </div>

      <div className="group">
        <h3>{t("Matches")}</h3>
        <table className="match-table compare macro">
          <thead>
            <tr>
              <th className="left">{t("Match")}</th>
              <th>{t("Result")}</th>
              <th>{t("Score")}</th>
              <th>{t("Place")}</th>
              <th>{t("K / D / A")}</th>
              <th>{t("Lane lead")}</th>
              <th>{t("Fights joined")}</th>
              <th>{t("Late fights skipped")}</th>
              <th title={t("Caught alone outside a team fight / deaths that left the team short for a fight or a mid boss")}>{t("Caught / left short")}</th>
              <th title={t("At the pit for the mid boss kills the player was alive for")}>{t("Mid boss")}</th>
              <th title={t("Items bought against the enemy, and bought in time")}>{t("Answers")}</th>
              <th>{t("Unspent souls")}</th>
              <th>{t("Enabled damage")}</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={r.match_id} onClick={() => onOpen(r.match_id)}>
                <td className="left">
                  <img src={r.hero_icon} alt="" loading="lazy" />
                  <span className="hero-cell">
                    {r.hero_name} <small className="role-tag">{r.role}</small>
                    {r.titles.length + r.bad_titles.length > 0 && (
                      <span className="titles">
                        {r.titles.map((title) => (
                          <i key={title} className="title good">
                            {title}
                          </i>
                        ))}
                        {r.bad_titles.map((title) => (
                          <i key={title} className="title bad">
                            {title}
                          </i>
                        ))}
                      </span>
                    )}
                    <small className="player-name">
                      {date(r.start_time)} · {clock(r.duration_s)}
                    </small>
                  </span>
                </td>
                <td className={r.won ? "match" : "off"}>{r.won ? t("Win") : t("Loss")}</td>
                <td>
                  <b>{fmt(r.score, 1)}</b>
                </td>
                <td className={`place-text place-${Math.min(r.place, 4)}`}>{r.place}</td>
                <td>
                  {r.kills} / {r.deaths} / {r.assists}
                </td>
                <td className={r.lane_souls_diff < 0 ? "off" : "match"}>
                  {r.lane_souls_diff > 0 ? "+" : ""}
                  {fmt(r.lane_souls_diff, 0)}
                </td>
                <td>
                  {r.fights_joined}/{r.fights_possible}
                </td>
                <td className={r.late_fights_skipped > 0 ? "off" : ""}>{r.late_fights_skipped}</td>
                <td className={r.costly_deaths > 0 ? "off" : ""}>
                  {r.caught_deaths} / {r.costly_deaths}
                </td>
                <td className={r.mid_boss_alive > r.mid_boss_present ? "off" : ""}>{r.mid_boss_alive > 0 ? `${r.mid_boss_present}/${r.mid_boss_alive}` : "–"}</td>
                <td title={r.late_needs.join("\n")}>{r.counter_score !== null ? `${fmt(r.counter_score, 0)}%` : "–"}</td>
                <td>{fmt(r.avg_unspent_souls, 0)}</td>
                <td>{r.enabled_total > 0 ? `+${fmt(r.enabled_total, 0)}` : "–"}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </>
  );
}

const LANE_TEAM_COLOR = ["#f2b84b", "#4fc3f7"];
const LANE_TEAM_NAME = ["Hidden King", "Archmother"];

/** The laning phase in full: every lane with its players side by side, then the power-ups under the bridges. */
export function MatchLanes({ report, onOpen }: { report: MatchReport; onOpen: (slot: number) => void }) {
  const bySlot = new Map(report.players.map((p) => [p.slot, p]));
  const k = (v: number) => `${fmt(v / 1000, 1)}k`;
  const best = (players: MatchPlayer[], value: (p: MatchPlayer) => number, more = true) => {
    const values = players.map(value);
    const top = more ? Math.max(...values) : Math.min(...values);
    return (p: MatchPlayer) => (players.length > 1 && value(p) === top && values.filter((v) => v === top).length === 1 ? "lead" : "");
  };
  const buffKinds = [...new Set(report.players.flatMap((p) => p.bridge_buffs.map((b) => b.name)))];
  const sampleTimes = report.players[0]?.timeline.map((s) => s.time) ?? [];
  return (
    <>
      {report.lanes.map((lane) => {
        const players = lane.sides.flatMap((side) => side.players.map((slot) => bySlot.get(slot)).filter((p): p is MatchPlayer => p !== undefined));
        const mark = {
          souls: best(players, (p) => p.lane_souls),
          lastHits: best(players, (p) => p.lane_detail.trooper_last_hits),
          denies: best(players, (p) => p.lane_detail.denies),
          dealt: best(players, (p) => p.lane_damage_dealt),
          taken: best(players, (p) => p.lane_damage_taken, false),
          guardian: best(players, (p) => p.lane_detail.guardian_damage),
          boxes: best(players, (p) => p.lane_detail.box_souls),
        };
        return (
          <div key={lane.lane} className="group">
            <h3>
              {t("{name} lane", { name: lane.name })}
              <small style={lane.winner !== null ? { color: LANE_TEAM_COLOR[lane.winner] } : undefined}>
                {lane.winner !== null ? t("{team} won the lane", { team: LANE_TEAM_NAME[lane.winner] }) : t("even")}
              </small>
            </h3>
            <div className="muted">
              {t("Everything up to 9:00, from the game's own snapshots. Damage dealt and taken, and kills and deaths in the K / D column after the slash, are against the lane opponents only; the rest is everything the player did in that time. Guardian damage is the player's objective damage, which in the laning phase is the enemy guardian. The best value of the lane is highlighted.")}
            </div>
            <div className="lane-sides">
              {lane.sides.map((side, team) => (
                <span key={team} style={{ borderColor: LANE_TEAM_COLOR[team] }}>
                  <b style={{ color: LANE_TEAM_COLOR[team] }}>{LANE_TEAM_NAME[team]}</b>
                  <span title={t("Souls of the side at 3:00, 6:00 and 9:00")}>{t("souls {list}", { list: side.souls_at.map(k).join(" → ") })}</span>
                  <span title={t("This side's guardian: first hit, when it fell, and how much of the damage it took came from players")}>
                    {t("own guardian: first hit {first}, {fell}, {n}% of its damage from players", {
                      first: side.guardian_first_hit !== null ? clock(side.guardian_first_hit) : "–",
                      fell: side.guardian_lost_at !== null ? t("fell {time}", { time: clock(side.guardian_lost_at) }) : t("held"),
                      n: fmt((side.guardian_player_damage / Math.max(side.guardian_player_damage + side.guardian_trooper_damage, 1)) * 100, 0),
                    })}
                  </span>
                </span>
              ))}
            </div>
            <table className="match-table compare lane-table">
              <thead>
                <tr>
                  <th className="left">{t("Player")}</th>
                  <th title={t("Souls at 3:00, 6:00 and 9:00")}>{t("Souls 3 / 6 / 9")}</th>
                  <th title={t("Souls ahead of the average lane opponent at 9:00")}>{t("Lead")}</th>
                  <th title={t("Trooper last hits out of the troopers that could have been last hit")}>{t("Last hits")}</th>
                  <th title={t("Denies, and the souls from denies (shared with the lane partner)")}>{t("Denies")}</th>
                  <th title={t("Kills / deaths / assists up to 9:00; after the slash, kills on and deaths to the lane opponents")}>{t("K / D / A")}</th>
                  <th title={t("Hero damage dealt to the lane opponents")}>{t("Dealt")}</th>
                  <th title={t("Hero damage taken from the lane opponents")}>{t("Taken")}</th>
                  <th title={t("Shots at enemy heroes that hit")}>{t("Accuracy")}</th>
                  <th title={t("Objective damage up to 9:00: the enemy guardian")}>{t("Guardian dmg")}</th>
                  <th title={t("Neutral creeps killed and the souls they gave")}>{t("Camps")}</th>
                  <th title={t("Souls from boxes and statues")}>{t("Boxes")}</th>
                  <th title={t("Permanent power-ups picked up from golden statues")}>{t("Statues")}</th>
                  <th title={t("Souls lost by dying")}>{t("Lost on death")}</th>
                  <th title={t("Trips to another lane, and how many of them had an enemy die there")}>{t("Roams")}</th>
                </tr>
              </thead>
              <tbody>
                {players.map((p) => {
                  const l = p.lane_detail;
                  return (
                    <tr key={p.slot} onClick={() => onOpen(p.slot)}>
                      <td className="left">
                        <img src={p.hero_icon} alt="" loading="lazy" />
                        <span style={{ color: LANE_TEAM_COLOR[p.team] }}>{p.hero_name}</span>
                      </td>
                      <td className={mark.souls(p)}>{l.souls_at.map(k).join(" / ")}</td>
                      <td className={p.lane_souls_diff < 0 ? "off" : "match"}>
                        {p.lane_souls_diff > 0 ? "+" : ""}
                        {fmt(p.lane_souls_diff, 0)}
                      </td>
                      <td className={mark.lastHits(p)}>
                        {l.trooper_last_hits}/{l.possible_last_hits} <small>{fmt((l.trooper_last_hits / Math.max(l.possible_last_hits, 1)) * 100, 0)}%</small>
                      </td>
                      <td className={mark.denies(p)}>
                        {l.denies} <small>+{fmt(l.deny_souls, 0)}</small>
                      </td>
                      <td>
                        {l.kills} / {l.deaths} / {l.assists}{" "}
                        <small>
                          ({p.lane_kills_on} / {p.lane_deaths_to})
                        </small>
                      </td>
                      <td className={mark.dealt(p)}>{fmt(p.lane_damage_dealt, 0)}</td>
                      <td className={mark.taken(p)}>{fmt(p.lane_damage_taken, 0)}</td>
                      <td>{fmt(l.accuracy_pct, 0)}%</td>
                      <td className={mark.guardian(p)}>{fmt(l.guardian_damage, 0)}</td>
                      <td>
                        {l.camp_kills} <small>+{fmt(l.camp_souls, 0)}</small>
                      </td>
                      <td className={mark.boxes(p)}>{fmt(l.box_souls, 0)}</td>
                      <td>{l.statue_buffs || "–"}</td>
                      <td className={l.souls_lost_on_death > 0 ? "off" : ""}>{l.souls_lost_on_death > 0 ? fmt(l.souls_lost_on_death, 0) : "–"}</td>
                      <td title={p.roams.map((r) => `${clock(r.start)} ${t("to {where} for {n}s", { where: r.name, n: r.seconds })}`).join("\n")}>
                        {p.roams.length > 0 ? t("{n} ({m} paid)", { n: p.roams.length, m: p.roams.filter((r) => r.enemy_deaths > 0).length }) : "–"}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        );
      })}

      {buffKinds.length > 0 && (
        <div className="group">
          <h3>{t("Bridge power-ups (whole match)")}</h3>
          <div className="muted">
            {t("Temporary power-ups picked up under the bridges, by kind, and by which snapshot of the match they had been picked up. The match record has how many each player took but not the moment or the place, so who contested a power-up cannot be read from it.")}
          </div>
          <table className="match-table compare lane-table">
            <thead>
              <tr>
                <th className="left">{t("Player")}</th>
                <th>{t("Total")}</th>
                {buffKinds.map((kind) => (
                  <th key={kind}>{kind}</th>
                ))}
                {sampleTimes.map((time) => (
                  <th key={time} title={t("Picked up by {time}", { time: clock(time) })}>
                    {clock(time)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {[0, 1].flatMap((team) => [
                <tr key={"team" + team} className="totals">
                  <td className="left">
                    <b style={{ color: LANE_TEAM_COLOR[team] }}>{LANE_TEAM_NAME[team]}</b>
                  </td>
                  <td>
                    <b>{report.players.filter((p) => p.team === team).reduce((sum, p) => sum + p.bridge_buffs.reduce((a, b) => a + b.amount, 0), 0)}</b>
                  </td>
                  {buffKinds.map((kind) => (
                    <td key={kind}>
                      {report.players.filter((p) => p.team === team).reduce((sum, p) => sum + (p.bridge_buffs.find((b) => b.name === kind)?.amount ?? 0), 0) || "–"}
                    </td>
                  ))}
                  {sampleTimes.map((t, i) => (
                    <td key={t}>{report.players.filter((p) => p.team === team).reduce((sum, p) => sum + (p.bridge_buffs_at[i] ?? 0), 0) || "–"}</td>
                  ))}
                </tr>,
                ...report.players
                  .filter((p) => p.team === team && p.bridge_buffs.length > 0)
                  .map((p) => (
                    <tr key={p.slot} onClick={() => onOpen(p.slot)}>
                      <td className="left">
                        <img src={p.hero_icon} alt="" loading="lazy" />
                        <span style={{ color: LANE_TEAM_COLOR[p.team] }}>{p.hero_name}</span>
                      </td>
                      <td>{p.bridge_buffs.reduce((a, b) => a + b.amount, 0)}</td>
                      {buffKinds.map((kind) => (
                        <td key={kind}>{p.bridge_buffs.find((b) => b.name === kind)?.amount ?? "–"}</td>
                      ))}
                      {sampleTimes.map((t, i) => (
                        <td key={t}>{p.bridge_buffs_at[i] || "–"}</td>
                      ))}
                    </tr>
                  )),
              ])}
            </tbody>
          </table>
        </div>
      )}
    </>
  );
}
