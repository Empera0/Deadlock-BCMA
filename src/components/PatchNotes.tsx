import { useEffect, useMemo, useState } from "react";
import { api, fmt, PatchChange, PatchNotes as Notes, PatchValue } from "../api";
import { date, percent, t, tn } from "../i18n";

const SEEN_KEY = "patch-seen";

/** A value's change: old → new, and by how much. */
function Change({ v }: { v: PatchValue }) {
  const unit = (n: number) => `${fmt(n, 2)}${v.postfix}`;
  if (v.from === null) return <>{t("new: {v}", { v: unit(v.to ?? 0) })}</>;
  if (v.to === null) return <>{t("gone (was {v})", { v: unit(v.from) })}</>;
  const by = v.to - v.from;
  const pct = v.from !== 0 ? (by / Math.abs(v.from)) * 100 : null;
  return (
    <>
      {unit(v.from)} → <b>{unit(v.to)}</b>{" "}
      <small className="muted">
        ({by >= 0 ? "+" : "−"}
        {fmt(Math.abs(by), 2)}
        {pct !== null && `, ${by >= 0 ? "+" : "−"}${percent(fmt(Math.abs(pct), 0))}`})
      </small>
    </>
  );
}

function Entry({ c, mine }: { c: PatchChange; mine: boolean }) {
  return (
    <div className={"patch-entry" + (mine ? " mine" : "")}>
      <div className="patch-head">
        <img src={c.icon} alt="" />
        <strong>{c.name}</strong>
        {c.owner && <span className="muted">{c.owner}</span>}
        {c.status !== "changed" && <span className={"patch-tag " + c.status}>{c.status === "added" ? t("new") : t("removed")}</span>}
        {mine && <span className="patch-tag mine">{t("in your builds")}</span>}
      </div>
      {c.values.length > 0 && (
        <ul>
          {c.values.map((v) => (
            <li key={v.label}>
              {v.label}: <Change v={v} />
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** What the last patch changed, from the game data before and after: a button in the side bar, marked until seen. */
export default function PatchNotes({ dataAt }: { dataAt: number }) {
  const [notes, setNotes] = useState<Notes | null>(null);
  const [open, setOpen] = useState(false);
  const [seen, setSeen] = useState(() => {
    try {
      return localStorage.getItem(SEEN_KEY) ?? "";
    } catch {
      return "";
    }
  });
  // The heroes and items of the player's saved builds, to mark what touches them.
  const [used, setUsed] = useState<Set<string>>(new Set());

  // Read again whenever the game data changes: a refresh may have brought a patch.
  useEffect(() => {
    api
      .patchNotes()
      .then(setNotes)
      .catch(() => {});
  }, [dataAt]);
  useEffect(() => {
    if (!open) return;
    api
      .listBuilds()
      .then((names) => Promise.all(names.map((n) => api.loadBuild(n).catch(() => null))))
      .then((builds) => setUsed(new Set(builds.flatMap((b) => (b ? [b.hero, ...b.purchases] : [])))))
      .catch(() => {});
  }, [open]);

  const groups = useMemo(() => {
    const changes = notes?.changes ?? [];
    return [
      { title: t("Heroes"), list: changes.filter((c) => c.kind !== "item") },
      { title: t("Items"), list: changes.filter((c) => c.kind === "item") },
    ].filter((g) => g.list.length > 0);
  }, [notes]);

  if (!notes || notes.changes.length === 0) return null;
  const fresh = seen !== String(notes.to_fetched);
  const show = () => {
    setOpen(true);
    setSeen(String(notes.to_fetched));
    try {
      localStorage.setItem(SEEN_KEY, String(notes.to_fetched));
    } catch {
      /* Shown as new again next time. */
    }
  };
  const touches = (c: PatchChange) => used.has(c.class_name) || (c.hero !== null && used.has(c.hero));
  const mine = notes.changes.filter(touches).length;

  return (
    <>
      <button className={"ghost" + (fresh ? " patch-fresh" : "")} onClick={show} title={t("What the last game patch changed in heroes, abilities and items")}>
        {t("Patch changes")}
        {fresh && <span className="patch-dot" />}
      </button>
      {open && (
        <div className="overlay" onClick={() => setOpen(false)}>
          <div className="whats-new" role="dialog" aria-label={t("Patch changes")} onClick={(e) => e.stopPropagation()}>
            <div className="whats-new-head">
              <b>
                {t("Game version {a} → {b}", { a: notes.from_version ?? "?", b: notes.to_version ?? "?" })}{" "}
                <small className="muted">{t("read on {date}", { date: date(notes.to_fetched) })}</small>
              </b>
              <button className="ghost small" onClick={() => setOpen(false)}>
                {t("Close")}
              </button>
            </div>
            <div className="whats-new-body">
              <p className="muted">
                {tn(notes.changes.length, "{n} thing changed, read from the game's own numbers.", "{n} things changed, read from the game's own numbers.")}{" "}
                {mine > 0 && tn(mine, "{n} of them is in your saved builds.", "{n} of them are in your saved builds.")}
              </p>
              {groups.map((g) => (
                <section key={g.title}>
                  <h3>{g.title}</h3>
                  {[...g.list]
                    .sort((a, b) => Number(touches(b)) - Number(touches(a)))
                    .map((c) => (
                      <Entry key={c.kind + c.class_name} c={c} mine={touches(c)} />
                    ))}
                </section>
              ))}
            </div>
          </div>
        </div>
      )}
    </>
  );
}
