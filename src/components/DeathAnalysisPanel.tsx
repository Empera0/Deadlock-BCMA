import { useState } from "react";
import { api, DeathAnalysis, DeathFix, fmt } from "../api";
import { clock } from "./MatchCharts";

const pct = (v: number) => `${fmt(v * 100, 0)}%`;

function FixChip({ fix, count }: { fix: DeathFix; count?: number }) {
  return (
    <span className={"death-fix" + (fix.owned ? " owned" : "")} title={fix.kill_time !== null ? `Still dead in ${fmt(fix.kill_time, 1)}s` : "Survives the fight"}>
      <img src={fix.icon} alt="" loading="lazy" />
      <span>
        {fix.owned ? `Use ${fix.name}` : fix.name}
        <small>
          {fix.owned ? "owned" : `${fmt(fix.cost)} souls`}
          {count !== undefined ? ` · ${count} deaths` : ` · ${pct(fix.health_taken)} of health`}
        </small>
      </span>
    </span>
  );
}

/** Why the player died, death by death: the killer's fight against the build of the time, and what would have helped. */
export default function DeathAnalysisPanel({ matchId, slot, hero }: { matchId: number; slot: number; hero: string }) {
  const [report, setReport] = useState<DeathAnalysis | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const run = () => {
    setBusy(true);
    setError("");
    api
      .deathAnalysis(matchId, slot)
      .then(setReport)
      .catch((e) => setError(String(e)))
      .finally(() => setBusy(false));
  };

  if (!report) {
    return (
      <div className="death-analysis">
        <button disabled={busy} onClick={run}>
          {busy ? "Studying the deaths…" : `Why did ${hero} die?`}
        </button>
        {error && <div className="error">{error}</div>}
      </div>
    );
  }
  return (
    <div className="death-analysis">
      <div className="insight">
        <strong>
          {pct(report.bullet_share)} gun damage · {pct(1 - report.bullet_share)} spirit around the deaths
        </strong>
        <span>{report.summary}</span>
      </div>
      {report.fixes.length > 0 && (
        <div className="death-fixes">
          <span className="muted">Helps most often:</span>
          {report.fixes.map(([f, n]) => (
            <FixChip key={f.item} fix={f} count={n} />
          ))}
        </div>
      )}
      {report.deaths.map((d) => (
        <details key={d.time} className="death-study">
          <summary>
            <b>{clock(d.time)}</b> · {d.killer_hero} · {d.kill_time !== null ? `kills alone in ${fmt(d.kill_time, 1)}s` : `${pct(d.health_taken)} of health alone`}
          </summary>
          <p>{d.verdict}</p>
          <div className="death-cols">
            <div>
              <h4>
                Damage taken {clock(d.window_from)}–{clock(d.window_to)} <small className="muted">({pct(d.bullet_share)} gun)</small>
              </h4>
              <table className="match-table">
                <tbody>
                  {d.taken.slice(0, 8).map((h) => (
                    <tr key={h.name}>
                      <td className="left">{h.name}</td>
                      <td>{fmt(h.amount, 0)}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            <div>
              <h4>The killer's full fight</h4>
              <div className="muted">
                Every ability and active item once and eight seconds of shooting: {fmt(d.fight_damage, 0)} damage against your {fmt(d.my_health, 0)} health,{" "}
                {pct(d.fight_bullet_share)} of it gun damage. The game recorded {fmt(d.time_to_kill, 1)}s from the first hit to the death.
              </div>
              <div className="muted">
                <b>You:</b> {d.my_items.join(", ") || "no items"}
              </div>
              <div className="muted">
                <b>{d.killer_hero}:</b> {d.killer_items.join(", ") || "no items"}
              </div>
            </div>
          </div>
          {d.fixes.length > 0 && (
            <div className="death-fixes">
              {d.fixes.map((f) => (
                <FixChip key={f.item} fix={f} />
              ))}
            </div>
          )}
        </details>
      ))}
      <p className="muted">
        The fight is the killer alone, with the builds both of you had at the time; the damage taken is everything that hit you in the
        match data's three-minute interval around the death. Items are vitality items up to 1,600 souls in the lane, 3,200 until 20
        minutes and 6,400 after; actives are used as the fight starts.
      </p>
    </div>
  );
}
