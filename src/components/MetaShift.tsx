import { useEffect, useState } from "react";
import { api, fmt, MetaReport, MetaShiftRow } from "../api";
import { date, percent, t } from "../i18n";
import ErrorNote from "./ErrorNote";

/** Rows shown per list before "show all". */
const SHOWN = 12;

const signed = (v: number, digits = 1) => `${v > 0 ? "+" : v < 0 ? "−" : ""}${fmt(Math.abs(v), digits)}`;

function Rows({ rows, kind }: { rows: MetaShiftRow[]; kind: "heroes" | "items" }) {
  const [all, setAll] = useState(false);
  const [clearOnly, setClearOnly] = useState(true);
  const list = rows.filter((r) => !clearOnly || r.clear);
  const shown = all ? list : list.slice(0, SHOWN);
  return (
    <>
      <label className="field inline">
        <input type="checkbox" checked={clearOnly} onChange={(e) => setClearOnly(e.target.checked)} />
        {t("Only clear changes")}
      </label>
      {list.length === 0 && <p className="muted">{t("No clear change: every difference is within what chance makes with this many matches.")}</p>}
      {list.length > 0 && (
        <table className="match-table meta-table">
          <thead>
            <tr>
              <th className="left">{kind === "heroes" ? t("Hero") : t("Item")}</th>
              <th>{t("Win rate before")}</th>
              <th>{t("After")}</th>
              <th>{t("Change")}</th>
              <th>{t("Picked before")}</th>
              <th>{t("After")}</th>
              <th>{t("Matches after")}</th>
            </tr>
          </thead>
          <tbody>
            {shown.map((r) => (
              <tr key={r.key}>
                <td className="left">
                  <img src={r.icon} alt="" loading="lazy" className="meta-icon" />
                  {r.name}
                  {kind === "items" && <small className="muted"> {fmt(r.cost)}</small>}
                  {!r.clear && <small className="muted" title={t("Within what chance makes with this many matches.")}> ~</small>}
                </td>
                <td>{percent(fmt(r.win_before, 1))}</td>
                <td>{percent(fmt(r.win_after, 1))}</td>
                <td className={r.clear ? (r.win_change > 0 ? "good" : "bad") : "muted"}>
                  <b>{signed(r.win_change)}</b>
                </td>
                <td>{percent(fmt(r.pick_before, 1))}</td>
                <td className="muted">
                  {percent(fmt(r.pick_after, 1))} ({signed(r.pick_change)})
                </td>
                <td>{fmt(r.matches_after)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
      {list.length > SHOWN && (
        <button className="ghost small" onClick={() => setAll(!all)}>
          {all ? t("Show fewer") : t("Show all {n}", { n: list.length })}
        </button>
      )}
    </>
  );
}

/** What a patch did: every hero's and item's win and pick rate in the days before it beside the days after it. */
export default function MetaShift({ badge, maxBadge }: { badge: number; maxBadge: number | null }) {
  const [patch, setPatch] = useState<number | null>(null);
  const [report, setReport] = useState<MetaReport | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [busy, setBusy] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [view, setView] = useState<"heroes" | "items">("heroes");

  useEffect(() => {
    let stale = false;
    setBusy(true);
    setError(null);
    api
      .metaShift(patch, badge, maxBadge)
      .then((r) => !stale && setReport(r))
      .catch((e) => !stale && setError(e))
      .finally(() => !stale && setBusy(false));
    return () => {
      stale = true;
    };
  }, [patch, badge, maxBadge, attempt]);

  const day = (unix: number) => date(unix);
  return (
    <div className="meta-shift">
      <div className="draft-bar">
        <label className="field inline">
          {t("Patch")}
          <select value={report?.patch.day ?? ""} onChange={(e) => setPatch(Number(e.target.value))}>
            {(report?.patches ?? []).map((p) => (
              <option key={p.day} value={p.day}>
                {p.title}
              </option>
            ))}
          </select>
        </label>
        <div className="segmented">
          <button className={view === "heroes" ? "on" : ""} onClick={() => setView("heroes")}>
            {t("Heroes")}
          </button>
          <button className={view === "items" ? "on" : ""} onClick={() => setView("items")}>
            {t("Items")}
          </button>
        </div>
        {busy && <span className="muted">{t("Working…")}</span>}
      </div>
      <ErrorNote error={error} onRetry={() => setAttempt((a) => a + 1)} />
      {report && (
        <>
          <p className="muted">
            {t(
              "The {before} days before the patch of {day} beside the {after} days after it, in the chosen ranks; the patch day itself is left out. A change is marked clear when it is larger than chance would make it with that many matches; the others are marked ~. Win rate moves with the patch, but also with who picks a hero: a hero picked much more often by new players can lose win rate without being weaker.",
              { before: report.days_before, after: report.days_after, day: day(report.patch.day) },
            )}
          </p>
          {report.days_after < 3 && <p className="bad">{t("Only {n} days after the patch yet: the numbers will move as more matches come in.", { n: report.days_after })}</p>}
          <Rows key={view + report.patch.day} rows={view === "heroes" ? report.shift.heroes : report.shift.items} kind={view} />
        </>
      )}
    </div>
  );
}
