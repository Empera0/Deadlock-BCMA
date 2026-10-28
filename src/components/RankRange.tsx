import { t } from "../i18n";
import { SETTINGS, tierFrom, tierTo, TIERS, useSetting } from "../settings";

/** The ranks win rates are read at, from one rank to another, kept in Settings for every screen that reads them. */
export default function RankRange({ inline = false }: { inline?: boolean }) {
  const [from, setFrom] = useSetting(SETTINGS.draftBadge);
  const [to, setTo] = useSetting(SETTINGS.draftMaxBadge);
  const low = Number(from) || 0;
  // A range that would end below its start ends at the start's rank.
  const pickFrom = (value: string) => {
    setFrom(value);
    if (to && Number(to) < Number(value)) setTo(String(Number(value) + 5));
  };
  return (
    <>
      <label className={"field" + (inline ? " inline" : "")}>
        {t("Ranks from")}
        <select value={from} onChange={(e) => pickFrom(e.target.value)}>
          <option value="0">{t("the lowest")}</option>
          {TIERS.map((name, i) => (
            <option key={name} value={tierFrom(i)}>
              {name}
            </option>
          ))}
        </select>
      </label>
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
    </>
  );
}
