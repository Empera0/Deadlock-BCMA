import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { Category, CATEGORY_LABEL, fmt, Item } from "../api";
import { t } from "../i18n";

interface Props {
  items: Item[];
  owned: Set<string>;
  purchased: Set<string>;
  onToggle: (className: string) => void;
}


/** Tooltip sections; translated where shown. */
const SECTIONS: [string, string][] = [
  ["innate", "Always"],
  ["passive", "Passive (conditional)"],
  ["active", "Active"],
  ["", "Other"],
];

function Tooltip({ item, x, y }: { item: Item; x: number; y: number }) {
  const box = useRef<HTMLDivElement>(null);
  const [size, setSize] = useState({ w: 340, h: 320 });
  useLayoutEffect(() => {
    if (box.current) setSize({ w: box.current.offsetWidth, h: box.current.offsetHeight });
  }, [item]);
  // Beside the cursor, flipped to the other side when it would run off the window.
  const left = x + 16 + size.w > window.innerWidth ? Math.max(8, x - 16 - size.w) : x + 16;
  const top = Math.max(8, y + 16 + size.h > window.innerHeight ? Math.min(y - 16, window.innerHeight - 8) - size.h : y + 16);
  return (
    <div ref={box} className="tooltip" style={{ left, top }}>
      <div className={"tooltip-title " + item.category}>
        {item.name}
        <span className="cost">{fmt(item.cost)}</span>
      </div>
      {item.active && <div className="tag">{t("Active item")}</div>}
      {item.components.length > 0 && <div className="tag">{t("Upgrade: discounted if you own the component")}</div>}
      {item.description && <div className="tooltip-desc">{item.description}</div>}
      {SECTIONS.map(([section, title]) => {
        // Values without a label are the game's own bookkeeping: not shown.
        const props = item.props.filter((p) => p.section === section && p.label.trim() !== "");
        if (props.length === 0) return null;
        return (
          <div key={section} className="tooltip-section">
            <div className="tooltip-section-title">{t(title)}</div>
            {props.map((p) => (
              <div key={p.key} className="tooltip-row">
                <span>{p.label}</span>
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

  const card = (i: Item) => (
    <button
      key={i.class_name}
      className={"store-item " + i.category + (owned.has(i.class_name) ? " owned" : purchased.has(i.class_name) ? " consumed" : "")}
      onClick={() => onToggle(i.class_name)}
      onMouseMove={(e) => setHover({ item: i, x: e.clientX, y: e.clientY })}
      onMouseLeave={() => setHover(null)}
    >
      <span className="store-art">
        <img src={i.icon} alt="" />
        {i.imbue && <em className="store-imbue">{t("Imbue")}</em>}
        {i.active && <em className="store-active">{t("Active")}</em>}
        {owned.has(i.class_name) && (
          <i className="store-cart" title={t("In your build")}>
            <svg viewBox="0 0 24 24" width="12" height="12">
              <path d="M3 4h3l2.4 10.2a1 1 0 0 0 1 .8h8.2a1 1 0 0 0 1-.8L20 8H7" fill="none" stroke="currentColor" strokeWidth="2.4" strokeLinecap="round" strokeLinejoin="round" />
              <circle cx="10" cy="19.5" r="1.6" fill="currentColor" />
              <circle cx="17" cy="19.5" r="1.6" fill="currentColor" />
            </svg>
          </i>
        )}
      </span>
      <span className="store-name">{i.name}</span>
    </button>
  );
  const panel = (tier: number, list: Item[]) => (
    <div key={tier} className={"store-tier t" + tier} style={{ gridArea: "t" + tier }}>
      <div className="store-tier-head">
        <span className="price-tag">
          <small>$</small>
          {list[0].cost}
        </span>
        <b>{t("Tier {n}", { n: tier })}</b>
        {tier === 4 && <em>{t("Experts only")}</em>}
      </div>
      <div className="store-items">{list.map(card)}</div>
    </div>
  );

  return (
    <section className={"shop store " + (query ? "searching" : category)}>
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
        {query ? (
          tiers.map((g) => (
            <div key={g.title} className="store-results">
              <div className="store-results-title">{g.title}</div>
              <div className="store-grid">{g.tiers.map(([tier, list]) => panel(tier, list))}</div>
            </div>
          ))
        ) : (
          <div className="store-grid">
            <StoreLogo category={category} />
            {tiers[0]?.tiers.map(([tier, list]) => panel(tier, list))}
          </div>
        )}
        {tiers.length === 0 && <p className="muted">{t("No items found.")}</p>}
      </div>
      {hover && <Tooltip {...hover} />}
    </section>
  );
}

/** Each of the game's three shops has a storefront of its own: the sign is drawn the way the game draws it. */
function StoreLogo({ category }: { category: Category }) {
  if (category === "weapon")
    return (
      <div className="store-logo fairfax">
        <b>FAIRFAX</b>
        <small>ARTILLERY BOUGHT AND SOLD</small>
      </div>
    );
  if (category === "vitality")
    return (
      <div className="store-logo mps">
        <b>MPS</b>
        <span>
          <small>MYSTIC · PHARMACEUTICAL · STORES</small>
          <em>
            voted <b>#1 PHARMACY</b> for <b>FAMILIES</b> and <b>BEASTS</b>
          </em>
        </span>
      </div>
    );
  return (
    <div className="store-logo curiosity">
      <svg viewBox="0 0 300 120" width="260" height="104" aria-label="Curiosity Catalog">
        <path id="arc" d="M30 92 Q150 -10 270 92" fill="none" />
        <text>
          <textPath href="#arc" startOffset="50%" textAnchor="middle">
            CURIOSITY
          </textPath>
        </text>
      </svg>
      <span>CATALOG</span>
      <small>№ 777</small>
    </div>
  );
}
