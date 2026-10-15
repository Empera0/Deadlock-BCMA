# Changelog

All notable changes to Deadlock BCMA are listed here. Versions follow [semantic versioning](https://semver.org).

## [0.2.13] – 2026-10-15

This release makes the counter planner team-aware and lane-aware, and has it account for how long each item's effect actually lasts. It contains 13 changes.

### Added

1. **Your own team.** Both teams can now be picked in full: click a hero for the enemy team, right-click one for your team. Click a teammate to play that hero instead.
2. **Lane opponents.** Click enemies in the team row to mark them as in your lane (up to three). The lane counters then answer your lane opponents only. The rest of the enemy team is answered from the mid game, when they join the fights.
3. **Hero by hero.** A new table lists every enemy with what it brings and which planned item answers it at which stage. An enemy left unanswered is shown as such.
4. **Teammates' items count.** When a teammate usually buys healing reduction, the healing-reduction job counts for less for you from the mid game on. The plan says which teammate covers it.
5. **Resist reduction for the whole team.** Resist reduction is now weighed by the damage your whole team deals, not only your hero's. Bullet and spirit reduction are each valued by your team's share of that damage.
6. **Earlier purchases carry over.** The mid game knows what your lane items still do, and the late game knows what your mid-game items do. Each stage answers what is still missing instead of repeating an earlier job. Jobs your usual build already does count for less at every stage.
7. **Item timing.** Counter items are now judged by how long their effect lasts, and every card shows it:
   - **Actives:** valued by their duration against a ten-second fight, so Decay (12 s) counts in full while 5-second actives count for less. The card shows the duration and the cooldown.
   - **Repeat lockouts:** effects that cannot hit the same enemy again for a while, such as Spirit Burn (8 s on, 20 s off), count for their share of the time.
   - **Build-ups:** effects that build up over several hits are noted on the card.
8. **Flight is its own job.** Bringing flying heroes down is now a separate job ("Bring their flyers down") and only counts against heroes that fly.

### Changed

9. **Melee-triggered items.** Items that only work on melee hits, such as Spirit Snatch, now count as harder to use, like headshot-only items.
10. **Weight of top players' purchases.** How much more often top players buy an item against an enemy now counts only as far as the item answers that enemy. This stops a single statistic from outweighing the team's actual needs.
11. **Weight of mobility.** Dashes and other movement abilities now weigh less against the threats that decide fights: damage, healing and crowd control.

### Fixed

12. **Metal Skin.** Its bullet immunity is now recognised. Previously only its small passive bullet resist was counted.
13. **Repeated jobs and abilities.** An item that reduces both bullet and spirit resist no longer lists the same job twice. A flying hero's flight is no longer listed both as movement and as flight.

## [0.2.0] – 2026-10-15

This release adds the **Build generator**, with builds read from top-ranked matches and counter items planned against a chosen enemy team, and gives the app a new navigation and a more compact shop.

### Added

**Build generator** (new page): three tabs for putting a build together.

- **Calculated**: the builds the app puts together from the hero's own kit, moved here from the builder's side panel.
- **Popular**: what top players actually build on a hero.
  - Read from the last 14 days of Eternus lobbies. When a hero has too few matches there, Ascendant and above is used instead.
  - Shows a core of the 8 items players buy and keep most often, in the order they are usually bought, with the minute of purchase and how often each one is kept.
  - Below the core, a counter slot with the situational items players add depending on the match.
  - **Use this build** loads it into the builder.
- **Counters**: counter items for your hero against an enemy team you pick (up to six heroes).
  - Each enemy is profiled from its abilities and its usual build at the top ranks: how much gun or spirit damage it builds, how it heals, how it moves, which stuns and silences it has, and which resists it buys.
  - Items are judged by the jobs they do against that team:
    - cutting healing
    - blunting gun damage
    - blunting spirit damage
    - pinning down movement
    - answering crowd control
    - getting through resists
  - **Two picks for each stage of the game.** A build only has room for about two counter items, so the app picks two for the lane (up to 1,600 souls), two for the mid game (3,200) and two for the late game (6,400), each with two alternatives.
  - **How the two picks are chosen:**
    - An item that answers two problems is preferred over two items that answer one each.
    - The second pick does not repeat a job the first already covers.
    - Items already in your hero's usual build are not suggested.
  - **Items are weighed by how easily they land for your hero.** Headshot-only effects, effects that need bursts of spirit damage on a gun hero, and bullet effects on a spirit hero count for less, and each card says why.
  - **A team of healers always gets its healing cut:** one of the two picks at every stage is then a reliable healing reduction.
  - **Your own mobility is taken into account:** when your hero can escape on its own (dashes, immunity, teleports, flight), answers to crowd control count for less.
  - Every suggestion lists the enemies it answers. It also shows how much more often top players buy it against those heroes.
  - **Other counter items**: a ranked table of every other item that answers something the team does, with its price, its stage, what it does and whom it works against.
  - A folded section shows the full enemy profile and every option job by job.
  - Choose your own hero by right-clicking it in the hero grid. A hero cannot be on both teams.
- Results from the community API are cached for six hours, so switching heroes and teams is quick.

### Changed

- **Navigation**: the builder, the build generator and match analysis now open from a sidebar on the left that can be collapsed. The update button and the game version moved to the sidebar's footer.
- **Shop**: smaller item tiles laid out closer to the in-game shop, so the whole shop fits on screen with less scrolling.
- **Builder side panel**: now holds Stats, Combo and Misc only; build suggestions moved to the build generator.
- **README**: describes the build generator.

### Fixed

- Cheat Death is no longer treated as healing reduction against enemies. The healing it cuts is its owner's own while the owner is death immune. This also corrects the healing-reduction checks in match analysis.

## [0.1.0] – first release

- Build calculator with a game-like shop, hero stats at any souls and level, and stat breakdowns.
- Ability combos with damage, time to kill and healing.
- Builds the app puts together from a hero's kit, by playstyle and by stage of the match.
- Misc: the Unstable Rift comeback resist.
- Match analysis:
  - match story, role-based player ranking and titles;
  - team fights, mid boss and objectives, and deaths;
  - lanes, and build timing against the enemy team;
  - trends across your last matches.
- Windows installer with a desktop shortcut and automatic updates from GitHub releases.

[0.2.13]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.0...v0.2.13
[0.2.0]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Empera0/Deadlock-BCMA/releases/tag/v0.1.0
