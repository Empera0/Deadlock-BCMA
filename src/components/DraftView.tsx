import { useEffect, useMemo, useState } from "react";
import { findLiveMatch } from "../live";
import { SETTINGS, useSetting } from "../settings";
import RankRange from "./RankRange";
import LobbyScout from "./LobbyScout";
import { api, LiveMatch, DraftFocus, DraftPick, DraftReason, DraftReport, fmt, Hero, MatchPreset } from "../api";
import ErrorNote from "./ErrorNote";
import { t } from "../i18n";

const TEAM_SIZE = 6;
/** With an account, "my heroes" are the ones played at least this often. */
const MY_HERO_MATCHES = 3;
const SHOWN = 12;

type Side = "allies" | "enemies" | "banned";
/** The draft sides; translated where shown. */
const SIDES: [Side, string][] = [
  ["allies", "Your team"],
  ["enemies", "Enemy team"],
  ["banned", "Bans"],
];

const signed = (v: number, digits = 1) => `${v >= 0 ? "+" : "−"}${fmt(Math.abs(v), digits)}`;
/** A fit smaller than this (in win rate points) is shown plain. */
const FIT_PP = 0.5;

/** How every other hero has done beside the placed allies and against the placed enemies, the best fit first. */
function FitTable({ fits, byId, placed, onAdd }: { fits: DraftFocus[]; byId: Map<number, Hero>; placed: Set<number>; onAdd: (id: number) => void }) {
  const rows = useMemo(() => {
    const ids = new Set(fits.flatMap((f) => f.fits.map((x) => x.hero_id)));
    const allies = new Set(fits.filter((f) => f.ally).map((f) => f.hero_id));
    // The heroes still free, and the allies with each other; enemies and bans have no row.
    return [...ids]
      .filter((id) => byId.has(id) && (!placed.has(id) || allies.has(id)))
      .map((id) => {
        const cells = fits.map((f) => (f.hero_id === id ? null : (f.fits.find((x) => x.hero_id === id) ?? null)));
        return { id, ally: allies.has(id), cells, total: cells.reduce((sum, c) => sum + (c?.points ?? 0), 0) };
      })
      .sort((a, b) => Number(b.ally) - Number(a.ally) || b.total - a.total);
  }, [fits, byId, placed]);
  return (
    <div className="draft-fit-wrap">
      <table className="draft-fit">
        <thead>
          <tr>
            <th />
            {fits.map((f) => (
              <th key={f.hero_id} className={f.ally ? "ally" : "enemy"} title={f.ally ? t("Win rate together, on the same team") : t("Win rate against this enemy")}>
                <img src={byId.get(f.hero_id)?.icon} alt="" />
                <small>{f.ally ? t("with") : t("against")}</small>
              </th>
            ))}
            <th title={t("Win rate points beyond what the heroes' own win rates make expected, added up")}>{t("Fit")}</th>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => {
            const h = byId.get(r.id)!;
            const used = placed.has(r.id);
            return (
              <tr key={r.id} className={r.ally ? "ally" : ""}>
                <td>
                  <button className="ghost small" disabled={used} onClick={() => onAdd(r.id)} title={used ? h.name : t("Add {name} to the side you are adding to", { name: h.name })}>
                    <img src={h.icon} alt="" />
                    {h.name}
                  </button>
                </td>
                {r.cells.map((c, i) => (
                  <td
                    key={fits[i].hero_id}
                    className={c && c.points >= FIT_PP ? "good" : c && c.points <= -FIT_PP ? "bad" : "muted"}
                    title={c?.rate != null ? t("{n} matches · {p} pts beyond expected", { n: fmt(c.matches), p: signed(c.points, 1) }) : undefined}
                  >
                    {c === null ? "" : c.rate === null ? "–" : `${fmt(c.rate, 0)}%`}
                  </td>
                ))}
                <td className={r.total >= FIT_PP ? "good" : r.total <= -FIT_PP ? "bad" : "muted"}>
                  <b>{signed(r.total, 1)}</b>
                </td>
              </tr>
            );
          })}
        </tbody>
      </table>
    </div>
  );
}

