# Changelog

All notable changes to Deadlock BCMA are listed here. Versions follow [semantic versioning](https://semver.org).

## [0.2.27] – 2026-10-16

This release makes the builders calculate what counter items do instead of estimating it. It also checks every hero ability against the Deadlock wiki and every item against its own in-game text.

### Added

1. **Measured counters.** What an item does to each enemy is now worked out by the calculator: the enemy's damage in fights against your hero, with its usual build, levels, boons and spirit power, is simulated with and without the item's effect. Every counter card now says how much each enemy loses, for example "Takes 18% off Haze's damage in a fight". The effects measured are:
   - fire-rate slows;
   - spirit power reduction;
   - spirit or all outgoing damage reduction;
   - disarms and silences;
   - bullet immunity, deflection and resists;
   - big-hit reduction.
2. **Win rates.**
   - Each item's win rate when top players buy it at a given point of the match is compared with everything bought at that same point. Players who get far are often already winning, so this keeps that out.
   - The difference moves the item's score, and the builds show it, for example "+4 pts wins".
   - Against each enemy, the win rate with the item is compared with the item's win rate in general.
3. **Why not something else?** Each phase of the Master builder lists the best items it did not buy. Each one shows its score, popularity, win rate, effect on your own damage and answer to the enemy, with the reasons.
4. **Your usual items count as counters.** Items from your hero's usual build are no longer left out of the counter suggestions. When one of them is a strong answer to the enemy team, it is suggested as "already in your usual build: buy it early". This is how Suppressor now shows up early against Haze or Wraith.

### Changed

5. **Ability audit.** All 42 heroes' abilities were checked against deadlock.wiki. Effects the game data does not name clearly are now counted, for example:
   - launches, pulls and stuns: Bebop, Billy, Dynamo, Holliday, Lash, Mo & Krill, Paradox, Doorman, Vindicta, Viscous;
   - silences: Celeste, Drifter, Mina, Paradox, Warden;
   - Exploding Uppercut's fire-rate slow, Afterburn's spirit damage cut, Affliction's healing prevention and Fire Scarabs' damage cut;
   - werewolf Silver's disarm and resist reduction.

   Wrong reads were removed:
   - Deadman Danny's own heat counted as a slow;
   - Djinn's Mark's stun, which does not trigger;
   - Binding Word's bullet bonus counted as a resist reduction.
6. **Item audit.** All 156 shop items were checked against their own in-game text:
   - Golden Goose Egg's and Cursed Relic's damage penalties are on their owner, not on enemies.
   - Cursed Relic, Silence Wave and Focus Lens silence; Phantom Strike disarms.
   - Inhibitor and Plated Armor now count against enemy guns.
   - Capacitor's and Lifestrike's slows are counted; Glass Cannon's unused slow numbers are not.
   - Arctic Blast's freeze counts as a stun. Knockdown no longer counts as helping allies.
   - "Close range" now comes only from items whose text says so.
   - "Headshot only" now applies only to items whose main effect needs a headshot.
7. **Timing of team effects.**
   - A teammate's ability counts for how long its effect lasts over its cooldown, upgrades included. Lash's Flog, fully upgraded, slows fire rate for 3s every 10s.
   - An ability counts for less than an item, because it reaches only the enemies it hits.
   - Ability upgrades that raise a value by a percentage are no longer added as flat seconds.
8. **No two items for the same job.** The Master builder counts a job that items already bought do for less, instead of stacking several items against the same threat.
9. **Answers on a fixed scale.** One outstanding answer, such as Metal Skin against Haze, no longer makes every other answer look small.

### Fixed

10. **Wraith's cards.** They now come from weapon hits, as in the game, instead of a short cooldown. A fire-rate slow now also slows her cards.
11. **Spirit power.** Spirit power reduced by an enemy (Spirit Sap, Threadsap) now stops at zero instead of going negative and shrinking magazines and ability damage.

## [0.2.26] – 2026-10-15

This release connects match analysis to the Master builder and makes the builder read heroes by the damage they actually deal.

### Added

1. **Compare a match with the master builder.** In match analysis, a player's details have a new **Compare with the master builder** button. It opens the Master builder with that match's teams, the player's lane opponents and the player's purchases, then builds for the same hero. Above the suggested build, each phase shows:
   - which items to buy instead of which, with the reasons on both sides;
   - suggested items to add;
   - purchases that could be skipped;
   - purchases that were good picks.

   Phases the player never reached in the match are marked as such.
2. **Purchases judged on the same terms.** The items a player bought are placed in the phase of the player's net worth at the time of purchase. They are then judged like the suggested items: how many top players buy them at that point, what they answer, whether the team already does it, and whether they suit the hero's range.

### Changed

3. **Damage by what heroes deal, not what they buy.** Whether a hero deals gun or spirit damage is now worked out by the calculator. It runs a fight with the hero's usual build at 30,000 souls: every ability and active item once, plus six seconds of shooting. Previously this came from the prices of the items in the build, which misjudged heroes that buy weapon items but deal most of their damage with abilities. This applies to the enemy team, your teammates and your own hero.
4. **Items that work through one kind of damage.** Items that only work through spirit damage (such as Suppressor), through bullets, or through bursts of spirit damage now count in proportion to how much of the hero's damage is of that kind. They are no longer suggested to heroes that deal little of it.
5. **No cheap items late.** From the second rift on, items below 1,600 souls are no longer suggested, and from the all-boons phase items below 3,200 souls. By then souls come quickly and slots are what is short. Upgrades bought on top of an owned item are still suggested.

### Fixed

6. **Local build settings removed from the repository.** A local build settings file that contained a path on the developer's computer has been removed from the repository and from its history.

## [0.2.25] – 2026-10-15

This release makes clear what the calculated builds are for. It contains 1 change.

### Changed

1. **Calculated builds: not for play.** The Calculated tab now opens with a notice:
   - These builds are not meant to be played in a match. They are the items that push the hero's main stats as high as they can go, picked by the numbers alone.
   - They can still point to items worth having in your own builds.
   - For a newly released hero, they give a general idea of what to buy for the late game.

   The previous explanation of how the builds are put together has been removed.

## [0.2.24] – 2026-10-15

This release replaces the Counters tab with the **Master builder**. It puts together a full build for your hero against the enemy team and alongside your own team, phase by phase. It contains 11 changes.

### Added

1. **Master builder.** A full build for your hero, in four phases by souls:
   - the lane (to 6,000);
   - the first rift (to 15,000);
   - the second rift (to 25,000);
   - all boons (to 40,000).

   Everyone else is assumed to build what top players usually build.
2. **What top players buy, by souls.** For each phase, the items your hero's top players buy at that point of the match, read from their net worth at the time of purchase (Ascendant and above, last 14 days). These are the backbone of the build.
3. **The engine's view.** Each item is also weighed by how much it adds to your hero's own numbers for your role and damage type, worked out by the calculator.
4. **Role and damage.** Choose your role (Auto, Carry, Tempo, Support, Frontline) and damage type (Auto, Gun, Spirit, Hybrid, Melee). Each role weighs popularity, your own numbers and the enemy team differently:
   - **Carry** builds bigger items that scale.
   - **Tempo** buys power sooner in the mid game.
   - **Support** values team items and answers to the enemy.
   - **Frontline** builds to stay alive.
5. **Late-game carries.** Heroes that win clearly more of the long matches (over 35 minutes) than of the short ones (under 25 minutes) are recognised as late-game carries. On Auto, a teammate who carries late makes your build a Tempo build. Late-game carries on the enemy team are pointed out.
6. **Your team already has it.** An item counts for less when your team already does what it does, through your own kit, your teammates' kits or their usual builds. This covers slows, stuns, silences, disarms, healing reduction, resist reduction, fire-rate and spirit-power reduction, grounding flyers and team healing:
   - One source is enough for healing reduction, disarms, grounding and fire-rate or spirit-power reduction.
   - Slows, stuns, silences and resist reduction count for a little less with every source.
   - Such items are tagged "team has it", with who does it and how.
7. **Upgrades and selling.** The build shows when an item is bought on top of one you own, and marks the smaller item as upgraded later. When the slots are full, cheap items are sold for better ones, more readily as the match goes on.
8. **Use this build.** Loads the master build into the builder.

### Changed

9. **Counter picks.** The two-per-stage counter picks are now folded under the master build. The hero-by-hero table and the other counter items stay below it.
10. **Teammates' slows and grounding.** Slows and ways to ground flyers in teammates' usual builds now lower those jobs too, like healing reduction already did.
11. **Close-range items on ranged heroes.** Passive items that only work close by (within 20 m, such as Hunter's Aura) count for half on a hero who fights from range.

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

[0.2.27]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.26...v0.2.27
[0.2.26]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.25...v0.2.26
[0.2.25]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.24...v0.2.25
[0.2.24]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.13...v0.2.24
[0.2.13]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.0...v0.2.13
[0.2.0]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Empera0/Deadlock-BCMA/releases/tag/v0.1.0
