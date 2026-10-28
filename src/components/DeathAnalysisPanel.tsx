import { useState } from "react";
import { api, DeathAnalysis, DeathFix, DeathTrend, fmt } from "../api";
import { clock } from "./MatchCharts";
import ErrorNote from "./ErrorNote";
import { percent, t, tn } from "../i18n";

const pct = (v: number) => percent(fmt(v * 100, 0));

const secs = (seconds: number) => (seconds >= 30 ? t("{n}s+", { n: 30 }) : t("{n}s", { n: fmt(seconds, 1) }));

function FixChip({ fix, count, killer }: { fix: DeathFix; count?: number; killer?: string }) {
  const effect =
    fix.kind === "offense"
      ? t("kills {hero} in {time}", { hero: killer ?? t("them"), time: secs(fix.my_time_to_kill) })
      : t("+{n}s alive", { n: fmt(fix.seconds_gained, 1) });
  return (
    <span
      className={"death-fix" + (fix.owned ? " owned" : "") + (fix.wins_duel ? " wins" : "")}
      title={t(fix.wins_duel ? "With it: alive {n}s longer, and your fight kills {hero} in {time} — first" : "With it: alive {n}s longer, and your fight kills {hero} in {time}", {
        n: fmt(fix.seconds_gained, 1),
        hero: killer ?? t("the killer"),
        time: secs(fix.my_time_to_kill),
      })}
    >
      <img src={fix.icon} alt="" loading="lazy" />
      <span>
        {fix.owned ? t("Use {item}", { item: fix.name }) : fix.name}
        <small>
          {fix.owned ? t("owned") : t("{n} souls", { n: fmt(fix.cost) })}
          {fix.replaces ? ` · ${t("in place of {item}", { item: fix.replaces })}` : ""}
          {count !== undefined ? ` · ${tn(count, "{n} death", "{n} deaths")}` : ` · ${effect}`}
          {count === undefined && fix.wins_duel ? ` · ${t("wins the duel")}` : ""}
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
          {busy ? t("Studying the deaths…") : t("Why did {hero} die?", { hero })}
        </button>
        <ErrorNote error={error} onRetry={run} />
      </div>
    );
  }
  return (
    <div className="death-analysis">
      <div className="insight">
        <strong>
          {t("{a} gun damage · {b} spirit around the deaths", { a: pct(report.bullet_share), b: pct(1 - report.bullet_share) })}
        </strong>
        <span>{report.summary}</span>
      </div>
      {report.fixes.length > 0 && (
        <div className="death-fixes">
          <span className="muted">{t("Helps most often:")}</span>
          {report.fixes.map(([f, n]) => (
            <FixChip key={f.item} fix={f} count={n} />
          ))}
        </div>
      )}
      {report.deaths.map((d) => (
        <details key={d.time} className="death-study">
          <summary>
            <b>{clock(d.time)}</b> · {d.killer_hero} ·{" "}
            {d.kill_time !== null ? t("kills alone in {n}s", { n: fmt(d.kill_time, 1) }) : t("{n} of health alone", { n: pct(d.health_taken) })}
          </summary>
          <p>{d.verdict}</p>
          <div className="death-cols">
            <div>
              <h4>
                {t("Damage taken {from}–{to}", { from: clock(d.window_from), to: clock(d.window_to) })}{" "}
                <small className="muted">({t("{n} gun", { n: pct(d.bullet_share) })})</small>
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
              <h4>{t("The killer's full fight")}</h4>
              <div className="muted">
                <b>{t("The duel:")}</b>{" "}
                {t("{hero} kills you in {a}; you kill {hero} in {b}.", { hero: d.killer_hero, a: secs(d.their_time_to_kill), b: secs(d.my_time_to_kill) })}
              </div>
              <div className="muted">
                {t(
                  "Every ability and active item once and eight seconds of shooting: {a} damage against your {b} health, {c} of it gun damage. The game recorded {n}s from the first hit to the death.",
                  { a: fmt(d.fight_damage, 0), b: fmt(d.my_health, 0), c: pct(d.fight_bullet_share), n: fmt(d.time_to_kill, 1) },
                )}
              </div>
              <div className="muted">
                <b>{t("You:")}</b> {d.my_items.join(", ") || t("no items")}
              </div>
              <div className="muted">
                <b>{d.killer_hero}:</b> {d.killer_items.join(", ") || t("no items")}
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
        {t("The fight is the killer alone, with the builds both of you had at the time; the damage taken is everything that hit you in the match data's three-minute interval around the death. The duel is both full fights, theirs on you and yours on them, with the builds of the time; past the fights' end their pace is carried on. Items up to 1,600 souls in the lane, 3,200 until 20 minutes and 6,400 after are tried on both sides: the ones that turn the duel come first, then the ones that change it most. An item that only keeps you alive under two seconds longer is not listed. With every slot taken, an item comes in place of the cheapest one you held, which is sold for it.")}
      </p>
    </div>
  );
}

/** Deaths over the last matches: who kills the player, with what, and what would have helped most often. */
export function DeathTrendView({ trend, onOpen }: { trend: DeathTrend; onOpen: (matchId: number) => void }) {
  return (
    <div className="death-analysis death-trend">
      <h3>
        {t("Why you die · last {n} matches, {d} deaths", { n: trend.matches, d: trend.deaths })}
      </h3>
      <div className="insight">
        <strong>
          {t("{a} gun damage · {b} spirit around your deaths · the killer alone was enough in {n} of {d}", {
            a: pct(trend.bullet_share),
            b: pct(1 - trend.bullet_share),
            n: trend.solo_kills,
            d: trend.deaths,
          })}
        </strong>
        <span>{trend.summary}</span>
      </div>
      <div className="death-cols">
        <div>
          <h4>{t("Who kills you")}</h4>
          <table className="match-table">
            <tbody>
              {trend.killers.map(([hero, kills, matches]) => (
                <tr key={hero}>
                  <td className="left">{hero}</td>
                  <td>{tn(kills, "{n} kill", "{n} kills")}</td>
                  <td className="muted">{tn(matches, "in {n} match", "in {n} matches")}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        <div>
          <h4>{t("What would have helped most often")}</h4>
          <div className="death-fixes">
            {trend.fixes.map(([f, deaths, matches]) => (
              <span key={f.item} className="death-fix" title={t("Helped in {d} deaths over {m} matches", { d: deaths, m: matches })}>
                <img src={f.icon} alt="" loading="lazy" />
                <span>
                  {f.name}
                  <small>
                    {t("{n} souls", { n: fmt(f.cost) })} · {t("{d} deaths, {m} matches", { d: deaths, m: matches })}
                  </small>
                </span>
              </span>
            ))}
          </div>
        </div>
      </div>
      <h4>{t("The matches")}</h4>
      <div className="death-matches">
        {trend.per_match.map(([id, hero, deaths]) => (
          <button key={id} className="ghost small" onClick={() => onOpen(id)} title={t("Open the match")}>
            {hero} · {tn(deaths, "{n} death", "{n} deaths")}
          </button>
        ))}
      </div>
      <p className="muted">
        {t("Each death is studied as in a match's Deaths section: the killer's full fight against the builds of the time, and the vitality items that would have kept you alive or slowed the kill. Open a match to see its deaths one by one.")}
      </p>
    </div>
  );
}
