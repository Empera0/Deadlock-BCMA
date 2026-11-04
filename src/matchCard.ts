import { fmt, MatchReport } from "./api";
import { image, rounded } from "./buildCard";
import { clock } from "./components/MatchCharts";
import { date, t } from "./i18n";

const TEAM_COLOR = ["#f2b84b", "#4fc3f7"];
const COLORS: Record<string, string> = { weapon: "#d3842f", vitality: "#6ba337", spirit: "#9a5fd4" };

/** Text cut to a width, with an ellipsis. */
function fit(ctx: CanvasRenderingContext2D, text: string, width: number): string {
  if (ctx.measureText(text).width <= width) return text;
  let s = text;
  while (s.length > 1 && ctx.measureText(s + "…").width > width) s = s.slice(0, -1);
  return s + "…";
}

/** Words wrapped to lines of a width; at most `max` lines. */
function wrap(ctx: CanvasRenderingContext2D, text: string, width: number, max: number): string[] {
  const lines: string[] = [];
  let line = "";
  for (const word of text.split(" ")) {
    const next = line ? `${line} ${word}` : word;
    if (ctx.measureText(next).width > width && line) {
      lines.push(line);
      line = word;
    } else line = next;
  }
  if (line) lines.push(line);
  if (lines.length > max) return [...lines.slice(0, max - 1), fit(ctx, lines.slice(max - 1).join(" "), width)];
  return lines;
}

/** A player's match as a picture to paste anywhere: the result, the hero, K/D/A, souls, place, titles, the items
 *  kept at the end and the three lessons. */
