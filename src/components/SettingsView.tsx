import { useEffect, useState } from "react";
import { api, ConsoleState } from "../api";
import { CONDEBUG_HELP } from "../live";
import { accountFrom, RANKS, SETTINGS, useSetting } from "../settings";
import ErrorNote from "./ErrorNote";
import { DAMAGES, FOCUSES } from "./GeneratorView";

/** What the player sets once: their account, the defaults of each screen, and what runs in the background. */
export default function SettingsView({ version, fetchedAt, onRefresh, onGuide }: { version: number | null; fetchedAt: number; onRefresh: () => void; onGuide: () => void }) {
  const [account, setAccount] = useSetting(SETTINGS.account);
  const [typed, setTyped] = useState(account);
  useEffect(() => setTyped(account), [account]);
  const [badge, setBadge] = useSetting(SETTINGS.draftBadge);
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
        <h3>Your account</h3>
        <p className="muted">
          Your Steam account ID: the short number, or the long 7656… one. The Draft screen adds your own record on each hero with it, the live
          match and lobby scouting find you by it, and Match analysis opens on it. To look at someone else's matches, type their ID there instead.
        </p>
        <div className="fields">
          <label className="field">
            Steam account ID
            <input value={typed} placeholder="e.g. 123456789" onChange={(e) => setTyped(e.target.value)} onBlur={save} onKeyDown={(e) => e.key === "Enter" && save()} />
          </label>
          {detected && detected !== account && (
            <button onClick={() => setAccount(detected)} title="The account the game is signed in to, from its console log">
              Use {detected} from the game
            </button>
          )}
        </div>
        {parsed === null && <div className="error">Only digits: the short account ID or the long SteamID.</div>}
        {account && parsed !== null && <div className="muted">Saved: {account}</div>}
      </section>

      <section className="group">
        <h3>The game's console log</h3>
        <ErrorNote error={logError} onRetry={readLog} />
        {log && log.found && (
          <div className="insight good">
            <strong>Found</strong>
            <span>
              {log.path}
              {log.account_id ? ` · signed in as ${log.account_id}` : ""}
              {log.match_id ? ` · in match ${log.match_id}` : ""}
              {log.finished?.length ? ` · ${log.finished.length} finished match${log.finished.length === 1 ? "" : "es"} in it` : ""}
            </span>
          </div>
        )}
        {log && !log.found && (
          <div className="insight bad">
            <strong>Not found</strong>
            <span>
              Looked for {log.path}. {CONDEBUG_HELP}
            </span>
          </div>
        )}
        <button className="ghost small" onClick={readLog}>
          Check again
        </button>
      </section>

      <section className="group">
        <h3>Defaults</h3>
        <div className="fields">
          <label className="field">
            Rank for the Draft screen
            <select value={badge} onChange={(e) => setBadge(e.target.value)}>
              {RANKS.map(([b, label]) => (
                <option key={b} value={b}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            Master builder role
            <select value={focus} onChange={(e) => setFocus(e.target.value)}>
              {FOCUSES.map(([key, name]) => (
                <option key={key} value={key}>
                  {name}
                </option>
              ))}
            </select>
          </label>
          <label className="field">
            Master builder damage
            <select value={damage} onChange={(e) => setDamage(e.target.value)}>
              {DAMAGES.map(([key, name]) => (
                <option key={key} value={key}>
                  {name}
                </option>
              ))}
            </select>
          </label>
        </div>
      </section>

      <section className="group">
        <h3>In the background</h3>
        <label className="toggle-row">
          <input type="checkbox" checked={watch === "1"} onChange={(e) => setWatch(e.target.checked ? "1" : "0")} />
          <span>
            <b>Watch for my match</b>
            <small className="muted">While the Build generator is open, build for your match as soon as it starts. Needs -condebug.</small>
          </span>
        </label>
        <label className="toggle-row">
          <input type="checkbox" checked={share !== "0"} onChange={(e) => setShare(e.target.checked ? "1" : "0")} />
          <span>
            <b>Share my finished matches</b>
            <small className="muted">
              When a match ends, ask deadlock-api for it, which keeps the match and its replay for anyone to study. Only the match number is
              sent. Needs -condebug.
            </small>
          </span>
        </label>
      </section>

      <section className="group">
        <h3>Game data</h3>
        <p className="muted">
          Game version {version ?? "?"}, read on {new Date(fetchedAt * 1000).toLocaleDateString("en-US")}. After a patch, update it to get the new
          heroes and items.
        </p>
        <div className="fields">
          <button onClick={onRefresh}>Update data</button>
          <button className="ghost" onClick={onGuide}>
            Open the guide
          </button>
        </div>
      </section>
    </div>
  );
}
