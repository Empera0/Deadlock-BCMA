import { useEffect, useRef, useState } from "react";
import { api, Build, EMPTY_MAP, fmt, Hero, JudgedItem, LiveMatch, MasterBuild, MasterItem, MasterReport, MatchPreset, PopularItem, PopularReport } from "../api";
import PowerCurve from "./PowerCurve";
import LobbyScout from "./LobbyScout";
import { CONDEBUG_HELP, findLiveMatch, savedAccount } from "../live";
import { readSetting, SETTINGS, useSetting } from "../settings";
import SuggestPanel from "./SuggestPanel";
import ErrorNote from "./ErrorNote";
import { LOCALE, t } from "../i18n";
import { startsLike } from "../engineText";

interface Props {
  heroes: Hero[];
  hero: Hero;
  build: Build;
  souls: number;
  /** A match player handed over from the match analysis. */
  preset: MatchPreset | null;
  /** Loads a build into the builder, with the abilities imbued items go into, for a hero when it is not the builder's. */
  onUse: (purchases: string[], what: string, imbues?: Record<string, number>, hero?: string) => void;
  /** Opens the combo with the player's build at some souls against an enemy's usual build at the same souls. */
  onFight: (fight: Fight) => void;
}

export interface Fight {
  hero: string;
  purchases: string[];
  imbues: Record<string, number>;
  souls: number;
  tiers: number[];
  enemy: Build;
  what: string;
}

/** Ability upgrade tiers the master builder assumes in each phase. */
const PHASE_TIERS = [
  [0, 0, 0, 0],
  [1, 1, 1, 0],
  [2, 2, 1, 1],
  [3, 3, 3, 3],
];

const minute = (m: number) => `${Math.floor(m)}:${String(Math.round((m % 1) * 60)).padStart(2, "0")}`;

/** A small item tile with its numbers underneath. */
function Tile({ item, picked, onClick }: { item: PopularItem; picked?: boolean; onClick?: () => void }) {
  return (
    <button
      className={"gen-tile " + item.category + (picked ? " picked" : "")}
      onClick={onClick}
      title={`${item.name} · ${t("{n} souls", { n: fmt(item.cost) })}\n${t("Bought in {a}% of matches, kept to the end in {b}%, won {c}%, bought at {m} on average", { a: fmt(item.bought_pct, 0), b: fmt(item.kept_pct, 0), c: fmt(item.win_pct, 1), m: minute(item.buy_minute) })}${item.description ? "\n\n" + item.description : ""}`}
    >
      <img src={item.icon} alt="" loading="lazy" />
      {item.active && <em>{t("Active")}</em>}
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
  const [attempt, setAttempt] = useState(0);
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
  }, [hero.id, attempt]);

  if (busy) return <p className="muted">{t("Reading what players build with {hero}…", { hero: hero.name })}</p>;
  if (error) return <ErrorNote error={error} onRetry={() => setAttempt((a) => a + 1)} />;
  if (!report) return null;
  const b = report.build;
  const picked = b.situational.find((s) => s.item === counter);
  return (
    <>
      <p className="muted">
        {t(
          "From {n} {hero} matches of the last {days} days ({rank} lobbies), read from the community Deadlock API. The core is the 8 items most players buy and keep to the end, in the order they are bought; under each: kept in % of matches · bought at.",
          { n: fmt(b.matches), hero: hero.name, days: report.days, rank: report.rank },
        )}
      </p>
      <div className="group">
        <h3>{t("Core build")}</h3>
        <div className="gen-tiles">
          {b.core.map((item) => (
            <Tile key={item.item} item={item} />
          ))}
        </div>
      </div>
      <div className="group">
        <h3>{t("Counter slot")}</h3>
        <p className="muted">
          {t(
            "Bought in a fair share of matches but not most: the item players change depending on the enemy. Pick one to add it to the build; the Master builder tab works out which ones a given match calls for.",
          )}
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
            picked ? t("Loaded the popular {hero} build with {item}", { hero: hero.name, item: picked.name }) : t("Loaded the popular {hero} build", { hero: hero.name }),
          )
        }
      >
        {picked ? t("Use this build with {item}", { item: picked.name }) : t("Use this build")}
      </button>
    </>
  );
}

