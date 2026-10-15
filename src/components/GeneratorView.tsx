import { useEffect, useState } from "react";
import { api, Build, CounterReport, fmt, Hero, PopularItem, PopularReport } from "../api";
import SuggestPanel from "./SuggestPanel";

interface Props {
  heroes: Hero[];
  hero: Hero;
  build: Build;
  souls: number;
  onUse: (purchases: string[], what: string) => void;
}

const minute = (m: number) => `${Math.floor(m)}:${String(Math.round((m % 1) * 60)).padStart(2, "0")}`;

/** A small item tile with its numbers underneath. */
function Tile({ item, picked, onClick }: { item: PopularItem; picked?: boolean; onClick?: () => void }) {
  return (
    <button
      className={"gen-tile " + item.category + (picked ? " picked" : "")}
      onClick={onClick}
      title={`${item.name} · ${fmt(item.cost)} souls\nBought in ${fmt(item.bought_pct, 0)}% of matches, kept to the end in ${fmt(item.kept_pct, 0)}%, won ${fmt(item.win_pct, 1)}%, bought at ${minute(item.buy_minute)} on average${item.description ? "\n\n" + item.description : ""}`}
    >
      <img src={item.icon} alt="" loading="lazy" />
      {item.active && <em>Active</em>}
      <span>{item.name}</span>
      <small>
        {fmt(item.kept_pct, 0)}% · {minute(item.buy_minute)}
      </small>
    </button>
  );
}

/** The build most players at a good rank go for with this hero, and the picks for the counter slot. */
function PopularPanel({ hero, onUse }: { hero: Hero; onUse: Props["onUse"] }) {
  const [report, setReport] = useState<PopularReport | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [counter, setCounter] = useState<string | null>(null);
  useEffect(() => {
    setBusy(true);
    setError("");
    setCounter(null);
    api
      .popularBuild(hero.id)
      .then(setReport)
      .catch((e) => {
        setReport(null);
        setError(String(e));
      })
      .finally(() => setBusy(false));
  }, [hero.id]);

  if (busy) return <p className="muted">Reading what players build with {hero.name}…</p>;
  if (error) return <div className="error">{error}</div>;
  if (!report) return null;
  const b = report.build;
  const picked = b.situational.find((s) => s.item === counter);
  return (
    <>
      <p className="muted">
        From {fmt(b.matches)} {hero.name} matches of the last {report.days} days ({report.rank} lobbies), read from the community Deadlock API. The core is
        the 8 items most players buy and keep to the end, in the order they are bought; under each: kept in % of matches · bought at.
      </p>
      <div className="group">
        <h3>Core build</h3>
        <div className="gen-tiles">
          {b.core.map((item) => (
            <Tile key={item.item} item={item} />
          ))}
        </div>
      </div>
      <div className="group">
        <h3>Counter slot</h3>
        <p className="muted">
          Bought in a fair share of matches but not most: the item players change depending on the enemy. Pick one to add it to the
          build; the Counters tab says which ones a given enemy team calls for.
        </p>
        <div className="gen-tiles">
          {b.situational.map((item) => (
            <Tile key={item.item} item={item} picked={item.item === counter} onClick={() => setCounter(counter === item.item ? null : item.item)} />
          ))}
        </div>
        {picked && <p className="gen-desc">{picked.description}</p>}
      </div>
      <button
        onClick={() =>
          onUse(
            [...b.core.map((c) => c.item), ...(counter ? [counter] : [])],
            `Loaded the popular ${hero.name} build${picked ? ` with ${picked.name}` : ""}`,
          )
        }
      >
        Use this build{picked ? ` with ${picked.name}` : ""}
      </button>
    </>
  );
}

/** Up to six enemy heroes, and what to buy against them. */
/** The stage of the game an item's price fits. */
const stageOf = (cost: number) => (cost <= 1600 ? "lane" : cost <= 3200 ? "mid game" : "late game");

