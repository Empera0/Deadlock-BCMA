import { Build, fmt, Item, Sheet } from "../api";

interface Props {
  build: Build;
  sheet: Sheet | null;
  items: Map<string, Item>;
  onChange: (patch: Partial<Build>) => void;
}

export default function BuildBar({ build, sheet, items, onChange }: Props) {
  const count = build.purchases.length;
  const upto = build.upto ?? count;
  const owned = new Set(sheet?.owned ?? []);
  const steps = new Map(sheet?.steps.map((s) => [s.item, s]) ?? []);

  const move = (index: number, dir: -1 | 1) => {
    const next = [...build.purchases];
    const j = index + dir;
    if (j < 0 || j >= next.length) return;
    [next[index], next[j]] = [next[j], next[index]];
    onChange({ purchases: next });
  };
  const remove = (className: string) =>
    onChange({
      purchases: build.purchases.filter((p) => p !== className),
      active: build.active.filter((a) => a !== className),
      upto: null,
    });
  const toggleActive = (className: string) =>
    onChange({
      active: build.active.includes(className)
        ? build.active.filter((a) => a !== className)
        : [...build.active, className],
    });

  return (
    <section className="buildbar">
      <div className="buildbar-head">
        <strong>Purchase order</strong>
        {sheet && (
          <span className="muted">
            {sheet.slots_used}/{sheet.slot_limit} slots · {fmt(sheet.spent)} souls spent
          </span>
        )}
        {count > 0 && (
          <>
            <input
              type="range"
              min={0}
              max={count}
              value={upto}
              onChange={(e) => onChange({ upto: Number(e.target.value) === count ? null : Number(e.target.value) })}
            />
            <span className="muted">
              {upto}/{count} purchases counted
            </span>
            <button className="ghost" onClick={() => onChange({ purchases: [], active: [], upto: null })}>
              Clear
            </button>
          </>
        )}
      </div>
      <div className="chips">
        {count === 0 && <span className="muted">Click an item in the shop to add it to your build.</span>}
        {build.purchases.map((className, i) => {
          const item = items.get(className);
          if (!item) return null;
          const future = i >= upto;
          const consumed = !future && !owned.has(className);
          const conditional = item.props.some((p) => p.modifier && p.section !== "innate");
          const on = build.active.includes(className);
          const step = steps.get(className);
          return (
            <div
              key={className}
              className={`chip ${item.category}${future ? " future" : ""}${consumed ? " consumed" : ""}`}
            >
              <span className="chip-index">{i + 1}</span>
              <img src={item.icon} alt="" />
              <div className="chip-text">
                <span>{item.name}</span>
                <small>{consumed ? "upgraded" : `${fmt(step ? step.paid : item.cost)} souls`}</small>
              </div>
              {item.imbue && !consumed && (
                <select
                  className="imbue"
                  title="Ability this item is imbued into"
                  value={build.imbues[className] ?? ""}
                  onChange={(e) => {
                    const imbues = { ...build.imbues };
                    if (e.target.value === "") delete imbues[className];
                    else imbues[className] = Number(e.target.value);
                    onChange({ imbues });
                  }}
                >
                  <option value="">Imbue…</option>
                  {sheet?.abilities.map((a, index) =>
                    item.imbue === "imbue_active_non_ult" && index === 3 ? null : (
                      <option key={a.class_name} value={index}>
                        {a.name}
                      </option>
                    ),
                  )}
                </select>
              )}
              {conditional && !item.imbue && (
                <button
                  className={"chip-btn cond" + (on ? " on" : "")}
                  title="Include conditional (passive/active) effects"
                  onClick={() => toggleActive(className)}
                >
                  ⚡
                </button>
              )}
              <button className="chip-btn" title="Move earlier" onClick={() => move(i, -1)}>
                ‹
              </button>
              <button className="chip-btn" title="Move later" onClick={() => move(i, 1)}>
                ›
              </button>
              <button className="chip-btn" title="Remove" onClick={() => remove(className)}>
                ×
              </button>
            </div>
          );
        })}
      </div>
      {sheet?.errors.map((e) => (
        <div key={e} className="error">
          {e}
        </div>
      ))}
    </section>
  );
}
