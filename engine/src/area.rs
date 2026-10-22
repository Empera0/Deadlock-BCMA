//! How many enemies an ability hits in a team fight. The combo simulation fights one target; an ability that
//! hits an area does its damage to everyone inside it, so in a team fight it is worth more than the one target
//! shows. Each damaging ability's shape is set here by hand, from its own size properties (checked against the
//! ability text), and the enemies inside it come from the area it covers: the bigger the area, the more of the
//! enemy team stands in it, fewer and fewer more with every square meter. Ability range and radius bonuses grow
//! the sizes (calc applies them), so Greater Expansion on Death Slam catches more enemies.
//!
//! How many enemies stand in a square meter is measured, not guessed: examples/area_calibrate.rs counts the
//! enemy heroes every cast really hit in team fights of match replays and writes engine/data/area-density.json,
//! with a density of its own for every ability cast often enough there, so each one hits on average what it
//! did in those matches, and one over all of them for the rest.

use crate::calc::AbilitySheet;
use crate::combo::ComboResult;

/// The most enemies an area is expected to catch in a team fight: rarely all five at once.
pub const TEAM_FIGHT_CAP: f64 = 4.0;
/// The enemy team, and how long the fights the engine measures last, seconds.
const ENEMY_TEAM: f64 = 5.0;
pub const TEAM_FIGHT_S: f64 = 12.0;
/// Locking the enemy team down is counted as at most this share of a fight.
const CONTROL_CAP: f64 = 0.5;
/// Abilities whose area damages but whose lockdown lands on one enemy: Petrifying Bola petrifies the direct hit.
const LOCKDOWN_ON_ONE: [&str; 1] = ["ability_viper_petrifybola"];
/// Enemies around the target per square meter in a team fight, when the measurements cannot be read.
const FALLBACK_DENSITY: f64 = 0.0011;

/// The measured densities: over all area abilities, and each ability's own.
fn densities() -> &'static (f64, std::collections::HashMap<String, f64>) {
    static MEASURED: std::sync::OnceLock<(f64, std::collections::HashMap<String, f64>)> = std::sync::OnceLock::new();
    MEASURED.get_or_init(|| {
        let v: serde_json::Value = serde_json::from_str(include_str!("../data/area-density.json")).unwrap_or_default();
        let all = v["density"].as_f64().unwrap_or(FALLBACK_DENSITY);
        let own = v["abilities"].as_object().map(|m| m.iter().filter_map(|(k, d)| Some((k.clone(), d.as_f64()?))).collect()).unwrap_or_default();
        (all, own)
    })
}

/// Enemies per square meter around the target of this ability in a team fight: its own measured density, or the
/// one over all area abilities.
fn density_of(class: &str) -> f64 {
    let (all, own) = densities();
    own.get(class).copied().unwrap_or(*all)
}

