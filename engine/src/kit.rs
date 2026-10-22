//! Turns an ability's numbers into what the combo simulation does with it.
//!
//! `generic` reads the plan from the data by naming conventions; `customize` then corrects the
//! abilities whose real behavior the data alone does not describe (checked against the game's
//! own ability text and deadlock.wiki).

use crate::calc::{AbilitySheet, ScaledProp};
use serde::Serialize;

const FALLBACK_SECONDS: f64 = 3.0;
const MAX_HITS: f64 = 200.0;

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Bullet,
    Spirit,
    Melee,
    /// Ignores resists.
    Pure,
}

impl Kind {
    fn word(self) -> &'static str {
        match self {
            Kind::Bullet => "bullet",
            Kind::Spirit => "spirit",
            Kind::Melee => "melee",
            Kind::Pure => "pure",
        }
    }
}

pub fn n(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// What a debuff does to the target, per stack. Resist values are percentage points removed.
#[derive(Debug, Clone, Copy, Default)]
pub struct Effect {
    pub bullet: f64,
    pub spirit: f64,
    /// Extra spirit damage taken, percent.
    pub spirit_amp: f64,
    /// Extra bullet damage taken, percent.
    pub bullet_amp: f64,
    /// Extra damage of every type taken, percent.
    pub all_amp: f64,
}

impl Effect {
    pub fn is_empty(&self) -> bool {
        self.bullet == 0.0 && self.spirit == 0.0 && self.spirit_amp == 0.0 && self.bullet_amp == 0.0 && self.all_amp == 0.0
    }

    pub fn text(&self) -> String {
        let mut parts = vec![];
        if self.bullet != 0.0 {
            parts.push(format!("-{}% bullet resist", n(self.bullet)));
        }
        if self.spirit != 0.0 {
            parts.push(format!("-{}% spirit resist", n(self.spirit)));
        }
        if self.spirit_amp != 0.0 {
            parts.push(format!("+{}% spirit damage taken", n(self.spirit_amp)));
        }
        if self.bullet_amp != 0.0 {
            parts.push(format!("+{}% bullet damage taken", n(self.bullet_amp)));
        }
        if self.all_amp != 0.0 {
            parts.push(format!("+{}% damage taken", n(self.all_amp)));
        }
        parts.join(", ")
    }
}

/// A temporary bonus to the hero's own weapon.
#[derive(Debug, Clone, Copy, Default)]
pub struct Buff {
    /// Flat bullet damage added after weapon damage scaling, per pellet.
    pub flat_bullet: f64,
    pub fire_rate: f64,
    pub weapon_pct: f64,
    /// Spirit damage added to every shot.
    pub spirit_per_shot: f64,
    /// Spirit damage added to every heavy melee hit.
    pub heavy_melee_spirit: f64,
    /// Spirit power, flat and as a percentage. Abilities cast while it lasts use the higher value.
    pub spirit_power: f64,
    pub spirit_power_pct: f64,
    pub melee_pct: f64,
    /// Share of the damage dealt that returns as health, percent. Separate sources combine multiplicatively.
    pub bullet_lifesteal: f64,
    pub spirit_lifesteal: f64,
    pub melee_lifesteal: f64,
}

impl Buff {
    pub fn is_empty(&self) -> bool {
        self.flat_bullet == 0.0
            && self.fire_rate == 0.0
            && self.weapon_pct == 0.0
            && self.spirit_per_shot == 0.0
            && self.heavy_melee_spirit == 0.0
            && self.spirit_power == 0.0
            && self.spirit_power_pct == 0.0
            && self.melee_pct == 0.0
            && self.bullet_lifesteal == 0.0
            && self.spirit_lifesteal == 0.0
            && self.melee_lifesteal == 0.0
    }

    pub fn add(&mut self, other: &Buff, times: f64) {
        self.flat_bullet += other.flat_bullet * times;
        self.fire_rate += other.fire_rate * times;
        self.weapon_pct += other.weapon_pct * times;
        self.spirit_per_shot += other.spirit_per_shot * times;
        self.heavy_melee_spirit += other.heavy_melee_spirit * times;
        self.spirit_power += other.spirit_power * times;
        self.spirit_power_pct += other.spirit_power_pct * times;
        self.melee_pct += other.melee_pct * times;
        self.bullet_lifesteal += other.bullet_lifesteal * times;
        self.spirit_lifesteal += other.spirit_lifesteal * times;
        self.melee_lifesteal += other.melee_lifesteal * times;
    }

    pub fn text(&self) -> String {
        let mut parts = vec![];
        if self.flat_bullet != 0.0 {
            parts.push(format!("+{} bullet damage", n(self.flat_bullet)));
        }
        if self.weapon_pct != 0.0 {
            parts.push(format!("+{}% weapon damage", n(self.weapon_pct)));
        }
        if self.fire_rate != 0.0 {
            parts.push(format!("+{}% fire rate", n(self.fire_rate)));
        }
        if self.spirit_per_shot != 0.0 {
            parts.push(format!("+{} spirit damage per shot", n(self.spirit_per_shot)));
        }
        if self.heavy_melee_spirit != 0.0 {
            parts.push(format!("+{} spirit damage on heavy melee", n(self.heavy_melee_spirit)));
        }
        if self.spirit_power != 0.0 {
            parts.push(format!("+{} spirit power", n(self.spirit_power)));
        }
        if self.spirit_power_pct != 0.0 {
            parts.push(format!("+{}% spirit power", n(self.spirit_power_pct)));
        }
        if self.melee_pct != 0.0 {
            parts.push(format!("+{}% melee damage", n(self.melee_pct)));
        }
        if self.bullet_lifesteal != 0.0 && self.bullet_lifesteal == self.spirit_lifesteal {
            parts.push(format!("+{}% bullet and spirit lifesteal", n(self.bullet_lifesteal)));
        } else {
            if self.bullet_lifesteal != 0.0 {
                parts.push(format!("+{}% bullet lifesteal", n(self.bullet_lifesteal)));
            }
            if self.spirit_lifesteal != 0.0 {
                parts.push(format!("+{}% spirit lifesteal", n(self.spirit_lifesteal)));
            }
        }
        if self.melee_lifesteal != 0.0 {
            parts.push(format!("+{}% melee lifesteal", n(self.melee_lifesteal)));
        }
        parts.join(", ")
    }
}

/// Extra damage as a percentage of the target's health.
#[derive(Debug, Clone, Copy, Default)]
pub struct Health {
    pub current: f64,
    pub max: f64,
    pub missing: f64,
}

impl Health {
    fn text(&self) -> String {
        let mut parts = vec![];
        if self.current != 0.0 {
            parts.push(format!("{}% of current health", n(self.current)));
        }
        if self.max != 0.0 {
            parts.push(format!("{}% of max health", n(self.max)));
        }
        if self.missing != 0.0 {
            parts.push(format!("{}% of missing health", n(self.missing)));
        }
        parts.join(" + ")
    }

    fn is_empty(&self) -> bool {
        self.current == 0.0 && self.max == 0.0 && self.missing == 0.0
    }
}

/// Health an ability restores to the hero who uses it.
#[derive(Debug, Clone, Default)]
pub struct Heal {
    pub label: String,
    pub flat: f64,
    /// Percent of the hero's own max health.
    pub max_pct: f64,
    /// Percent of the hero's own missing health.
    pub missing_pct: f64,
    /// Seconds the heal is spread over; 0 heals at once.
    pub seconds: f64,
    /// Once per hit of the ability instead of once per cast.
    pub per_hit: bool,
    /// Granted when the target dies instead of on the cast.
    pub on_kill: bool,
}

impl Heal {
    pub fn text(&self) -> String {
        let mut parts = vec![];
        if self.flat != 0.0 {
            parts.push(format!("{} health", n(self.flat)));
        }
        if self.max_pct != 0.0 {
            parts.push(format!("{}% of your max health", n(self.max_pct)));
        }
        if self.missing_pct != 0.0 {
            parts.push(format!("{}% of your missing health", n(self.missing_pct)));
        }
        let mut s = parts.join(" + ");
        if self.per_hit {
            s += " per hit";
        }
        if self.seconds > 0.0 {
            s += &format!(" over {}s", n(self.seconds));
        }
        if self.on_kill {
            s += " when the target dies";
        }
        s
    }
}

/// What a plan needs to know about the hero beyond the ability itself.
pub struct Context<'a> {
    /// Damage of one shot.
    pub per_shot: f64,
    pub shots_per_second: f64,
    /// All of the hero's abilities, for effects that feed another ability.
    pub abilities: &'a [AbilitySheet],
    /// Hero conditions switched on so far, with their count (see `hero_conditions`).
    pub flags: &'a [(String, f64)],
}

/// A hero-specific state the player switches on in a combo, e.g. Shiv's full rage.
pub struct HeroCondition {
    /// Starts with `hero:`; abilities check it through `Context::flags`.
    pub id: String,
    pub name: String,
    pub text: String,
    /// For a counted condition these are per stack.
    pub buff: Buff,
    pub effect: Effect,
    /// Set for a counted condition: the highest count that still adds something.
    pub max: Option<f64>,
    /// What is counted, e.g. `kills`.
    pub unit: String,
}

/// Bebop's bomb stacks give half as much beyond these counts.
const BOMB_HIT_SOFT_CAP: f64 = 60.0;
const BOMB_KILL_SOFT_CAP: f64 = 7.0;

fn soft_capped(count: f64, cap: f64) -> f64 {
    count.min(cap) + (count - cap).max(0.0) / 2.0
}

