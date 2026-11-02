import { useEffect, useRef, useState } from "react";
import { api, fmt, Hero, LaneEnemy } from "../api";
import { percent, t } from "../i18n";
import { savedAccount } from "../live";
import { SETTINGS, useSetting } from "../settings";

/** How often the console log is read for a match that started, ms. */
const EVERY_MS = 30_000;
/** Late-game strength (win rate points, long matches over short) that is worth a word. */
const LATE_PP = 2;

const signed = (v: number) => `${v >= 0 ? "+" : "−"}${fmt(Math.abs(v), 1)}`;

/**
 * When a match starts, a card for the player's lane: how the player's hero has done against each lane enemy, when
 * that enemy is strongest, and what players buy against it. Read once, at the start; it does not follow the match.
 */
export default function LaneNote({ heroes }: { heroes: Hero[] }) {
  const [setting] = useSetting(SETTINGS.laneNote);
  const [badgeSetting] = useSetting(SETTINGS.draftBadge);
  const [maxSetting] = useSetting(SETTINGS.draftMaxBadge);
  const on = setting !== "0";
  const [note, setNote] = useState<{ hero: number; enemies: LaneEnemy[] } | null>(null);
  // The match the note was made for, so it is made once.
  const done = useRef<number | null>(null);

  useEffect(() => {
    if (!on) return;
    let busy = false;
    const tick = async () => {
      if (busy) return;
      busy = true;
      try {
        const c = await api.consoleMatch();
        const account = c.account_id ? String(c.account_id) : savedAccount();
        if (!c.found || !c.match_id || done.current === c.match_id || !account) return;
        done.current = c.match_id;
        try {
          const live = await api.matchTeams(c.match_id, account);
          if (live.lane.length === 0) return;
          const enemies = await api.laneNote(live.hero, live.lane, Number(badgeSetting) || 0, maxSetting ? Number(maxSetting) : null);
          if (enemies.length > 0) setNote({ hero: live.hero, enemies });
        } catch {
          // The teams could not be read (Valve not broadcasting): no note for this match.
        }
      } finally {
        busy = false;
      }
    };
    tick();
    const timer = window.setInterval(tick, EVERY_MS);
    return () => window.clearInterval(timer);
  }, [on, badgeSetting, maxSetting]);

  if (!note) return null;
  const mine = heroes.find((h) => h.id === note.hero);
  return (
    <div className="lane-note" role="status">
      <div className="lane-note-head">
        <b>{t("Your lane")}</b>
        {mine && <span className="muted">{t("as {hero}", { hero: mine.name })}</span>}
        <button className="ghost small" onClick={() => setNote(null)} title={t("Close")}>
          ×
        </button>
      </div>
      {note.enemies.map((e) => (
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
    </div>
  );
}