/** The roles and damage the master builder can build for; translated where shown. */
export const FOCUSES: [string, string, string][] = [
  ["auto", "Auto", "Picked from the teams: tempo when a teammate carries late, carry otherwise"],
  ["carry", "Carry", "Your own damage first: bigger items that scale into the late game"],
  ["tempo", "Tempo", "Power early in the mid game, to fight before their carries come online"],
  ["support", "Support", "Items that answer their team and help yours"],
  ["frontline", "Frontline", "Staying alive at the front while your carries deal the damage"],
];
/** The damage kinds; translated where shown. */
export const DAMAGES: [string, string][] = [
  ["auto", "Auto"],
  ["gun", "Gun"],
  ["spirit", "Spirit"],
  ["hybrid", "Hybrid"],
  ["melee", "Melee"],
];

/** How often the console log is read while watching, how long a failed team fetch waits, and how many tries a
 * match gets. */
const LIVE_CHECK_MS = 20_000;
const LIVE_RETRY_MS = 120_000;
const LIVE_TRIES = 3;

/** Both teams, the lane and the role: a full build for the player's hero, phase by phase. */
/** A small item tile for the comparison. */
function MiniItem({ item, cost }: { item: { name: string; icon: string; category: string; description: string }; cost: number }) {
  return (
    <span className={"mini-item " + item.category} title={`${item.name} · ${t("{n} souls", { n: fmt(cost) })}\n\n${item.description}`}>
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
    j.popular_pct >= 0.5 ? t("{n}% of top players buy it at this point", { n: fmt(j.popular_pct, 0) }) : t("Top players rarely buy it at this point"),
    ...(Math.abs(j.win_pp) >= 1
      ? [
          t(j.win_pp > 0 ? "top players who buy it here win {n} points more often than with other items bought here" : "top players who buy it here win {n} points less often than with other items bought here", {
            n: fmt(Math.abs(j.win_pp), 0),
          }),
        ]
      : []),
    ...j.notes.filter((n) => startsLike(n, "Your team has it") || startsLike(n, "Works within") || startsLike(n, "Harder")),
  ];
}

function whyIn(x: MasterItem, style: string): string[] {
  return [
    ...(x.popular_pct >= 0.5 ? [t("{n}% of top players buy it here", { n: fmt(x.popular_pct, 0) })] : []),
    ...(Math.abs(x.win_pp) >= 1
      ? [
          t(x.win_pp > 0 ? "they win {n} points more often with it than with other items bought here" : "they win {n} points less often with it than with other items bought here", {
            n: fmt(Math.abs(x.win_pp), 0),
          }),
        ]
      : []),
    ...(x.engine_pct >= 1 ? [t("+{n}% to your {style} damage", { n: fmt(x.engine_pct, 0), style: t(style) })] : []),
    ...x.jobs.map((j) => j.toLocaleLowerCase(LOCALE)),
  ];
}

/** The player's purchases in the match set against the master build, phase by phase: what to buy instead of what. */
function MatchComparison({ master, preset }: { master: MasterBuild; preset: MatchPreset }) {
  const bought = new Set(master.judged.map((j) => j.item));
  return (
    <div className="group compare-match">
      <h3>
        {t("Match {id}: {player}'s build against the suggested one", { id: preset.match_id, player: preset.player_name })}{" "}
        <small className="muted">
          · {t("ended at {n} souls", { n: fmt(preset.net_worth) })} · {preset.won ? t("won") : t("lost")}
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
              <small>{t("{from}–{to} souls", { from: fmt(phase.souls_from), to: fmt(phase.souls_to) })}</small>
              {!reached && <small className="muted">{t("not reached in this match")}</small>}
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
                    {sw.in.name}: {whyIn(sw.in, master.style).join("; ") || t("fits the build better")}.
                  </span>
                </span>
              </div>
            ))}
            {add.length > 0 && (
              <div className="compare-line">
                <small>{reached ? t("Also buy") : t("Next")}:</small>
                {add.map((x) => (
                  <MiniItem key={x.item} item={x} cost={x.cost} />
                ))}
              </div>
            )}
            {left.length > 0 && (
              <div className="compare-line">
                <small>{t("Could skip:")}</small>
                {left.map((j) => (
                  <span key={j.item} title={whyOut(j).join("\n")}>
                    <MiniItem item={j} cost={j.cost} />
                  </span>
                ))}
              </div>
            )}
            {kept.length > 0 && (
              <div className="compare-line good">
                <small>{t("Good picks:")}</small>
                {kept.map((j) => (
                  <MiniItem key={j.item} item={j} cost={j.cost} />
                ))}
              </div>
            )}
            {mine.length === 0 && suggested.length === 0 && <small className="muted">{t("Nothing in this phase.")}</small>}
          </div>
        );
      })}
    </div>
  );
}

