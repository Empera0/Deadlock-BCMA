use engine::misc::rift_comeback;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn the_rift_comeback_scales_with_the_deficit_and_the_minute() {
    // 130k against 131k: under 1% behind, so only a sliver of the bonus.
    let r = rift_comeback(30.0, 130_000.0, 131_000.0);
    assert!(r.own_team_trails);
    assert!(close(r.behind_pct, 1_000.0 / 131_000.0 * 100.0));
    assert!(close(r.max_resist, 40.0));
    assert!(r.resist > 1.9 && r.resist < 2.1, "{}", r.resist);
    // 15% behind or more: the full bonus; the cap is 10% + 1% a minute, at most 40%.
    let full = rift_comeback(20.0, 85_000.0, 100_000.0);
    assert!(close(full.scale, 1.0) && close(full.resist, 30.0));
    assert!(close(full.capture_trailing, 6.0) && close(full.capture_leading, 18.0) && close(full.troopers_if_trailing_wins, 14.0));
    assert!(close(rift_comeback(45.0, 50_000.0, 100_000.0).resist, 40.0));
    // Even teams: nothing.
    let even = rift_comeback(25.0, 100_000.0, 100_000.0);
    assert!(close(even.resist, 0.0) && close(even.capture_trailing, 12.0) && !even.own_team_trails);
}
