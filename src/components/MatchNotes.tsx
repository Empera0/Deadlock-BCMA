import { useState } from "react";
import { allTags, useMatchNotes } from "../matchNotes";
import { t } from "../i18n";

/** Tags and a note on a match, by its title. */
export default function MatchNotes({ matchId }: { matchId: number }) {
  const [all, set] = useMatchNotes();
  const mine = all[matchId] ?? { note: "", tags: [] };
  const [typed, setTyped] = useState("");
  const [open, setOpen] = useState(false);
  const suggestions = allTags(all).filter((x) => !mine.tags.includes(x));
  const add = (tag: string) => {
    if (!tag.trim()) return;
    set(matchId, { ...mine, tags: [...mine.tags, tag.trim()] });
    setTyped("");
  };
  return (
    <div className="match-notes">
      {mine.tags.map((tag) => (
        <span key={tag} className="tag-chip">
          {tag}
          <button className="chip-btn" title={t("Remove")} onClick={() => set(matchId, { ...mine, tags: mine.tags.filter((x) => x !== tag) })}>
            ×
          </button>
        </span>
      ))}
      <input
        className="tag-input"
        list={`tags-${matchId}`}
        placeholder={t("Add a tag…")}
        value={typed}
        onChange={(e) => setTyped(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && add(typed)}
        onBlur={() => add(typed)}
      />
      <datalist id={`tags-${matchId}`}>
        {suggestions.map((x) => (
          <option key={x} value={x} />
        ))}
      </datalist>
      <button className="ghost small" onClick={() => setOpen(!open)}>
        {mine.note ? t("Note ✓") : t("Note")}
      </button>
      {open && (
        <textarea
          className="match-note"
          placeholder={t("What you tried, what went wrong, what to do next time…")}
          value={mine.note}
          onChange={(e) => set(matchId, { ...mine, note: e.target.value })}
        />
      )}
    </div>
  );
}