/// The states this hero's kit reacts to. Values come from the hero's own abilities.
pub fn hero_conditions(abilities: &[AbilitySheet]) -> Vec<HeroCondition> {
    let get = |class: &str, key: &str| {
        abilities
            .iter()
            .find(|a| a.class_name == class)
            .and_then(|a| a.props.iter().find(|p| p.key == key))
            .map(|p| p.value)
            .unwrap_or(0.0)
    };
    let has = |class: &str| abilities.iter().any(|a| a.class_name == class);
    let mut out = vec![];
    let mut add = |id: &str, name: &str, text: String, buff: Buff, effect: Effect| {
        out.push(HeroCondition { id: id.into(), name: name.into(), text, buff, effect, max: None, unit: String::new() });
    };

    if has("citadel_ability_shiv_killing_blow") {
        let amp = get("citadel_ability_shiv_killing_blow", "BuffDamage");
        add(
            "hero:shiv_rage",
            "Full rage",
            format!(
                "+{}% damage; Serrated Knives also deal {}% of current health on impact and Slice and Dice hits a second time",
                n(amp),
                n(get("citadel_ability_shiv_dagger", "FullRageCurrentHealthDamagePct"))
            ),
            Buff::default(),
            Effect { all_amp: amp, ..Default::default() },
        );
    }
    if has("ability_drifter_hunger") {
        let amp = get("ability_drifter_hunger", "AmpDamagePercent");
        add(
            "hero:drifter_isolated",
            "Target is isolated",
            format!("+{}% damage against the isolated target", n(amp)),
            Buff::default(),
            Effect { all_amp: amp, ..Default::default() },
        );
    }
    if has("ability_frank_revive") {
        let buff = Buff {
            fire_rate: get("ability_frank_revive", "BonusFireRate"),
            spirit_per_shot: get("ability_frank_revive", "BonusDamagePerBullet"),
            ..Default::default()
        };
        // Only with the upgrade that grants the bonus.
        if !buff.is_empty() {
            add("hero:victor_reanimation", "Reanimation on cooldown", buff.text(), buff, Effect::default());
        }
    }
    if has("citadel_ability_tangotether") {
        add(
            "hero:ivy_connected",
            "Connected to an ally",
            "Kudzu Connection grants its full bonuses instead of half".into(),
            Buff::default(),
            Effect::default(),
        );
    }
    if has("citadel_ability_chrono_time_wall") {
        let amp = get("citadel_ability_chrono_time_wall", "FriendlyBulletDamageBonus");
        add(
            "hero:paradox_time_wall",
            "Shooting through Time Wall",
            format!("+{}% bullet damage", n(amp)),
            Buff::default(),
            Effect { bullet_amp: amp, ..Default::default() },
        );
    }
    if has("ability_priest_knockback") {
        add(
            "hero:venator_wall",
            "Target is near a wall",
            format!("Gutshot deals {} more bullet damage", n(get("ability_priest_knockback", "BonusDamage"))),
            Buff::default(),
            Effect::default(),
        );
    }
    if has("ability_doorman_hotel") {
        add(
            "hero:doorman_late_checkout",
            "Late checkout",
            format!("Hotel Guest deals {} more spirit damage when the stay runs out", n(get("ability_doorman_hotel", "LateCheckoutDamage"))),
            Buff::default(),
            Effect::default(),
        );
    }
    if has("ability_magician_magicbolt") {
        add(
            "hero:sinclair_assistant",
            "Assistant is out",
            format!("The Assistant also casts Vexing Bolt for {}% of its damage", n(get("ability_magician_magicbolt", "CloneDamagePercentage"))),
            Buff::default(),
            Effect::default(),
        );
    }

    // --- Counted states: the bonus is per stack and the player enters how many. ---
    let mut counted = |id: &str, name: &str, unit: &str, max: f64, text: String, buff: Buff| {
        out.push(HeroCondition { id: id.into(), name: name.into(), text, buff, effect: Effect::default(), max: Some(max), unit: unit.into() });
    };
    if has("citadel_ability_sticky_bomb") {
        let (hit, kill) = (get("citadel_ability_sticky_bomb", "BonusDamagePctPerPlayerHit"), get("citadel_ability_sticky_bomb", "BonusDamagePctPerPlayerKilled"));
        counted(
            "hero:bebop_bomb_hits",
            "Sticky Bomb hero hits",
            "hits",
            500.0,
            format!("+{}% Sticky Bomb damage per hit (half as much after {} hits)", n(hit), n(BOMB_HIT_SOFT_CAP)),
            Buff::default(),
        );
        counted(
            "hero:bebop_bomb_kills",
            "Sticky Bomb kills",
            "kills",
            100.0,
            format!("+{}% Sticky Bomb damage per kill (half as much after {} kills)", n(kill), n(BOMB_KILL_SOFT_CAP)),
            Buff::default(),
        );
    }
    if has("ability_guided_arrow") {
        let per = get("ability_guided_arrow", "BonusTechPowerPerKill");
        counted("hero:talon_owl_kills", "Guided Owl kills", "kills", 100.0, format!("+{} spirit power per kill", n(per)), Buff { spirit_power: per, ..Default::default() });
    }
    if has("citadel_ability_hornet_snipe") {
        let per = get("citadel_ability_hornet_snipe", "WeaponDamageBonusPerKill");
        counted("hero:vindicta_kills", "Assassinate kills", "kills", 100.0, format!("+{}% weapon damage per kill", n(per)), Buff { weapon_pct: per, ..Default::default() });
    }
    if has("ability_drifter_hunger") {
        let per = get("ability_drifter_hunger", "WeaponDmgPerIsolationKill");
        counted("hero:drifter_kills", "Isolated kills", "kills", 100.0, format!("+{}% weapon damage per kill", n(per)), Buff { weapon_pct: per, ..Default::default() });
    }
    if has("ability_artist_tether") {
        let per = get("ability_artist_tether", "SpiritPerPickup");
        if per > 0.0 {
            counted(
                "hero:violet_sketches",
                "Sketches of spirit heroes",
                "sketches",
                50.0,
                format!("+{} spirit power per sketch collected from a spirit hero", n(per)),
                Buff { spirit_power: per, ..Default::default() },
            );
        }
    }
    if has("ability_nurse_leech_therapy") {
        let c = "ability_nurse_leech_therapy";
        let (fire_rate, spirit, regen) = (get(c, "FireRatePerLeechPickup"), get(c, "SpiritPowerPerLeechStack"), get(c, "RegenPerLeechPickup"));
        let buff = Buff { fire_rate, spirit_power: spirit, ..Default::default() };
        counted(
            "hero:nurse_leeches",
            "Leeches collected",
            "leeches",
            get(c, "MaxLeechBonusStacks").max(1.0),
            format!("{} and +{} health regen per leech collected; Lurking Terrors fears {}s longer per leech", buff.text(), n(regen), n(get("ability_nurse_fear", "FearDurationPerLeechStack"))),
            buff,
        );
    }
    if has("ability_vampirebat_batswarm") {
        let (per, max) = (get("ability_vampirebat_batswarm", "BonusBatsPerProc"), get("ability_vampirebat_batswarm", "BonusBatsMax"));
        if per > 0.0 {
            counted(
                "hero:mina_bites",
                "Love Bites landed",
                "bites",
                (max / per).floor(),
                format!("+{} Nox Nostra bats per bite (up to {})", n(per), n(max)),
                Buff::default(),
            );
        }
    }
    out
}

#[derive(Debug, Clone)]
pub enum Part {
    Instant {
        label: String,
        kind: Kind,
        raw: f64,
        health: Health,
        /// Seconds between the cast and the hit.
        delay: f64,
        /// (health threshold %, bonus damage %) against targets below the threshold.
        low_hp: Option<(f64, f64)>,
        /// (maximum damage, health % at which it is reached): grows with the target's missing health.
        ramp: Option<(f64, f64)>,
    },
    Dot {
        label: String,
        kind: Kind,
        dps: f64,
        seconds: f64,
        /// Per second.
        health: Health,
    },
    Hits {
        label: String,
        kind: Kind,
        raw: f64,
        count: u32,
        /// Seconds the hits are spread over; 0 lands them all at once.
        spread: f64,
        /// Damage multiplier for every hit after the first.
        falloff: f64,
        /// Added to every hit.
        health: Health,
    },
}

#[derive(Debug, Clone)]
pub struct Debuff {
    pub label: String,
    pub effect: Effect,
    pub seconds: f64,
    pub max_stacks: u32,
    /// Applied by every hit of a multi-hit part instead of once per cast.
    pub per_hit: bool,
}

#[derive(Debug, Clone)]
pub enum Passive {
    /// Damage on a headshot, with a cooldown.
    HeadshotProc { kind: Kind, raw: f64, cooldown: f64, debuff: Option<(Effect, f64)> },
    /// Shots fill a bar; a full bar starts a damage-over-time effect that later shots extend.
    BuildupDot { per_shot: f64, dps: f64, seconds: f64, extend_per_shot: f64, extend_per_headshot: f64, max_seconds: f64 },
    /// Each shot adds stacks of a self buff.
    /// `proc_raw` spirit damage lands every `proc_every` stacks gained (0 disables it).
    StackingBuff { per_stack: Buff, max_stacks: f64, seconds: f64, per_shot: f64, per_headshot: f64, proc_every: f64, proc_raw: f64 },
    /// A self buff that is always on.
    Always { buff: Buff },
    /// Debuff applied to the target by a melee hit.
    MeleeDebuff { effect: Effect, seconds: f64 },
    /// Shots fill a bar; a full bar deals a burst of spirit damage, with a cooldown.
    /// `ramp` grows the damage with the target's missing health: (maximum, health % for the maximum).
    BuildupProc { per_shot: f64, raw: f64, cooldown: f64, ramp: Option<(f64, f64)> },
    /// Each shot adds a stack of a debuff on the target.
    BulletDebuff { effect: Effect, max_stacks: u32, seconds: f64 },
}

#[derive(Debug, Clone)]
pub struct Plan {
    pub name: String,
    /// False for passives: they act on their own instead of being a combo action.
    pub castable: bool,
    pub cast_delay: f64,
    pub cooldown: f64,
    /// More than 1 for a charged ability: each charge comes back after `cooldown`,
    /// and `between` is the wait between two casts.
    pub charges: u32,
    pub between: f64,
    pub parts: Vec<Part>,
    pub debuffs: Vec<Debuff>,
    pub buff: Buff,
    pub buff_seconds: f64,
    pub passives: Vec<Passive>,
    /// Counts as a melee hit for on-melee item effects.
    pub melee_hit: bool,
    /// Every hit adds this fraction to the buildup bar of the named passive.
    pub feeds: Option<(String, f64)>,
    /// Health the ability restores to its user.
    pub heals: Vec<Heal>,
    /// Percent of the damage this ability deals that returns as health.
    pub leech: f64,
    pub notes: Vec<String>,
}

