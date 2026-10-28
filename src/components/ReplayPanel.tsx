import { useState } from "react";
import { api, fmt, ReplayStudy } from "../api";
import ErrorNote from "./ErrorNote";
import { clock } from "./MatchCharts";
import { t } from "../i18n";

/** One player of a match, read from its replay: each ability's casts, and the last seconds before each death. */
export default function ReplayPanel({ matchId, slot, hero }: { matchId: number; slot: number; hero: string }) {
  const [study, setStudy] = useState<ReplayStudy | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const run = () => {
    setBusy(true);
    setError(null);
    api
      .replayStudy(matchId, slot)
      .then(setStudy)
      .catch(setError)
      .finally(() => setBusy(false));
  };

  return (
    <div className="group replay-panel">
      <h3>{t("From the replay")}</h3>
      {!study && (
        <>
          <p className="muted">
            {t(
              "The replay shows every hit and every stun: {hero}'s last seconds before each death, and what each ability's casts caught. Reading it takes about a minute the first time; Valve keeps the replays of only some matches.",
              { hero },
            )}
          </p>
          <button disabled={busy} onClick={run}>
            {busy ? t("Reading the replay…") : t("Study the replay")}
          </button>
          <ErrorNote error={error} onRetry={run} />
        </>
      )}
      {study && (
        <>
          {study.abilities.length > 0 && (
            <>
              <h4>{t("Abilities")}</h4>
              <table className="match-table compare timing">
                <thead>
                  <tr>
                    <th className="left">{t("Ability")}</th>
                    <th>{t("Casts")}</th>
                    <th title={t("Enemy heroes a cast reached")}>{t("Caught a cast")}</th>
                    <th title={t("What a cast of it catches on average in team fights of top players' matches, at your build")}>{t("Usual")}</th>
                    <th title={t("Enemy heroes a cast stunned, slept, silenced or held")}>{t("Held a cast")}</th>
                    <th>{t("Damage a cast")}</th>
                    <th className="left wide">{t("In short")}</th>
                  </tr>
                </thead>
                <tbody>
                  {study.abilities.map((a) => (
                    <tr key={a.ability}>
                      <td className="left">
                        <img className="row-icon" src={a.icon} alt="" loading="lazy" /> {a.ability}
                      </td>
                      <td>{a.casts}</td>
                      <td className={a.usual === null ? "" : a.enemies_per_cast >= a.usual + 0.15 ? "up" : a.enemies_per_cast <= a.usual - 0.15 ? "down" : ""}>
                        {fmt(a.enemies_per_cast, 2)}
                      </td>
                      <td className="muted">{a.usual === null ? "–" : fmt(a.usual, 2)}</td>
                      <td>{a.held_per_cast > 0 ? fmt(a.held_per_cast, 2) : "–"}</td>
                      <td>{fmt(a.damage_per_cast, 0)}</td>
                      <td className="left wide muted">{a.verdict}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <p className="muted">
                {t("Only the casts that reached an enemy hero count. When an ability is used matters more than how often: it is judged by what its casts did, never by how long it waited.")}
              </p>
            </>
          )}
          <h4>{t("The last seconds before each death")}</h4>
          {study.deaths.map((d) => (
            <details key={d.time} className="death-study">
              <summary>
                <b>{clock(d.time)}</b> · {t("killed by {hero}", { hero: d.killer })} · {d.summary}
              </summary>
              {d.health_start !== null && <div className="muted">{t("Health when the first of these hits landed: {n}", { n: fmt(d.health_start, 0) })}</div>}
              <table className="match-table compare timing">
                <thead>
                  <tr>
                    <th>{t("When")}</th>
                    <th className="left">{t("Who")}</th>
                    <th className="left">{t("With")}</th>
                    <th>{t("Damage")}</th>
                    <th>{t("Health left")}</th>
                  </tr>
                </thead>
                <tbody>
                  {[
                    ...d.hits.map((m) => ({ at: m.at, row: m, status: null as null | (typeof d.statuses)[number] })),
                    ...d.statuses.map((s) => ({ at: s.at, row: null, status: s })),
                  ]
                    .sort((a, b) => a.at - b.at)
                    .map((x, i) =>
                      x.row ? (
                        <tr key={i}>
                          <td>
                            {x.row.until - x.row.at >= 0.2
                              ? t("{a}s to {b}s", { a: fmt(x.row.at, 1), b: fmt(x.row.until, 1) })
                              : t("{n}s", { n: fmt(x.row.at, 1) })}
                          </td>
                          <td className="left">{x.row.hero}</td>
                          <td className="left">
                            {x.row.source}
                            {x.row.hits > 1 && <small className="muted"> · {t("{n} hits", { n: x.row.hits })}</small>}
                          </td>
                          <td>{fmt(x.row.damage, 0)}</td>
                          <td className={x.row.health_after <= 0 ? "down" : ""}>{fmt(Math.max(0, x.row.health_after), 0)}</td>
                        </tr>
                      ) : (
                        <tr key={i} className="replay-status">
                          <td>{t("{n}s", { n: fmt(x.status!.at, 1) })}</td>
                          <td className="left">{x.status!.hero}</td>
                          <td className="left" colSpan={3}>
                            <b className="timing-bad">{x.status!.statuses.map((s) => t(s)).join(t(" and "))}</b> · {x.status!.source}
                          </td>
                        </tr>
                      ),
                    )}
                </tbody>
              </table>
            </details>
          ))}
        </>
      )}
    </div>
  );
}
