import { invoke } from "@tauri-apps/api/core";

export type Category = "weapon" | "vitality" | "spirit";

export interface Scale {
  class: string;
  stat: string;
  coef: number;
}

export interface Prop {
  key: string;
  label: string;
  value: number;
  postfix: string;
  section: string;
  modifier: string | null;
  scale: Scale | null;
}

export interface Item {
  id: number;
  class_name: string;
  name: string;
  icon: string;
  category: Category;
  tier: number;
  cost: number;
  active: boolean;
  components: string[];
  props: Prop[];
  imbue: string;
  description: string;
}

export interface Hero {
  id: number;
  class_name: string;
  name: string;
  icon: string;
  card: string;
  hero_type: string;
  levels: { level: number; required_souls: number }[];
  invest: Record<Category, InvestTier[]>;
}

export interface InvestTier {
  threshold: number;
  bonus: number;
}

export interface GameData {
  client_version: number | null;
  fetched_at: number;
  heroes: Hero[];
  items: Item[];
}

export interface Target {
  health: number;
  bullet_resist: number;
  spirit_resist: number;
}

export interface Build {
  hero: string;
  purchases: string[];
  upto: number | null;
  souls: number | null;
  active: string[];
  ability_tiers: number[];
  target: Target;
  /** Hero whose ultimate Sinclair has copied. */
  copied_ult: string | null;
  /** Imbue items: item class name -> index of the ability. */
  imbues: Record<string, number>;
}

export interface Term {
  label: string;
  value: number;
  unit: string;
}

export type Group = "weapon" | "vitality" | "spirit" | "target" | "other";

export interface Stat {
  key: string;
  label: string;
  group: Group;
  /** Infinite values arrive as null in JSON. */
  value: number | null;
  unit: string;
  formula: string;
  terms: Term[];
}

export interface ScaledProp {
  key: string;
  label: string;
  value: number;
  unit: string;
  formula: string;
  vs_target: number | null;
  note: string | null;
}

export interface AbilitySheet {
  class_name: string;
  name: string;
  icon: string;
  tier: number;
  max_tier: number;
  props: ScaledProp[];
  imbued: string[];
  slot_class: string;
  /** The game's own description of the ability and its one-line summary. */
  description: string;
  quip: string;
  /** What each upgrade tier changes, in tier order. */
  upgrade_notes: string[];
}

export interface SuggestedBuild {
  /** The playstyle's key and name. */
  key: string;
  name: string;
  goal: string;
  /** Every item in the order it was bought, sold ones included. */
  items: {
    item: string;
    name: string;
    icon: string;
    category: Category;
    cost: number;
    /** Has to be used to do its main thing; used once in the rotation. */
    active: boolean;
    /** Part of what it gives depends on a condition; that part counts for half. */
    conditional: boolean;
    description: string;
    stage: number;
    share: number;
    sold: boolean;
    upgraded: boolean;
  }[];
  /** The stages of the match the build was put together in; damage is per window. */
  stages: { name: string; souls: number; max_cost: number; min_cost: number; spent: number; sold: string[]; damage: number[]; effective_health: number }[];
  /** What the build ends with, in the order it was bought. */
  purchases: string[];
  spent: number;
  budget: number;
  /** Damage in every window, without items and with the build. */
  damage_before: number[];
  damage_after: number[];
  effective_health_before: number;
  effective_health_after: number;
}

export interface Suggestions {
  /** The damage windows by name, in the order every damage list uses. */
  windows: string[];
  /** What the hero does in each window without items. */
  bare: number[];
  ability_share: number;
  /** The souls each stage ends at, and how many matches they were read from (0: built-in defaults). */
  stage_souls: { lane: number; mid: number; late: number; matches: number; players: number };
  playstyles: { key: string; name: string; goal: string }[];
  builds: SuggestedBuild[];
}

export interface Step {
  item: string;
  name: string;
  paid: number;
  total_spent: number;
}

