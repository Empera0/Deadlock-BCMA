import { useEffect, useState } from "react";
import { api, DraftFitRow, fmt, Hero, HeroPageReport, PopularReport } from "../api";
import { percent, t } from "../i18n";
import ErrorNote from "./ErrorNote";

/** Rows shown in each list of matchups. */
const SHOWN = 7;

const signed = (v: number) => `${v >= 0 ? "+" : "−"}${fmt(Math.abs(v), 1)}`;

function Matchups({ title, help, rows, byId }: { title: string; help: string; rows: DraftFitRow[]; byId: Map<number, Hero> }) {
  return (
    <div className="group hero-page-list">
      <h3 title={help}>{title}</h3>
      {rows.length === 0 && <p className="muted">{t("Not enough matches.")}</p>}
      {rows.map((r) => {
        const h = byId.get(r.hero_id);
        if (!h) return null;
        return (
          <div key={r.hero_id} className="hero-page-row" title={t("{n} matches · {p} pts beyond expected", { n: fmt(r.matches), p: signed(r.points) })}>
            <img src={h.icon} alt="" />
            <span>{h.name}</span>
            <b>{percent(fmt(r.rate ?? 0, 1))}</b>
            <small className={r.points >= 0.5 ? "good" : r.points <= -0.5 ? "bad" : "muted"}>{signed(r.points)}</small>
          </div>
        );
      })}
    </div>
  );
}

/** One hero at the chosen ranks: its numbers, who it beats and loses to, who it pairs well with, and its build. */
export default function HeroPage({
  hero,
  heroes,
  badge,
  maxBadge,
  onBack,
  onBuild,
  onItems,
}: {
  hero: Hero;
  heroes: Hero[];
  badge: number;
  maxBadge: number | null;
  onBack: () => void;
  onBuild: () => void;
  onItems: () => void;
}) {
  const [page, setPage] = useState<HeroPageReport | null>(null);
  const [build, setBuild] = useState<PopularReport | null>(null);
  const [error, setError] = useState("");
  const [attempt, setAttempt] = useState(0);
  const byId = new Map(heroes.map((h) => [h.id, h]));

  useEffect(() => {
    let stale = false;
    setError("");
    setPage(null);
    api
      .heroPage(hero.id, badge, maxBadge)
      .then((p) => !stale && setPage(p))
      .catch((e) => !stale && setError(String(e)));
    api
      .popularBuild(hero.id)
      .then((b) => !stale && setBuild(b))
      .catch(() => !stale && setBuild(null));
    return () => {
      stale = true;
    };
  }, [hero.id, badge, maxBadge, attempt]);

  const tier = page?.tier;
  const worst = page ? [...page.against].reverse().slice(0, SHOWN) : [];
  return (
    <div className="hero-page">
      <div className="hero-page-head group" style={hero.background ? { ["--hero-bg" as string]: `url("${hero.background}")` } : undefined}>
        <button className="ghost small hero-page-back" onClick={onBack}>
          ← {t("All heroes")}
        </button>
        <img className="hero-page-card" src={hero.card} alt="" />
        <div className="hero-page-title">
          <h2>
            {tier && <span className={"tier-badge " + tier.tier}>{tier.tier}</span>}
            {hero.name}
          </h2>
          <span className="muted">{t(hero.hero_type)}</span>
          {tier && (
            <div className="hero-page-stats">
              <span>
                <small>{t("Win rate")}</small>
                <b>{percent(fmt(tier.win_rate, 1))}</b>
              </span>
              <span>
                <small>{t("Picked")}</small>
                <b>{percent(fmt(tier.pick_rate, 1))}</b>
              </span>
              <span>
                <small>{t("Banned")}</small>
                <b>{percent(fmt(tier.ban_share, 1))}</b>
              </span>
              <span>
                <small>{t("K / D / A")}</small>
                <b>
                  {fmt(tier.kills, 1)} / {fmt(tier.deaths, 1)} / {fmt(tier.assists, 1)}
                </b>
              </span>
              <span>
                <small>{t("Souls")}</small>
                <b>{fmt(tier.net_worth, 0)}</b>
              </span>
              <span>
                <small>{t("Matches")}</small>
                <b>{fmt(tier.matches)}</b>
              </span>
            </div>
          )}
          <div className="hero-page-actions">
            <button onClick={onBuild}>{t("Open its build")}</button>
            <button className="ghost" onClick={onItems}>
              {t("Its items")}
            </button>
          </div>
        </div>
      </div>
      <ErrorNote error={error} onRetry={() => setAttempt((a) => a + 1)} />
      {!page && !error && <p className="muted">{t("Working…")}</p>}
      {page && (
        <div className="hero-page-grid">
          <Matchups title={t("Strong against")} help={t("Its win rate against each hero, best first beyond what the two heroes' own win rates make expected")} rows={page.against.slice(0, SHOWN)} byId={byId} />
          <Matchups title={t("Weak against")} help={t("Its win rate against each hero, worst first beyond what the two heroes' own win rates make expected")} rows={worst} byId={byId} />
          <Matchups title={t("Best teammates")} help={t("The win rate of the two together, best first beyond what their own win rates make expected")} rows={page.with.slice(0, SHOWN)} byId={byId} />
        </div>
      )}
      {build && (
        <div className="group">
          <h3>
            {t("Popular build")} <small className="muted">{t("{rank} lobbies, {n} matches", { rank: build.rank, n: fmt(build.build.matches) })}</small>
          </h3>
          <div className="hero-page-build">
            {build.build.core.map((i) => (
              <span key={i.item} className={"hero-page-item " + i.category} title={`${i.name} · ${percent(fmt(i.kept_pct, 0))}`}>
                <img src={i.icon} alt="" />
                <small>{i.name}</small>
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
