import { useEffect, useMemo, useRef, useState } from "react";
import { Hero } from "../api";
import { t } from "../i18n";

export interface Command {
  key: string;
  label: string;
  /** What kind of thing it is, shown on the right. */
  hint: string;
  icon?: string;
  run: () => void;
}

/** Ctrl+K: jump to a screen, open a hero, or run an action, by typing a few letters of it. */
export default function CommandPalette({ open, onClose, commands }: { open: boolean; onClose: () => void; commands: Command[] }) {
  const [query, setQuery] = useState("");
  const [at, setAt] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (open) {
      setQuery("");
      setAt(0);
      setTimeout(() => input.current?.focus(), 0);
    }
  }, [open]);
  const shown = useMemo(() => {
    const q = query.trim().toLocaleLowerCase();
    const list = q ? commands.filter((c) => c.label.toLocaleLowerCase().includes(q)) : commands;
    // Matches at the start of a word first.
    return [...list].sort((a, b) => Number(b.label.toLocaleLowerCase().startsWith(q)) - Number(a.label.toLocaleLowerCase().startsWith(q))).slice(0, 12);
  }, [query, commands]);
  if (!open) return null;
  const pick = (c: Command | undefined) => {
    if (!c) return;
    onClose();
    c.run();
  };
  return (
    <div className="overlay palette-overlay" onClick={onClose}>
      <div className="palette" role="dialog" aria-label={t("Search")} onClick={(e) => e.stopPropagation()}>
        <input
          ref={input}
          value={query}
          placeholder={t("Type a screen, a hero or an action…")}
          onChange={(e) => (setQuery(e.target.value), setAt(0))}
          onKeyDown={(e) => {
            if (e.key === "Escape") onClose();
            if (e.key === "ArrowDown") (e.preventDefault(), setAt((a) => Math.min(a + 1, shown.length - 1)));
            if (e.key === "ArrowUp") (e.preventDefault(), setAt((a) => Math.max(a - 1, 0)));
            if (e.key === "Enter") pick(shown[at]);
          }}
        />
        <div className="palette-list">
          {shown.map((c, i) => (
            <button key={c.key} className={"palette-item" + (i === at ? " on" : "")} onMouseEnter={() => setAt(i)} onClick={() => pick(c)}>
              {c.icon ? <img src={c.icon} alt="" /> : <span className="palette-dot" />}
              <span>{c.label}</span>
              <small>{c.hint}</small>
            </button>
          ))}
          {shown.length === 0 && <p className="muted">{t("Nothing found.")}</p>}
        </div>
      </div>
    </div>
  );
}

/** The heroes as commands that open them in the Build maker. */
export function heroCommands(heroes: Hero[], open: (h: Hero) => void): Command[] {
  return heroes.map((h) => ({ key: "hero-" + h.class_name, label: h.name, hint: t("Hero"), icon: h.icon, run: () => open(h) }));
}
