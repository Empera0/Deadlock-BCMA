import { useEffect, useState } from "react";
import { api, fmt, RiftComeback, Sheet } from "../api";

/** Map mechanics that depend on the state of the match: for now the Unstable Rift's comeback bonus. */
export default function MiscPanel({ sheet }: { sheet: Sheet }) {
  const [minute, setMinute] = useState(25);
  const [own, setOwn] = useState(130_000);
  const [enemy, setEnemy] = useState(131_000);
  const [rift, setRift] = useState<RiftComeback | null>(null);
  useEffect(() => {
    api.riftComeback(minute, own, enemy).then(setRift).catch(() => setRift(null));
  }, [minute, own, enemy]);
  // Resists from different sources stack by what each lets through.
  const stacked = (base: number, extra: number) => 100 * (1 - (1 - base / 100) * (1 - extra / 100));
  const stat = (key: string) => sheet.stats.find((s) => s.key === key)?.value ?? 0;
  const number = (value: number, set: (v: number) => void, step = 1) => (
    <input type="number" value={value} step={step} onChange={(e) => set(Math.max(0, Number(e.target.value) || 0))} />
  );

  return (
    <aside className="panel">
      <div className="group">
        <h3>Unstable Rift comeback</h3>
        <p className="muted">
          Inside the rift (35 m) the team behind in net worth gets bullet, spirit and debuff resist. The most it can be is 10% plus 1% per
          minute of the match, at most 40% from 30:00. How much of that the team gets depends on how far behind it is: nothing when even,
          rising in a straight line to all of it at 15% behind. The same scale shortens its capture time and adds rift troopers if it
          wins. Values from the 16 September 2026 patch.
        </p>
        <div className="fields misc-fields">
          <label>
            Minute {number(minute, setMinute)}
          </label>
          <label>
            Your team's souls {number(own, setOwn, 1000)}
          </label>
          <label>
            Enemy team's souls {number(enemy, setEnemy, 1000)}
          </label>
        </div>
      </div>

      {rift && (
        <div className="group">
          <h3>
            Result
            <small>{rift.behind_pct === 0 ? "teams even" : rift.own_team_trails ? "your team is behind" : "the enemy team is behind"}</small>
          </h3>
          <div className="tiles small">
            <div>
              <span>Behind by</span>
              <b>{fmt(rift.behind_pct, 2)}%</b>
            </div>
            <div>
              <span>Share of the full bonus</span>
              <b>{fmt(rift.scale * 100, 1)}%</b>
            </div>
            <div>
              <span>Most resist at minute {minute}</span>
              <b>{fmt(rift.max_resist, 0)}%</b>
            </div>
            <div>
              <span>Resist the trailing team gets</span>
              <b>{fmt(rift.resist, 2)}%</b>
            </div>
            <div>
              <span>Capture time: trailing / leading</span>
              <b>
                {fmt(rift.capture_trailing, 1)}s / {fmt(rift.capture_leading, 1)}s
              </b>
            </div>
            <div>
              <span>Rift troopers if the trailing team wins</span>
              <b>{fmt(rift.troopers_if_trailing_wins, 1)}</b>
            </div>
          </div>
          {rift.own_team_trails && rift.resist > 0 && (
            <p className="muted">
              Your build inside the rift: bullet resist {fmt(stat("bullet_resist"), 1)}% → {fmt(stacked(stat("bullet_resist"), rift.resist), 1)}%, spirit
              resist {fmt(stat("spirit_resist"), 1)}% → {fmt(stacked(stat("spirit_resist"), rift.resist), 1)}%, debuff resist {fmt(stat("debuff_resist"), 1)}% →{" "}
              {fmt(stacked(stat("debuff_resist"), rift.resist), 1)}% (stacked with the build's own, each letting through a share of what is left).
            </p>
          )}
          <p className="muted">
            The comeback bonus also raises the souls a trailing team gets for winning the rift, but how much is not published; the base
            bounty is split among the team.
          </p>
        </div>
      )}
    </aside>
  );
}