/// A size: one of the ability's properties (meters or degrees), twice one (a radius given for a width), or a
/// number when the data has none.
#[derive(Clone, Copy)]
enum Size {
    Prop(&'static str),
    Twice(&'static str),
    Fixed(f64),
}

#[derive(Clone, Copy)]
enum Shape {
    /// A circle of the given radius.
    Circle(Size),
    /// A cone: its full angle in degrees, and its reach.
    Cone(Size, Size),
    /// A strip: its full width and its length.
    Line(Size, Size),
    /// A bolt that jumps to enemies within the radius, at most the given number of times.
    Chain(Size, Size),
}

use Shape::*;
use Size::*;

/// Every ability that deals its damage to more than one enemy, hero by hero. Abilities not listed hit one target.
const SHAPES: &[(&str, Shape)] = &[
    // Abrams: Siphon Life drains every enemy nearby; Seismic Impact lands on an area.
    ("citadel_ability_bull_heal", Circle(Prop("Radius"))),
    ("citadel_ability_bull_leap", Circle(Prop("ImpactRadius"))),
    // Apollo: the sigil explodes around itself; the lunges and the ultimate's slash pierce along their path.
    ("ability_fencer_throwblade", Circle(Prop("SigilRadius"))),
    ("ability_fencer_lunge", Line(Twice("SlashCollisionRadius"), Prop("SlashLength"))),
    ("ability_fencer_ultimate", Line(Twice("DashRadius"), Prop("DashRange"))),
    // Baba: Threadsap bursts along its thread; the brew covers an area; Feed The Birds covers a cone; Granny Long
    // Legs' ground pound hits around her.
    ("ability_baba_bench_run", Circle(Prop("Radius"))),
    ("ability_baba_bubbling_brew", Line(Twice("ExplodeRadius"), Prop("AbilityCastRange"))),
    ("ability_baba_hexing_brew", Circle(Prop("Radius"))),
    ("ability_baba_ultimate_2", Cone(Prop("LockonConeAngle"), Prop("AbilityCastRange"))),
    // Bebop: enemies knocked up land with an explosion around them; the bomb explodes around its target; the beam
    // hits everyone along it.
    ("citadel_ability_uppercut", Circle(Prop("OnLandDamageRadius"))),
    ("citadel_ability_sticky_bomb", Circle(Prop("Radius"))),
    ("citadel_ability_bebop_laser_beam", Line(Prop("BeamWidth"), Prop("BeamLength"))),
    // Billy: the slam's shockwave; Chain Gang chains every enemy nearby.
    ("ability_punkgoat_ult", Circle(Prop("WaveEndRadius"))),
    ("ability_punkgoat_tether", Circle(Prop("AbilityCastRange"))),
    // Calico: the bomb cluster, the slash around her, the pulse around her.
    ("ability_nano_clustergrenade", Circle(Prop("Radius"))),
    ("ability_nano_dash", Circle(Prop("SlashRadius"))),
    ("ability_nano_shadow_pulse", Circle(Prop("Radius"))),
    // Celeste: a cone, the prism's burst, the beam's explosion, and the orb bouncing between enemies.
    ("ability_unicorn_radiantblast", Cone(Prop("TargetingConeAngle"), Prop("AbilityCastRange"))),
    ("ability_unicorn_prismaticguard", Circle(Prop("ExplodeRadius"))),
    ("ability_unicorn_luminousstrike", Circle(Prop("ExplosionRadius"))),
    ("ability_unicorn_dazzlingorb", Chain(Prop("BounceRadius"), Prop("MaxBounces"))),
    // Deadman Danny: missiles along a line, the landing blast, the fuel spray.
    ("ability_deadpack_chargeblast", Line(Twice("ExplosionRadius"), Prop("Range"))),
    ("ability_deadpack_rocketeer", Circle(Prop("Radius"))),
    ("ability_deadpack_flamethrower", Line(Prop("BeamWidth"), Prop("AbilityCastRange"))),
    // Close The Deal's crash hurts everyone where it lands.
    ("ability_deadpack_ultimate", Circle(Fixed(6.0))),
    // Drifter: Rend swipes a cone.
    ("drifter_blood_blast", Cone(Prop("TargetingConeAngle"), Prop("AbilityCastRange"))),
    // Dynamo: the pulse runs along the ground; Singularity pulls in everyone nearby.
    ("citadel_ability_stomp", Line(Prop("StompWidth"), Prop("StompRange"))),
    ("citadel_ability_self_vacuum", Circle(Prop("VacuumRadius"))),
    // Graves: the Deadheads find enemies around the jar; the hands along their wall; the decree's aura.
    ("ability_necro_hauntingskull", Circle(Prop("TargetSearchRadius"))),
    ("ability_necro_zombiewall", Line(Twice("AuraRadius"), Prop("ZombieWallLength"))),
    ("ability_necro_gravestone", Circle(Prop("AuraRadius"))),
    // Grey Talon: the trap's radius and the owl's explosion.
    ("ability_immobilize_trap", Circle(Prop("Radius"))),
    ("ability_guided_arrow", Circle(Prop("ExplosionRadius"))),
    // Holliday: the barrel and the bounce pad landing.
    ("ability_explosive_barrel", Circle(Prop("Radius"))),
    ("ability_bounce_pad", Circle(Prop("Radius"))),
    // Infernus: the napalm spray, the trail behind the dash, the bomb around him.
    ("ability_incendiary_projectile", Cone(Fixed(30.0), Prop("AbilityCastRange"))),
    ("ability_flame_dash", Line(Twice("FlameAuraRadius"), Fixed(15.0))),
    ("ability_fire_bomb", Circle(Prop("Radius"))),
    // Ivy: the thorns, the smash around her, the drop's explosion.
    ("citadel_ability_tengu_urn", Circle(Prop("Radius"))),
    ("citadel_ability_tengu_stone_form", Circle(Prop("Radius"))),
    ("citadel_ability_tengu_airlift", Circle(Prop("OnLandDamageRadius"))),
    // Kelvin: the grenade's cloud and the beam.
    ("ability_ice_grenade", Circle(Prop("Radius"))),
    ("ability_icebeam", Line(Twice("PathWidth"), Prop("PathLength"))),
    // Lady Geist: the bomb and the spread of blood shards.
    ("ability_blood_bomb", Circle(Prop("Radius"))),
    ("ability_blood_shards", Cone(Prop("SpreadAngleDegrees"), Fixed(20.0))),
    // Lash: the stomp in front of him, the whip's cone, and Death Slam connecting to every enemy in its cone.
    ("citadel_ability_lash_down_strike", Cone(Fixed(180.0), Prop("Radius"))),
    ("ability_lash_flog", Cone(Prop("TargetingConeAngle"), Prop("AbilityCastRange"))),
    ("citadel_ability_lash_ultimate", Cone(Prop("LockonConeAngle"), Prop("AbilityCastRange"))),
    // McGinnis: the wall's eruption and the rockets' explosions.
    ("citadel_ability_fissure_wall", Line(Twice("WallImpactRange"), Fixed(20.0))),
    ("citadel_ability_rocket_barrage", Circle(Prop("ExplosionRadius"))),
    // Mina: Rake's cone and the bats seeking targets in theirs.
    ("ability_vampirebat_steallife", Cone(Prop("TargetingConeAngle"), Prop("AbilityCastRange"))),
    ("ability_vampirebat_batswarm", Cone(Prop("LockonConeAngle"), Prop("AbilityCastRange"))),
    // Mirage: the scarabs spread out; the whirlwind travels through enemies.
    ("mirage_fire_beetles", Cone(Prop("SpreadAngleDegrees"), Fixed(15.0))),
    ("mirage_tornado", Line(Twice("Radius"), Prop("AbilityCastRange"))),
    // Mo & Krill: Scorn and the burrow's jump around him, the sand spray in front.
    ("ability_intimidate", Circle(Prop("Radius"))),
    ("ability_burrow", Circle(Prop("Radius"))),
    ("ability_throw_sand", Line(Prop("InitialWidth"), Prop("AbilityCastRange"))),
    // Nurse Harrow: the jar's leeches spread around it; the horrors fill the half circle in front of her.
    ("ability_nurse_leech_therapy", Circle(Prop("Radius"))),
    ("ability_nurse_fear", Cone(Prop("ConeAngle"), Prop("AbilityCastRange"))),
    // Paige: the dragon's path, the magic's area, the line of knights.
    ("ability_bookworm_dragonfire", Line(Twice("Radius"), Prop("DragonTravelRange"))),
    ("ability_bookworm_aoemagic", Circle(Prop("Radius"))),
    ("ability_bookworm_knightcharge", Line(Fixed(8.5), Fixed(40.0))),
    // Paradox: the grenade's pulses.
    ("citadel_ability_chrono_pulse_grenade", Circle(Prop("Radius"))),
    // Pocket: the barrage's impacts, the cloak's path, the satchel's burst, Affliction around him.
    ("synth_barrage", Circle(Prop("Radius"))),
    ("synth_plasma_flux", Line(Twice("Radius"), Fixed(20.0))),
    ("synth_pulse", Circle(Prop("Radius"))),
    ("synth_affliction", Circle(Prop("Radius"))),
    // Rat King: the scrap grenade's shrapnel and the rats leaping onto enemies around their landing.
    ("ability_ratking_scrap_grenade", Circle(Prop("ShrapnelSphereRadius"))),
    ("ability_ratking_ratnibble", Circle(Prop("Radius"))),
    // The banner's slam.
    ("ability_ratking_standard_bearer", Circle(Prop("Radius"))),
    // Rem: the pillow's knockback area and the gaze.
    ("ability_familiar_ability02", Circle(Prop("Radius"))),
    ("ability_familiar_ability01", Cone(Fixed(40.0), Prop("AbilityCastRange"))),
    // Seven: the ball's path, the charge's burst, the shock bouncing between enemies, the storm around him.
    ("citadel_ability_lightning_ball", Line(Twice("ShockRadius"), Fixed(25.0))),
    ("citadel_ability_static_charge", Circle(Prop("ShockRadius"))),
    ("ability_power_surge", Chain(Prop("ChainRadius"), Prop("ChainCount"))),
    ("citadel_ability_storm_cloud", Circle(Prop("Radius"))),
    // Shiv: the dash damages everyone along its path.
    ("citadel_ability_shiv_dash", Line(Twice("DashRadius"), Prop("DashRange"))),
    // Sinclair: the bolt's explosion, and the hex turning every enemy in its area into a rabbit.
    ("ability_magician_magicbolt", Circle(Prop("Radius"))),
    ("ability_magician_animalhexarea", Circle(Prop("Radius"))),
    // Solomon: the bishop's path and the rooks.
    ("ability_chessmaster_throwbishop", Line(Twice("Radius"), Prop("AbilityCastRange"))),
    ("ability_chessmaster_forkpiece", Line(Fixed(4.0), Fixed(30.0))),
    // The queen strikes the squares around her.
    ("ability_chessmaster_queen", Circle(Fixed(8.0))),
    // The Doorman: the bell's explosion and the cart's path.
    ("ability_doorman_bomb", Circle(Prop("Radius"))),
    ("ability_doorman_luggage_cart", Line(Fixed(4.0), Prop("AbilityCastRange"))),
    // Venator: the grenade's fire and the shotgun's cone.
    ("ability_priest_flashbang", Circle(Prop("Radius"))),
    ("ability_priest_knockback", Cone(Prop("TargetingConeAngle"), Prop("AbilityCastRange"))),
    // Victor: the bolts' spread, the aura, the reanimation's wave.
    ("ability_frank_shocktarget2", Cone(Prop("SpreadAngle"), Prop("AbilityCastRange"))),
    ("ability_frank_painaura", Circle(Prop("Radius"))),
    ("ability_frank_revive", Circle(Prop("Radius"))),
    // Vindicta: the stake catches everyone around it.
    ("citadel_ability_hornet_chain", Circle(Prop("CaptureRadius"))),
    // Violet: the brush's explosion and the landscape.
    ("ability_artist_rollingprojectile", Cone(Prop("ExplosionCone"), Prop("ExplosionRadius"))),
    ("ability_artist_bamboogrove", Circle(Prop("GroveRadius"))),
    // Viscous: the goo, the punch's area, the ball rolling through.
    ("viscous_goo_grenade", Circle(Prop("Radius"))),
    ("viscous_telepunch", Circle(Prop("Radius"))),
    ("viscous_goo_bowling_ball", Line(Twice("BallHitRadius"), Fixed(30.0))),
    // Vyper: the bola's explosion.
    ("ability_viper_petrifybola", Circle(Prop("Radius"))),
    // Warden: the flask and Last Stand's pulses in front of him.
    ("ability_warden_crowd_control", Circle(Prop("Radius"))),
    ("ability_warden_riot_protocol", Cone(Prop("ConeAngle"), Prop("Radius"))),
    // Yamato: the slash's reach and the crimson slash in front.
    ("citadel_ability_power_slash", Line(Twice("SlashCollisionRadius"), Prop("SlashLength"))),
    ("citadel_ability_healing_slash", Cone(Fixed(120.0), Prop("Radius"))),
];

fn size(a: &AbilitySheet, s: Size) -> f64 {
    let prop = |key: &str| a.props.iter().find(|p| p.key == key).map(|p| p.value).unwrap_or(0.0);
    match s {
        Prop(key) => prop(key),
        Twice(key) => 2.0 * prop(key),
        Fixed(v) => v,
    }
}

fn shape_of(a: &AbilitySheet) -> Option<Shape> {
    SHAPES.iter().find(|s| s.0 == a.class_name).map(|s| s.1)
}

/// The square meters the ability covers, and the most targets it can reach (a chain's bounces); `None` for an
/// ability that hits one target.
pub fn covered(a: &AbilitySheet) -> Option<(f64, f64)> {
    let area = match shape_of(a)? {
        Circle(r) => std::f64::consts::PI * size(a, r).powi(2),
        Cone(angle, reach) => size(a, angle).clamp(0.0, 360.0) / 360.0 * std::f64::consts::PI * size(a, reach).powi(2),
        Line(width, length) => size(a, width) * size(a, length),
        Chain(radius, _) => std::f64::consts::PI * size(a, radius).powi(2),
    };
    let most = match shape_of(a)? {
        Chain(_, count) => 1.0 + size(a, count).max(0.0),
        _ => TEAM_FIGHT_CAP,
    };
    Some((area, most.min(TEAM_FIGHT_CAP)))
}

/// Enemies the ability hits in a team fight, the target included: 1 for a single-target ability.
pub fn targets(a: &AbilitySheet) -> f64 {
    covered(a).map(|(area, most)| enemies_in(area, most, density_of(&a.class_name))).unwrap_or(1.0)
}

/// Enemies an area catches at a density of enemies per square meter, the target included, up to `most`.
pub fn enemies_in(area: f64, most: f64, density: f64) -> f64 {
    if area <= 0.0 || most <= 1.0 {
        return 1.0;
    }
    let room = most - 1.0;
    1.0 + room * (1.0 - (-density * area / room).exp())
}

/// The ability a combo source belongs to: its name, or its name with a part in brackets.
pub fn ability_of<'a>(abilities: &'a [AbilitySheet], source: &str) -> Option<&'a AbilitySheet> {
    abilities.iter().find(|a| source == a.name || source.starts_with(&format!("{} (", a.name)))
}

