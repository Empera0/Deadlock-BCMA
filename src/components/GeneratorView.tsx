import { useEffect, useState } from "react";
import { api, Build, fmt, Hero, JudgedItem, MasterBuild, MasterItem, MasterReport, MatchPreset, PopularItem, PopularReport } from "../api";
import SuggestPanel from "./SuggestPanel";

interface Props {
  heroes: Hero[];
  hero: Hero;
  build: Build;
  souls: number;
  /** A match player handed over from the match analysis. */
  preset: MatchPreset | null;
  /** Loads a build into the builder, with the abilities imbued items go into, for a hero when it is not the builder's. */
  onUse: (purchases: string[], what: string, imbues?: Record<string, number>, hero?: string) => void;
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
          build; the Master builder tab works out which ones a given match calls for.
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

/** The roles and damage the master builder can build for. */
const FOCUSES: [string, string, string][] = [
  ["auto", "Auto", "Picked from the teams: tempo when a teammate carries late, carry otherwise"],
  ["carry", "Carry", "Your own damage first: bigger items that scale into the late game"],
  ["tempo", "Tempo", "Power early in the mid game, to fight before their carries come online"],
  ["support", "Support", "Items that answer their team and help yours"],
  ["frontline", "Frontline", "Staying alive at the front while your carries deal the damage"],
];
const DAMAGES: [string, string][] = [
  ["auto", "Auto"],
  ["gun", "Gun"],
  ["spirit", "Spirit"],
  ["hybrid", "Hybrid"],
  ["melee", "Melee"],
];

/** Both teams, the lane and the role: a full build for the player's hero, phase by phase. */
/** The stage of the game an item's price fits. */
const stageOf = (cost: number) => (cost <= 1600 ? "lane" : cost <= 3200 ? "mid game" : "late game");

/** A small item tile for the comparison. */
function MiniItem({ item, cost }: { item: { name: string; icon: string; category: string; description: string }; cost: number }) {
  return (
    <span className={"mini-item " + item.category} title={`${item.name} · ${fmt(cost)} souls\n\n${item.description}`}>
      <img src={item.icon} alt="" />
      <span>
        <b>{item.name}</b>
        <small>{fmt(cost)}</small>
      </span>
    </span>
  );
}

/** Why the suggested item beats the one bought, in a few words each. */
function whyOut(j: JudgedItem): string[] {
  return [
    j.popular_pct >= 0.5 ? `${fmt(j.popular_pct, 0)}% of top players buy it at this point` : "Top players rarely buy it at this point",
    ...(Math.abs(j.win_pp) >= 1 ? [`top players who buy it here win ${fmt(Math.abs(j.win_pp), 0)} points ${j.win_pp > 0 ? "more" : "less"} often than with other items bought here`] : []),
    ...j.notes.filter((n) => n.startsWith("Your team has it") || n.startsWith("Works within") || n.startsWith("Harder")),
  ];
}

function whyIn(x: MasterItem, style: string): string[] {
  return [
    ...(x.popular_pct >= 0.5 ? [`${fmt(x.popular_pct, 0)}% of top players buy it here`] : []),
    ...(Math.abs(x.win_pp) >= 1 ? [`they win ${fmt(Math.abs(x.win_pp), 0)} points ${x.win_pp > 0 ? "more" : "less"} often with it than with other items bought here`] : []),
    ...(x.engine_pct >= 1 ? [`+${fmt(x.engine_pct, 0)}% to your ${style} damage`] : []),
    ...x.jobs.map((j) => j.toLowerCase()),
  ];
}

/** The player's purchases in the match set against the master build, phase by phase: what to buy instead of what. */
function MatchComparison({ master, preset }: { master: MasterBuild; preset: MatchPreset }) {
  const bought = new Set(master.judged.map((j) => j.item));
  return (
    <div className="group compare-match">
      <h3>
        Match {preset.match_id}: {preset.player_name}'s build against the suggested one{" "}
        <small className="muted">
          · ended at {fmt(preset.net_worth)} souls · {preset.won ? "won" : "lost"}
        </small>
      </h3>
      {master.phases.map((phase, i) => {
        const mine = master.judged.filter((j) => j.phase === i);
        const suggested = master.items.filter((x) => x.phase === i);
        const kept = mine.filter((j) => j.in_master);
        const extra = mine.filter((j) => !j.in_master);
        const missing = suggested.filter((x) => !bought.has(x.item)).sort((a, b) => b.score - a.score);
        // Each missing item takes the place of a bought one: the same kind first, then the closest price.
        const left = [...extra];
        const swaps: { out: JudgedItem; in: MasterItem }[] = [];
        const add: MasterItem[] = [];
        for (const m of missing) {
          if (left.length === 0) {
            add.push(m);
            continue;
          }
          const pool = left.some((j) => j.category === m.category) ? left.filter((j) => j.category === m.category) : left;
          const out = pool.reduce((a, b) => (Math.abs(b.cost - m.cost) < Math.abs(a.cost - m.cost) ? b : a));
          left.splice(left.indexOf(out), 1);
          swaps.push({ out, in: m });
        }
        const reached = preset.net_worth > phase.souls_from;
        return (
          <div key={phase.key} className="compare-phase">
            <div className="master-phase-head">
              <b>{phase.title}</b>
              <small>
                {fmt(phase.souls_from)}–{fmt(phase.souls_to)} souls
              </small>
              {!reached && <small className="muted">not reached in this match</small>}
            </div>
            {swaps.map((sw) => (
              <div key={sw.out.item + sw.in.item} className="swap">
                <MiniItem item={sw.out} cost={sw.out.cost} />
                <span className="swap-arrow">→</span>
                <MiniItem item={sw.in} cost={sw.in.cost} />
                <span className="swap-why">
                  <span className="out">
                    {sw.out.name}: {whyOut(sw.out).join("; ")}.
                  </span>
                  <span className="in">
                    {sw.in.name}: {whyIn(sw.in, master.style).join("; ") || "fits the build better"}.
                  </span>
                </span>
              </div>
            ))}
            {add.length > 0 && (
              <div className="compare-line">
                <small>{reached ? "Also buy" : "Next"}:</small>
                {add.map((x) => (
                  <MiniItem key={x.item} item={x} cost={x.cost} />
                ))}
              </div>
            )}
            {left.length > 0 && (
              <div className="compare-line">
                <small>Could skip:</small>
                {left.map((j) => (
                  <span key={j.item} title={whyOut(j).join("\n")}>
                    <MiniItem item={j} cost={j.cost} />
                  </span>
                ))}
              </div>
            )}
            {kept.length > 0 && (
              <div className="compare-line good">
                <small>Good picks:</small>
                {kept.map((j) => (
                  <MiniItem key={j.item} item={j} cost={j.cost} />
                ))}
              </div>
            )}
            {mine.length === 0 && suggested.length === 0 && <small className="muted">Nothing in this phase.</small>}
          </div>
        );
      })}
    </div>
  );
}

function MasterPanel({ heroes, hero, preset, onUse }: { heroes: Hero[]; hero: Hero; preset: MatchPreset | null; onUse: Props["onUse"] }) {
  const [focus, setFocus] = useState("auto");
  const [damage, setDamage] = useState("auto");
  const [enemies, setEnemies] = useState<number[]>([]);
  const [mine, setMine] = useState(hero.class_name);
  useEffect(() => {
    if (!preset) setMine(hero.class_name);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [hero.class_name]);
  const me = heroes.find((h) => h.class_name === mine) ?? hero;
  const [allies, setAllies] = useState<number[]>([]);
  const [lane, setLane] = useState<number[]>([]);
  // The same hero cannot be on both teams.
  useEffect(() => {
    setEnemies((on) => on.filter((id) => id !== me.id));
    setAllies((on) => on.filter((id) => id !== me.id));
  }, [me.id]);
  useEffect(() => setLane((on) => on.filter((id) => enemies.includes(id))), [enemies]);
  const [report, setReport] = useState<MasterReport | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  // Click: enemy team. Right-click: your team. A hero moves from one team to the other.
  const toggle = (id: number) => {
    if (id === me.id) return;
    setAllies((on) => on.filter((x) => x !== id));
    setEnemies((on) => (on.includes(id) ? on.filter((x) => x !== id) : on.length >= 6 ? on : [...on, id]));
  };
  const toggleAlly = (id: number) => {
    if (id === me.id) return;
    setEnemies((on) => on.filter((x) => x !== id));
    setAllies((on) => (on.includes(id) ? on.filter((x) => x !== id) : on.length >= 5 ? on : [...on, id]));
  };
  const toggleLane = (id: number) => setLane((on) => (on.includes(id) ? on.filter((x) => x !== id) : on.length >= 3 ? on : [...on, id]));
  // Playing a teammate: the old hero takes its place on the team.
  const playAs = (h: Hero) => {
    setAllies((on) => [...on.filter((x) => x !== h.id), me.id]);
    setMine(h.class_name);
  };
  const byId = (id: number) => heroes.find((h) => h.id === id);
  const run = (who: string, al: number[], en: number[], ln: number[]) => {
    setBusy(true);
    setError("");
    const bought = preset && preset.hero === who ? preset.bought.map((b) => [b.item, b.net_worth] as [string, number]) : null;
    api
      .masterBuild(who, al, en, ln, focus, damage, bought)
      .then(setReport)
      .catch((e) => setError(String(e)))
      .finally(() => setBusy(false));
  };
  const find = () => run(mine, allies, enemies, lane);
  // A player handed over from a match: both teams, the lane, and a build to set the purchases against.
  useEffect(() => {
    if (!preset) return;
    const ids = (classes: string[]) => classes.map((c) => heroes.find((h) => h.class_name === c)?.id).filter((x): x is number => x !== undefined);
    const al = ids(preset.allies);
    const en = ids(preset.enemies);
    const ln = ids(preset.lane);
    setMine(preset.hero);
    setAllies(al);
    setEnemies(en);
    setLane(ln);
    run(preset.hero, al, en, ln);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [preset?.key]);

  return (
    <>
      <p className="muted">
        Click a hero for the enemy team, right-click one for your team, mark the enemies in your lane and pick your role. Everyone else is
        taken to build what top players build. Your build is put together phase by phase by souls (lane to 6k, first rift to 15k, second
        rift to 25k, all boons to 40k), weighing for every item how many top players of your hero buy it then, what it adds to your own
        numbers, and what it answers on their team, less what your teammates already bring.
      </p>
      <div className="enemy-grid">
        {heroes.map((h) => (
          <button
            key={h.id}
            className={
              "enemy-cell" + (enemies.includes(h.id) ? " on" : "") + (allies.includes(h.id) ? " ally" : "") + (h.id === me.id ? " mine" : "")
            }
            onClick={() => toggle(h.id)}
            onContextMenu={(e) => {
              e.preventDefault();
              toggleAlly(h.id);
            }}
            title={h.id === me.id ? `You play ${h.name}` : `${h.name}: click for the enemy team, right-click for your team`}
          >
            <img src={h.icon} alt="" loading="lazy" />
            <span>{h.name}</span>
          </button>
        ))}
      </div>
      <div className="teams">
        <div className="team-row">
          <span className="team-label">Your team</span>
          <span className="team-chip you" title="You">
            <img src={me.icon} alt="" />
            {me.name}
            <small>you</small>
          </span>
          {allies.map((id) => {
            const h = byId(id);
            return (
              h && (
                <button key={id} className="team-chip" onClick={() => playAs(h)} title={`Play ${h.name} instead`}>
                  <img src={h.icon} alt="" />
                  {h.name}
                </button>
              )
            );
          })}
          {allies.length === 0 && <small className="muted">right-click heroes to add your teammates</small>}
        </div>
        <div className="team-row">
          <span className="team-label">Enemy team</span>
          {enemies.map((id) => {
            const h = byId(id);
            return (
              h && (
                <button
                  key={id}
                  className={"team-chip enemy" + (lane.includes(id) ? " lane" : "")}
                  onClick={() => toggleLane(id)}
                  title={lane.includes(id) ? `${h.name} is in your lane` : `Mark ${h.name} as in your lane`}
                >
                  <img src={h.icon} alt="" />
                  {h.name}
                  {lane.includes(id) && <small>lane</small>}
                </button>
              )
            );
          })}
          {enemies.length === 0 ? (
            <small className="muted">click heroes to add the enemy team</small>
          ) : (
            lane.length === 0 && <small className="muted">click the enemies in your lane</small>
          )}
        </div>
        <div className="team-row">
          <span className="team-label">Your role</span>
          {FOCUSES.map(([key, name, goal]) => (
            <button key={key} className={"choice" + (focus === key ? " on" : "")} title={goal} onClick={() => setFocus(key)}>
              {name}
            </button>
          ))}
        </div>
        <div className="team-row">
          <span className="team-label">Damage</span>
          {DAMAGES.map(([key, name]) => (
            <button key={key} className={"choice" + (damage === key ? " on" : "")} onClick={() => setDamage(key)}>
              {name}
            </button>
          ))}
        </div>
        <div className="gen-actions">
          <button disabled={busy} onClick={find}>
            {busy ? "Working…" : `Build for ${me.name}`}
          </button>
        </div>
      </div>
      {error && <div className="error">{error}</div>}

      {report && (
        <>
          {preset && report.master.hero === preset.hero && report.master.judged.length > 0 && <MatchComparison master={report.master} preset={preset} />}
          <div className="group master">
            <h3>
              {report.master.hero_name} as {report.master.focus_name} <small className="muted">· {report.master.focus_goal} · {report.master.style} damage</small>
            </h3>
            {report.master.notes.map((n) => (
              <p key={n} className="muted team-note">
                {n}.
              </p>
            ))}
            {report.master.phases.map((phase, i) => (
              <div key={phase.key} className="master-phase">
                <div className="master-phase-head">
                  <b>{phase.title}</b>
                  <small>
                    {fmt(phase.souls_from)}–{fmt(phase.souls_to)} souls
                  </small>
                  {phase.sold.length > 0 && <small className="muted">sells {phase.sold.join(", ")}</small>}
                </div>
                <div className="master-items">
                  {report.master.items
                    .filter((x) => x.phase === i)
                    .map((x) => (
                      <div
                        key={x.item}
                        className={"master-item " + x.category + (x.sold || x.upgraded ? " sold" : "")}
                        title={`${x.name} · ${fmt(x.cost)} souls${x.paid < x.cost ? ` (${fmt(x.paid)} as an upgrade)` : ""}\n\n${x.description}${x.notes.length ? "\n\n" + x.notes.join("\n") : ""}`}
                      >
                        <img src={x.icon} alt="" loading="lazy" />
                        <div>
                          <b>{x.name}</b>
                          <small>
                            {fmt(x.cost)}
                            {x.upgrades ? ` · ${fmt(x.paid)} on top of ${x.upgrades}` : ""}
                            {x.imbued ? ` · on ${x.imbued}` : ""}
                            {x.active ? " · active" : ""}
                            {x.sold ? " · sold later" : ""}
                            {x.upgraded ? " · upgraded later" : ""}
                          </small>
                          <span className="master-why">
                            {x.popular_pct >= 0.5 && <i className="why-pop">{fmt(x.popular_pct, 0)}% of top players</i>}
                            {Math.abs(x.win_pp) >= 1 && (
                              <i className={x.win_pp > 0 ? "why-win" : "why-lose"} title={`Top players win ${fmt(x.win_pct, 0)}% of the matches in which they buy it at this point`}>
                                {x.win_pp > 0 ? "+" : "−"}
                                {fmt(Math.abs(x.win_pp), 0)} pts wins
                              </i>
                            )}
                            {x.engine_pct >= 1 && <i className="why-engine">+{fmt(x.engine_pct, 0)}% {report.master.style}</i>}
                            {x.notes.some((n) => n.startsWith("Your team has it")) && <i className="why-team">team has it</i>}
                            {x.jobs.map((j) => (
                              <i key={j} className="why-job">
                                {j}
                              </i>
                            ))}
                          </span>
                        </div>
                      </div>
                    ))}
                </div>
                {phase.considered.length > 0 && (
                  <details className="considered" open>
                    <summary>Why not something else? The best items not bought here</summary>
                    <table className="match-table compare">
                      <thead>
                        <tr>
                          <th className="left">Item</th>
                          <th title="The item's score: popularity, own numbers and the answer to their team, weighed by the role">Score</th>
                          <th title="Share of your hero's top players who buy it at this point">Top players</th>
                          <th title="Win rate when bought at this point, against everything bought at this point (points, shrunk for small samples)">Wins</th>
                          <th title="What it adds to your own damage">Own numbers</th>
                          <th title="Its answer to the enemy team, against the best answer at this point">Answer</th>
                          <th className="left">Why</th>
                        </tr>
                      </thead>
                      <tbody>
                        {phase.considered.map((c) => (
                          <tr key={c.item} className={c.left_out ? "left-out" : ""}>
                            <td className="left">
                              <span className="other-item">
                                <img src={c.icon} alt="" />
                                <span>
                                  <b>{c.name}</b>
                                  <small className="muted">{fmt(c.cost)}</small>
                                </span>
                              </span>
                            </td>
                            <td>{fmt(c.score, 2)}</td>
                            <td>{fmt(c.popular_pct, 0)}%</td>
                            <td>{c.win_pct > 0 ? `${fmt(c.win_pct, 0)}% (${c.win_pp >= 0 ? "+" : "−"}${fmt(Math.abs(c.win_pp), 1)})` : "–"}</td>
                            <td>+{fmt(c.engine_pct, 0)}%</td>
                            <td>{fmt(c.counter_pct, 0)}%</td>
                            <td className="left other-what">
                              {c.left_out && <small className="hard">Few top players buy it at this point and it is no strong answer, so it was left out</small>}
                              {[...c.jobs.map((j) => j.toLowerCase()), ...c.notes].map((n) => (
                                <small key={n}>{n}</small>
                              ))}
                            </td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </details>
                )}
              </div>
            ))}
            <button
              onClick={() =>
                onUse(report.master.purchases, `Loaded the master build for ${report.master.hero_name} as ${report.master.focus_name}`, report.master.imbues, report.master.hero)
              }
            >
              Use this build
            </button>
          </div>

          <details className="counter-details">
            <summary>Counter picks only: two per stage</summary>
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
                        {p.in_core && <small className="tag-core">in your build</small>}
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

          </details>

          <div className="group">
            <h3>Hero by hero</h3>
            {report.plan.team.map((t) => (
              <p key={t} className="muted team-note">
                {t}.
              </p>
            ))}
            <table className="match-table compare hero-answers">
              <thead>
                <tr>
                  <th className="left">Enemy</th>
                  <th className="left">What they bring</th>
                  <th className="left">Answered by</th>
                </tr>
              </thead>
              <tbody>
                {report.plan.heroes.map((h) => {
                  const icon = heroes.find((x) => x.class_name === h.hero)?.icon;
                  return (
                    <tr key={h.hero}>
                      <td className="left">
                        <span className="other-item">
                          {icon && <img src={icon} alt="" />}
                          <span>
                            <b>{h.hero_name}</b>
                            {h.in_lane && <small className="lane-tag">in your lane</small>}
                          </span>
                        </span>
                      </td>
                      <td className="left">
                        <ul className="plain">
                          {h.threats.map((t) => (
                            <li key={t}>{t}</li>
                          ))}
                        </ul>
                      </td>
                      <td className="left">
                        {h.answers.length > 0 ? (
                          <ul className="plain">
                            {h.answers.map((a) => (
                              <li key={a}>{a}</li>
                            ))}
                          </ul>
                        ) : (
                          <span className="muted">Nothing above; see the other counter items</span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>

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
                  <th title="Share of its damage in a fight that is gun and spirit damage, worked out by the calculator with its usual build late in the match">Gun / spirit</th>
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
                      <td className="left">{[...p.mobility, ...p.flying].join(", ") || "–"}</td>
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

/** Builds put together for the hero: by the calculator, from what players build, and for a match by the master builder. */
export default function GeneratorView({ heroes, hero, build, souls, preset, onUse }: Props) {
  const [tab, setTab] = useState<"calculated" | "popular" | "master">(preset ? "master" : "calculated");
  useEffect(() => {
    if (preset) setTab("master");
  }, [preset?.key]);
  return (
    <div className="generator-view">
      <div className="side-tabs gen-tabs">
        <button className={tab === "calculated" ? "on" : ""} onClick={() => setTab("calculated")}>
          Calculated
        </button>
        <button className={tab === "popular" ? "on" : ""} onClick={() => setTab("popular")}>
          Popular
        </button>
        <button className={tab === "master" ? "on" : ""} onClick={() => setTab("master")}>
          Master builder
        </button>
      </div>
      <div className="gen-body">
        {tab === "calculated" && <SuggestPanel hero={hero} build={build} souls={souls} onUse={(p) => onUse(p, "Loaded a build the calculator put together")} />}
        {tab === "popular" && <PopularPanel hero={hero} onUse={onUse} />}
        {tab === "master" && <MasterPanel heroes={heroes} hero={hero} preset={preset} onUse={onUse} />}
      </div>
    </div>
  );
}
