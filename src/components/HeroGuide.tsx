import { useEffect, useState } from "react";
import { api, Build, EMPTY_MAP, fmt, Hero, MasterItem, MasterReport, OrderReport, PatchChange, PopularItem } from "../api";
import { readSetting, SETTINGS } from "../settings";
import ErrorNote from "./ErrorNote";
import PowerCurve from "./PowerCurve";
import { percent, t } from "../i18n";

function ItemRow({ items }: { items: MasterItem[] }) {
  return (
    <div className="hero-page-build">
      {items.map((i) => (
        <span
          key={i.item}
          className={"hero-page-item " + i.category + (i.sold ? " sold" : "")}
          title={`${i.name} · ${t("bought by {p}% of top players at this point", { p: fmt(i.popular_pct, 0) })}${i.sold ? ` · ${t("sold later")}` : ""}`}
        >
          <img src={i.icon} alt="" />
          <small>{i.name}</small>
          <small className="muted">{i.sold ? t("sold later") : percent(fmt(i.popular_pct, 0))}</small>
        </span>
      ))}
    </div>
  );
}

/** The hero's build as the Master builder makes it without teams, as a guide: its items phase by phase with the
 *  ones sold to make room, its power curve with the spikes, the ability order top players take, and what the last
 *  patch changed for the hero. */
export default function HeroGuide({ hero, situational }: { hero: Hero; situational: PopularItem[] }) {
  const [report, setReport] = useState<MasterReport | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [order, setOrder] = useState<OrderReport | null>(null);
  const [changes, setChanges] = useState<PatchChange[]>([]);

  useEffect(() => {
    let stale = false;
    setReport(null);
    setError(null);
    api
      .masterBuild(hero.class_name, [], [], [], readSetting(SETTINGS.masterFocus), readSetting(SETTINGS.masterDamage), null)
      .then((r) => {
        if (stale) return;
        setReport(r);
        const input: Build = {
          hero: hero.class_name,
          purchases: r.master.purchases,
          upto: null,
          souls: null,
          active: [],
          ability_tiers: [0, 0, 0, 0],
          target: { health: 2000, bullet_resist: 0, spirit_resist: 0 },
          copied_ult: null,
          imbues: r.master.imbues,
          map: EMPTY_MAP,
        };
        return api.abilityOrder(input).then((o) => !stale && setOrder(o));
      })
      .catch((e) => !stale && setError(e));
    api
      .patchNotes()
      .then((n) => !stale && setChanges((n?.changes ?? []).filter((c) => c.class_name === hero.class_name || c.hero === hero.class_name)))
      .catch(() => {});
    return () => {
      stale = true;
    };
  }, [hero.class_name]);

  const names = hero.abilities?.slice(0, 4).map((a) => a.name) ?? [];
  const icons = hero.abilities?.slice(0, 4).map((a) => a.icon) ?? [];
  const top = order?.popular[0];
  const master = report?.master;
  const owned = new Set(master?.purchases ?? []);
  return (
    <div className="group hero-guide">
      <h3>
        {t("Guide")}{" "}
        {report && <small className="muted">{t("{rank} lobbies · {focus}, built without teams the way the Master builder does", { rank: report.rank, focus: master?.focus_name ?? "" })}</small>}
      </h3>
      <ErrorNote error={error} />
      {!report && !error && <p className="muted">{t("Working…")}</p>}
      {master &&
        master.phases.map((phase, i) => {
          const items = master.items.filter((x) => x.phase === i);
          return items.length > 0 ? (
            <div key={phase.key} className="hero-guide-phase">
              <b>
                {phase.title}
                <br />
                <small className="muted">{t("{a}–{b} souls", { a: fmt(phase.souls_from), b: fmt(phase.souls_to) })}</small>
              </b>
              <div>
                <ItemRow items={items} />
                {phase.sold.length > 0 && <small className="muted">{t("Sold to make room: {items}", { items: phase.sold.join(", ") })}</small>}
              </div>
            </div>
          ) : null;
        })}
      {situational.filter((x) => !owned.has(x.item)).length > 0 && (
        <div className="hero-guide-phase">
          <b>{t("When the match calls for it")}</b>
          <div className="hero-page-build">
            {situational
              .filter((x) => !owned.has(x.item))
              .map((i) => (
                <span key={i.item} className={"hero-page-item " + i.category} title={`${i.name} · ${percent(fmt(i.win_pct, 1))}`}>
                  <img src={i.icon} alt="" />
                  <small>{i.name}</small>
                </span>
              ))}
          </div>
        </div>
      )}
      {top && (
        <div className="hero-guide-phase">
          <b>
            {t("Ability order")} <small className="muted">{t("{n} matches, {w}% won", { n: fmt(top.matches), w: fmt(top.win_pct, 1) })}</small>
          </b>
          <div className="order-steps">
            {top.steps.map(([index, tier], i) => (
              <span key={i}>
                <span className="order-step" title={`${names[index] ?? ""} ${tier}`}>
                  <img src={icons[index]} alt={names[index]} />
                  <b>{tier}</b>
                </span>
              </span>
            ))}
          </div>
        </div>
      )}
      {changes.length > 0 && (
        <div className="hero-guide-phase">
          <b>{t("Changed in the last patch")}</b>
          <ul>
            {changes.map((c) => (
              <li key={c.kind + c.class_name}>
                <b>{c.name}</b>: {c.values.map((v) => `${v.label} ${v.from === null ? "–" : fmt(v.from, 2)} → ${v.to === null ? "–" : fmt(v.to, 2)}${v.postfix}`).join(", ") || (c.status === "added" ? t("new") : t("removed"))}
              </li>
            ))}
          </ul>
        </div>
      )}
      {master && <PowerCurve master={master} />}
    </div>
  );
}
