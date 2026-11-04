import { useEffect, useState } from "react";

/** Notes and tags the player puts on matches, kept on this computer (and in backups), by match ID. */
export interface MatchNote {
  note: string;
  tags: string[];
}

const KEY = "match-notes";
const EVENT = "bcma-match-notes";

function readAll(): Record<string, MatchNote> {
  try {
    return JSON.parse(localStorage.getItem(KEY) ?? "{}");
  } catch {
    return {};
  }
}

function writeAll(all: Record<string, MatchNote>) {
  try {
    localStorage.setItem(KEY, JSON.stringify(all));
  } catch {
    /* Not kept: the note holds for this session only. */
  }
  window.dispatchEvent(new CustomEvent(EVENT));
}

/** Every match's note, kept in step across the screens, and a way to change one. */
export function useMatchNotes(): [Record<string, MatchNote>, (matchId: number, note: MatchNote) => void] {
  const [all, setAll] = useState(readAll);
  useEffect(() => {
    const on = () => setAll(readAll());
    window.addEventListener(EVENT, on);
    return () => window.removeEventListener(EVENT, on);
  }, []);
  const set = (matchId: number, note: MatchNote) => {
    const next = { ...readAll() };
    const tags = [...new Set(note.tags.map((x) => x.trim()).filter(Boolean))];
    if (!note.note.trim() && tags.length === 0) delete next[matchId];
    else next[matchId] = { note: note.note, tags };
    writeAll(next);
  };
  return [all, set];
}

/** Every tag used, the most used first. */
export function allTags(all: Record<string, MatchNote>): string[] {
  const count = new Map<string, number>();
  for (const n of Object.values(all)) for (const tag of n.tags) count.set(tag, (count.get(tag) ?? 0) + 1);
  return [...count.entries()].sort((a, b) => b[1] - a[1]).map(([tag]) => tag);
}