impl Plan {
    /// Human-readable summary of what the simulation does with this ability.
    pub fn describe(&self) -> Vec<String> {
        let mut out = vec![];
        for part in &self.parts {
            out.push(match part {
                Part::Instant { label, kind, raw, health, delay, low_hp, ramp } => {
                    let mut s = match ramp {
                        Some((max, at)) => format!("{label}: {} to {} {} (maximum at {}% health or less)", n(*raw), n(*max), kind.word(), n(*at)),
                        None if *raw > 0.0 => format!("{label}: {} {}", n(*raw), kind.word()),
                        None => format!("{label}: {}", kind.word()),
                    };
                    if !health.is_empty() {
                        s += &format!("{}{}", if *raw > 0.0 { " + " } else { " " }, health.text());
                    }
                    if *delay > 0.0 {
                        s += &format!(", {}s after the cast", n(*delay));
                    }
                    if let Some((threshold, bonus)) = low_hp {
                        s += &format!(", +{}% below {}% health", n(*bonus), n(*threshold));
                    }
                    s
                }
                Part::Dot { label, kind, dps, seconds, health } => {
                    let mut pieces = vec![];
                    if *dps > 0.0 {
                        pieces.push(format!("{} {} DPS", n(*dps), kind.word()));
                    }
                    if !health.is_empty() {
                        pieces.push(format!("{} per second ({})", health.text(), kind.word()));
                    }
                    format!("{label}: {} for {}s", pieces.join(" + "), n(*seconds))
                }
                Part::Hits { label, kind, raw, count, spread, falloff, health } => {
                    let mut s = format!("{label}: {count} hits × {} {}", n(*raw), kind.word());
                    if !health.is_empty() {
                        s += &format!(" + {}", health.text());
                    }
                    if *spread > 0.0 {
                        s += &format!(" over {}s", n(*spread));
                    }
                    if *falloff != 1.0 {
                        s += &format!(", hits after the first deal {}%", n(falloff * 100.0));
                    }
                    s
                }
            });
        }
        for d in &self.debuffs {
            let stacks = if d.max_stacks > 1 && d.max_stacks < u32::MAX {
                format!(", stacks up to {}", d.max_stacks)
            } else if d.max_stacks == u32::MAX && !d.per_hit {
                ", stacks with every cast".to_string()
            } else {
                String::new()
            };
            let when = if d.per_hit { " per hit" } else { "" };
            out.push(format!("Debuff: {}{when} for {}s{stacks}", d.effect.text(), n(d.seconds)));
        }
        if !self.buff.is_empty() {
            if self.buff_seconds.is_finite() {
                out.push(format!("Self buff: {} for {}s", self.buff.text(), n(self.buff_seconds)));
            } else {
                out.push(format!("Self buff: {} (permanent)", self.buff.text()));
            }
        }
        for p in &self.passives {
            out.push(match p {
                Passive::HeadshotProc { kind, raw, cooldown, debuff } => {
                    let extra = debuff.map(|(e, s)| format!(" and {} for {}s", e.text(), n(s))).unwrap_or_default();
                    format!("Passive: +{} {} damage on a headshot{extra}, {}s cooldown", n(*raw), kind.word(), n(*cooldown))
                }
                Passive::BuildupDot { per_shot, dps, seconds, .. } => format!(
                    "Passive: {}% buildup per shot, then {} spirit DPS for {}s (shots extend it)",
                    n(per_shot * 100.0),
                    n(*dps),
                    n(*seconds)
                ),
                Passive::StackingBuff { per_stack, max_stacks, seconds, per_headshot, proc_every, proc_raw, .. } => {
                    let proc = if *proc_every > 0.0 && *proc_raw > 0.0 {
                        format!("; every {} stacks gained deal {} spirit damage", n(*proc_every), n(*proc_raw))
                    } else {
                        String::new()
                    };
                    let headshot = if *per_headshot > 0.0 { format!(" ({} more on a headshot)", n(*per_headshot)) } else { String::new() };
                    format!(
                        "Passive: each shot adds a stack{headshot} of {}, up to {} stacks, lasting {}s{proc}",
                        per_stack.text(),
                        n(*max_stacks),
                        n(*seconds)
                    )
                }
                Passive::Always { buff } => format!("Passive: {}", buff.text()),
                Passive::MeleeDebuff { effect, seconds } => {
                    format!("Passive: melee hits apply {} for {}s", effect.text(), n(*seconds))
                }
                Passive::BulletDebuff { effect, max_stacks, seconds } => format!(
                    "Passive: each shot applies {} for {}s, up to {} stacks",
                    effect.text(),
                    n(*seconds),
                    max_stacks
                ),
                Passive::BuildupProc { per_shot, raw, cooldown, ramp } => {
                    let damage = match ramp {
                        Some((max, at)) => format!("{} to {} spirit damage (maximum at {}% health or less)", n(*raw), n(*max), n(*at)),
                        None => format!("{} spirit damage", n(*raw)),
                    };
                    let wait = if *cooldown > 0.0 { format!(", {}s cooldown", n(*cooldown)) } else { String::new() };
                    format!("Passive: {}% buildup per shot, then {damage}{wait}", n(per_shot * 100.0))
                }
            });
        }
        if self.charges > 1 && self.castable {
            out.push(format!(
                "{} charges: {}s between casts, a spent charge returns after {}s (one at a time)",
                self.charges,
                n(self.between),
                n(self.cooldown)
            ));
        }
        for h in &self.heals {
            out.push(format!("Heal ({}): {}", h.label, h.text()));
        }
        if self.leech > 0.0 {
            out.push(format!("Heals you for {}% of the damage it deals", n(self.leech)));
        }
        out.extend(self.notes.iter().cloned());
        if out.is_empty() {
            out.push("No damage to simulate".into());
        }
        out
    }

    pub fn has_damage(&self) -> bool {
        !self.parts.is_empty()
    }
}

fn kind_of(p: &ScaledProp) -> Option<Kind> {
    match p.damage_kind.as_deref() {
        Some("spirit") => Some(Kind::Spirit),
        Some("bullet") => Some(Kind::Bullet),
        Some("melee") => Some(Kind::Melee),
        _ => None,
    }
}

/// True for keys like `DamagePerBite` or `ArmorReductionPerBite`, but not for `...Percent...`.
fn is_per_unit(key: &str) -> bool {
    key.match_indices("Per").any(|(i, _)| !key[i..].starts_with("Percent") && !key[i..].starts_with("PerSecond"))
}

fn instant(label: &str, kind: Kind, raw: f64) -> Part {
    Part::Instant { label: label.into(), kind, raw, health: Health::default(), delay: 0.0, low_hp: None, ramp: None }
}

fn delayed(label: &str, kind: Kind, raw: f64, delay: f64) -> Part {
    Part::Instant { label: label.into(), kind, raw, health: Health::default(), delay, low_hp: None, ramp: None }
}

fn dot(label: &str, kind: Kind, dps: f64, seconds: f64) -> Part {
    Part::Dot { label: label.into(), kind, dps, seconds, health: Health::default() }
}

fn hits(label: &str, kind: Kind, raw: f64, count: f64, spread: f64, falloff: f64) -> Part {
    Part::Hits { label: label.into(), kind, raw, count: count.round().clamp(0.0, MAX_HITS) as u32, spread, falloff, health: Health::default() }
}

fn once(label: &str, effect: Effect, seconds: f64) -> Debuff {
    Debuff { label: label.into(), effect, seconds, max_stacks: 1, per_hit: false }
}

/// Reads the plan from the data by naming conventions.
fn generic(ability: &AbilitySheet, duration: Option<f64>, hit_override: Option<f64>) -> Plan {
    let props = &ability.props;
    let v = |key: &str| props.iter().find(|p| p.key == key).map(|p| p.value);
    let seconds = duration.unwrap_or_else(|| {
        ["AbilityDuration", "AbilityChannelTime", "BurnDuration", "DebuffDuration"]
            .iter()
            .find_map(|k| v(k))
            .unwrap_or(FALLBACK_SECONDS)
    });

    // Multi-hit abilities: an interval means hits spread over the duration, a count means all at once.
    let interval = props
        .iter()
        .find(|p| !p.label.is_empty() && p.key.ends_with("Interval"))
        .map(|p| p.value)
        .filter(|i| *i > 0.0);
    let (count, spread) = match (hit_override, interval, v("Detonations")) {
        (Some(h), Some(_), _) => (h, seconds),
        (Some(h), None, _) => (h, 0.0),
        (None, Some(i), _) => ((seconds / i * v("RatsAttached").unwrap_or(1.0)).floor(), seconds),
        (None, None, Some(c)) => (c, 0.0),
        (None, None, None) => (1.0, 0.0),
    };

    let mut parts = vec![];
    let ramp = v("MinDPS").zip(v("MaxDPS"));
    for p in props {
        // Unlabelled values are internals; only what the game shows is read by convention.
        if p.label.is_empty() {
            continue;
        }
        // Percentages, bonuses and thresholds modify other damage; they are not damage of their own.
        let Some(kind) = kind_of(p) else { continue };
        if p.value <= 0.0 || p.unit.contains('%') || p.key.contains("Bonus") || p.key.contains("Threshold") || p.key == "MagicDamagePerBullet" {
            continue;
        }
        let key = p.key.as_str();
        if key == "MinDPS" || (key == "MinDamage" && v("MaxDamage").is_some()) {
            continue;
        }
        if key == "MaxDPS" && ramp.is_some() {
            let (min, max) = ramp.unwrap();
            parts.push(dot("Damage (average of the ramp)", kind, (min + max) / 2.0, seconds));
        } else if key.contains("DPS") || key.contains("PerSecond") {
            parts.push(dot(&p.label, kind, p.value, seconds));
        } else if key.contains("Damage") && is_per_unit(key) {
            parts.push(hits(&p.label, kind, p.value, count, spread, 1.0));
        } else if key.contains("Damage") {
            parts.push(instant(&p.label, kind, p.value));
        }
    }

    let mut debuffs = vec![];
    let mut buff = Buff::default();
    for p in props {
        if p.label.is_empty() {
            continue;
        }
        let key = p.key.as_str();
        let mut effect = Effect::default();
        match p.modifier.as_deref().unwrap_or("") {
            m if m.contains("BULLET_AND_MELEE_RESIST_REDUCTION") => effect.bullet = p.value.abs(),
            m if m.contains("TECH_RESIST_REDUCTION") => effect.spirit = p.value.abs(),
            m if m.contains("BULLET_DAMAGE_TAKEN_INCREASE") => effect.bullet_amp = p.value,
            m if m.contains("ALL_DAMAGE_TAKEN_INCREASE") => effect.all_amp = p.value,
            // Per-kill and per-stack bonuses are not a buff the cast itself grants.
            _ if is_per_unit(key) && !key.contains("Reduction") => {}
            m if m.contains("FLAT_BULLET_DAMAGE_POST_SCALE") && p.value > 0.0 => buff.flat_bullet += p.value,
            m if m.ends_with("VALUE_FIRE_RATE") && p.value > 0.0 => buff.fire_rate += p.value,
            m if m.ends_with("WEAPON_DAMAGE_INCREASE") && p.value > 0.0 => buff.weapon_pct += p.value,
            m if m.ends_with("VALUE_TECH_POWER") && p.value > 0.0 => buff.spirit_power += p.value,
            m if m.ends_with("TECH_POWER_PERCENT") && p.value > 0.0 => buff.spirit_power_pct += p.value,
            _ => {}
        }
        if key == "IncomingDamagePercentFromCaster" {
            effect.all_amp = p.value;
        }
        if key == "MagicDamagePerBullet" {
            buff.spirit_per_shot += p.value;
        }
        if !effect.is_empty() {
            let per_hit = is_per_unit(key);
            debuffs.push(Debuff {
                label: p.label.clone(),
                effect,
                seconds: v(&format!("{key}Duration")).or(v("DebuffDuration")).unwrap_or(seconds),
                max_stacks: if per_hit { u32::MAX } else { 1 },
                per_hit,
            });
        }
    }
    let buff_seconds = ["WeaponDamageBonusDuration", "WeaponPowerIncreaseDuration", "SelfBuffDuration", "BuffDuration", "AbilityDuration", "MovementSpeedBonusDuration"]
        .iter()
        .find_map(|k| v(k))
        .unwrap_or(seconds);

    Plan {
        name: ability.name.clone(),
        castable: true,
        cast_delay: v("AbilityCastDelay").unwrap_or(0.0),
        cooldown: v("AbilityCooldown").unwrap_or(0.0),
        charges: v("AbilityCharges").unwrap_or(0.0).clamp(0.0, 20.0).round() as u32,
        between: v("AbilityCooldownBetweenCharge").unwrap_or(0.0).max(0.0),
        parts,
        debuffs,
        buff,
        buff_seconds,
        passives: vec![],
        melee_hit: false,
        feeds: None,
        heals: vec![],
        leech: 0.0,
        notes: vec![],
    }
}

/// Builds the simulation plan for one ability.
pub fn plan(ability: &AbilitySheet, ctx: &Context, duration: Option<f64>, hit_override: Option<f64>) -> Plan {
    let mut plan = generic(ability, duration, hit_override);
    customize(ability, &mut plan, ctx, duration, hit_override);
    healing(ability, &mut plan, ctx, duration);
    if plan.has_damage() {
        plan.notes.extend(crate::area::describe(ability));
    }
    plan
}

