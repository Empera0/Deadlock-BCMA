import { useEffect, useState } from "react";
import { api, Build, fmt, Hero, Suggestions } from "../api";
import ErrorNote from "./ErrorNote";

interface Props {
  hero: Hero;
  build: Build;
  souls: number;
  onUse: (purchases: string[]) => void;
}

/** The playstyles a build can be put together for. */
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
        <h3>Builds for {hero.name}</h3>
        <div className="calc-warning">
          <b>These are not builds to play in a match.</b> They are the items that push the hero's main stats as high as they can go,
          picked by the numbers alone. Please do not play these builds as they are. They can point you to items worth having in the builds
          you make, and for a newly released hero they give a general idea of what to buy for the late game.
        </div>
        <div className="style-picker">
          {STYLES.map(([key, name, what]) => (
            <button key={key} className={styles.includes(key) ? "on" : ""} title={what} onClick={() => toggle(key)}>
              {name}
            </button>
          ))}
        </div>
        <button disabled={busy || styles.length === 0} onClick={make}>
          {busy ? "Working…" : `Put builds together at ${fmt(souls)} souls`}
        </button>
        <ErrorNote error={error} onRetry={make} />
        {result && made !== key && <p className="muted">The builds below were made for different souls, upgrades, target or playstyles.</p>}
      </div>

      {result && (
        <>
          <p className="muted">
            Stages end at {fmt(result.stage_souls.lane)} souls (9:00), {fmt(result.stage_souls.mid)} (20:00) and {fmt(result.stage_souls.late)} (30:00):{" "}
            {result.stage_souls.matches > 0
              ? `the median of ${result.stage_souls.players} players in the ${result.stage_souls.matches} matches looked at in this app.`
              : "built-in values; open some matches in Match analysis and the stages will follow them."}
          </p>
          <div className="suggested-numbers">
            <span>Without items:</span>
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
                  {fmt(b.spent)} of {fmt(b.budget)} souls
                </small>
              </h3>
              <p className="muted">{b.goal}.</p>
              <div className="suggested-numbers">
                {result.windows.map((w, i) => (
                  <span key={w} className={own[b.key] === i ? "own" : ""}>
                    {w} <b>{change(b.damage_before[i], b.damage_after[i])}</b>
                  </span>
                ))}
                <span className={b.key === "frontline" ? "own" : ""} title="Health divided by what gets through the resists, without anything an active adds">
                  Effective health <b>{change(b.effective_health_before, b.effective_health_after)}</b>
                </span>
              </div>
              {b.stages.map((stage, s) => (
                <div key={stage.name} className="suggested-stage">
                  <div className="suggested-stage-head">
                    <b>{stage.name}</b>
                    <small>
                      {fmt(stage.spent)} of {fmt(stage.souls)} souls · items {stage.min_cost > 0 ? `${fmt(stage.min_cost)}–` : "up to "}
                      {fmt(stage.max_cost)}
                    </small>
                    {stage.sold.length > 0 && <small className="warn">sell {stage.sold.join(", ")}</small>}
                  </div>
                  <div className="suggested-items">
                    {b.items
                      .filter((i) => i.stage === s)
                      .map((i) => (
                        <div
                          key={i.item}
                          className={"suggested-item " + i.category + (i.sold ? " sold" : "") + (i.upgraded ? " upgraded" : "")}
                          title={[
                            `${i.name} · ${fmt(i.cost)} souls · ${fmt(i.share, 0)}% of what the build gains`,
                            i.active ? "Active: has to be used; counted as used once." : "",
                            i.conditional ? "Part of it needs a condition; that part is counted for half." : "",
                            i.sold ? "Sold later to make room." : i.upgraded ? "Goes into an upgrade bought later." : "",
                            i.description,
                          ]
                            .filter(Boolean)
                            .join("\n")}
                        >
                          <img src={i.icon} alt="" loading="lazy" />
                          {i.active && <em>Active</em>}
                          <span>{i.name}</span>
                        </div>
                      ))}
                  </div>
                </div>
              ))}
              <details className="suggested-why">
                <summary>What each item does and why it was picked</summary>
                {b.items.map((i) => (
                  <p key={i.item}>
                    <b>{i.name}</b>{" "}
                    <small>
                      ({b.stages[i.stage]?.name}, {fmt(i.share, 0)}% of the gain{i.sold ? ", sold later" : i.upgraded ? ", upgraded later" : ""})
                    </small>
                    {i.active && <small className="tag-active"> active</small>}
                    {i.conditional && <small className="tag-cond"> conditional</small>} {i.description || "No description in the game data."}
                  </p>
                ))}
              </details>
              <button onClick={() => onUse(b.purchases)}>Use this build</button>
            </div>
          ))}
        </>
      )}
    </aside>
  );
}
