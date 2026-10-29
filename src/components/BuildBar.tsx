import { useState } from "react";
import { Build, fmt, Hero, Item, Sheet } from "../api";
import { readShareCode, shareCode } from "../share";
import { t } from "../i18n";

interface Props {
  build: Build;
  heroes: Hero[];
  sheet: Sheet | null;
  items: Map<string, Item>;
  onChange: (patch: Partial<Build>) => void;
}

/** Copies the build as a code, and reads a pasted one. */
function ShareButtons({ build, heroes, items, onChange }: Pick<Props, "build" | "heroes" | "items" | "onChange">) {
  const [importing, setImporting] = useState(false);
  const [code, setCode] = useState("");
  const [note, setNote] = useState("");
  const share = () => {
    const text = shareCode(build, heroes, items);
    navigator.clipboard.writeText(text).then(
      () => setNote(t("Build code copied")),
      () => {
        // Without clipboard access the code is shown to copy by hand.
        setCode(text);
        setImporting(true);
        setNote(t("Copy the code from the field"));
      },
    );
  };
  const load = () => {
    try {
      onChange(readShareCode(code, heroes, [...items.values()]));
      setImporting(false);
      setCode("");
      setNote(t("Build loaded from the code"));
    } catch (e) {
      setNote(e instanceof Error ? e.message : String(e));
    }
  };
  return (
    <span className="share">
      <button className="ghost" onClick={share} title={t("Copy the build as a code to paste anywhere")}>
        {t("Share")}
      </button>
      <button className="ghost" onClick={() => setImporting(!importing)} title={t("Load a build from a code")}>
        {t("Import")}
      </button>
      {importing && (
        <>
          <input
            className="share-code"
            placeholder={t("Paste a BCMA1. build code")}
            value={code}
            onChange={(e) => setCode(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && load()}
            onFocus={(e) => e.target.select()}
            autoFocus
          />
          <button onClick={load} disabled={!code.trim()}>
            {t("Load")}
          </button>
        </>
      )}
      {note && <span className="muted">{note}</span>}
    </span>
  );
}

export default function BuildBar({ build, heroes, sheet, items, onChange }: Props) {
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
        <strong>{t("Purchase order")}</strong>
        {sheet && (
          <span className="muted">
            {t("{a}/{b} slots · {n} souls spent", { a: sheet.slots_used, b: sheet.slot_limit, n: fmt(sheet.spent) })}
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
              {t("{a}/{b} purchases counted", { a: upto, b: count })}
            </span>
            <button className="ghost" onClick={() => onChange({ purchases: [], active: [], upto: null })}>
              {t("Clear")}
            </button>
          </>
        )}
        <ShareButtons build={build} heroes={heroes} items={items} onChange={onChange} />
      </div>
      <div className="chips">
        {count === 0 && <span className="muted">{t("Click an item in the shop to add it to your build.")}</span>}
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
              <span className="chip-slot" title={item.name}>
                <img src={item.icon} alt="" />
              </span>
              <div className="chip-text">
                <span>{item.name}</span>
                <small>{consumed ? t("upgraded") : t("{n} souls", { n: fmt(step ? step.paid : item.cost) })}</small>
              </div>
              {item.imbue && !consumed && (
                <select
                  className="imbue"
                  title={t("Ability this item is imbued into")}
                  value={build.imbues[className] ?? ""}
                  onChange={(e) => {
                    const imbues = { ...build.imbues };
                    if (e.target.value === "") delete imbues[className];
                    else imbues[className] = Number(e.target.value);
                    onChange({ imbues });
                  }}
                >
                  <option value="">{t("Imbue…")}</option>
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
                  title={t("Include conditional (passive/active) effects")}
                  onClick={() => toggleActive(className)}
                >
                  ⚡
                </button>
              )}
              <button className="chip-btn" title={t("Move earlier")} onClick={() => move(i, -1)}>
                ‹
              </button>
              <button className="chip-btn" title={t("Move later")} onClick={() => move(i, 1)}>
                ›
              </button>
              <button className="chip-btn" title={t("Remove")} onClick={() => remove(className)}>
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
