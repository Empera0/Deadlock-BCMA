import { useEffect, useState } from "react";
import { api, Build, fmt, OrderReport, Sheet } from "../api";
import ErrorNote from "./ErrorNote";

const ROMAN = ["", "I", "II", "III"];

/** The order to spend ability points in for this build, and the orders top players go with. */
export default function AbilityOrderPanel({ build, sheet, onTiers }: { build: Build; sheet: Sheet; onTiers: (tiers: number[]) => void }) {
  const [report, setReport] = useState<OrderReport | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  // A different hero or items make the order stale.
  useEffect(() => setReport(null), [build.hero, build.purchases, build.souls]);

  const run = () => {
    setBusy(true);
    setError("");
    api
      .abilityOrder(build)
      .then(setReport)
      .catch((e) => setError(String(e)))
      .finally(() => setBusy(false));
  };
  const name = (i: number) => sheet.abilities[i]?.name ?? `Ability ${i + 1}`;
  const icon = (i: number) => sheet.abilities[i]?.icon;
  // The tiers after the first n steps of the order.
  const tiersAfter = (n: number) => {
    const tiers = sheet.abilities.map(() => 0);
    report?.order.steps.slice(0, n).forEach((s) => (tiers[s.index] = s.tier));
    return tiers;
  };

  return (
    <div className="group ability-order">
      <h3>
        Upgrade order
        <button className="ghost small" disabled={busy} onClick={run}>
          {busy ? "Working…" : report ? "Again" : "Work it out"}
        </button>
      </h3>
      <ErrorNote error={error} onRetry={run} />
      {!report && !error && <p className="muted">Which upgrade to put each ability point into with this build, and what top players do.</p>}
      {report && (
        <>
          <ol className="order-steps">
            {report.order.steps.map((s, n) => (
              <li key={n} title={`${s.upgrade}\n\n${s.ap} point${s.ap > 1 ? "s" : ""}, ${s.total_ap} in all: level ${s.level} (${fmt(s.souls)} souls)`}>
                <button className="ghost" onClick={() => onTiers(tiersAfter(n + 1))} title="Set the abilities to this point of the order">
                  <img src={s.icon} alt="" />
                  <span>
                    {s.ability} {ROMAN[s.tier]}
                    <small className="muted">
                      Lv {s.level}
                      {s.gain_pct >= 0.1 ? ` · +${fmt(s.gain_pct, 1)}%` : " · no damage"}
                    </small>
                  </span>
                </button>
              </li>
            ))}
          </ol>
          <p className="muted">{report.order.note} Click a step to set your abilities to it.</p>
          {report.popular.length > 0 && (
            <>
              <h4>What top players do ({report.rank})</h4>
              {report.popular.map((o, k) => (
                <div key={k} className="order-popular">
                  <span className="muted">
                    {fmt(o.matches)} matches · {fmt(o.win_pct, 1)}% wins
                  </span>
                  <span className="order-line">
                    {o.steps.map(([i, t], n) => (
                      <span key={n} title={`${name(i)} ${ROMAN[t]}`}>
                        {icon(i) && <img src={icon(i)} alt="" />}
                        <small>{ROMAN[t]}</small>
                      </span>
                    ))}
                  </span>
                </div>
              ))}
            </>
          )}
        </>
      )}
    </div>
  );
}
