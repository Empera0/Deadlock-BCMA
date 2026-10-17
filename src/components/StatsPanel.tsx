import { useState } from "react";
import { Build, fmt, Group, Hero, Ranked, ScaledProp, Sheet, Stat } from "../api";
import InvestBars from "./InvestBars";
import Advisor from "./Advisor";

interface Props {
  heroes: Hero[];
  hero: Hero;
  build: Build;
  sheet: Sheet;
  ranked: Ranked[];
  metric: string;
  conditional: boolean;
  onMetric: (metric: string) => void;
  onConditional: (on: boolean) => void;
  onChange: (patch: Partial<Build>) => void;
  onBuy: (className: string) => void;
}

const GROUPS: [Group, string][] = [
  ["weapon", "Weapon"],
  ["vitality", "Vitality"],
  ["spirit", "Spirit"],
  ["target", "Versus target"],
  ["other", "Other effects"],
];

const METRICS: [string, string][] = [
  ["dps_sustained", "DPS (with reload)"],
  ["dps_burst", "DPS (burst)"],
  ["target_dps", "DPS vs target"],
  ["spirit_power", "Spirit power"],
  ["max_health", "Max health"],
  ["ehp_bullet", "Effective health (bullets)"],
  ["ehp_spirit", "Effective health (spirit)"],
];

