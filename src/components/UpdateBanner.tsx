import { useEffect, useRef, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { check, Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

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
    setProgress("Downloading…");
    await found.download((event) => {
      if (event.event === "Started") total = event.data.contentLength ?? 0;
      if (event.event === "Progress") {
        done += event.data.chunkLength;
        setProgress(total > 0 ? `Downloading ${found.version}… ${Math.round((done / total) * 100)}%` : `Downloading ${found.version}…`);
      }
    });
  };

  const install = async (found: Update) => {
    setProgress(`Installing ${found.version}…`);
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
        setProgress(`Update failed: ${String(e)}`);
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
      setProgress(`Update failed: ${String(e)}`);
    } finally {
      busy.current = false;
    }
  };

  const failed = progress.startsWith("Update failed");
  return (
    <span className="app-version">
      {version && <small>App {version}</small>}
      {update && (
        <button className="update-button" disabled={progress !== "" && !failed} onClick={retry} title={failed ? progress : update.body ?? ""}>
          {failed ? `Retry update to ${update.version}` : progress || (ready ? `Restart to update to ${update.version}` : `Update to ${update.version}`)}
        </button>
      )}
    </span>
  );
}
