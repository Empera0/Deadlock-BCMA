import { useEffect, useState } from "react";
import { Hero, MatchPreset } from "../api";
import { t } from "../i18n";
import { CONDEBUG_HELP } from "../live";
import { useLive } from "../liveMatch";
import { SETTINGS, useSetting } from "../settings";
import ErrorNote from "./ErrorNote";
import { LaneEnemies } from "./LaneNote";
import LobbyScout from "./LobbyScout";

/** Minutes and seconds since a time, ticking. */
function useElapsed(since: number): string {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(timer);
  }, []);
  const s = Math.max(0, Math.floor((now - since) / 1000));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

/** The match being played, followed on its own: both teams player by player, the note for the lane, and a way to
 *  plan the build for it. Read once, at the start of the match. */
export default function LiveView({ heroes, onPlan }: { heroes: Hero[]; onPlan: (preset: MatchPreset) => void }) {
  const live = useLive();
  const [, setTrack] = useSetting(SETTINGS.liveTrack);
  const elapsed = useElapsed(live.since);
  const byId = new Map(heroes.map((h) => [h.id, h]));
  const cls = (ids: number[]) => ids.map((id) => byId.get(id)?.class_name).filter((c): c is string => !!c);
  const plan = () => {
    const m = live.live;
    const me = m && byId.get(m.hero);
    if (!m || !me) return;
    onPlan({
      key: `live-${m.match_id}-${Date.now()}`,
      match_id: m.match_id,
      player_name: t("Live match"),
      hero: me.class_name,
      allies: cls(m.allies),
      enemies: cls(m.enemies),
      lane: cls(m.lane),
      net_worth: 0,
      won: false,
      bought: [],
    });
  };

  return (
    <div className="live-view">
      <div className="live-head group">
        <span className={"live-dot " + live.phase} />
        <div className="live-status">
          {live.phase === "off" && (
            <>
              <b>{t("Following your matches is off")}</b>
              <button onClick={() => setTrack("1")}>{t("Turn it on")}</button>
            </>
          )}
          {live.phase === "no-log" && (
            <>
              <b>{t("The game's console log was not found")}</b>
              <small className="muted">{CONDEBUG_HELP}</small>
            </>
          )}
          {live.phase === "waiting" && (
            <>
              <b>{t("Waiting for your match")}</b>
              <small className="muted">{t("When a match starts, its teams and players show up here on their own.")}</small>
            </>
          )}
          {live.phase === "loading" && (
            <>
              <b>{t("Match {id} started · {time}", { id: live.matchId ?? "", time: elapsed })}</b>
              <small className="muted">{t("Reading the teams from the match's broadcast… the first time takes about a minute.")}</small>
            </>
          )}
          {live.phase === "ready" && (
            <>
              <b>{t("Match {id} · {time}", { id: live.matchId ?? "", time: elapsed })}</b>
              <small className="muted">{t("Read once at the start; it does not follow the match.")}</small>
            </>
          )}
          {live.phase === "ended" && (
            <>
              <b>{t("Match {id} ended", { id: live.matchId ?? "" })}</b>
              <small className="muted">{t("Its analysis opens in Match analysis once it is out.")}</small>
            </>
          )}
        </div>
        {live.live && live.phase !== "ended" && <button onClick={plan}>{t("Plan my build")}</button>}
        {live.phase === "loading" && live.error && (
          <button className="ghost" onClick={live.retry}>
            {t("Try again")}
          </button>
        )}
      </div>
      {live.phase === "loading" && <ErrorNote error={live.error} />}
      {live.lane && live.lane.length > 0 && (
        <div className="group live-lane">
          <h3>{t("Your lane")}</h3>
          <div className="live-lane-list">
            <LaneEnemies enemies={live.lane} />
          </div>
        </div>
      )}
      {live.live && <LobbyScout live={live.live} heroes={heroes} />}
    </div>
  );
}
