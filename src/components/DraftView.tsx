import { useEffect, useMemo, useState } from "react";
import { findLiveMatch } from "../live";
import { RANKS, SETTINGS, useSetting } from "../settings";
import LobbyScout from "./LobbyScout";
import { api, LiveMatch, DraftPick, DraftReason, DraftReport, fmt, Hero, MatchPreset } from "../api";
import ErrorNote from "./ErrorNote";


const TEAM_SIZE = 6;
/** With an account, "my heroes" are the ones played at least this often. */
const MY_HERO_MATCHES = 3;
const SHOWN = 12;

type Side = "allies" | "enemies" | "banned";
const SIDES: [Side, string][] = [
  ["allies", "Your team"],
  ["enemies", "Enemy team"],
  ["banned", "Bans"],
];

const signed = (v: number, digits = 1) => `${v >= 0 ? "+" : "−"}${fmt(Math.abs(v), digits)}`;

/** Reasons colored for the player's team: what helps an enemy hero is bad news. */
function Reasons({ reasons, enemy = false }: { reasons: DraftReason[]; enemy?: boolean }) {
  if (!reasons.length) return null;
  return (
    <ul className="draft-reasons">
      {reasons.map((r) => (
        <li key={r.label} className={r.points >= 0 !== enemy ? "good" : "bad"} title={`${fmt(r.matches)} matches`}>
          {r.label} <b>{signed(r.points, 2)}</b>
        </li>
      ))}
    </ul>
  );
}

function PickRow({ pick, rank, onPlan }: { pick: DraftPick; rank: number; onPlan: () => void }) {
  return (
    <div className="draft-pick">
      <span className="draft-rank">{rank}</span>
      <img src={pick.icon} alt="" />
      <div className="draft-pick-main">
        <div className="draft-pick-head">
          <strong>{pick.name}</strong>
          <span className="muted">{pick.hero_type}</span>
          <b className={pick.edge >= 0 ? "good" : "bad"} title="Win rate points the pick adds to your team's chance">
            {signed(pick.edge, 2)} pts
          </b>
          {pick.team_rate !== null && <span className="muted">team {fmt(pick.team_rate, 1)}%</span>}
        </div>
        <div className="draft-parts muted">
          Strength {signed(pick.strength, 2)} · matchups {signed(pick.counter, 2)} · lane {signed(pick.lane, 2)}
          {pick.mine && ` · your record ${signed(pick.comfort, 2)} (${fmt(pick.mine[1], 0)}% in ${pick.mine[0]})`}
        </div>
        <Reasons reasons={pick.reasons} />
      </div>
      <button className="ghost small" onClick={onPlan} title="Open the master builder for this hero with both teams and the lane">
        Plan build
      </button>
    </div>
  );
}

