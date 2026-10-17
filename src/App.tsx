import { useCallback, useEffect, useMemo, useState } from "react";
import { api, Build, Combo, ComboResult, EMPTY_MAP, EMPTY_ZONE, fmt, GameData, Ranked, Sheet, MatchPreset } from "./api";
import BuildBar from "./components/BuildBar";
import ComboPanel from "./components/ComboPanel";
import GeneratorView from "./components/GeneratorView";
import MiscPanel from "./components/MiscPanel";
import HeroPicker from "./components/HeroPicker";
import MatchView from "./components/MatchView";
import Shop from "./components/Shop";
import StatsPanel from "./components/StatsPanel";
import UpdateBanner from "./components/UpdateBanner";

const STORAGE_KEY = "current-build";
const COMBO_KEY = "current-combo";

function restoreCombo(): Combo {
  const empty: Combo = { actions: [], headshot_rate: 0, own_health: 50, enemy: null, zone: EMPTY_ZONE };
  try {
    const raw = localStorage.getItem(COMBO_KEY);
    if (!raw) return empty;
    const saved = JSON.parse(raw);
    return { ...empty, ...saved, zone: { ...EMPTY_ZONE, ...saved.zone } };
  } catch {
    return empty;
  }
}

const emptyBuild = (hero: string): Build => ({
  hero,
  purchases: [],
  upto: null,
  souls: null,
  active: [],
  ability_tiers: [0, 0, 0, 0],
  imbues: {},
  map: EMPTY_MAP,
  copied_ult: null,
  target: { health: 2000, bullet_resist: 0, spirit_resist: 0 },
});

function restoreBuild(): Build | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const saved = JSON.parse(raw);
    return { ...emptyBuild(""), ...saved, map: { ...EMPTY_MAP, ...saved.map, statues: { ...EMPTY_MAP.statues, ...saved.map?.statues } } };
  } catch {
    return null;
  }
}

