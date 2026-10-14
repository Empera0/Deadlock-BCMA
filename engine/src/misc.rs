//! Map mechanics that change with the state of the match rather than with a build.

use serde::Serialize;

/// The Unstable Rift's comeback bonus for the team that is behind in net worth, as of the 16 September 2026
/// patch (deadlock.wiki, Unstable Rift and its update history):
/// - inside the rift (35 m), the trailing team gets bullet, spirit and debuff resist;
/// - the largest the resist can be grows with the match: 10% plus 1% per minute, at most 40% (from 30:00);
/// - all comeback bonuses scale in a straight line with how far the team is behind: nothing at 0% behind, the
///   full bonus at 15% behind or more;
/// - at the full bonus the trailing team captures in 6 s instead of 12 s and the leading team needs 18 s, and a
///   trailing winner gets up to 9 more rift troopers than the usual 5.
#[derive(Debug, Clone, Serialize)]
pub struct RiftComeback {
    /// Percent of the leading team's net worth the trailing team is behind by.
    pub behind_pct: f64,
    /// How much of the full bonus applies, 0-1.
    pub scale: f64,
    /// The largest resist at this minute, and what the trailing team gets: the same for bullet, spirit and debuff.
    pub max_resist: f64,
    pub resist: f64,
    /// Seconds to capture for the trailing team and for the leading team.
    pub capture_trailing: f64,
    pub capture_leading: f64,
    /// Rift troopers a trailing team gets for winning the rift.
    pub troopers_if_trailing_wins: f64,
    /// The team asked about is the one behind.
    pub own_team_trails: bool,
}

pub const RIFT_FULL_BONUS_BEHIND_PCT: f64 = 15.0;
pub const RIFT_RESIST_START: f64 = 10.0;
pub const RIFT_RESIST_PER_MINUTE: f64 = 1.0;
pub const RIFT_RESIST_CAP: f64 = 40.0;
pub const RIFT_CAPTURE_S: f64 = 12.0;
pub const RIFT_TROOPERS: f64 = 5.0;
pub const RIFT_EXTRA_TROOPERS: f64 = 9.0;

/// The comeback bonus at a minute of the match with the two teams' net worth.
pub fn rift_comeback(minute: f64, own_souls: f64, enemy_souls: f64) -> RiftComeback {
    let (lead, trail) = (own_souls.max(enemy_souls), own_souls.min(enemy_souls));
    let behind_pct = if lead > 0.0 { (lead - trail) / lead * 100.0 } else { 0.0 };
    let scale = (behind_pct / RIFT_FULL_BONUS_BEHIND_PCT).clamp(0.0, 1.0);
    let max_resist = (RIFT_RESIST_START + RIFT_RESIST_PER_MINUTE * minute.max(0.0)).min(RIFT_RESIST_CAP);
    RiftComeback {
        behind_pct,
        scale,
        max_resist,
        resist: max_resist * scale,
        capture_trailing: RIFT_CAPTURE_S * (1.0 - 0.5 * scale),
        capture_leading: RIFT_CAPTURE_S * (1.0 + 0.5 * scale),
        troopers_if_trailing_wins: RIFT_TROOPERS + RIFT_EXTRA_TROOPERS * scale,
        own_team_trails: own_souls < enemy_souls,
    }
}