/// The health each ability restores to its user: direct heals, heals over time and lifesteal.
fn healing(ability: &AbilitySheet, plan: &mut Plan, ctx: &Context, duration: Option<f64>) {
    let v = |key: &str| ability.props.iter().find(|p| p.key == key).map(|p| p.value).unwrap_or(0.0);
    let on = |flag: &str| ctx.flags.iter().any(|(f, _)| f == flag);
    let seconds = |key: &str| duration.unwrap_or_else(|| v(key));
    let flat = |label: &str, amount: f64| Heal { label: label.into(), flat: amount, ..Default::default() };
    let over = |label: &str, per_second: f64, time: f64| Heal { label: label.into(), flat: per_second * time, seconds: time, ..Default::default() };
    // Lifesteal the ability grants for a while. A buff the ability already has keeps its own length.
    let steal = |plan: &mut Plan, bullet: f64, spirit: f64, melee: f64, time: f64| {
        if bullet + spirit + melee <= 0.0 {
            return;
        }
        if plan.buff.is_empty() {
            plan.buff_seconds = time;
        }
        plan.buff.bullet_lifesteal += bullet;
        plan.buff.spirit_lifesteal += spirit;
        plan.buff.melee_lifesteal += melee;
    };

    match ability.class_name.as_str() {
        // --- Abrams ---
        "citadel_ability_bull_heal" => plan.leech = v("HealingFactor"),
        "citadel_ability_passive_beefy" => {
            if v("MeleeLifesteal") > 0.0 {
                plan.passives.push(Passive::Always { buff: Buff { melee_lifesteal: v("MeleeLifesteal"), ..Default::default() } });
            }
            plan.notes.push("Regenerating a share of the damage Abrams takes is not simulated: the combo has no incoming damage.".into());
        }
        // --- Apollo ---
        "ability_fencer_riposte" => {
            plan.leech = v("AbilityLifestealPercentHero");
            steal(plan, v("TargetLifesteal"), v("TargetLifesteal"), 0.0, v("TargetLifestealDuration"));
        }
        "ability_fencer_lunge" => plan.heals.push(Heal { per_hit: true, ..flat("hero hit", v("HealFixedHealth")) }),
        // --- Deadman Danny ---
        "ability_deadpack_rocketeer" if v("HealOnHit") > 0.0 => plan.heals.push(flat("explosion hitting a hero", v("HealOnHit"))),
        // --- Baba ---
        "ability_baba_bench_run" => steal(plan, 0.0, 0.0, v("MeleeLifesteal"), seconds("AbilityDuration")),
        // --- Bebop ---
        "citadel_ability_uppercut" => plan.heals.push(Heal { label: "uppercut".into(), missing_pct: v("MissingHPHeal"), ..Default::default() }),
        "citadel_ability_bebop_laser_beam" => plan.leech = v("BeamLifesteal"),
        // --- Calico ---
        "ability_nano_dash" => plan.heals.push(flat("hero hit", v("HealAmount"))),
        "ability_nano_catform" => plan.heals.push(over("cat form", v("HealthRegen"), seconds("BuffDuration"))),
        "ability_nano_shadow_pulse" => plan.heals.push(flat("return", v("HealAmount"))),
        // --- Celeste ---
        "ability_unicorn_radiantblast" => steal(plan, 0.0, v("AbilityLifestealPercentHero"), 0.0, seconds("AbilityDuration")),
        // --- Drifter ---
        "ability_drifter_hunger" => {
            plan.heals.push(Heal { label: "kill".into(), missing_pct: v("HealOnKillPct"), on_kill: true, ..Default::default() })
        }
        // --- Dynamo ---
        "citadel_ability_nikuman" => {
            let time = seconds("AbilityChannelTime");
            plan.heals.push(Heal { max_pct: v("HealMaxHealthPercent") * time, ..over("aurora", v("HealingPerSecond"), time) });
        }
        // --- Violet ---
        "ability_artist_tether" if v("HealthRegen") > 0.0 => plan.heals.push(over("sketching", v("HealthRegen"), v("BuffDuration"))),
        "ability_artist_bamboogrove" => steal(plan, 0.0, v("AbilityLifestealPercentHero"), 0.0, seconds("AbilityDuration")),
        // --- Grey Talon ---
        "ability_power_jump" => steal(plan, v("BulletLifestealPercent"), v("TechLifestealPercent"), 0.0, seconds("AbilityDuration")),
        // --- Haze ---
        "ability_smoke_bomb" => {
            let time = if v("PostInvisBuffDuration") > 0.0 { v("PostInvisBuffDuration") } else { seconds("AbilityDuration") };
            steal(plan, v("BulletLifesteal"), 0.0, 0.0, time);
        }
        // --- Infernus ---
        "ability_incendiary_projectile" => {
            // Lifesteal against the soaked victims, so it lasts as long as the debuff.
            let time = plan.debuffs.first().map(|d| d.seconds).unwrap_or_else(|| v("DebuffDuration"));
            steal(plan, v("LifestealPercentHero"), v("LifestealPercentHero"), 0.0, time);
        }
        "ability_fire_bomb" => plan.leech = v("LifeStealPercentOnHit"),
        // --- Ivy ---
        "citadel_ability_tangotether" => {
            let share = if on("hero:ivy_connected") { 1.0 } else { 0.5 };
            let time = if v("AbilityDuration") <= 0.0 && duration.is_none() { f64::INFINITY } else { seconds("AbilityDuration") };
            steal(plan, v("BulletLifestealPercent") * share, 0.0, 0.0, time);
        }
        "citadel_ability_tengu_stone_form" => {
            plan.heals.push(Heal { label: "stone form".into(), max_pct: v("MaxHealthRegen"), ..Default::default() })
        }
        // --- Kelvin ---
        "ability_ice_grenade" => {
            plan.heals.push(flat("blast", v("HealAmount")));
            plan.notes.push("The heal assumes Kelvin stands in his own blast.".into());
        }
        "ability_ice_dome" => plan.heals.push(over("shelter", v("BonusHealthRegen"), seconds("AbilityDuration"))),
        // --- Lady Geist ---
        "ability_life_drain" => plan.leech = v("LifeDrainHealthMult"),
        "ability_blood_bomb" | "ability_blood_shards" => {
            plan.notes.push(format!("Costs {}% of Lady Geist's current health; the cost is not simulated.", n(v("SelfDamagePct"))));
        }
        // --- Lash ---
        "ability_lash_flog" => plan.leech = v("HealPctVsHeroes"),
        // --- Nurse Harrow ---
        "ability_nurse_leech_therapy" => plan.leech = v("HealPercentOfLeechDamage"),
        // --- McGinnis ---
        "citadel_ability_mobile_resupply" => {
            let time = seconds("AbilityDuration");
            plan.heals.push(Heal { max_pct: v("MaxHealthRegenPct") * time, ..over("specter", v("ExternalBonusHealthRegen"), time) });
            plan.notes.push("The heal assumes McGinnis stays inside the specter's radius.".into());
        }
        // --- Mina ---
        "ability_vampirebat_steallife" => plan.heals.push(Heal { on_kill: true, ..flat("kill", v("RakeHealPerKill")) }),
        // --- Mirage ---
        "mirage_fire_beetles" => plan.leech = 100.0,
        // --- Mo & Krill ---
        "ability_intimidate" => plan.leech = v("DamageHealMult") * 100.0,
        "ability_ult_combo" => plan.leech = v("LifeStealPercentOnHit"),
        // --- Silver ---
        "ability_werewolf_transformation" => plan.heals.push(Heal { missing_pct: v("MissingHealthPercentHeal"), ..flat("transformation", v("HealAmount")) }),
        // --- Victor ---
        "ability_frank_shocktarget2" => plan.heals.push(Heal { missing_pct: v("MissingHealthPercentHeal"), ..flat("hit", v("HealOnHit")) }),
        "ability_frank_selfzap" => {
            let time = seconds("AbilityDuration");
            plan.heals.push(Heal { seconds: time, ..flat("jumpstart", v("TotalHealthRegen")) });
        }
        // --- Viscous ---
        "viscous_restorative_goo" => plan.heals.push(over("cube", v("BonusHealthRegen"), seconds("AbilityDuration"))),
        "viscous_telepunch" => plan.leech = v("LifeStealPercentOnHit"),
        // --- Warden ---
        "ability_warden_riot_protocol" => plan.leech = v("HealthStealPctHero"),
        // --- Wraith ---
        "citadel_ability_card_toss" => plan.heals.push(flat("heart, one card in four", v("HeartHeal") * 0.25)),
        // --- Yamato ---
        "citadel_ability_healing_slash" => plan.heals.push(Heal { max_pct: v("HealMaxHealth"), ..flat("hero hit", v("HealFixedHealth")) }),
        "citadel_ability_infinity_slash" => {
            plan.heals.push(Heal { label: "kill".into(), max_pct: v("MaxHealthRegen"), on_kill: true, ..Default::default() })
        }
        _ => {}
    }
    plan.heals.retain(|h| h.flat > 0.0 || h.max_pct > 0.0 || h.missing_pct > 0.0);
}