function CounterPanel({ heroes, hero }: { heroes: Hero[]; hero: Hero }) {
  const [enemies, setEnemies] = useState<number[]>([]);
  const [mine, setMine] = useState(hero.class_name);
  useEffect(() => setMine(hero.class_name), [hero.class_name]);
  const me = heroes.find((h) => h.class_name === mine) ?? hero;
  // The same hero cannot be on both teams.
  useEffect(() => setEnemies((on) => on.filter((id) => id !== me.id)), [me.id]);
  const [report, setReport] = useState<CounterReport | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const toggle = (id: number) => setEnemies((on) => (on.includes(id) ? on.filter((x) => x !== id) : on.length >= 6 ? on : [...on, id]));
  const find = () => {
    setBusy(true);
    setError("");
    api
      .counterAdvice(mine, enemies)
      .then(setReport)
      .catch((e) => setError(String(e)))
      .finally(() => setBusy(false));
  };
  const name = (id: number) => heroes.find((h) => h.id === id)?.name ?? "?";

  return (
    <>
      <p className="muted">
        Click the enemy team (up to six) and right-click the hero you play. Out of twelve slots, two go to counters, so for each stage of the game the engine
        picks the two items that answer the most of what this team does: one item that answers two problems beats two that answer one,
        items that land on their own beat ones that need headshots, a team of healers always gets its healing cut, and nothing your usual
        build already has is suggested.
      </p>
      <div className="enemy-grid">
        {heroes.map((h) => (
          <button
            key={h.id}
            className={"enemy-cell" + (enemies.includes(h.id) ? " on" : "") + (h.id === me.id ? " mine" : "")}
            onClick={() => h.id !== me.id && toggle(h.id)}
            onContextMenu={(e) => {
              e.preventDefault();
              setMine(h.class_name);
            }}
            title={h.id === me.id ? `You play ${h.name}` : `${h.name}: click for the enemy team, right-click to play it`}
          >
            <img src={h.icon} alt="" loading="lazy" />
            <span>{h.name}</span>
          </button>
        ))}
      </div>
      <div className="gen-actions">
        <span className="you-play">
          <img src={me.icon} alt="" />
          You play <b>{me.name}</b>
          <small className="muted">(right-click a hero to change)</small>
        </span>
        <button disabled={busy || enemies.length === 0} onClick={find}>
          {busy ? "Working…" : `Find counters against ${enemies.length || "the"} enem${enemies.length === 1 ? "y" : "ies"}`}
        </button>
        {enemies.length > 0 && <small className="muted">{enemies.map(name).join(", ")}</small>}
      </div>
      {error && <div className="error">{error}</div>}

      {report && (
        <>
          {report.plan.stages.map((stage) => (
            <div key={stage.key} className="group stage-group">
              <h3>
                {stage.title} <small className="muted">· {stage.note}</small>
              </h3>
              <div className="pick-row">
                {stage.picks.map((p, i) => (
                  <div key={p.advice.item} className={"pick-card " + p.advice.category}>
                    <span className="pick-rank">{i + 1}</span>
                    <img src={p.advice.icon} alt="" loading="lazy" />
                    <div>
                      <div className="counter-head">
                        <b>{p.advice.name}</b>
                        <small>{fmt(p.advice.cost)} souls</small>
                        {p.advice.active && <small className="tag-active">active</small>}
                      </div>
                      <div className="pick-jobs">{p.jobs.join(" · ")}</div>
                      <div className="counter-effect">{p.advice.effect}</div>
                      <p className="counter-what">{p.advice.description}</p>
                      <ul>
                        {p.advice.reasons.map((r) => (
                          <li key={r} className={r.startsWith("Top players") ? "lift" : r.startsWith("Harder") ? "hard" : ""}>
                            {r}
                          </li>
                        ))}
                      </ul>
                    </div>
                  </div>
                ))}
              </div>
              {stage.alternatives.length > 0 && (
                <div className="pick-alts">
                  <small>If those do not fit:</small>
                  {stage.alternatives.map((p) => (
                    <span key={p.advice.item} className="pick-alt" title={`${p.advice.effect}\n\n${p.advice.description}`}>
                      <img src={p.advice.icon} alt="" />
                      {p.advice.name} <small>{p.jobs.join(", ")}</small>
                    </span>
                  ))}
                </div>
              )}
            </div>
          ))}

          {report.plan.others.length > 0 && (
            <div className="group">
              <h3>Other counter items</h3>
              <p className="muted">
                Everything else that answers something this team does, best first. Swap one in when a pick above does not suit how the
                match is going.
              </p>
              <table className="match-table compare other-counters">
                <thead>
                  <tr>
                    <th className="left">Item</th>
                    <th>Souls</th>
                    <th className="left">Answers</th>
                    <th className="left">What it does</th>
                    <th className="left">Against</th>
                  </tr>
                </thead>
                <tbody>
                  {report.plan.others.map((o) => {
                    const notes = o.advice.reasons.filter((r) => r.startsWith("Harder") || r.startsWith("Top players"));
                    return (
                      <tr key={o.advice.item} className={o.advice.category}>
                        <td className="left">
                          <span className="other-item">
                            <img src={o.advice.icon} alt="" loading="lazy" />
                            <span>
                              <b>{o.advice.name}</b>
                              {o.advice.active && <small className="tag-active">active</small>}
                              <small className="muted">{stageOf(o.advice.cost)}</small>
                            </span>
                          </span>
                        </td>
                        <td>{fmt(o.advice.cost)}</td>
                        <td className="left">
                          <div className="pick-jobs">{o.jobs.join(" · ")}</div>
                          <div className="counter-effect">{o.advice.effect}</div>
                        </td>
                        <td className="left other-what">
                          {o.advice.description}
                          {notes.map((n) => (
                            <small key={n} className={n.startsWith("Harder") ? "hard" : "lift"}>
                              {n}
                            </small>
                          ))}
                        </td>
                        <td className="left other-against">{o.advice.against.join(", ")}</td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}

          <details className="counter-details">
            <summary>The enemy team and every option, job by job</summary>
          <div className="group">
            <h3>The enemy team</h3>
            <p className="muted">
              Read from their kits and from what they build in {report.rank} lobbies over the last {report.days} days.
              {report.plan.own_escape.length > 0 && ` ${me.name} can get out of trouble with ${report.plan.own_escape.join(", ")}, so answers to crowd control count for less.`}
            </p>
            <table className="match-table compare enemy-table">
              <thead>
                <tr>
                  <th className="left">Hero</th>
                  <th className="left">Usual build</th>
                  <th title="Share of the usual build's weapon and spirit souls">Gun / spirit</th>
                  <th className="left">Heals with</th>
                  <th className="left">Moves with</th>
                  <th className="left">Locks down with</th>
                </tr>
              </thead>
              <tbody>
                {report.plan.enemies.map((p) => {
                  const b = report.builds.find((x) => x.hero === p.hero);
                  return (
                    <tr key={p.hero}>
                      <td className="left">
                        <b>{p.hero_name}</b>
                      </td>
                      <td className="left">
                        <span className="enemy-build-items">
                          {b?.core.map((c) => (
                            <img key={c.item} src={c.icon} alt={c.name} title={`${c.name} · kept in ${fmt(c.kept_pct, 0)}% · bought at ${minute(c.buy_minute)}`} />
                          ))}
                        </span>
                      </td>
                      <td>
                        {fmt(Math.abs(p.weapon_pct), 0)}% / {fmt(Math.abs(100 - p.weapon_pct), 0)}%
                      </td>
                      <td className="left">{p.heals.join(", ") || "–"}</td>
                      <td className="left">{p.mobility.join(", ") || "–"}</td>
                      <td className="left">{p.lockdown.join(", ") || "–"}</td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

          {report.plan.roles.map((role) => (
            <div key={role.key} className="group">
              <h3>{role.title}</h3>
              <p className="role-need">{role.need}.</p>
              <div className="counter-row">
                {role.items.map((a) => (
                  <div key={a.item} className={"counter-card " + a.category} title={a.description}>
                    <img src={a.icon} alt="" loading="lazy" />
                    <div>
                      <div className="counter-head">
                        <b>{a.name}</b>
                        <small>{fmt(a.cost)}</small>
                        {a.active && <small className="tag-active">active</small>}
                      </div>
                      <div className="counter-effect">{a.effect}</div>
                      <p className="counter-what">{a.description || "No description in the game data."}</p>
                      {a.reasons
                        .filter((r) => r.startsWith("Top players"))
                        .map((r) => (
                          <small key={r} className="counter-lift">
                            {r}
                          </small>
                        ))}
                      <small className="counter-when">{a.when}</small>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ))}
          </details>
        </>
      )}
    </>
  );
}

/** Builds put together for the hero: by the calculator, from what players build, and against an enemy team. */
export default function GeneratorView({ heroes, hero, build, souls, onUse }: Props) {
  const [tab, setTab] = useState<"calculated" | "popular" | "counters">("calculated");
  return (
    <div className="generator-view">
      <div className="side-tabs gen-tabs">
        <button className={tab === "calculated" ? "on" : ""} onClick={() => setTab("calculated")}>
          Calculated
        </button>
        <button className={tab === "popular" ? "on" : ""} onClick={() => setTab("popular")}>
          Popular
        </button>
        <button className={tab === "counters" ? "on" : ""} onClick={() => setTab("counters")}>
          Counters
        </button>
      </div>
      <div className="gen-body">
        {tab === "calculated" && <SuggestPanel hero={hero} build={build} souls={souls} onUse={(p) => onUse(p, "Loaded a build the calculator put together")} />}
        {tab === "popular" && <PopularPanel hero={hero} onUse={onUse} />}
        {tab === "counters" && <CounterPanel heroes={heroes} hero={hero} />}
      </div>
    </div>
  );
}
