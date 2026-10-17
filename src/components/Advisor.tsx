import { useEffect, useState } from "react";
import { Advice, AdviceMove, api, Build, fmt } from "../api";

const STYLES: [string, string][] = [
  ["hybrid", "Full kit"],
  ["gun", "Gun carry"],
  ["spirit", "Spirit burst"],
  ["melee", "Melee brawler"],
  ["frontline", "Frontline"],
];

const pct = (v: number) => `+${fmt(v * 100, 1)}%`;

function MoveRow({ move, onApply }: { move: AdviceMove; onApply: () => void }) {
  return (
    <button className="advice-move" onClick={onApply} title="Apply to the build">
      {move.sell_icon && (
        <>
          <img src={move.sell_icon} alt="" className="sold" />
          <span className="muted">{move.sell_name}</span>
          <span className="muted">→</span>
        </>
      )}
      <img src={move.buy_icon} alt="" />
      <span>{move.buy_name}</span>
      <span className="muted">
        {move.sell ? `${fmt(move.net_cost)} after ${fmt(move.refund)} back` : fmt(move.paid)}
      </span>
      <b>{pct(move.gain)}</b>
    </button>
  );
}

/** What to do with the next souls: buy, upgrade an owned item, or sell one for something better. */
export default function Advisor({ build, onChange }: { build: Build; onChange: (patch: Partial<Build>) => void }) {
  const [style, setStyle] = useState("hybrid");
  const [advice, setAdvice] = useState<Advice | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  // A changed build makes the old advice stale.
  useEffect(() => setAdvice(null), [build, style]);

  const run = () => {
    setBusy(true);
    setError("");
    api
      .adviseBuild(build, style)
      .then(setAdvice)
      .catch((e) => setError(String(e)))
      .finally(() => setBusy(false));
  };
  const apply = (m: AdviceMove) => {
    const kept = build.purchases.slice(0, build.upto ?? build.purchases.length).filter((c) => !m.removes.includes(c));
    const imbues = { ...build.imbues };
    if (m.sell) delete imbues[m.sell];
    if (m.imbue !== null) imbues[m.buy] = m.imbue;
    onChange({ purchases: [...kept, m.buy], upto: null, imbues, active: build.active.filter((c) => c !== m.sell) });
  };
  const section = (title: string, moves: AdviceMove[], empty: string) => (
    <>
      <h4>{title}</h4>
      {moves.length === 0 ? <p className="muted">{empty}</p> : moves.map((m) => <MoveRow key={`${m.sell}-${m.buy}`} move={m} onApply={() => apply(m)} />)}
    </>
  );

  return (
    <div className="group advisor">
      <h3>Buy, upgrade or sell</h3>
      <div className="fields">
        <label className="field wide">
          Measured for
          <select value={style} onChange={(e) => setStyle(e.target.value)}>
            {STYLES.map(([key, label]) => (
              <option key={key} value={key}>
                {label}
              </option>
            ))}
          </select>
        </label>
        <button disabled={busy} onClick={run}>
          {busy ? "Working…" : advice ? "Again" : "Advise"}
        </button>
      </div>
      {error && <div className="error">{error}</div>}
      {advice && (
        <>
          <p className="muted">
            {fmt(advice.free_souls)} souls free · {advice.slots_used}/{advice.slot_limit} slots. Gains are on the playstyle's fight. Click a
            move to apply it; selling returns half the price.
          </p>
          {advice.slots_used < advice.slot_limit && section("Buy", advice.buys, "Nothing affordable adds to the fight.")}
          {section("Upgrade an owned item", advice.upgrades, "No affordable upgrade adds to the fight.")}
          {section("Sell one for another", advice.swaps, "No swap beats the build as it is.")}
          {advice.worth.length > 0 && (
            <>
              <h4>What each item does for the fight</h4>
              <div className="advice-worth">
                {advice.worth.map((w) => (
                  <span key={w.item} title={`${fmt(w.cost)} souls: the fight loses ${fmt(w.loss * 100, 1)}% without it`}>
                    <img src={w.icon} alt="" />
                    {w.name} <b>{fmt(w.loss * 100, 1)}%</b>
                  </span>
                ))}
              </div>
            </>
          )}
        </>
      )}
    </div>
  );
}
