import { useEffect, useState } from "react";
import { api, Build, FarmReport, fmt, RiftComeback, Sheet } from "../api";
import { t } from "../i18n";

/** How fast the build clears each jungle camp at a minute of the match. */
function Farming({ build }: { build: Build }) {
  const [minute, setMinute] = useState(10);
  const [report, setReport] = useState<FarmReport | null>(null);
  useEffect(() => {
    let stale = false;
    api
      .farmReport(build, minute)
      .then((r) => !stale && setReport(r))
      .catch(() => !stale && setReport(null));
    return () => {
      stale = true;
    };
  }, [build, minute]);
  return (
    <div className="group">
      <h3>{t("Farming the jungle")}</h3>
      <p className="muted">
        {t("How long the build takes to clear each camp and the souls per minute of clearing such camps one after another: its three basic abilities once on the camp (area abilities hit every haunt), then the gun, with weapon damage against units (Monster Rounds, Cultist Sacrifice). Haunts grow 2.1% in health per minute.")}
      </p>
      <label className="field inline">
        {t("Minute")}
        <input type="number" min={0} value={minute} onChange={(e) => setMinute(Math.max(0, Number(e.target.value) || 0))} />
      </label>
      {report && (
        <>
          <table className="farm-table">
            <thead>
              <tr>
                <th>{t("Camp")}</th>
                <th>{t("Health")}</th>
                <th>{t("Souls")}</th>
                <th>{t("Clear")}</th>
                <th>{t("Souls / min")}</th>
              </tr>
            </thead>
            <tbody>
              {report.camps.map((c) => (
                <tr key={c.name} className={c.available ? "" : "muted"} title={c.available ? "" : t("Not spawned yet at this minute")}>
                  <td>{c.name}</td>
                  <td>{fmt(c.health, 0)}</td>
                  <td>{fmt(c.souls, 0)}</td>
                  <td>{Number.isFinite(c.clear_seconds) ? t("{n}s", { n: fmt(c.clear_seconds, 1) }) : "—"}</td>
                  <td>
                    <b>{fmt(c.souls_per_minute, 0)}</b>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          {report.cultist_per_minute > 0 && (
            <p>{t("Cultist Sacrifice adds about {n} souls per minute, consuming a large haunt each time it is ready.", { n: fmt(report.cultist_per_minute, 0) })}</p>
          )}
          {report.notes.map((n) => (
            <p key={n} className="muted">
              {n}
            </p>
          ))}
        </>
      )}
    </div>
  );
}

/** Map mechanics that depend on the state of the match: for now the Unstable Rift's comeback bonus. */
export default function MiscPanel({ sheet, build }: { sheet: Sheet; build: Build }) {
  const [minute, setMinute] = useState(25);
  const [own, setOwn] = useState(130_000);
  const [enemy, setEnemy] = useState(131_000);
  const [rift, setRift] = useState<RiftComeback | null>(null);
  useEffect(() => {
    api.riftComeback(minute, own, enemy).then(setRift).catch(() => setRift(null));
  }, [minute, own, enemy]);
  // Resists from different sources stack by what each lets through.
  const stacked = (base: number, extra: number) => 100 * (1 - (1 - base / 100) * (1 - extra / 100));
  const stat = (key: string) => sheet.stats.find((s) => s.key === key)?.value ?? 0;
  const number = (value: number, set: (v: number) => void, step = 1) => (
    <input type="number" value={value} step={step} onChange={(e) => set(Math.max(0, Number(e.target.value) || 0))} />
  );

  return (
    <aside className="panel">
      <Farming build={build} />
      <div className="group">
        <h3>{t("Unstable Rift comeback")}</h3>
        <p className="muted">
          {t("Inside the rift (35 m) the team behind in net worth gets bullet, spirit and debuff resist. The most it can be is 10% plus 1% per minute of the match, at most 40% from 30:00. How much of that the team gets depends on how far behind it is: nothing when even, rising in a straight line to all of it at 15% behind. The same scale shortens its capture time and adds rift troopers if it wins. Values from the 16 September 2026 patch.")}
        </p>
        <div className="fields misc-fields">
          <label>
            {t("Minute")} {number(minute, setMinute)}
          </label>
          <label>
            {t("Your team's souls")} {number(own, setOwn, 1000)}
          </label>
          <label>
            {t("Enemy team's souls")} {number(enemy, setEnemy, 1000)}
          </label>
        </div>
      </div>

      {rift && (
        <div className="group">
          <h3>
            {t("Result")}
            <small>{rift.behind_pct === 0 ? t("teams even") : rift.own_team_trails ? t("your team is behind") : t("the enemy team is behind")}</small>
          </h3>
          <div className="tiles small">
            <div>
              <span>{t("Behind by")}</span>
              <b>{fmt(rift.behind_pct, 2)}%</b>
            </div>
            <div>
              <span>{t("Share of the full bonus")}</span>
              <b>{fmt(rift.scale * 100, 1)}%</b>
            </div>
            <div>
              <span>{t("Most resist at minute {n}", { n: minute })}</span>
              <b>{fmt(rift.max_resist, 0)}%</b>
            </div>
            <div>
              <span>{t("Resist the trailing team gets")}</span>
              <b>{fmt(rift.resist, 2)}%</b>
            </div>
            <div>
              <span>{t("Capture time: trailing / leading")}</span>
              <b>
                {t("{n}s", { n: fmt(rift.capture_trailing, 1) })} / {t("{n}s", { n: fmt(rift.capture_leading, 1) })}
              </b>
            </div>
            <div>
              <span>{t("Rift troopers if the trailing team wins")}</span>
              <b>{fmt(rift.troopers_if_trailing_wins, 1)}</b>
            </div>
          </div>
          {rift.own_team_trails && rift.resist > 0 && (
            <p className="muted">
              {t(
                "Your build inside the rift: bullet resist {b1}% → {b2}%, spirit resist {s1}% → {s2}%, debuff resist {d1}% → {d2}% (stacked with the build's own, each letting through a share of what is left).",
                {
                  b1: fmt(stat("bullet_resist"), 1),
                  b2: fmt(stacked(stat("bullet_resist"), rift.resist), 1),
                  s1: fmt(stat("spirit_resist"), 1),
                  s2: fmt(stacked(stat("spirit_resist"), rift.resist), 1),
                  d1: fmt(stat("debuff_resist"), 1),
                  d2: fmt(stacked(stat("debuff_resist"), rift.resist), 1),
                },
              )}
            </p>
          )}
          <p className="muted">
            {t("The comeback bonus also raises the souls a trailing team gets for winning the rift, but how much is not published; the base bounty is split among the team.")}
          </p>
        </div>
      )}
    </aside>
  );
}
