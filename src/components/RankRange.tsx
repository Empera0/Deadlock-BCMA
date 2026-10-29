import { t } from "../i18n";
import { SETTINGS, tierFrom, tierTo, TIERS, useSetting } from "../settings";

/** A rank range, from one rank to another, kept in Settings: by default the Draft's, which every screen reading win
 *  rates shares. With `top`, the range can also be left to the top of the ladder (the Build generator's default). */
export default function RankRange({
  inline = false,
  fromKey = SETTINGS.draftBadge,
  toKey = SETTINGS.draftMaxBadge,
  top = false,
  label = "Ranks from",
}: {
  inline?: boolean;
  fromKey?: string;
  toKey?: string;
  top?: boolean;
  label?: string;
}) {
  const [from, setFrom] = useSetting(fromKey);
  const [to, setTo] = useSetting(toKey);
  const low = Number(from) || 0;
  // A range that would end below its start ends at the start's rank.
  const pickFrom = (value: string) => {
    setFrom(value);
    if (to && value !== "" && Number(to) < Number(value)) setTo(String(Number(value) + 5));
  };
  return (
    <>
      <label className={"field" + (inline ? " inline" : "")}>
        {t(label)}
        <select value={from} onChange={(e) => pickFrom(e.target.value)}>
          {top && <option value="">{t("the top players")}</option>}
          <option value="0">{t("the lowest")}</option>
          {TIERS.map((name, i) => (
            <option key={name} value={tierFrom(i)}>
              {name}
            </option>
          ))}
        </select>
      </label>
      {from !== "" && (
        <label className={"field" + (inline ? " inline" : "")}>
          {t("to")}
          <select value={to} onChange={(e) => setTo(e.target.value)}>
            <option value="">{t("the highest")}</option>
            {TIERS.map((name, i) =>
              tierTo(i) >= low ? (
                <option key={name} value={tierTo(i)}>
                  {name}
                </option>
              ) : null,
            )}
          </select>
        </label>
      )}
    </>
  );
}
