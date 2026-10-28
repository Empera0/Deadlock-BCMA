import { useEffect, useState } from "react";
import { api, fmt, Hero, LiveMatch, Scout } from "../api";
import ErrorNote from "./ErrorNote";
import { t } from "../i18n";

/** Tags that are only worth knowing, not a warning: a losing streak or a new hero. Matched in both languages. */
const soft = (tag: string) => /losses in a row|^New on this hero|^First match|mağlubiyet|^Bu kahramanda/.test(tag);

function PlayerCard({ s, icon, myLane, me }: { s: Scout; icon?: string; myLane: number; me: boolean }) {
  return (
    <div className={"scout" + (s.ally ? " ally" : " enemy")}>
      <div className="scout-head">
        {icon && <img src={icon} alt="" className="scout-hero" />}
        <div>
          <strong>{s.name}</strong>
          <small className="muted">
            {s.hero_name} · {s.rank}
            {me ? ` · ${t("you")}` : myLane > 0 && s.lane === myLane ? ` · ${s.ally ? t("laning with you") : t("in your lane")}` : ""}
          </small>
        </div>
      </div>
      <div className="scout-stats">
        <span title={t("Matches on this hero, and the win rate on it (pulled toward 50% while there are few)")}>
          {t("{n} on hero", { n: fmt(s.hero_matches) })} · <b>{fmt(s.hero_win_pct, 0)}%</b>
        </span>
        {s.hero_matches > 0 && (
          <span title={t("Kills, deaths and assists per match on this hero")}>
            {fmt(s.hero_kda[0], 1)}/{fmt(s.hero_kda[1], 1)}/{fmt(s.hero_kda[2], 1)}
          </span>
        )}
        <span className="muted">
          {t("{n} matches · {w}% overall", { n: fmt(s.matches), w: fmt(s.win_pct, 0) })}
        </span>
      </div>
      {s.most_played.length > 0 && (
        <div className="scout-most muted">{t("Plays most: {list}", { list: s.most_played.map(([h, n]) => `${h} (${n})`).join(", ") })}</div>
      )}
      {s.tags.length > 0 && (
        <div className="scout-tags">
          {s.tags.map((tag) => (
            <span key={tag} className={soft(tag) ? "soft" : "hot"}>
              {tag}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}

/** Both teams of the match being played, player by player: rank, record on the hero, most played heroes, streak, party. */
export default function LobbyScout({ live, heroes }: { live: LiveMatch; heroes: Hero[] }) {
  const [scouts, setScouts] = useState<Scout[] | null>(null);
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let stale = false;
    setScouts(null);
    setError("");
    api
      .scoutLobby(live.players)
      .then((s) => !stale && setScouts(s))
      .catch((e) => !stale && setError(String(e)));
    return () => {
      stale = true;
    };
  }, [live.match_id, live.players, attempt]);
  const icon = (id: number) => heroes.find((h) => h.id === id)?.icon;

  if (!live.players.length) return null;
  return (
    <div className="group lobby-scout">
      <h3>{t("Lobby · match {id}", { id: live.match_id })}</h3>
      <ErrorNote error={error} onRetry={() => setAttempt((a) => a + 1)} />
      {!scouts && !error && <p className="muted">{t("Looking up the players…")}</p>}
      {scouts && (
        <div className="lobby-teams">
          {[true, false].map((ally) => (
            <div key={String(ally)}>
              <h4>{ally ? t("Your team") : t("Enemy team")}</h4>
              {scouts
                .filter((s) => s.ally === ally)
                .map((s) => (
                  <PlayerCard key={s.account_id} s={s} icon={icon(s.hero)} myLane={scouts[0]?.lane ?? 0} me={s.account_id === scouts[0]?.account_id} />
                ))}
            </div>
          ))}
        </div>
      )}
      <p className="muted">
        {t("Records are from the statistics service's matches of each player. \"Likely queued with\" means the two are Steam friends on the same team.")}
      </p>
    </div>
  );
}
