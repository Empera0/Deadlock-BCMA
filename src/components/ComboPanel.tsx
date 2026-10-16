import { Action, Combo, ComboResult, fmt, Hero, Item, Sheet } from "../api";
import ComboTarget from "./ComboTarget";

interface Props {
  heroes: Hero[];
  items: Item[];
  sheet: Sheet;
  combo: Combo;
  result: ComboResult | null;
  onChange: (combo: Combo) => void;
}

function label(action: Action, sheet: Sheet, result: ComboResult | null): string {
  switch (action.kind) {
    case "shoot":
      return "Shoot";
    case "wait":
      return "Wait";
    case "melee":
      return action.heavy ? "Heavy melee" : "Light melee";
    case "ability":
      return sheet.abilities[action.index]?.name ?? `Ability ${action.index + 1}`;
    case "item":
      return result?.usable_items.find((i) => i.item === action.item && !i.condition)?.name ?? "Item (not owned)";
    case "condition": {
      const name = result?.usable_items.find((i) => i.item === action.item && i.condition)?.name;
      return name ? `${name} (condition on)` : "Condition (not owned)";
    }
  }
}

export default function ComboPanel({ heroes, items, sheet, combo, result, onChange }: Props) {
  const add = (action: Action) => onChange({ ...combo, actions: [...combo.actions, action] });
  const update = (index: number, action: Action) =>
    onChange({ ...combo, actions: combo.actions.map((a, i) => (i === index ? action : a)) });
  const remove = (index: number) => onChange({ ...combo, actions: combo.actions.filter((_, i) => i !== index) });
  const move = (index: number, dir: -1 | 1) => {
    const next = [...combo.actions];
    const j = index + dir;
    if (j < 0 || j >= next.length) return;
    [next[index], next[j]] = [next[j], next[index]];
    onChange({ ...combo, actions: next });
  };

  const top = result ? Math.max(...result.sources.map((s) => s.damage), 1) : 1;
  const healthLeft = result ? Math.max(0, result.target_health - result.total) : 0;
  const topHeal = result ? Math.max(...result.heal_sources.map((h) => h.amount), 1) : 1;
  const ownHealth = combo.own_health ?? 50;

  return (
    <aside className="panel">
      <ComboTarget heroes={heroes} items={items} combo={combo} result={result} onChange={onChange} />

      <div className="group">
        <h3>Add action</h3>
        <div className="combo-add">
          {sheet.abilities.map((a, index) => (
            <button
              key={a.class_name}
              className="spirit"
              disabled={result?.abilities[index]?.castable === false}
              title={result?.abilities[index]?.lines.join("\n")}
              onClick={() => add({ kind: "ability", index, duration: null, hits: null })}
            >
              <img src={a.icon} alt="" />
              {a.name}
            </button>
          ))}
          <button className="weapon" onClick={() => add({ kind: "shoot", seconds: 2 })}>
            Shoot
          </button>
          <button className="weapon" onClick={() => add({ kind: "melee", heavy: false })}>
            Light melee
          </button>
          <button className="weapon" onClick={() => add({ kind: "melee", heavy: true })}>
            Heavy melee
          </button>
          <button onClick={() => add({ kind: "wait", seconds: 1 })}>Wait</button>
          {result?.usable_items.map((i) => (
            <button
              key={i.item + i.condition}
              className={i.condition ? "condition" : "vitality"}
              title={i.description}
              onClick={() =>
                add(i.condition ? { kind: "condition", item: i.item, count: i.max_count !== null ? 1 : null } : { kind: "item", item: i.item })
              }
            >
              {i.name}
              {i.condition && <small>condition</small>}
            </button>
          ))}
        </div>
        <label className="field inline">
          Headshot rate %
          <input
            type="number"
            min={0}
            max={100}
            value={combo.headshot_rate}
            onChange={(e) => onChange({ ...combo, headshot_rate: Math.min(100, Math.max(0, Number(e.target.value) || 0)) })}
          />
        </label>
        <label className="field inline" title="Your own health when the combo starts. Heals based on missing health depend on it.">
          Your health %
          <input
            type="number"
            min={0}
            max={100}
            value={ownHealth}
            onChange={(e) => onChange({ ...combo, own_health: Math.min(100, Math.max(0, Number(e.target.value) || 0)) })}
          />
        </label>
      </div>

      <div className="group">
        <h3>
          Sequence
          {combo.actions.length > 0 && (
            <button className="ghost small" onClick={() => onChange({ ...combo, actions: [] })}>
              Clear
            </button>
          )}
        </h3>
        {combo.actions.length === 0 && <p className="muted">Add actions above to build a combo.</p>}
        {combo.actions.map((action, i) => (
          <div key={i} className="combo-step">
            <span className="chip-index">{i + 1}</span>
            <strong>{label(action, sheet, result)}</strong>
            {(action.kind === "shoot" || action.kind === "wait") && (
              <label>
                <input
                  type="number"
                  min={0}
                  step={0.5}
                  value={action.seconds}
                  onChange={(e) => update(i, { ...action, seconds: Math.max(0, Number(e.target.value) || 0) })}
                />
                s
              </label>
            )}
            {action.kind === "condition" &&
              (() => {
                const info = result?.usable_items.find((u) => u.condition && u.item === action.item);
                if (!info || info.max_count === null) return null;
                return (
                  <label title={info.description}>
                    <input
                      type="number"
                      min={0}
                      max={info.max_count}
                      step={1}
                      value={action.count ?? 1}
                      onChange={(e) =>
                        update(i, { ...action, count: Math.min(info.max_count ?? 0, Math.max(0, Math.round(Number(e.target.value) || 0))) })
                      }
                    />
                    {info.unit}
                  </label>
                );
              })()}
            {action.kind === "ability" && (
              <>
                <label title="How long the ability stays on the target. Empty uses the ability's own duration.">
                  <input
                    type="number"
                    min={0}
                    step={0.5}
                    placeholder="auto"
                    value={action.duration ?? ""}
                    onChange={(e) =>
                      update(i, { ...action, duration: e.target.value === "" ? null : Math.max(0, Number(e.target.value) || 0) })
                    }
                  />
                  s
                </label>
                <label title="How many times a multi-hit ability connects (bites, detonations, rockets). Empty uses the ability's own count.">
                  <input
                    type="number"
                    min={0}
                    step={1}
                    placeholder="auto"
                    value={action.hits ?? ""}
                    onChange={(e) =>
                      update(i, { ...action, hits: e.target.value === "" ? null : Math.max(0, Math.round(Number(e.target.value) || 0)) })
                    }
                  />
                  hits
                </label>
              </>
            )}
            <span className="spacer" />
            <button className="chip-btn" title="Move earlier" onClick={() => move(i, -1)}>
              ‹
            </button>
            <button className="chip-btn" title="Move later" onClick={() => move(i, 1)}>
              ›
            </button>
            <button className="chip-btn" title="Remove" onClick={() => remove(i)}>
              ×
            </button>
          </div>
        ))}
      </div>

      {result && combo.actions.length > 0 && (
        <>
          <div className="group">
            <h3>Result</h3>
            <div className="tiles">
              <div>
                <span>Total damage</span>
                <b>{fmt(result.total, 0)}</b>
              </div>
              <div>
                <span>Duration</span>
                <b>{fmt(result.full_duration, 1)}s</b>
              </div>
              <div>
                <span>DPS</span>
                <b>{fmt(result.dps, 0)}</b>
              </div>
              <div className={result.kill_time !== null ? "kill" : ""}>
                <span>Kill time</span>
                <b>{result.kill_time !== null ? `${fmt(result.kill_time, 1)}s` : "survives"}</b>
              </div>
            </div>
            <div className="health-bar" title={`${fmt(healthLeft, 0)} of ${fmt(result.target_health, 0)} health left`}>
              <div style={{ width: `${Math.min(100, (result.total / Math.max(result.target_health, 1)) * 100)}%` }} />
            </div>
            <div className="muted">
              {result.kill_time !== null
                ? `Target (${fmt(result.target_health, 0)} health) dies ${fmt(result.kill_time, 1)}s in · ${fmt(result.total - result.target_health, 0)} overkill`
                : `Target keeps ${fmt(healthLeft, 0)} of ${fmt(result.target_health, 0)} health`}
            </div>
          </div>

          <div className="group">
            <h3>Damage by source</h3>
            {result.sources.map((s) => (
              <div key={s.source + s.kind} className={"source " + s.kind}>
                <div className="source-head">
                  <span>
                    {s.source} <small>{s.kind}</small>
                  </span>
                  <b>
                    {fmt(s.damage, 0)} <small>{fmt((s.damage / Math.max(result.total, 1)) * 100, 1)}%</small>
                  </b>
                </div>
                <div className="source-bar">
                  <div style={{ width: `${(s.damage / top) * 100}%` }} />
                </div>
              </div>
            ))}
          </div>

          {result.healing > 0 && (
            <div className="group">
              <h3>Healing</h3>
              <div className="tiles">
                <div>
                  <span>Total healing</span>
                  <b className="heal">{fmt(result.healing, 0)}</b>
                </div>
                <div>
                  <span>Of your max health</span>
                  <b>{fmt((result.healing / Math.max(result.own_max_health, 1)) * 100, 1)}%</b>
                </div>
                <div>
                  <span>Your health</span>
                  <b>
                    {fmt(result.own_health_start, 0)} → {fmt(result.own_health_end, 0)}
                  </b>
                </div>
              </div>
              <div className="muted">
                Of {fmt(result.own_max_health, 0)} max health. Incoming damage is not simulated; healing past max health is counted in the total
                but not in your health.
              </div>
              {result.heal_sources.map((h) => (
                <div key={h.source} className="source heal">
                  <div className="source-head">
                    <span>{h.source}</span>
                    <b>
                      {fmt(h.amount, 0)} <small>{fmt((h.amount / Math.max(result.healing, 1)) * 100, 1)}%</small>
                    </b>
                  </div>
                  <div className="source-bar">
                    <div style={{ width: `${(h.amount / topHeal) * 100}%` }} />
                  </div>
                </div>
              ))}
            </div>
          )}

          <div className="group">
            <h3>Timeline</h3>
            {result.events.map((e, i) => (
              <div key={i} className="event">
                <span className="muted">{fmt(e.time, 2)}s</span>
                <span>
                  {e.source} <small>{e.detail}</small>
                </span>
                <b className={e.heal !== null ? "heal" : ""}>
                  {e.heal !== null ? `+${fmt(e.heal, 0)}` : e.damage !== null ? fmt(e.damage, 0) : ""}
                </b>
              </div>
            ))}
          </div>
        </>
      )}

      {result && (
        <div className="group">
          <h3>How abilities are simulated</h3>
          {result.abilities.map((a) => (
            <div key={a.name} className="ability">
              <div className="ability-head">
                <strong>{a.name}</strong>
                {!a.castable && <small>passive</small>}
              </div>
              {a.lines.map((line) => (
                <div key={line} className="passive">
                  {line}
                </div>
              ))}
            </div>
          ))}
        </div>
      )}

      {result && (result.passives.length > 0 || result.unsupported.length > 0) && (
        <div className="group">
          <h3>Item effects in the simulation</h3>
          {result.passives.map((p) => (
            <div key={p} className="passive">
              {p}
            </div>
          ))}
          {result.unsupported.length > 0 && (
            <div className="muted">Not simulated yet: {result.unsupported.join(", ")}</div>
          )}
        </div>
      )}
    </aside>
  );
}
