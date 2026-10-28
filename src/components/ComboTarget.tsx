import { useEffect, useMemo, useState } from "react";
import { api, Build, Combo, ComboResult, EMPTY_MAP, fmt, Hero, Item, Zone } from "../api";
import HeroPicker from "./HeroPicker";
import Shop from "./Shop";
import { t } from "../i18n";

interface Props {
  heroes: Hero[];
  items: Item[];
  combo: Combo;
  result: ComboResult | null;
  onChange: (combo: Combo) => void;
}

const enemyBuild = (hero: string): Build => ({
  hero,
  purchases: [],
  upto: null,
  souls: 10_000,
  active: [],
  ability_tiers: [0, 0, 0, 0],
  imbues: {},
  map: EMPTY_MAP,
  copied_ult: null,
  target: { health: 2000, bullet_resist: 0, spirit_resist: 0 },
});

/** The combo's target: an enemy hero with items, and the map objects around the fight. */
export default function ComboTarget({ heroes, items, combo, result, onChange }: Props) {
  const [picking, setPicking] = useState(false);
  const [shopping, setShopping] = useState(false);
  const [owned, setOwned] = useState<Set<string>>(new Set());
  const enemy = combo.enemy;
  const hero = enemy ? heroes.find((h) => h.class_name === enemy.hero) : undefined;
  const byClass = useMemo(() => new Map(items.map((i) => [i.class_name, i])), [items]);

  useEffect(() => {
    if (!enemy) return;
    let stale = false;
    api
      .compute(enemy)
      .then((s) => !stale && setOwned(new Set(s.owned)))
      .catch(() => !stale && setOwned(new Set(enemy.purchases)));
    return () => {
      stale = true;
    };
  }, [enemy]);

  const setEnemy = (e: Build | null) => onChange({ ...combo, enemy: e });
  const setZone = (z: Partial<Zone>) => onChange({ ...combo, zone: { ...combo.zone, ...z } });
  const toggleItem = (className: string) => {
    if (!enemy) return;
    const purchases = enemy.purchases.includes(className)
      ? enemy.purchases.filter((p) => p !== className)
      : [...enemy.purchases, className];
    setEnemy({ ...enemy, purchases });
  };
  const spent = enemy?.purchases.reduce((sum, c) => sum + (byClass.get(c)?.cost ?? 0), 0) ?? 0;
  const number = (value: number, set: (v: number) => void, step = 1) => (
    <input type="number" min={0} step={step} value={value} onChange={(e) => set(Math.max(0, Number(e.target.value) || 0))} />
  );
  const target = result?.target;
  const zone = combo.zone;

  return (
    <div className="group">
      <h3>
        {t("Target")}
        {enemy && (
          <button className="ghost small" onClick={() => setEnemy(null)} title={t("Use the health and resists set in the Stats tab")}>
            {t("Manual target")}
          </button>
        )}
      </h3>
      <div className="enemy-row">
        <button className="enemy-hero" onClick={() => setPicking(true)}>
          {hero ? <img src={hero.icon} alt="" /> : <span className="enemy-empty">?</span>}
          <span>{hero ? hero.name : t("Pick an enemy hero")}</span>
        </button>
        {enemy && (
          <label className="field inline" title={t("The enemy's total souls: its level and boons follow from them")}>
            {t("Souls")}
            {number(Math.max(enemy.souls ?? 0, spent), (v) => setEnemy({ ...enemy, souls: Math.round(v) }), 500)}
          </label>
        )}
      </div>
      {enemy && (
        <div className="enemy-items">
          {enemy.purchases.map((c) => {
            const item = byClass.get(c);
            if (!item) return null;
            return (
              <button key={c} className={"enemy-item " + item.category} title={t("{item}: click to remove", { item: item.name })} onClick={() => toggleItem(c)}>
                <img src={item.icon} alt="" />
                <span>{item.name}</span>
              </button>
            );
          })}
          <button className="ghost small" onClick={() => setShopping(true)}>
            {t("+ Items")}
          </button>
        </div>
      )}

      <div className="zone">
        <label>
          <input type="checkbox" checked={zone.rift} onChange={(e) => setZone({ rift: e.target.checked })} />
          {t("Inside the Unstable Rift")}
        </label>
        {zone.rift && (
          <div className="fields misc-fields">
            <label>
              {t("Minute")} {number(zone.minute, (v) => setZone({ minute: v }))}
            </label>
            <label>
              {t("Your team's souls")} {number(zone.own_souls, (v) => setZone({ own_souls: v }), 1000)}
            </label>
            <label>
              {t("Enemy team's souls")} {number(zone.enemy_souls, (v) => setZone({ enemy_souls: v }), 1000)}
            </label>
          </div>
        )}
        <label title={t("Allied heroes within 28 m of a Walker get 15% bullet and spirit resist")}>
          <input type="checkbox" checked={zone.enemy_walker} onChange={(e) => setZone({ enemy_walker: e.target.checked })} />
          {t("Target near its own Walker")}
        </label>
        <label title={t("A Walker's fireballs take 10% bullet and spirit resist from the enemies they hit")}>
          <input type="checkbox" checked={zone.own_walker} onChange={(e) => setZone({ own_walker: e.target.checked })} />
          {t("Target near your Walker")}
        </label>
      </div>

      {target && (
        <>
          <div className="tiles small">
            <div>
              <span>{t("Health")}</span>
              <b>{fmt(target.health, 0)}</b>
            </div>
            <div>
              <span>{t("Bullet resist")}</span>
              <b>{fmt(target.bullet_resist, 1)}%</b>
            </div>
            <div>
              <span>{t("Spirit resist")}</span>
              <b>{fmt(target.spirit_resist, 1)}%</b>
            </div>
          </div>
          {target.notes.map((note) => (
            <div key={note} className="passive">
              {note}
            </div>
          ))}
        </>
      )}

      {picking && (
        <HeroPicker
          heroes={heroes}
          selected={enemy?.hero ?? ""}
          onClose={() => setPicking(false)}
          onPick={(h) => {
            setEnemy(enemy ? { ...enemy, hero: h, ability_tiers: [0, 0, 0, 0] } : enemyBuild(h));
            setPicking(false);
          }}
        />
      )}
      {shopping && enemy && (
        <div className="overlay" onClick={() => setShopping(false)}>
          <div className="enemy-shop" onClick={(e) => e.stopPropagation()}>
            <div className="enemy-shop-head">
              <strong>{t("{hero} items", { hero: hero?.name ?? t("Enemy") })}</strong>
              <span className="muted">{t("{n} souls spent", { n: fmt(spent, 0) })}</span>
              <button className="ghost small" onClick={() => setShopping(false)}>
                {t("Done")}
              </button>
            </div>
            <Shop items={items} owned={owned} purchased={new Set(enemy.purchases)} onToggle={toggleItem} />
          </div>
        </div>
      )}
    </div>
  );
}
