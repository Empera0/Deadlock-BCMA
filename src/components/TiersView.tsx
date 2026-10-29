import { useEffect, useMemo, useState } from "react";
import { api, fmt, HeroTier } from "../api";
import { percent, t } from "../i18n";
import { SETTINGS, useSetting } from "../settings";
import ErrorNote from "./ErrorNote";
import RankRange from "./RankRange";

type Key = "ranked_rate" | "pick_rate" | "ban_share" | "kda" | "net_worth" | "matches";
/** The columns a list can be sorted by; translated where shown. */
const COLUMNS: [Key, string, string][] = [
  ["ranked_rate", "Win rate", "Its win rate; ranked by the same pulled toward 50% by its matches, so a hero played rarely is not put on top by a lucky run"],
  ["pick_rate", "Picked", "Share of matches it is in"],
  ["ban_share", "Banned", "Its share of all bans"],
  ["kda", "K / D / A", "Its players' kills, deaths and assists per match"],
  ["net_worth", "Souls", "Its players' souls per match"],
  ["matches", "Matches", "Matches it was played in"],
];
/** Hero types; translated where shown. */
const TYPES = ["brawler", "marksman", "mystic", "assassin"];

const kda = (h: HeroTier) => (h.kills + h.assists) / Math.max(h.deaths, 1);

/** Every hero at the chosen ranks: how often it wins, is picked and banned, and what its players average. */
export default function TiersView({ onBuild }: { onBuild: (heroClass: string) => void }) {
  const [badgeSetting] = useSetting(SETTINGS.draftBadge);
  const [maxSetting] = useSetting(SETTINGS.draftMaxBadge);
  const badge = Number(badgeSetting) || 0;
  const maxBadge = maxSetting ? Number(maxSetting) : null;
  const [rows, setRows] = useState<HeroTier[] | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [sort, setSort] = useState<Key>("ranked_rate");
  const [type, setType] = useState("");
  const [query, setQuery] = useState("");

  useEffect(() => {
    let stale = false;
    setBusy(true);
    setError("");
    api
      .heroTiers(badge, maxBadge)
      .then((r) => !stale && setRows(r))
      .catch((e) => !stale && setError(String(e)))
      .finally(() => !stale && setBusy(false));
    return () => {
      stale = true;
    };
  }, [badge, maxBadge, attempt]);

  const shown = useMemo(() => {
    const value = (h: HeroTier) => (sort === "kda" ? kda(h) : h[sort]);
    return (rows ?? [])
      .filter((h) => (!type || h.hero_type === type) && (!query || h.name.toLowerCase().includes(query.toLowerCase())))
      .sort((a, b) => value(b) - value(a));
  }, [rows, sort, type, query]);
  const total = rows ? Math.round(rows.reduce((n, h) => n + h.matches, 0) / 12) : 0;

  return (
    <div className="tiers-view">
      <div className="draft-bar">
        <RankRange inline />
        <select value={type} onChange={(e) => setType(e.target.value)}>
          <option value="">{t("Every type")}</option>
          {TYPES.map((x) => (
            <option key={x} value={x}>
              {t(x)}
            </option>
          ))}
        </select>
        <input className="draft-search" placeholder={t("Find a hero…")} value={query} onChange={(e) => setQuery(e.target.value)} />
        {busy && <span className="muted">{t("Working…")}</span>}
      </div>
      <ErrorNote error={error} onRetry={() => setAttempt((a) => a + 1)} />
      {rows && (
        <>
          <p className="muted">
            {t(
              "From about {n} matches of the last 30 days in the chosen ranks. Tiers compare each hero with the average one: S and A win clearly more, C and D clearly less. Click a column to sort by it, and a hero to see its build.",
              { n: fmt(total) },
            )}
          </p>
          <table className="hero-tiers">
            <thead>
              <tr>
                <th>{t("Tier")}</th>
                <th>{t("Hero")}</th>
                {COLUMNS.map(([key, label, help]) => (
                  <th key={key} className={sort === key ? "on" : ""} title={t(help)} onClick={() => setSort(key)}>
                    {t(label)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {shown.map((h) => (
                <tr key={h.hero_id}>
                  <td>
                    <span className={"tier-badge " + h.tier}>{h.tier}</span>
                  </td>
                  <td>
                    <button className="ghost small tier-hero" onClick={() => onBuild(h.hero)} title={t("See what players build with {hero}", { hero: h.name })}>
                      <img src={h.icon} alt="" loading="lazy" />
                      <span>{h.name}</span>
                      <small className="muted">{t(h.hero_type)}</small>
                    </button>
                  </td>
                  <td title={t("Ranked by {n}", { n: percent(fmt(h.ranked_rate, 1)) })}>{percent(fmt(h.win_rate, 1))}</td>
                  <td>{percent(fmt(h.pick_rate, 1))}</td>
                  <td>{percent(fmt(h.ban_share, 1))}</td>
                  <td>
                    {fmt(h.kills, 1)} / {fmt(h.deaths, 1)} / {fmt(h.assists, 1)}
                  </td>
                  <td>{fmt(h.net_worth, 0)}</td>
                  <td>{fmt(h.matches)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </div>
  );
}
