import { useEffect, useState } from "react";
import { api, ConsoleState } from "../api";
import { CONDEBUG_HELP } from "../live";
import { date, LANG, LANGUAGES, NAMES, setLang, setNames, t, tn } from "../i18n";
import { accountFrom, SETTINGS, useSetting } from "../settings";
import RankRange from "./RankRange";
import ErrorNote from "./ErrorNote";
import { DAMAGES, FOCUSES } from "./GeneratorView";

/** What the player sets once: their account, the defaults of each screen, and what runs in the background. */
export default function SettingsView({ version, fetchedAt, onRefresh, onGuide }: { version: number | null; fetchedAt: number; onRefresh: () => void; onGuide: () => void }) {
  const [account, setAccount] = useSetting(SETTINGS.account);
  const [typed, setTyped] = useState(account);
  useEffect(() => setTyped(account), [account]);
  const [focus, setFocus] = useSetting(SETTINGS.masterFocus);
  const [damage, setDamage] = useSetting(SETTINGS.masterDamage);
  const [watch, setWatch] = useSetting(SETTINGS.watchLive);
  const [share, setShare] = useSetting(SETTINGS.shareMatches);
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
          <input type="checkbox" checked={share !== "0"} onChange={(e) => setShare(e.target.checked ? "1" : "0")} />
          <span>
            <b>{t("Share my finished matches")}</b>
            <small className="muted">
              {t("When a match ends, ask deadlock-api for it, which keeps the match and its replay for anyone to study. Only the match number is sent. Needs -condebug.")}
            </small>
          </span>
        </label>
      </section>

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
