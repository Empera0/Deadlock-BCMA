import { useEffect, useState } from "react";
import { api, fmt, ItemTimingReport } from "../api";
import { clock } from "./MatchCharts";

const TONE: Record<string, string> = { early: "good", "on time": "good", late: "info", "very late": "bad" };

/** When the player bought each item, against when top players of the hero buy it. */
export default function ItemTimingPanel({ matchId, slot, hero }: { matchId: number; slot: number; hero: string }) {
  const [report, setReport] = useState<ItemTimingReport | null>(null);
  const [error, setError] = useState("");
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
  }, [matchId, slot]);

  return (
    <div className="group">
      <h3>Against top players</h3>
      {error && <div className="error">{error}</div>}
      {!report && !error && <div className="muted">Reading when top {hero} players buy each item…</div>}
      {report && (
        <>
          <div className="insight">
            <span>{report.summary}</span>
          </div>
          <table className="match-table compare timing">
            <thead>
              <tr>
                <th className="left">Item</th>
                <th>Bought</th>
                <th>Top players</th>
                <th>Most of them</th>
                <th>Off by</th>
                <th title="How much more often top players who bought it around your minute won than all who bought it">Wins then</th>
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
                    {Math.abs(i.minutes_off) < 0.5 ? "–" : `${i.minutes_off > 0 ? "+" : "−"}${fmt(Math.abs(i.minutes_off), 0)} min`} · {i.tone}
                    {i.shift_min >= 0.5 && (
                      <small className="muted shifted" title="Souls that went to items bought ahead of when top players buy them, at your income then">
                        {fmt(i.shift_min, 1)} min of it: {i.shifted_by.join(", ")} bought first
                      </small>
                    )}
                  </td>
                  <td className={i.win_pp_then === null ? "muted" : i.win_pp_then >= 1 ? "up" : i.win_pp_then <= -1 ? "down" : "muted"}>
                    {i.win_pp_then === null ? "–" : `${i.win_pp_then > 0 ? "+" : ""}${fmt(i.win_pp_then, 1)} pts`}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {report.missing.length > 0 && (
            <div className="death-fixes">
              <span className="muted">Most top players buy, you did not:</span>
              {report.missing.map((m) => (
                <span key={m.item} className="death-fix">
                  <img src={m.icon} alt="" loading="lazy" />
                  <span>
                    {m.name}
                    <small>
                      {fmt(m.cost)} souls · by {clock(m.median_s)}
                    </small>
                  </span>
                </span>
              ))}
            </div>
          )}
          <p className="muted">
            Top players are those of the last 14 days at Ascendant and above. The minute they buy each item is the median of their purchases;
            most of them buy it between the two times shown. Early is before the first quarter of them, late after the last quarter. An item
            bought ahead of when top players buy it (a counter rushed early) takes the souls of the items that usually come before it: that delay
            is worked out from your own soul income at the time, shown under the item, and taken out before it is called late. Wins then
            compares those who bought it within two minutes of your purchase with everyone who bought it; it follows how the match was going as
            much as the item.
          </p>
        </>
      )}
    </div>
  );
}