/// The damage a combo does to the enemy team in a team fight: each ability's damage on every enemy it hits.
/// Shooting, melee and items count once.
pub fn team_damage(abilities: &[AbilitySheet], r: &ComboResult) -> f64 {
    r.total + extra_damage(abilities, r)
}

/// Damage on the enemies an ability hits beyond the combo's one target.
pub fn extra_damage(abilities: &[AbilitySheet], r: &ComboResult) -> f64 {
    r.sources.iter().filter_map(|s| ability_of(abilities, &s.source).map(|a| s.damage * (targets(a) - 1.0))).sum()
}

/// Share of the enemy team's fighting time an ability's lockdowns take away (0 to the cap): each cast's stun,
/// sleep or hold on every enemy it catches, over the enemy team's time in a fight of the given length.
pub fn control_share(casts: &[(&AbilitySheet, f64)], fight_s: f64) -> f64 {
    let locked: f64 = casts
        .iter()
        .map(|(a, n)| {
            let on = if LOCKDOWN_ON_ONE.contains(&a.class_name.as_str()) { 1.0 } else { targets(a) };
            n * crate::team::sheet_lockdown_seconds(a).min(fight_s) * on
        })
        .sum();
    (locked / (fight_s * ENEMY_TEAM)).min(CONTROL_CAP)
}

