import { useEffect, useState } from "react";
import { api, fmt, ItemPayoff, OrderCompare } from "../api";
import { clock } from "./MatchCharts";
import { t } from "../i18n";
import ErrorNote from "./ErrorNote";

/** Reads once per match and player. */
function useOnce<T>(load: () => Promise<T>, key: string): { value?: T; error?: unknown } | null {
  const [state, setState] = useState<{ value?: T; error?: unknown } | null>(null);
  useEffect(() => {
    let live = true;
    setState(null);
    load().then(
      (value) => live && setState({ value }),
      (error) => live && setState({ error }),
    );
    return () => {
      live = false;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);
  return state;
}

/** What each item gave back in the match against what it cost. */
export function ItemPayoffPanel({ matchId, slot }: { matchId: number; slot: number }) {
  const state = useOnce(() => api.itemPayoff(matchId, slot), `${matchId}-${slot}`);
  const rows = state?.value ?? [];
  const top = Math.max(...rows.map((r) => r.per_1k ?? 0), 1);
  return (
    <div className="group">
      <h3>{t("What your items gave back")}</h3>
      {state === null && <p className="muted">{t("Reading the match…")}</p>}
      {state?.error !== undefined && <ErrorNote error={state.error} />}
      {rows.length > 0 && (
        <>
          <table className="match-table payoff-table">
            <thead>
              <tr>
                <th className="left">{t("Item")}</th>
                <th>{t("Held")}</th>
                <th>{t("Damage")}</th>
                <th>{t("Healing")}</th>
                <th title={t("Enemy healing it prevented, and teammates' damage its resist reduction added (an estimate)")}>{t("Other")}</th>
                <th>{t("Per 1,000 souls")}</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r: ItemPayoff, i) => (
                <tr key={r.item + i} className={r.weak ? "weak" : ""}>
                  <td className="left">
                    <img src={r.icon} alt="" className="meta-icon" />
                    {r.name} <small className="muted">{fmt(r.cost)}</small>
                    {r.weak && <small className="bad"> · {t("gave little")}</small>}
                  </td>
                  <td>
                    {clock(r.bought_at)}–{r.sold_at !== null ? clock(r.sold_at) : t("end")}
                  </td>
                  <td>{r.damage > 0 ? fmt(r.damage, 0) : ""}</td>
                  <td>{r.healing > 0 ? fmt(r.healing, 0) : ""}</td>
                  <td>{r.heal_prevented + r.enabled > 0 ? fmt(r.heal_prevented + r.enabled, 0) : ""}</td>
                  <td>
                    {r.per_1k !== null ? (
                      <span className="payoff-bar">
                        <i style={{ width: `${(r.per_1k / top) * 100}%` }} />
                        {fmt(r.per_1k, 0)}
                      </span>
                    ) : (
                      <small className="muted">{t("stats only")}</small>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
          <p className="muted">
            {t(
              "From the game's own records of where damage and healing came from. Items that only raise stats (health, fire rate, spirit power…) leave no record of their own: their worth is in your other numbers, so they show as stats only. Some effects the game credits to another source. An item marked as giving little gave back less than a third of what your other measured items did per soul and minute held.",
            )}
          </p>
        </>
      )}
    </div>
  );
}

/** The player's upgrades in order beside the order top players of the hero go with most. */
export function OrderComparePanel({ matchId, slot }: { matchId: number; slot: number }) {
  const state = useOnce(() => api.matchOrder(matchId, slot), `${matchId}-${slot}`);
  const c: OrderCompare | null | undefined = state?.value;
  const step = (index: number, tier: number, names: string[], icons: string[], extra?: string) => (
    <span className="order-step" title={`${names[index]} ${tier}${extra ? ` · ${extra}` : ""}`}>
      <img src={icons[index]} alt={names[index]} />
      <b>{tier}</b>
    </span>
  );
  return (
    <div className="group">
      <h3>{t("Ability order")}</h3>
      {state === null && <p className="muted">{t("Reading the match…")}</p>}
      {state?.error !== undefined && <ErrorNote error={state.error} />}
      {c && (
        <>
          <div className="order-row">
            <b>{t("You")}</b>
            <div className="order-steps">
              {c.mine.map((s, i) => (
                <span key={i} className={s.same ? "" : "differs"}>
                  {step(s.index, s.tier, c.names, c.icons, clock(s.time))}
                  <small>{clock(s.time)}</small>
                </span>
              ))}
            </div>
          </div>
          {c.top.length > 0 ? (
            <>
              <div className="order-row">
                <b>{t("Top players")}</b>
                <div className="order-steps">
                  {c.top.map(([index, tier], i) => (
                    <span key={i}>{step(index, tier, c.names, c.icons)}</span>
                  ))}
                </div>
              </div>
              <p>
                {c.shared >= c.mine.length
                  ? t("Your order is the one top players go with most.")
                  : t("Your first {n} upgrades follow the order top players go with most ({m} matches, {w}% won); then yours goes its own way.", {
                      n: c.shared,
                      m: fmt(c.top_matches),
                      w: fmt(c.top_win_pct, 1),
                    })}{" "}
                {c.first_max_top !== null &&
                  c.first_max_mine !== c.first_max_top &&
                  t("They take {a} to its last tier first; you took {b}.", {
                    a: c.names[c.first_max_top],
                    b: c.first_max_mine !== null ? c.names[c.first_max_mine] : t("none"),
                  })}
              </p>
              <p className="muted">{t("Upgrades in a different place from the top order are dimmed. Hover one for the ability and when you took it.")}</p>
            </>
          ) : (
            <p className="muted">{t("No order of top players of this hero is known yet.")}</p>
          )}
        </>
      )}
    </div>
  );
}