function StatRow({ stat }: { stat: Stat }) {
  const [open, setOpen] = useState(false);
  return (
    <div className={"stat" + (open ? " open" : "")}>
      <button className="stat-head" onClick={() => setOpen(!open)}>
        <span>
          {stat.label}
        </span>
        <b>
          {fmt(stat.value)}
          <small>{stat.unit}</small>
        </b>
      </button>
      {open && (
        <div className="stat-body">
          <code>{stat.formula}</code>
          {stat.terms.map((t, i) => (
            <div key={i} className="term">
              <span>{t.label}</span>
              <span>
                {fmt(t.value, 3)}
                {t.unit}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

function PropRow({ prop }: { prop: ScaledProp }) {
  return (
    <div className="prop" title={prop.note ?? prop.formula}>
      <span>{prop.label}</span>
      <span>
        <b>
          {fmt(prop.value)}
          {prop.unit}
        </b>
        {prop.vs_target !== null && <small> → vs target {fmt(prop.vs_target)}</small>}
        {prop.formula.includes("×") && <code>{prop.formula}</code>}
      </span>
    </div>
  );
}

function NumberField(props: { label: string; value: number; onChange: (v: number) => void }) {
  return (
    <label className="field">
      {props.label}
      <input
        type="number"
        value={props.value}
        onChange={(e) => props.onChange(Number(e.target.value) || 0)}
      />
    </label>
  );
}

export default function StatsPanel(p: Props) {
  const { build, sheet } = p;
  const setTier = (index: number, tier: number) => {
    const tiers = sheet.abilities.map((a, i) => (i === index ? tier : a.tier));
    p.onChange({ ability_tiers: tiers });
  };
  const visible = (s: Stat) => s.group === "target" || s.value !== 0 || s.terms.length > 0;

  return (
    <aside className="panel">
      <InvestBars hero={p.hero} sheet={sheet} />

      {GROUPS.map(([group, title]) => {
        const stats = sheet.stats.filter((s) => s.group === group && visible(s));
        if (stats.length === 0) return null;
        return (
          <div key={group} className={"group " + group}>
            <h3>{title}</h3>
            {group === "target" && (
              <div className="fields">
                <NumberField
                  label="Health"
                  value={build.target.health}
                  onChange={(v) => p.onChange({ target: { ...build.target, health: v } })}
                />
                <NumberField
                  label="Bullet resist %"
                  value={build.target.bullet_resist}
                  onChange={(v) => p.onChange({ target: { ...build.target, bullet_resist: v } })}
                />
                <NumberField
                  label="Spirit resist %"
                  value={build.target.spirit_resist}
                  onChange={(v) => p.onChange({ target: { ...build.target, spirit_resist: v } })}
                />
              </div>
            )}
            {stats.map((s) => (
              <StatRow key={s.key} stat={s} />
            ))}
          </div>
        );
      })}

      <div className="group">
        <h3>
          Abilities
          <small className={sheet.ability_points_used > sheet.ability_points ? "over" : ""}>
            {sheet.ability_points_used}/{sheet.ability_points} ability points
          </small>
        </h3>
        {sheet.abilities.map((a, i) => (
          <div key={a.class_name} className="ability">
            <div className="ability-head">
              <img src={a.icon} alt="" />
              <strong>{a.name}</strong>
              {a.imbued.length > 0 && <small>imbued: {a.imbued.join(", ")}</small>}
              {a.slot_class === "ability_magician_copyult" && (
                <select
                  className="copy-ult"
                  title="Enemy hero whose ultimate is copied"
                  value={build.copied_ult ?? ""}
                  onChange={(e) => p.onChange({ copied_ult: e.target.value || null })}
                >
                  <option value="">Copy ultimate…</option>
                  {p.heroes
                    .filter((h) => h.class_name !== p.hero.class_name)
                    .map((h) => (
                      <option key={h.class_name} value={h.class_name}>
                        {h.name}
                      </option>
                    ))}
                </select>
              )}
              <div className="tiers">
                {Array.from({ length: a.max_tier + 1 }, (_, t) => (
                  <button key={t} className={t === a.tier ? "on" : ""} onClick={() => setTier(i, t)}>
                    {t}
                  </button>
                ))}
              </div>
            </div>
            {(a.quip || a.description) && (
              <details className="ability-text">
                <summary>{a.quip || a.description}</summary>
                {a.quip && a.description && <p>{a.description}</p>}
                {a.upgrade_notes.map((note, tier) =>
                  note ? (
                    <p key={tier} className={tier < a.tier ? "taken" : ""}>
                      <b>Upgrade {tier + 1}</b> {note}
                    </p>
                  ) : null,
                )}
              </details>
            )}
            {a.props.filter((prop) => prop.label).map((prop) => (
              <PropRow key={prop.key} prop={prop} />
            ))}
          </div>
        ))}
      </div>

      {sheet.item_effects.length > 0 && (
        <div className="group">
          <h3>Item effects</h3>
          {sheet.item_effects.map((e) => (
            <div key={e.item} className="ability">
              <div className="ability-head">
                <strong>{e.name}</strong>
              </div>
              {e.props.map((prop) => (
                <PropRow key={prop.key} prop={prop} />
              ))}
            </div>
          ))}
        </div>
      )}

      <div className="group">
        <h3>Next item</h3>
        <div className="fields">
          <label className="field wide">
            Target stat
            <select value={p.metric} onChange={(e) => p.onMetric(e.target.value)}>
              {METRICS.map(([key, label]) => (
                <option key={key} value={key}>
                  {label}
                </option>
              ))}
            </select>
          </label>
          <label className="check">
            <input type="checkbox" checked={p.conditional} onChange={(e) => p.onConditional(e.target.checked)} />
            Include conditional effects
          </label>
        </div>
        {p.ranked.length === 0 && <p className="muted">No item increases this stat.</p>}
        {p.ranked.slice(0, 12).map((r) => (
          <button key={r.item} className={"rank " + r.category} onClick={() => p.onBuy(r.item)}>
            <span>{r.name}</span>
            <span className="muted">{fmt(r.paid)}</span>
            <span>
              +{fmt(r.delta)} <small>({fmt(r.delta_pct, 1)}%)</small>
            </span>
            <b title="Gain per 1,000 souls">{fmt(r.per_1k)}/1k</b>
          </button>
        ))}
      </div>

      <Advisor build={build} onChange={p.onChange} />
    </aside>
  );
}