/// A fight's worth with its lockdowns: while enemies are locked down they deal nothing back, so taking a share X
/// of their fighting time is as good as X / (1 - X) more of everything the team's damage does.
pub fn with_control(damage: f64, share: f64) -> f64 {
    damage / (1.0 - share.min(CONTROL_CAP))
}

/// How the ability's area is described: "a 10 m half circle", for the combo's notes.
pub fn describe(a: &AbilitySheet) -> Option<String> {
    let t = targets(a);
    if t < 1.05 {
        return None;
    }
    let what = match shape_of(a)? {
        Circle(r) => format!("a {:.1} m radius", size(a, r)),
        Cone(angle, reach) => format!("a {:.0}° cone {:.1} m long", size(a, angle), size(a, reach)),
        Line(width, length) => format!("a {:.1} m wide, {:.1} m long strip", size(a, width), size(a, length)),
        Chain(radius, count) => format!("{:.0} bounces within {:.1} m", size(a, count), size(a, radius)),
    };
    let lock = crate::team::sheet_lockdown_seconds(a);
    let on = if LOCKDOWN_ON_ONE.contains(&a.class_name.as_str()) { 1.0 } else { t };
    let held = if lock > 0.0 { format!(" and locks {on:.1} of them down for {lock:.1}s") } else { String::new() };
    Some(format!("In a team fight it hits about {t:.1} enemies ({what}){held}; the combo shows one."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{build::BuildInput, calc, data::GameData};

    #[test]
    fn every_shape_names_a_real_ability_with_its_sizes() {
        let data: GameData = serde_json::from_str(include_str!("../../src-tauri/data/snapshot.json")).unwrap();
        for (class, shape) in SHAPES {
            let hero = data.heroes.iter().find(|h| h.abilities.iter().any(|a| a.class_name == *class)).unwrap_or_else(|| panic!("{class} is in no hero's kit"));
            let b = BuildInput { hero: hero.class_name.clone(), ability_tiers: vec![1, 1, 1, 1], ..Default::default() };
            let sheet = calc::compute(&data, &b).unwrap();
            let a = sheet.abilities.iter().find(|a| a.class_name == *class).unwrap();
            let sizes = match shape {
                Circle(r) => vec![*r],
                Cone(x, y) | Line(x, y) | Chain(x, y) => vec![*x, *y],
            };
            for s in sizes {
                assert!(size(a, s) > 0.0, "{class}: a size is missing");
            }
            assert!(targets(a) > 1.0, "{class}");
        }
    }
}