export interface Sheet {
  hero: string;
  level: number;
  boons: number;
  souls: number;
  spent: number;
  spent_weapon: number;
  spent_vitality: number;
  spent_spirit: number;
  ability_points: number;
  ability_points_used: number;
  slots_used: number;
  slot_limit: number;
  owned: string[];
  steps: Step[];
  errors: string[];
  stats: Stat[];
  abilities: AbilitySheet[];
  item_effects: { item: string; name: string; props: ScaledProp[] }[];
}

export interface Ranked {
  item: string;
  name: string;
  category: Category;
  cost: number;
  paid: number;
  delta: number;
  delta_pct: number;
  per_1k: number;
}

export type Action =
  | { kind: "shoot"; seconds: number }
  | { kind: "melee"; heavy: boolean }
  | { kind: "ability"; index: number; duration: number | null; hits: number | null }
  | { kind: "item"; item: string }
  | { kind: "condition"; item: string; count: number | null }
  | { kind: "wait"; seconds: number };

export interface Combo {
  actions: Action[];
  headshot_rate: number;
  /** Own health at the start of the combo, % of max. */
  own_health: number;
}

export type DamageKind = "bullet" | "spirit" | "melee" | "pure";

export interface ComboResult {
  total: number;
  duration: number;
  full_duration: number;
  dps: number;
  target_health: number;
  kill_time: number | null;
  sources: { source: string; kind: DamageKind; damage: number }[];
  events: { time: number; source: string; detail: string; damage: number | null; heal: number | null }[];
  abilities: { name: string; castable: boolean; lines: string[] }[];
  passives: string[];
  usable_items: { item: string; name: string; description: string; condition: boolean; max_count: number | null; unit: string }[];
  unsupported: string[];
  healing: number;
  heal_sources: { source: string; amount: number }[];
  own_max_health: number;
  own_health_start: number;
  own_health_end: number;
}

export const api = {
  getData: () => invoke<GameData>("get_data"),
  refreshData: () => invoke<GameData>("refresh_data"),
  compute: (build: Build) => invoke<Sheet>("compute", { build }),
  popularBuild: (heroId: number) => invoke<PopularReport>("popular_build", { heroId }),
  masterBuild: (myHero: string, allies: number[], enemies: number[], lane: number[], focus: string, damage: string, bought: [string, number][] | null) =>
    invoke<MasterReport>("master_build", { myHero, allies, enemies, lane, focus, damage, bought }),
  riftComeback: (minute: number, ownSouls: number, enemySouls: number) =>
    invoke<RiftComeback>("rift_comeback", { minute, ownSouls, enemySouls }),
  suggestBuilds: (build: Build, styles: string[]) => invoke<Suggestions>("suggest_builds", { build, styles }),
  rankNext: (build: Build, metric: string, conditional: boolean) =>
    invoke<Ranked[]>("rank_next", { build, metric, conditional }),
  simulateCombo: (build: Build, combo: Combo) => invoke<ComboResult>("simulate_combo", { build, combo }),
  saveBuild: (name: string, build: Build) => invoke<void>("save_build", { name, build }),
  listBuilds: () => invoke<string[]>("list_builds"),
  loadBuild: (name: string) => invoke<Build>("load_build", { name }),
  deleteBuild: (name: string) => invoke<void>("delete_build", { name }),
  fetchMatch: (matchId: number) => invoke<MatchReport>("fetch_match", { matchId }),
  matchHistory: (accountId: string) => invoke<HistoryEntry[]>("match_history", { accountId }),
  findLocalMatches: () => invoke<LocalScan>("find_local_matches"),
  analyzeHistory: (accountId: string, count: number) => invoke<{ rows: MatchSummary[]; skipped: number }>("analyze_history", { accountId, count }),
};

export function fmt(v: number | null | undefined, digits = 2): string {
  if (v === null || v === undefined || !Number.isFinite(v)) return "∞";
  return v.toLocaleString("en-US", { maximumFractionDigits: digits });
}

export const CATEGORY_LABEL: Record<Category, string> = {
  weapon: "Weapon",
  vitality: "Vitality",
  spirit: "Spirit",
};

