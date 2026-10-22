import { useState } from "react";
import { api, DeathAnalysis, DeathFix, DeathTrend, fmt } from "../api";
import { clock } from "./MatchCharts";
import ErrorNote from "./ErrorNote";

const pct = (v: number) => `${fmt(v * 100, 0)}%`;

const secs = (t: number) => (t >= 30 ? "30s+" : `${fmt(t, 1)}s`);

function FixChip({ fix, count, killer }: { fix: DeathFix; count?: number; killer?: string }) {
  const effect =
    fix.kind === "offense" ? `kills ${killer ?? "them"} in ${secs(fix.my_time_to_kill)}` : `+${fmt(fix.seconds_gained, 1)}s alive`;
  return (
    <span
      className={"death-fix" + (fix.owned ? " owned" : "") + (fix.wins_duel ? " wins" : "")}
      title={`With it: alive ${fmt(fix.seconds_gained, 1)}s longer, and your fight kills ${killer ?? "the killer"} in ${secs(fix.my_time_to_kill)}${fix.wins_duel ? " — first" : ""}`}
    >
      <img src={fix.icon} alt="" loading="lazy" />
      <span>
        {fix.owned ? `Use ${fix.name}` : fix.name}
        <small>
          {fix.owned ? "owned" : `${fmt(fix.cost)} souls`}
          {fix.replaces ? ` · in place of ${fix.replaces}` : ""}
          {count !== undefined ? ` · ${count} deaths` : ` · ${effect}`}
          {count === undefined && fix.wins_duel ? " · wins the duel" : ""}
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
        <ErrorNote error={error} onRetry={run} />
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
                <b>The duel:</b> {d.killer_hero} kills you in {secs(d.their_time_to_kill)}; you kill {d.killer_hero} in {secs(d.my_time_to_kill)}.
              </div>
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
                <FixChip key={f.item} fix={f} killer={d.killer_hero} />
              ))}
            </div>
          )}
        </details>
      ))}
      <p className="muted">
        The fight is the killer alone, with the builds both of you had at the time; the damage taken is everything that hit you in the
        match data's three-minute interval around the death. The duel is both full fights, theirs on you and yours on them, with the
        builds of the time; past the fights' end their pace is carried on. Items up to 1,600 souls in the lane, 3,200 until 20 minutes and
        6,400 after are tried on both sides: the ones that turn the duel come first, then the ones that change it most. An item that only
        keeps you alive under two seconds longer is not listed. With every slot taken, an item comes in place of the cheapest one you
        held, which is sold for it.
      </p>
    </div>
  );
}

/** Deaths over the last matches: who kills the player, with what, and what would have helped most often. */
export function DeathTrendView({ trend, onOpen }: { trend: DeathTrend; onOpen: (matchId: number) => void }) {
  return (
    <div className="death-analysis death-trend">
      <h3>
        Why you die · last {trend.matches} matches, {trend.deaths} deaths
      </h3>
      <div className="insight">
        <strong>
          {pct(trend.bullet_share)} gun damage · {pct(1 - trend.bullet_share)} spirit around your deaths · the killer alone was enough in{" "}
          {trend.solo_kills} of {trend.deaths}
        </strong>
        <span>{trend.summary}</span>
      </div>
      <div className="death-cols">
        <div>
          <h4>Who kills you</h4>
          <table className="match-table">
            <tbody>
              {trend.killers.map(([hero, kills, matches]) => (
                <tr key={hero}>
                  <td className="left">{hero}</td>
                  <td>
                    {kills} kill{kills === 1 ? "" : "s"}
                  </td>
                  <td className="muted">
                    in {matches} match{matches === 1 ? "" : "es"}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div>
          <h4>What would have helped most often</h4>
          <div className="death-fixes">
            {trend.fixes.map(([f, deaths, matches]) => (
              <span key={f.item} className="death-fix" title={`Helped in ${deaths} deaths over ${matches} matches`}>
                <img src={f.icon} alt="" loading="lazy" />
                <span>
                  {f.name}
                  <small>
                    {fmt(f.cost)} souls · {deaths} deaths, {matches} matches
                  </small>
                </span>
              </span>
            ))}
          </div>
        </div>
      </div>
      <h4>The matches</h4>
      <div className="death-matches">
        {trend.per_match.map(([id, hero, deaths]) => (
          <button key={id} className="ghost small" onClick={() => onOpen(id)} title="Open the match">
            {hero} · {deaths} death{deaths === 1 ? "" : "s"}
          </button>
        ))}
      </div>
      <p className="muted">
        Each death is studied as in a match's Deaths section: the killer's full fight against the builds of the time, and the vitality items
        that would have kept you alive or slowed the kill. Open a match to see its deaths one by one.
      </p>
    </div>
  );
}
