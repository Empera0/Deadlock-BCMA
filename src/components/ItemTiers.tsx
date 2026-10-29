import { useEffect, useMemo, useState } from "react";
import { api, Category, CATEGORY_LABEL, fmt, Hero, ItemTier } from "../api";
import { percent, t } from "../i18n";
import ErrorNote from "./ErrorNote";

type Key = "tier" | "win_rate" | "pick_rate" | "buy_minute" | "matches" | "cost";
/** The columns a list can be sorted by; translated where shown. */
const COLUMNS: [Key, string, string][] = [
  ["cost", "Cost", "What it costs"],
  ["win_rate", "Win rate", "Matches won by the players who bought it"],
  ["pick_rate", "Bought by", "Share of players who bought it"],
  ["buy_minute", "Bought at", "When it is bought on average"],
  ["matches", "Matches", "Matches it was bought in"],
];
const ORDER = ["S", "A", "B", "C", "D"];

/** Every item at the chosen ranks, for all heroes or one: how often it is bought, when, and how its buyers do. */
export default function ItemTiers({ heroes, badge, maxBadge, hero, onHero }: { heroes: Hero[]; badge: number; maxBadge: number | null; hero: number | null; onHero: (id: number | null) => void }) {
  const [rows, setRows] = useState<ItemTier[] | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [attempt, setAttempt] = useState(0);
  const [sort, setSort] = useState<Key>("tier");
  const [category, setCategory] = useState<Category | "">("");
  const [query, setQuery] = useState("");

  useEffect(() => {
    let stale = false;
    setBusy(true);
    setError("");
    api
      .itemTiers(badge, maxBadge, hero)
      .then((r) => !stale && setRows(r))
      .catch((e) => !stale && setError(String(e)))
      .finally(() => !stale && setBusy(false));
    return () => {
      stale = true;
    };
  }, [badge, maxBadge, hero, attempt]);

  const shown = useMemo(() => {
    const value = (i: ItemTier) => (sort === "tier" ? (5 - ORDER.indexOf(i.tier)) * 1000 + i.ranked_rate : sort === "buy_minute" ? -i.buy_minute : i[sort]);
    return (rows ?? [])
      .filter((i) => (!category || i.category === category) && (!query || i.name.toLowerCase().includes(query.toLowerCase())))
      .sort((a, b) => value(b) - value(a));
  }, [rows, sort, category, query]);

  return (
    <>
      <div className="draft-bar">
        <div className="segmented">
          <button className={category === "" ? "on" : ""} onClick={() => setCategory("")}>
            {t("All")}
          </button>
          {(Object.keys(CATEGORY_LABEL) as Category[]).map((c) => (
            <button key={c} className={category === c ? "on" : ""} onClick={() => setCategory(c)}>
              {t(CATEGORY_LABEL[c])}
            </button>
          ))}
        </div>
        <select value={hero ?? ""} onChange={(e) => onHero(e.target.value ? Number(e.target.value) : null)}>
          <option value="">{t("Every hero")}</option>
          {[...heroes]
            .sort((a, b) => a.name.localeCompare(b.name))
            .map((h) => (
              <option key={h.id} value={h.id}>
                {h.name}
              </option>
            ))}
        </select>
        <input className="draft-search" placeholder={t("Find an item…")} value={query} onChange={(e) => setQuery(e.target.value)} />
        {busy && <span className="muted">{t("Working…")}</span>}
      </div>
      <ErrorNote error={error} onRetry={() => setAttempt((a) => a + 1)} />
      {rows && (
        <>
          <p className="muted">
            {t(
              "Items are compared with the others of the same cost: an expensive item is bought by players already ahead, so its raw win rate is high whatever it does. S and A win clearly more than their cost's average, C and D clearly less.",
            )}
          </p>
          <table className="hero-tiers">
            <thead>
              <tr>
                <th className={sort === "tier" ? "on" : ""} onClick={() => setSort("tier")}>
                  {t("Tier")}
                </th>
                <th>{t("Item")}</th>
                {COLUMNS.map(([key, label, help]) => (
                  <th key={key} className={sort === key ? "on" : ""} title={t(help)} onClick={() => setSort(key)}>
                    {t(label)}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {shown.map((i) => (
                <tr key={i.item}>
                  <td>
                    <span className={"tier-badge " + i.tier}>{i.tier}</span>
                  </td>
                  <td>
                    <span className={"item-cell " + i.category}>
                      <img src={i.icon} alt="" loading="lazy" />
                      {i.name}
                    </span>
                  </td>
                  <td>{fmt(i.cost)}</td>
                  <td title={t("Ranked by {n}", { n: percent(fmt(i.ranked_rate, 1)) })}>{percent(fmt(i.win_rate, 1))}</td>
                  <td>{percent(fmt(i.pick_rate, 1))}</td>
                  <td>{t("{n} min", { n: fmt(i.buy_minute, 1) })}</td>
                  <td>{fmt(i.matches)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </>
  );
}
