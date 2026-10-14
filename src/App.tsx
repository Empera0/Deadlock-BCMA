import { useCallback, useEffect, useMemo, useState } from "react";
import { api, Build, Combo, ComboResult, fmt, GameData, Ranked, Sheet } from "./api";
import BuildBar from "./components/BuildBar";
import ComboPanel from "./components/ComboPanel";
import SuggestPanel from "./components/SuggestPanel";
import MiscPanel from "./components/MiscPanel";
import HeroPicker from "./components/HeroPicker";
import MatchView from "./components/MatchView";
import Shop from "./components/Shop";
import StatsPanel from "./components/StatsPanel";
import UpdateBanner from "./components/UpdateBanner";

const STORAGE_KEY = "current-build";
const COMBO_KEY = "current-combo";

function restoreCombo(): Combo {
  const empty: Combo = { actions: [], headshot_rate: 0, own_health: 50 };
  try {
    const raw = localStorage.getItem(COMBO_KEY);
    return raw ? { ...empty, ...JSON.parse(raw) } : empty;
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
  copied_ult: null,
  target: { health: 2000, bullet_resist: 0, spirit_resist: 0 },
});

function restoreBuild(): Build | null {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    return raw ? { ...emptyBuild(""), ...JSON.parse(raw) } : null;
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
  const [tab, setTab] = useState<"stats" | "combo" | "builds" | "misc">("stats");
  const [mode, setMode] = useState<"build" | "match">("build");
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
    <div className="app">
      <header className={mode}>
        <div className="mode-tabs">
          <button className={mode === "build" ? "on" : ""} onClick={() => setMode("build")}>
            Builder
          </button>
          <button className={mode === "match" ? "on" : ""} onClick={() => setMode("match")}>
            Match analysis
          </button>
        </div>
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
        <div className="saves">
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
        <div className="data-info">
          <UpdateBanner />
          <small>
            Game version {data.client_version ?? "?"} · {new Date(data.fetched_at * 1000).toLocaleDateString("en-US")}
          </small>
          <button className="ghost" onClick={refresh}>
            Update data
          </button>
          {status && <small className="status">{status}</small>}
        </div>
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
        />
      </div>

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
              <button className={tab === "builds" ? "on" : ""} onClick={() => setTab("builds")}>
                Builds
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
              <ComboPanel sheet={sheet} combo={combo} result={comboResult} onChange={setCombo} />
            ) : tab === "misc" ? (
              <MiscPanel sheet={sheet} />
            ) : (
              <SuggestPanel
                hero={hero}
                build={build}
                souls={sheet.souls}
                onUse={(purchases) => {
                  patch({ purchases, active: purchases, upto: null, imbues: {} });
                  setStatus("Loaded a build the calculator put together");
                }}
              />
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
  );
}
