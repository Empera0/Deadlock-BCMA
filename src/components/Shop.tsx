import { useMemo, useState } from "react";
import { Category, CATEGORY_LABEL, fmt, Item } from "../api";
import { t } from "../i18n";

interface Props {
  items: Item[];
  owned: Set<string>;
  purchased: Set<string>;
  onToggle: (className: string) => void;
}

const TIER_NAME = ["", "I", "II", "III", "IV", "V"];

/** Tooltip sections; translated where shown. */
const SECTIONS: [string, string][] = [
  ["innate", "Always"],
  ["passive", "Passive (conditional)"],
  ["active", "Active"],
  ["", "Other"],
];

function Tooltip({ item, x, y }: { item: Item; x: number; y: number }) {
  const left = Math.min(x + 16, window.innerWidth - 330);
  const top = Math.min(y + 16, window.innerHeight - 320);
  return (
    <div className="tooltip" style={{ left, top }}>
      <div className={"tooltip-title " + item.category}>
        {item.name}
        <span className="cost">{fmt(item.cost)}</span>
      </div>
      {item.active && <div className="tag">{t("Active item")}</div>}
      {item.components.length > 0 && <div className="tag">{t("Upgrade: discounted if you own the component")}</div>}
      {item.description && <div className="tooltip-desc">{item.description}</div>}
      {SECTIONS.map(([section, title]) => {
        const props = item.props.filter((p) => p.section === section);
        if (props.length === 0) return null;
        return (
          <div key={section} className="tooltip-section">
            <div className="tooltip-section-title">{t(title)}</div>
            {props.map((p) => (
              <div key={p.key} className="tooltip-row">
                <span>{p.label || p.key}</span>
                <span>
                  {p.value > 0 && p.modifier ? "+" : ""}
                  {fmt(p.value)}
                  {p.postfix}
                  {p.scale && p.scale.coef !== 0 ? ` (${t("×{n} scaling", { n: fmt(p.scale.coef) })})` : ""}
                </span>
              </div>
            ))}
          </div>
        );
      })}
    </div>
  );
}

export default function Shop({ items, owned, purchased, onToggle }: Props) {
  const [category, setCategory] = useState<Category>("weapon");
  const [query, setQuery] = useState("");
  const [hover, setHover] = useState<{ item: Item; x: number; y: number } | null>(null);

  // What a search looks through: the name, the stat names and the tooltip text.
  const haystacks = useMemo(
    () =>
      new Map(
        items.map((i) => [
          i.class_name,
          [i.name, ...i.props.map((p) => p.label), i.description ?? ""].join(" · ").toLowerCase(),
        ]),
      ),
    [items],
  );

  const tiers = useMemo(() => {
    const words = query.toLowerCase().split(/\s+/).filter(Boolean);
    const matches = (i: Item) => {
      const text = haystacks.get(i.class_name) ?? "";
      // The whole phrase, or failing that every word somewhere.
      return text.includes(words.join(" ")) || words.every((w) => text.includes(w));
    };
    const visible = items.filter((i) => (words.length ? matches(i) : i.category === category));
    const group = (list: Item[]) => {
      const byTier = new Map<number, Item[]>();
      for (const i of list) byTier.set(i.tier, [...(byTier.get(i.tier) ?? []), i]);
      return [...byTier.entries()].sort((a, b) => a[0] - b[0]);
    };
    if (!words.length) return [{ title: "", tiers: group(visible) }];
    // Items named like the search or granting that stat come first; the rest only mention it.
    const phrase = words.join(" ");
    const gives = (i: Item) =>
      i.name.toLowerCase().includes(phrase) ||
      i.props.some((p) => p.modifier && p.value > 0 && p.label.toLowerCase().includes(phrase));
    return [
      { title: t("Named like it or gives it"), tiers: group(visible.filter(gives)) },
      { title: t("Mentions it"), tiers: group(visible.filter((i) => !gives(i))) },
    ].filter((g) => g.tiers.length > 0);
  }, [items, category, query, haystacks]);

  return (
    <section className="shop">
      <div className="shop-head">
        {(Object.keys(CATEGORY_LABEL) as Category[]).map((c) => (
          <button
            key={c}
            className={"tab " + c + (c === category && !query ? " on" : "")}
            onClick={() => {
              setCategory(c);
              setQuery("");
            }}
          >
            {t(CATEGORY_LABEL[c])}
          </button>
        ))}
        <input placeholder={t("Search name, stat or description…")} value={query} onChange={(e) => setQuery(e.target.value)} />
      </div>
      <div className="shop-body">
        {tiers.map((g) => (
          <div key={g.title}>
            {g.title && <div className="shop-group-title">{g.title}</div>}
            {g.tiers.map(([tier, list]) => (
          <div key={tier} className="tier">
            <div className={"tier-label " + (tiers.length === 1 && !query ? category : "")}>
              <small>{t("Tier {n}", { n: TIER_NAME[tier] ?? tier })}</small>
              <b>{fmt(list[0].cost)}</b>
            </div>
            <div className="tier-items">
              {list.map((i) => (
                <button
                  key={i.class_name}
                  className={
                    "item " +
                    i.category +
                    (owned.has(i.class_name) ? " owned" : purchased.has(i.class_name) ? " consumed" : "")
                  }
                  onClick={() => onToggle(i.class_name)}
                  onMouseMove={(e) => setHover({ item: i, x: e.clientX, y: e.clientY })}
                  onMouseLeave={() => setHover(null)}
                >
                  <span className="item-art">
                    <img src={i.icon} alt="" loading="lazy" />
                    {i.active && <em>{t("Active")}</em>}
                    {owned.has(i.class_name) && <i className="item-check">✓</i>}
                  </span>
                  <span className="item-name">{i.name}</span>
                </button>
              ))}
            </div>
          </div>
            ))}
          </div>
        ))}
        {tiers.length === 0 && <p className="muted">{t("No items found.")}</p>}
      </div>
      {hover && <Tooltip {...hover} />}
    </section>
  );
}
