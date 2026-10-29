# Changelog

All notable changes to Deadlock BCMA are listed here. Versions follow [semantic versioning](https://semver.org).

## [0.4.2] – 2026-10-29

### Fixed

1. The purchase order at the bottom of the Build maker shows each item's shop art again, framed in its category's color, instead of a mix of plain icons and art.

## [0.4.1] – 2026-10-29

### Fixed

1. Item tooltips in the shop are readable again: their values were drawn dark on dark. They also leave out the game's unnamed internal values and stay inside the window.
2. The purchase order at the bottom of the Build maker is no longer cut off.

## [0.4.0] – 2026-10-29

A new look from top to bottom, the shop drawn as the game draws it, and three new tools: hero tiers, patch changes and builds at your own ranks.

### Added

1. **Hero tiers.** A new screen: every hero at the ranks you choose, with its win rate, how often it is picked and banned, its players' K/D/A and souls, and a tier from S to D against the average hero. A hero played rarely is not put on top by a lucky run. Click a hero to see what players build with it.
2. **Patch changes.** After a patch, the app compares the game's numbers before and after: hero stats, every ability value and upgrade, item costs and values, new and removed ones. A button at the top right lists them, the ones in your saved builds first.
3. **Builds at your ranks.** The Popular and Master builds can be read from a rank range of your choice instead of the top players, on the Build generator or in Settings.
4. **Themes.** Six looks in Settings (Obsidian, Graphite, Midnight, Moss, Plum and the light Paper), with compact or comfortable spacing.
5. **Search.** Ctrl K finds any screen, hero or action.

### Changed

1. **A new layout.** The side bar is gone: the screens are tabs across the top, each screen's tools sit in one row under them, and its content is in rounded cards.
2. **The shop as in the game.** The Build maker's shop is drawn as the game's three storefronts (Fairfax, MPS and the Curiosity Catalog), with the tiers in framed panels, price tags and paper cards. Your purchase order sits at the bottom as the game's item slots.
3. **Draft as a lobby.** The two teams face each other with the chance between them, a hero picker below, and the picks and the fit table side by side.
4. **Match analysis in cards,** with a title plate that shows who won.

## [0.3.2] – 2026-10-28

The Draft screen reads any rank range and shows how heroes fit together.

### Added

1. **Fit with the placed heroes.** A table on the Draft screen: for each hero you place, how every other hero has done beside it (an ally) or against it (an enemy), and which of those results the heroes' own win rates do not explain. Click a hero in it to add it. Shown apart from the picks, which it does not change.
2. **Rank range.** The draft reads matches from a rank range you choose (from and to), on the Draft screen or as a default in Settings. It never needed your own matches; your account only adds your record on each hero.

### Fixed

1. Every hero can be placed in the draft. Heroes with few matches in the chosen ranks were hidden; now they are only left out of the suggested picks.

## [0.3.1] – 2026-10-28

This release speaks Turkish: the whole app, the engine's sentences, and, if you like, the game's Turkish names.

### Added

1. **Turkish.** Settings has a language: English or Türkçe. In Turkish, every screen, the guide, the errors and the engine's sentences (the match story, why you died, the build notes, the combo log) are written in plain Turkish, and numbers and dates are written the Turkish way. Item and ability descriptions and the names of their values come from the game's own Turkish.
2. **Names apart from the language.** Hero, item and ability names have a setting of their own: as the English game names them, or as the Turkish game does (Haze is Duman, Metal Skin is Metal Deri). The app can speak Turkish with English names, or the other way round. Build codes work either way.

### Changed

1. **Automatic checks** now also fail when a text of the app has no Turkish, and the monthly refresh also takes the game's Turkish texts and names again.

## [0.3.0] – 2026-10-23

This release reads your matches from their replays: the last seconds before each death and what each of your abilities caught. It adds a Settings screen, readable errors, automatic checks and a monthly refresh of the numbers the app measures.

### Added

1. **From the replay.** When Valve kept a match's replay, Match analysis reads it (about a minute the first time, then kept on this computer):
   - The last ten seconds before each death, hit by hit: who, with which ability, item, gun or melee, how much, and the health you had left, with every stun, sleep, silence or hold put on you and by what.
   - Each of your abilities: the casts that reached an enemy hero, how many enemies a cast caught and held, against what a cast of it catches on average at your build. Casts are judged by what they did, never by how long the ability waited.
2. **Settings.** A new screen keeps what you set once: your Steam account ID (short or long; the one the game is signed in to is offered from the console log), the Draft screen's rank, the Master builder's role and damage, watching for your match and sharing finished matches, with the console log's state and the game data. Draft uses your account without asking, the Build generator starts on your defaults, and Match analysis opens on your account; typing someone else's ID there looks at their matches without changing yours, with a button back to your own.
3. **Monthly data.** Once a month the game data, the gun share of each hero's damage and the enemies each ability hits are measured again on recent matches and replays, checked by the tests and proposed as an update.

### Changed

1. **Readable errors.** Errors are no longer shown as raw text: they say what went wrong in plain words (the statistics service is busy, cannot be reached, answered in an unknown form, had a problem of its own; the console log is missing), what to do about it, and have a Try again button. The raw message is under Details.
2. **Automatic checks.** Every change is now tested before it reaches a release: the engine's tests, the interface, and the app compiling on Windows.

### Fixed

1. **The guide's ?** now shows on every screen; it was missing on Draft and Match analysis.

## [0.2.49] – 2026-10-23

This release holds every item you bought against the minute top players of your hero buy it.

### Added

1. **Against top players.** A new section of Match analysis, under Build timing: each item the player bought, against when top players of the hero buy it (the last 14 days, Ascendant and up): their median minute and the middle half of their purchases, and whether it came early, on time, late or very late. Items most top players buy that the player never did, nor anything built on them, are listed, and a summary says how the big items came. Wins then shows how much more often top players who bought the item around the same minute won; it follows how the match was going as much as the item. Items sold again within 30 seconds are misclicks and left out.
2. **Items rushed ahead of their time.** A counter bought early takes the souls of the items top players buy before it, so the items after it come later without any souls being lost. For each purchase, the souls that went before it to items bought ahead of their time are turned into minutes at your own soul income then, shown under the item, and taken out before it is called late. An upgrade counts only what it cost on top of its components.

## [0.2.48] – 2026-10-22

This release adds a guide to every screen and fixes spirit builds that came out full of gun items.

### Added

1. **Guide.** A guide says how to use each screen step by step: getting started, the Build maker, the Build generator and Master builder, Draft, Match analysis, and the setup (`-condebug`, the Steam account ID, sharing matches). It opens on the first start, from the Guide button at the bottom of the side bar, and from the **?** beside each screen's title, at that screen.

### Fixed

1. **Spirit builds with gun items.** A spirit build was measured with the shooting counted in full, so heroes whose abilities hit softly got gun items: Shiv got Titanic Magazine, Swift Striker and Glass Cannon. The shooting and melee now count at a quarter, about the gun's share of a spirit build, while abilities count in full whatever damage they deal. Over all 44 heroes, those with four or more weapon items among the twelve best for a spirit build go from 21 to 6; the ones left are heroes whose abilities grow with weapon damage, such as Venator and Rat King.

## [0.2.47] – 2026-10-22

This release judges builds by team fights: area abilities count for every enemy they hit, measured on match replays, and stuns count for the time they hold the enemy. The death study becomes a duel, and finished matches can be shared with deadlock-api.

### Added

1. **Area abilities in team fights.** Every damaging ability of the 44 heroes has its shape (a circle, a cone, a strip, a bolt bouncing between enemies), and counts for every enemy it hits in a team fight. How many is measured, not guessed: 150,369 casts in 154 high-rank replays of this patch, with a figure of its own for 97 abilities (Lash's Death Slam 1.8 enemies a cast, Ground Strike 1.3, Seven's Storm Cloud 3.2). A bigger area still catches more. This counts in the Master builder, item suggestions, the upgrade order, the power curve and build comparison, and the combo notes say how many enemies each ability hits.
2. **Stuns count.** Stuns, sleeps, holds and fears count for the time the enemies they catch cannot fight back, in every place above.
3. **Share my finished matches.** When a match ends, the app asks deadlock-api for it, which keeps the match and, when Valve kept one, its replay for anyone to study. It reads the match number from the game's console log (`-condebug`), tries again for the replay after 30 minutes and 3 hours, and sends nothing but the match number. A switch at the bottom of the side bar turns it off.

### Changed

1. **The death study is a duel.** Each death sets the killer's full fight against yours: how fast they kill you and how fast you would have killed them, with the builds of the time. Weapon and spirit items are tried beside vitality ones, on both sides: an item that lets you kill first comes first, then the ones that change the fight most. An item that only keeps you alive under two seconds longer is no longer suggested, and each suggestion says the seconds it buys or how fast you kill with it.
2. **Ability range, radius and duration items work on abilities.** Greater Expansion, Mystic Expansion and the like now grow ability ranges and areas, and ability duration items lengthen stuns, slows and channels too, as the game marks them. The sheet has an ability radius stat beside ability range.

### Fixed

1. **Caught alone.** A death outnumbered in your own lane while it is still being laned (until the later of its two guardians falls) no longer counts as being caught alone; it reads "outnumbered in lane".
2. **Slots in the death study.** With every slot taken, a suggested item comes in place of the cheapest one you held, and says so.
3. **Zipline damage** shows as "Zip line" instead of its class name.

## [0.2.46] – 2026-10-21

This release adds Nurse Harrow, with her kit read the way the game plays it.

### Added

1. **Nurse Harrow.** The game data is updated to the latest patch, which adds Nurse Harrow to every screen: builds, combos, the Master builder, the Draft screen and match analysis.
2. **Her kit in the combo.** Leech Therapy heals her for the damage it deals. The leeches she collects are a condition you set in the combo: each adds fire rate and health regen (and spirit power once upgraded), and makes Lurking Terrors fear longer. Wretched Vapors cast on herself speeds up her shooting while it lasts. Healing Needles heals allies and is left out of the damage combo, with what it heals noted. Lurking Terrors notes its fear, slow and fire rate cut.
3. **Her kit in counters and teams.** Lurking Terrors counts as crowd control (the fear) and as a channelled ultimate that interrupting items answer. Against her, healing reduction is worth more since she heals. On your team, she already heals and helps, so team items count for a little less.

### Changed

1. **Gun and spirit damage measured again.** The share of each hero's damage that is the gun is measured again on 300 recent high-rank matches, now for 43 heroes; Nurse Harrow, measured on her own matches, deals 24% of hers with the gun. The Master builder and the counter plan judge items by these shares.

## [0.2.45] – 2026-10-18

This release scouts the players of your live match and builds around your own damage type.

### Added

1. **Lobby scouting.** Once your live match's players are in, the Master builder and the Draft screen show both teams player by player: Steam name, hero, rank, matches and win rate on that hero (pulled toward 50% while there are few) with kills, deaths and assists per match, matches and win rate overall, their three most played heroes, the streak they come in with, and who is likely queued with whom (Steam friends on the same team). Tags point out a main, a player new to the hero, a streak and a party, and your lane opponents are marked. All twelve players take three requests to the statistics service.

### Changed

1. **Items that fit your damage score higher.** In the Master builder, items whose effect comes through bullets, spirit damage or melee hits score up to 30% more when your build deals that kind of damage, and up to 30% less when it deals the other. Each such item says so ("Fits your build: works through your bullets…" or "Harder to use: …").
2. **Counters planned for the damage you build.** The counter plan now follows the damage chosen in the Master builder instead of the hero's usual one: a gun Lash cuts healing with Toxic Bullets, a spirit Lash with Healbane or Spirit Burn.

## [0.2.44] – 2026-10-18

This release finds your match from the game itself instead of the watch list of top matches.

### Changed

1. **Your live match, from your own game.** The app now reads the match you are playing from the game's console log, which Deadlock writes when it is started with the `-condebug` launch option (in Steam: Deadlock, Properties, Launch Options). The log also names your account, so no Steam ID needs typing. The teams come from the statistics service watching that match: every player's hero, team and lane, so the enemies in your lane are marked too. It works in any match, not only the top 200, and takes about a minute after the match starts.
2. **Watch for my match** reads the log every 20 seconds and builds for a new match once its teams are in. A fetch that fails is tried again after two minutes, at most three times a match, since the service watches only a few matches an hour from one computer.
3. **Load my live match** in the Master builder and the Draft screen does the same once, and falls back to the watch list without the log. A match number can also be typed in. Without the log, the app says how to turn `-condebug` on.

## [0.2.43] – 2026-10-18

This release lets the Master builder watch for your match and build for it on its own.

### Added

1. **Watch for my match.** A switch beside "Load my live match" in the Master builder: while the Master builder is open it looks for your match every 30 seconds, and as soon as a new one shows up it fills both teams and builds for it, once per match. The switch is remembered, and the line beside it says when it last looked. It reads the game's watch list, which holds only the top 200 matches being played and a match only once it has started, so the build comes in the first minutes rather than during hero selection.

## [0.2.42] – 2026-10-17

This release loads your live match, studies your deaths over many matches, works out the order to upgrade abilities in, and compares two builds.

### Added

1. **Your live match.** "Load my live match" in the Master builder and the Draft screen fills both teams from the match you are playing, found by your Steam account ID; the Master builder then builds at once. The game's watch list this comes from holds only the top 200 matches being played, and a match only once it has started; the app says so when your match is not there. The account ID is shared between the Draft and Match analysis screens.
2. **Why do I die?** A button in Match analysis studies every death of your last 10 matches and puts them together: who kills you and in how many matches, how much of the damage around your deaths is gun and how much spirit, how often the killer's fight alone was enough, and the items that would have helped in the most deaths. Each match opens with a click.
3. **Upgrade order.** The Stats tab works out which ability to put each point into with your build, and the level each step is reached at. It is measured on a 30-second fight in which every ability is cast as often as its cooldown allows, so cooldown upgrades count; a tier that adds nothing but opens a strong one is looked past, and the ultimate waits for its unlock. Upgrades that change no damage come last and are marked. The three most played orders of top players are shown with their win rates, and clicking a step sets your abilities to it.
4. **Compare.** A new tab beside Stats, Combo and Misc sets your build against a saved build or a build code: every stat side by side with the difference, fight damage, effective health and souls spent, and two charts of fight damage and effective health over the souls.

## [0.2.41] – 2026-10-17

This release explains why each item was not bought and lists the counters at the end of each phase.

### Changed

1. **Why not something else.** Every item in the list of items not bought now has a sentence saying what it lost to and why: the weakest item the phase chose on its score, and the parts it fell short on, such as how many top players buy it at that point, what it adds to your own numbers, how much of the enemy team it answers, what it does for its price, a job an item bought by then already does, fewer wins when bought there, and anything that makes it harder to use. Items that were never in the running say which bar they missed.
2. **Counters at the end of each phase.** Counter items are still paid for first, but are listed after the rest of the phase's purchases, so each phase reads as the build first and its counters after.

## [0.2.40] – 2026-10-17

This release puts the counter items into the master build itself and removes several picks that made no sense for the hero.

### Changed

1. **Counters inside the build.** The items that answer the enemy team are now bought into the master build, in the first phase of their stage that pays for them, and carry a gold "Counter" tag. The separate counter lists below the build are gone; "Hero by hero" still shows which item answers which enemy. Counters are the last items sold to make room.
2. **Real damage shares.** How much of an enemy's damage is gun and how much is spirit now comes from 200 recent Phantom+ matches, by damage type, instead of a simulated fight. Venator, Rat King, Silver and Haze are mostly gun; Wraith and Grey Talon deal most of their damage as spirit damage.

### Fixed

3. **Phantom Strike for ranged heroes.** Items that take you onto the target count for about a third for heroes that fight from range, so a caster such as Seven is no longer sent into the enemy team.
4. **Indomitable against several stuns.** It cleanses only the next stun or silence, so against a team with several (Mina, Haze, Drifter) the first one baits it; its value now drops with every stun the team has.
5. **Stacked answers.** A third item for a job two items in the build already do, such as Unstoppable on top of Debuff Reducer and Reactive Barrier, gets no credit for that job.
6. **Purchase order.** A component is never bought after the item it goes into, such as Extra Stamina after Arcane Surge.

## [0.2.39] – 2026-10-17

This release makes the Master builder and the Draft screen fast, and keeps their statistics on your computer.

### Changed

1. **Statistics kept for a month.** The win rates and purchase statistics the Master builder, popular builds and the Draft screen read are now kept on disk for 30 days, or until the game is patched, instead of six hours. The statistics service is asked about once a month; your own record on each hero is refreshed daily.
2. **Faster Master builder.** The build itself takes about a second; the wait came from some forty questions to the statistics service asked one after another. What is not kept yet is now fetched up front, four requests at a time.

### Fixed

3. **Working without the statistics service.** When the service cannot be reached, the statistics kept last time are used, from an earlier game version if need be, instead of failing.

## [0.2.38] – 2026-10-17

This release installs updates without a click.

### Changed

1. **Automatic updates.** When the app starts and a newer version is out, it now downloads it, installs it and restarts on its own; the version line shows the progress. While the app is open it looks again every 30 minutes: an update found then is downloaded in the background and waits behind a "Restart to update" button, or for the next start, so a restart never interrupts your work. If an update fails, a button tries it again.

## [0.2.37] – 2026-10-17

This release adds a draft helper that suggests which hero to pick and which to ban, from win rates fitted on real matches.

### Added

1. **Draft.** A new Draft screen in the sidebar takes your team, the enemy team, the bans and the enemies in your lane, and shows:
   - the best picks, each with the win rate points it adds to your team and the team's chance with it, broken down into the hero's own strength, its matchups against each enemy, its lane results against your lane enemies and, with a Steam account ID, your own record on it over the last 90 days ("Only heroes I play" narrows the list to them);
   - the enemy picks that hurt your team most, the best bans with how often each hero is banned, and the draft's chance as it stands;
   - a "Plan build" button on each pick that opens the master builder with both teams and the lane.

   Win rates come from the last 30 days of matches at the chosen rank. The weights were fitted on 8,700 Ascendant+ matches against the statistics of the 30 days before them, and the predicted chances match the results: drafts given 45% won 45%, drafts given 55% won 56%. Pairs of allies did not predict wins and are not counted. At this level a draft moves a match by a few points, and the screen says so.

## [0.2.36] – 2026-10-17

This release makes the shop look the same on every screen.

### Fixed

1. **Shop layout on different screens.** Shop tiles had a fixed width, so each screen size and Windows display scaling fit a different number of items per row. Every tier now shows eight items a row, scaled to the shop's width up to a cap, so the shop looks the same on every computer.

## [0.2.35] – 2026-10-17

This release explains each death in a match, advises what to buy, upgrade or sell next, and lets builds be shared as a code.

### Added

1. **Death analysis.** In the match view, the Deaths section of a player has a "Why did … die?" button. For each death it shows:
   - the killer's full fight (every ability and active item once and eight seconds of shooting) against the builds both players had at the time: how fast it kills on its own, or how much of the health it takes, and how much of it is gun damage;
   - what hit the player, by hero and source, in the match data's three-minute interval around the death;
   - the fixes: the player's own defensive actives (Metal Skin, Ethereal Shift and the like) used as the fight starts, and the vitality items that would have kept the player alive or slowed the kill most for their price, up to 1,600 souls in the lane, 3,200 until 20 minutes and 6,400 after.

   A summary names the gun and spirit share of the damage, the hero that killed the player most and the item that would have helped in the most deaths.
2. **Buy, upgrade or sell.** The Stats tab has an advisor that, for a chosen playstyle, lists the affordable items that add the most to its fight, the best upgrades of owned items, and one-for-one swaps that sell an owned item at half its price for a better one. It also shows how much each owned item adds to the fight, so the weakest is easy to see. Clicking a move applies it to the build.
3. **Build share codes.** The purchase bar has Share and Import. Share copies a short `BCMA1.` code with the hero, purchase order, souls, ability upgrades, imbues and toggled effects; Import loads a pasted code and says what is wrong with a damaged one.

## [0.2.34] – 2026-10-17

This release shows how a master build grows over the match, adds a jungle farming calculator, keeps the game data current by itself, and tells you what changed after each update.

### Added

1. **Power curve.** Under each master build, two charts show its fight damage (every ability and active item once and six seconds of shooting) and its effective health every 1,000 souls. Steps that raise the fight damage by 15% or more, and by over twice the usual step, are marked as power spikes, with the items bought there. Hovering a point shows the souls, both numbers and what was bought.
2. **Farming the jungle.** The Misc tab now shows, for a minute of the match, how long the build takes to clear a small, medium and large camp and the souls per minute of clearing such camps one after another. It counts the three basic abilities once on the camp (area abilities hit every haunt) and then the gun, with weapon damage that only works on units (Monster Rounds, Cultist Sacrifice), and Cultist Sacrifice's extra souls. Haunt health, bounties and resists come from the wiki; haunts grow 2.1% in health per minute.
3. **What's new.** After the app updates itself, a dialog shows the changes of the new version, and of any versions skipped, once.

### Changed

4. **Game data on every start.** The app now fetches the game data again each time it starts, so a new hero or patch shows up without pressing "Update data"; the status line says when a new hero arrives.

## [0.2.33] – 2026-10-17

This release lets the combo account for map buffs, parries and the enemy's defensive items, makes lane push count abilities, and connects the Master builder to the combo.

### Added

1. **Map buffs.** The combo panel has a "Your map buffs" section with the match minute, the Casting and Gun powerups, and the statue buffs collected by kind. Powerups grow from minute 5 to minute 40 (Casting: +15 to +65 spirit power and 12% to 20% cooldown reduction; Gun: +12% to +35% fire rate and +35% to +70% ammo); statue buffs count at their level for the minute (from minutes 10 and 30). The buffs belong to the build, so the Stats tab counts them too.
2. **Parry.** A Parry action stuns the target for 2.75 s, during which it takes 25% more damage from every source.
3. **The enemy's defensive items.** When the target is an enemy hero, its active items become combo actions:
   - Metal Skin makes it immune to weapon damage, Ethereal Shift to all damage, for their duration.
   - Divine Barrier, Guardian Ward and Majestic Leap give it a barrier.
   - Colossus raises its bullet and spirit resist.
   - Dispel Magic removes its debuffs and damage over time.

   Weapon and Spirit Shielding grant their barrier once enough damage of their kind lands, and Cheat Death keeps it at 1 health for 4.5 s with 60% less damage taken. Damage taken by barriers is shown apart and does not count toward the kill.
4. **Fight from the Master builder.** Each phase of a master build has a row of the enemy team, lane opponents first. Choosing an enemy loads your build as it stands at the end of that phase and opens the combo against that enemy's usual build at the same souls.

### Changed

5. **Lane push counts abilities.** Wave clear now adds the area damage of the three basic abilities, per cooldown, on the troopers they catch, so heroes that clear waves with abilities (Seven, Infernus, Violet) push with them and spirit items count toward it. Gun items such as Monster Rounds still count most for gun heroes.

## [0.2.32] – 2026-10-17

This release adds Violet and brings the damage calculation in line with the formulas documented on the Deadlock wiki.

### Added

1. **Violet.** The game data is updated to version 6793 with the new hero Violet:
   - **Living Ink:** the next 3 bullets deal more bullet damage and splash spirit damage.
   - **Flow State:** damage grows from 50 to 100 as the brush travels; set the seconds to the travel time.
   - **Still Life:** spirit damage over the 2.6 s sketch, then a finishing hit and a 1 s stun.
   - **Abstract Landscape:** damage and silence to an enemy leaving the display, and spirit power for Violet.
   - **Sketches:** sketches collected from spirit heroes are a counted combo condition.
2. **Lucky Shot and Stalker in the combo.** Lucky Shot's empowered bullets deal their damage again as bonus weapon damage (about a quarter more from the gun), and Stalker's wound deals its spirit damage over time.

### Changed

3. **Resist reductions.** Several reductions now combine by what each leaves before they come off the target's resist, so 25% and 20% make 40%, not 45%.
4. **Damage amplification on the target.** Amps from different sources, such as Napalm, Malice and Escalating Exposure, now multiply each other instead of adding up.
5. **Fire rate slows.** Slows now combine multiplicatively and lengthen the time between shots, at most by half, instead of being subtracted from the fire rate bonus.
6. **Spirit power percentages.** Percentage bonuses to spirit power, such as Boundless Spirit, now multiply each other.
7. **Alchemical Fire.** The fire's damage ramps up over its first 3 seconds, which the combo now follows.

### Fixed

8. **Headshot multipliers.** Every hero was given the usual 1.65. Drifter now deals 1.20, The Doorman and Vyper 1.35, Celeste and Kelvin 1.40, and Baba, Billy, Rem and Violet 1.45; Deadman Danny, Graves and Paige cannot deal critical damage.
9. **Melee damage.** Melee now gains half of every weapon damage bonus, and the combo applies the target's bullet resist to it.
10. **Magazine size.** Flat bonuses are added before the percentage bonuses and the result is rounded up.

## [0.2.31] – 2026-10-16

This release keeps the lane push from crowding out other lane items on spirit heroes and shows why each item was or was not bought without an extra click.

### Changed

1. **Lane push follows how the hero clears waves.** Lane push is measured with the gun, so it now counts by how much of their souls the hero's top players put into weapon items: in full from 60%, never less than a fifth, and always in full when Gun is chosen as the damage type. On spirit heroes such as Seven, Cultist Sacrifice no longer takes the lane souls only for its wave clear, which leaves room for answers to the lane opponents such as Suppressor. Gun heroes are not affected.
2. **The items not bought are shown open.** Under each phase of a master build, "Why not something else?" now opens by itself, so the best items left out and the reasons are visible at once.

## [0.2.30] – 2026-10-16

This release lets the combo run against a real enemy hero with its items and the map around the fight, and makes the Master builder count the gun on spirit heroes and the lane push in the laning phase.

### Added

1. **An enemy hero as the combo target.** The combo can now run against an enemy hero: pick the hero, its souls and its items, and its health and resists come from that build. Its active items (Metal Skin, shields and the like) are assumed unused, and the target section says so. "Manual target" goes back to the health and resists set in the Stats tab.
2. **Where the fight happens.** The combo target section has three switches:
   - **Inside the Unstable Rift:** with the minute and both teams' souls, the trailing team's comeback resist is added to the target when the enemy team is behind. When your team is behind, the resist protects you, not the target, and a note says so.
   - **Target near its own Walker:** the Walker's aura gives it 15% bullet and spirit resist (within 28 m).
   - **Target near your Walker:** the Walker's fireballs take 10% bullet and spirit resist from it. The Walker's own damage is not counted.

   Resists from the rift and the Walker stack with the target's own, each letting through a share of what is left.
3. **Lane push in the Master builder.** Clearing the trooper wave faster puts pressure on your lane opponent, so in the lane, and less in the first rift, items now score for how much faster the build clears a wave. This counts the gun's sustained damage, weapon damage that only works on troopers (Monster Rounds, Cultist Sacrifice) and bullet resist against troopers, which lets you stand in the wave. An item that clears a wave 30% faster is considered even if few players buy it, and its card says how much faster it clears, for example "Clears the wave 55% faster: pressure on the lane".

### Fixed

4. **Gun items on spirit heroes.** The Master builder measured spirit builds over the abilities alone, so no gun item could add anything, and spirit heroes such as Seven got builds without a single weapon item. Spirit builds are now measured over the abilities plus a fight with shooting between casts: spirit items still lead, and gun items earn their place by what they add.

## [0.2.29] – 2026-10-16

This release makes the Master builder value stuns by how much of a fight they take away from the enemy, the same way for every hero.

### Changed

1. **Stuns are worth their share of a fight.** While an enemy is stunned it deals nothing back, so an item's stuns now add to what it does for your own fight, not only to its answer to the enemy team. Stunning an enemy for part of a 12-second fight counts as much as doing that much more of everything else. The builder counts:
   - how many times the stun lands in a fight, from its delay, its build-up and its cooldown;
   - how easily it lands: a stun that comes with melee hits (Crushing Fists) counts in full for a melee build and at half for the others.

   For example, Crushing Fists is worth +14% to a melee fight and +7% to a gun or spirit fight, Arctic Blast +9%, Knockdown +4%, and Echo Shard on Ivy's Stone Form +14%. With this, melee Ivy now buys Crushing Fists. The cards show what each stun is worth, for example "Stuns for 0.75s, 2 times a fight: ... worth +14% to your fight".

### Fixed

2. **Stuns with no cooldown of their own.** A stun that comes with something else, such as Lightning Scroll with the ultimate, counts once a fight.

## [0.2.28] – 2026-10-16

This release makes the Master builder build properly for a playstyle a hero's top players do not use, such as melee Ivy, and counts the stuns your own items land.

### Added

1. **Your own stuns count.** An item that stuns, freezes or immobilizes enemies (Crushing Fists, Knockdown, Arctic Blast) now adds to its answer to the enemy team for as long as the stun lasts: a stunned enemy cannot fight back. This also counts the second cast of a stunning ability that Echo Shard resets, such as Ivy's Stone Form. The cards say so, for example "Stuns with a second Stone Form for 1.50s".
2. **Imbued items go into their best ability.** Items that are imbued into an ability (Echo Shard, Duration Extender, Compress Cooldown and the like) are tried in each ability they can go into and placed where they do the most. The card shows it, for example "on Stone Form".
3. **Echo Shard in fights.** The calculator now casts the ability Echo Shard resets a second time in a fight (Stone Form, Echo Shard, Stone Form).

### Changed

4. **Playstyles top players do not use.** When you choose a damage type your hero's top players do not usually build for (melee Ivy, for example):
   - The build follows what items add to that damage more than what is popular.
   - Weapon and spirit items that do nothing for it get no credit for being popular.
   - Items that clearly add to it are considered even if few players buy them.

   A note above the build says when this applies.
5. **Melee fights.** A melee fight is now twelve hits instead of six, long enough for stacking melee items to pay off. Melee hits and melee-triggered items count in full, the rest of the damage at half, so a hero's ability damage no longer drowns out what melee items add.
6. **Use this build.** Loading a master build now switches the builder to that hero, and keeps the abilities its imbued items go into.

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

[0.4.2]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.3.2...v0.4.0
[0.3.2]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.3.1...v0.3.2
[0.3.1]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.49...v0.3.0
[0.2.49]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.48...v0.2.49
[0.2.48]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.47...v0.2.48
[0.2.47]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.46...v0.2.47
[0.2.46]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.45...v0.2.46
[0.2.45]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.44...v0.2.45
[0.2.44]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.43...v0.2.44
[0.2.43]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.42...v0.2.43
[0.2.42]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.41...v0.2.42
[0.2.41]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.40...v0.2.41
[0.2.40]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.39...v0.2.40
[0.2.39]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.38...v0.2.39
[0.2.38]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.37...v0.2.38
[0.2.37]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.36...v0.2.37
[0.2.36]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.35...v0.2.36
[0.2.35]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.34...v0.2.35
[0.2.34]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.33...v0.2.34
[0.2.33]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.32...v0.2.33
[0.2.32]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.31...v0.2.32
[0.2.31]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.30...v0.2.31
[0.2.30]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.29...v0.2.30
[0.2.29]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.28...v0.2.29
[0.2.28]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.27...v0.2.28
[0.2.27]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.26...v0.2.27
[0.2.26]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.25...v0.2.26
[0.2.25]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.24...v0.2.25
[0.2.24]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.13...v0.2.24
[0.2.13]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.2.0...v0.2.13
[0.2.0]: https://github.com/Empera0/Deadlock-BCMA/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/Empera0/Deadlock-BCMA/releases/tag/v0.1.0
