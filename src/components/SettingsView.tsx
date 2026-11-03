import { useEffect, useMemo, useState } from "react";
import { api, ConsoleState } from "../api";
import { CONDEBUG_HELP } from "../live";
import { date, LANG, LANGUAGES, NAMES, setLang, setNames, t, tn } from "../i18n";
import { accountFrom, CustomColors, customColors, DEFAULT_CUSTOM, FONTS, MADE_THEMES, SETTINGS, THEMES, themeTokens, useSetting } from "../settings";
import RankRange from "./RankRange";
import BackupPanel from "./BackupPanel";
import ErrorNote from "./ErrorNote";
import { DAMAGES, FOCUSES } from "./GeneratorView";

/** What the player sets once: their account, the defaults of each screen, and what runs in the background. */
/** The colors of the Custom theme, each with a picker and its code; the rest of the theme is worked out from them. */
function ThemeEditor({ colors, onChange }: { colors: CustomColors; onChange: (c: CustomColors) => void }) {
  const field = (key: "bg" | "panel" | "text" | "accent" | "weapon" | "vitality" | "spirit", label: string, fallback: string) => {
    const value = colors[key] || fallback;
    return (
      <label className="field color-field">
        <input type="color" value={value} onChange={(e) => onChange({ ...colors, [key]: e.target.value })} />
        <input type="text" value={colors[key]} placeholder={fallback} onChange={(e) => /^#?[0-9a-f]{0,6}$/i.test(e.target.value) && onChange({ ...colors, [key]: e.target.value })} />
        {t(label)}
      </label>
    );
  };
  // Starting from another theme: its colors, to change from there.
  const startFrom = (key: string) => {
    const made = MADE_THEMES.find(([k]) => k === key)?.[2];
    const plain = THEMES.find(([k]) => k === key)?.[2];
    if (made) onChange({ ...made });
    else if (plain) onChange({ ...DEFAULT_CUSTOM, light: key === "paper", bg: plain[0], panel: plain[1], text: plain[2], accent: plain[3] });
  };
  const sample = themeTokens(colors);
  return (
    <div className="theme-editor">
      {field("bg", "Background", DEFAULT_CUSTOM.bg)}
      {field("panel", "Cards", DEFAULT_CUSTOM.panel)}
      {field("text", "Text", DEFAULT_CUSTOM.text)}
      {field("accent", "Accent", DEFAULT_CUSTOM.accent)}
      {field("weapon", "Weapon", "#d98a3d")}
      {field("vitality", "Vitality", "#7cb342")}
      {field("spirit", "Spirit", "#b07fe0")}
      <label className="field inline">
        <input type="checkbox" checked={colors.light} onChange={(e) => onChange({ ...colors, light: e.target.checked })} />
        {t("Light theme")}
      </label>
      <div className="theme-editor-actions">
        <label className="field inline">
          {t("Start from")}
          <select value="" onChange={(e) => startFrom(e.target.value)}>
            <option value="">…</option>
            {[...THEMES.map(([k, n]) => [k, n]), ...MADE_THEMES.map(([k, n]) => [k, n])].map(([k, n]) => (
              <option key={k} value={k}>
                {t(n)}
              </option>
            ))}
          </select>
        </label>
        <button className="ghost small" onClick={() => onChange(DEFAULT_CUSTOM)}>
          {t("Reset")}
        </button>
        <small className="muted" style={{ color: sample["--muted"] }}>
          {t("Lines, hover and muted text are worked out from these colors. Leave Weapon, Vitality or Spirit empty for the usual ones.")}
        </small>
      </div>
    </div>
  );
}

export default function SettingsView({ version, fetchedAt, onRefresh, onGuide }: { version: number | null; fetchedAt: number; onRefresh: () => void; onGuide: () => void }) {
  const [account, setAccount] = useSetting(SETTINGS.account);
  const [typed, setTyped] = useState(account);
  useEffect(() => setTyped(account), [account]);
  const [focus, setFocus] = useSetting(SETTINGS.masterFocus);
  const [damage, setDamage] = useSetting(SETTINGS.masterDamage);
  const [watch, setWatch] = useSetting(SETTINGS.watchLive);
  const [share, setShare] = useSetting(SETTINGS.shareMatches);
  const [postMatch, setPostMatch] = useSetting(SETTINGS.postMatch);
  const [laneNote, setLaneNote] = useSetting(SETTINGS.laneNote);
  const [liveTrack, setLiveTrack] = useSetting(SETTINGS.liveTrack);
  const [liveSwitch, setLiveSwitch] = useSetting(SETTINGS.liveSwitch);
  const [theme, setTheme] = useSetting(SETTINGS.theme);
  const [density, setDensity] = useSetting(SETTINGS.density);
  const [fontScale, setFontScale] = useSetting(SETTINGS.fontScale);
  const [corners, setCorners] = useSetting(SETTINGS.corners);
  const [font, setFont] = useSetting(SETTINGS.font);
  const [motion, setMotion] = useSetting(SETTINGS.motion);
  const [customText, setCustomText] = useSetting(SETTINGS.customTheme);
  // Read again whenever the kept colors change.
  const custom = useMemo(() => customColors(), [customText]);
  const setCustom = (c: CustomColors) => setCustomText(JSON.stringify(c));
  const swatches: [string, string, [string, string, string, string]][] = [
    ...THEMES,
    ...MADE_THEMES.map(([key, name, c]): [string, string, [string, string, string, string]] => [key, name, [c.bg, c.panel, c.text, c.accent]]),
    ["custom", "Custom", [custom.bg, custom.panel, custom.text, custom.accent]],
  ];
  const [log, setLog] = useState<ConsoleState | null>(null);
  const [logError, setLogError] = useState("");

  const readLog = () => {
    setLogError("");
    api
      .consoleMatch()
      .then(setLog)
      .catch((e) => setLogError(String(e)));
  };
  useEffect(readLog, []);

  const parsed = typed.trim() ? accountFrom(typed) : "";
  const save = () => {
    if (parsed !== null) setAccount(parsed);
  };
  const detected = log?.account_id ? String(log.account_id) : "";

  return (
    <div className="settings-view">
      <section className="group">
        <h3>{t("Look")}</h3>
        <div className="theme-grid">
          {swatches.map(([key, name, [bg, panel, text, accent]]) => (
            <button key={key} className={"theme-card" + (theme === key ? " on" : "")} onClick={() => setTheme(key)}>
              <span className="theme-swatch" style={{ background: bg }}>
                <i style={{ background: panel }} />
                <div>
                  <b style={{ background: text, width: "70%" }} />
                  <b style={{ background: text, width: "45%", opacity: 0.5 }} />
                  <b style={{ background: accent, width: "35%" }} />
                </div>
              </span>
              <span>{t(name)}</span>
            </button>
          ))}
        </div>
        {theme === "custom" && <ThemeEditor colors={custom} onChange={setCustom} />}
        <div className="fields">
          <label className="field">
            {t("Spacing")}
            <select value={density} onChange={(e) => setDensity(e.target.value)}>
              <option value="compact">{t("Compact")}</option>
              <option value="comfortable">{t("Comfortable")}</option>
            </select>
          </label>
          <label className="field range-field">
            {t("Size: {n}%", { n: fontScale })}
            <input type="range" min={80} max={140} step={5} value={Number(fontScale) || 100} onChange={(e) => setFontScale(e.target.value)} />
          </label>
          <label className="field">
            {t("Corners")}
            <select value={corners} onChange={(e) => setCorners(e.target.value)}>
              <option value="sharp">{t("Sharp")}</option>
              <option value="normal">{t("Normal")}</option>
              <option value="round">{t("Round")}</option>
            </select>
          </label>
          <label className="field">
            {t("Font")}
            <select value={font} onChange={(e) => setFont(e.target.value)}>
              {FONTS.map(([key, name]) => (
                <option key={key} value={key}>
                  {t(name)}
                </option>
              ))}
            </select>
          </label>
          <label className="field inline">
            <input type="checkbox" checked={motion !== "0"} onChange={(e) => setMotion(e.target.checked ? "1" : "0")} />
            {t("Animations")}
          </label>
        </div>
      </section>

      <section className="group">
        <h3>{t("Language")}</h3>
        <div className="fields">
          <label className="field">
            {t("Language of the app")}
            <select value={LANG} onChange={(e) => setLang(e.target.value as typeof LANG)}>
              {LANGUAGES.map(([code, name]) => (
                <option key={code} value={code}>
                  {name}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            {t("Hero, item and ability names")}
            <select value={NAMES} onChange={(e) => setNames(e.target.value as typeof NAMES)}>
              <option value="en">{t("English (as in the English game)")}</option>
              <option value="tr">{t("Turkish (as in the Turkish game)")}</option>
            </select>
          </label>
        </div>
        <p className="muted">
          {t(
            "The two are set apart: the app can speak Turkish while the names stay as the English game has them, or the other way round. Build codes work either way. The app starts again after a change.",
          )}
        </p>
      </section>

      <section className="group">
        <h3>{t("Your account")}</h3>
        <p className="muted">
          {t(
            "Your Steam account ID: the short number, or the long 7656… one. The Draft screen adds your own record on each hero with it, the live match and lobby scouting find you by it, and Match analysis opens on it. To look at someone else's matches, type their ID there instead.",
          )}
        </p>
        <div className="fields">
          <label className="field">
            {t("Steam account ID")}
            <input value={typed} placeholder={t("e.g. 123456789")} onChange={(e) => setTyped(e.target.value)} onBlur={save} onKeyDown={(e) => e.key === "Enter" && save()} />
          </label>
          {detected && detected !== account && (
            <button onClick={() => setAccount(detected)} title={t("The account the game is signed in to, from its console log")}>
              {t("Use {id} from the game", { id: detected })}
            </button>
          )}
        </div>
        {parsed === null && <div className="error">{t("Only digits: the short account ID or the long SteamID.")}</div>}
        {account && parsed !== null && <div className="muted">{t("Saved: {id}", { id: account })}</div>}
      </section>

      <section className="group">
        <h3>{t("The game's console log")}</h3>
        <ErrorNote error={logError} onRetry={readLog} />
        {log && log.found && (
          <div className="insight good">
            <strong>{t("Found")}</strong>
            <span>
              {log.path}
              {log.account_id ? ` · ${t("signed in as {id}", { id: log.account_id })}` : ""}
              {log.match_id ? ` · ${t("in match {id}", { id: log.match_id })}` : ""}
              {log.finished?.length ? ` · ${tn(log.finished.length, "{n} finished match in it", "{n} finished matches in it")}` : ""}
            </span>
          </div>
        )}
        {log && !log.found && (
          <div className="insight bad">
            <strong>{t("Not found")}</strong>
            <span>
              {t("Looked for {path}.", { path: log.path })} {CONDEBUG_HELP}
            </span>
          </div>
        )}
        <button className="ghost small" onClick={readLog}>
          {t("Check again")}
        </button>
      </section>

      <section className="group">
        <h3>{t("Defaults")}</h3>
        <div className="fields">
          <RankRange label="Draft and hero ranks from" />
          <RankRange fromKey={SETTINGS.buildBadge} toKey={SETTINGS.buildMaxBadge} top label="Builds from" />
          <label className="field">
            {t("Master builder role")}
            <select value={focus} onChange={(e) => setFocus(e.target.value)}>
              {FOCUSES.map(([key, name]) => (
                <option key={key} value={key}>
                  {t(name)}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            {t("Master builder damage")}
            <select value={damage} onChange={(e) => setDamage(e.target.value)}>
              {DAMAGES.map(([key, name]) => (
                <option key={key} value={key}>
                  {t(name)}
                </option>
              ))}
            </select>
          </label>
        </div>
      </section>

      <section className="group">
        <h3>{t("In the background")}</h3>
        <label className="toggle-row">
          <input type="checkbox" checked={watch === "1"} onChange={(e) => setWatch(e.target.checked ? "1" : "0")} />
          <span>
            <b>{t("Watch for my match")}</b>
            <small className="muted">{t("While the Build generator is open, build for your match as soon as it starts. Needs -condebug.")}</small>
          </span>
        </label>
        <label className="toggle-row">
          <input type="checkbox" checked={liveTrack !== "0"} onChange={(e) => setLiveTrack(e.target.checked ? "1" : "0")} />
          <span>
            <b>{t("Follow my matches")}</b>
            <small className="muted">{t("When a match starts, read its teams and players once for the Live screen and the lane note. Needs -condebug.")}</small>
          </span>
        </label>
        <label className="toggle-row">
          <input type="checkbox" checked={liveSwitch !== "0"} onChange={(e) => setLiveSwitch(e.target.checked ? "1" : "0")} />
          <span>
            <b>{t("Open the Live screen when a match starts")}</b>
            <small className="muted">{t("Otherwise the Live tab is only marked.")}</small>
          </span>
        </label>
        <label className="toggle-row">
          <input type="checkbox" checked={laneNote !== "0"} onChange={(e) => setLaneNote(e.target.checked ? "1" : "0")} />
          <span>
            <b>{t("Lane note at the start of a match")}</b>
            <small className="muted">
              {t("When a match starts, show how your hero does against your lane enemies, when they are strongest and what is bought against them. Read once, from the draft's ranks. Needs -condebug.")}
            </small>
          </span>
        </label>
        <label className="toggle-row">
          <input type="checkbox" checked={postMatch !== "0"} onChange={(e) => setPostMatch(e.target.checked ? "1" : "0")} />
          <span>
            <b>{t("Show how my match went")}</b>
            <small className="muted">{t("When a match ends, show a short card with the result, K/D/A and place, and a button to its full analysis. Needs -condebug.")}</small>
          </span>
        </label>
        <label className="toggle-row">
          <input type="checkbox" checked={share !== "0"} onChange={(e) => setShare(e.target.checked ? "1" : "0")} />
          <span>
            <b>{t("Share my finished matches")}</b>
            <small className="muted">
              {t("When a match ends, ask deadlock-api for it, which keeps the match and its replay for anyone to study. Matches you opened in the game are offered too, with where Valve serves them (its server and file keys). Only these numbers are sent. Needs -condebug for matches as they end.")}
            </small>
          </span>
        </label>
      </section>

      <BackupPanel />

      <section className="group">
        <h3>{t("Game data")}</h3>
        <p className="muted">
          {t("Game version {v}, read on {date}. After a patch, update it to get the new heroes and items.", { v: version ?? "?", date: date(fetchedAt) })}
        </p>
        <div className="fields">
          <button onClick={onRefresh}>{t("Update data")}</button>
          <button className="ghost" onClick={onGuide}>
            {t("Open the guide")}
          </button>
        </div>
      </section>
    </div>
  );
}
