import { useEffect, useState } from "react";
import { api, fmt, HABIT_GRID, HABIT_KINDS, Habits, MatchPlayer, MatchReport, MatchReview, PhaseHabits, VersusAverage } from "../api";
import { clock } from "./MatchCharts";
import { percent, t } from "../i18n";
import ErrorNote from "./ErrorNote";

const TEAM_COLOR = ["#f2b84b", "#4fc3f7"];
const KIND_COLORS = ["#e2574c", "#f2b84b", "#26a69a", "#6f7fe0", "#9fb4c4", "#6b6f7a", "#2b2b2b"];
const PHASE_NAMES = ["0–10 min", "10–20 min", "20+ min", "Whole match"];
const PHASE_HELP = ["Laning (0–10 min)", "Mid game (10–20 min)", "Late game (20+ min)", "Whole match"];
/** A difference in a kind of time is pointed out from this many percentage points on. */
const NOTABLE_PP = 5;

type Loaded<T> = { value: T } | { error: unknown } | null;

function useLoad<T>(load: () => Promise<T>, key: string): Loaded<T> {
  const [state, setState] = useState<Loaded<T>>(null);
  useEffect(() => {
    let live = true;
    setState(null);
    load().then(
      (value) => live && setState({ value }),
      (error) => live && setState({ error }),
    );
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return state;
}

const pct = (share: number) => percent(fmt(share * 100, 0));

/** Two bars of what the time went to: the player's and the top players'. */
function TimeBars({ mine, top }: { mine: PhaseHabits; top: PhaseHabits | null }) {
  const bar = (p: PhaseHabits) => (
    <span className="habit-bar">
      {p.time.map((share, i) => (
        <i key={i} style={{ width: `${share * 100}%`, background: KIND_COLORS[i] }} title={`${t(HABIT_KINDS[i])}: ${pct(share)}`} />
      ))}
    </span>
  );
  const minutes = (share: number) => (share * mine.seconds) / 60;
  return (
    <>
      <div className="habit-bar-row">
        <b>{t("You")}</b>
        {bar(mine)}
      </div>
      {top && (
        <div className="habit-bar-row">
          <b>{t("Top players")}</b>
          {bar(top)}
        </div>
      )}
      <table className="match-table habit-table">
        <thead>
          <tr>
            <th className="left">{t("Time spent")}</th>
            <th>{t("You")}</th>
            {top && <th>{t("Top players")}</th>}
            {top && <th>{t("Difference")}</th>}
          </tr>
        </thead>
        <tbody>
          {HABIT_KINDS.map((kind, i) => {
            const diff = top ? mine.time[i] - top.time[i] : 0;
            const notable = top !== null && Math.abs(diff) * 100 >= NOTABLE_PP;
            return (
              <tr key={kind}>
                <td className="left">
                  <i className="habit-swatch" style={{ background: KIND_COLORS[i] }} />
                  {t(kind)}
                </td>
                <td>
                  {pct(mine.time[i])} <small className="muted">({clock(mine.time[i] * mine.seconds)})</small>
                </td>
                {top && <td>{pct(top.time[i])}</td>}
                {top && (
                  <td className={notable ? "habit-notable" : "muted"}>
                    {diff >= 0 ? "+" : "−"}
                    {t("{n} min", { n: fmt(Math.abs(minutes(diff)), 1) })}
                  </td>
                )}
              </tr>
            );
          })}
          {mine.unspent !== null && (
            <tr>
              <td className="left">{t("Souls carried unspent, on average")}</td>
              <td>{fmt(mine.unspent, 0)}</td>
              {top && <td>{top.unspent !== null ? fmt(top.unspent, 0) : "–"}</td>}
              {top && <td className={top.unspent !== null && mine.unspent - top.unspent > 500 ? "habit-notable" : "muted"}>{top.unspent !== null ? (mine.unspent >= top.unspent ? "+" : "−") + fmt(Math.abs(mine.unspent - top.unspent), 0) : ""}</td>}
            </tr>
          )}
        </tbody>
      </table>
    </>
  );
}

/** Sentences for the biggest differences from the top players in one part of the match. */
function timeNotes(mine: PhaseHabits, top: PhaseHabits): string[] {
  const out = HABIT_KINDS.map((kind, i) => ({ kind, diff: mine.time[i] - top.time[i] }))
    .filter((d) => Math.abs(d.diff) * 100 >= NOTABLE_PP)
    .sort((a, b) => Math.abs(b.diff) - Math.abs(a.diff))
    .slice(0, 3)
    .map((d) =>
      t(d.diff > 0 ? "{n} minutes more on “{kind}” than top players." : "{n} minutes less on “{kind}” than top players.", {
        n: fmt((Math.abs(d.diff) * mine.seconds) / 60, 1),
        kind: t(d.kind),
      }),
    );
  if (mine.unspent !== null && top.unspent !== null && mine.unspent - top.unspent > 500) out.push(t("You carried {n} more unspent souls on average: buy sooner.", { n: fmt(mine.unspent - top.unspent, 0) }));
  return out;
}

/** Where on the map the time went, cell by cell, over the minimap turned so the own base is at the bottom. */
function Heat({ phase, image, flip, label }: { phase: PhaseHabits; image: string; flip: boolean; label: string }) {
  const top = Math.max(...phase.heat, 1e-9);
  const cell = 1000 / HABIT_GRID;
  return (
    <figure className="habit-heat">
      <svg viewBox="0 0 1000 1000" className="map">
        <image href={image} width={1000} height={1000} className="map-image-bright" transform={flip ? "translate(0 1000) scale(1 -1)" : undefined} />
        {phase.heat.map((share, i) =>
          share > 0 ? (
            <rect
              key={i}
              x={(i % HABIT_GRID) * cell}
              y={Math.floor(i / HABIT_GRID) * cell}
              width={cell}
              height={cell}
              fill="#ff6a3d"
              opacity={Math.min(0.85, Math.sqrt(share / top) * 0.85)}
            >
              <title>{pct(share)}</title>
            </rect>
          ) : null,
        )}
      </svg>
      <figcaption>
        <b>{label}</b>
        <small>
          {t("Own base {a} · own half {b} · enemy half {c}", { a: pct(phase.base), b: pct(phase.own_half), c: pct(phase.enemy_half) })}
        </small>
      </figcaption>
    </figure>
  );
}

function heatNotes(mine: PhaseHabits, top: PhaseHabits): string[] {
  const out: string[] = [];
  const diff = (mine.enemy_half - top.enemy_half) * 100;
  if (diff <= -8) out.push(t("You spent {n} points less of your time on the enemy's half than top players: push out and take space.", { n: fmt(-diff, 0) }));
  if (diff >= 8) out.push(t("You spent {n} points more of your time on the enemy's half than top players: watch for getting caught.", { n: fmt(diff, 0) }));
  const base = (mine.base - top.base) * 100;
  if (base >= 4) out.push(t("You spent {n} points more of your time in your base than top players.", { n: fmt(base, 0) }));
  return out;
}

/** Health over the laning phase: the player and each lane opponent. */
function LaneHealth({ review, report }: { review: MatchReview; report: MatchReport }) {
  const W = 600;
  const H = 120;
  const lane = review.lane;
  const n = lane.health[0]?.[1].length ?? 0;
  if (n < 2) return null;
  const bySlot = new Map(report.players.map((p) => [p.slot, p]));
  const me = lane.health[0][0];
  const player = bySlot.get(me);
  const x = (i: number) => (i / (n - 1)) * W;
  const y = (v: number) => H - (v / 100) * H;
  return (
    <svg className="lane-health" viewBox={`0 -4 ${W} ${H + 8}`} preserveAspectRatio="none">
      {[1, 2, 3, 4, 5, 6, 7, 8, 9].map((m) => (
        <line key={m} x1={x((m * 60) / 5)} x2={x((m * 60) / 5)} y1={0} y2={H} stroke="var(--line)" />
      ))}
      {lane.health.map(([slot, values]) => (
        <polyline
          key={slot}
          points={values.map((v, i) => `${x(i)},${y(v)}`).join(" ")}
          fill="none"
          stroke={slot === me ? "#fff" : "#ff6a3d"}
          strokeOpacity={slot === me ? 0.95 : 0.6}
          strokeWidth={slot === me ? 2 : 1.4}
          vectorEffect="non-scaling-stroke"
        >
          <title>{bySlot.get(slot)?.hero_name}</title>
        </polyline>
      ))}
      {player?.deaths_at
        .filter((d) => d.time < n * 5)
        .map((d, i) => <circle key={"d" + i} cx={x(d.time / 5)} cy={H} r={4} fill="#ff5a52" />)}
      {player?.kills_at
        .filter((k) => k.time < n * 5)
        .map((k, i) => <circle key={"k" + i} cx={x(k.time / 5)} cy={0} r={4} fill="#7cb342" />)}
    </svg>
  );
}

function LaneCard({ review, report, player }: { review: MatchReview; report: MatchReport; player: MatchPlayer }) {
  const lane = review.lane;
  const names = lane.opponents.map((o) => report.players.find((p) => p.slot === o)?.hero_name ?? "?").join(", ");
  if (lane.opponents.length === 0) return <p className="muted">{t("No lane opponent was found for this player.")}</p>;
  const turn = lane.turn;
  return (
    <>
      <p className="muted">
        {t("Against {names}. The white line is your health, the orange ones theirs; red dots are your deaths, green ones your kills.", { names })}
      </p>
      <LaneHealth review={review} report={report} />
      {turn && Math.abs(turn[2]) >= 300 && (
        <p className={turn[2] > 0 ? "good" : "bad"}>
          {turn[2] > 0
            ? t("The lane turned your way between {a} and {b}: you gained {n} souls on them.", { a: clock(turn[0]), b: clock(turn[1]), n: fmt(turn[2], 0) })
            : t("The lane turned against you between {a} and {b}: you fell {n} souls behind them.", { a: clock(turn[0]), b: clock(turn[1]), n: fmt(-turn[2], 0) })}
        </p>
      )}
      <table className="match-table habit-table">
        <thead>
          <tr>
            <th className="left">{t("Minute")}</th>
            <th title={t("Health lost in the minute, in percent of the bar added up: 100 is a full bar.")}>{t("Health you lost")}</th>
            <th>{t("Health they lost")}</th>
            <th title={t("Seconds spent fighting heroes in the minute.")}>{t("Trading")}</th>
            <th>{t("Kills / deaths")}</th>
          </tr>
        </thead>
        <tbody>
          {lane.minutes.map((m) => (
            <tr key={m.minute}>
              <td className="left">
                {m.minute}–{m.minute + 1}
              </td>
              <td className={m.lost > m.their_lost + 25 ? "bad" : ""}>{percent(fmt(m.lost, 0))}</td>
              <td className={m.their_lost > m.lost + 25 ? "good" : ""}>{percent(fmt(m.their_lost, 0))}</td>
              <td>{t("{n}s", { n: m.fighting })}</td>
              <td>
                {m.kills > 0 && <b className="good">+{m.kills}</b>} {m.deaths > 0 && <b className="bad">−{m.deaths}</b>}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <table className="match-table habit-table">
        <thead>
          <tr>
            <th className="left">{t("Game's snapshot")}</th>
            <th>{t("Souls")}</th>
            <th>{t("Last hits")}</th>
            <th>{t("Damage dealt to them")}</th>
            <th>{t("Damage taken from them")}</th>
          </tr>
        </thead>
        <tbody>
          {lane.marks.map((m) => (
            <tr key={m.time}>
              <td className="left">{clock(m.time)}</td>
              <td className={m.souls >= m.their_souls ? "good" : "bad"}>
                {fmt(m.souls, 0)} <small className="muted">/ {fmt(m.their_souls, 0)}</small>
              </td>
              <td>
                {fmt(m.last_hits, 0)} <small className="muted">/ {fmt(m.their_last_hits, 1)}</small>
              </td>
              <td>{fmt(m.dealt, 0)}</td>
              <td className={m.taken > m.dealt * 1.3 ? "bad" : ""}>{fmt(m.taken, 0)}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="muted">{t("The game records souls and damage only every three minutes; health is recorded every second, so trades show minute by minute. Their numbers are the average of the lane opponents.")}</p>
      {player.lane_opponents.length > 1 && <p className="muted">{t("Two opponents: their health lost is the average of both.")}</p>}
    </>
  );
}

function AverageCard({ v }: { v: VersusAverage }) {
  return (
    <>
      <p className="muted">
        {t("Your average from {n} matches on this hero ({w} won). Green is better than your average, red worse.", { n: v.matches, w: v.wins })}
      </p>
      <table className="match-table habit-table">
        <thead>
          <tr>
            <th className="left" />
            <th>{t("This match")}</th>
            <th>{t("Your average")}</th>
            <th>{t("Difference")}</th>
          </tr>
        </thead>
        <tbody>
          {v.rows.map((r) => {
            const diff = r.this - r.average;
            const rel = r.average !== 0 ? (diff / Math.abs(r.average)) * 100 : 0;
            const good = r.higher_is_better ? diff > 0 : diff < 0;
            const tone = Math.abs(rel) < 8 ? "muted" : good ? "good" : "bad";
            const digits = r.average < 10 ? 2 : 0;
            return (
              <tr key={r.label}>
                <td className="left">{r.label}</td>
                <td>{fmt(r.this, digits)}</td>
                <td>{fmt(r.average, digits)}</td>
                <td className={tone}>
                  {rel >= 0 ? "+" : "−"}
                  {percent(fmt(Math.abs(rel), 0))}
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </>
  );
}

/** A player's habits in the match: time, places, the lane minute by minute and the match beside their average. */
export default function MatchHabits({ report, slot, onPick }: { report: MatchReport; slot: number; onPick: (slot: number) => void }) {
  const player = report.players.find((p) => p.slot === slot) ?? report.players[0];
  const key = `${report.match_id}-${player.slot}`;
  const review = useLoad(() => api.matchReview(report.match_id, player.slot), key);
  const top = useLoad<Habits>(() => api.topHabits(report.match_id, player.slot), key);
  const average = useLoad(() => api.heroAverage(report.match_id, player.slot), key);
  const [phase, setPhase] = useState(3);
  const [heatPhase, setHeatPhase] = useState(0);
  const mine = review && "value" in review ? review.value : null;
  const ref = top && "value" in top ? top.value : null;
  const refPhase = (i: number) => (ref && ref.phases[i]?.seconds > 0 ? ref.phases[i] : null);

  const phaseTabs = (value: number, set: (n: number) => void, whole: boolean) => (
    <div className="side-tabs habit-phases">
      {PHASE_NAMES.slice(0, whole ? 4 : 3).map((name, i) => (
        <button key={name} className={i === value ? "on" : ""} title={t(PHASE_HELP[i])} disabled={!!mine && !(mine.habits.phases[i]?.seconds > 0)} onClick={() => set(i)}>
          {t(name)}
        </button>
      ))}
    </div>
  );
  const topNote =
    top === null ? (
      <p className="muted">{t("Reading top players' matches on this hero…")}</p>
    ) : "error" in top ? (
      <ErrorNote error={top.error} />
    ) : (
      <p className="muted">{t("Top players: {n} recent high-rank matches on this hero, in the same lane when there were some. Read once a week.", { n: top.value.matches })}</p>
    );

  return (
    <div className="habits">
      <div className="player-picker">
        {report.players.map((p) => (
          <button key={p.slot} className={p.slot === player.slot ? "on" : ""} onClick={() => onPick(p.slot)} title={p.hero_name}>
            <img src={p.hero_icon} alt={p.hero_name} style={{ borderColor: TEAM_COLOR[p.team] }} />
          </button>
        ))}
      </div>
      {review === null && <p className="muted">{t("Reading the match…")}</p>}
      {review && "error" in review && <ErrorNote error={review.error} />}
      {mine && (
        <div className="habit-cards">
          <section className="group habit-card">
            <h3>{t("Where the time went")}</h3>
            {phaseTabs(phase, setPhase, true)}
            {mine.habits.phases[phase] && <TimeBars mine={mine.habits.phases[phase]} top={refPhase(phase)} />}
            {refPhase(phase) && (
              <ul className="habit-notes">
                {timeNotes(mine.habits.phases[phase], refPhase(phase) as PhaseHabits).map((n) => (
                  <li key={n}>{n}</li>
                ))}
              </ul>
            )}
            {topNote}
          </section>

          <section className="group habit-card">
            <h3>{t("Where on the map")}</h3>
            {phaseTabs(heatPhase, setHeatPhase, false)}
            <div className="habit-heats">
              <Heat phase={mine.habits.phases[heatPhase]} image={report.map.image} flip={player.team === 1} label={t("You")} />
              {refPhase(heatPhase) && <Heat phase={refPhase(heatPhase) as PhaseHabits} image={report.map.image} flip={player.team === 1} label={t("Top players")} />}
            </div>
            {refPhase(heatPhase) && (
              <ul className="habit-notes">
                {heatNotes(mine.habits.phases[heatPhase], refPhase(heatPhase) as PhaseHabits).map((n) => (
                  <li key={n}>{n}</li>
                ))}
              </ul>
            )}
            <p className="muted">{t("Your base is at the bottom on both maps. Brighter cells are where more time was spent alive.")}</p>
          </section>

          <section className="group habit-card">
            <h3>{t("The lane minute by minute")}</h3>
            <LaneCard review={mine} report={report} player={player} />
          </section>

          <section className="group habit-card">
            <h3>{t("This match and your average")}</h3>
            {average === null && <p className="muted">{t("Reading the player's record…")}</p>}
            {average && "error" in average && <ErrorNote error={average.error} />}
            {average && "value" in average && (average.value ? <AverageCard v={average.value} /> : <p className="muted">{t("No earlier matches of this player on this hero were found.")}</p>)}
          </section>
        </div>
      )}
    </div>
  );
}
