import { Action, Combo, ComboResult, fmt, Hero, Item, MapBuffs, Sheet, Statues } from "../api";
import ComboTarget from "./ComboTarget";
import { t } from "../i18n";

interface Props {
  heroes: Hero[];
  items: Item[];
  sheet: Sheet;
  combo: Combo;
  result: ComboResult | null;
  onChange: (combo: Combo) => void;
  map: MapBuffs;
  onMap: (map: MapBuffs) => void;
}

/** Statue buffs; translated where shown. */
const STATUES: [keyof Statues, string][] = [
  ["weapon", "Weapon damage"],
  ["fire_rate", "Fire rate"],
  ["ammo", "Ammo"],
  ["spirit", "Spirit power"],
  ["cooldown", "Cooldown"],
  ["health", "Health"],
  ["bullet_resist", "Bullet resist"],
  ["spirit_resist", "Spirit resist"],
];

/** Powerups and permanent statue buffs the hero carries; they count in the stats and the combo. */
function MapBuffsGroup({ map, onMap }: { map: MapBuffs; onMap: (map: MapBuffs) => void }) {
  const total = Object.values(map.statues).reduce((a, b) => a + b, 0);
  return (
    <details className="group map-buffs" open={map.casting || map.gun || total > 0}>
      <summary>
        <h3>{t("Your map buffs")}</h3>
      </summary>
      <label className="field inline" title={t("Powerups grow from minute 5 to 40; statue buffs are stronger from minutes 10 and 30")}>
        {t("Minute")}
        <input type="number" min={0} value={map.minute} onChange={(e) => onMap({ ...map, minute: Math.max(0, Number(e.target.value) || 0) })} />
      </label>
      <div className="zone">
        <label title={t("Spirit power and cooldown reduction for 160s")}>
          <input type="checkbox" checked={map.casting} onChange={(e) => onMap({ ...map, casting: e.target.checked })} />
          {t("Casting powerup")}
        </label>
        <label title={t("Fire rate and ammo for 160s")}>
          <input type="checkbox" checked={map.gun} onChange={(e) => onMap({ ...map, gun: e.target.checked })} />
          {t("Gun powerup")}
        </label>
      </div>
      <div className="statues">
        {STATUES.map(([key, label]) => (
          <label key={key}>
            {t(label)}
            <input
              type="number"
              min={0}
              value={map.statues[key]}
              onChange={(e) => onMap({ ...map, statues: { ...map.statues, [key]: Math.max(0, Math.round(Number(e.target.value) || 0)) } })}
            />
          </label>
        ))}
      </div>
      <p className="muted">{t("Statue buffs collected, counted at their level for the minute.")}</p>
    </details>
  );
}

function label(action: Action, sheet: Sheet, result: ComboResult | null): string {
  switch (action.kind) {
    case "shoot":
      return t("Shoot");
    case "wait":
      return t("Wait");
    case "parry":
      return t("Parry");
    case "enemy_item":
      return t("Enemy: {item}", { item: result?.enemy_items.find((i) => i.item === action.item)?.name ?? t("item (not owned)") });
    case "melee":
      return action.heavy ? t("Heavy melee") : t("Light melee");
    case "ability":
      return sheet.abilities[action.index]?.name ?? t("Ability {n}", { n: action.index + 1 });
    case "item":
      return result?.usable_items.find((i) => i.item === action.item && !i.condition)?.name ?? t("Item (not owned)");
    case "condition": {
      const name = result?.usable_items.find((i) => i.item === action.item && i.condition)?.name;
      return name ? t("{name} (condition on)", { name }) : t("Condition (not owned)");
    }
  }
}

