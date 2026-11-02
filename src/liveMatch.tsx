import { createContext, ReactNode, useContext, useEffect, useRef, useState } from "react";
import { api, LaneEnemy, LiveMatch } from "./api";
import { savedAccount } from "./live";
import { SETTINGS, useSetting } from "./settings";

/** How often the console log is read, and how often the teams are asked for again while Valve has not started
 *  broadcasting the match, ms. */
const WATCH_MS = 20_000;
const RETRY_MS = 60_000;

export type LivePhase = "off" | "no-log" | "waiting" | "loading" | "ready" | "ended";

export interface LiveState {
  phase: LivePhase;
  matchId: number | null;
  /** When the match was first seen, ms. */
  since: number;
  live: LiveMatch | null;
  lane: LaneEnemy[] | null;
  /** Why the teams are not in yet (Valve not broadcasting, the service busy…). */
  error: string;
  /** Reads the teams again now. */
  retry: () => void;
}

const empty: LiveState = { phase: "off", matchId: null, since: 0, live: null, lane: null, error: "", retry: () => {} };
const Context = createContext<LiveState>(empty);

/** The match being played, followed on its own: the console log names it, then its teams, and the note for the
 *  player's lane, are read once. Everything that shows the live match reads it from here. */
export function LiveProvider({ children }: { children: ReactNode }) {
  const [setting] = useSetting(SETTINGS.liveTrack);
  const [badge] = useSetting(SETTINGS.draftBadge);
  const [maxBadge] = useSetting(SETTINGS.draftMaxBadge);
  const on = setting !== "0";
  const [state, setState] = useState<LiveState>(empty);
  const current = useRef<{ id: number; tried: number; done: boolean } | null>(null);
  const [nudge, setNudge] = useState(0);

  useEffect(() => {
    if (!on) {
      setState({ ...empty, phase: "off" });
      return;
    }
    let busy = false;
    const tick = async () => {
      if (busy) return;
      busy = true;
      try {
        const c = await api.consoleMatch().catch(() => null);
        if (!c || !c.found) {
          setState((s) => ({ ...s, phase: "no-log" }));
          return;
        }
        const id = c.match_id ?? null;
        if (id === null) {
          // The match ended (or none yet): what was read stays to look at.
          setState((s) => ({ ...s, phase: s.matchId !== null ? "ended" : "waiting" }));
          current.current = null;
          return;
        }
        if (current.current?.id !== id) {
          current.current = { id, tried: 0, done: false };
          setState({ ...empty, phase: "loading", matchId: id, since: Date.now() });
        }
        const cur = current.current;
        if (cur.done || Date.now() - cur.tried < RETRY_MS) return;
        cur.tried = Date.now();
        const account = c.account_id ? String(c.account_id) : savedAccount();
        if (!account) {
          setState((s) => ({ ...s, error: "The console log names no account: set your Steam account ID in Settings." }));
          return;
        }
        try {
          const live = await api.matchTeams(id, account);
          cur.done = true;
          setState((s) => ({ ...s, phase: "ready", live, error: "" }));
          if (live.lane.length > 0) {
            const lane = await api.laneNote(live.hero, live.lane, Number(badge) || 0, maxBadge ? Number(maxBadge) : null).catch(() => null);
            setState((s) => (s.matchId === id ? { ...s, lane } : s));
          }
        } catch (e) {
          setState((s) => ({ ...s, error: String(e) }));
        }
      } finally {
        busy = false;
      }
    };
    tick();
    const timer = window.setInterval(tick, WATCH_MS);
    return () => window.clearInterval(timer);
  }, [on, badge, maxBadge, nudge]);

  const retry = () => {
    if (current.current) current.current.tried = 0;
    setNudge((n) => n + 1);
  };
  return <Context.Provider value={{ ...state, retry }}>{children}</Context.Provider>;
}

export function useLive(): LiveState {
  return useContext(Context);
}
