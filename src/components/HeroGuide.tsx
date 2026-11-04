import { useEffect, useState } from "react";
import { api, Build, EMPTY_MAP, fmt, Hero, OrderReport, PatchChange, PopularItem, PopularReport } from "../api";
import { percent, t } from "../i18n";

/** The parts of a match the core items are sorted into, by the minute top players buy them: up to, in minutes. */
const PHASES: [string, number][] = [
  ["Laning (0–10 min)", 10],
  ["Mid game (10–20 min)", 20],
  ["Late game (20+ min)", Infinity],
];
/** A purchase that raises the fight damage by this share or more is a power spike. */
const SPIKE = 0.15;

function ItemRow({ items }: { items: PopularItem[] }) {
  return (
    <div className="hero-page-build">
      {items.map((i) => (
        <span key={i.item} className={"hero-page-item " + i.category} title={`${i.name} · ${t("bought at {m} min", { m: fmt(i.buy_minute, 0) })} · ${percent(fmt(i.win_pct, 1))}`}>
          <img src={i.icon} alt="" />
          <small>{i.name}</small>
          <small className="muted">{t("{m} min", { m: fmt(i.buy_minute, 0) })}</small>
        </span>
      ))}
    </div>
  );
}

/** The build top players go with as a guide: its items by part of the match, where it spikes, the ability order
 *  they take, and what the last patch changed for the hero. */
export default function HeroGuide({ hero, build }: { hero: Hero; build: PopularReport }) {
  const [order, setOrder] = useState<OrderReport | null>(null);
  const [spikes, setSpikes] = useState<{ item: string; gain: number }[] | null>(null);
  const [changes, setChanges] = useState<PatchChange[]>([]);
  const core = [...build.build.core].sort((a, b) => a.buy_minute - b.buy_minute);

  useEffect(() => {
    let stale = false;
    const input: Build = {
      hero: hero.class_name,
      purchases: core.map((i) => i.item),
      upto: null,
      souls: null,
      active: [],
      ability_tiers: [0, 0, 0, 0],
      target: { health: 2000, bullet_resist: 0, spirit_resist: 0 },
      copied_ult: null,
      imbues: {},
      map: EMPTY_MAP,
    };
    api
      .abilityOrder(input)
      .then((o) => !stale && setOrder(o))
      .catch(() => {});
    // The fight damage after each purchase, for the spikes.
    api
      .powerCurve(core.map((_, i) => ({ ...input, upto: i + 1, ability_tiers: [1, 1, 1, 1] })))
      .then((rows) => {
        if (stale) return;
        const out: { item: string; gain: number }[] = [];
        rows.forEach((r, i) => {
          const before = i > 0 ? rows[i - 1] : null;
          if (r && before && before.fight_damage > 0 && r.fight_damage / before.fight_damage - 1 >= SPIKE) out.push({ item: core[i].name, gain: r.fight_damage / before.fight_damage - 1 });
        });
        setSpikes(out);
      })
      .catch(() => !stale && setSpikes([]));
    api
      .patchNotes()
      .then((n) => !stale && setChanges((n?.changes ?? []).filter((c) => c.class_name === hero.class_name || c.hero === hero.class_name)))
      .catch(() => {});
    return () => {
      stale = true;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hero.class_name, build]);

  const names = hero.abilities?.slice(0, 4).map((a) => a.name) ?? [];
  const icons = hero.abilities?.slice(0, 4).map((a) => a.icon) ?? [];
  const top = order?.popular[0];
  let from = 0;
  return (
    <div className="group hero-guide">
      <h3>
        {t("Guide")} <small className="muted">{t("{rank} lobbies, {n} matches", { rank: build.rank, n: fmt(build.build.matches) })}</small>
      </h3>
      {PHASES.map(([name, to]) => {
        const items = core.filter((i) => i.buy_minute >= from && i.buy_minute < to);
        from = to;
        return items.length > 0 ? (
          <div key={name} className="hero-guide-phase">
            <b>{t(name)}</b>
            <ItemRow items={items} />
          </div>
        ) : null;
      })}
      {build.build.situational.length > 0 && (
        <div className="hero-guide-phase">
          <b>{t("When the match calls for it")}</b>
          <ItemRow items={build.build.situational} />
        </div>
      )}
      <div className="hero-guide-phase">
        <b>{t("Power spikes")}</b>
        {spikes === null ? (
          <span className="muted">{t("Working…")}</span>
        ) : spikes.length === 0 ? (
          <span className="muted">{t("No purchase stands out: the build grows evenly.")}</span>
        ) : (
          <span>{spikes.map((s) => t("{item} (+{p}% fight damage)", { item: s.item, p: fmt(s.gain * 100, 0) })).join(" · ")}</span>
        )}
      </div>
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
    </div>
  );
}
