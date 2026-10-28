import { useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { check, Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { LANG, t } from "../i18n";

/** While the app runs, it looks for a newer release this often. */
const CHECK_EVERY_MS = 30 * 60 * 1000;

/**
 * The app's version, and updates without asking: one found when the app starts is downloaded, installed and the
 * app restarts on its own. One found later, while the app is in use, is downloaded in the background and waits
 * for a click (or the next start), so a restart never interrupts work.
 */
export default function UpdateBanner() {
  const [version, setVersion] = useState("");
  const [update, setUpdate] = useState<Update | null>(null);
  const [progress, setProgress] = useState("");
  const [ready, setReady] = useState(false);
  const busy = useRef(false);

  const download = async (found: Update) => {
    let total = 0;
    let done = 0;
    setProgress(t("Downloading…"));
    await found.download((event) => {
      if (event.event === "Started") total = event.data.contentLength ?? 0;
      if (event.event === "Progress") {
        done += event.data.chunkLength;
        setProgress(total > 0 ? t("Downloading {v}… {pct}%", { v: found.version, pct: Math.round((done / total) * 100) }) : t("Downloading {v}…", { v: found.version }));
      }
    });
  };

  const install = async (found: Update) => {
    setProgress(t("Installing {v}…", { v: found.version }));
    await found.install();
    await relaunch();
  };

  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
    const look = async (atStart: boolean) => {
      if (busy.current) return;
      busy.current = true;
      try {
        const found = await check();
        if (!found) return;
        setUpdate(found);
        await download(found);
        if (atStart) {
          await install(found);
        } else {
          setReady(true);
          setProgress("");
        }
      } catch (e) {
        // No network or no release: the app carries on, and the button lets the update be tried again.
        setProgress(`${t("Update failed:")} ${String(e)}`);
      } finally {
        busy.current = false;
      }
    };
    look(true);
    const timer = setInterval(() => look(false), CHECK_EVERY_MS);
    return () => clearInterval(timer);
  }, []);

  const retry = async () => {
    if (!update || busy.current) return;
    busy.current = true;
    try {
      if (!ready) await download(update);
      await install(update);
    } catch (e) {
      setProgress(`${t("Update failed:")} ${String(e)}`);
    } finally {
      busy.current = false;
    }
  };

  const failed = progress.startsWith(t("Update failed:"));
  return (
    <span className="app-version">
      {version && <small>{t("App {v}", { v: version })}</small>}
      {update && (
        <button className="update-button" disabled={progress !== "" && !failed} onClick={retry} title={failed ? progress : LANG === "en" ? update.body ?? "" : ""}>
          {failed
            ? t("Retry update to {v}", { v: update.version })
            : progress || t(ready ? "Restart to update to {v}" : "Update to {v}", { v: update.version })}
        </button>
      )}
    </span>
  );
}
