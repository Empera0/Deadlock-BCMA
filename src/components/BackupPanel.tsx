import { useRef, useState } from "react";
import { api } from "../api";
import { t } from "../i18n";
import ErrorNote from "./ErrorNote";

/** Everything the window keeps, as one object. */
function allSettings(): Record<string, string> {
  const out: Record<string, string> = {};
  try {
    for (let i = 0; i < localStorage.length; i++) {
      const key = localStorage.key(i);
      if (key !== null) out[key] = localStorage.getItem(key) ?? "";
    }
  } catch {
    /* Nothing kept: the backup holds the builds and matches only. */
  }
  return out;
}

/** A backup of the settings, saved builds and kept matches in one file, and putting one back. */
export default function BackupPanel() {
  const [withMatches, setWithMatches] = useState(true);
  const [note, setNote] = useState("");
  const [error, setError] = useState("");
  const file = useRef<HTMLInputElement>(null);

  const save = async () => {
    setError("");
    setNote(t("Working…"));
    try {
      const path = await api.exportBackup(allSettings(), withMatches);
      setNote(t("Saved to {path}", { path }));
    } catch (e) {
      setNote("");
      setError(String(e));
    }
  };
  const restore = async (picked: File | undefined) => {
    if (!picked) return;
    setError("");
    setNote(t("Working…"));
    try {
      const done = await api.importBackup(await picked.text());
      try {
        for (const [key, value] of Object.entries(done.settings ?? {})) localStorage.setItem(key, value);
      } catch {
        /* The settings could not be kept: the builds and matches are back all the same. */
      }
      setNote(t("Restored {b} builds and {m} matches; the app starts again.", { b: done.builds, m: done.matches }));
      setTimeout(() => window.location.reload(), 1500);
    } catch (e) {
      setNote("");
      setError(String(e));
    }
  };

  return (
    <section className="group">
      <h3>{t("Backup")}</h3>
      <p className="muted">{t("Your settings, saved builds and, if you like, the matches kept on this computer, in one file: to move to another computer or keep safe.")}</p>
      <label className="check">
        <input type="checkbox" checked={withMatches} onChange={(e) => setWithMatches(e.target.checked)} />
        {t("Include the kept matches (a larger file)")}
      </label>
      <div className="fields">
        <button onClick={save}>{t("Save a backup")}</button>
        <button className="ghost" onClick={() => file.current?.click()}>
          {t("Restore a backup")}
        </button>
        <input ref={file} type="file" accept=".json,application/json" style={{ display: "none" }} onChange={(e) => restore(e.target.files?.[0])} />
      </div>
      {note && <p className="muted">{note}</p>}
      <ErrorNote error={error} />
    </section>
  );
}
