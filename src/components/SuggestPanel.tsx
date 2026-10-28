import { useEffect, useState } from "react";
import { api, Build, fmt, Hero, Suggestions } from "../api";
import ErrorNote from "./ErrorNote";
import { t } from "../i18n";

interface Props {
  hero: Hero;
  build: Build;
  souls: number;
  onUse: (purchases: string[]) => void;
}

/** The playstyles a build can be put together for; translated where shown. */
const STYLES: [string, string, string][] = [
  ["hybrid", "Full kit", "Abilities, active items and the gun together"],
  ["gun", "Gun carry", "Shooting"],
  ["spirit", "Spirit burst", "Abilities and active items, no gun"],
  ["melee", "Melee brawler", "Abilities and active items, then heavy and light melee"],
  ["frontline", "Frontline", "Kit damage and staying alive in balance"],
];

/** Builds the calculator puts together itself for the hero, at the souls the builder is set to. */
export default function SuggestPanel({ hero, build, souls, onUse }: Props) {
  const [result, setResult] = useState<Suggestions | null>(null);
  const [made, setMade] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [styles, setStyles] = useState<string[]>(STYLES.map((s) => s[0]));
  // What the builds depend on: when it changes, the ones on screen are for something else.
  const key = [hero.class_name, souls, build.ability_tiers.join(""), build.target.health, build.target.bullet_resist, build.target.spirit_resist, styles.join(",")].join("|");
  useEffect(() => setError(""), [key]);

  const make = () => {
    setBusy(true);
    setError("");
    api
      .suggestBuilds({ ...build, souls }, styles)
      .then((r) => {
        setResult(r);
        setMade(key);
      })
      .catch((e) => setError(String(e)))
      .finally(() => setBusy(false));
  };
  const toggle = (style: string) => setStyles((on) => (on.includes(style) ? on.filter((s) => s !== style) : [...on, style]));
  const change = (before: number, after: number) => `${fmt(before, 0)} → ${fmt(after, 0)}`;
  // The window a playstyle is judged by.
  const own: Record<string, number> = { hybrid: 0, gun: 1, spirit: 2, melee: 3, frontline: 0 };

  return (
    <aside className="panel">
      <div className="group">
        <h3>{t("Builds for {hero}", { hero: hero.name })}</h3>
        <div className="calc-warning">
          <b>{t("These are not builds to play in a match.")}</b>{" "}
          {t(
            "They are the items that push the hero's main stats as high as they can go, picked by the numbers alone. Please do not play these builds as they are. They can point you to items worth having in the builds you make, and for a newly released hero they give a general idea of what to buy for the late game.",
          )}
        </div>
        <div className="style-picker">
          {STYLES.map(([key, name, what]) => (
            <button key={key} className={styles.includes(key) ? "on" : ""} title={t(what)} onClick={() => toggle(key)}>
              {t(name)}
            </button>
          ))}
        </div>
        <button disabled={busy || styles.length === 0} onClick={make}>
          {busy ? t("Working…") : t("Put builds together at {n} souls", { n: fmt(souls) })}
        </button>
        <ErrorNote error={error} onRetry={make} />
        {result && made !== key && <p className="muted">{t("The builds below were made for different souls, upgrades, target or playstyles.")}</p>}
      </div>

      {result && (
        <>
          <p className="muted">
            {t("Stages end at {a} souls (9:00), {b} (20:00) and {c} (30:00):", {
              a: fmt(result.stage_souls.lane),
              b: fmt(result.stage_souls.mid),
              c: fmt(result.stage_souls.late),
            })}{" "}
            {result.stage_souls.matches > 0
              ? t("the median of {n} players in the {m} matches looked at in this app.", { n: result.stage_souls.players, m: result.stage_souls.matches })
              : t("built-in values; open some matches in Match analysis and the stages will follow them.")}
          </p>
          <div className="suggested-numbers">
            <span>{t("Without items:")}</span>
            {result.windows.map((w, i) => (
              <span key={w}>
                {w} <b>{fmt(result.bare[i], 0)}</b>
              </span>
            ))}
          </div>
          {result.builds.map((b) => (
            <div key={b.key} className="group suggested">
              <h3>
                {b.name}
                <small>
                  {t("{a} of {b} souls", { a: fmt(b.spent), b: fmt(b.budget) })}
                </small>
              </h3>
              <p className="muted">{b.goal}.</p>
              <div className="suggested-numbers">
                {result.windows.map((w, i) => (
                  <span key={w} className={own[b.key] === i ? "own" : ""}>
                    {w} <b>{change(b.damage_before[i], b.damage_after[i])}</b>
                  </span>
                ))}
                <span className={b.key === "frontline" ? "own" : ""} title={t("Health divided by what gets through the resists, without anything an active adds")}>
                  {t("Effective health")} <b>{change(b.effective_health_before, b.effective_health_after)}</b>
                </span>
              </div>
              {b.stages.map((stage, s) => (
                <div key={stage.name} className="suggested-stage">
                  <div className="suggested-stage-head">
                    <b>{stage.name}</b>
                    <small>
                      {t("{a} of {b} souls", { a: fmt(stage.spent), b: fmt(stage.souls) })} ·{" "}
                      {stage.min_cost > 0
                        ? t("items {a}–{b}", { a: fmt(stage.min_cost), b: fmt(stage.max_cost) })
                        : t("items up to {n}", { n: fmt(stage.max_cost) })}
                    </small>
                    {stage.sold.length > 0 && <small className="warn">{t("sell {items}", { items: stage.sold.join(", ") })}</small>}
                  </div>
                  <div className="suggested-items">
                    {b.items
                      .filter((i) => i.stage === s)
                      .map((i) => (
                        <div
                          key={i.item}
                          className={"suggested-item " + i.category + (i.sold ? " sold" : "") + (i.upgraded ? " upgraded" : "")}
                          title={[
                            `${i.name} · ${t("{n} souls", { n: fmt(i.cost) })} · ${t("{n}% of what the build gains", { n: fmt(i.share, 0) })}`,
                            i.active ? t("Active: has to be used; counted as used once.") : "",
                            i.conditional ? t("Part of it needs a condition; that part is counted for half.") : "",
                            i.sold ? t("Sold later to make room.") : i.upgraded ? t("Goes into an upgrade bought later.") : "",
                            i.description,
                          ]
                            .filter(Boolean)
                            .join("\n")}
                        >
                          <img src={i.icon} alt="" loading="lazy" />
                          {i.active && <em>{t("Active")}</em>}
                          <span>{i.name}</span>
                        </div>
                      ))}
                  </div>
                </div>
              ))}
              <details className="suggested-why">
                <summary>{t("What each item does and why it was picked")}</summary>
                {b.items.map((i) => (
                  <p key={i.item}>
                    <b>{i.name}</b>{" "}
                    <small>
                      ({b.stages[i.stage]?.name}, {t("{n}% of the gain", { n: fmt(i.share, 0) })}
                      {i.sold ? `, ${t("sold later")}` : i.upgraded ? `, ${t("upgraded later")}` : ""})
                    </small>
                    {i.active && <small className="tag-active">  {t("active")}</small>}
                    {i.conditional && <small className="tag-cond">  {t("conditional")}</small>} {i.description || t("No description in the game data.")}
                  </p>
                ))}
              </details>
              <button onClick={() => onUse(b.purchases)}>{t("Use this build")}</button>
            </div>
          ))}
        </>
      )}
    </aside>
  );
}
