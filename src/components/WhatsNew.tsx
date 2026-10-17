import { ReactNode, useEffect, useState } from "react";
import { getVersion } from "@tauri-apps/api/app";
import changelog from "../../CHANGELOG.md?raw";

const SEEN_KEY = "seen-version";

/** Version numbers compared part by part. */
function newer(a: string, b: string): boolean {
  const x = a.split(".").map(Number);
  const y = b.split(".").map(Number);
  for (let i = 0; i < Math.max(x.length, y.length); i++) {
    if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) > (y[i] ?? 0);
  }
  return false;
}

/** The changelog's sections for the versions after `seen`, up to `current`. */
function sections(seen: string, current: string): { version: string; date: string; body: string }[] {
  const out: { version: string; date: string; body: string }[] = [];
  const re = /^## \[(\d+\.\d+\.\d+)\](?:\s*–\s*(\S+))?\s*$/gm;
  const heads = [...changelog.matchAll(re)];
  heads.forEach((m, i) => {
    const version = m[1];
    if (!newer(version, seen) || newer(version, current)) return;
    const end = heads[i + 1]?.index ?? changelog.search(/^\[\d+\.\d+\.\d+\]:/m);
    out.push({ version, date: m[2] ?? "", body: changelog.slice((m.index ?? 0) + m[0].length, end > 0 ? end : undefined).trim() });
  });
  return out;
}

/** **bold** spans inside a line. */
function inline(text: string): ReactNode[] {
  return text.split(/(\*\*[^*]+\*\*)/).map((part, i) => (part.startsWith("**") ? <b key={i}>{part.slice(2, -2)}</b> : part));
}

/** The small subset of Markdown the changelog uses: headings, numbered items, nested bullets, paragraphs. */
function render(body: string): ReactNode {
  const out: ReactNode[] = [];
  let item: { text: string; sub: string[]; more: string[] } | null = null;
  const flush = () => {
    if (!item) return;
    const it = item;
    out.push(
      <div key={out.length} className="news-item">
        <p>{inline(it.text)}</p>
        {it.sub.length > 0 && (
          <ul>
            {it.sub.map((s, i) => (
              <li key={i}>{inline(s)}</li>
            ))}
          </ul>
        )}
        {it.more.map((m, i) => (
          <p key={i}>{inline(m)}</p>
        ))}
      </div>,
    );
    item = null;
  };
  for (const raw of body.split("\n")) {
    const line = raw.trimEnd();
    if (!line.trim()) continue;
    if (line.startsWith("### ")) {
      flush();
      out.push(<h4 key={out.length}>{line.slice(4)}</h4>);
    } else if (/^\d+\.\s/.test(line)) {
      flush();
      item = { text: line.replace(/^\d+\.\s/, ""), sub: [], more: [] };
    } else if (/^\s+-\s/.test(line) && item) {
      item.sub.push(line.replace(/^\s+-\s/, ""));
    } else if (item && /^\s+/.test(raw)) {
      item.more.push(line.trim());
    } else {
      flush();
      out.push(<p key={out.length}>{inline(line)}</p>);
    }
  }
  flush();
  return out;
}

/** After the app updates itself, what the new version brings, once. */
export default function WhatsNew() {
  const [news, setNews] = useState<{ version: string; date: string; body: string }[]>([]);
  useEffect(() => {
    getVersion()
      .then((current) => {
        let seen: string | null = null;
        try {
          seen = localStorage.getItem(SEEN_KEY);
          localStorage.setItem(SEEN_KEY, current);
        } catch {
          return;
        }
        // A fresh install has nothing to compare with.
        if (seen && newer(current, seen)) setNews(sections(seen, current).reverse());
      })
      .catch(() => {});
  }, []);
  if (news.length === 0) return null;
  const latest = news[news.length - 1];
  return (
    <div className="overlay" onClick={() => setNews([])}>
      <div className="whats-new" role="dialog" aria-label="What's new" onClick={(e) => e.stopPropagation()}>
        <div className="whats-new-head">
          <b>Updated to {latest.version}</b>
          <button className="ghost small" onClick={() => setNews([])}>
            Close
          </button>
        </div>
        <div className="whats-new-body">
          {[...news].reverse().map((n) => (
            <section key={n.version}>
              <h3>
                {n.version} <small className="muted">{n.date}</small>
              </h3>
              {render(n.body)}
            </section>
          ))}
        </div>
      </div>
    </div>
  );
}