export async function matchCard(report: MatchReport, slot: number): Promise<Blob> {
  const p = report.players.find((x) => x.slot === slot);
  if (!p) throw new Error(t("Pick a player in the overview."));
  const lessons = report.lessons.find(([s]) => s === slot)?.[1] ?? [];
  const kept = p.purchases.filter((x) => x.sold_at === null && x.cost > 0);
  const place = report.ranking.findIndex((r) => r.slot === slot) + 1;
  const score = report.ranking.find((r) => r.slot === slot)?.score ?? 0;
  const W = 1200;
  const H = 300 + 120 + (lessons.length > 0 ? 70 + lessons.length * 86 : 0) + 60;
  const canvas = document.createElement("canvas");
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext("2d")!;
  const font = getComputedStyle(document.documentElement).fontFamily || "sans-serif";

  const bg = ctx.createLinearGradient(0, 0, W, H);
  bg.addColorStop(0, "#0c0c0f");
  bg.addColorStop(1, "#16141a");
  ctx.fillStyle = bg;
  ctx.fillRect(0, 0, W, H);
  // A band in the colour of the result.
  ctx.fillStyle = p.won ? "#3f8f3a" : "#b4453b";
  ctx.fillRect(0, 0, 10, H);

  const [icon, ...icons] = await Promise.all([image(p.hero_icon), ...kept.map((x) => image(x.icon))]);
  if (icon) {
    ctx.save();
    ctx.beginPath();
    ctx.arc(110, 120, 66, 0, Math.PI * 2);
    ctx.clip();
    ctx.drawImage(icon, 44, 54, 132, 132);
    ctx.restore();
    ctx.strokeStyle = TEAM_COLOR[p.team];
    ctx.lineWidth = 5;
    ctx.beginPath();
    ctx.arc(110, 120, 66, 0, Math.PI * 2);
    ctx.stroke();
  }
  ctx.fillStyle = p.won ? "#7cd36a" : "#ff6b5f";
  ctx.font = `800 26px ${font}`;
  ctx.fillText(p.won ? t("Victory") : t("Defeat"), 206, 74);
  ctx.fillStyle = "#f2efe9";
  ctx.font = `700 46px ${font}`;
  ctx.fillText(fit(ctx, p.hero_name, 560), 204, 124);
  ctx.fillStyle = "#9a978f";
  ctx.font = `500 19px ${font}`;
  ctx.fillText(fit(ctx, [p.player_name, t("Match {id}", { id: report.match_id }), clock(report.duration_s), date(report.start_time)].filter(Boolean).join(" · "), 640), 206, 158);

  // The place in the match, top right.
  ctx.textAlign = "right";
  ctx.fillStyle = place === 1 ? "#e6cf9c" : "#f2efe9";
  ctx.font = `800 64px ${font}`;
  ctx.fillText(place === 1 ? "MVP" : `#${place}`, W - 48, 110);
  ctx.fillStyle = "#9a978f";
  ctx.font = `500 18px ${font}`;
  ctx.fillText(t("{n} of 12 · score {s}", { n: place, s: fmt(score, 1) }), W - 48, 140);
  ctx.textAlign = "left";

  // The numbers.
  const numbers: [string, string][] = [
    [t("K / D / A"), `${p.kills} / ${p.deaths} / ${p.assists}`],
    [t("Souls"), fmt(p.net_worth)],
    [t("Hero damage"), fmt(p.hero_damage, 0)],
    [t("Objective damage"), fmt(p.objective_damage, 0)],
    [t("Last hits"), String(p.last_hits)],
    [t("Healing"), fmt(p.healing, 0)],
  ];
  numbers.forEach(([label, value], i) => {
    const x = 48 + i * 184;
    rounded(ctx, x, 206, 176, 76, 12);
    ctx.fillStyle = "rgba(255,255,255,0.06)";
    ctx.fill();
    ctx.fillStyle = "#9a978f";
    ctx.font = `500 15px ${font}`;
    ctx.fillText(fit(ctx, label, 150), x + 14, 232);
    ctx.fillStyle = "#f2efe9";
    ctx.font = `700 26px ${font}`;
    ctx.fillText(fit(ctx, value, 150), x + 14, 266);
  });

  // The items kept at the end, and the titles.
  kept.slice(0, 12).forEach((item, n) => {
    const x = 48 + n * 74;
    const y = 304;
    rounded(ctx, x, y, 66, 66, 10);
    ctx.fillStyle = COLORS[item.category] ?? "#555";
    ctx.fill();
    const img = icons[n];
    if (img) {
      ctx.save();
      rounded(ctx, x + 2, y + 2, 62, 62, 8);
      ctx.clip();
      ctx.drawImage(img, x + 2, y + 2, 62, 62);
      ctx.restore();
    }
  });
  const titles = p.titles.map((x) => x.name).slice(0, 4).join(" · ");
  if (titles) {
    ctx.fillStyle = "#e6cf9c";
    ctx.font = `700 17px ${font}`;
    ctx.fillText(fit(ctx, titles.toLocaleUpperCase(), W - 96), 48, 400);
  }

  // The lessons.
  if (lessons.length > 0) {
    let y = 460;
    ctx.fillStyle = "#f2efe9";
    ctx.font = `700 22px ${font}`;
    ctx.fillText(t("Three lessons for {hero}", { hero: p.hero_name }), 48, y);
    y += 22;
    lessons.forEach((l, i) => {
      rounded(ctx, 48, y, W - 96, 76, 12);
      ctx.fillStyle = "rgba(255,255,255,0.05)";
      ctx.fill();
      ctx.fillStyle = "#e6cf9c";
      ctx.font = `800 26px ${font}`;
      ctx.fillText(String(i + 1), 66, y + 44);
      ctx.fillStyle = "#f2efe9";
      ctx.font = `700 18px ${font}`;
      ctx.fillText(fit(ctx, l.title, W - 200), 100, y + 28);
      ctx.fillStyle = "#b9b5ad";
      ctx.font = `500 15px ${font}`;
      wrap(ctx, l.detail, W - 200, 2).forEach((line, j) => ctx.fillText(line, 100, y + 50 + j * 18));
      y += 86;
    });
  }

  ctx.textAlign = "right";
  ctx.fillStyle = "#e6cf9c";
  ctx.font = `700 15px ${font}`;
  ctx.fillText("DEADLOCK BCMA", W - 48, H - 26);
  return new Promise((resolve, reject) => canvas.toBlob((b) => (b ? resolve(b) : reject(new Error(t("The picture could not be made")))), "image/png"));
}
