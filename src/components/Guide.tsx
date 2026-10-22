import { ReactNode, useEffect, useState } from "react";

const SEEN_KEY = "guide-seen";

export type GuideTopic = "start" | "build" | "generator" | "draft" | "match" | "setup";

interface Step {
  title: string;
  body: ReactNode;
}

const TOPICS: { key: GuideTopic; title: string; intro: string; steps: Step[] }[] = [
  {
    key: "start",
    title: "Getting started",
    intro: "Deadlock BCMA works out builds from the game's own numbers and from what top players buy, and studies your matches. Four screens, on the left:",
    steps: [
      { title: "Build maker", body: "Put a build together item by item and see every number it gives: damage, health, resists, a fight against a target, the order to upgrade abilities in." },
      { title: "Build generator", body: "Let the app build for you: the popular build of top players, or the Master builder, which builds for one match against the enemy team you give it." },
      { title: "Draft", body: "During hero selection: which heroes do well into the enemy picks and with your team, and what the enemy will likely ban." },
      { title: "Match analysis", body: "Open any match by its number, or your recent ones by your Steam account: how it went, why you died, what the build lacked." },
      {
        title: "First thing to set up",
        body: (
          <>
            Add <code>-condebug</code> to Deadlock's launch options in Steam (Properties, Launch Options). The app then reads the match you are in from the game's
            console log, finds your account on its own, and can build for your match as it starts. See <b>Setup</b> for the rest.
          </>
        ),
      },
      { title: "Open this guide again", body: "The Guide button at the bottom of the side bar opens it any time, and the ? next to each screen's title opens it at that screen." },
    ],
  },
  {
    key: "build",
    title: "Build maker",
    intro: "Your build, with the game's numbers worked out for it.",
    steps: [
      { title: "Pick a hero and set the souls", body: "Click the hero at the top left to change it. The souls set the level and the ability points; leave them empty to follow what the items cost." },
      {
        title: "Buy items",
        body: "Click an item in the shop to buy it, and again to sell it. Search finds an item by name, stat or what it does. The bar at the top lists the build in order: there an item's effects that only work sometimes can be switched on, an imbued item can be put into an ability, and items can be moved or removed.",
      },
      {
        title: "Read the tabs on the right",
        body: (
          <>
            <b>Stats</b>: every stat with where it comes from, the next item worth buying, and the upgrade order for your abilities. <b>Combo</b>: line up abilities,
            shooting, melee and items and see what they deal to a target you set, step by step. <b>Misc</b>: how fast the build farms the jungle, and the Unstable Rift. <b>Compare</b>: set this build
            against a saved one or a friend's build code.
          </>
        ),
      },
      { title: "Buy, upgrade or sell", body: "At the bottom of the Stats tab, the advisor says which purchase, upgrade or swap adds the most to your fight, measured, not guessed." },
      { title: "Save and share", body: "Save builds by name, or copy the build code (BCMA1.…) and send it: pasting a code on the bar loads that build." },
    ],
  },
  {
    key: "generator",
    title: "Build generator",
    intro: "Builds made for you: the popular one, or one made for a match.",
    steps: [
      { title: "Popular", body: "What top players of the hero buy, phase by phase, with how often and how often they win with it. The core build is what nearly all of them buy." },
      {
        title: "Master builder",
        body: "Enter the enemy team (and your team, and who is in your lane, if you know them). The app builds phase by phase: what top players buy, what each item adds to your own fight, and what answers the enemy team.",
      },
      {
        title: "Focus and damage",
        body: "Focus is your role: carry, tempo, support or frontline (auto picks one from the teams). Damage is gun, spirit, hybrid or melee (auto follows what top players of the hero play). Pick spirit and the build is made for spirit damage even on a gun hero.",
      },
      {
        title: "Gold COUNTER items",
        body: "Items marked COUNTER are there for the enemy team: healing reduction against healers, an answer to their stuns, bullet resist against guns. They come at the end of their phase.",
      },
      {
        title: "Why not something else?",
        body: "Under each phase, the best items not bought and why: what they lost to, by popularity, by what they add to you, by their answer to the enemy team, or by price.",
      },
      {
        title: "Your live match",
        body: "With -condebug set, Load my live match fills both teams from the match you are playing, about a minute after it starts. Watch for my match does it on its own every match while the screen is open. The lobby list shows every player's rank, record on the hero and party.",
      },
      {
        title: "How items are measured",
        body: "Items are measured on a team fight: abilities that hit an area count for every enemy they hit (measured on match replays), stuns count for the time the enemy cannot fight back.",
      },
    ],
  },
  {
    key: "draft",
    title: "Draft",
    intro: "Which hero to pick while the draft is on.",
    steps: [
      { title: "Enter the picks", body: "Add the enemy picks and your team's picks as they come in; bans too. Load my live match fills them from the game when -condebug is set." },
      { title: "Read the list", body: "Heroes are ranked by how they do into the enemy picks and with your team, from top players' matches. Each line says what it counts, with how many matches stand behind it." },
      { title: "Your own record", body: "Your Steam account ID adds your own record on each hero to the ranking. It is optional." },
      { title: "Then build", body: "Open the Master builder from the draft for the hero you pick: the teams carry over." },
    ],
  },
  {
    key: "match",
    title: "Match analysis",
    intro: "What happened in a match and what to do differently.",
    steps: [
      { title: "Open a match", body: "Type a match number, or your Steam account ID for your recent matches. Matches stay on this computer once opened." },
      { title: "The match in short", body: "The story first: the lanes, the soul lead, where it turned, who carried. Then each player: build timing, team fights joined and missed, map play, objectives, titles." },
      {
        title: "Deaths",
        body: "Every death with who was around and where. Caught alone means outside a team fight with no teammate near; outnumbered in lane is a gank in your own lane, not your mistake.",
      },
      {
        title: "Why did you die?",
        body: "Each death is a duel: how fast the killer's full fight kills you, and how fast yours would have killed them, with the builds of the time. The items listed would have turned it or changed it most; ones that only keep you alive a second longer are left out.",
      },
      {
        title: "Against top players",
        body: "Each item you bought, against the minute top players of your hero buy it: early, on time, late or very late, and the items most of them buy that you never did. An item rushed ahead of its time (a counter) pushes back the ones after it; that delay is worked out from your soul income and not held against you.",
      },
      { title: "Why you die over many matches", body: "From the recent matches list: who kills you most, with what kind of damage, and the item that would have helped most often." },
      { title: "Open the build", body: "Open final build in the builder loads a player's build of that match into the Build maker." },
    ],
  },
  {
    key: "setup",
    title: "Setup",
    intro: "What the app needs from your computer and the game.",
    steps: [
      {
        title: "-condebug",
        body: (
          <>
            In Steam, open Deadlock's Properties and add <code>-condebug</code> to Launch Options, then start the game again. The game then writes a console log
            the app reads: the match you are in and the account you play on. Live match, Watch for my match and sharing your matches need it.
          </>
        ),
      },
      { title: "Steam account ID", body: "The Draft and Match analysis screens take your Steam account ID (the short number, or the long 7656… one). With -condebug the app finds it in the log." },
      {
        title: "Share my finished matches",
        body: "At the bottom of the side bar. When a match ends, the app asks deadlock-api for it, which keeps the match and, when Valve kept one, its replay, so it can be studied. Only the match number is sent; untick it to stop.",
      },
      { title: "Updates", body: "The app updates itself and says what is new. Update data, at the bottom of the side bar, fetches the latest heroes and items after a patch." },
    ],
  },
];

