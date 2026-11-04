import { Build, fmt, Hero, Item, Sheet } from "./api";
import { percent, t } from "./i18n";
import { shareCode } from "./share";

/** The numbers the card shows, by the sheet's keys, with how they are written. */
const NUMBERS: [string, (v: number) => string][] = [
  ["dps_sustained", (v) => fmt(v, 0)],
  ["max_health", (v) => fmt(v, 0)],
  ["bullet_resist", (v) => percent(fmt(v, 0))],
  ["spirit_resist", (v) => percent(fmt(v, 0))],
  ["spirit_power", (v) => fmt(v, 0)],
  ["move_speed", (v) => fmt(v, 1)],
];
const COLORS: Record<string, string> = { weapon: "#d3842f", vitality: "#6ba337", spirit: "#9a5fd4" };

export function image(src: string): Promise<HTMLImageElement | null> {
  return new Promise((resolve) => {
    if (!src) return resolve(null);
    const img = new Image();
    // The asset server allows any origin, so the canvas stays exportable.
    img.crossOrigin = "anonymous";
    img.onload = () => resolve(img);
    img.onerror = () => resolve(null);
    img.src = src;
  });
}

export function rounded(ctx: CanvasRenderingContext2D, x: number, y: number, w: number, h: number, r: number) {
  ctx.beginPath();
  ctx.roundRect(x, y, w, h, r);
}

/** The build as a picture to paste anywhere: the hero, its items in buying order, its main numbers and its code. */
export async function buildCard(build: Build, hero: Hero, heroes: Hero[], items: Map<string, Item>, sheet: Sheet | null): Promise<Blob> {
  const W = 1200;
  const purchases = build.purchases.map((c) => items.get(c)).filter((i): i is Item => !!i);
  const perRow = 8;
  const rows = Math.max(1, Math.ceil(purchases.length / perRow));
  const code = shareCode(build, heroes, items);
  // The whole code, over as many lines as it takes: a cut one would not load.
  const perLine = 140;
  const codeLines = Array.from({ length: Math.ceil(code.length / perLine) }, (_, i) => code.slice(i * perLine, (i + 1) * perLine));
  const H = 330 + rows * 150 + 40 + codeLines.length * 18 + 30;
  const canvas = document.createElement("canvas");
  canvas.width = W;
  canvas.height = H;
  const ctx = canvas.getContext("2d")!;
  const font = getComputedStyle(document.documentElement).fontFamily || "sans-serif";

  // Background: dark, with the hero's art on the right.
  const bg = ctx.createLinearGradient(0, 0, W, H);
  bg.addColorStop(0, "#0c0c0f");
  bg.addColorStop(1, "#16141a");
  ctx.fillStyle = bg;
  ctx.fillRect(0, 0, W, H);
  const [art, icon, ...icons] = await Promise.all([image(hero.background ?? ""), image(hero.icon), ...purchases.map((i) => image(i.icon))]);
  if (art) {
    const h = 420;
    const w = (art.width / art.height) * h;
    ctx.globalAlpha = 0.35;
    ctx.drawImage(art, W - w + 120, -40, w, h);
    ctx.globalAlpha = 1;
    const fade = ctx.createLinearGradient(W - w + 120, 0, W - w + 420, 0);
    fade.addColorStop(0, "#0c0c0f");
    fade.addColorStop(1, "rgba(12,12,15,0)");
    ctx.fillStyle = fade;
    ctx.fillRect(W - w + 120, -40, 300, h);
  }

  // The hero.
  if (icon) {
    ctx.save();
    ctx.beginPath();
    ctx.arc(110, 110, 62, 0, Math.PI * 2);
    ctx.clip();
    ctx.drawImage(icon, 48, 48, 124, 124);
    ctx.restore();
  }
  ctx.fillStyle = "#f2efe9";
  ctx.font = `700 46px ${font}`;
  ctx.fillText(hero.name, 200, 100);
  ctx.fillStyle = "#9a978f";
  ctx.font = `500 20px ${font}`;
  const spent = sheet ? t("{n} souls", { n: fmt(sheet.spent) }) : "";
  ctx.fillText([t("{n} items", { n: purchases.length }), spent].filter(Boolean).join(" · "), 202, 136);

  // Its numbers.
  if (sheet) {
    let x = 48;
    for (const [key, write] of NUMBERS) {
      const stat = sheet.stats.find((s) => s.key === key);
      if (!stat || stat.value === null) continue;
      rounded(ctx, x, 196, 176, 76, 12);
      ctx.fillStyle = "rgba(255,255,255,0.06)";
      ctx.fill();
      ctx.fillStyle = "#9a978f";
      ctx.font = `500 15px ${font}`;
      ctx.fillText(stat.label, x + 14, 222, 150);
      ctx.fillStyle = "#f2efe9";
      ctx.font = `700 28px ${font}`;
      ctx.fillText(write(stat.value), x + 14, 258);
      x += 184;
    }
  }

  // The items in buying order.
  purchases.forEach((item, n) => {
    const x = 48 + (n % perRow) * 138;
    const y = 300 + Math.floor(n / perRow) * 150;
    rounded(ctx, x, y, 124, 124, 12);
    ctx.fillStyle = COLORS[item.category] ?? "#555";
    ctx.fill();
    const img = icons[n];
    if (img) {
      ctx.save();
      rounded(ctx, x + 3, y + 3, 118, 118, 10);
      ctx.clip();
      ctx.drawImage(img, x + 3, y + 3, 118, 118);
      ctx.restore();
    }
    ctx.fillStyle = "rgba(0,0,0,0.75)";
    rounded(ctx, x + 6, y + 6, 30, 24, 8);
    ctx.fill();
    ctx.fillStyle = "#fff";
    ctx.font = `700 15px ${font}`;
    ctx.fillText(String(n + 1), x + (n + 1 >= 10 ? 11 : 16), y + 23);
    ctx.fillStyle = "#d8d4cc";
    ctx.font = `500 14px ${font}`;
    ctx.fillText(item.name, x, y + 142, 124);
  });

  // The code to load it, and where it comes from.
  ctx.fillStyle = "#6f6b63";
  ctx.font = `500 13px ui-monospace, Consolas, monospace`;
  codeLines.forEach((line, i) => ctx.fillText(line, 48, H - 30 - (codeLines.length - 1 - i) * 18, W - 280));
  ctx.textAlign = "right";
  ctx.fillStyle = "#e6cf9c";
  ctx.font = `700 15px ${font}`;
  ctx.fillText("DEADLOCK BCMA", W - 48, H - 30);

  return new Promise((resolve, reject) => canvas.toBlob((b) => (b ? resolve(b) : reject(new Error(t("The picture could not be made")))), "image/png"));
}
