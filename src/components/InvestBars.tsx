import { Category, CATEGORY_LABEL, fmt, Hero, Sheet } from "../api";

interface Props {
  hero: Hero;
  sheet: Sheet;
}

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
          Level <b>{sheet.level}</b>
        </span>
        <span>
          Boons <b>{sheet.boons}</b>
        </span>
        <span>
          Souls spent <b>{fmt(sheet.spent)}</b>
        </span>
      </div>
      {(Object.keys(BONUS) as Category[]).map((cat) => {
        const tiers = hero.invest[cat] ?? [];
        const { unit, name } = BONUS[cat];
        const souls = spent[cat];
        const reached = tiers.filter((t) => t.threshold <= souls).length;
        const current = reached > 0 ? tiers[reached - 1].bonus : 0;
        const next = tiers[reached];
        return (
          <div key={cat} className={"invest-row " + cat}>
            <div className="invest-head">
              <strong>{CATEGORY_LABEL[cat]}</strong>
              <span className="muted">{fmt(souls)} souls</span>
              <b>
                +{fmt(current)}
                {unit} {name}
              </b>
            </div>
            <div className="invest-bar">
              {tiers.map((t, i) => {
                const from = i > 0 ? tiers[i - 1].threshold : 0;
                const gain = t.bonus - (i > 0 ? tiers[i - 1].bonus : 0);
                const fill = Math.min(1, Math.max(0, (souls - from) / (t.threshold - from)));
                return (
                  <div
                    key={t.threshold}
                    className={"seg" + (fill >= 1 ? " reached" : "") + (i === reached ? " next" : "")}
                    title={`${fmt(t.threshold)} souls: +${fmt(t.bonus)}${unit} ${name} in total (+${fmt(gain)}${unit} from this spike)`}
                  >
                    <span className="seg-threshold">{short(t.threshold)}</span>
                    <div className="seg-track">
                      <div style={{ width: `${fill * 100}%` }} />
                    </div>
                    <span className="seg-total">
                      +{fmt(t.bonus)}
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
                  Next spike at <b>{fmt(next.threshold)}</b>: +{fmt(next.bonus - current)}
                  {unit} {name} · {fmt(next.threshold - souls)} souls to go
                </>
              ) : (
                "All spikes reached"
              )}
            </div>
          </div>
        );
      })}
    </div>
  );
}