/** The guide: how to use each screen, opened on the first start and from the side bar or a screen's ? any time. */
export default function Guide({ open, topic, onClose, onTopic }: { open: boolean; topic: GuideTopic; onClose: () => void; onTopic: (t: GuideTopic) => void }) {
  if (!open) return null;
  const t = TOPICS.find((x) => x.key === topic) ?? TOPICS[0];
  return (
    <div className="overlay" onClick={onClose}>
      <div className="whats-new guide" role="dialog" aria-label="Guide" onClick={(e) => e.stopPropagation()}>
        <div className="whats-new-head">
          <b>Guide</b>
          <button className="ghost small" onClick={onClose}>
            Close
          </button>
        </div>
        <div className="guide-body">
          <nav className="guide-topics">
            {TOPICS.map((x) => (
              <button key={x.key} className={"ghost small" + (x.key === t.key ? " on" : "")} onClick={() => onTopic(x.key)}>
                {x.title}
              </button>
            ))}
          </nav>
          <section className="whats-new-body">
            <h3>{t.title}</h3>
            <p className="muted">{t.intro}</p>
            <ol className="guide-steps">
              {t.steps.map((s) => (
                <li key={s.title}>
                  <b>{s.title}</b>
                  <p>{s.body}</p>
                </li>
              ))}
            </ol>
          </section>
        </div>
      </div>
    </div>
  );
}

/** Whether the guide should open on its own: the first time the app starts. */
export function useFirstStart(open: () => void) {
  useEffect(() => {
    try {
      if (localStorage.getItem(SEEN_KEY)) return;
      localStorage.setItem(SEEN_KEY, "1");
    } catch {
      return;
    }
    open();
  }, []);
}

/** A ? that opens the guide at a screen's topic. */
export function GuideButton({ onOpen }: { onOpen: () => void }) {
  return (
    <button className="ghost small guide-button" onClick={onOpen} title="How to use this screen">
      ?
    </button>
  );
}

export function useGuide(): { open: boolean; topic: GuideTopic; show: (t?: GuideTopic) => void; close: () => void; setTopic: (t: GuideTopic) => void } {
  const [open, setOpen] = useState(false);
  const [topic, setTopic] = useState<GuideTopic>("start");
  return {
    open,
    topic,
    show: (t) => {
      setTopic(t ?? "start");
      setOpen(true);
    },
    close: () => setOpen(false),
    setTopic,
  };
}
