import { useState } from "react";
import { fmt, Hero, LaneEnemy } from "../api";
import { percent, t } from "../i18n";
import { useLive } from "../liveMatch";
import { SETTINGS, useSetting } from "../settings";

/** Late-game strength (win rate points, long matches over short) that is worth a word. */
const LATE_PP = 2;

const signed = (v: number) => `${v >= 0 ? "+" : "−"}${fmt(Math.abs(v), 1)}`;

/** Each enemy in the player's lane: how the player's hero has done against it, when it is strongest, and what is
 *  bought against it. */
export function LaneEnemies({ enemies }: { enemies: LaneEnemy[] }) {
  return (
    <>
      {enemies.map((e) => (
        <div key={e.hero_id} className="lane-note-enemy">
          <img src={e.icon} alt="" />
          <div>
            <strong>{e.name}</strong>
            <span>
              {e.lane.rate !== null ? (
                <span title={t("{n} matches · {p} pts beyond expected", { n: fmt(e.lane.matches), p: signed(e.lane.points) })}>
                  {t("In lane you win {n}", { n: percent(fmt(e.lane.rate, 0)) })}
                </span>
              ) : (
                <span className="muted">{t("Few lane matches between the two")}</span>
              )}
              {e.overall.rate !== null && <span className="muted"> · {t("over the match {n}", { n: percent(fmt(e.overall.rate, 0)) })}</span>}
            </span>
            {e.late !== null && Math.abs(e.late) >= LATE_PP && (
              <small className={e.late > 0 ? "warn" : "muted"}>{e.late > 0 ? t("Grows strong late: press the lane early.") : t("Strongest early: play the lane safe.")}</small>
            )}
            {e.counters.length > 0 && (
              <div className="lane-note-items">
                <small className="muted">{t("Bought against it:")}</small>
                {e.counters.map((i) => (
                  <span key={i.item} title={t("{name}: {p} pts more wins than usual against this hero", { name: i.name, p: signed(i.win_pp) })}>
                    <img src={i.icon} alt="" />
                    {i.name}
                  </span>
                ))}
              </div>
            )}
          </div>
        </div>
      ))}
    </>
  );
}

/**
 * When a match starts, a card for the player's lane, from the match the app follows (read once, at the start; it
 * does not follow the match). Not shown on the Live screen, which shows the same.
 */
export default function LaneNote({ heroes, hidden }: { heroes: Hero[]; hidden: boolean }) {
  const [setting] = useSetting(SETTINGS.laneNote);
  const live = useLive();
  // The match whose card was closed.
  const [closed, setClosed] = useState<number | null>(null);
  if (setting === "0" || hidden || live.phase !== "ready" || !live.lane?.length || closed === live.matchId) return null;
  const mine = heroes.find((h) => h.id === live.live?.hero);
  return (
    <div className="lane-note" role="status">
      <div className="lane-note-head">
        <b>{t("Your lane")}</b>
        {mine && <span className="muted">{t("as {hero}", { hero: mine.name })}</span>}
        <button className="ghost small" onClick={() => setClosed(live.matchId)} title={t("Close")}>
          ×
        </button>
      </div>
      <LaneEnemies enemies={live.lane} />
    </div>
  );
}