export interface MatchSample {
  time: number;
  level: number;
  net_worth: number;
  kills: number;
  deaths: number;
  assists: number;
  hero_damage: number;
  damage_taken: number;
  healing: number;
  objective_damage: number;
  creep_damage: number;
  neutral_damage: number;
  mitigated: number;
  last_hits: number;
  denies: number;
  souls: { name: string; amount: number }[];
  game_health: number;
  game_spirit: number;
  game_weapon: number;
  calc_health: number | null;
  calc_spirit: number | null;
  calc_weapon: number | null;
  /** The calculator's resists for the build at this moment, percent. */
  bullet_resist: number;
  spirit_resist: number;
  build: Build;
}

export interface MatchPlayer {
  slot: number;
  account_id: number;
  /** Steam name; empty when it could not be looked up. */
  player_name: string;
  team: number;
  won: boolean;
  hero: string;
  hero_name: string;
  hero_icon: string;
  /** Frontliner, Support, Assassin, Marksman or Caster, and what decided it. */
  role: string;
  role_why: string;
  kills: number;
  deaths: number;
  assists: number;
  net_worth: number;
  level: number;
  last_hits: number;
  denies: number;
  hero_damage: number;
  damage_taken: number;
  healing: number;
  objective_damage: number;
  creep_damage: number;
  neutral_damage: number;
  mitigated: number;
  game_mvp_rank: number | null;
  macro_stats: { name: string; amount: number }[];
  souls: { name: string; amount: number }[];
  kills_at: { time: number; other: number }[];
  deaths_at: { time: number; other: number }[];
  heal_prevented: number;
  self_healing: number;
  teammate_support: number;
  close_calls: number;
  revives_at: number[];
  late_fights_skipped: number;
  objectives_taken: { time: number; name: string }[];
  fights_won: number;
  fights_lost: number;
  first_deaths: number;
  /** Against the lane opponents in the laning phase. */
  lane_damage_dealt: number;
  lane_damage_taken: number;
  lane_kills_on: number;
  lane_deaths_to: number;
  roams: { start: number; seconds: number; lane: number; name: string; enemy_deaths: number; kills: number; died: boolean }[];
  /** Clearly won team fights the team took nothing from, after which this player went to neutral camps. */
  wins_farmed: number;
  /** Mid boss kills the team went to that the player was alive for, and how many of those the player was at. */
  mid_boss_alive: number;
  mid_boss_present: number;
  lane_detail: LaneDetail;
  /** Titles for leading the match in something, good and bad. */
  titles: { name: string; what: string; value: string; tone: "good" | "bad" }[];
  /** The parts of the score the most points came from, and the ones the most were lost in. */
  strengths: { name: string; amount: number }[];
  gaps: { name: string; amount: number }[];
  /** Temporary power-ups picked up under the bridges, by kind, and the running count at every snapshot. */
  bridge_buffs: { name: string; amount: number }[];
  bridge_buffs_at: number[];
  /** Deaths outside a team fight with no teammate near, to several enemies or deep on the enemy's side. */
  caught_deaths: number;
  /** Deaths outside a team fight that left the team short for a fight or a mid boss. */
  costly_deaths: number;
  lane: number;
  lane_opponents: number[];
  lane_time: number;
  lane_souls: number;
  lane_souls_diff: number;
  lane_last_hits: number;
  lane_denies: number;
  lane_kills: number;
  lane_deaths: number;
  lane_hero_damage: number;
  last_hit_pct: number;
  trooper_last_hits: number;
  possible_last_hits: number;
  avg_unspent_souls: number;
  avg_unspent_ap: number;
  dead_seconds: number;
  souls_lost_on_death: number;
  time_split: { name: string; amount: number }[];
  fights_possible: number;
  fights_joined: number;
  fights_missed: number;
  insights: { tone: "good" | "bad" | "info"; title: string; text: string }[];
  /** When the match started calling for an item, and when it was bought. */
  build_timing: { tone: "good" | "bad" | "info"; need: string; needed_at: number | null; bought_at: number | null; item: string; text: string; credit: number | null; advice: BuildAdvice[]; spent_on: string[] }[];
  /** Why each item that answers enemies was bought: whom it was aimed at when it was bought. */
  item_reasons: { time: number; item: string; icon: string; lane: boolean; tone: "good" | "bad" | "info"; effect: string; text: string }[];
  /** How well the build answered the enemy team, percent; null when the match called for nothing. */
  counter_score: number | null;
  enabled: { source: string; effect: string; amplified: number; bonus: number; by_teammate: { name: string; amount: number }[]; needs: string }[];
  enabled_total: number;
  hero_shots: number;
  hero_hits: number;
  hero_headshots: number;
  purchases: { time: number; item: string; name: string; icon: string; category: Category; cost: number; sold_at: number | null }[];
  abilities: { time: number; index: number; name: string; icon: string; tier: number }[];
  timeline: MatchSample[];
  damage_sources: { name: string; amount: number }[];
  heal_sources: { name: string; amount: number }[];
  unknown_items: number;
  build: Build;
}