/** Which hero to pick into the enemies, what to ban, and how the draft stands. */
export default function DraftView({ heroes, onPlan }: { heroes: Hero[]; onPlan: (preset: MatchPreset) => void }) {
  const [teams, setTeams] = useState<Record<Side, number[]>>({ allies: [], enemies: [], banned: [] });
  const [lane, setLane] = useState<number[]>([]);
  const [side, setSide] = useState<Side>("enemies");
  const [badgeSetting, setBadgeSetting] = useSetting(SETTINGS.draftBadge);
  const badge = Number(badgeSetting) || 0;
  const [account] = useSetting(SETTINGS.account);
  const [onlyMine, setOnlyMine] = useState(false);
  const [liveNote, setLiveNote] = useState("");
  const [live, setLive] = useState<LiveMatch | null>(null);
  const [query, setQuery] = useState("");
  const [report, setReport] = useState<DraftReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const byId = useMemo(() => new Map(heroes.map((h) => [h.id, h])), [heroes]);
  const placed = new Set([...teams.allies, ...teams.enemies, ...teams.banned]);

  // The statistics are cached, so the draft is worked out again on every change.
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let stale = false;
    const timer = setTimeout(() => {
      setBusy(true);
      setError("");
      api
        .draftPicks(teams.allies, teams.enemies, lane, teams.banned, badge, account.trim() || null)
        .then((r) => !stale && setReport(r))
        .catch((e) => !stale && setError(String(e)))
        .finally(() => !stale && setBusy(false));
    }, 300);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [teams, lane, badge, account, attempt]);

  const add = (id: number) => {
    if (placed.has(id)) return;
    const limit = side === "allies" ? TEAM_SIZE - 1 : side === "enemies" ? TEAM_SIZE : 12;
    if (teams[side].length >= limit) return;
    setTeams({ ...teams, [side]: [...teams[side], id] });
  };
  const remove = (s: Side, id: number) => {
    setTeams({ ...teams, [s]: teams[s].filter((x) => x !== id) });
    setLane(lane.filter((x) => x !== id));
  };
  const toggleLane = (id: number) => setLane(lane.includes(id) ? lane.filter((x) => x !== id) : [...lane, id]);
  const plan = (pick: DraftPick) => {
    const cls = (ids: number[]) => ids.map((id) => byId.get(id)?.class_name).filter((c): c is string => !!c);
    onPlan({
      key: `draft-${pick.hero}-${Date.now()}`,
      match_id: 0,
      player_name: "Draft",
      hero: pick.hero,
      allies: cls(teams.allies),
      enemies: cls(teams.enemies),
      lane: cls(lane),
      net_worth: 0,
      won: false,
      bought: [],
    });
  };

  const shown = useMemo(() => {
    const picks = report?.picks ?? [];
    const mine = onlyMine ? picks.filter((p) => (p.mine?.[0] ?? 0) >= MY_HERO_MATCHES) : picks;
    return mine.slice(0, SHOWN);
  }, [report, onlyMine]);
  // Heroes with too few matches to be in the game yet are left out once the statistics are in.
  const playable = report ? new Set([...report.picks.map((p) => p.hero_id), ...placed]) : null;
  const grid = heroes.filter((h) => (!playable || playable.has(h.id)) && (!query || h.name.toLowerCase().includes(query.toLowerCase())));

  return (
    <div className="draft-view">
      <div className="draft-bar">
        <label className="field inline">
          Rank
          <select
            value={badge}
            onChange={(e) => setBadgeSetting(e.target.value)}
          >
            {RANKS.map(([b, label]) => (
              <option key={b} value={b}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <span className="muted" title="Your own record on each hero comes from this account. Set it in Settings.">
          {account.trim() ? `Your account: ${account.trim()}` : "No account set: add it in Settings for your own record"}
        </span>
        <button className="ghost" onClick={() => (setTeams({ allies: [], enemies: [], banned: [] }), setLane([]))}>
          Clear
        </button>
        <button
          className="ghost"
          title="Fills both teams and your lane from the match you are playing now, read from the game's console log (-condebug)"
          onClick={() => {
            setLiveNote("Looking for your match… the teams take about a minute the first time.");
            findLiveMatch()
              .then((live) => {
                setLive(live);
                setTeams({ allies: live.allies, enemies: live.enemies, banned: [] });
                setLane(live.lane);
                setLiveNote(`Match ${live.match_id}: both teams loaded`);
              })
              .catch((e) => setLiveNote(String(e)));
          }}
        >
          Load my live match
        </button>
        {liveNote && <span className="muted">{liveNote}</span>}
        {busy && <span className="muted">Working…</span>}
      </div>

      <div className="draft-layout">
        <div className="draft-left">
          {SIDES.map(([s, label]) => (
            <div key={s} className={"draft-side " + s + (side === s ? " on" : "")} onClick={() => setSide(s)}>
              <div className="draft-side-head">
                <strong>{label}</strong>
                <span className="muted">
                  {s === "allies" ? `${teams.allies.length}/${TEAM_SIZE - 1} besides you` : s === "enemies" ? `${teams.enemies.length}/${TEAM_SIZE}` : teams.banned.length}
                </span>
                {side === s && <span className="draft-adding">adding here</span>}
              </div>
              <div className="draft-chips">
                {teams[s].map((id) => {
                  const h = byId.get(id);
                  if (!h) return null;
                  const inLane = lane.includes(id);
                  return (
                    <span key={id} className={"draft-chip" + (inLane ? " lane" : "")}>
                      <img src={h.icon} alt="" />
                      <span>{h.name}</span>
                      {s === "enemies" && (
                        <button
                          className="ghost small"
                          onClick={(e) => (e.stopPropagation(), toggleLane(id))}
                          title="Mark as an enemy in your lane"
                        >
                          {inLane ? "in lane" : "lane?"}
                        </button>
                      )}
                      <button className="ghost small" onClick={(e) => (e.stopPropagation(), remove(s, id))} title="Remove">
                        ×
                      </button>
                    </span>
                  );
                })}
                {teams[s].length === 0 && <span className="muted">Click a hero below to add it here.</span>}
              </div>
            </div>
          ))}

          <input className="draft-search" placeholder="Find a hero…" value={query} onChange={(e) => setQuery(e.target.value)} />
          <div className="draft-grid">
            {grid.map((h) => (
              <button key={h.id} className={"draft-hero" + (placed.has(h.id) ? " used" : "")} disabled={placed.has(h.id)} onClick={() => add(h.id)} title={h.name}>
                <img src={h.icon} alt="" loading="lazy" />
                <span>{h.name}</span>
              </button>
            ))}
          </div>
        </div>

        <div className="draft-right">
          {live && <LobbyScout live={live} heroes={heroes} />}
          <ErrorNote error={error} onRetry={() => setAttempt((a) => a + 1)} />
          {report && (
            <>
              {report.team_rate !== null && (
                <div className="draft-chance">
                  <span>Draft as it stands</span>
                  <div className="draft-meter">
                    <div style={{ width: `${report.team_rate}%` }} />
                  </div>
                  <b>{fmt(report.team_rate, 1)}%</b>
                </div>
              )}
              {report.notes.map((n) => (
                <div key={n} className="passive">
                  {n}
                </div>
              ))}

              <div className="group">
                <h3>
                  Best picks
                  {account.trim() && (
                    <label className="check">
                      <input type="checkbox" checked={onlyMine} onChange={(e) => setOnlyMine(e.target.checked)} />
                      Only heroes I play ({MY_HERO_MATCHES}+ matches)
                    </label>
                  )}
                </h3>
                {shown.length === 0 && <p className="muted">No hero fits.</p>}
                {shown.map((p, i) => (
                  <PickRow key={p.hero_id} pick={p} rank={i + 1} onPlan={() => plan(p)} />
                ))}
              </div>

              {report.threats.length > 0 && (
                <div className="group">
                  <h3>Enemy picks, the most dangerous first</h3>
                  {report.threats.map((t) => (
                    <div key={t.hero_id} className="draft-threat">
                      <img src={t.icon} alt="" />
                      <strong>{t.name}</strong>
                      <b className={t.edge > 0 ? "bad" : "good"}>{signed(t.edge, 2)} pts</b>
                      <Reasons reasons={t.reasons} enemy />
                    </div>
                  ))}
                </div>
              )}

              {report.bans.length > 0 && (
                <div className="group">
                  <h3>Bans</h3>
                  {report.bans.map((b) => (
                    <div key={b.hero_id} className="draft-threat">
                      <img src={b.icon} alt="" />
                      <strong>{b.name}</strong>
                      <b className="bad">{signed(b.edge, 2)} pts</b>
                      <span className="muted">{fmt(b.ban_share, 1)}% of bans</span>
                      <Reasons reasons={b.reasons} enemy />
                    </div>
                  ))}
                </div>
              )}

              <p className="muted">
                Win rates from the last 30 days of matches at the chosen rank; your own record from your last 90 days. A pick counts its own
                win rate and its matchups against every enemy (and, in your lane, its lane results), weighted as fitted on 8,700 Ascendant+
                matches: with these weights, drafts given 45% won 45% and drafts given 55% won 56%. Pairs of allies did not predict wins and are
                not counted. A draft moves a match by a few points at most.
              </p>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
