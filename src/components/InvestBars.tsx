import { Category, CATEGORY_LABEL, fmt, Hero, Sheet } from "../api";
import { t } from "../i18n";

interface Props {
  hero: Hero;
  sheet: Sheet;
}

/** The bonus each kind of investment gives; translated where shown. */
const BONUS: Record<Category, { unit: string; name: string }> = {
  weapon: { unit: "%", name: "Weapon Damage" },
  vitality: { unit: "%", name: "Base Health" },
  spirit: { unit: "", name: "Spirit Power" },
};

const short = (souls: number) => (souls >= 1000 ? `${souls / 1000}k` : String(souls));

export default function InvestBars({ hero, sheet }: Props) {
  const spent: Record<Category, number> = {
    weapon: sheet.spent_weapon,
    vitality: sheet.spent_vitality,
    spirit: sheet.spent_spirit,
  };

  return (
    <div className="invest">
      <div className="level-line">
        <span>
          {t("Level")} <b>{sheet.level}</b>
        </span>
        <span>
          {t("Boons")} <b>{sheet.boons}</b>
        </span>
        <span>
          {t("Souls spent")} <b>{fmt(sheet.spent)}</b>
        </span>
      </div>
      {(Object.keys(BONUS) as Category[]).map((cat) => {
        const tiers = hero.invest[cat] ?? [];
        const unit = BONUS[cat].unit;
        const name = t(BONUS[cat].name);
        const souls = spent[cat];
        const reached = tiers.filter((tier) => tier.threshold <= souls).length;
        const current = reached > 0 ? tiers[reached - 1].bonus : 0;
        const next = tiers[reached];
        return (
          <div key={cat} className={"invest-row " + cat}>
            <div className="invest-head">
              <strong>{t(CATEGORY_LABEL[cat])}</strong>
              <span className="muted">{t("{n} souls", { n: fmt(souls) })}</span>
              <b>
                +{fmt(current)}
                {unit} {name}
              </b>
            </div>
            <div className="invest-bar">
              {tiers.map((tier, i) => {
                const from = i > 0 ? tiers[i - 1].threshold : 0;
                const gain = tier.bonus - (i > 0 ? tiers[i - 1].bonus : 0);
                const fill = Math.min(1, Math.max(0, (souls - from) / (tier.threshold - from)));
                return (
                  <div
                    key={tier.threshold}
                    className={"seg" + (fill >= 1 ? " reached" : "") + (i === reached ? " next" : "")}
                    title={t("{n} souls: +{total} {name} in total (+{gain} from this spike)", {
                      n: fmt(tier.threshold),
                      total: `${fmt(tier.bonus)}${unit}`,
                      name,
                      gain: `${fmt(gain)}${unit}`,
                    })}
                  >
                    <span className="seg-threshold">{short(tier.threshold)}</span>
                    <div className="seg-track">
                      <div style={{ width: `${fill * 100}%` }} />
                    </div>
                    <span className="seg-total">
                      +{fmt(tier.bonus)}
                      {unit}
                    </span>
                    <span>(+{fmt(gain)})</span>
                  </div>
                );
              })}
            </div>
            <div className="invest-next">
              {next ? (
                <>
                  {t("Next spike at {n}: +{gain} {name} · {left} souls to go", {
                    n: fmt(next.threshold),
                    gain: `${fmt(next.bonus - current)}${unit}`,
                    name,
                    left: fmt(next.threshold - souls),
                  })}
                </>
              ) : (
                t("All spikes reached")
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