export interface MatchReport {
  match_id: number;
  start_time: number;
  duration_s: number;
  winning_team: number;
  players: MatchPlayer[];
  objectives: {
    team: number;
    name: string;
    lane: number | null;
    destroyed_at: number | null;
    /** Enemy players who were at it when it fell. */
    takers: number[];
    player_damage: number;
    trooper_damage: number;
    left: number | null;
    top: number | null;
  }[];
  mid_boss: MidBossKill[];
  deaths: MatchDeath[];
  lanes: MatchLane[];
  /** The match in a few sentences. */
  story: { tone: "good" | "bad" | "info"; title: string; text: string }[];
  ranking: { slot: number; score: number; parts: { label: string; value: number; points: number; max_points: number }[] }[];
  fights: TeamFight[];
  duels: { from: number; to: number; damage: number; kills: number }[];
  map: {
    image: string;
    step: number;
    paths: { slot: number; points: { time: number; left: number; top: number }[] }[];
    deaths: { time: number; victim: number; killer: number; left: number; top: number }[];
    fights: { time: number; left: number; top: number }[];
  };
}

export interface MatchSummary {
  match_id: number;
  start_time: number;
  duration_s: number;
  hero_name: string;
  hero_icon: string;
  won: boolean;
  kills: number;
  deaths: number;
  assists: number;
  net_worth: number;
  score: number;
  place: number;
  parts: { label: string; value: number; points: number; max_points: number }[];
  hero_damage: number;
  objective_damage: number;
  lane_souls_diff: number;
  last_hit_pct: number;
  fights_joined: number;
  fights_possible: number;
  late_fights_skipped: number;
  avg_unspent_souls: number;
  dead_seconds: number;
  enabled_total: number;
  role: string;
  /** Late-game team fights with the match even that the player stayed out of. */
  even_fights_skipped: number;
  caught_deaths: number;
  costly_deaths: number;
  /** Mid boss kills the player was alive for, and how many of those the player was at the pit for. */
  mid_boss_alive: number;
  mid_boss_present: number;
  counter_score: number | null;
  /** What the match called for that came very late or never, by name. */
  late_needs: string[];
  titles: string[];
  bad_titles: string[];
  lane_damage_dealt: number;
  lane_damage_taken: number;
  lane_kills_on: number;
  lane_deaths_to: number;
  roams: number;
  roams_paid: number;
  wins_farmed: number;
}

export interface FightSide {
  present: number[];
  absent: { slot: number; doing: string; distance: number; missed: boolean }[];
  dead: number[];
  deaths: number[];
}

export interface TeamFight {
  start: number;
  end: number;
  sides: FightSide[];
  /** [destroying team, objective name] */
  objectives: [number, string][];
  late: boolean;
  kills: { time: number; victim: number; killer: number }[];
  /** Souls of the players present on each side when the fight started. */
  souls: number[];
  winner: number | null;
  /** A late-game fight with the match still in the balance. */
  even: boolean;
  /** Walkers each team still had, and each team's souls, when the fight started. */
  walkers: number[];
  team_souls: number[];
  /** What the winner made of a fight it clearly won; null without a clear winner. */
  follow: FightFollowUp | null;
}

