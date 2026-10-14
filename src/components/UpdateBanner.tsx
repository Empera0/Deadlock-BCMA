import { useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import { check, Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

/** The app's version, and a button to install a newer one when the GitHub releases have it. */
export default function UpdateBanner() {
  const [version, setVersion] = useState("");
  const [update, setUpdate] = useState<Update | null>(null);
  const [progress, setProgress] = useState<string>("");

  useEffect(() => {
    getVersion().then(setVersion).catch(() => {});
    // No network or no release yet: the app simply carries on.
    check()
      .then((found) => setUpdate(found))
      .catch(() => {});
  }, []);

  const install = async () => {
    if (!update) return;
    let total = 0;
    let done = 0;
    setProgress("Downloading…");
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        if (event.event === "Progress") {
          done += event.data.chunkLength;
          setProgress(total > 0 ? `Downloading… ${Math.round((done / total) * 100)}%` : "Downloading…");
        }
        if (event.event === "Finished") setProgress("Installing…");
      });
      await relaunch();
    } catch (e) {
      setProgress(`Update failed: ${String(e)}`);
    }
  };

  return (
    <span className="app-version">
      {version && <small>App {version}</small>}
      {update && (
        <button className="update-button" disabled={progress !== "" && !progress.startsWith("Update failed")} onClick={install} title={update.body ?? ""}>
          {progress || `Update to ${update.version}`}
        </button>
      )}
    </span>
  );
}