export default function ComboPanel({ heroes, items, sheet, combo, result, onChange, map, onMap }: Props) {
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
  const toHealth = result ? result.total - result.absorbed : 0;
  const healthLeft = result ? Math.max(0, result.target_health - toHealth) : 0;
  const topHeal = result ? Math.max(...result.heal_sources.map((h) => h.amount), 1) : 1;
  const ownHealth = combo.own_health ?? 50;

  return (
    <aside className="panel">
      <ComboTarget heroes={heroes} items={items} combo={combo} result={result} onChange={onChange} />
      <MapBuffsGroup map={map} onMap={onMap} />

      <div className="group">
        <h3>{t("Add action")}</h3>
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
            {t("Shoot")}
          </button>
          <button className="weapon" onClick={() => add({ kind: "melee", heavy: false })}>
            {t("Light melee")}
          </button>
          <button className="weapon" onClick={() => add({ kind: "melee", heavy: true })}>
            {t("Heavy melee")}
          </button>
          <button onClick={() => add({ kind: "wait", seconds: 1 })}>{t("Wait")}</button>
          <button title={t("You parry the target's melee: stunned for 2.75s and +25% damage taken")} onClick={() => add({ kind: "parry" })}>
            {t("Parry")}
          </button>
          {result?.enemy_items.map((i) => (
            <button key={"enemy" + i.item} className="enemy-action" title={i.description} onClick={() => add({ kind: "enemy_item", item: i.item })}>
              {i.name}
              <small>{t("enemy")}</small>
            </button>
          ))}
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
              {i.condition && <small>{t("condition")}</small>}
            </button>
          ))}
        </div>
        <label className="field inline">
          {t("Headshot rate %")}
          <input
            type="number"
            min={0}
            max={100}
            value={combo.headshot_rate}
            onChange={(e) => onChange({ ...combo, headshot_rate: Math.min(100, Math.max(0, Number(e.target.value) || 0)) })}
          />
        </label>
        <label className="field inline" title={t("Your own health when the combo starts. Heals based on missing health depend on it.")}>
          {t("Your health %")}
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
          {t("Sequence")}
          {combo.actions.length > 0 && (
            <button className="ghost small" onClick={() => onChange({ ...combo, actions: [] })}>
              {t("Clear")}
            </button>
          )}
        </h3>
        {combo.actions.length === 0 && <p className="muted">{t("Add actions above to build a combo.")}</p>}
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
                {t("s")}
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
                <label title={t("How long the ability stays on the target. Empty uses the ability's own duration.")}>
                  <input
                    type="number"
                    min={0}
                    step={0.5}
                    placeholder={t("auto")}
                    value={action.duration ?? ""}
                    onChange={(e) =>
                      update(i, { ...action, duration: e.target.value === "" ? null : Math.max(0, Number(e.target.value) || 0) })
                    }
                  />
                  {t("s")}
                </label>
                <label title={t("How many times a multi-hit ability connects (bites, detonations, rockets). Empty uses the ability's own count.")}>
                  <input
                    type="number"
                    min={0}
                    step={1}
                    placeholder={t("auto")}
                    value={action.hits ?? ""}
                    onChange={(e) =>
                      update(i, { ...action, hits: e.target.value === "" ? null : Math.max(0, Math.round(Number(e.target.value) || 0)) })
                    }
                  />
                  {t("hits")}
                </label>
              </>
            )}
            <span className="spacer" />
            <button className="chip-btn" title={t("Move earlier")} onClick={() => move(i, -1)}>
              ‹
            </button>
            <button className="chip-btn" title={t("Move later")} onClick={() => move(i, 1)}>
              ›
            </button>
            <button className="chip-btn" title={t("Remove")} onClick={() => remove(i)}>
              ×
            </button>
          </div>
        ))}
      </div>

      {result && combo.actions.length > 0 && (
        <>
          <div className="group">
            <h3>{t("Result")}</h3>
            <div className="tiles">
              <div>
                <span>{t("Total damage")}</span>
                <b>{fmt(result.total, 0)}</b>
              </div>
              <div>
                <span>{t("Duration")}</span>
                <b>{t("{n}s", { n: fmt(result.full_duration, 1) })}</b>
              </div>
              <div>
                <span>{t("DPS")}</span>
                <b>{fmt(result.dps, 0)}</b>
              </div>
              <div className={result.kill_time !== null ? "kill" : ""}>
                <span>{t("Kill time")}</span>
                <b>{result.kill_time !== null ? t("{n}s", { n: fmt(result.kill_time, 1) }) : t("survives")}</b>
              </div>
            </div>
            <div className="health-bar" title={t("{a} of {b} health left", { a: fmt(healthLeft, 0), b: fmt(result.target_health, 0) })}>
              <div style={{ width: `${Math.min(100, (toHealth / Math.max(result.target_health, 1)) * 100)}%` }} />
            </div>
            <div className="muted">
              {result.kill_time !== null
                ? t("Target ({h} health) dies {n}s in · {o} overkill", { h: fmt(result.target_health, 0), n: fmt(result.kill_time, 1), o: fmt(toHealth - result.target_health, 0) })
                : t("Target keeps {a} of {b} health", { a: fmt(healthLeft, 0), b: fmt(result.target_health, 0) })}
              {result.absorbed > 0 && ` · ${t("{n} taken by barriers", { n: fmt(result.absorbed, 0) })}`}
            </div>
          </div>

          <div className="group">
            <h3>{t("Damage by source")}</h3>
            {result.sources.map((s) => (
              <div key={s.source + s.kind} className={"source " + s.kind}>
                <div className="source-head">
                  <span>
                    {s.source} <small>{t(s.kind)}</small>
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
              <h3>{t("Healing")}</h3>
              <div className="tiles">
                <div>
                  <span>{t("Total healing")}</span>
                  <b className="heal">{fmt(result.healing, 0)}</b>
                </div>
                <div>
                  <span>{t("Of your max health")}</span>
                  <b>{fmt((result.healing / Math.max(result.own_max_health, 1)) * 100, 1)}%</b>
                </div>
                <div>
                  <span>{t("Your health")}</span>
                  <b>
                    {fmt(result.own_health_start, 0)} → {fmt(result.own_health_end, 0)}
                  </b>
                </div>
              </div>
              <div className="muted">
                {t("Of {n} max health. Incoming damage is not simulated; healing past max health is counted in the total but not in your health.", {
                  n: fmt(result.own_max_health, 0),
                })}
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
            <h3>{t("Timeline")}</h3>
            {result.events.map((e, i) => (
              <div key={i} className="event">
                <span className="muted">{t("{n}s", { n: fmt(e.time, 2) })}</span>
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
          <h3>{t("How abilities are simulated")}</h3>
          {result.abilities.map((a) => (
            <div key={a.name} className="ability">
              <div className="ability-head">
                <strong>{a.name}</strong>
                {!a.castable && <small>{t("passive")}</small>}
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
          <h3>{t("Item effects in the simulation")}</h3>
          {result.passives.map((p) => (
            <div key={p} className="passive">
              {p}
            </div>
          ))}
          {result.unsupported.length > 0 && (
            <div className="muted">{t("Not simulated yet: {items}", { items: result.unsupported.join(", ") })}</div>
          )}
        </div>
      )}
    </aside>
  );
}
