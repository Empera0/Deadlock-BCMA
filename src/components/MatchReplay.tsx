import { useEffect, useMemo, useRef, useState } from "react";
import { MatchReport } from "../api";
import { clock } from "./MatchCharts";
import { t } from "../i18n";

const TEAM_COLOR = ["#f2b84b", "#4fc3f7"];
const SIZE = 1000;
/** Game seconds a real second plays at each speed. */
const SPEEDS = [5, 15, 30, 60];
/** Seconds of the match before a death the replay starts from when the death is picked. */
const LEAD_IN_S = 15;
/** Seconds a past event stays in the feed, and of path drawn behind the followed player. */
const FEED_S = 25;
const TRAIL_S = 40;

/** The match played back on the minimap: everyone's position and health, deaths and fallen objectives. */
export default function MatchReplay({ report, focus }: { report: MatchReport; focus: number | null }) {
  const { frames, frame_step: step } = report.map;
  const [time, setTime] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeed] = useState(15);
  const [follow, setFollow] = useState<number | null>(focus);
  const last = useRef<number | null>(null);
  const duration = report.duration_s;
  const bySlot = useMemo(() => new Map(report.players.map((p) => [p.slot, p])), [report]);

  useEffect(() => {
    if (!playing) {
      last.current = null;
      return;
    }
    let frame = 0;
    const tick = (now: number) => {
      const dt = last.current === null ? 0 : (now - last.current) / 1000;
      last.current = now;
      setTime((v) => {
        const next = v + dt * speed;
        if (next >= duration) {
          setPlaying(false);
          return duration;
        }
        return next;
      });
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [playing, speed, duration]);

  /** A player's place and health at the current time, between the two nearest frames. */
  const at = (f: (typeof frames)[number], when: number) => {
    const i = Math.min(Math.floor(when / step), f.x.length - 1);
    const j = Math.min(i + 1, f.x.length - 1);
    const k = Math.min(1, Math.max(0, when / step - i));
    // Across a death or a respawn the dot jumps instead of sliding over the map.
    const blend = f.hp[i] > 0 && f.hp[j] > 0 ? k : 0;
    return {
      x: ((f.x[i] + (f.x[j] - f.x[i]) * blend) / 1000) * SIZE,
      y: ((f.y[i] + (f.y[j] - f.y[i]) * blend) / 1000) * SIZE,
      hp: f.hp[k < 0.5 ? i : j] ?? 0,
    };
  };

  const deaths = report.map.deaths;
  const fallen = report.objectives.filter((o) => o.destroyed_at !== null);
  const feed = [
    ...deaths.filter((d) => d.time <= time && d.time > time - FEED_S).map((d) => ({ time: d.time, kind: "death" as const, d })),
    ...fallen.filter((o) => (o.destroyed_at as number) <= time && (o.destroyed_at as number) > time - FEED_S).map((o) => ({ time: o.destroyed_at as number, kind: "objective" as const, o })),
  ].sort((a, b) => b.time - a.time);
  const fight = report.fights.find((f) => time >= f.start && time <= f.end);
  const jump = (to: number) => {
    setTime(Math.max(0, Math.min(duration, to)));
  };
  const followed = follow !== null ? frames.find((f) => f.slot === follow) : undefined;
  const trail =
    followed &&
    Array.from({ length: Math.floor(TRAIL_S / step) }, (_, n) => time - n * step)
      .filter((w) => w >= 0)
      .map((w) => at(followed, w))
      .filter((p) => p.hp > 0);

  return (
    <div className="replay">
      <div className="replay-stage">
        <svg className="map" viewBox={`0 0 ${SIZE} ${SIZE}`}>
          <image href={report.map.image} width={SIZE} height={SIZE} className="map-image-bright" />
          {report.objectives
            .filter((o) => o.left !== null && o.top !== null)
            .map((o, i) => {
              const down = o.destroyed_at !== null && o.destroyed_at <= time;
              const x = (o.left as number) * SIZE;
              const y = (o.top as number) * SIZE;
              return (
                <g key={i} opacity={down ? 0.35 : 1}>
                  <rect x={x - 9} y={y - 9} width={18} height={18} rx={3} fill={down ? "#222" : TEAM_COLOR[o.team]} stroke="#000" strokeWidth={2}>
                    <title>{o.name}</title>
                  </rect>
                  {down && <path d={`M${x - 7} ${y - 7} L${x + 7} ${y + 7} M${x - 7} ${y + 7} L${x + 7} ${y - 7}`} stroke="#ff5a52" strokeWidth={3} />}
                </g>
              );
            })}
          {trail && trail.length > 1 && (
            <polyline points={trail.map((p) => `${p.x},${p.y}`).join(" ")} fill="none" stroke="#fff" strokeOpacity={0.5} strokeWidth={4} strokeDasharray="6 6" />
          )}
          {deaths
            .filter((d) => d.time <= time && d.time > time - FEED_S)
            .map((d, i) => {
              const x = d.left * SIZE;
              const y = d.top * SIZE;
              return (
                <g key={i} stroke="#ff5a52" strokeWidth={4} opacity={1 - (time - d.time) / FEED_S}>
                  <line x1={x - 10} y1={y - 10} x2={x + 10} y2={y + 10} />
                  <line x1={x - 10} y1={y + 10} x2={x + 10} y2={y - 10} />
                </g>
              );
            })}
          {frames.map((f) => {
            const p = bySlot.get(f.slot);
            if (!p) return null;
            const now = at(f, time);
            if (now.hp <= 0) return null;
            const big = f.slot === follow;
            const r = big ? 30 : 22;
            return (
              <g key={f.slot} transform={`translate(${now.x} ${now.y})`} onClick={() => setFollow(big ? null : f.slot)} style={{ cursor: "pointer" }}>
                <title>
                  {p.hero_name}
                  {p.player_name ? ` (${p.player_name})` : ""} · {t("{n}% health", { n: now.hp })}
                </title>
                <clipPath id={`replay-clip-${f.slot}`}>
                  <circle r={r - 3} />
                </clipPath>
                <circle r={r} fill="#111" stroke={TEAM_COLOR[p.team]} strokeWidth={big ? 6 : 4} />
                <image href={p.hero_icon} x={-(r - 3)} y={-(r - 3)} width={(r - 3) * 2} height={(r - 3) * 2} clipPath={`url(#replay-clip-${f.slot})`} />
                <rect x={-r} y={r + 3} width={r * 2} height={6} fill="#000" opacity={0.7} />
                <rect x={-r} y={r + 3} width={(r * 2 * now.hp) / 100} height={6} fill={now.hp > 50 ? "#7cb342" : now.hp > 20 ? "#f2b84b" : "#ff5a52"} />
              </g>
            );
          })}
        </svg>
        <div className="replay-feed">
          <div className="replay-clock">
            <b>{clock(time)}</b>
            {fight && <span className="replay-fight">{t("Team fight")}</span>}
          </div>
          {feed.map((e, i) =>
            e.kind === "death" ? (
              <div key={i} className="replay-event">
                <small>{clock(e.time)}</small>
                <img src={bySlot.get(e.d.killer)?.hero_icon} alt="" style={{ borderColor: TEAM_COLOR[bySlot.get(e.d.killer)?.team ?? 0] }} />
                <span>›</span>
                <img src={bySlot.get(e.d.victim)?.hero_icon} alt="" style={{ borderColor: TEAM_COLOR[bySlot.get(e.d.victim)?.team ?? 0] }} />
              </div>
            ) : (
              <div key={i} className="replay-event">
                <small>{clock(e.time)}</small>
                <span style={{ color: TEAM_COLOR[1 - e.o.team] }}>{e.o.name}</span>
              </div>
            ),
          )}
          {feed.length === 0 && <div className="muted">{t("Kills and fallen objectives show up here as they happen.")}</div>}
        </div>
      </div>
      <div className="replay-controls">
        <b className="replay-time">{clock(time)}</b>
        <button onClick={() => (time >= duration ? (setTime(0), setPlaying(true)) : setPlaying(!playing))}>{playing ? t("Pause") : t("Play")}</button>
        <button className="ghost small" onClick={() => jump(time - 10)}>
          −10s
        </button>
        <button className="ghost small" onClick={() => jump(time + 10)}>
          +10s
        </button>
        <span className="replay-speeds">
          {SPEEDS.map((s) => (
            <button key={s} className={"ghost small" + (s === speed ? " on" : "")} onClick={() => setSpeed(s)} title={t("{n} match seconds a second", { n: s })}>
              {s}×
            </button>
          ))}
        </span>
        <span className="muted">{follow !== null ? t("Following {hero}. Click a hero on the map to follow another, or the same one to stop.", { hero: bySlot.get(follow)?.hero_name ?? "?" }) : t("Click a hero on the map to follow them.")}</span>
      </div>
      <div className="replay-timeline">
        <div className="replay-track">
          {report.fights.map((f, i) => (
            <span key={i} className="replay-band" style={{ left: `${(f.start / duration) * 100}%`, width: `${(Math.max(f.end - f.start, 5) / duration) * 100}%` }} title={t("Team fight {n} at {time}", { n: i + 1, time: clock(f.start) })} onClick={() => jump(f.start - LEAD_IN_S)} />
          ))}
          {fallen.map((o, i) => (
            <span
              key={"o" + i}
              className="replay-mark objective"
              style={{ left: `${((o.destroyed_at as number) / duration) * 100}%`, background: TEAM_COLOR[1 - o.team] }}
              title={`${clock(o.destroyed_at as number)} · ${o.name}`}
              onClick={() => jump((o.destroyed_at as number) - LEAD_IN_S)}
            />
          ))}
          {deaths.map((d, i) => {
            const victim = bySlot.get(d.victim);
            const mine = follow !== null && (d.victim === follow || d.killer === follow);
            return (
              <span
                key={"d" + i}
                className={"replay-mark death" + (mine ? " mine" : "")}
                style={{ left: `${(d.time / duration) * 100}%`, background: TEAM_COLOR[victim?.team ?? 0] }}
                title={`${clock(d.time)} · ${t("{a} killed by {b}", { a: victim?.hero_name ?? "?", b: bySlot.get(d.killer)?.hero_name ?? "?" })}`}
                onClick={() => {
                  jump(d.time - LEAD_IN_S);
                  setPlaying(true);
                }}
              />
            );
          })}
          <span className="replay-head" style={{ left: `${(time / duration) * 100}%` }} />
        </div>
        <input type="range" min={0} max={duration} step={1} value={Math.round(time)} onChange={(e) => jump(Number(e.target.value))} />
        <div className="muted">
          {t("Marks under the bar: deaths (in the colour of the team that lost the player), fallen objectives (in the colour of the team that took them) and team fights. Click one to watch it from {n} seconds before.", { n: LEAD_IN_S })}
        </div>
      </div>
    </div>
  );
}