export interface HistoryEntry {
  match_id: number;
  start_time: number;
  duration_s: number;
  hero_name: string;
  hero_icon: string;
  kills: number;
  deaths: number;
  assists: number;
  net_worth: number;
  won: boolean;
}

export interface LocalScan {
  folder: string;
  found: boolean;
  files: number;
  matches: { match_id: number; seen_at: number; has_replay: boolean }[];
}

/** A mid boss kill: who was at the pit and what the rejuvenator was turned into. */
export interface MidBossKill {
  time: number;
  team_killed: number;
  team_claimed: number;
  /** One team killed the boss, the other took the rejuvenator. */
  stolen: boolean;
  sides: FightSide[];
  /** Both teams had at least two players at the pit. */
  contested: boolean;
  late: boolean;
  even: boolean;
  walkers: number[];
  team_souls: number[];
  /** Seconds the rejuvenator lasted, and what happened in them. */
  window_s: number;
  taken: { time: number; name: string }[];
  lost: { time: number; name: string }[];
  kills: number[];
  lead_change: number;
}

/** An item that would have answered a need, with an estimate of what it would have changed. */
export interface BuildAdvice {
  item: string;
  icon: string;
  cost: number;
  effect: string;
  estimate: number | null;
  note: string;
}

/** One death: who was around, where it happened, and what the team then did without the player. */
export interface MatchDeath {
  time: number;
  slot: number;
  killer: number;
  enemies: number;
  allies: number;
  nearest_ally_m: number | null;
  /** Meters past the middle of the map toward the enemy base; negative on the own side. */
  depth_m: number;
  alone: boolean;
  deep: boolean;
  in_fight: boolean;
  caught: boolean;
  cost: string[];
  left: number;
  top: number;
}

/** A lane: who played it and how the laning phase went between the two sides. */
export interface MatchLane {
  lane: number;
  /** West, Middle or East on the minimap. */
  name: string;
  sides: {
    players: number[];
    souls: number;
    last_hits: number;
    denies: number;
    kills: number;
    damage: number;
    guardian_lost_at: number | null;
    guardian_first_hit: number | null;
    guardian_player_damage: number;
    guardian_trooper_damage: number;
    souls_at: number[];
  }[];
  winner: number | null;
}

/** The time right after a team fight one team clearly won. */
export interface FightFollowUp {
  window_s: number;
  lead: number;
  taken: string[];
  boss: boolean;
  farming: number[];
}

/** One player's laning phase in numbers, from the game's snapshots up to 9:00. */
export interface LaneDetail {
  souls_at: number[];
  trooper_last_hits: number;
  possible_last_hits: number;
  denies: number;
  deny_souls: number;
  kills: number;
  deaths: number;
  assists: number;
  hero_damage: number;
  damage_taken: number;
  healing: number;
  guardian_damage: number;
  camp_kills: number;
  camp_souls: number;
  box_souls: number;
  statue_buffs: number;
  souls_lost_on_death: number;
  accuracy_pct: number;
}

/** The Unstable Rift's comeback bonus for the team behind in net worth. */
export interface RiftComeback {
  behind_pct: number;
  /** Share of the full bonus, 0-1: it is full at 15% behind. */
  scale: number;
  max_resist: number;
  /** Bullet, spirit and debuff resist the trailing team gets inside the rift. */
  resist: number;
  capture_trailing: number;
  capture_leading: number;
  troopers_if_trailing_wins: number;
  own_team_trails: boolean;
}

/** An item as players build it: how often it is bought and kept, and when. */
export interface PopularItem {
  item: string;
  name: string;
  icon: string;
  category: Category;
  cost: number;
  active: boolean;
  description: string;
  bought_pct: number;
  kept_pct: number;
  win_pct: number;
  buy_minute: number;
}

export interface PopularBuild {
  hero: string;
  hero_name: string;
  matches: number;
  core: PopularItem[];
  situational: PopularItem[];
}

export interface PopularReport {
  build: PopularBuild;
  days: number;
  /** The rank the numbers come from, e.g. Eternus. */
  rank: string;
}