/// Corrections for abilities the data alone does not describe.
fn customize(ability: &AbilitySheet, plan: &mut Plan, ctx: &Context, duration: Option<f64>, hit_override: Option<f64>) {
    let weapon = ctx.per_shot;
    let on = |flag: &str| ctx.flags.iter().any(|(f, _)| f == flag);
    let count = |flag: &str| ctx.flags.iter().find(|(f, _)| f == flag).map(|(_, c)| *c).unwrap_or(0.0);
    // A value from one of the hero's other abilities.
    let sibling = |class: &str, key: &str| {
        ctx.abilities
            .iter()
            .find(|a| a.class_name == class)
            .and_then(|a| a.props.iter().find(|p| p.key == key))
            .map(|p| p.value)
            .unwrap_or(0.0)
    };
    let props = &ability.props;
    let v = |key: &str| props.iter().find(|p| p.key == key).map(|p| p.value).unwrap_or(0.0);
    let kind = |key: &str, fallback: Kind| props.iter().find(|p| p.key == key).and_then(kind_of).unwrap_or(fallback);
    let seconds = |key: &str| duration.unwrap_or_else(|| v(key));
    let passive_only = |plan: &mut Plan, passive: Passive| {
        plan.castable = false;
        plan.parts.clear();
        plan.debuffs.clear();
        plan.buff = Buff::default();
        plan.passives.push(passive);
    };

    match ability.class_name.as_str() {
        // --- Apollo ---
        "ability_fencer_lunge" => {
            plan.parts = vec![hits("Perfect stab", Kind::Spirit, v("PerfectDamage"), hit_override.unwrap_or(v("MaxStabs")), 0.0, 1.0)];
            plan.notes.push("Assumes every stab is released in the perfect window.".into());
        }
        "ability_fencer_ultimate" => {
            plan.parts = vec![
                instant("Impact", Kind::Spirit, v("ImpactDamage")),
                Part::Instant {
                    label: "Delayed damage".into(),
                    kind: Kind::Spirit,
                    raw: v("DelayedDamage"),
                    health: Health::default(),
                    delay: v("DebuffDuration"),
                    low_hp: Some((v("LowHealthEnemyThresholdPct"), v("BonusDamagePercent"))),
                    ramp: None,
                },
            ];
        }
        // --- Bebop ---
        "citadel_ability_uppercut" => {
            plan.parts = vec![instant("Uppercut", Kind::Melee, v("UppercutDamage"))];
            plan.notes.push("The landing explosion only hits other enemies near the landing spot, not the launched target.".into());
        }
        "citadel_ability_sticky_bomb" => {
            // Earlier bomb hits and kills add permanent bonus damage, at half rate beyond a soft cap.
            let bonus = v("BonusDamagePctPerPlayerHit") * soft_capped(count("hero:bebop_bomb_hits"), BOMB_HIT_SOFT_CAP)
                + v("BonusDamagePctPerPlayerKilled") * soft_capped(count("hero:bebop_bomb_kills"), BOMB_KILL_SOFT_CAP);
            plan.parts = vec![delayed("Explosion", Kind::Spirit, v("Damage") * (1.0 + bonus / 100.0), v("AbilityDuration"))];
            if bonus > 0.0 {
                plan.notes.push(format!("+{}% from earlier bomb hits and kills.", n(bonus)));
            } else {
                plan.notes.push("Earlier bomb hits and kills add permanent damage: set them with the Sticky Bomb conditions.".into());
            }
        }
        "citadel_ability_hook" => {
            if v("BulletAmp") > 0.0 {
                let effect = Effect { bullet_amp: v("BulletAmp"), ..Default::default() };
                plan.debuffs.push(once("Bullet amp", effect, v("BulletAmpDuration")));
            }
        }
        // --- Billy ---
        "ability_punkgoat_goatflip" => {
            plan.buff.weapon_pct += v("WeaponDamageBurst");
            plan.buff_seconds = v("WeaponDamageBurstDuration");
        }
        "ability_punkgoat_blasted" => {
            let effect = Effect { bullet_amp: v("BulletDamageAmp"), ..Default::default() };
            passive_only(plan, Passive::MeleeDebuff { effect, seconds: v("BulletDamageAmpDuration") });
        }
        "ability_punkgoat_tether" => {
            plan.parts = vec![delayed("Yank", Kind::Spirit, v("Damage"), seconds("AbilityDuration"))];
        }
        // --- Calico ---
        "ability_nano_clustergrenade" => {
            plan.parts = vec![hits("Bomb", Kind::Spirit, v("Damage"), hit_override.unwrap_or(v("GrenadeCount")), 0.0, 0.65)];
            plan.notes.push("Assumes every bomb hits the target; lower the hit count if only some connect.".into());
        }
        "ability_nano_shadow_pulse" => {
            plan.parts = vec![
                instant("Vanish", Kind::Spirit, v("Damage")),
                delayed("Return", Kind::Spirit, v("Damage"), v("AbilityChannelTime")),
            ];
        }
        // --- Celeste ---
        "ability_unicorn_radiantblast" => {
            plan.parts = vec![instant("Flare", Kind::Spirit, v("FlareDamage"))];
            plan.buff.spirit_per_shot = v("Damage");
            plan.buff_seconds = seconds("AbilityDuration");
        }
        "ability_unicorn_luminousstrike" => {
            plan.debuffs.push(Debuff {
                label: "Spirit amp".into(),
                effect: Effect { spirit_amp: v("MagicIncreasePerStack"), ..Default::default() },
                seconds: v("BuffDuration"),
                max_stacks: v("BuffMaxStacks").max(1.0) as u32,
                per_hit: false,
            });
        }
        // --- Drifter ---
        "drifter_blood_blast" => {
            // The last upgrade swaps the light melee scaling for heavy melee scaling.
            plan.parts = vec![
                instant("Swipe", Kind::Spirit, v("BonusDamage")),
                instant("Close-range melee", kind("Damage", Kind::Melee), v("Damage") + v("DamageHeavyMelee")),
            ];
            plan.notes.push("The melee part only applies within close range.".into());
        }
        "drifter_shadow_mark" => {
            plan.parts = vec![Part::Dot {
                label: "Bleed".into(),
                kind: Kind::Spirit,
                dps: 0.0,
                seconds: seconds("AbilityDuration"),
                health: Health { current: v("DotHealthPercent"), ..Default::default() },
            }];
        }
        "ability_drifter_hunger" => {
            plan.castable = false;
            plan.buff = Buff::default();
            plan.notes.push(format!(
                "Passive: +{}% damage against isolated enemies. Switch on the Target is isolated condition, or cast Eternal Night.",
                n(v("AmpDamagePercent"))
            ));
        }
        "drifter_darkness" => {
            // Enemies in the darkness count as isolated.
            let effect = Effect { all_amp: sibling("ability_drifter_hunger", "AmpDamagePercent"), ..Default::default() };
            plan.debuffs.push(once("Isolated", effect, seconds("AbilityDuration")));
        }
        // --- Dynamo ---
        "citadel_ability_stomp" => {
            for d in &mut plan.debuffs {
                d.seconds = v("SlowDuration");
            }
        }
        "citadel_ability_self_vacuum" => {
            for part in &mut plan.parts {
                if let Part::Dot { health, .. } = part {
                    health.max = v("DPSPercentHealth");
                }
            }
        }
        // --- Graves ---
        "ability_necro_hauntingskull" => {
            // Each Deadhead attacks again after a search delay for as long as it lives.
            let life = duration.unwrap_or(v("SkullLifetime"));
            let interval = match (v("TargetSearchDelayMin"), v("TargetSearchDelayMax")) {
                (a, b) if a > 0.0 && b > 0.0 => (a + b) / 2.0,
                _ => 1.4,
            };
            let all = v("SkullCount") * (life / interval).floor().max(1.0);
            plan.parts = vec![hits("Deadhead hit", Kind::Spirit, v("Damage"), hit_override.unwrap_or(all), life, 1.0)];
            plan.notes.push(format!(
                "{} Deadheads live {}s and attack about every {}s (interval estimated from the game data): {} hits if all of them stay on the target. Enter the hits that land in the hits box.",
                n(v("SkullCount")),
                n(life),
                n(interval),
                n(all)
            ));
        }
        "ability_necro_fear" => {
            // Shooting a target for `ShootDurationForMax` seconds steals the full amounts.
            let shots = (v("ShootDurationForMax") * ctx.shots_per_second).round().max(1.0);
            plan.castable = false;
            plan.parts.clear();
            plan.debuffs.clear();
            plan.buff = Buff::default();
            plan.passives.push(Passive::StackingBuff {
                per_stack: Buff { weapon_pct: v("MaxStolenAttackDamage") / shots, ..Default::default() },
                max_stacks: shots,
                seconds: v("AbilityDuration"),
                per_shot: 1.0,
                per_headshot: 0.0,
                proc_every: 0.0,
                proc_raw: 0.0,
            });
            plan.passives.push(Passive::BulletDebuff {
                effect: Effect { spirit: v("MaxStolenSpiritResist") / shots, ..Default::default() },
                max_stacks: shots as u32,
                seconds: v("AbilityDuration"),
            });
            plan.notes.push(format!(
                "Reaches +{}% weapon damage and -{}% spirit resist on the target after {} shots ({}s of shooting).",
                n(v("MaxStolenAttackDamage")),
                n(v("MaxStolenSpiritResist")),
                n(shots),
                n(v("ShootDurationForMax"))
            ));
        }
        "ability_necro_gravestone" => {
            plan.buff = Buff::default();
            // The gravestone spawns a burst of ghouls at a fixed interval; each one that reaches a hero explodes.
            let life = seconds("AbilityDuration");
            let bursts = ((life - v("SummonInitialDelay")) / v("SummonFrequency").max(0.1)).floor().max(0.0) + 1.0;
            let spawned = bursts * v("SummonBurstCount").max(1.0);
            let reached = hit_override.unwrap_or(spawned);
            plan.parts = vec![Part::Hits {
                label: "Ghoul explosion".into(),
                kind: Kind::Spirit,
                raw: v("Damage"),
                count: reached.round().clamp(0.0, MAX_HITS) as u32,
                spread: life,
                falloff: 1.0,
                health: Health { current: v("CurrentHealthDamagePercentage"), ..Default::default() },
            }];
            plan.notes.push(format!(
                "{} ghouls spawn over {}s ({} every {}s). Enter how many reach the target in the hits box (now {}). Ghouls only melee troopers; against heroes they explode.",
                n(spawned),
                n(life),
                n(v("SummonBurstCount").max(1.0)),
                n(v("SummonFrequency")),
                n(reached)
            ));
            plan.notes.push("The gravestone aura stacks in the game data (slow and spirit resist per stack) are not simulated.".into());
        }
        // --- Haze ---
        "ability_stacking_damage" => {
            let per_stack = Buff { flat_bullet: v("DamageBonusFixedPerStack"), ..Default::default() };
            passive_only(
                plan,
                Passive::StackingBuff {
                    per_stack,
                    max_stacks: v("MaxStacks"),
                    seconds: v("AbilityDuration"),
                    per_shot: 1.0,
                    per_headshot: 2.0,
                    proc_every: v("ProcDamageStackCount"),
                    proc_raw: v("ProcDamage"),
                },
            );
        }
        // --- Holliday ---
        "ability_crackshot" => {
            let shred = Effect { bullet: v("BulletResistReduction").abs(), ..Default::default() };
            let debuff = (!shred.is_empty()).then(|| (shred, v("BulletResistReductionDuration")));
            passive_only(
                plan,
                Passive::HeadshotProc { kind: Kind::Bullet, raw: v("Damage"), cooldown: v("AbilityCooldown"), debuff },
            );
        }
        // --- Infernus ---
        "ability_afterburn" => {
            passive_only(
                plan,
                Passive::BuildupDot {
                    per_shot: v("BuildUpBulletPercentPerHit") / 100.0,
                    dps: v("DPS"),
                    seconds: v("BurnDurationBase"),
                    extend_per_shot: v("RefillDuration"),
                    extend_per_headshot: v("RefillDurationCrit"),
                    max_seconds: v("BurnDuration"),
                },
            );
        }
        "ability_fire_bomb" => {
            plan.parts = vec![delayed("Explosion", Kind::Spirit, v("Damage"), v("ExplodeDelay"))];
        }
        // --- Ivy ---
        "citadel_ability_tangotether" => {
            if !on("hero:ivy_connected") {
                plan.buff.fire_rate *= 0.5;
                plan.notes.push("Without a connected ally Ivy gets half of the bonuses. Switch on the Connected to an ally condition for the full amount.".into());
            }
            // The last upgrade makes the connection permanent.
            if v("AbilityDuration") <= 0.0 && duration.is_none() {
                plan.buff_seconds = f64::INFINITY;
            }
        }
        "citadel_ability_tengu_airlift" => {
            // Bonus outgoing damage for Ivy is the same as the target taking more damage from her.
            let effect = Effect { all_amp: v("AirDropOutgoingDamagePercent"), ..Default::default() };
            plan.debuffs.push(once("Outgoing damage bonus", effect, v("BuffDuration")));
        }
        // --- Lady Geist ---
        "ability_blood_bomb" => {
            plan.parts = vec![delayed("Explosion", Kind::Spirit, v("Damage"), v("ArmingDuration"))];
        }
        "ability_blood_shards" => {
            plan.parts = vec![hits("Blood shard", Kind::Spirit, v("HealthToDamage"), hit_override.unwrap_or(v("NumBloodShards")), 0.0, 1.0)];
            // One stack per cast, not per shard.
            plan.debuffs = vec![Debuff {
                label: "Malice".into(),
                effect: Effect { all_amp: v("VulnerabilityPerStack"), ..Default::default() },
                seconds: v("DebuffDuration"),
                max_stacks: v("MaxStacks").max(1.0) as u32,
                per_hit: false,
            }];
            plan.notes.push("Assumes every shard hits the target.".into());
        }
        "ability_health_swap" => {
            plan.notes.push("The health swap is not simulated.".into());
        }
        // --- Lash ---
        "citadel_ability_lash_down_strike" => {
            // The hit count is used as the fall height in meters.
            let height = hit_override.unwrap_or(0.0).max(0.0);
            let raw = v("StompDamage") + height.min(25.0) * v("StompDamagePerMeterPrimary") + (height - 25.0).max(0.0) * v("StompDamagePerMeterSecondary");
            plan.parts = vec![instant("Stomp", Kind::Spirit, raw)];
            plan.notes.push(format!(
                "Fall height adds {} damage per meter up to 25m, then {} per meter. Enter the height in the hits box (now {}m).",
                n(v("StompDamagePerMeterPrimary")),
                n(v("StompDamagePerMeterSecondary")),
                n(height)
            ));
        }
        // --- McGinnis ---
        "citadel_ability_shieldedsentry" => {
            plan.parts = vec![dot("Turret", Kind::Spirit, v("TurretDPS"), duration.unwrap_or(v("TurretLifetime")))];
            plan.notes.push("Assumes one turret shoots the target for its whole lifetime; set the seconds to shorten it.".into());
        }
        "citadel_ability_fissure_wall" => {
            let effect = Effect { all_amp: v("BonusDamagePercent"), ..Default::default() };
            plan.debuffs.push(once("Damage amp", effect, v("DebuffDuration")));
        }
        "citadel_ability_rocket_barrage" => {
            let channel = seconds("AbilityDuration");
            let rockets = hit_override.unwrap_or((channel * v("GrenadesPerSecond")).floor());
            plan.parts = vec![hits("Rocket", Kind::Spirit, v("DamagePerRocket"), rockets, channel, 1.0)];
            plan.notes.push("Assumes every rocket hits the target.".into());
        }
        // --- Mina ---
        "ability_vampirebat_steallife" => {
            for part in &mut plan.parts {
                if let Part::Instant { health, .. } = part {
                    health.missing = v("MissingHealthDamagePercentage");
                }
            }
            plan.feeds = Some(("Love Bites".into(), sibling("ability_vampirebat_lovebites", "BuildUpPerDagger") / 100.0));
        }
        "ability_vampirebat_lovebites" => {
            plan.castable = false;
            plan.parts.clear();
            plan.buff = Buff::default();
            plan.passives.push(Passive::Always { buff: Buff { spirit_per_shot: v("MagicDamagePerBullet"), ..Default::default() } });
            plan.passives.push(Passive::BuildupProc {
                per_shot: v("BuildUpPerShot") / 100.0,
                raw: v("BonusDamage"),
                cooldown: v("PerTargetCooldown"),
                ramp: None,
            });
            plan.notes.push(format!(
                "Rake adds {}% and each Nox Nostra bat {}% to the bite buildup.",
                n(v("BuildUpPerDagger")),
                n(v("BuildUpPerBat") * 0.2)
            ));
        }
        "ability_vampirebat_batswarm" => {
            let bonus_bats = (v("BonusBatsPerProc") * count("hero:mina_bites")).min(v("BonusBatsMax"));
            let bats = hit_override.unwrap_or(v("BatCount") + bonus_bats);
            let per_second = v("BatPerSecond").max(1.0);
            plan.parts = vec![hits("Bat", Kind::Spirit, v("Damage"), bats, bats / per_second, 1.0)];
            // Bats build up Love Bites at a fraction of their listed rate.
            let per_bat = sibling("ability_vampirebat_lovebites", "BuildUpPerBat") * v("BatEffectiveness") / 100.0;
            plan.feeds = Some(("Love Bites".into(), per_bat));
            plan.notes.push("Assumes every bat hits the same target. Earlier Love Bites add bats: set them with the Love Bites landed condition.".into());
        }
        // --- Mirage ---
        "mirage_fire_beetles" => {
            plan.parts = vec![dot("Scarab", Kind::Spirit, v("DPS"), duration.unwrap_or(v("StealDuration")))];
        }
        "mirage_sand_phantom" => {
            // Each added mark doubles the damage: 1/2/4/8 times the base at 1/2/3/4 marks.
            let marks = hit_override.unwrap_or(v("MaxStacks")).clamp(1.0, v("MaxStacks").max(1.0));
            let raw = v("ProcDamageBase") * v("DMarkMultiplierPerStack").max(1.0).powf(marks - 1.0);
            plan.parts = vec![instant("Consume marks", Kind::Spirit, raw)];
            plan.notes.push(format!(
                "Assumes {} marks on the target (one per {}s of shooting). Enter the mark count in the hits box.",
                n(marks),
                n(v("ProcCooldown"))
            ));
        }
        // --- Mo & Krill ---
        "ability_intimidate" => {
            plan.debuffs.push(Debuff {
                label: "Damage amp".into(),
                effect: Effect { all_amp: v("DamageBonus"), ..Default::default() },
                seconds: v("DebuffDuration"),
                max_stacks: u32::MAX,
                per_hit: false,
            });
        }
        "ability_burrow" => {
            plan.parts = vec![dot("Spin", Kind::Spirit, v("DPS"), duration.unwrap_or(v("SpinDuration")))];
        }
        // --- Paige ---
        "ability_bookworm_dragonfire" => {
            plan.parts = vec![
                instant("Impact", Kind::Spirit, v("Damage")),
                dot("Burn", Kind::Spirit, v("DPS"), duration.unwrap_or(v("GroundFlameDuration"))),
            ];
            plan.notes.push("Assumes the target stands in the burning trail for its whole duration.".into());
        }
        "ability_bookworm_knightcharge" => {
            plan.notes.push("Counts one knight at base damage; distance amplifies it up to the listed maximum.".into());
        }
        // --- Paradox ---
        "citadel_ability_chrono_pulse_grenade" => {
            let total = seconds("AbilityDuration");
            let pulses = hit_override.unwrap_or((total / v("PulseInterval").max(0.1)).floor());
            plan.parts = vec![hits("Pulse", Kind::Spirit, v("PulseDamage"), pulses, total, 1.0)];
            plan.debuffs = vec![Debuff {
                label: "Damage amp".into(),
                effect: Effect { all_amp: v("DamageAmplificationPerStack"), ..Default::default() },
                seconds: v("DebuffDuration"),
                max_stacks: u32::MAX,
                per_hit: true,
            }];
        }
        "citadel_ability_chrono_time_wall" => {
            plan.notes.push(format!(
                "Shots fired through the wall deal +{}% bullet damage. Switch on the Shooting through Time Wall condition.",
                n(v("FriendlyBulletDamageBonus"))
            ));
        }
        "citadel_ability_chrono_kinetic_carbine" => {
            // Spirit damage worth a number of bullets, multiplied by the charge bonus.
            let bonus = props.iter().find(|p| p.key == "MaxBonusBulletDamage").map(|p| p.coef).unwrap_or(0.0);
            let bullets = props.iter().find(|p| p.key == "MaxBonusBulletDamage").map(|p| p.base).unwrap_or(0.0);
            plan.parts = vec![instant("Fully charged shot", Kind::Spirit, bullets * weapon * (1.0 + bonus / 100.0))];
            plan.notes.push(format!("{} bullets worth of weapon damage × {}; assumes a full charge.", n(bullets), n(1.0 + bonus / 100.0)));
        }
        "citadel_ability_chrono_swap" => {
            for part in &mut plan.parts {
                if let Part::Instant { health, .. } = part {
                    health.max = v("MaxHealthDamage");
                }
            }
        }
        // --- Pocket ---
        "synth_barrage" => {
            let channel = seconds("AbilityChannelTime");
            plan.parts = vec![hits("Projectile", Kind::Spirit, v("DamagePerProjectile"), hit_override.unwrap_or(v("ProjectileAmount")), channel, 1.0)];
            plan.debuffs = vec![Debuff {
                label: "Damage amp".into(),
                effect: Effect { all_amp: v("AmpPercentPerStack"), ..Default::default() },
                seconds: v("AmpDuration"),
                max_stacks: u32::MAX,
                per_hit: true,
            }];
        }
        // --- Rem ---
        "ability_familiar_attach" => {
            if v("HopOffEffecDuration") > 0.0 {
                plan.buff_seconds = v("HopOffEffecDuration");
            }
        }
        "ability_familiar_ability01" => {
            plan.parts = vec![delayed("Wake-up damage", Kind::Spirit, v("AwakeDamage"), v("AbilityChannelTime") + v("SleepDuration"))];
            plan.notes.push("Assumes the target sleeps for the full duration; damage wakes it earlier.".into());
        }
        "ability_familiar_helpinghands" => {
            plan.parts.clear();
            plan.notes.push("Helpers are not simulated.".into());
        }
        // --- Seven ---
        "citadel_ability_lightning_ball" => {
            plan.parts = vec![dot("Lightning Ball", Kind::Spirit, v("DPS"), duration.unwrap_or(v("MaxLifetime")))];
            plan.notes.push("Assumes the target stays inside the ball for its whole lifetime; set the seconds to shorten it.".into());
        }
        "citadel_ability_static_charge" => {
            plan.parts = vec![delayed("Shock", Kind::Spirit, v("Damage"), v("ShockDelay"))];
        }
        "ability_power_surge" => {
            plan.parts.clear();
            plan.buff.spirit_per_shot = v("DamagePerChain");
            plan.buff_seconds = seconds("AbilityDuration");
        }
        "citadel_ability_storm_cloud" => {
            plan.parts = vec![dot("Storm Cloud", Kind::Spirit, v("DPS"), seconds("AbilityChannelTime"))];
        }
        // --- Shiv ---
        "citadel_ability_shiv_dagger" => {
            let stacks = hit_override.unwrap_or(1.0).max(1.0);
            plan.parts = vec![dot("Bleed", Kind::Spirit, v("BleedDPSPerStack") * stacks, duration.unwrap_or(v("BleedDuration")))];
            plan.notes.push(format!("Each knife adds a bleed stack and refreshes it. Enter the stack count in the hits box (now {}).", n(stacks)));
            if on("hero:shiv_rage") {
                plan.parts.push(Part::Instant {
                    label: "Full-rage impact".into(),
                    kind: Kind::Spirit,
                    raw: 0.0,
                    health: Health { current: v("FullRageCurrentHealthDamagePct") * stacks, ..Default::default() },
                    delay: 0.0,
                    low_hp: None,
                    ramp: None,
                });
            }
        }
        "citadel_ability_shiv_dash" => {
            for d in &mut plan.debuffs {
                d.max_stacks = u32::MAX;
            }
            // At full rage an echo retraces the dash and hits again.
            if on("hero:shiv_rage") {
                plan.parts.push(delayed("Echo", Kind::Spirit, v("ImpactDamage"), 1.0));
                plan.notes.push("The echo is assumed to hit 1s after the dash.".into());
            }
        }
        "citadel_ability_shiv_killing_blow" => {
            plan.parts = vec![instant("Killing Blow", Kind::Pure, v("Damage"))];
            plan.debuffs.clear();
            plan.notes.push(format!("Executes targets below {}% health. Switch on the Full rage condition for the rage bonuses.", n(v("EnemyHealthPercent"))));
        }
        // --- Silver ---
        "ability_werewolf_unloadgun" => {
            let shots = hit_override.unwrap_or(v("MaxShots")).round().clamp(0.0, 20.0) as u32;
            plan.parts = (0..shots)
                .map(|i| Part::Instant {
                    label: "Bonus damage per shot".into(),
                    kind: Kind::Bullet,
                    raw: 0.0,
                    health: Health { current: v("CurrentHealthDamagePercentage"), ..Default::default() },
                    delay: 0.2 * i as f64,
                    low_hp: None,
                    ramp: None,
                })
                .collect();
            plan.buff = Buff::default();
            let all_hit = v("BonusCurrentHealthDamagePercentage");
            if all_hit > 0.0 && shots as f64 >= v("MaxShots") {
                plan.parts.push(Part::Instant {
                    label: "All shots hit".into(),
                    kind: Kind::Bullet,
                    raw: 0.0,
                    health: Health { current: all_hit, ..Default::default() },
                    delay: 0.2 * shots.saturating_sub(1) as f64,
                    low_hp: None,
                    ramp: None,
                });
            }
            plan.notes.push("Only the bonus damage of the empowered shots; add a Shoot action for the shots themselves. Lower the hit count if some shots miss.".into());
        }
        "ability_werewolf_kickflip" => {
            plan.parts = vec![
                instant("Kick", Kind::Spirit, v("Damage")),
                instant("Bonus on the marked target", Kind::Spirit, v("BonusDamage")),
            ];
            plan.notes.push("The bonus applies to the next shot on the marked target.".into());
        }
        "ability_werewolf_transformation" => {
            plan.notes.push("The fire rate bonus assumes maximum stacks.".into());
        }
        // --- Sinclair ---
        "ability_magician_magicbolt" => {
            plan.notes.push("Uses the maximum damage (bolt at full travel time).".into());
            if on("hero:sinclair_assistant") {
                plan.parts.push(instant("Assistant's bolt", Kind::Spirit, v("MaxDamage") * v("CloneDamagePercentage") / 100.0));
            }
        }
        "ability_magician_cloneturret" => {
            plan.parts.clear();
            plan.buff.spirit_per_shot = v("Damage");
            plan.buff_seconds = seconds("AbilityDuration");
            plan.notes.push("The Assistant attacks whenever you fire.".into());
        }
        "ability_magician_animalhexarea" => {
            let effect = Effect { all_amp: v("DamageAmpPercentage"), ..Default::default() };
            plan.debuffs.push(once("Damage amp", effect, v("HexDuration")));
        }
        // --- The Doorman ---
        "ability_doorman_hotel" => {
            plan.parts = vec![instant("Stay", Kind::Spirit, v("Damage"))];
            if on("hero:doorman_late_checkout") {
                plan.parts.push(delayed("Late checkout", Kind::Spirit, v("LateCheckoutDamage"), seconds("AbilityDuration")));
            } else {
                plan.notes.push(format!("Late checkout adds {} more spirit damage (Late checkout condition).", n(v("LateCheckoutDamage"))));
            }
        }
        // --- Venator ---
        "ability_priest_flashbang" => {
            plan.notes.push(format!(
                "The burning target deals {} pure DPS to other enemies near it for {}s; a single target takes none of it.",
                n(v("DPS")),
                n(v("BurnDuration"))
            ));
        }
        "ability_priest_beartrap" => {
            for d in &mut plan.debuffs {
                d.seconds = v("RevealDuration");
            }
        }
        "ability_priest_knockback" => {
            if on("hero:venator_wall") {
                plan.parts.push(instant("Wall bonus", kind("BonusDamage", Kind::Bullet), v("BonusDamage")));
            } else {
                plan.notes.push(format!("Targets near a wall take {} more bullet damage (Target is near a wall condition).", n(v("BonusDamage"))));
            }
        }
        "ability_priest_weaponswap" => {
            let stakes = hit_override.unwrap_or(v("StakeCount"));
            plan.parts = vec![
                hits("Stake", Kind::Bullet, v("Damage"), stakes, 0.0, 1.0),
                instant("Blessed final stake", Kind::Pure, v("BonusDamage")),
            ];
        }
        // --- Victor ---
        "ability_frank_painaura" => {
            // The debuff is reapplied for as long as the target stays in the aura.
            for d in &mut plan.debuffs {
                d.seconds += seconds("AbilityDuration");
            }
        }
        // --- Violet ---
        "ability_artist_homingbrush" => {
            // The next few bullets hit harder and splash spirit damage; they are fired at the gun's own pace.
            let shots = hit_override.unwrap_or(v("MaxShots"));
            let extra = (v("BulletDamageMultiplier") / 100.0 - 1.0).max(0.0) * weapon;
            let spread = if ctx.shots_per_second > 0.0 { shots / ctx.shots_per_second } else { 0.0 };
            plan.parts = vec![
                hits("Empowered bullets (bonus bullet damage)", Kind::Bullet, extra, shots, spread, 1.0),
                hits("Ink splash", Kind::Spirit, v("Damage"), shots, spread, 1.0),
            ];
            plan.buff = Buff::default();
            plan.notes.push(format!(
                "Counts the bonus of {} empowered bullets ({}% bullet damage) on top of the shooting; enter how many hit in the hits box. Each hit on a hero takes {}s off all ability cooldowns (not simulated).",
                n(shots),
                n(v("BulletDamageMultiplier")),
                n(v("CooldownReductionPerHit"))
            ));
        }
        "ability_artist_rollingprojectile" => {
            // Damage grows from the minimum to the maximum as the brush travels.
            let travel = duration.unwrap_or(v("MaxDamageTime")).clamp(0.0, v("MaxDamageTime").max(0.0));
            let share = if v("MaxDamageTime") > 0.0 { travel / v("MaxDamageTime") } else { 1.0 };
            let damage = v("MinDamage") + (v("MaxDamage") - v("MinDamage")) * share;
            plan.parts = vec![instant("Brush", Kind::Spirit, damage)];
            plan.notes.push(format!(
                "Deals {} to {} spirit damage, the maximum after {}s of travel; set the seconds to the travel time.",
                n(v("MinDamage")),
                n(v("MaxDamage")),
                n(v("MaxDamageTime"))
            ));
        }
        "ability_artist_tether" => {
            let channel = seconds("AbilityChannelTime");
            plan.parts = vec![dot("Sketching", Kind::Spirit, v("DPS"), channel), delayed("Finished sketch", Kind::Spirit, v("Damage"), channel)];
            plan.buff = Buff::default();
            plan.notes.push(format!(
                "Assumes Violet keeps looking at the target for the whole {}s sketch, which then stuns it for {}s.",
                n(channel),
                n(v("StunDuration"))
            ));
        }
        "ability_artist_bamboogrove" => {
            plan.parts = vec![instant("Leaving the ink", Kind::Spirit, v("Damage"))];
            plan.notes.push(format!(
                "Counts the hit of an enemy leaving the display, which also silences it for {}s. Violet and her allies inside get the spirit power.",
                n(v("SilenceDuration"))
            ));
        }
        "ability_frank_revive" => {
            plan.buff = Buff::default();
            plan.notes.push("Triggers on lethal damage. With the first upgrade, switch on the Reanimation on cooldown condition for its bonuses.".into());
        }
        // --- Viscous ---
        "viscous_goo_grenade" => {
            // One hit per bounce; repeat hits on the same target deal 50%, 38% and 26%.
            let falloff = [1.0, 0.5, 0.38, 0.26];
            let count = (hit_override.unwrap_or(1.0).round().max(1.0) as usize).min(falloff.len());
            let damage = v("Damage");
            plan.parts = (0..count).map(|i| instant(if i == 0 { "Splatter" } else { "Bounce" }, Kind::Spirit, damage * falloff[i])).collect();
            plan.notes.push("Each bounce can hit again for 50%, 38% and 26%. Enter the number of hits in the hits box.".into());
        }
        // --- Vindicta ---
        "citadel_ability_hornet_sting" => {
            plan.parts.push(Part::Dot {
                label: "Bleed".into(),
                kind: Kind::Spirit,
                dps: 0.0,
                seconds: duration.unwrap_or(v("DebuffDuration")),
                health: Health { current: v("DotHealthPercent"), ..Default::default() },
            });
        }
        "citadel_ability_hornet_snipe" => {
            let (damage, bonus) = (v("Damage"), v("LowHealthEnemyDamageBonus"));
            plan.parts = vec![Part::Instant {
                label: "Fully charged shot".into(),
                kind: Kind::Spirit,
                raw: damage,
                health: Health::default(),
                delay: 0.0,
                low_hp: (damage > 0.0).then(|| (v("LowHealthEnemyThresholdPct"), bonus / damage * 100.0)),
                ramp: None,
            }];
            plan.buff = Buff::default();
        }
        // --- Vyper ---
        "ability_viper_debuffdagger" => {
            // The hit count is used as the number of stacks already on the target.
            let stacks = hit_override.unwrap_or(0.0).clamp(0.0, v("MaxStacks"));
            plan.parts = vec![instant("Dagger", Kind::Spirit, v("Damage") + stacks * v("DamagePerStack"))];
            let shred = v("BulletResistReduction").abs() + stacks * v("BulletResistReductionPerStack").abs();
            plan.debuffs = vec![once("Bullet resist", Effect { bullet: shred, ..Default::default() }, v("StackDuration"))];
            plan.notes.push(format!(
                "Each earlier dagger on the target adds {} damage. Enter the existing stacks in the hits box (now {}).",
                n(v("DamagePerStack")),
                n(stacks)
            ));
        }
        "ability_viper_venom" => {
            plan.parts = vec![Part::Instant {
                label: "Venom".into(),
                kind: Kind::Spirit,
                raw: v("VenomMinDamage"),
                health: Health::default(),
                delay: v("VenomDuration"),
                low_hp: None,
                ramp: Some((v("VenomMaxDamage"), v("VenomMaxDamageHealthPercentage"))),
            }];
            if v("BuildUpPerShot") > 0.0 {
                plan.passives.push(Passive::BuildupProc {
                    per_shot: v("BuildUpPerShot") / 100.0,
                    raw: v("VenomMinDamage"),
                    cooldown: 0.0,
                    ramp: Some((v("VenomMaxDamage"), v("VenomMaxDamageHealthPercentage"))),
                });
            }
        }
        // --- Wraith ---
        "citadel_ability_card_toss" => {
            // Cards are not on a cooldown: weapon hits fill a card resource (CardResourcePerBulletHit per hit, more on
            // a headshot) and every ResourcePerCard of it gives a card, thrown as soon as it is there. So the cards
            // come as fast as Wraith hits, and anything that slows her fire rate slows them too.
            // One card in four is a spade (bonus damage); jokers and the diamond shred are left out.
            let spade = v("SpadeDamageBonus") / 100.0 * 0.25;
            let card = plan.parts.iter().find_map(|p| if let Part::Instant { raw, .. } = p { Some(*raw) } else { None }).unwrap_or(v("Damage"));
            let per_hit = v("CardResourcePerBulletHit");
            if per_hit > 0.0 && v("ResourcePerCard") > 0.0 {
                passive_only(
                    plan,
                    Passive::StackingBuff {
                        per_stack: Buff::default(),
                        max_stacks: 0.0,
                        // The card resource does not run out.
                        seconds: f64::INFINITY,
                        per_shot: per_hit,
                        per_headshot: (v("CardResourcePerBulletCrit") - per_hit).max(0.0),
                        proc_every: v("ResourcePerCard"),
                        proc_raw: card * (1.0 + spade),
                    },
                );
                plan.notes.push(format!(
                    "Cards come from weapon hits: {} card resource a hit ({} on a headshot), a card every {}, each thrown at once for the average of the suits (+{}% damage from spades one card in four). Jokers and the diamond resist shred are not counted.",
                    n(per_hit),
                    n(v("CardResourcePerBulletCrit")),
                    n(v("ResourcePerCard")),
                    n(spade * 100.0)
                ));
            }
        }
        // --- Yamato ---
        "citadel_ability_power_slash" => {
            plan.notes.push("Fully charged; releasing early deals 30% or 50%.".into());
        }
        // --- Kelvin ---
        "ability_icepath" => {
            plan.buff.spirit_power_pct = v("BonusSpiritPct");
            plan.buff_seconds = seconds("AbilityDuration");
            if !plan.buff.is_empty() {
                plan.notes.push("The spirit bonus applies while Kelvin stays on the Ice Path.".into());
            }
        }
        // --- Baba ---
        "ability_baba_bubbling_brew" => {
            plan.buff.spirit_power = v("TechPowerGain") * v("StolenSpiritEffectiveness") / 100.0;
            plan.buff_seconds = v("DebuffDuration");
            plan.notes.push(format!(
                "Steals {} spirit from the target, worth {}% of that for Baba. One hit per cast is counted.",
                n(v("TechPowerGain")),
                n(v("StolenSpiritEffectiveness"))
            ));
        }
        "ability_baba_bench_run" => {
            plan.parts.clear();
            plan.buff.heavy_melee_spirit = v("HeavyMeleeSpiritDamage");
            plan.buff_seconds = seconds("AbilityDuration");
        }
        "ability_baba_hexing_brew" => {
            let burn = duration.unwrap_or(v("BurnDuration"));
            plan.parts = vec![dot("Scalding brew", Kind::Spirit, v("DamagePerSecond"), burn)];
            plan.debuffs = vec![once("Spirit resist", Effect { spirit: v("SpiritResistReduction").abs(), ..Default::default() }, burn)];
            plan.notes.push("Assumes the scalding brew; the pot cycles between a barrier, a silence and this burn.".into());
        }
        "ability_baba_ultimate_2" => {
            plan.parts = vec![delayed("Pigeon barrage", Kind::Spirit, v("BarrageMaxDamage"), v("AbilityChannelTime"))];
            plan.notes.push("Assumes the full channel with maximum bread on the target.".into());
        }
        // --- Deadman Danny ---
        // Added the day he came out: these follow the game's own ability text and data.
        "ability_deadpack_chargeblast" => {
            let launched = v("MarksTotal").max(1.0);
            let landed = hit_override.unwrap_or(1.0).round().clamp(1.0, launched);
            plan.parts = vec![
                hits("Missile explosion", Kind::Spirit, v("Damage"), landed, 0.0, 1.0),
                hits("Mark consumed by a charged shot", Kind::Spirit, v("CoatedDamage"), landed, 0.0, 1.0),
            ];
            plan.notes.push(format!(
                "Counts {} missile(s) exploding on the target, each leaving a mark that a charged shot then consumes. {} missiles are launched along a line; enter how many hit in the hits box. The fading slow is not simulated.",
                n(landed),
                n(launched)
            ));
        }
        "ability_deadpack_rocketeer" => {
            plan.parts = vec![
                instant("Landing explosion (spirit)", Kind::Spirit, v("ExplosionDamage")),
                instant("Landing explosion (weapon)", Kind::Bullet, v("ExplosionBulletDamage")),
            ];
            plan.notes.push(format!(
                "Counts the explosion when Danny dives back down after flying; it has a spirit part and a weapon part. Danny takes {}% of his max health himself and is stunned for {}s if no hero is hit (not simulated).",
                n(v("MaxHealthSelfDamagePercent")),
                n(v("OverheatStunDurationSelf"))
            ));
        }
        "ability_deadpack_flamethrower" => {
            if v("BurstMarkStacks") > 0.0 {
                plan.notes.push("The last upgrade adds a Strix Salvo mark to the backblast; the extra marked damage is not counted here.".into());
            }
        }
        "ability_deadpack_ultimate" => {
            // The fall: damage for every meter of the drop from the top of the flight.
            let meters = v("MaxHeight") / crate::raw::UNITS_PER_METER;
            let low = Some((v("LowHealthEnemyThresholdPct"), v("BonusDamagePercent")));
            let part = |label: &str, raw: f64| Part::Instant { label: label.into(), kind: Kind::Spirit, raw, health: Health::default(), delay: 0.0, low_hp: low, ramp: None };
            plan.parts = vec![part("Fall", v("DamagePerMeter") * meters), part("Impact", v("Damage"))];
            plan.notes.push(format!(
                "Counts {} spirit damage per meter over the whole {}m drop, then the impact. Danny takes {}% of it himself (not simulated).",
                n(v("DamagePerMeter")),
                n(meters),
                n(v("SelfDamagePercent"))
            ));
        }
        // --- Solomon ---
        // His page on the wiki had no ability details yet; these follow the game's own ability text and data.
        "ability_chessmaster_throwbishop" => {
            let time = duration.unwrap_or(FALLBACK_SECONDS);
            let tick = if v("DPSTickInterval") > 0.0 { v("DPSTickInterval") } else { 0.25 };
            let ticks = hit_override.unwrap_or((time / tick).floor());
            plan.parts = vec![
                instant("Bishop lands", Kind::Spirit, v("ImpactDamage")),
                instant("Bishop moves through", Kind::Spirit, v("Damage")),
                hits("Diagonal square", Kind::Spirit, v("DPS") * tick, ticks, time, 1.0),
            ];
            // The last upgrade makes the squares strip spirit resist, one stack per tick.
            plan.debuffs = vec![Debuff {
                label: "Spirit resist".into(),
                effect: Effect { spirit: v("TechArmorDamageReductionPerStack").abs(), ..Default::default() },
                seconds: v("StackDuration"),
                max_stacks: v("MaxStacks").max(1.0) as u32,
                per_hit: true,
            }];
            plan.notes.push(format!(
                "Counts the landing, one pass of the moving bishop and {}s on its diagonal squares ({} damage per second). Set the seconds, or the number of square ticks in the hits box.",
                n(time),
                n(v("DPS"))
            ));
        }
        "ability_chessmaster_knightbuff" => {
            plan.parts.clear();
            plan.buff = Buff::default();
            plan.notes.push("Gives an ally stamina and a barrier (double the barrier on Solomon himself); it deals no damage.".into());
        }
        "ability_chessmaster_forkpiece" => {
            let rooks = hit_override.unwrap_or(1.0).round().clamp(0.0, 2.0 * (1.0 + v("MaxReflectCount")));
            plan.parts = vec![Part::Hits {
                label: "Rook".into(),
                kind: Kind::Spirit,
                raw: v("Damage"),
                count: rooks as u32,
                spread: 0.0,
                falloff: 1.0,
                health: Health { current: v("CurrentHealthPercent"), ..Default::default() },
            }];
            plan.notes.push(format!(
                "One rook hit is counted. Two rooks are released and each can come back off a wall {} time(s): enter how many hits land in the hits box.",
                n(v("MaxReflectCount"))
            ));
        }
        "ability_chessmaster_queen" => {
            let time = duration.unwrap_or(FALLBACK_SECONDS);
            let moves = hit_override.unwrap_or(1.0).round().clamp(1.0, 1.0 + v("QueenBonusMoves"));
            plan.parts = vec![
                hits("Queen", Kind::Spirit, v("Damage"), moves, (moves - 1.0) * v("MoveCooldown"), 1.0),
                dot("Queen's squares", Kind::Spirit, v("DPS"), time),
            ];
            plan.notes.push(format!(
                "Counts the queen landing and {}s on her squares. She can move {} more time(s), {}s apart; enter how many of her moves hit in the hits box.",
                n(time),
                n(v("QueenBonusMoves")),
                n(v("MoveCooldown"))
            ));
            if v("CheckmateThreshold") > 0.0 {
                plan.notes.push(format!("Checkmate executes heroes below {} health; the execute is not simulated.", n(v("CheckmateThreshold"))));
            }
        }
        // --- Nurse Harrow ---
        "ability_nurse_leech_therapy" => {
            plan.notes.push(format!(
                "Assumes the leeches stay on the target for the whole {}s; they also eat {}s of the target's buffs per second. Leeches left on the ground are collected afterwards (see the Leeches collected condition).",
                n(seconds("AbilityDuration")),
                n(v("LeechEatDurationPerSecond"))
            ));
        }
        "ability_nurse_adrenaline_rush" => {
            // Specter Rush speeds up every action: shooting and reloading as much as anything.
            plan.parts.clear();
            plan.buff = Buff { fire_rate: v("RushTimeScaleBonusPercent"), ..Default::default() };
            plan.buff_seconds = seconds("AbilityDuration");
            plan.notes.push(format!(
                "Cast on yourself: +{}% speed of all actions for {}s, read as fire rate. Damage taken meanwhile is put off and lands {}% larger when it ends, followed by {}s {}% slower; neither is simulated.",
                n(v("RushTimeScaleBonusPercent")),
                n(seconds("AbilityDuration")),
                n(v("DeferredDamageBonusPercent")),
                n(v("CrashDuration")),
                n(v("CrashTimeScalePenaltyPercent"))
            ));
        }
        "ability_nurse_healing_needles" => {
            // A clip of syringes for allies: no damage, and while it is loaded the gun does not shoot enemies.
            plan.castable = false;
            plan.parts.clear();
            plan.debuffs.clear();
            plan.notes.push(format!(
                "Heals allies {} per syringe ({}% more on a headshot); charges come back by dealing damage. Not part of the damage combo.",
                n(v("HealRegen")),
                n(v("HeadshotHealBonusPercent"))
            ));
        }
        "ability_nurse_fear" => {
            let fear = v("DebuffDuration") + v("FearDurationPerLeechStack") * count("hero:nurse_leeches");
            plan.notes.push(format!(
                "Assumes the target faces you, so it takes full damage until feared ({}s to build up, then {}s of fear). Facing away it takes {}%. It also slows {}% and cuts fire rate {}%, and the target cannot use stamina or movement abilities.",
                n(v("BuildupTime")),
                n(fear),
                n(v("FacingAwayDamagePercent")),
                n(v("SlowPercent")),
                n(v("FireRateSlow"))
            ));
            if v("ChannelDamageImmune") > 0.0 {
                plan.notes.push("You take no damage while channelling.".into());
            }
        }
        // --- Warden ---
        "ability_warden_lock_down" => {
            plan.parts = vec![delayed("Binding Word", Kind::Spirit, v("Damage"), v("EscapeTime"))];
            let effect = Effect { bullet_amp: v("BulletArmorReduction").abs(), ..Default::default() };
            plan.debuffs = vec![once("Bullet damage amp", effect, v("BulletArmorReductionDuration"))];
            plan.notes.push("Assumes the target fails to escape.".into());
        }
        _ => {}
    }
    plan.parts.retain(|p| match p {
        Part::Instant { raw, health, .. } => *raw > 0.0 || !health.is_empty(),
        Part::Dot { dps, health, seconds, .. } => (*dps > 0.0 || !health.is_empty()) && *seconds > 0.0,
        Part::Hits { raw, count, .. } => *raw > 0.0 && *count > 0,
    });
    plan.debuffs.retain(|d| !d.effect.is_empty() && d.seconds > 0.0);
}
