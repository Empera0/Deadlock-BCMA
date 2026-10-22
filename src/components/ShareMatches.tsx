import { useEffect, useState } from "react";
import { api } from "../api";

const ON_KEY = "share-matches";
const STATE_KEY = "shared-matches";
/** How often the console log is read for a match that ended, ms. */
const EVERY_MS = 60_000;
/** A match whose replay is not out yet is asked about again after these waits, ms: Valve keeps the replay a while
 * after the match, or not at all. */
const RETRY_MS = [30 * 60_000, 3 * 60 * 60_000];
/** Matches remembered, the most recent kept. */
const KEEP = 200;

interface Entry {
  tries: number;
  last: number;
  metadata: boolean;
  replay: boolean;
}

function load(): Record<string, Entry> {
  try {
    return JSON.parse(localStorage.getItem(STATE_KEY) ?? "{}");
  } catch {
    return {};
  }
}

function save(state: Record<string, Entry>) {
  try {
    const kept = Object.entries(state)
      .sort((a, b) => b[1].last - a[1].last)
      .slice(0, KEEP);
    localStorage.setItem(STATE_KEY, JSON.stringify(Object.fromEntries(kept)));
  } catch {
    /* Storage unavailable: the matches are asked about again next time. */
  }
}

/** Whether a match is due: never shared, or shared without its replay and its next wait is over. */
function due(e: Entry | undefined, now: number): boolean {
  if (!e) return true;
  if (e.replay) return false;
  const wait = RETRY_MS[e.tries - 1];
  return wait !== undefined && now - e.last >= wait;
}

/**
 * Shares every match the player finishes with the statistics service (deadlock-api), read from the game's console
 * log: the service fetches its salts from Steam and keeps them, so the match and its replay can be studied. On by
 * default; the switch is remembered.
 */
export default function ShareMatches() {
  const [on, setOn] = useState(() => {
    try {
      return localStorage.getItem(ON_KEY) !== "0";
    } catch {
      return true;
    }
  });
  const [last, setLast] = useState<string>("");

  useEffect(() => {
    try {
      localStorage.setItem(ON_KEY, on ? "1" : "0");
    } catch {
      /* Not remembered. */
    }
    if (!on) return;
    let busy = false;
    const tick = async () => {
      if (busy) return;
      busy = true;
      try {
        const c = await api.consoleMatch();
        if (!c.found) return;
        const state = load();
        const now = Date.now();
        for (const id of c.finished ?? []) {
          const key = String(id);
          if (!due(state[key], now)) continue;
          try {
            const s = await api.shareMatch(id);
            state[key] = { tries: (state[key]?.tries ?? 0) + 1, last: now, metadata: s.metadata, replay: s.replay };
            setLast(`Match ${id} shared${s.replay ? " with its replay" : s.metadata ? "; replay not out yet" : ""}`);
          } catch {
            // Not known to the service yet: asked again on the next round, as if never tried.
          }
        }
        save(state);
      } finally {
        busy = false;
      }
    };
    tick();
    const timer = window.setInterval(tick, EVERY_MS);
    return () => window.clearInterval(timer);
  }, [on]);

  return (
    <label
      className="share-matches"
      title="When a match ends, the app reads its number from the game's console log (-condebug) and asks deadlock-api for it, which then keeps the match and its replay for anyone to study. Only the match number is sent."
    >
      <input type="checkbox" checked={on} onChange={(e) => setOn(e.target.checked)} />
      <span>
        Share my finished matches
        {on && last && <small>{last}</small>}
      </span>
    </label>
  );
}
