import { useEffect, useRef, useState } from "react";
import { api, fmt, MatchReport } from "../api";
import { t } from "../i18n";
import { savedAccount } from "../live";
import { SETTINGS, useSetting } from "../settings";

const SEEN_KEY = "post-match-seen";
/** How often the console log is read for a match that ended, and how long a match is waited for, ms. */
const EVERY_MS = 60_000;
const WAIT_MS = 30 * 60_000;

function seen(): number[] {
  try {
    return JSON.parse(localStorage.getItem(SEEN_KEY) ?? "null") ?? [];
  } catch {
    return [];
  }
}

function remember(ids: number[]) {
  try {
    localStorage.setItem(SEEN_KEY, JSON.stringify(ids.slice(-50)));
  } catch {
    /* Shown again next time at worst. */
  }
}

interface Ready {
  report: MatchReport;
  slot: number;
}

/**
 * After a match ends (the console log says so), its analysis is fetched (from this PC's Steam cache, Valve or the
 * statistics service, whichever has it first) and a short card says how it went, with a way to open it in full.
 */
export default function PostMatch({ onOpen }: { onOpen: (matchId: number) => void }) {
  const [setting] = useSetting(SETTINGS.postMatch);
  const on = setting !== "0";
  const [ready, setReady] = useState<Ready | null>(null);
  // The match waited for, and since when.
  const waiting = useRef<{ id: number; since: number } | null>(null);
  // The first look of this run only notes the matches already played: a card is for a match that ends now.
  const primed = useRef(false);

  useEffect(() => {
    if (!on) return;
    let busy = false;
    const tick = async () => {
      if (busy) return;
      busy = true;
      try {
        const c = await api.consoleMatch();
        if (!c.found) return;
        const finished = c.finished ?? [];
        if (!primed.current) {
          primed.current = true;
          remember([...seen(), ...finished.filter((id) => !seen().includes(id))]);
          return;
        }
        const known = seen();
        const fresh = finished.filter((id) => !known.includes(id));
        if (fresh.length > 0 && waiting.current?.id !== fresh[fresh.length - 1]) {
          waiting.current = { id: fresh[fresh.length - 1], since: Date.now() };
          remember([...known, ...fresh.slice(0, -1)]);
        }
        const w = waiting.current;
        if (!w) return;
        try {
          const report = await api.fetchMatch(w.id);
          const account = Number(c.account_id ?? savedAccount());
          const me = report.players.find((p) => p.account_id === account);
          if (me) setReady({ report, slot: me.slot });
          waiting.current = null;
          remember([...seen(), w.id]);
        } catch {
          // Not out yet: tried again next round, until it has been too long.
          if (Date.now() - w.since > WAIT_MS) {
            waiting.current = null;
            remember([...seen(), w.id]);
          }
        }
      } finally {
        busy = false;
      }
    };
    tick();
    const timer = window.setInterval(tick, EVERY_MS);
    return () => window.clearInterval(timer);
  }, [on]);

  if (!ready) return null;
  const { report, slot } = ready;
  const me = report.players.find((p) => p.slot === slot)!;
  const place = report.ranking.findIndex((r) => r.slot === slot) + 1;
  return (
    <div className={"post-match " + (me.won ? "won" : "lost")} role="status">
      <img src={me.hero_icon} alt="" />
      <div>
        <b>{me.won ? t("Victory") : t("Defeat")}</b>
        <span>
          {me.hero_name} · {me.kills} / {me.deaths} / {me.assists} · {t("{n} souls", { n: fmt(me.net_worth) })}
        </span>
        {place > 0 && <small className="muted">{t("{n}. of the twelve players", { n: place })}</small>}
      </div>
      <div className="post-match-actions">
        <button
          onClick={() => {
            onOpen(report.match_id);
            setReady(null);
          }}
        >
          {t("Open the analysis")}
        </button>
        <button className="ghost small" onClick={() => setReady(null)} title={t("Close")}>
          ×
        </button>
      </div>
    </div>
  );
}
