import { englishOf } from "../engineText";
import { t } from "../i18n";

/** What went wrong, said plainly: a title, what to do about it, and the raw message for anyone who needs it. */
export interface Explained {
  title: string;
  hint: string;
  raw: string;
}

/** Turns an error from the app's backend or the statistics service into words a player can act on. */
export function explain(error: unknown): Explained {
  const raw = (error instanceof Error ? error.message : String(error)).replace(/^Error:\s*/, "").trim();
  // Sorted by the backend's English words; shown in the app's language.
  const english = englishOf(raw);
  const has = (re: RegExp) => re.test(english);
  if (has(/rate limit|429|too many requests|busy/i)) {
    return { title: t("The statistics service is busy"), hint: t("It limits how often one computer can ask. Wait a minute and try again."), raw };
  }
  if (has(/download failed|failed to download|failed to read|error sending request|connection|dns|timed out|timeout|unreachable|network/i)) {
    return {
      title: t("Could not reach the statistics service"),
      hint: t("Check your internet connection. The service may also be down for a moment; what was fetched before still works."),
      raw,
    };
  }
  if (has(/unexpected form|expected value|missing field|invalid type|eof while parsing/i)) {
    return {
      title: t("The statistics service answered in a form the app does not know"),
      hint: t("The service has likely changed. An app update usually fixes this; until then, try again later."),
      raw,
    };
  }
  if (has(/5\d\d|bad gateway|service unavailable|internal server error/i)) {
    return { title: t("The statistics service had a problem"), hint: t("It is on their side. Try again in a few minutes."), raw };
  }
  if (has(/console log was not found|-condebug/i)) {
    return { title: t("The game's console log was not found"), hint: t("Add -condebug to Deadlock's launch options in Steam and start the game again (see Settings)."), raw };
  }
  // The app's own messages are already written for players: keep them as the title.
  if (raw.length > 0 && raw.length <= 160 && /^[A-ZÇĞİÖŞÜ]/.test(raw) && !has(/panicked|unwrap|\bat src\//)) {
    return { title: raw, hint: "", raw: "" };
  }
  return { title: t("Something went wrong"), hint: t("Try again. If it keeps happening, the details below help find the cause."), raw };
}

/** An error, explained, with a button to try again when there is something to retry. */
export default function ErrorNote({ error, onRetry }: { error: unknown; onRetry?: () => void }) {
  if (error === null || error === undefined || error === "") return null;
  const e = explain(error);
  return (
    <div className="error error-note" role="alert">
      <b>{e.title}</b>
      {e.hint && <span>{e.hint}</span>}
      <span className="error-actions">
        {onRetry && (
          <button className="ghost small" onClick={onRetry}>
            {t("Try again")}
          </button>
        )}
        {e.raw && (
          <details>
            <summary>{t("Details")}</summary>
            <code>{e.raw}</code>
          </details>
        )}
      </span>
    </div>
  );
}