function MasterPanel({
  heroes,
  hero,
  preset,
  onUse,
  onFight,
}: {
  heroes: Hero[];
  hero: Hero;
  preset: MatchPreset | null;
  onUse: Props["onUse"];
  onFight: Props["onFight"];
}) {
  const [fighting, setFighting] = useState("");
  // The enemy's usual build at some souls: its core items in the order they are bought, while the souls last.
  const fight = async (master: MasterBuild, phase: number, enemyId: number) => {
    const enemy = heroes.find((h) => h.id === enemyId);
    const p = master.phases[phase];
    if (!enemy || !p) return;
    setFighting(`${phase}-${enemyId}`);
    try {
      const report = await api.popularBuild(enemyId);
      let spent = 0;
      const purchases = report.build.core.filter((c) => (spent += c.cost) <= p.souls_to).map((c) => c.item);
      const tiers = PHASE_TIERS[phase] ?? [0, 0, 0, 0];
      const mine = master.items.filter((x) => x.phase <= phase && !x.sold).map((x) => x.item);
      onFight({
        hero: master.hero,
        purchases: mine,
        imbues: Object.fromEntries(Object.entries(master.imbues).filter(([k]) => mine.includes(k))),
        souls: p.souls_to,
        tiers,
        enemy: {
          hero: enemy.class_name,
          purchases,
          upto: null,
          souls: p.souls_to,
          active: [],
          ability_tiers: tiers,
          imbues: {},
          copied_ult: null,
          target: { health: 2000, bullet_resist: 0, spirit_resist: 0 },
          map: EMPTY_MAP,
        },
        what: t("Fighting {hero} at the end of {phase} ({n} souls each)", { hero: enemy.name, phase: p.title, n: fmt(p.souls_to) }),
      });
    } finally {
      setFighting("");
    }
  };
  // The defaults come from Settings; a change here holds for this screen only.
  const [focus, setFocus] = useState(() => readSetting(SETTINGS.masterFocus));
  const [damage, setDamage] = useState(() => readSetting(SETTINGS.masterDamage));
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
  // The player's match being played now, from the game's watch list, with the account set on the Draft or Match screen.
  const [liveNote, setLiveNote] = useState("");
  // The match the teams were last filled from, so a watched match is built once.
  const [liveId, setLiveId] = useState<number | null>(null);
  const [lastLive, setLastLive] = useState<LiveMatch | null>(null);
  const applyLive = (live: LiveMatch) => {
    const cls = heroes.find((h) => h.id === live.hero)?.class_name;
    if (!cls) throw new Error(t("Your hero is not in the game data: use Update data"));
    setLiveId(live.match_id);
    setLastLive(live);
    setMine(cls);
    setAllies(live.allies);
    setEnemies(live.enemies);
    setLane(live.lane);
    setLiveNote(t(live.lane.length ? "Match {id}: both teams loaded, with your lane." : "Match {id}: both teams loaded.", { id: live.match_id }));
    run(cls, live.allies, live.enemies, live.lane);
  };
  const [liveBusy, setLiveBusy] = useState(false);
  const loadLive = () => {
    setLiveBusy(true);
    setLiveNote(t("Looking for your match… the teams take about a minute the first time."));
    findLiveMatch()
      .then(applyLive)
      .catch((e) => setLiveNote(String(e instanceof Error ? e.message : e)))
      .finally(() => setLiveBusy(false));
  };
  // A match number typed by hand, for when the console log is not there.
  const [typedMatch, setTypedMatch] = useState("");
  const loadTyped = () => {
    const id = Number(typedMatch.trim());
    const account = savedAccount();
    if (!Number.isInteger(id) || id <= 0) return setLiveNote(t("Enter the match number (numbers only)."));
    if (!account) return setLiveNote(t("Set your Steam account ID in Settings first."));
    setLiveBusy(true);
    setLiveNote(t("Getting the teams of match {id}… about a minute.", { id }));
    api
      .matchTeams(id, account)
      .then(applyLive)
      .catch((e) => setLiveNote(String(e instanceof Error ? e.message : e)))
      .finally(() => setLiveBusy(false));
  };
  // Watching: while this screen is open, the game's console log is read every 20 seconds; a new match in it has
  // its teams fetched (about a minute), and is built for. A failed fetch is tried again after two minutes, at most
  // three times a match: the spectating service takes only a few matches an hour from one computer.
  const [watchSetting, setWatchSetting] = useSetting(SETTINGS.watchLive);
  const watching = watchSetting === "1";
  const toggleWatch = (on: boolean) => setWatchSetting(on ? "1" : "0");
  const tries = useRef<{ match: number; count: number; next: number }>({ match: 0, count: 0, next: 0 });
  const pending = useRef(false);
  useEffect(() => {
    if (!watching) return;
    let stopped = false;
    const look = async () => {
      if (pending.current) return;
      const time = new Date().toLocaleTimeString(LOCALE, { hour: "2-digit", minute: "2-digit" });
      try {
        const c = await api.consoleMatch();
        if (stopped) return;
        if (!c.found) return setLiveNote(`${t("Watching: the game's console log was not found.")} ${CONDEBUG_HELP}`);
        if (!c.match_id) return setLiveNote(t("Watching the game's console log: no match in progress (checked {time}).", { time }));
        if (c.match_id === liveId) return;
        const tried = tries.current;
        if (tried.match !== c.match_id) tries.current = { match: c.match_id, count: 0, next: 0 };
        if (tries.current.count >= LIVE_TRIES || Date.now() < tries.current.next) return;
        const account = c.account_id ? String(c.account_id) : savedAccount();
        if (!account) return setLiveNote(t("Watching: the console log names no account. Set your Steam account ID in Settings."));
        pending.current = true;
        tries.current.count += 1;
        setLiveNote(t("Match {id} started: getting the teams… about a minute.", { id: c.match_id }));
        try {
          const live = await api.matchTeams(c.match_id, account);
          if (!stopped) applyLive(live);
        } catch (e) {
          tries.current.next = Date.now() + LIVE_RETRY_MS;
          if (!stopped) {
            const left = LIVE_TRIES - tries.current.count;
            setLiveNote(`${t("Match {id}:", { id: c.match_id })} ${String(e instanceof Error ? e.message : e)} ${left > 0 ? t("Trying again in two minutes.") : t("Use the button to try once more.")}`);
          }
        } finally {
          pending.current = false;
        }
      } catch (e) {
        if (!stopped) setLiveNote(String(e instanceof Error ? e.message : e));
      }
    };
    look();
    const timer = setInterval(look, LIVE_CHECK_MS);
    return () => {
      stopped = true;
      clearInterval(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [watching, liveId]);
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
        {t(
          "Click a hero for the enemy team, right-click one for your team, mark the enemies in your lane and pick your role. Everyone else is taken to build what top players build. Your build is put together phase by phase by souls (lane to 6k, first rift to 15k, second rift to 25k, all boons to 40k), weighing for every item how many top players of your hero buy it then, what it adds to your own numbers, and what it answers on their team, less what your teammates already bring.",
        )}
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
            title={h.id === me.id ? t("You play {hero}", { hero: h.name }) : t("{hero}: click for the enemy team, right-click for your team", { hero: h.name })}
          >
            <img src={h.icon} alt="" loading="lazy" />
            <span>{h.name}</span>
          </button>
        ))}
      </div>
      <div className="teams">
        <div className="team-row">
          <span className="team-label">{t("Your team")}</span>
          <span className="team-chip you" title={t("You")}>
            <img src={me.icon} alt="" />
            {me.name}
            <small>{t("you")}</small>
          </span>
          {allies.map((id) => {
            const h = byId(id);
            return (
              h && (
                <button key={id} className="team-chip" onClick={() => playAs(h)} title={t("Play {hero} instead", { hero: h.name })}>
                  <img src={h.icon} alt="" />
                  {h.name}
                </button>
              )
            );
          })}
          {allies.length === 0 && <small className="muted">{t("right-click heroes to add your teammates")}</small>}
        </div>
        <div className="team-row">
          <span className="team-label">{t("Enemy team")}</span>
          {enemies.map((id) => {
            const h = byId(id);
            return (
              h && (
                <button
                  key={id}
                  className={"team-chip enemy" + (lane.includes(id) ? " lane" : "")}
                  onClick={() => toggleLane(id)}
                  title={lane.includes(id) ? t("{hero} is in your lane", { hero: h.name }) : t("Mark {hero} as in your lane", { hero: h.name })}
                >
                  <img src={h.icon} alt="" />
                  {h.name}
                  {lane.includes(id) && <small>{t("lane")}</small>}
                </button>
              )
            );
          })}
          {enemies.length === 0 ? (
            <small className="muted">{t("click heroes to add the enemy team")}</small>
          ) : (
            lane.length === 0 && <small className="muted">{t("click the enemies in your lane")}</small>
          )}
        </div>
        <div className="team-row">
          <span className="team-label">{t("Your role")}</span>
          {FOCUSES.map(([key, name, goal]) => (
            <button key={key} className={"choice" + (focus === key ? " on" : "")} title={t(goal)} onClick={() => setFocus(key)}>
              {t(name)}
            </button>
          ))}
        </div>
        <div className="team-row">
          <span className="team-label">{t("Damage")}</span>
          {DAMAGES.map(([key, name]) => (
            <button key={key} className={"choice" + (damage === key ? " on" : "")} onClick={() => setDamage(key)}>
              {t(name)}
            </button>
          ))}
        </div>
        <div className="gen-actions">
          <button disabled={busy} onClick={find}>
            {busy ? t("Working…") : t("Build for {hero}", { hero: me.name })}
          </button>
          <button className="ghost" disabled={busy || liveBusy} onClick={loadLive} title={t("Fills both teams and your lane from the match you are playing now, read from the game's console log (-condebug)")}>
            {t("Load my live match")}
          </button>
          <label className="check" title={t("While this screen is open, reads the game's console log every 20 seconds and builds for a new match as soon as its teams are in")}>
            <input type="checkbox" checked={watching} onChange={(e) => toggleWatch(e.target.checked)} />
            {t("Watch for my match")}
          </label>
          <input
            className="match-number"
            placeholder={t("or match number")}
            value={typedMatch}
            onChange={(e) => setTypedMatch(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && loadTyped()}
            title={t("The match number, when the console log is not there")}
          />
          {liveNote && <span className="muted">{liveNote}</span>}
        </div>
      </div>
      <ErrorNote error={error} onRetry={find} />
      {lastLive && <LobbyScout live={lastLive} heroes={heroes} />}

      {report && (
        <>
          {preset && report.master.hero === preset.hero && report.master.judged.length > 0 && <MatchComparison master={report.master} preset={preset} />}
          <div className="group master">
            <h3>
              {t("{hero} as {role}", { hero: report.master.hero_name, role: report.master.focus_name })}{" "}
              <small className="muted">
                · {report.master.focus_goal} · {t("{style} damage", { style: t(report.master.style) })}
              </small>
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
                  <small>{t("{from}–{to} souls", { from: fmt(phase.souls_from), to: fmt(phase.souls_to) })}</small>
                  {phase.sold.length > 0 && <small className="muted">{t("sells {items}", { items: phase.sold.join(", ") })}</small>}
                  {enemies.length > 0 && (
                    <span className="fight-row" title={t("Open the combo with this build against the enemy's usual build at the same souls")}>
                      <small className="muted">{t("fight")}</small>
                      {[...lane, ...enemies.filter((id) => !lane.includes(id))].map((id) => {
                        const h = byId(id);
                        return (
                          h && (
                            <button
                              key={id}
                              className="fight-btn"
                              disabled={fighting !== ""}
                              title={t("Combo against {hero}'s usual build at {n} souls", { hero: h.name, n: fmt(phase.souls_to) })}
                              onClick={() => fight(report.master, i, id)}
                            >
                              <img src={h.icon} alt={h.name} />
                            </button>
                          )
                        );
                      })}
                    </span>
                  )}
                </div>
                <div className="master-items">
                  {report.master.items
                    .filter((x) => x.phase === i)
                    .map((x) => (
                      <div
                        key={x.item}
                        className={"master-item " + x.category + (x.sold || x.upgraded ? " sold" : "") + (x.counter ? " counter" : "")}
                        title={`${x.name} · ${t("{n} souls", { n: fmt(x.cost) })}${x.paid < x.cost ? ` ${t("({n} as an upgrade)", { n: fmt(x.paid) })}` : ""}\n\n${x.description}${x.notes.length ? "\n\n" + x.notes.join("\n") : ""}`}
                      >
                        <img src={x.icon} alt="" loading="lazy" />
                        <div>
                          <b>
                            {x.name}
                            {x.counter && (
                              <em className="counter-tag" title={t("Bought to answer the enemy team")}>
                                {t("Counter")}
                              </em>
                            )}
                          </b>
                          <small>
                            {fmt(x.cost)}
                            {x.upgrades ? ` · ${t("{n} on top of {item}", { n: fmt(x.paid), item: x.upgrades })}` : ""}
                            {x.imbued ? ` · ${t("on {ability}", { ability: x.imbued })}` : ""}
                            {x.active ? ` · ${t("active")}` : ""}
                            {x.sold ? ` · ${t("sold later")}` : ""}
                            {x.upgraded ? ` · ${t("upgraded later")}` : ""}
                          </small>
                          <span className="master-why">
                            {x.popular_pct >= 0.5 && <i className="why-pop">{t("{n}% of top players", { n: fmt(x.popular_pct, 0) })}</i>}
                            {Math.abs(x.win_pp) >= 1 && (
                              <i className={x.win_pp > 0 ? "why-win" : "why-lose"} title={t("Top players win {n}% of the matches in which they buy it at this point", { n: fmt(x.win_pct, 0) })}>
                                {t("{sign}{n} pts wins", { sign: x.win_pp > 0 ? "+" : "−", n: fmt(Math.abs(x.win_pp), 0) })}
                              </i>
                            )}
                            {x.engine_pct >= 1 && <i className="why-engine">{t("+{n}% {style}", { n: fmt(x.engine_pct, 0), style: t(report.master.style) })}</i>}
                            {x.notes.some((n) => startsLike(n, "Your team has it")) && <i className="why-team">{t("team has it")}</i>}
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
                    <summary>{t("Why not something else? The best items not bought here")}</summary>
                    <table className="match-table compare">
                      <thead>
                        <tr>
                          <th className="left">{t("Item")}</th>
                          <th title={t("The item's score: popularity, own numbers and the answer to their team, weighed by the role")}>{t("Score")}</th>
                          <th title={t("Share of your hero's top players who buy it at this point")}>{t("Top players")}</th>
                          <th title={t("Win rate when bought at this point, against everything bought at this point (points, shrunk for small samples)")}>{t("Wins")}</th>
                          <th title={t("What it adds to your own damage in a team fight: an ability that hits an area counts for every enemy it hits, so range and radius items count")}>{t("Own numbers")}</th>
                          <th title={t("Its answer to the enemy team, against the best answer at this point")}>{t("Answer")}</th>
                          <th className="left">{t("Why")}</th>
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
                              <span className={"verdict" + (c.left_out ? " hard" : "")}>{c.verdict}.</span>
                              {c.jobs.length > 0 && <small>{t("Answers: {jobs}", { jobs: c.jobs.map((j) => j.toLocaleLowerCase(LOCALE)).join(", ") })}</small>}
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
                onUse(
                  report.master.purchases,
                  t("Loaded the master build for {hero} as {role}", { hero: report.master.hero_name, role: report.master.focus_name }),
                  report.master.imbues,
                  report.master.hero,
                )
              }
            >
              {t("Use this build")}
            </button>
          </div>

          <PowerCurve master={report.master} />


          <div className="group">
            <h3>{t("Hero by hero")}</h3>
            {report.plan.team.map((line) => (
              <p key={line} className="muted team-note">
                {line}.
              </p>
            ))}
            <table className="match-table compare hero-answers">
              <thead>
                <tr>
                  <th className="left">{t("Enemy")}</th>
                  <th className="left">{t("What they bring")}</th>
                  <th className="left">{t("Answered by")}</th>
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
                            {h.in_lane && <small className="lane-tag">{t("in your lane")}</small>}
                          </span>
                        </span>
                      </td>
                      <td className="left">
                        <ul className="plain">
                          {h.threats.map((threat) => (
                            <li key={threat}>{threat}</li>
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
                          <span className="muted">{t("Nothing above; see the other counter items")}</span>
                        )}
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>


          <details className="counter-details">
            <summary>{t("The enemy team and every option, job by job")}</summary>
          <div className="group">
            <h3>{t("The enemy team")}</h3>
            <p className="muted">
              {t("Read from their kits and from what they build in {rank} lobbies over the last {days} days.", { rank: report.rank, days: report.days })}
              {report.plan.own_escape.length > 0 &&
                ` ${t("{hero} can get out of trouble with {abilities}, so answers to crowd control count for less.", { hero: me.name, abilities: report.plan.own_escape.join(", ") })}`}
            </p>
            <table className="match-table compare enemy-table">
              <thead>
                <tr>
                  <th className="left">{t("Hero")}</th>
                  <th className="left">{t("Usual build")}</th>
                  <th title={t("Share of its damage in a fight that is gun and spirit damage, worked out by the calculator with its usual build late in the match")}>{t("Gun / spirit")}</th>
                  <th className="left">{t("Heals with")}</th>
                  <th className="left">{t("Moves with")}</th>
                  <th className="left">{t("Locks down with")}</th>
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
                            <img key={c.item} src={c.icon} alt={c.name} title={`${c.name} · ${t("kept in {n}%", { n: fmt(c.kept_pct, 0) })} · ${t("bought at {m}", { m: minute(c.buy_minute) })}`} />
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
                        {a.active && <small className="tag-active">{t("active")}</small>}
                      </div>
                      <div className="counter-effect">{a.effect}</div>
                      <p className="counter-what">{a.description || t("No description in the game data.")}</p>
                      {a.reasons
                        .filter((r) => startsLike(r, "Top players"))
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
export default function GeneratorView({ heroes, hero, build, souls, preset, onUse, onFight }: Props) {
  const [tab, setTab] = useState<"calculated" | "popular" | "master">(preset ? "master" : "calculated");
  useEffect(() => {
    if (preset) setTab("master");
  }, [preset?.key]);
  return (
    <div className="generator-view">
      <div className="side-tabs gen-tabs">
        <button className={tab === "calculated" ? "on" : ""} onClick={() => setTab("calculated")}>
          {t("Calculated")}
        </button>
        <button className={tab === "popular" ? "on" : ""} onClick={() => setTab("popular")}>
          {t("Popular")}
        </button>
        <button className={tab === "master" ? "on" : ""} onClick={() => setTab("master")}>
          {t("Master builder")}
        </button>
      </div>
      <div className="gen-body">
        {tab === "calculated" && <SuggestPanel hero={hero} build={build} souls={souls} onUse={(p) => onUse(p, t("Loaded a build the calculator put together"))} />}
        {tab === "popular" && <PopularPanel hero={hero} onUse={onUse} />}
        {tab === "master" && <MasterPanel heroes={heroes} hero={hero} preset={preset} onUse={onUse} onFight={onFight} />}
      </div>
    </div>
  );
}