export default function App() {
  const [data, setData] = useState<GameData | null>(null);
  const [build, setBuild] = useState<Build | null>(null);
  const [sheet, setSheet] = useState<Sheet | null>(null);
  const [ranked, setRanked] = useState<Ranked[]>([]);
  const [metric, setMetric] = useState("dps_sustained");
  const [conditional, setConditional] = useState(false);
  const [picking, setPicking] = useState(false);
  const [saved, setSaved] = useState<string[]>([]);
  const [name, setName] = useState("");
  const [status, setStatus] = useState("");
  const [tab, setTab] = useState<"stats" | "combo" | "misc">("stats");
  const [mode, setMode] = useState<"build" | "generator" | "match">("build");
  /** A match player handed to the master builder. */
  const [masterPreset, setMasterPreset] = useState<MatchPreset | null>(null);
  const [navOpen, setNavOpen] = useState(() => {
    try {
      return localStorage.getItem("nav-open") !== "0";
    } catch {
      return true;
    }
  });
  const toggleNav = () =>
    setNavOpen((open) => {
      try {
        localStorage.setItem("nav-open", open ? "0" : "1");
      } catch {
        /* a convenience only */
      }
      return !open;
    });
  const [combo, setCombo] = useState<Combo>(restoreCombo);
  const [comboResult, setComboResult] = useState<ComboResult | null>(null);

  useEffect(() => {
    api
      .getData()
      .then((d) => {
        setData(d);
        const restored = restoreBuild();
        const valid = restored && d.heroes.some((h) => h.class_name === restored.hero);
        setBuild(valid ? restored : emptyBuild(d.heroes[0].class_name));
      })
      .catch((e) => setStatus(String(e)));
    api.listBuilds().then(setSaved).catch(() => {});
  }, []);

  useEffect(() => {
    if (!build) return;
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(build));
    } catch {
      // without persistent storage the build only lives for this session
    }
    let stale = false;
    api
      .compute(build)
      .then((s) => !stale && setSheet(s))
      .catch((e) => !stale && setStatus(String(e)));
    api
      .rankNext(build, metric, conditional)
      .then((r) => !stale && setRanked(r))
      .catch(() => !stale && setRanked([]));
    return () => {
      stale = true;
    };
  }, [build, metric, conditional, data]);

  useEffect(() => {
    if (!build) return;
    try {
      localStorage.setItem(COMBO_KEY, JSON.stringify(combo));
    } catch {
      // without persistent storage the combo only lives for this session
    }
    let stale = false;
    api
      .simulateCombo(build, combo)
      .then((r) => !stale && setComboResult(r))
      .catch((e) => !stale && setStatus(String(e)));
    return () => {
      stale = true;
    };
  }, [build, combo, data]);

  const patch = useCallback((p: Partial<Build>) => setBuild((b) => (b ? { ...b, ...p } : b)), []);
  const items = useMemo(() => new Map(data?.items.map((i) => [i.class_name, i]) ?? []), [data]);
  const owned = useMemo(() => new Set(sheet?.owned ?? []), [sheet]);
  const purchased = useMemo(() => new Set(build?.purchases ?? []), [build]);

  if (!data || !build) return <div className="loading">{status || "Loading…"}</div>;
  const hero = data.heroes.find((h) => h.class_name === build.hero) ?? data.heroes[0];

  const toggleItem = (className: string) => {
    if (build.purchases.includes(className)) {
      patch({
        purchases: build.purchases.filter((p) => p !== className),
        active: build.active.filter((a) => a !== className),
        upto: null,
      });
    } else {
      patch({ purchases: [...build.purchases, className], upto: null });
    }
  };

  const run = async (label: string, action: () => Promise<void>) => {
    try {
      await action();
      setStatus(label);
    } catch (e) {
      setStatus(String(e));
    }
  };
  const save = () =>
    run(`Saved "${name}"`, async () => {
      await api.saveBuild(name, build);
      setSaved(await api.listBuilds());
    });
  const load = (n: string) =>
    run(`Loaded "${n}"`, async () => {
      setBuild({ ...emptyBuild(hero.class_name), ...(await api.loadBuild(n)) });
      setName(n);
    });
  const remove = () =>
    run(`Deleted "${name}"`, async () => {
      await api.deleteBuild(name);
      setSaved(await api.listBuilds());
    });
  const refresh = () => {
    setStatus("Downloading data…");
    run("Data updated", async () => setData(await api.refreshData()));
  };

  const maxSouls = hero.levels[hero.levels.length - 1]?.required_souls ?? 50000;
  const souls = Math.max(build.souls ?? 0, sheet?.spent ?? 0);

  return (
    <div className={"app" + (navOpen ? " nav-open" : "")}>
      <nav className="sidebar">
        <button className="nav-toggle" onClick={toggleNav} title={navOpen ? "Collapse" : "Expand"}>
          <svg viewBox="0 0 24 24" width="20" height="20">
            <path d="M4 6h16M4 12h16M4 18h16" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          </svg>
          <b>Deadlock BCMA</b>
        </button>
        {(
          [
            ["build", "Build maker", "M14.7 6.3a4 4 0 0 0-5.4 5.4L3 18l3 3 6.3-6.3a4 4 0 0 0 5.4-5.4l-2.5 2.5-2.5-.5-.5-2.5z"],
            ["generator", "Build generator", "M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9zM19 15l.8 2.2L22 18l-2.2.8L19 21l-.8-2.2L16 18l2.2-.8z"],
            ["match", "Match analysis", "M4 20V10M10 20V4M16 20v-7M22 20H2"],
          ] as const
        ).map(([key, label, icon]) => (
          <button key={key} className={"nav-item" + (mode === key ? " on" : "")} onClick={() => setMode(key)} title={label}>
            <svg viewBox="0 0 24 24" width="20" height="20">
              <path d={icon} fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinejoin="round" strokeLinecap="round" />
            </svg>
            <span>{label}</span>
          </button>
        ))}
        <div className="nav-foot">
          <UpdateBanner />
          <small>
            Game {data.client_version ?? "?"} · {new Date(data.fetched_at * 1000).toLocaleDateString("en-US")}
          </small>
          <button className="ghost" onClick={refresh}>
            Update data
          </button>
        </div>
      </nav>
      <div className="page">
      <header className={mode}>
        <button className="hero-button" onClick={() => setPicking(true)}>
          <img src={hero.icon} alt="" />
          <span>
            <strong>{hero.name}</strong>
            <small>Change hero</small>
          </span>
        </button>
        <label className="souls">
          Total souls
          <input
            type="range"
            min={0}
            max={maxSouls}
            step={100}
            value={souls}
            onChange={(e) => patch({ souls: Number(e.target.value) })}
          />
          <input
            type="number"
            min={0}
            step={100}
            value={souls}
            onChange={(e) => patch({ souls: Math.max(0, Math.round(Number(e.target.value) || 0)) })}
          />
          {sheet && (
            <>
              <b>Level</b>
              <input
                type="number"
                className="level-input"
                title="Sets total souls to the amount needed for this level"
                min={1}
                max={hero.levels.length}
                value={sheet.level}
                onChange={(e) => {
                  const level = Math.min(hero.levels.length, Math.max(1, Math.round(Number(e.target.value) || 1)));
                  patch({ souls: hero.levels[level - 1]?.required_souls ?? 0 });
                }}
              />
            </>
          )}
        </label>
        <div className="saves" style={mode === "build" ? undefined : { display: "none" }}>
          <input placeholder="Build name" value={name} onChange={(e) => setName(e.target.value)} />
          <button onClick={save} disabled={!name.trim()}>
            Save
          </button>
          <select value="" onChange={(e) => e.target.value && load(e.target.value)}>
            <option value="">Load…</option>
            {saved.map((s) => (
              <option key={s}>{s}</option>
            ))}
          </select>
          <button className="ghost" onClick={remove} disabled={!saved.includes(name)}>
            Delete
          </button>
        </div>
        <div className="data-info">{status && <small className="status">{status}</small>}</div>
      </header>

      {/* Kept mounted while the builder is shown so a loaded match stays loaded. */}
      <div style={{ display: mode === "match" ? "contents" : "none" }}>
        <MatchView
          onOpenBuild={(b, target) => {
            // The builder keeps its own target unless the build is opened against an enemy of the match.
            setBuild({ ...emptyBuild(b.hero), ...b, target: target ?? build.target });
            setMode("build");
            setStatus(target ? "Opened a build from the match, with that enemy as the target" : "Opened a build from the match");
          }}
          onOpenMaster={(preset) => {
            setMasterPreset(preset);
            setMode("generator");
          }}
        />
      </div>

      {mode === "generator" && sheet && (
        <GeneratorView
          heroes={data.heroes}
          hero={hero}
          build={build}
          souls={sheet.souls}
          preset={masterPreset}
          onUse={(purchases, what, imbues, heroClass) => {
            // A build for another hero (the master builder's) switches the builder to that hero.
            if (heroClass && heroClass !== build.hero) {
              setBuild({ ...emptyBuild(heroClass), target: build.target, purchases, active: purchases, imbues: imbues ?? {} });
            } else {
              patch({ purchases, active: purchases, upto: null, imbues: imbues ?? {} });
            }
            setMode("build");
            setStatus(what);
          }}
          onFight={(f) => {
            setBuild({ ...emptyBuild(f.hero), target: build.target, map: build.map, purchases: f.purchases, active: f.purchases, imbues: f.imbues, souls: f.souls, ability_tiers: f.tiers });
            setCombo((c) => ({ ...c, enemy: f.enemy }));
            setMode("build");
            setTab("combo");
            setStatus(f.what);
          }}
        />
      )}

      {mode === "build" && <BuildBar build={build} sheet={sheet} items={items} onChange={patch} />}

      <main style={{ display: mode === "build" ? undefined : "none" }}>
        <Shop items={data.items} owned={owned} purchased={purchased} onToggle={toggleItem} />
        {sheet && (
          <div className="side">
            <div className="side-tabs">
              <button className={tab === "stats" ? "on" : ""} onClick={() => setTab("stats")}>
                Stats
              </button>
              <button className={tab === "combo" ? "on" : ""} onClick={() => setTab("combo")}>
                Combo
              </button>
              <button className={tab === "misc" ? "on" : ""} onClick={() => setTab("misc")}>
                Misc
              </button>
            </div>
            {tab === "stats" ? (
              <StatsPanel
                heroes={data.heroes}
                hero={hero}
                build={build}
                sheet={sheet}
                ranked={ranked}
                metric={metric}
                conditional={conditional}
                onMetric={setMetric}
                onConditional={setConditional}
                onChange={patch}
                onBuy={toggleItem}
              />
            ) : tab === "combo" ? (
              <ComboPanel heroes={data.heroes} items={data.items} sheet={sheet} combo={combo} result={comboResult} onChange={setCombo} map={build.map} onMap={(map) => patch({ map })} />
            ) : (
              <MiscPanel sheet={sheet} />
            )}
          </div>
        )}
      </main>

      {picking && (
        <HeroPicker
          heroes={data.heroes}
          selected={build.hero}
          onClose={() => setPicking(false)}
          onPick={(h) => {
            patch({ hero: h, ability_tiers: [0, 0, 0, 0] });
            setPicking(false);
          }}
        />
      )}
      <footer>
        Click a stat to see its formula and contributions.
        {sheet && ` · ${fmt(sheet.souls)} souls`}
      </footer>
      </div>
    </div>
  );
}
