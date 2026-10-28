import { useEffect, useState } from "react";
import { api, Build, fmt, OrderReport, Sheet } from "../api";
import ErrorNote from "./ErrorNote";
import { t } from "../i18n";

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
  const name = (i: number) => sheet.abilities[i]?.name ?? t("Ability {n}", { n: i + 1 });
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
        {t("Upgrade order")}
        <button className="ghost small" disabled={busy} onClick={run}>
          {busy ? t("Working…") : report ? t("Again") : t("Work it out")}
        </button>
      </h3>
      <ErrorNote error={error} onRetry={run} />
      {!report && !error && <p className="muted">{t("Which upgrade to put each ability point into with this build, and what top players do.")}</p>}
      {report && (
        <>
          <ol className="order-steps">
            {report.order.steps.map((s, n) => (
              <li key={n} title={`${s.upgrade}\n\n${t(s.ap > 1 ? "{n} points, {total} in all: level {level} ({souls} souls)" : "{n} point, {total} in all: level {level} ({souls} souls)", { n: s.ap, total: s.total_ap, level: s.level, souls: fmt(s.souls) })}`}>
                <button className="ghost" onClick={() => onTiers(tiersAfter(n + 1))} title={t("Set the abilities to this point of the order")}>
                  <img src={s.icon} alt="" />
                  <span>
                    {s.ability} {ROMAN[s.tier]}
                    <small className="muted">
                      {t("Lv {n}", { n: s.level })}
                      {s.gain_pct >= 0.1 ? ` · +${fmt(s.gain_pct, 1)}%` : ` · ${t("no damage")}`}
                    </small>
                  </span>
                </button>
              </li>
            ))}
          </ol>
          <p className="muted">
            {report.order.note} {t("Click a step to set your abilities to it.")}
          </p>
          {report.popular.length > 0 && (
            <>
              <h4>{t("What top players do ({rank})", { rank: report.rank })}</h4>
              {report.popular.map((o, k) => (
                <div key={k} className="order-popular">
                  <span className="muted">
                    {t("{n} matches · {w}% wins", { n: fmt(o.matches), w: fmt(o.win_pct, 1) })}
                  </span>
                  <span className="order-line">
                    {o.steps.map(([i, tier], n) => (
                      <span key={n} title={`${name(i)} ${ROMAN[tier]}`}>
                        {icon(i) && <img src={icon(i)} alt="" />}
                        <small>{ROMAN[tier]}</small>
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