/** Reasons colored for the player's team: what helps an enemy hero is bad news. */
function Reasons({ reasons, enemy = false }: { reasons: DraftReason[]; enemy?: boolean }) {
  if (!reasons.length) return null;
  return (
    <ul className="draft-reasons">
      {reasons.map((r) => (
        <li key={r.label} className={r.points >= 0 !== enemy ? "good" : "bad"} title={t("{n} matches", { n: fmt(r.matches) })}>
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
          <span className="muted">{t(pick.hero_type)}</span>
          <b className={pick.edge >= 0 ? "good" : "bad"} title={t("Win rate points the pick adds to your team's chance")}>
            {t("{n} pts", { n: signed(pick.edge, 2) })}
          </b>
          {pick.team_rate !== null && <span className="muted">{t("team {n}%", { n: fmt(pick.team_rate, 1) })}</span>}
        </div>
        <div className="draft-parts muted">
          {t("Strength {a} · matchups {b} · lane {c}", { a: signed(pick.strength, 2), b: signed(pick.counter, 2), c: signed(pick.lane, 2) })}
          {pick.mine && ` · ${t("your record {n} ({pct}% in {m})", { n: signed(pick.comfort, 2), pct: fmt(pick.mine[1], 0), m: pick.mine[0] })}`}
        </div>
        <Reasons reasons={pick.reasons} />
      </div>
      <button className="ghost small" onClick={onPlan} title={t("Open the master builder for this hero with both teams and the lane")}>
        {t("Plan build")}
      </button>
    </div>
  );
}

/** Which hero to pick into the enemies, what to ban, and how the draft stands. */
export default function DraftView({ heroes, onPlan }: { heroes: Hero[]; onPlan: (preset: MatchPreset) => void }) {
  const [teams, setTeams] = useState<Record<Side, number[]>>({ allies: [], enemies: [], banned: [] });
  const [lane, setLane] = useState<number[]>([]);
  const [side, setSide] = useState<Side>("enemies");
  const [badgeSetting] = useSetting(SETTINGS.draftBadge);
  const badge = Number(badgeSetting) || 0;
  const [maxSetting] = useSetting(SETTINGS.draftMaxBadge);
  const maxBadge = maxSetting ? Number(maxSetting) : null;
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
        .draftPicks(teams.allies, teams.enemies, lane, teams.banned, badge, maxBadge, account.trim() || null)
        .then((r) => !stale && setReport(r))
        .catch((e) => !stale && setError(String(e)))
        .finally(() => !stale && setBusy(false));
    }, 300);
    return () => {
      stale = true;
      clearTimeout(timer);
    };
  }, [teams, lane, badge, maxBadge, account, attempt]);

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
      player_name: t("Draft"),
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
  // Every hero can be placed; heroes with too few matches are only left out of the picks.
  const grid = heroes.filter((h) => !query || h.name.toLowerCase().includes(query.toLowerCase()));

  return (
    <div className="draft-view">
      <div className="draft-bar">
        <RankRange inline />
        <span className="muted" title={t("Your own record on each hero comes from this account. Set it in Settings.")}>
          {account.trim() ? t("Your account: {id}", { id: account.trim() }) : t("No account set: add it in Settings for your own record")}
        </span>
        <button className="ghost" onClick={() => (setTeams({ allies: [], enemies: [], banned: [] }), setLane([]))}>
          {t("Clear")}
        </button>
        <button
          className="ghost"
          title={t("Fills both teams and your lane from the match you are playing now, read from the game's console log (-condebug)")}
          onClick={() => {
            setLiveNote(t("Looking for your match… the teams take about a minute the first time."));
            findLiveMatch()
              .then((live) => {
                setLive(live);
                setTeams({ allies: live.allies, enemies: live.enemies, banned: [] });
                setLane(live.lane);
                setLiveNote(t("Match {id}: both teams loaded.", { id: live.match_id }));
              })
              .catch((e) => setLiveNote(String(e)));
          }}
        >
          {t("Load my live match")}
        </button>
        {liveNote && <span className="muted">{liveNote}</span>}
        {busy && <span className="muted">{t("Working…")}</span>}
      </div>

      <div className="draft-layout">
        <div className="draft-left">
          {SIDES.map(([s, label]) => (
            <div key={s} className={"draft-side " + s + (side === s ? " on" : "")} onClick={() => setSide(s)}>
              <div className="draft-side-head">
                <strong>{t(label)}</strong>
                <span className="muted">
                  {s === "allies"
                    ? t("{n}/{m} besides you", { n: teams.allies.length, m: TEAM_SIZE - 1 })
                    : s === "enemies"
                      ? `${teams.enemies.length}/${TEAM_SIZE}`
                      : teams.banned.length}
                </span>
                {side === s && <span className="draft-adding">{t("adding here")}</span>}
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
                          title={t("Mark as an enemy in your lane")}
                        >
                          {inLane ? t("in lane") : t("lane?")}
                        </button>
                      )}
                      <button className="ghost small" onClick={(e) => (e.stopPropagation(), remove(s, id))} title={t("Remove")}>
                        ×
                      </button>
                    </span>
                  );
                })}
                {teams[s].length === 0 && <span className="muted">{t("Click a hero below to add it here.")}</span>}
              </div>
            </div>
          ))}

          <input className="draft-search" placeholder={t("Find a hero…")} value={query} onChange={(e) => setQuery(e.target.value)} />
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
                  <span>{t("Draft as it stands")}</span>
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
                <h3>{t("Fit with the placed heroes")}</h3>
                {report.fits.length === 0 ? (
                  <p className="muted">{t("Add heroes to either team: this table then shows how every other hero has done beside your allies and against the enemies.")}</p>
                ) : (
                  <>
                    <p className="muted">
                      {t("Each column is a placed hero: beside an ally, the win rate of the two together; against an enemy, the row hero's win rate into it. Green and red mark what the heroes' own win rates do not explain. Click a hero to add it.")}
                    </p>
                    <FitTable fits={report.fits} byId={byId} placed={placed} onAdd={add} />
                  </>
                )}
              </div>

              <div className="group">
                <h3>
                  {t("Best picks")}
                  {account.trim() && (
                    <label className="check">
                      <input type="checkbox" checked={onlyMine} onChange={(e) => setOnlyMine(e.target.checked)} />
                      {t("Only heroes I play ({n}+ matches)", { n: MY_HERO_MATCHES })}
                    </label>
                  )}
                </h3>
                {shown.length === 0 && <p className="muted">{t("No hero fits.")}</p>}
                {shown.map((p, i) => (
                  <PickRow key={p.hero_id} pick={p} rank={i + 1} onPlan={() => plan(p)} />
                ))}
              </div>

              {report.threats.length > 0 && (
                <div className="group">
                  <h3>{t("Enemy picks, the most dangerous first")}</h3>
                  {report.threats.map((threat) => (
                    <div key={threat.hero_id} className="draft-threat">
                      <img src={threat.icon} alt="" />
                      <strong>{threat.name}</strong>
                      <b className={threat.edge > 0 ? "bad" : "good"}>{t("{n} pts", { n: signed(threat.edge, 2) })}</b>
                      <Reasons reasons={threat.reasons} enemy />
                    </div>
                  ))}
                </div>
              )}

              {report.bans.length > 0 && (
                <div className="group">
                  <h3>{t("Bans")}</h3>
                  {report.bans.map((b) => (
                    <div key={b.hero_id} className="draft-threat">
                      <img src={b.icon} alt="" />
                      <strong>{b.name}</strong>
                      <b className="bad">{t("{n} pts", { n: signed(b.edge, 2) })}</b>
                      <span className="muted">{t("{n}% of bans", { n: fmt(b.ban_share, 1) })}</span>
                      <Reasons reasons={b.reasons} enemy />
                    </div>
                  ))}
                </div>
              )}

              <p className="muted">
                {t("Win rates from the last 30 days of matches in the chosen ranks, from everyone who played them; your own record, only when an account is set, from your last 90 days. A pick counts its own win rate and its matchups against every enemy (and, in your lane, its lane results), weighted as fitted on 8,700 Ascendant+ matches: with these weights, drafts given 45% won 45% and drafts given 55% won 56%. Pairs of allies did not predict wins and are not counted in the picks; the fit table shows them apart. A draft moves a match by a few points at most.")}
              </p>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
