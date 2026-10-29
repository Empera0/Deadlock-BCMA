import { useCallback, useEffect, useMemo, useState } from "react";
import { api, Build, Combo, ComboResult, EMPTY_MAP, EMPTY_ZONE, fmt, GameData, Ranked, Sheet, MatchPreset } from "./api";
import BuildBar from "./components/BuildBar";
import ComparePanel from "./components/ComparePanel";
import DraftView from "./components/DraftView";
import ComboPanel from "./components/ComboPanel";
import GeneratorView from "./components/GeneratorView";
import MiscPanel from "./components/MiscPanel";
import HeroPicker from "./components/HeroPicker";
import MatchView from "./components/MatchView";
import Shop from "./components/Shop";
import StatsPanel from "./components/StatsPanel";
import Guide, { GuideButton, useFirstStart, useGuide } from "./components/Guide";
import SettingsView from "./components/SettingsView";
import TiersView from "./components/TiersView";
import CommandPalette, { heroCommands } from "./components/CommandPalette";
import PatchNotes from "./components/PatchNotes";
import ShareMatches from "./components/ShareMatches";
import UpdateBanner from "./components/UpdateBanner";
import WhatsNew from "./components/WhatsNew";
import { date, t } from "./i18n";

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

type Mode = "build" | "generator" | "draft" | "heroes" | "match" | "settings";
/** The screens, in the order of the top menu; the names translated where shown. */
const SCREENS: [Mode, string][] = [
  ["build", "Build maker"],
  ["generator", "Build generator"],
  ["draft", "Draft"],
  ["heroes", "Hero tiers"],
  ["match", "Match analysis"],
  ["settings", "Settings"],
];

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
  const [tab, setTab] = useState<"stats" | "combo" | "misc" | "compare">("stats");
  const [mode, setMode] = useState<Mode>("build");
  const guide = useGuide();
  useFirstStart(() => guide.show("start"));
  /** A match player handed to the master builder. */
  const [masterPreset, setMasterPreset] = useState<MatchPreset | null>(null);
  // Set to open the Build generator on its Popular tab, from the hero list.
  const [popularAt, setPopularAt] = useState(0);
  const [searching, setSearching] = useState(false);
  // Ctrl+K (or Cmd+K) opens the search from anywhere.
  useEffect(() => {
    const on = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setSearching((v) => !v);
      }
    };
    window.addEventListener("keydown", on);
    return () => window.removeEventListener("keydown", on);
  }, []);
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
        // The game data is fetched again on every start, so a new hero or a patch shows up without a click.
        api
          .refreshData()
          .then((fresh) => {
            setData(fresh);
            const known = new Set(d.heroes.map((h) => h.class_name));
            const added = fresh.heroes.filter((h) => !known.has(h.class_name)).map((h) => h.name);
            if (added.length) setStatus(t("Game data updated: new hero {names}", { names: added.join(", ") }));
            else if (fresh.client_version !== d.client_version) setStatus(t("Game data updated to version {v}", { v: fresh.client_version ?? "?" }));
          })
          .catch(() => {});
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

  if (!data || !build) return <div className="loading">{status || t("Loading…")}</div>;
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
    run(t("Saved \"{name}\"", { name }), async () => {
      await api.saveBuild(name, build);
      setSaved(await api.listBuilds());
    });
  const load = (n: string) =>
    run(t("Loaded \"{name}\"", { name: n }), async () => {
      setBuild({ ...emptyBuild(hero.class_name), ...(await api.loadBuild(n)) });
      setName(n);
    });
  const remove = () =>
    run(t("Deleted \"{name}\"", { name }), async () => {
      await api.deleteBuild(name);
      setSaved(await api.listBuilds());
    });
  const refresh = () => {
    setStatus(t("Downloading data…"));
    run(t("Data updated"), async () => setData(await api.refreshData()));
  };

  const maxSouls = hero.levels[hero.levels.length - 1]?.required_souls ?? 50000;
  const souls = Math.max(build.souls ?? 0, sheet?.spent ?? 0);

  return (
    <div className="app">
      <header className="topbar">
        <span className="brand">
          DEADLOCK <i>BCMA</i>
        </span>
        <nav className="tabs">
          {SCREENS.map(([key, label]) => (
            <button key={key} className={mode === key ? "on" : ""} onClick={() => setMode(key)}>
              {t(label)}
            </button>
          ))}
        </nav>
        <button className="search" onClick={() => setSearching(true)} title={t("Search a screen, a hero or an action")}>
          <svg viewBox="0 0 24 24" width="14" height="14">
            <path d="M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-4-4" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
          </svg>
          <span>{t("Search…")}</span>
          <kbd>Ctrl K</kbd>
        </button>
        <div className="top-actions">
          <UpdateBanner />
          <WhatsNew />
          <PatchNotes dataAt={data.fetched_at} />
          <ShareMatches quiet />
          <button
            className="ghost icon"
            onClick={refresh}
            title={t("Update data") + " · " + t("Game") + ` ${data.client_version ?? "?"} · ${date(data.fetched_at)}`}
          >
            <svg viewBox="0 0 24 24" width="15" height="15">
              <path d="M20 11a8 8 0 1 0-2.3 5.7M20 4v7h-7" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
          </button>
          <GuideButton onOpen={() => guide.show(mode)} />
        </div>
      </header>
      <CommandPalette
        open={searching}
        onClose={() => setSearching(false)}
        commands={[
          ...SCREENS.map(([key, label]) => ({ key: "screen-" + key, label: t(label), hint: t("Screen"), run: () => setMode(key) })),
          ...heroCommands(data.heroes, (h) => {
            patch({ hero: h.class_name, ability_tiers: [0, 0, 0, 0] });
            setMode("build");
          }),
          { key: "refresh", label: t("Update data"), hint: t("Action"), run: refresh },
          { key: "guide", label: t("Open the guide"), hint: t("Action"), run: () => guide.show(mode) },
        ]}
      />
      <div className="page">
      <header className={mode}>
        <button className="hero-button" onClick={() => setPicking(true)}>
          <img src={hero.icon} alt="" />
          <span>
            <strong>{hero.name}</strong>
            <small>{t("Change hero")}</small>
          </span>
        </button>
        <label className="souls">
          {t("Total souls")}
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
              <b>{t("Level")}</b>
              <input
                type="number"
                className="level-input"
                title={t("Sets total souls to the amount needed for this level")}
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
          <input placeholder={t("Build name")} value={name} onChange={(e) => setName(e.target.value)} />
          <button onClick={save} disabled={!name.trim()}>
            {t("Save")}
          </button>
          <select value="" onChange={(e) => e.target.value && load(e.target.value)}>
            <option value="">{t("Load…")}</option>
            {saved.map((s) => (
              <option key={s}>{s}</option>
            ))}
          </select>
          <button className="ghost" onClick={remove} disabled={!saved.includes(name)}>
            {t("Delete")}
          </button>
        </div>
        <div className="data-info">{status && <small className="status">{status}</small>}</div>
      </header>
      <Guide open={guide.open} topic={guide.topic} onClose={guide.close} onTopic={guide.setTopic} />

      {/* Kept mounted while the builder is shown so a loaded match stays loaded. */}
      <div style={{ display: mode === "match" ? "contents" : "none" }}>
        <MatchView
          onOpenBuild={(b, target) => {
            // The builder keeps its own target unless the build is opened against an enemy of the match.
            setBuild({ ...emptyBuild(b.hero), ...b, target: target ?? build.target });
            setMode("build");
            setStatus(target ? t("Opened a build from the match, with that enemy as the target") : t("Opened a build from the match"));
          }}
          onOpenMaster={(preset) => {
            setMasterPreset(preset);
            setMode("generator");
          }}
        />
      </div>

      {mode === "settings" && (
        <SettingsView version={data.client_version ?? null} fetchedAt={data.fetched_at} onRefresh={refresh} onGuide={() => guide.show("setup")} />
      )}

      {/* Kept mounted so the draft stays filled in while a build is planned. */}
      <div style={{ display: mode === "draft" ? "contents" : "none" }}>
        <DraftView
          heroes={data.heroes}
          onPlan={(preset) => {
            setMasterPreset(preset);
            setMode("generator");
          }}
        />
      </div>

      {mode === "heroes" && (
        <TiersView
          heroes={data.heroes}
          onBuild={(heroClass) => {
            if (heroClass !== build.hero) setBuild({ ...emptyBuild(heroClass), target: build.target, map: build.map });
            setPopularAt(Date.now());
            setMode("generator");
          }}
        />
      )}

      {mode === "generator" && sheet && (
        <GeneratorView
          heroes={data.heroes}
          hero={hero}
          build={build}
          souls={sheet.souls}
          preset={masterPreset}
          popularAt={popularAt}
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

      <main
        className="arena"
        style={{ display: mode === "build" ? undefined : "none", ...(hero.background ? { ["--hero-bg" as string]: `url("${hero.background}")` } : {}) }}
      >
        <Shop items={data.items} owned={owned} purchased={purchased} onToggle={toggleItem} />
        {sheet && (
          <div className="side">
            <div className="side-tabs">
              <button className={tab === "stats" ? "on" : ""} onClick={() => setTab("stats")}>
                {t("Stats")}
              </button>
              <button className={tab === "combo" ? "on" : ""} onClick={() => setTab("combo")}>
                {t("Combo")}
              </button>
              <button className={tab === "misc" ? "on" : ""} onClick={() => setTab("misc")}>
                {t("Misc")}
              </button>
              <button className={tab === "compare" ? "on" : ""} onClick={() => setTab("compare")}>
                {t("Compare")}
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
            ) : tab === "compare" ? (
              <ComparePanel heroes={data.heroes} items={items} build={build} sheet={sheet} />
            ) : (
              <MiscPanel sheet={sheet} build={build} />
            )}
          </div>
        )}
      </main>

      {mode === "build" && <BuildBar build={build} heroes={data.heroes} sheet={sheet} items={items} onChange={patch} />}

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
      <footer style={mode === "build" ? undefined : { display: "none" }}>
        {t("Click a stat to see its formula and contributions.")}
        {sheet && ` · ${t("{n} souls", { n: fmt(sheet.souls) })}`}
      </footer>
      </div>
    </div>
  );
}