export interface CounterAdvice {
  item: string;
  name: string;
  icon: string;
  category: Category;
  cost: number;
  active: boolean;
  description: string;
  /** What it does for its job, e.g. -35% healing on whoever it hits. */
  effect: string;
  score: number;
  reasons: string[];
  when: string;
  against: string[];
}

/** What an enemy hero brings, from its kit and its usual build. */
export interface EnemyProfile {
  hero: string;
  hero_name: string;
  weapon_pct: number;
  heals: string[];
  mobility: string[];
  lockdown: string[];
  resists: string[];
  flying: string[];
}

/** A job items do against the enemy team: why the team calls for it, and the best items for it by price. */
export interface RoleAdvice {
  key: string;
  title: string;
  need: string;
  weight: number;
  items: CounterAdvice[];
}

/** An item for a counter slot, with every job it does against the team. */
export interface CounterPick {
  advice: CounterAdvice;
  /** Already in the player's usual build. */
  in_core: boolean;
  jobs: string[];
}

/** One enemy: what it threatens and which planned item answers it, stage by stage. */
export interface HeroAnswer {
  hero: string;
  hero_name: string;
  in_lane: boolean;
  threats: string[];
  answers: string[];
}

/** The two counter items for one stage of the game. */
export interface CounterStage {
  key: string;
  title: string;
  note: string;
  picks: CounterPick[];
  alternatives: CounterPick[];
}

/** An item of the master build, with the three things it was weighed on. */
export interface MasterItem {
  item: string;
  name: string;
  icon: string;
  category: Category;
  cost: number;
  active: boolean;
  description: string;
  phase: number;
  paid: number;
  score: number;
  popular_pct: number;
  win_pct: number;
  win_pp: number;
  engine_pct: number;
  jobs: string[];
  notes: string[];
  sold: boolean;
  upgrades: string | null;
  upgraded: boolean;
  /** The ability it is imbued into. */
  imbued: string | null;
}

/** An item weighed in a phase and not bought. */
export interface Considered {
  item: string;
  name: string;
  icon: string;
  category: Category;
  cost: number;
  score: number;
  popular_pct: number;
  win_pct: number;
  win_pp: number;
  engine_pct: number;
  counter_pct: number;
  jobs: string[];
  notes: string[];
  left_out: boolean;
}

export interface MasterPhase {
  key: string;
  title: string;
  souls_from: number;
  souls_to: number;
  sold: string[];
  considered: Considered[];
}

/** An item a player bought in a match, judged on the same terms as the master build's items. */
export interface JudgedItem {
  item: string;
  name: string;
  icon: string;
  category: Category;
  cost: number;
  active: boolean;
  description: string;
  phase: number;
  net_worth: number;
  popular_pct: number;
  win_pct: number;
  win_pp: number;
  jobs: string[];
  notes: string[];
  in_master: boolean;
}

/** A player of a match handed to the master builder: who played what, and what the player bought. */
export interface MatchPreset {
  /** Match and slot, so the same player is not built twice. */
  key: string;
  match_id: number;
  player_name: string;
  /** Hero class names. */
  hero: string;
  allies: string[];
  enemies: string[];
  lane: string[];
  net_worth: number;
  won: boolean;
  /** Every purchase with the player's net worth at the time. */
  bought: { item: string; net_worth: number; time: number }[];
}

export interface MasterBuild {
  hero: string;
  hero_name: string;
  focus: string;
  focus_name: string;
  focus_goal: string;
  style: string;
  phases: MasterPhase[];
  items: MasterItem[];
  purchases: string[];
  imbues: Record<string, number>;
  notes: string[];
  judged: JudgedItem[];
}

export interface MasterReport {
  master: MasterBuild;
  builds: PopularBuild[];
  plan: { enemies: EnemyProfile[]; roles: RoleAdvice[]; own_escape: string[];
    stages: CounterStage[];
    others: CounterPick[];
    allies: EnemyProfile[];
    heroes: HeroAnswer[];
    team: string[];
  };
  days: number;
  rank: string;
}
