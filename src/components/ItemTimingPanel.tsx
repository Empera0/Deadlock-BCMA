import { useEffect, useState } from "react";
import { api, fmt, ItemTimingReport } from "../api";
import { clock } from "./MatchCharts";
import ErrorNote from "./ErrorNote";
import { t } from "../i18n";

const TONE: Record<string, string> = { early: "good", "on time": "good", late: "info", "very late": "bad" };

/** When the player bought each item, against when top players of the hero buy it. */
export default function ItemTimingPanel({ matchId, slot, hero }: { matchId: number; slot: number; hero: string }) {
  const [report, setReport] = useState<ItemTimingReport | null>(null);
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let stale = false;
    setReport(null);
    setError("");
    api
      .itemTiming(matchId, slot)
      .then((r) => !stale && setReport(r))
      .catch((e) => !stale && setError(String(e)));
    return () => {
      stale = true;
    };
  }, [matchId, slot, attempt]);

  return (
    <div className="group">
      <h3>{t("Against top players")}</h3>
      <ErrorNote error={error} onRetry={() => setAttempt((a) => a + 1)} />
      {!report && !error && <div className="muted">{t("Reading when top {hero} players buy each item…", { hero })}</div>}
      {report && (
        <>
          <div className="insight">
            <span>{report.summary}</span>
          </div>
          <table className="match-table compare timing">
            <thead>
              <tr>
                <th className="left">{t("Item")}</th>
                <th>{t("Bought")}</th>
                <th>{t("Top players")}</th>
                <th>{t("Most of them")}</th>
                <th>{t("Off by")}</th>
                <th title={t("How much more often top players who bought it around your minute won than all who bought it")}>{t("Wins then")}</th>
              </tr>
            </thead>
            <tbody>
              {report.items.map((i) => (
                <tr key={i.item}>
                  <td className="left">
                    <img className="row-icon" src={i.icon} alt="" loading="lazy" /> {i.name}
                  </td>
                  <td>
                    <b className={"timing-" + (TONE[i.tone] ?? "info")}>{clock(i.bought_at)}</b>
                  </td>
                  <td>{clock(i.median_s)}</td>
                  <td className="muted">
                    {clock(i.early_s)}–{clock(i.late_s)}
                  </td>
                  <td className={"timing-" + (TONE[i.tone] ?? "info")}>
                    {Math.abs(i.minutes_off) < 0.5 ? "–" : t("{sign}{n} min", { sign: i.minutes_off > 0 ? "+" : "−", n: fmt(Math.abs(i.minutes_off), 0) })} · {t(i.tone)}
                    {i.shift_min >= 0.5 && (
                      <small className="muted shifted" title={t("Souls that went to items bought ahead of when top players buy them, at your income then")}>
                        {t("{n} min of it: {items} bought first", { n: fmt(i.shift_min, 1), items: i.shifted_by.join(", ") })}
                      </small>
                    )}
                  </td>
                  <td className={i.win_pp_then === null ? "muted" : i.win_pp_then >= 1 ? "up" : i.win_pp_then <= -1 ? "down" : "muted"}>
                    {i.win_pp_then === null ? "–" : t("{sign}{n} pts", { sign: i.win_pp_then > 0 ? "+" : "", n: fmt(i.win_pp_then, 1) })}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {report.missing.length > 0 && (
            <div className="death-fixes">
              <span className="muted">{t("Most top players buy, you did not:")}</span>
              {report.missing.map((m) => (
                <span key={m.item} className="death-fix">
                  <img src={m.icon} alt="" loading="lazy" />
                  <span>
                    {m.name}
                    <small>
                      {t("{n} souls", { n: fmt(m.cost) })} · {t("by {time}", { time: clock(m.median_s) })}
                    </small>
                  </span>
                </span>
              ))}
            </div>
          )}
          <p className="muted">
            {t("Top players are those of the last 14 days at Ascendant and above. The minute they buy each item is the median of their purchases; most of them buy it between the two times shown. Early is before the first quarter of them, late after the last quarter. An item bought ahead of when top players buy it (a counter rushed early) takes the souls of the items that usually come before it: that delay is worked out from your own soul income at the time, shown under the item, and taken out before it is called late. Wins then compares those who bought it within two minutes of your purchase with everyone who bought it; it follows how the match was going as much as the item.")}
          </p>
        </>
      )}
    </div>
  );
}
