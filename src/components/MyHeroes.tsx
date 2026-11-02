import { useEffect, useState } from "react";
import { api, fmt, Mastery, MyHeroes as Report, Versus } from "../api";
import { percent, t } from "../i18n";
import { SETTINGS, TIERS, useSetting } from "../settings";
import ErrorNote from "./ErrorNote";

type Key = "kills" | "deaths" | "assists" | "souls" | "damage" | "last_hits";
/** The values compared, and which way is better; translated where shown. */
const COLUMNS: [Key, string, "higher" | "lower", number][] = [
  ["kills", "Kills", "higher", 1],
  ["deaths", "Deaths", "lower", 1],
  ["assists", "Assists", "higher", 1],
  ["souls", "Souls", "higher", 0],
  ["damage", "Hero damage", "higher", 0],
  ["last_hits", "Last hits", "higher", 0],
];

/** A rank badge's name: the tier's name and its subrank. */
export function rankName(badge: number): string {
  const tier = TIERS[Math.floor(badge / 10) - 1];
  return tier ? `${tier} ${badge % 10}` : "";
}

/** The player's value, and how far it is from the average, colored by which way is better. */
function Cell({ v, better, digits, unit = (n: string) => n }: { v: Versus; better: "higher" | "lower"; digits: number; unit?: (n: string) => string }) {
  const diff = v.mine - v.average;
  // Within 5% of the average: neither.
  const near = Math.abs(diff) <= Math.abs(v.average) * 0.05;
  const good = better === "higher" ? diff > 0 : diff < 0;
  return (
    <td title={t("Average: {n}", { n: unit(fmt(v.average, digits)) })}>
      <b>{unit(fmt(v.mine, digits))}</b>
      <small className={near ? "muted" : good ? "good" : "bad"}>
        {diff >= 0 ? "+" : "−"}
        {unit(fmt(Math.abs(diff), digits))}
      </small>
    </td>
  );
}

/** The heroes the player plays, each beside the average player of the player's rank on it. */
export default function MyHeroes({ badge, maxBadge, onOpen }: { badge: number; maxBadge: number | null; onOpen: (heroId: number) => void }) {
  const [account] = useSetting(SETTINGS.account);
  const [report, setReport] = useState<Report | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [attempt, setAttempt] = useState(0);

  useEffect(() => {
    if (!account.trim()) return;
    let stale = false;
    setBusy(true);
    setError("");
    api
      .myHeroes(account.trim(), badge, maxBadge)
      .then((r) => !stale && setReport(r))
      .catch((e) => !stale && setError(String(e)))
      .finally(() => !stale && setBusy(false));
    return () => {
      stale = true;
    };
  }, [account, badge, maxBadge, attempt]);

  if (!account.trim()) return <p className="muted">{t("Set your Steam account ID in Settings to see your heroes here.")}</p>;
  const range = (r: Report) =>
    r.max_badge !== null && Math.floor(r.min_badge / 10) === Math.floor(r.max_badge / 10)
      ? TIERS[Math.floor(r.min_badge / 10) - 1] ?? ""
      : `${rankName(r.min_badge) || t("the lowest")} – ${r.max_badge ? rankName(r.max_badge) : t("the highest")}`;
  return (
    <>
      {busy && <p className="muted">{t("Working…")}</p>}
      <ErrorNote error={error} onRetry={() => setAttempt((a) => a + 1)} />
      {report && (
        <>
          <p className="muted">
            {report.badge >= 10
              ? t("Your rank: {rank}. Each hero beside the average {tier} player on it, over the last 30 days; your numbers are from all your matches on it.", { rank: rankName(report.badge), tier: range(report) })
              : t("No rank yet: each hero beside the average player of the ranks chosen above ({tier}), over the last 30 days; your numbers are from all your matches on it.", { tier: range(report) })}{" "}
            {t("Green is better than the average, red worse; the small number is the difference per match.")}
          </p>
          {report.heroes.length === 0 && <p className="muted">{t("No matches found for this account.")}</p>}
          {report.heroes.length > 0 && (
            <table className="hero-tiers my-heroes">
              <thead>
                <tr>
                  <th>{t("Hero")}</th>
                  <th>{t("Matches")}</th>
                  <th>{t("Win rate")}</th>
                  {COLUMNS.map(([key, label]) => (
                    <th key={key}>{t(label)}</th>
                  ))}
                  <th>{t("Accuracy")}</th>
                </tr>
              </thead>
              <tbody>
                {report.heroes.map((h: Mastery) => (
                  <tr key={h.hero_id}>
                    <td>
                      <button className="ghost small tier-hero" onClick={() => onOpen(h.hero_id)} title={t("Open the page of {hero}", { hero: h.name })}>
                        <img src={h.icon} alt="" loading="lazy" />
                        <span>{h.name}</span>
                      </button>
                    </td>
                    <td>{fmt(h.matches)}</td>
                    <Cell v={h.win_rate} better="higher" digits={0} unit={percent} />
                    {COLUMNS.map(([key, , better, digits]) => (
                      <Cell key={key} v={h[key]} better={better} digits={digits} />
                    ))}
                    {h.accuracy ? <Cell v={h.accuracy} better="higher" digits={0} unit={percent} /> : <td className="muted">–</td>}
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </>
      )}
    </>
  );
}
