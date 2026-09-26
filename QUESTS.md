# Jane: Quests

The quest bible. Written September 2026. Pair with `PLAN.md` (the agreement), `STORY.md` (what is true), `VOICE.md` (how Castle talks), `PLATFORM.md` section 2 (co-op) and `jane/src/sim/catalog.ts` (what a row can say).

**Status: draft for John's review.** Part 3 is built as rows and tested (see the note at its head); nothing else here is. Part 1 is research, Part 2 is the plan for all three regions, Part 3 is the Lowfields in enough detail to become rows, and the ASKS appendix lists everything Part 3 needs that does not exist yet.

Contents

1. Knowledge: what makes questing compelling and harsh, with sources, and the rule each one gives this game
2. The quest geography of the county, and the whole-game table
3. The Lowfields: nineteen side quests in full
4. ASKS: items, units, props, dialogue trees, placement, engine

---

# Part 1. Knowledge

Each principle is stated, sourced, and turned into a rule. Evidence is graded honestly:

- **Designer**: the person who built the thing said so, on the record.
- **Study**: peer-reviewed research. Most of it was not done on games; where that matters it is said.
- **Anecdote**: players' recollections, blogs, forum threads. Useful for what people remember, weak for why.

Sources marked *(read)* were fetched and read while writing this. Sources marked *(memory)* are standard references cited from knowledge and not re-fetched; check them before quoting them anywhere public.

## 1.1 Classic World of Warcraft, 2004 to 2006

### K1. Quests exist to move the player through the world

*Designer, second hand.* John Staats' *WoW Diary* records that quests were not part of the original design. They were first added only "to help direct players of the appropriate level to new zones"; the testers became dependent on them, and Blizzard then put them in every zone, which turned a level-grind game into a quest game. The quest was a travel instruction before it was a story. Source: review of *The WoW Diary*, Wolfshead Online *(read)*: https://wolfsheadonline.com/review-john-staats-wow-diary/

**So in this game:** every quest must put the heroine somewhere she has not stood, or send her back to somewhere she has stood with something changed. A quest that can be completed without walking anywhere new is cut. In a county with no fast travel, the walk is the content and the quest is the reason for the walk.

### K2. Hubs and breadcrumbs, and never a Christmas tree

*Designer.* Jeff Kaplan's GDC 2009 talk names the "Christmas tree effect": a player walks into a hub, the minimap lights up with offers, and "we've lost all control as designers to guide the players to a really fun experience". With too many offers at once the player reads none of them. The structure that worked was a hub with a few givers, short chains out and back, and a breadcrumb quest that walks the player to the next hub. Sources *(read)*: https://www.gamedeveloper.com/game-platforms/gdc-learning-from-i-world-of-warcraft-i-s-quest-design-mistakes and https://www.engadget.com/2009-03-27-kaplan-on-being-the-cruise-director-of-azeroth-at-gdc-09.html

**So in this game:** a giver offers one quest at a time. The parish board shows one notice, pinned over the others. No place in the county offers more than three quests at once, counting every giver within its fence. Each hub has one breadcrumb that ends at the next outpost (`footpath_three` walks the town to the farm).

### K3. Short text. The mystery goes in the story, never in the instruction

*Designer.* Kaplan again: quest text was capped at 511 characters, designers suffer from "medium envy", and "we need to stop writing a book in our game, because nobody wants to read it". His sharper point is about where mystery belongs: a quest may be about a mystery, but what the player must *do* is never mysterious. "Something's wrong in the forest, go and figure it out" is a broken quest. Source *(read)*: the Game Developer write-up above; also https://www.theregister.com/2009/03/27/gdc09_jeffrey_kaplan_session

**So in this game:** this is `VOICE.md` rule 2 with a number on it. A quest description is at most 320 characters. It says who asks, what is asked, and where, in plain nouns. The one thing it leaves out is the thing the asker did not say. "The farmer wants his three scarecrows counted. He would not say what number he is hoping for" is the model: the task is exact, the hole is in the motive.

### K4. Finding things by reading is the fun, and wrong directions are the failure

*Anecdote, but a famous one.* "Lost in Battle" (Mankrik's wife) is the best remembered quest in the Barrens because there were no markers, the zone was huge, and the text was the only guide. It is also remembered as a nuisance, because the directions were poor: the text pointed at a camp that was not what it said, and the body was named "Beaten Corpse", not "Mankrik's Wife". Players asked zone chat so often that the question became a joke. The lesson cuts both ways: navigation by description creates ownership of the discovery, and a description that is wrong or vague destroys trust. Sources *(read)*: https://wowpedia.fandom.com/wiki/Mankrik's_Wife and https://www.pcgamer.com/wow-classic-lost-in-battle-mankriks-wife-how-to-find/

**So in this game:** there are no quest markers (`PLAN.md` 2.5), so the text carries the whole load. Every direction names a thing the generator guarantees exists on every seed (a site, a named patch, an anchored small place: see ASKS section D) and a relation the generator guarantees ("by the well on the Halt road", "on the rim of the Top Field, farm side"). Never a compass bearing, never a distance in metres, because both change per seed. The object found must be named what the text called it. Omens may lie. Directions may not.

### K5. The zone is one story, told in fragments by many hands

*Anecdote, consistent across many retrospectives.* Duskwood is the classic zone closest to this game in tone, and its best chain, "The Legend of Stalvan", is a detective story: the player carries letters and questions between townsfolk across four zones and assembles a murder from fragments. Nobody narrates it. Each fragment is small, and the player does the joining. The zone's other chains (the Embalmer and Stitches, Mor'Ladim, the Night Watch) all say the same thing from different sides: this town is losing. Sources *(read)*: https://wowpedia.fandom.com/wiki/The_Legend_of_Stalvan_quest_chain and https://www.engadget.com/2008-09-19-know-your-lore-stalvan-mistmantle.html

**So in this game:** each region gets one sentence that every quest in it must support. The Lowfields: *the paperwork has outlived the people.* A lost-property book nobody staffs, a parish board nobody reads, a company notice to a gang that is dead, a nurse's list with nobody ticked. No quest states the sentence. `STORY.md` stays unexplained; quests add evidence, never conclusions.

### K6. Travel is content, but a long chain must pay on every leg

*Designer plus anecdote.* "The Missing Diplomat" sends an Alliance player from Stormwind to Duskwood, the Wetlands and Dustwallow Marsh over a dozen steps with very little killing, and is remembered as the first time the player felt like someone in the world and not a rat catcher. Kaplan's counter-example is the Myzrael chain: it "starts at level 30, spans 14 levels" and ends on an elite the player cannot beat, and "the player loses trust in the game". Long travel is loved when each leg delivers something and the end is reachable by the person who started it. Sources *(read)*: https://wowpedia.fandom.com/wiki/The_Missing_Diplomat_quest_chain , https://arcanewordsmith.wordpress.com/2020/03/23/classic-quests-the-missing-diplomat/ , https://www.shacknews.com/article/57889/lead-blizzard-dev-outlines-9

**So in this game:** one cross-map chain per region, no more (`left_luggage` in the Lowfields). Every leg of it ends at a fire, a find, or a new fact. A chain that begins at threat N never requires threat above N+2, and the step up is announced in the text of the quest before it.

### K7. Danger is a map, and the best-remembered danger walks the road

*Anecdote, strong and consistent.* What players recall from old Duskwood is Stitches: an elite abomination released by a quest chain who walked the road to Darkshire killing everyone he met. Players shouted "STITCHES ON THE ROAD" in chat; the town bells rang. It is remembered fifteen years later because the danger was placed in the one spot players believed was safe, it was announced, and it was shared. Elite pockets and the graveyard you were told not to cut through did the same job quietly. Source *(read)*: https://us.forums.blizzard.com/en/wow/t/stitches-duskwood-and-old-school/72422

**So in this game:** this is `PLAN.md` 2.6 already, and quests are how the player meets it. Each region has at least one quest that sends her to the rim of a patch two steps above the region's base, with a warning in plain words from someone who is themselves afraid (`after_the_bell`, `not_turnips`, `her_coat`). One thing per region should walk a road at night. It is announced by a sign first.

### K8. Collection quests fail on density, count and bag space, not on principle

*Designer.* Kaplan: "I don't think collection quests are broken, but lots of times we do a bad job." The failures he lists are too few creatures, creatures too far apart, too many items asked for, and items that eat the bag. His own worst quest, "The Green Hills of Stranglethorn", took 19 bag slots when the starting backpack had 16: "This is the worst quest in World of Warcraft. I made it." He also describes fixing drop "streaks" by raising the chance after each failure. Source *(read)*: Shacknews and Game Developer, above.

**So in this game:** bags are 24 slots and stay 24 (`PLAN.md` 2.5), so a quest may hold at most three slots at any moment, and hands them back on completion. Quest items never drop by chance: they are picked up from a place (a prop), or they drop every time. Where a kill quest asks for N, the patch holds at least N+1, close together, and they come back: one kill never holds a patch back, so each stands up again within ten game minutes (at the next ten-minute mark, when the ecology looks at its patch, and never in view), and a patch she has held back with a hunt refills within ten game minutes of being allowed to (`WORLD.md` §4.1).

### K9. The reason must live inside the world, and completion should change something

*Designer.* Kaplan on collection quests with no rationale: "You never want the player to even think somebody made the game." His fix is consequence: when the collecting is done, something happens, "there's a celebration moment to it".

**So in this game:** every kill quest names who wants it done and what they are afraid of, in their own register. Every chain ends by changing the county in a way that can be seen afterwards: a fire that now burns (`the_lampmans_brazier`), a lamp that no longer does (`to_be_collected`), a trunk standing open on the platform. The log never says what it meant (`VOICE.md` rule 5).

### K10. Rewards must make questing the smart way to play

*Designer.* Kaplan: "Experience and gold turned out to be the most important rewards", because they made doing quests the efficient way to advance whether or not the player cared about the story. *Anecdote:* what players remember fondly are the rewards that changed how they played: the first bag, the first green item, the class quest that gave a new ability.

**So in this game:** there is no XP and no coin, so the whole reward budget is things that change play: growth (a preserve jar is health, a gold-leaf page is mana and spell power), a fire in a new place, a key to something seen earlier, crafting materials for a potion she cannot otherwise make yet, Repair materials the mine will ask for. Every chain ends in one of those. A reward that is only more apples is allowed for the first link of a chain and nowhere else.

### K11. Guide without rails: what went wrong later

*Anecdote and criticism; the designers have not said this in these words.* Later expansions added map markers and rebuilt zones as linear sequences of small hubs. Players who liked the old game complain that Cataclysm-era zones were "completely on rails": one path, no reason to read, no reason to look around, nothing to get wrong. The markers removed the Mankrik problem and removed the discovery with it. Source *(read)*: https://foreveranoob.wordpress.com/2010/12/27/cataclysm-questing-is-too-linear-and-too-short/ ; also https://en.wikipedia.org/wiki/World_of_Warcraft:_Cataclysm . Further reading that could not be fetched (403): https://massivelyop.com/2025/09/12/casually-classic-the-quest-design-that-world-of-warcraft-kind-of-forgot/

**So in this game:** chains are short (two to four) and independent. Any chain in a region can be started without finishing another. The only ordering is inside a chain. Kaplan's "bad flow" example was four kill quests followed by four collection quests; no chain here repeats a requirement type in consecutive links unless the second one changes the conditions (day, then night).

## 1.2 What keeps people playing: the research

### K12. Autonomy, competence, relatedness

*Study, and one of the few done on games.* Ryan, Rigby and Przybylski (2006) ran four studies and found that enjoyment and the wish to keep playing were predicted by how far a game satisfied three needs from Self-Determination Theory: **autonomy** (I chose this), **competence** (I am getting better and the game shows me) and **relatedness** (I matter to someone here). Rigby's later PENS model is the applied version. Clear controls, consistent feedback and real choice of goals raised the scores. Sources *(read, abstracts)*: https://selfdeterminationtheory.org/SDT/documents/2006_RyanRigbyPrzybylski_MandE.pdf and https://selfdeterminationtheory.org/player-experience-of-needs-satisfaction-pens/

**So in this game:** autonomy is free order between chains and the right to refuse (every offer has a "not now" that costs nothing). Competence is felt through threat: the patch that killed her on the second day is walkable on the sixth, and no number tells her so. Relatedness is thin by design (the living are behind doors), so it is carried by the dog, by givers who remember what she did, and by co-op. A quest whose completion nobody and nothing acknowledges fails this test.

### K13. Flow needs a dial the player holds

*Study (psychology), applied to games by designers.* Csikszentmihalyi's flow is the state between anxiety (challenge above skill) and boredom (skill above challenge). Sweetser and Wyeth's GameFlow (2005) turned it into eight criteria for games; Jenova Chen's thesis argues the best way to keep different players in flow is to let them set the difficulty themselves through their choices inside the world, not through a menu. Sources *(read, abstracts)*: https://dl.acm.org/doi/10.1145/1077246.1077253 and https://www.jenovachen.com/flowingames/p31-chen.pdf

**So in this game:** the threat map is the dial. Every region offers quests at its base threat, one step up and two steps up at the same time, and says which is which in the fiction. A player who wants it harder goes to the Top Field on the first night. A player who wants it gentler does the station's chain first. Nothing scales to her.

### K14. The goal-gradient effect: people speed up near the end

*Study, not on games.* Kivetz, Urminsky and Zheng (2006) showed in field experiments with coffee cards that people buy faster the closer they are to the free coffee, and that the illusion of progress works as well as real progress. Source *(read, abstract)*: https://journals.sagepub.com/doi/abs/10.1509/jmkr.43.1.39

**So in this game:** counts are small enough to see the end from the start: three places, five men, six pumpkins. Kill toasts show "4/6" (built). Chains are short enough that after the first link the last is in sight. This is the honest engine behind "one more quest": the next end is always near.

### K15. Endowed progress: a head start beats a blank card

*Study, not on games.* Nunes and Drèze (2006): car-wash cards needing eight stamps were completed by about 19 percent of customers; cards needing ten with two already stamped were completed by about 34 percent. Same work, more finishing. Source *(memory)*: Journal of Consumer Research 32(4), https://doi.org/10.1086/500480 ; summary *(read)*: https://www.coglode.com/research/goal-gradient-effect

**So in this game:** the engine already credits `location` and `acquire` from before the quest was taken (`quests.ts`). Use that on purpose: put the first object of a chain where she will have passed it. Two of the three lost things in `lost_property` lie on the walk she made on the first evening. Taking a quest and finding it one third done is a gift that costs nothing.

### K16. Open loops pull people back. The famous version of this claim is wrong

*Study, with a correction.* The Zeigarnik effect (unfinished tasks are remembered better) is quoted in every game design talk. A 2025 meta-analysis by Ghibellini and Meier found no reliable memory advantage for interrupted tasks. What does replicate is the **Ovsiankina effect**: given the chance, people go back and resume an interrupted task about two thirds of the time. The pull is behavioural, not a memory trick. Source *(read, abstract)*: https://www.nature.com/articles/s41599-025-05000-w

**So in this game:** do not rely on the player remembering; rely on her wanting to go back when she next passes. Locked things are shown early and stand beside a road she will walk again: the trunk on the platform on the first evening, the rock across the adit on Quarry Steps, the dead lamps. The log holds few quests, so each open one is felt. Because saving is only at beds and fires, a session tends to end at a hub, which is exactly where the next offer is.

### K17. Curiosity is a gap the player believes she can close

*Study.* Loewenstein (1994): curiosity arises when attention is drawn to a specific gap in what one knows, and it is strongest when the gap is small and the missing piece feels within reach. Large, vague unknowns do not produce curiosity; they produce indifference. Later game research adds that players tolerate a gap only while they are confident they can close it. Sources: Loewenstein, "The Psychology of Curiosity", Psychological Bulletin 116(1) *(memory)*: https://www.cmu.edu/dietrich/sds/docs/loewenstein/PsychofCuriosity.pdf ; To, Ali, Kaufman and Hammer, "Integrating Curiosity and Uncertainty in Game Design", DiGRA 2016 *(search summary only)*: https://dl.digra.org/index.php/dl/article/download/793/793/790

**So in this game:** this is the research behind `VOICE.md` rules 3 and 4. One wrong detail, and it must be checkable. "There is no No. 14" is a gap of exactly one lamp, and she can walk the road and count. "Something terrible happened here" is a gap of unlimited size and nobody walks anywhere for it.

### K18. Variable rewards work, which is why they need rules

*Study (animal learning), applied to games by a practitioner.* John Hopson's "Behavioral Game Design" (2001) set out how reward schedules shape play: variable-ratio rewards (unpredictable payout per action) produce the most persistent behaviour, which is why slot machines and random drops use them. The same fact is the ethical problem: persistence is not enjoyment, and a schedule can keep someone playing past the point where she is glad to be. Source *(read, summary)*: https://www.gamedeveloper.com/design/behavioral-game-design

**So in this game:** the county's variable reward is the omen: true about a third of the time, per seed. That uncertainty is in the world and is settled once, not re-rolled per kill. No quest item has a drop chance. No reward is random. No quest repeats daily. Nothing in the game is built to be done again for another pull of the lever. If a player stops for the night at a fire, the design has worked.

### K19. Loss makes risk mean something, if the risk was chosen

*Study.* Kahneman and Tversky's prospect theory (1979): losses weigh roughly twice as much as equal gains. A game with nothing to lose cannot make a walk frightening. *Designer:* Hidetaka Miyazaki on why his games are hard: "It's not a matter of simply cranking up the difficulty; it's doing so fairly", and death is justified when the player "can understand why they were killed". The classic WoW corpse run is the same bargain: the penalty is time and distance, never the character. Sources: Kahneman and Tversky, Econometrica 47(2) *(memory)*: https://www.jstor.org/stable/1914185 ; Miyazaki *(read)*: https://www.gamesradar.com/games/action-rpg/fromsoftware-head-hidetaka-miyazaki-says-games-like-elden-ring-and-dark-souls-arent-about-simply-cranking-up-the-difficulty-its-doing-so-fairly/ and https://80.lv/articles/dark-souls-creator-explained-why-fromsoftware-games-are-so-difficult

**So in this game:** death costs the walk back from the last fire, and nothing else. That is already harsh in a county ten minutes wide. It is fair only if she chose the risk, so: every quest that leaves the region's base threat says so before she accepts, in the giver's words. No quest hides its danger to make a surprise. Placing a fire near a dangerous patch is how a quest is tuned; the brazier in `the_lampmans_brazier` exists to shorten the run back from the east Lowfields, and she earns it.

### K20. People play for different things. Know which ones this game serves

*Theory, then survey.* Bartle (1996) sorted MUD players into achievers, explorers, socialisers and killers; it is an armchair taxonomy, influential and unproven. Quantic Foundry's model is empirical: factor analysis of surveys from several hundred thousand players gives twelve motivations in six pairs (action, social, mastery, achievement, immersion, creativity). It is self-report, but large and stable across regions. Sources: Bartle *(memory)*: https://mud.co.uk/richard/hcds.htm ; Quantic Foundry *(read)*: https://quanticfoundry.com/gamer-motivation-model/ and https://quanticfoundry.com/2015/12/21/map-of-gaming-motivations/

**So in this game:** Jane serves **Discovery** and **Fantasy/Story** first (immersion and creativity), **Challenge** and **Completion** second, **Community** through co-op. It does not serve Competition, Destruction or Power-as-numbers, and should not try. For quests this means: more finding and reading than killing; a log that can be finished per region (Completion) and says so; nothing ranked, timed or scored.

### K21. A quest should contain a decision

*Designer.* Sid Meier: "A game is a series of interesting decisions." In his 2012 talk he defines one by exclusion: if everyone picks the same option, or if the choice might as well be random, it is not a decision. Good ones trade something now against something later, or one kind of safety against another. Source *(read)*: https://www.gamedeveloper.com/design/gdc-2012-sid-meier-on-how-to-see-games-as-sets-of-interesting-decisions

**So in this game:** with three requirement types the decisions live around the quest, not in its rows: go now or after a rest, by the lit road or the short path, by day when it is safer or by night when it counts. Each chain should also hold one real trade. In the Lowfields: the car's two planks and fire stone will light the lampman's brazier or mend a stair in the mine, not both, until more wood is found. The parcel on the platform can be taken, and the platform lamp never lights again.

### K22. Let the place tell it, and let it be missable

*Designer.* Harvey Smith and Matthias Worch, "What Happened Here?" (GDC 2010): environmental storytelling invites the player to assemble what happened from what is left, and a conclusion she reaches herself is held harder than one she is told. Smith: "It has to be possible to miss some things to make finding them meaningful." Sources *(read)*: https://niemanstoryboard.org/2011/01/14/harvey-smith-on-environmental-storytelling-and-embedding-narrative/ and https://www.worch.com/2010/03/11/gdc-2010/

**So in this game:** this suits a county where the living are rare. Most givers are things: a book, a board, a stone, a glovebox, a slate. Each is a scene that reads without the quest (eleven unopened parcels on a step) and the quest is only the reason to look. Side quests are never required by the spine, so all of them can be missed.

### K23. Harsh but fair: the world teaches, the landmark pulls

*Designer.* Miyamoto built the first Zelda on the memory of exploring caves as a child without a map, and wanted the player to feel she was "becoming familiar with the history of the land". *Breath of the Wild*'s team described placing large shapes on the horizon so that there is always something visible to walk toward and something hidden behind it. Sources *(read)*: https://www.videogameschronicle.com/features/zelda-at-40-how-shigeru-miyamotos-childhood-explorations-inspired-nintendos-legendary-classic/ and https://www.nintendolife.com/news/2017/10/zelda_breath_of_the_wilds_ingenious_design_is_all_about_triangles_apparently

**So in this game:** the School is the landmark (`PLAN.md` 2.2). Quests should use what can be seen: "the rim of the Top Field", "past the last lit lamp", "the top of Quarry Steps". Fair means four things, all checkable: she was warned; she could see it coming; her death has a cause she can name; and the way back is long but never blocked.

## 1.3 What not to do

| Do not | Why | Source |
| --- | --- | --- |
| Fetch quests with no asker and no consequence | "You never want the player to think somebody made the game" | Kaplan, K9 |
| Kill ten of anything without a reason in the fiction | The undead are townsfolk (`STORY.md`); a body count with no cause turns unease into pest control | K9, `STORY.md` 7.3 |
| Quest items on a drop chance | Wastes time, produces streaks, and in a game with no XP the wasted kills pay nothing at all | Kaplan, K8, K18 |
| More than three bag slots for one quest | Green Hills took 19 of 16. Bags here are 24 for ever | Kaplan, K8 |
| A mysterious instruction | The story may be unclear; the task may not | Kaplan, K3 |
| Directions that are wrong, or that depend on a seed | Omens may lie; directions may not | Mankrik, K4 |
| A chain that outgrows the player who started it | Trust, once lost, is not given back | Kaplan on Myzrael, K6 |
| A hub that offers everything at once | Nothing gets read | Kaplan, K2 |
| Text that confirms an omen, or contradicts `STORY.md` | `VOICE.md` rule 5; the county is undecided, not explained | |
| A quest that cannot be handed in | The Phaser build shipped 102 quests; 61 had no hand-in and 33 were never offered. Its dog tree also made two hand-ins unreachable by row order | `archive/phaser-remake-2026/POSTMORTEM.md` P0.4, P0.5 |
| A once-only trigger that is the only way to complete something | It fired out of order and the progress was gone for good | POSTMORTEM P0.5 |
| A quest that needs the party to split | At four players each is at 38 percent alone | `PLATFORM.md` section 2 |
| Rewards invented to fill a slot: coins, reputation, cosmetic titles | The game has none of these systems. Do not add one for a quest | `PLAN.md` cohesion test 1 |

## 1.4 The checklist

Hold every quest against this page. A "no" anywhere is a redline.

**It belongs**

1. Does it support its region's sentence (Lowfields: *the paperwork has outlived the people*) without stating it?
2. Is the giver someone or something that would really ask this, in a register from `VOICE.md`?
3. Does the text contain exactly one wrong detail, and can she go and check it?
4. Does it leave `STORY.md` unexplained? No text confirms an omen, names what the dog is, or says what happened to anyone.

**It is clear**

5. Is the description 320 characters or fewer, and does it say who, what and where in plain nouns?
6. Does every direction name a site, a named patch or an anchored place, and a relation the generator guarantees? No bearings, no metres.
7. Is the thing found called what the text called it?

**It moves her**

8. Does it take her somewhere new, or back somewhere changed?
9. Is the threat it plays at written down, by day and by night? If it leaves the region's base, does the giver warn her before she accepts?
10. Is there a fire within one run of the hardest part?

**It respects her time**

11. Requirements are only kill, acquire, location. Counts are six or fewer.
12. No drop chances. At most three bag slots. Slots are given back at hand-in.
13. For kill quests: N+1 targets, close together, returning (one kill is back within ten game minutes, a held patch within ten game minutes of being allowed to, K8); and a named asker with a named fear.
14. Is the first step placed where she may already have been (endowed progress)?

**It pays**

15. Does the chain end in growth, a fire, a key to something seen earlier, or materials she needs next? Is the completion visible in the world afterwards?
16. Is there one real decision in or around the chain?

**It cannot break**

17. Is there a row that offers it and a row that hands it in, both reachable, and is the hand-in row *above* every idle row in its tree? Does the giver's tree also hold a `questReady` fallback if completion normally happens elsewhere?
18. Does it survive being done out of order: place visited first, item found first, giver met last?
19. Do its rewards survive being run once per player (no `flag` with `add`, no single-use key handed out four times)?
20. Does it work for a party that stays together, and never ask them to part?

---

# Part 2. The quest geography

## 2.1 Ground rules

**Names, never coordinates.** Every seed moves everything. A quest may refer to:

- a **site** id from `sites.json` (`station`, `town`, `farm`, `car_wood`, `gold_mine`, `julie_house`);
- an **area** id from `areas.json` (`top_field`, `long_hedge`, ...);
- an **anchor**: a small place of a stated kind from `pois.json`, placed under a stated constraint and given a stable name (ASKS section D). `poi_<n>` marks are indexes and mean nothing to a writer; anchors are the fix;
- a **key** or **mark** inside a set chunk (`town_square`, `farm_gate`, `car_wreck`).

**How a giver works, with the engine as it stands.** A prop may carry a `talk` tree. A tree's `start` list picks the first row whose conditions hold; a node's option may run `quest` or `handin`. So a notice board, a book or a gravestone is a full quest giver today, with no new code. A prop's `use` list runs after its loot is taken, so picking something up can fire `location`, `quest` or `handin`. The `night` condition exists, so "only after the bell" is a row.

**What the engine forces on the design** (all found by reading `actions.ts`, `quests.ts`, `dialogue.ts`, `interact.ts`):

- `kill` counts by unit **def**, and only while the quest is in the log. A kill quest tied to a place therefore needs its own unit row (`pumpkin_top`), or any pumpkin anywhere counts.
- `location` and `acquire` count retroactively. Good: use it (K15).
- Rewards run **once per player**. `give` pays everyone. `show`, `hide`, `unlock` are harmless when repeated. `flag` with `add` is not. A consumed key given as a reward would be handed out four times and clutter three bags: keys are therefore **found as loot**, not rewarded.
- A plain key is consumed when it turns. So a quest must never require `acquire` of a key it also expects her to use. The pattern used below: picking the key up fires a `location`; opening the locked thing fires another and hands in.
- `take` removes from the talker only. In co-op a quest item may be left in a friend's bag. None of the items below do anything afterwards. See ASKS section F, item 4.
- A node's own `actions` run with whichever option is chosen, including the refusal. Accept actions go on the option.
- The dog is not there from nine to six, so anything handed to the dog is handed in by day. Things can be handed in at any hour, which is one more reason most givers are things.
- `ZoneId` is a closed list. There are no new interiors. **Every side quest in the Lowfields plays outdoors in the county zone**, and the living speak through doors.
- There is no coin. Every reward is an item, a key, a fire or growth.

**The living are behind doors.** *(Proposed, for John to confirm.)* `STORY.md` says the townsfolk are "the ones inside the lamps" and that nobody looks east. In the Lowfields they do not come out at all. The farmer talks through his door. Mrs Allen talks through her sister's letterbox. They are polite, they are in the middle of something, and the door does not open. This costs no art (a door is a prop with a `talk` tree), suits the tone better than a cheerful villager sprite, and makes the dog on the step the only face in the region. A later art pass can put a shape at a window.

**Growth from quests.** *(Proposed numbers.)* A small preserve jar is +3 strength (15 health). A gold-leaf page is +3 spirit. The Lowfields hand out three of each through quests (below), and two more of each should lie loose in the dangerous patches for whoever explores. With the mine's big jar that takes her from 150 health to about phase 2's 300, which is `PLAN.md` 2.6's "half from bosses, half found". Growth is an action, not a bag item: see ASKS section F, item 1.

## 2.2 The Lowfields (base threat 1)

*The paperwork has outlived the people.* Farmland, hedges, a town that still puts up notices. By day it looks normal. The quests are errands left by institutions that have no one behind them any more.

| | Place | What is there for quests |
| --- | --- | --- |
| **Hub** | `town`, Castle (threat 0, fire) | The parish board in the square (three notices, one at a time). Mrs Allen's sister's door. The allotments a short walk out |
| **Outpost** | `station`, Castle Halt (threat 0, fire) | The lost-property book, the unclaimed shelf, the Goldskin trunk. The start of the game, and where two chains end |
| **Outpost** | `farm`, Lowfield Farm (threat 0, fire) | The farmer's door. Three scarecrows. The far end of Footpath No. 3 |
| Home | `julie_house` (bed, bench, the dog) | The spine, and Julie's garden book |
| Forward camp | `gold_mine` mouth (fire) | The Company notice and the pay hatch |

| Patch | Threat | Hosts | Role |
| --- | --- | --- | --- |
| `allotments` | 2 | `rats_in_the_sheds`, `plot_nine` | First step up, five minutes from the hub's fire. A night quest with a short run home |
| `long_hedge` | 2 | `footpath_three`, `if_found`, one scarecrow | The shortcut that costs danger. Breadcrumb from hub to outpost |
| `sallow_bottom` | 2 | `before_the_bell`, Mrs Allen's cottage | Herb ground. The rose omen |
| `top_field` | **3** | `after_the_bell` (rim only), `not_turnips` | **Dangerous detour 1.** Warned by a frightened man |
| `behind_the_car` | **3** | `her_coat` | **Dangerous detour 2.** Warned by the woman who went in |
| `quarry_steps` | **3** | `stood_down`, `down_at_five` | **Dangerous detour 3.** Pays in Repair materials for the mine; shows a verb gate for later |

| Small place (kind) | Used as |
| --- | --- |
| `well`, `signpost`, `cart` on the safe road | The three lost things (`lost_property`): the reading tutorial, on the walk she has already made |
| `cart` on the mine road | The carter's cart (`left_luggage`) |
| `shrine` beside the last lit lamp | A. Pell's stone: giver of `number_fourteen` and `the_lampmans_brazier` |
| `scarecrow` x3 | The farmer's count, by day and by night |
| `cottage` x3 | The nurse's round |
| `hollow_tree` x2 | The walker's haversack; the nurse's coat |
| `camp` at the foot of Quarry Steps | The gang's dinner camp and tally slate |
| `jetty` at Sallow Bottom | Where the wild roses are |
| `stones` | Left free for omens |

**How the chains lead out and back.**

```
Castle Halt --(safe lamp road: glove, hat)--> Julie's --(tin)--> CASTLE
   ^   book, shelf, trunk                       garden book         board, Mrs Allen's door
   |                                               |                   |
   |                                        Sallow Bottom (2)     allotments (2)
   |                                                                   |
   |                                   Footpath No. 3 through the Long Hedge (2)
   |                                                                   v
   |     the wood with the car <----- Pell's stone, dead lamps ----- LOWFIELD FARM
   |        |  behind the car (3)        brazier (new fire)           Top Field (3)
   |        v
   |     three cottages ----> back to Castle (Mrs Allen)
   |
   +---- mine road: carter's cart, Company notice, Quarry Steps (3) ----> the Gold Mine
```

The station chain opens the region and closes it: its first quest is done on the first walk, its second sends her the whole width of the Lowfields to the mine road and back to the platform she started on, stronger. The town's board pushes her to the farm. The farm pushes her to the Top Field. The stone on the farm road pushes her along the dead lamps toward the wood. The car pushes her round three patches and back to the hub. The mine road pays in what the mine will ask for.

**Suggested order for a new player** (nothing enforces it): `lost_property`, the spine to the kitchen, `before_the_bell`, `footpath_three`, `three_scarecrows`, `rats_in_the_sheds`, `number_fourteen`, `the_nurses_round`, then the three detours once she has a jar or two.

## 2.3 The Waters (base threat 2 to 3). Outline only

*Everything here was tended once.* Gardens, locks, reed beds, a museum: things that needed looking after and are now looking after themselves.

- **Hub:** `reed_camp`, the Reedcutters' fire. **Outposts (to be built at M6):** a tollhouse with a fire at `tollhouse_reach` (threat 2, on the road); the portico of `library` with a fire.
- **Patches:** `tollhouse_reach` 2 (the way in), `eel_beds` 3, `glasshouse_row` 3, **`drowned_lane` 4** (on the road: here the road is the danger and the detour is the safe way, which inverts the Lowfields), **`mothers_garden` 4**, **`still_pool` 4**.
- **Small places:** `jetty`, `boat`, `greenhouse`, `statue`, `pump`, `well`, `cottage`, `stones`.
- **Growth:** three jars, three pages from quests; verbs Explosion and Grow come from the spine and reopen the Lowfields (the adit on Quarry Steps; a dry bed at Sallow Bottom).
- **Omens carried:** the bridge counts who crosses; nobody has drowned in the lake; do not pick the white roses after dark.

## 2.4 The Works (base threat 4 to 5). Outline only

*The shift never ended.* Slag, rails, a canteen with the rota still on the wall.

- **Hub:** `canteen`, the works canteen (threat 2 by `PLAN.md`, fire). **Outposts (to be built at M7):** a watchman's hut with a fire on `cinder_walk` (3, on the road); the graveyard gate lodge.
- **Patches:** `cinder_walk` 3 (the way in), **`chapel_rise` 5**, **`sidings` 5**, **`cooling_yard` 5**, **`slag_mere` 5**, **`bellfield` 6** (the last field before the School).
- **Small places:** `signal`, `wagon`, `hut`, `pipe_end`, `pump`, `shrine`, `cart`, `camp`.
- **Growth:** the largest jars and pages; Electric and Fire reopen earlier regions. Two quests here send her back to the Lowfields with new verbs.
- **Omens carried:** the Headmaster still rings the bell at nine; the train does not always stop; the lamps on the east road go out at ten.
- **Tone ceiling** (`STORY.md` 7.5): the children are never enemies and never shown. Any quest touching the School is about registers, coats on pegs and a bell.

## 2.5 The whole game

Forty-eight side quests: 19 in the Lowfields, 15 in the Waters, 14 in the Works. With the spine (seven rows live in `quests.json`, about five to come, one per remaining dungeon) the log tops out at sixty, inside `PLAN.md` 2.4.

Type: K kill, A acquire, L location. Threat is by day; add 1 at night outside lamplight (2 in the Works).

### The Lowfields (full detail in Part 3)

| # | id | Name | Giver | Where | Type | Threat | Teaches or reveals | Reward |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| A1 | `lost_property` | Lost Property | Lost-property book, Castle Halt | The safe road: well, signpost, cart | A x3 | 0 to 1 | Finding by reading. No markers | Apples, a plain key for the unclaimed shelf |
| A2 | `left_luggage` | Left Luggage | The same book | Mine road and back to the platform | L x2 | 1 to 2 | The width of the Lowfields. What Goldskin shipped | **Page** |
| A3 | `to_be_collected` | To Be Collected | The same book | The platform, after nine | L | 0 (night walk 1) | Night changes a safe place. A choice with a cost | Light stone; the parcel choice (**page**, and a lamp lost) |
| B1 | `rats_in_the_sheds` | Rats in the Sheds | Parish board, Castle | `allotments` | K x6 | 2 | First patch above base | Plain key: the Society's shed (potion makings) |
| B2 | `plot_nine` | Plot 9 | Parish board | `allotments`, after nine | K x1 | 3 (night) | The undead were neighbours | **Jar** |
| C1 | `three_scarecrows` | Three Scarecrows | The farmer's door | Farm gate, Long Hedge, Top Field rim | L x3 | 1, 2, 3 | Where the Top Field is, by day | Apples |
| C2 | `after_the_bell` | After the Bell | The farmer's door | The same three, after nine | L x3 | 2, 3, 4 | The scarecrow omen, without confirming it | Stone Skin potion, nasturtiums |
| C3 | `not_turnips` | Not Turnips | The farmer's door | `top_field` | K x6 | 3 | A patch well above base, with a warning | **Jar** |
| D1 | `the_nurses_round` | The Nurse's Round | Glovebox of the car in the wood | Three cottages near three patches | L x3 | 1 to 2 | The east and south Lowfields. Who is still alive | Small potion |
| D2 | `her_coat` | Her Coat | The glovebox | `behind_the_car` | L x2 | 3 | The second dangerous detour | Her case: Life Steal potion, the parcel |
| D3 | `mrs_allens_dressing` | Mrs Allen's Dressing | The nurse's case | A door in Castle | A x1 | 0 | Back to the hub. Julie was known here | **Jar** |
| E1 | `number_fourteen` | No. 14 | A. Pell's stone, by the last lit lamp | The dead lamps, after nine | L x3 | 2 (night) | Pays off the dog's "Fourteen" | Light stones |
| E2 | `the_lampmans_brazier` | The Lampman's Brazier | The same stone | Past the third dead lamp | A x2 | 1 to 2 | A real trade: wood for a fire or for the mine | **A new fire** in the east Lowfields |
| F1 | `before_the_bell` | Before the Bell | Julie's garden book, the yard | `sallow_bottom` | A x3 | 2 | Herb ground. The rose omen | Rock and water: the rest of Stone Skin |
| F2 | `rose_and_stone` | Rose and Stone | Julie's garden book; handed to the dog | Julie's bench | A x1 | 0 | A second recipe family | Makings of Critical. A line from the dog |
| G1 | `stood_down` | Stood Down | Company notice, the mine mouth | `quarry_steps` | K x5 | 3 | Third detour. The Company's voice | Iron x4, wood x2 (the mine wants both) |
| G2 | `down_at_five` | Down at Five: 4 | Tally slate, the gang's camp | Top of Quarry Steps | L | 3 | A verb gate seen early (the adit, for Explosion) | **Page** |
| H1 | `footpath_three` | Footpath No. 3 | Parish board | The path through `long_hedge` to the farm | L x2 | 2 | The shortcut, and what it costs. Breadcrumb to the farm | Apples; the path |
| H2 | `if_found` | If Found | A haversack in a hollow tree | Long Hedge to Castle Halt | A x1 | 2 | Breadcrumb back west | Makings of Firelash, and how |

### The Waters (outline)

| # | id | Name | Giver | Where | Type | Threat | Teaches or reveals | Reward |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| W1 | `the_toll` | The Toll | Toll board at the tollhouse (keeper absent) | Both bridges | L x2 | 2 | The way into the Waters. The bridge omen | Tollhouse fire lit |
| W2 | `three_crossings` | Three Crossings | The toll board | The bridge, three times in one night | L x3 | 3 (night) | "The bridge counts who crosses" | Page |
| W3 | `the_keepers_tally` | The Keeper's Tally | The toll board | A sunk boat below the bridge | A x1 | 3 | Who stopped crossing, and when | Key to the keeper's box: potion makings |
| W4 | `cutting_order` | Cutting Order | Order slate at the Reedcutters' fire | `eel_beds` | A x3 | 3 | Reed ground; bank fighting | Water caps, moss |
| W5 | `what_the_eels_eat` | What the Eels Eat | The order slate | `eel_beds` | K x6 (marsh rats) | 3 | The cutters will not go back in until it is done | Jar |
| W6 | `the_missing_cutter` | One Hook Empty | Coat hooks at the fire: five hooks, four coats | A jetty at `still_pool` | L | 4 | **Detour.** The lake omen, first sight | Page |
| W7 | `the_planting_book` | The Planting Book | A book in a broken greenhouse | `mothers_garden` rim | A x3 (herbs) | 3 to 4 | What grew here, in whose hand | Seeds: rare herbs placed at Julie's |
| W8 | `dead_heading` | Dead-heading | The planting book: "weekly, without fail" | `mothers_garden` | K x5 (flowers) | 4 | **Detour.** Snakeroot source | Jar |
| W9 | `mother` | Mother | A garden statue's plaque | `mothers_garden`, after nine | L | 5 (night) | The statue faces the other way at night | Page |
| W10 | `overdue` | Overdue | Returns box at the ruined library | Greenhouses and cottages on `glasshouse_row` | A x3 | 3 | Julie's margins in three books | Light stones; the library fire |
| W11 | `the_last_page` | The Last Page | The returns box | The library stacks | L | 3 | Breadcrumb into the Grow spine | Leads to the spine |
| W12 | `road_closed` | Road Closed | Council sign at the head of the lane | `drowned_lane`, end to end | L x2 | 4 | **The road is the danger.** | Stone Skin potions |
| W13 | `the_diversion` | Diversion | The same sign, overleaf | Stones and a hollow tree round the lane | L x3 | 2 to 3 | The safe way round; a fire at the far end | **A new fire** |
| W14 | `nobody_has_drowned` | Nobody Has Drowned | A shrine with a list of names, none crossed out | Three jetties on the lake | A x3 (name tags) | 3 to 4 | The lake omen | Jar |
| W15 | `the_statue_faces` | Which Way It Faces | The shrine | `lake_statue`, noon and after nine | L x2 | 4 | It is not the same way | Page |

### The Works (outline)

| # | id | Name | Giver | Where | Type | Threat | Teaches or reveals | Reward |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| K1 | `shift_rota` | Shift Rota | The rota on the canteen wall | Three watchman's huts | L x3 | 3 to 4 | The shape of the Works; where the fires are | Hut fire on Cinder Walk lit |
| K2 | `dinners_out` | Dinners Out | The canteen hatch (someone behind it) | Three dead signals on `cinder_walk` | A x3 then L x3 | 3 | Somebody still cooks for the night shift | Jar |
| K3 | `relieve_the_night_shift` | Relief | The rota: "night shift to be relieved at six. Not relieved." | `cooling_yard` | K x6 (soldiers) | 5 | **Detour.** Killing as mercy, uneasily | Jar |
| K4 | `signals_at_danger` | Signals at Danger | A dead signal's lamp log | `cinder_walk` | L x3 | 3 | The train omen | Light stones |
| K5 | `the_foremans_diary` | The Foreman's Diary | A slag wagon with a locker | `sidings` | A x3 (pages) | 5 | **Detour.** 2020's "worker diary explaining events" | Page |
| K6 | `last_wagon` | Last Wagon | The diary's final entry | End of the `sidings` | K x1 (the pay clerk) | 5 | What the Works ran on | Key: Factory side door (a shortcut in the spine) |
| K7 | `register_of_burials` | Register of Burials | The lodge book at the graveyard gate | Three graves on `chapel_rise` | L x3 | 5 | Names from the Lowfields quests, with dates | Jar |
| K8 | `flowers_for_the_rise` | Flowers for the Rise | The lodge book | White water roses from Sallow Bottom or Julie's cellar | A x3 | 5 | **Cross-region return.** The rose omen, late | Page |
| K9 | `goldskins_four` | The Four | A carved stone on `chapel_rise` | Four markers round the burial ground | L x4 | 5 | Foreshadows the four corners. Names nobody | Potions for the Burial |
| K10 | `what_the_pipes_say` | What the Pipes Say | A pipe mouth (listen) | Three pipe mouths toward `slag_mere` | L x3 | 4 to 5 | The pipe network as a map | Shortcut hatch unlocked |
| K11 | `the_mere` | The Mere | The last pipe mouth | `slag_mere` | K x6 (cactus) | 5 | **Detour.** | Jar |
| K12 | `the_bell_rope` | The Bell Rope | A shrine at the foot of the hill | `bellfield`, at nine | L | 6 to 8 | The last field. The bell from beneath it | Page |
| K13 | `fourteen_again` | No. 14, Again | The dog, once Electric is known | The Lowfields dead lamps | L x3 | 1 | **Verb return.** The dead run relit for good; pays off Pell | The road safe at night |
| K14 | `the_adit` | The Adit | The tally slate on Quarry Steps, once Explosion is known | Quarry Steps | L | 3 | **Verb return.** What was working behind the rock | Large jar |

---

# Part 3. The Lowfields in full

> **Audited, 23 September 2026.** `QUEST-TREE.md` walks every quest step by step as the player meets it, and `jane/test/quest-audit.test.ts` holds every step to five rules on three seeds (descriptive, doable, findable, easy hand-in, no secrets). Changes since the text below: every quest has a `returnTo` line the tracker and log show once it is ready; step texts name a landmark ("up the farm track", "at the end of her prints"); four tracks were added in `paths.json` (the farm track to the Top Field, the quarry track, the cinder path to the allotments, the nurse's prints) with fingerposts and signs that say each place's name where it is; the dog's mine and burial directions were corrected (the mine is at the end of the mine road, not in the yard; the burial is off the graveyard road, not "the other stair").
>
> **Built, 21 September 2026.** All nineteen quests below are rows, and `jane/test/quests.test.ts` plays every one of them end to end on a generated county (offered, accepted, done, handed in, paid, not payable twice), and checks for every quest in the game that something reachable gives it and hands it in, that every place it names is produced, every creature spawned and every item to be had.
>
> Where the rows are: `jane/src/data/{quests,dialogue,items,units,props,triggers}/lowfields.json`, the placements in `jane/src/data/placements/lowfields.json`, the dog's three rows in `jane/src/data/dialogue.json`, the art in `jane/src/art/props/lowfields.ts` and `jane/src/art/icons/lowfields.ts`.
>
> Where the build differs from the text below, and why:
>
> 1. **`plot_nine` asks for a `location`, not a `kill`.** The Tenant's death fires `location plot_tenant_down`. A kill only counts while the quest is in the log and he does not return, so putting him down before reading the notice would have made the quest impossible for good (checklist 18). The text is unchanged.
> 2. **The dog's two lamp rows sit above `epilogue`**, and so above the three `offer_` rows, not below them. `epilogue` answers unconditionally once the Burial is done; below it the line would never be heard by anyone who finished the spine first. They are still below every `questReady` row, `waiting` and `intro`, above the poke chain, and shown once.
> 3. **`before_the_bell` does not wait for `see_the_kitchen`.** The tree in ASKS C has no such row, and adding one needs a line of text for a book that will not open yet. One row and one line, if wanted.
> 4. **Rects are wider.** `hedge_stile_town` and `hedge_stile_farm` are 27 x 27, not 9 x 9: a rect is centred on its fingerpost, which may stand up to twelve cells from the end of the path, and the rect has to hold the path. `scarecrow_top_wide` is 240 x 220 centred on `scarecrow_top`, not 120 x 90 centred between the two: the twin is an anchor 32 to 70 m off, not exactly 40, and a row cannot centre a rect between two anchors. The test holds both scarecrows more than 24 cells across and 14 down inside it.
> 5. **The cold brazier and the lit one stand side by side**, a few cells apart, not on the same cells. Only a chunk's slot can put two props on one cell.
> 6. **The adit rock stands on the ledge a few cells from the face**, not across a mouth in it: a placed prop needs a clear margin. It wants a slot from `quarrySteps`.
> 7. **Creatures placed in an area stand close round its centre**: seven pumpkins and six quarrymen within about five cells, not in rows over 60 m or in two groups. The rats are split, four on the cinder path and four by the shed. No row sets a phase; the ground's threat does (3, 3 and 2, tested).
> 8. **Two sprites more than asked for**: `campfire_cold` (the campfire's only frame is burning) and `adit_rock` (a prop three cells wide needs a sprite 24 px wide, and no rock is).
> 9. **One mark more**: `nurses_case`, beside the case, so the car can be reached by name.
> 10. **One line written**: after A1's hand-in the book shows `lp_done`, "Printed at the foot of the page: Finders may take one article from the unclaimed shelf. One."
> 11. **Not built**: the three proposed omens (3.2) and whatever sets `omen:scarecrow_closer`. The scarecrow triggers are in, and tested with the flag set by hand. Nobody has counted the creatures between the car and the coat.

Nineteen quests in eight chains. Conventions:

- Quest rows are given as they would sit in `quests.json`. `grow` is the proposed growth verb (ASKS F1).
- Dialogue is given per tree in ASKS section C as a `start` order, and per quest here as node text. Option labels are in **bold**. Actions are in `code`.
- "Hand-in home" names the tree that holds the `questReady` row. Every quest has one (checklist 17).
- Threat is the ground's threat by day / by night. Hubs are 0.
- All text follows `VOICE.md`. `{name}` appears only where someone who knows her name addresses her: the dog, and Julie's handwriting.

---

## Chain A. Lost Property (Castle Halt)

The opening chain and the closing one. The giver is a book on a table under the platform canopy. The office is not staffed. It is still open.

### A1. `lost_property`: Lost Property

| | |
| --- | --- |
| Chain | A, 1 of 3 |
| Giver | The lost-property book: prop `table`, key `lost_property_book`, tree `lost_property_book`, on the station platform |
| Given at / plays at | `station` / the safe road: anchors `halt_well`, `halt_signpost` (both station to Julie's), `halt_cart` (Julie's to Castle) |
| Prerequisite | None. Offered from the first minute |
| Danger | 0 to 1 by day; the road is lamp-lit, so 1 by night. Nothing lethal is allowed on this road (`PLAN.md` 2.3) |

```json
"lost_property": {
  "name": "Lost Property",
  "description": "The lost-property book at Castle Halt lists three things lost on the road to Castle and never found: a red glove by the well, a hat left on a signpost, a dinner tin under an overturned cart. There is nobody to hand them to. The book asks anyway.",
  "completion": "You enter all three. Beside each line a date of return was already pencilled in. All three dates are next Sunday.",
  "requirements": [
    { "type": "acquire", "target": "lost_glove", "qty": 1, "text": "A red glove, by the well" },
    { "type": "acquire", "target": "lost_hat", "qty": 1, "text": "A hat, on a signpost" },
    { "type": "acquire", "target": "lost_tin", "qty": 1, "text": "A dinner tin, under a cart" }
  ],
  "rewards": [{ "do": "give", "item": "apple", "qty": 2 }, { "do": "give", "item": "key_generic", "qty": 1 }]
}
```

**Text**

- `lp_offer`: "CASTLE HALT. LOST PROPERTY. The office is not staffed. It is still open." / "Reported lost on the road to Castle, not yet found: one child's glove, red, by the well. One man's hat, left on a signpost 'for a moment'. One carter's dinner tin, under the seat of a cart that did not arrive." / "Finders please enter items below. A pencil is provided." Options: **I'll look** `quest lost_property` · **Not now**
- `lp_wait`: "Glove, well. Hat, signpost. Tin, cart. The pencil is on its string."
- `lp_in`: "Three lines to fill in. The column headed CLAIMED BY is wider than the others." Options: **Write them in** `handin lost_property`, `take lost_glove`, `take lost_hat`, `take lost_tin` · **Not yet**
- After hand-in the next read shows `ll_offer` (A2). A printed line at the foot of the page: "Finders may take one article from the unclaimed shelf. One."
- The unclaimed shelf: prop `chest`, key `unclaimed_shelf`, locked, `keyTag: "generic"`, beside the book. Loot: `light_stone` 1, `small_water` 2, `grape` 4.
- The three pick-ups are `lost_thing` props with loot. Item descriptions are in ASKS section A.

**Omen:** none. No omen may be true on this road.
**Co-op:** `acquire` counts across the party's bags; whoever writes them in has only her own taken. Everyone is paid the apples and a key; the shelf is first come, which is what "One." is for.
**Why it is fun:** she took the quest at the door of the game and finds it two-thirds done by the things she passed on her first walk (K15), and it teaches that in this county the text is the map (K4).

### A2. `left_luggage`: Left Luggage

| | |
| --- | --- |
| Chain | A, 2 of 3. **The cross-map quest** |
| Giver | The same book |
| Given at / plays at | `station` / anchor `carters_cart` on the Castle to Gold Mine road, within sight of the mine; then back to the platform |
| Prerequisite | `lost_property` done |
| Danger | Mine road 1, rising to 2 inside the mine's ring. The walk back is the width of the Lowfields and will usually end after the bell: 2 on the unlit stretch |

```json
"left_luggage": {
  "name": "Left Luggage",
  "description": "A tin-lined trunk from the Goldskin Mining Co. has stood on the platform since it missed its train. The key went with the carter, last seen on the mine road walking beside a full cart. The book does not say why he was not riding.",
  "completion": "Straw, and under it a gilder's book: tissue leaves with gold between them. Every leaf has been used but one. The consignment note gives the trunk's weight full, and it is the weight of the trunk empty.",
  "requirements": [
    { "type": "location", "target": "carters_cart", "qty": 1, "text": "Find the carter's cart on the mine road" },
    { "type": "location", "target": "trunk_open", "qty": 1, "text": "Open the trunk at Castle Halt" }
  ],
  "rewards": [{ "do": "grow", "stat": "spirit", "amount": 3, "id": "page_left_luggage" }, { "do": "give", "item": "gold_dust", "qty": 2 }]
}
```

**Text**

- `ll_offer`: "LEFT LUGGAGE. One trunk, tin-lined, heavy. Consignor: Goldskin Mining Co. To go by the Sunday train." / "Held for want of the key, which the carter has. The carter was last seen on the mine road with a full cart, walking beside it." Options: **I'll find him** `quest left_luggage` · **Not now**
- `ll_wait`: "The trunk has not moved. There is a ring of clean platform round it where nobody has stood."
- At `carters_cart`: a `lost_thing` under the cart, key `carters_key_drop`, loot `key_left_luggage` 1, `use`: `location carters_cart`. Beside it a `note`, tree `carters_note`: "Horse would not go past the quarry turning. Gone up on foot to see what it is looking at. Back directly."
- The trunk: prop `boss_chest`, key `left_luggage_trunk`, locked, `keyTag: "left_luggage"`, label "The trunk". Loot: `small_water` 1. `use`: `location trunk_open`, `handin left_luggage`.
- `ll_in` (fallback, if the trunk was opened before the quest was taken): "The trunk is open. Somebody ought to write that down." Options: **Write it down** `handin left_luggage` · **Not yet**

**Omen:** none of its own. The note's "the quarry turning" points at chain G.
**Co-op:** locations are the world's; one key, one trunk, growth is everyone's. The party should walk it together: at night on the unlit stretch a lone player in a party of four is at 38 percent.
**Why it is fun:** the locked trunk was on the platform on the first evening (K16), the key is at the far end of the region (K6), and she comes back to where the game began with gold leaf on her hands, stronger, probably in the dark.

### A3. `to_be_collected`: To Be Collected

| | |
| --- | --- |
| Chain | A, 3 of 3. **Night only to complete** |
| Giver | The same book, a later page, in pencil, in a different hand |
| Plays at | Rect `platform` at `station`, between nine and six |
| Prerequisite | `left_luggage` done |
| Danger | The platform is a hub (0). Getting there after nine is the lit Halt road: 1 |

```json
"to_be_collected": {
  "name": "To Be Collected",
  "description": "A pencilled entry says a parcel is left on the platform crate each evening after the bell, and that it is not lost property and is always collected. It does not say who leaves it. It asks that someone check it is still being collected.",
  "completion": "There is a parcel on the crate. Brown paper, string, no label. It is warm. You were watching the platform and you did not see it put there.",
  "requirements": [{ "type": "location", "target": "platform_after_nine", "qty": 1, "text": "Stand on the platform after the nine o'clock bell" }],
  "rewards": [{ "do": "give", "item": "light_stone", "qty": 1 }]
}
```

**Text**

- `tbc_offer`: "In pencil, a different hand: Parcel, brown paper, string. Left on the platform crate each evening after the bell. NOT lost property. Do not hand in. It is collected." / "Underneath: would somebody check that it still is." Options: **I'll check** `quest to_be_collected` · **Not now**
- `tbc_wait`: "After the bell, the entry says. The crate is at the north end of the platform."
- `tbc_in`: "There is a column for the checker's initials. Yours would be the first in it." Options: **Initial it** `handin to_be_collected` · **Not yet**
- Triggers (ASKS D): standing on `platform` at night runs `location platform_after_nine`. The parcel is shown when she comes within a screen and a half of the station at night (unless `took_parcel`) and hidden when she comes that near by day. She never sees it arrive or go, which is the dog's rule applied to a parcel.
- The parcel: prop `lost_thing`, key `night_parcel`, hidden at build. No loot. `use`: `grow spirit 3 id page_night_parcel`, `flag took_parcel`, `hide night_parcel`, `hide station_lamp`, `show station_lamp_dead`, `toast "You take it. Nobody stops you. Behind you the platform lamp goes out."` (The lamp going out in front of her is the one permitted exception to "nothing is seen to change": she did it.)
- `closed` (book, afterwards, parcel left): "Parcel: collected. Parcel: collected. Parcel: collected. The entries go back further than the book does."
- `closed_taken` (book, afterwards, parcel taken): "Parcel: NOT collected. Enquiries are being made."

**The decision (K21):** leave it, and nothing happens, ever. Take it, and she gains a gold-leaf page, the platform lamp never lights again, and the book says enquiries are being made. No text says by whom. `took_parcel` is a flag the Works may read (`signals_at_danger`).
**Omen tie:** "The train does not always stop." This quest does not confirm or deny it.
**Co-op:** the trigger fires for whoever steps on. One parcel; growth is the world's; the lamp is everybody's loss, so the party should agree before one of them takes it.
**Why it is fun:** the safest square in the game is a different place after the bell, the task is exact and the meaning is withheld (K3), and the reward is a temptation with a visible, permanent price.

---

## Chain B. The Allotments (Castle)

Giver for both: the parish board in the square (prop `notice_board`, key `parish_board`, tree `parish_board`). One notice shows at a time (K2).

### B1. `rats_in_the_sheds`: Rats in the Sheds

| | |
| --- | --- |
| Chain | B, 1 of 2. **Pure kill, with a reason** |
| Plays at | Area `allotments` |
| Prerequisite | None (shown after `footpath_three` has been taken or refused once; see the tree order) |
| Danger | 2 by day. The town's fire is a short run away |

```json
"rats_in_the_sheds": {
  "name": "Rats in the Sheds",
  "description": "The Castle Allotment Society has rats in its sheds and asks members to deal with any they see. Six would be a start. The notice adds that they did not use to be this size, and does not say since when.",
  "completion": "Six. The sheds are quiet. Nothing in them had been eaten; the seed drawers were all open and all full.",
  "requirements": [{ "type": "kill", "target": "rat", "qty": 6, "text": "Rats in the allotments" }],
  "rewards": [{ "do": "give", "item": "key_generic", "qty": 1 }, { "do": "give", "item": "pansy", "qty": 2 }]
}
```

**Text**

- `rats_offer`: "CASTLE ALLOTMENT SOCIETY. Rats in the sheds again. Members are asked to deal with any they see. Six would be a start." / "The Society notes that they did not use to be this size." Options: **Take it down** `quest rats_in_the_sheds` · **Leave it**
- `rats_wait`: "Six would be a start, it says. The allotments are out past the last house."
- `rats_in`: "A second sheet behind the first: the shed key is on the nail behind this board. Members only. Anyone who has dealt with six is a member." Options: **Take the key** `handin rats_in_the_sheds` · **Not yet**
- The Society's shed: prop `chest`, key `allotment_shed`, in `allotments`, locked `generic`. Loot: `small_water` 3, `nasturtium` 2, `honeylace_lily` 2, `small_empty_vial` 2.

**Note for the builder:** `rat` is a shared row, so rats killed anywhere count. That is acceptable here (the Society does not care where), and the placement ask puts eight in the allotments so that is where it will be done.
**Omen:** none.
**Co-op:** kills count for the party. Everyone gets a key; only one shed. Spare plain keys are never wasted in this game.
**Why it is fun:** it is the first step off the base threat, five minutes from a fire, for a reward that is a locked door she has already walked past and the makings of two potions (K10).

### B2. `plot_nine`: Plot 9

| | |
| --- | --- |
| Chain | B, 2 of 2. **Night only. Named kill** |
| Plays at | Rect `plot_nine` inside `allotments`, between nine and six |
| Prerequisite | `rats_in_the_sheds` done |
| Danger | 3 (2, plus night). One enemy, a guard-class skeleton at phase 2. The town's lamps are in sight |

```json
"plot_nine": {
  "name": "Plot 9",
  "description": "Plot 9 of the Castle allotments is not let, and is freshly dug every morning. The Society would like someone to have a word with whoever it is. After nine, presumably. It asks that the word be a quiet one.",
  "completion": "He had a good coat and his own spade. He was not digging anything up. A hand's depth down in Plot 9 there is a jar with Julie's label, buried lid upward, carefully.",
  "requirements": [{ "type": "kill", "target": "plot_tenant", "qty": 1, "text": "Whoever digs Plot 9" }],
  "rewards": [{ "do": "grow", "stat": "strength", "amount": 3, "id": "jar_plot_nine" }]
}
```

**Text**

- `plot_offer`: "CASTLE ALLOTMENT SOCIETY. Plot 9 is not let. Members are reminded not to dig it." / "Would whoever digs it each night please stop, or else apply for it in the usual way." Options: **Take it down** `quest plot_nine` · **Leave it**
- `plot_wait`: "Plot 9 is the one with no name on the stake. By day there is nobody there. The earth is soft."
- `plot_in`: "Someone has written under the notice, small: thank you. he was secretary for eleven years." Options: **Leave it up** `handin plot_nine` · **Not yet**
- A `sign` at the plot, tree `plot_nine_stake`, day: "PLOT 9. No name on the stake. The soil has been turned today, a spade's depth, in rows." Night: "PLOT 9. Somebody is working it."
- Unit `plot_tenant`, "The Tenant of Plot 9", `nightOnly` (ASKS F2): there after the bell, never seen arriving.

**Omen (new, proposed):** `plot_nine_not_let`. If true, the plot is dug again the morning after he is put down (the tile fill returns). If false, it stays as she left it. No text comments.
**Co-op:** one target; the kill and the jar are everyone's.
**Why it is fun:** a small administrative notice turns out to be a man, and the kill the board asked for politely is the first one she may wish she had not made (`STORY.md` 7.3); the jar says he was looking after something of Julie's.

---

## Chain C. Lowfield Farm

Giver for all three: the farmhouse door (prop `door_talk`, key `farm_door`, tree `farmer`, speaker "The Farmer"). It does not open. The three scarecrows are anchors `scarecrow_gate` (within sight of the farm gate), `scarecrow_hedge` (inside `long_hedge`) and `scarecrow_top` (on the rim of `top_field`, on the side nearest the farm).

### C1. `three_scarecrows`: Three Scarecrows

| | |
| --- | --- |
| Chain | C, 1 of 3 |
| Prerequisite | None |
| Danger | 1 at the gate, 2 on the Long Hedge, 3 at the rim of the Top Field. By day. She is told to stay on the rim |

```json
"three_scarecrows": {
  "name": "Three Scarecrows",
  "description": "The farmer wants his three scarecrows counted while it is light: by the gate, on the Long Hedge, and at the rim of the Top Field. He would not open the door, and he would not say what number he is hoping for.",
  "completion": "Three, by daylight. He thanked you through the door. There was a bag of apples on the step that had not been there when you knocked.",
  "requirements": [
    { "type": "location", "target": "scarecrow_gate_day", "qty": 1, "text": "The scarecrow by the gate" },
    { "type": "location", "target": "scarecrow_hedge_day", "qty": 1, "text": "The scarecrow on the Long Hedge" },
    { "type": "location", "target": "scarecrow_top_day", "qty": 1, "text": "The scarecrow at the Top Field" }
  ],
  "rewards": [{ "do": "give", "item": "apple", "qty": 4 }]
}
```

**Text**

- `farmer_first`: "Who is that. No, I can hear you are not from here. You will be Julie's girl." / "I would come out, only I am in the middle of something." / "There are three scarecrows on this farm. Gate, Long Hedge, top field. Walk round and count them for me while it is light. I know it is three. I would like to hear it from someone else." Options: **I'll count them** `quest three_scarecrows` · **Not today**
- `farmer_wait_1`: "Gate, hedge, top field. Stay on the rim of the top field, mind. You can see it from the fence."
- `farmer_in_1`: "Three. Good. That is the right number for the daytime." Options: **Three** `handin three_scarecrows` · **Not yet**
- Scarecrow trees, by day (each runs its `location` on leaving the node): gate, "A scarecrow. One. Straw, a sack, a coat somebody wore for years. It faces the road." Hedge, "Two. The same straw, a better hat. It faces the farm." Top field, "Three. It stands where the field ends. Behind it the rows are full of something round and orange, and the farmer said turnips."

**Omen tie:** "The scarecrow in the top field was not there yesterday." By day nothing is wrong.
**Co-op:** locations are the world's.
**Why it is fun:** `VOICE.md`'s own example quest, made real: an exact errand with a hole where the motive should be (K3, K17), and it walks her to the rim of the most dangerous patch in the region by daylight so that she knows where it is before anyone sends her in.

### C2. `after_the_bell`: After the Bell

| | |
| --- | --- |
| Chain | C, 2 of 3. **Night only to complete** |
| Prerequisite | `three_scarecrows` done |
| Danger | 2, 3 and **4** (the rim, at night). Nothing need be fought: the places are touched, not held. The farm's fire is the run home. The warning is in the offer |

```json
"after_the_bell": {
  "name": "After the Bell",
  "description": "The farmer wants the same three scarecrows counted again after nine. He says to touch the rim of the Top Field and come straight back, and that he would understand if you would rather not. He said he would rather not.",
  "completion": "Three, after dark as well. He was quiet for a while behind the door. Then he said you would have told him if it were four.",
  "requirements": [
    { "type": "location", "target": "scarecrow_gate_night", "qty": 1, "text": "By the gate, after the bell" },
    { "type": "location", "target": "scarecrow_hedge_night", "qty": 1, "text": "On the Long Hedge, after the bell" },
    { "type": "location", "target": "scarecrow_top_night", "qty": 1, "text": "At the Top Field, after the bell" }
  ],
  "rewards": [{ "do": "give", "item": "potion_stoneskin", "qty": 1 }, { "do": "give", "item": "nasturtium", "qty": 2 }]
}
```

**Text**

- `farmer_second`: "Now I need them counted after the bell. Same three." / "Touch the rim of the top field and come straight back. Do not go in among the rows. What is growing in there is not what I planted, and it is worse at night." / "If you would rather not, I understand. I would rather not." Options: **After the bell, then** `quest after_the_bell` · **No**
- `farmer_wait_2`: "After the bell. I will be up. I am always up."
- `farmer_in_2`: "Three after dark as well. Thank you." / "You would tell me if it were four." Options: **It was three** `handin after_the_bell` · **Not yet**
- Scarecrow trees, by night: gate, "One. Somebody has done up the top button of its coat." Hedge, "Two. It faces the farm." Top field, "Three. Straw, an old coat, a turnip for a head. The rows behind it are not still."
- By night and with no quest, the same lines show: the texts are true on every seed.

**Omen tie, and how it stays unconfirmed:** if `omen:scarecrow_closer` is true for the seed, the prop at `scarecrow_top` is hidden after the bell and its twin `scarecrow_top_near`, forty metres nearer the farm on the line to the farm gate, is shown; by day they swap back. The night `location` fires from whichever is standing. The text is identical either way. A player who noticed where it stood by day will know. The log will not.
**Co-op:** touch-and-leave suits a party that stays together. Nobody needs to split to cover three places; the count is world-wide and order is free.
**Why it is fun:** the same walk as C1 with the county's one rule applied to it (K11: a repeated type must change the conditions), a rim at threat 4 that she only has to touch, and a payoff that depends on whether she was paying attention yesterday.

### C3. `not_turnips`: Not Turnips

| | |
| --- | --- |
| Chain | C, 3 of 3. **Pure kill, with a reason. The patch well above base, clearly warned** |
| Plays at | Area `top_field` |
| Prerequisite | `after_the_bell` done |
| Danger | **3** by day against a region base of 1; 4 by night, and she is told not to. Phase-3 pumpkins have nearly three times the health of the yard's. The farm's fire is the run back |

```json
"not_turnips": {
  "name": "Not Turnips",
  "description": "The farmer planted turnips in the Top Field. He counts six of something else from the upstairs window, nearer the yard than they were on Sunday. He says it is past anything he would ask of a stranger, and that there is nobody else to ask.",
  "completion": "Six. Cut, they are only pumpkins, hollow, with nothing in them. There is no seed in any of the six.",
  "requirements": [{ "type": "kill", "target": "pumpkin_top", "qty": 6, "text": "The things in the Top Field" }],
  "rewards": [{ "do": "grow", "stat": "strength", "amount": 3, "id": "jar_farm" }, { "do": "give", "item": "apple", "qty": 3 }]
}
```

**Text**

- `farmer_third`: "Those things in the top field. I count six from the upstairs window, and they are nearer the yard than they were on Sunday." / "It is past anything I would ask of a stranger. I am asking because there is nobody else, and I will not pretend that is the same as it being safe." / "By day. Not after the bell. If it goes badly, the fire in my yard is yours." Options: **I'll go up** `quest not_turnips` · **Not yet**
- `farmer_wait_3`: "I can still see them from the window. They do not move while I watch. I cannot watch all the time."
- `farmer_in_3`: "I counted none this morning. First time since the spring." / "There is a jar on the step. The last of what my wife put up, the year Julie showed her how. Eat it all. It does not keep once it is open." Options: **Thank you** `handin not_turnips` · **Not yet**
- `farmer_idle` (after the chain): "Still in the middle of something. You know where the fire is."

**Omen tie:** the scarecrow again. If the omen is true, the near twin stands inside the aggro range of the first pumpkin at night.
**Co-op:** kills are shared. A party of two deals 124 percent together, so this is a good one to bring a friend to, and a bad one to wander off from.
**Why it is fun:** she has been shown the field twice and warned three times by a man who is afraid for her (K7, K19), so going in is her choice, and the jar is the first time the county feeds her.

---

## Chain D. The Nurse's Round (the car in the wood)

The car is 2020's. Its owner is a district nurse who is not there. Giver: the car itself. `car_wreck` keeps its loot (two planks, water, a fire stone); once that is taken, USE reads the glovebox (tree `car_glovebox`).

### D1. `the_nurses_round`: The Nurse's Round

| | |
| --- | --- |
| Chain | D, 1 of 3. **Giver absent** |
| Given at / plays at | `car_wood` / anchors `cottage_allen` (nearest `sallow_bottom`), `cottage_pike` (nearest `long_hedge`), `cottage_crane` (nearest `quarry_steps`) |
| Prerequisite | None |
| Danger | The cottages are roadside: 1 to 2. The car itself is off the road in a wood: 1 to 2. A long walk: three corners of the region |

```json
"the_nurses_round": {
  "name": "The Nurse's Round",
  "description": "A district nurse left her car in the wood with three visits still unticked: Mrs Allen at Sallow Bottom, Mr Pike by the Long Hedge, the Misses Crane under Quarry Steps. The list says Tuesday. It does not say which.",
  "completion": "Three calls and nobody seen. One gone to town, one who will not answer after nine, one step with eleven parcels on it. You tick all three. It seems the thing to do.",
  "requirements": [
    { "type": "location", "target": "cottage_allen", "qty": 1, "text": "Mrs Allen, Sallow Bottom" },
    { "type": "location", "target": "cottage_pike", "qty": 1, "text": "Mr Pike, by the Long Hedge" },
    { "type": "location", "target": "cottage_crane", "qty": 1, "text": "The Misses Crane, under Quarry Steps" }
  ],
  "rewards": [{ "do": "give", "item": "potion_manashield", "qty": 1 }]
}
```

**Text**

- `glovebox_first`: "A road atlas of somewhere else. A district nurse's visiting list on a clipboard. The car keys are not here." / "Tues. Mrs Allen, Sallow Bottom: dressing. Mr Pike, by the Long Hedge: chest. The Misses Crane, under Quarry Steps: leave it on the step, do not go in." / "None of the three is ticked." Options: **Take the list** `quest the_nurses_round` · **Leave it**
- `glovebox_wait_1`: "Allen, Pike, Crane. Her pencil is still clipped to the board."
- On each cottage step, a `note` whose tree runs the `location`:
  - `note_allen`: "Nurse: gone to my sister's in Castle till it passes. Hers is the door facing the fire in the square. Thank you for coming out all this way. E. Allen."
  - `note_pike`: "NURSE. Knock twice and wait. If it is after nine do not knock. I will not know it is you. PIKE."
  - `note_crane`: "Eleven parcels on the step in the nurse's wrapping, none opened. The oldest has moss on it. A card has been pushed out under the door: NEXT TIME PLEASE BRING YOUR BAG."
- `glovebox_in_1`: "Three ticks. Under the clipboard there was a second sheet all along, folded small." Options: **Unfold it** `handin the_nurses_round` · **Not yet**

**Omen (new, proposed):** `pike_after_nine`. If true, knocking at Mr Pike's after the bell is answered. If false, nothing answers at any hour. Either way the note is just a note.
**Co-op:** locations are shared; the party can walk the round together in any order.
**Why it is fun:** it is the Lowfields' Stalvan (K5): three fragments on three steps, each an ordinary note with one hole in it, and it tours her past all three dangerous patches without sending her into any of them.

### D2. `her_coat`: Her Coat

| | |
| --- | --- |
| Chain | D, 2 of 3. **Giver absent. Dangerous detour, clearly warned** |
| Plays at | Area `behind_the_car`: anchor `nurses_coat` (a hollow tree, at least 80 m past the car); then back at the car |
| Prerequisite | `the_nurses_round` done |
| Danger | **3** by day, in a wood: spiders and the walking dead, short sight lines. No fire nearer than Julie's or the farm. The nurse herself wrote the warning |

```json
"her_coat": {
  "name": "Her Coat",
  "description": "The nurse's case is on the back seat of the car, locked. Her coat is not in the car. A woman's prints lead off behind it into the wood, and they are not hurried. Her second note says not to come and look, and that she means it kindly.",
  "completion": "The case opens. Dressings, a thermometer, a parcel made up for Mrs Allen. Everything is in its place. Nothing in it has been used.",
  "requirements": [
    { "type": "location", "target": "nurses_coat", "qty": 1, "text": "Find her coat, behind the car" },
    { "type": "location", "target": "nurses_case", "qty": 1, "text": "Open her case" }
  ],
  "rewards": [{ "do": "give", "item": "potion_lifesteal", "qty": 1 }]
}
```

**Text**

- `glovebox_second` (shown on handing in D1): "In her hand: If I am not back by dark, do not come and look. I mean it kindly. The wood behind the car is not the wood in front of it." / "Her case is on the back seat, locked. Her coat is gone. There are prints behind the car, a woman's shoe, going in. They are not hurried. None come back." Options: **Follow them** `quest her_coat` · **Do as she says**
- `glovebox_wait_2`: "Behind the car, she wrote. Not in front."
- At `nurses_coat`: a `lost_thing`, key `nurses_coat_drop`, loot `key_nurses_case` 1, `use`: `location nurses_coat`, `toast "A nurse's coat on a branch at head height, buttoned to the neck. The key is in the pocket. The lining is dry."`
- The case: prop `chest`, key `nurses_case`, beside the car, locked, `keyTag: "nurses_case"`, label "The nurse's case". Loot: `nurses_parcel` 1. `use`: `location nurses_case`, `handin her_coat`, `quest mrs_allens_dressing`.
- `glovebox_in_2` (fallback): "The case is open on the seat." Options: **Note it on the list** `handin her_coat` · **Not yet**

**Omen:** none needed. The second note is a plain statement that happens to be a warning.
**Co-op:** one key, one case; the potion goes to everyone. A wood at threat 3 is where a party should stay within sight of each other.
**Why it is fun:** the warning comes from the person who ignored it (K19, K22), what she finds is small and exact (a buttoned coat, a dry lining) and explains nothing, and the fight in and out is the hardest the region offers off a road.

### D3. `mrs_allens_dressing`: Mrs Allen's Dressing

| | |
| --- | --- |
| Chain | D, 3 of 3. Breadcrumb back to the hub |
| Given at / plays at | The nurse's case / prop `door_talk`, key `door_allen`, on the house facing the square's fire in `town`. Tree `mrs_allen` |
| Prerequisite | `her_coat` done (the case gives it) |
| Danger | None at the door. The walk from the wood to Castle |

```json
"mrs_allens_dressing": {
  "name": "Mrs Allen's Dressing",
  "description": "There is a parcel in the nurse's case made up for Mrs Allen, who has gone to her sister's in Castle till it passes. Her note said the door facing the fire in the square. It did not say what it is that will pass.",
  "completion": "She would not open the door. The parcel went in through the flap and a jar came out the same way, with Julie's writing on the lid.",
  "requirements": [{ "type": "acquire", "target": "nurses_parcel", "qty": 1, "text": "The parcel for Mrs Allen" }],
  "rewards": [{ "do": "grow", "stat": "strength", "amount": 3, "id": "jar_mrs_allen" }]
}
```

**Text**

- `allen_in`: "Is that the nurse? No. You do not walk like the nurse." / "Put it through the flap, would you. I am not opening this door until it passes, and my sister agrees with me. Or she did." / "Here. Julie brought me these the last winter she was about. I have not the stomach for it now. You look as if you have been walking." Options: **Pass it through** `handin mrs_allens_dressing`, `take nurses_parcel` · **Not yet**
- `allen_before` (no parcel, quest not done): "We are not seeing anyone, thank you. Lovely evening. You will want to be in before the lamps."
- `allen_after`: "The dressing is a great comfort. Nurse will be along on Tuesday, I expect. She always is."

**Omen:** none.
**Co-op:** the parcel may be in any bag. If a friend carries it and someone else does the talking, it is left in her bag as a parcel nobody is waiting for; harmless (ASKS F4).
**Why it is fun:** after the wood, a door in the safest street in the county, a living voice, and a jar; it closes the loop the first note opened and leaves "Or she did" behind it.

---

## Chain E. The Lampman (the farm road)

Giver for both: A. Pell's stone, a `shrine` anchor `pell_shrine` set beside the last lit lamp of the Lowfields road with the longest dark remainder. Tree `pell_stone`. Beyond it stand the county's only three dead lamps, `lamp_12`, `lamp_13`, `lamp_15`, a lamp's distance apart; past them, `pell_brazier`. **This chain pays off the dog's "Fourteen. Do not count them yourself. It is a different number if you count."**

### E1. `number_fourteen`: No. 14

| | |
| --- | --- |
| Chain | E, 1 of 2. **Giver dead. Night only to complete. Pays off the dog** |
| Prerequisite | None. (The dog's line sits at the fifth poke; the quest works without it and lands harder with it) |
| Danger | An unlit road at night: 2. Short: three lamps and back to the last lit one |

```json
"number_fourteen": {
  "name": "No. 14",
  "description": "A stone by the last lit lamp remembers A. Pell, lampman, who went out from it to see to No. 14. The dead lamps beyond have numbers on them. By day there is no telling a dead lamp from a live one, so it will have to be after the bell.",
  "completion": "You walked the dead run and came back to the light. Whatever you told the stone, it has been told before.",
  "requirements": [
    { "type": "location", "target": "lamp_12", "qty": 1, "text": "The first dead lamp" },
    { "type": "location", "target": "lamp_13", "qty": 1, "text": "The second dead lamp" },
    { "type": "location", "target": "lamp_15", "qty": 1, "text": "The third dead lamp" }
  ],
  "rewards": [{ "do": "give", "item": "light_stone", "qty": 2 }]
}
```

**Text**

- `pell_first`: "A. PELL. LAMPMAN TO THIS PARISH 31 YEARS. WENT OUT FROM THIS LAMP TO SEE TO No. 14." / "Underneath, scratched later: they are numbered from the square. count them if you like." Options: **Count them** `quest number_fourteen` · **Walk on**
- `pell_wait`: "WENT OUT FROM THIS LAMP TO SEE TO No. 14. The road beyond it has posts and no light."
- Dead lamp trees. By day (no `location`): "A lamp post. No. 12 on the plate. By day there is no telling whether it works." Likewise 13 and 15.
- By night (each runs its `location`): `lamp_12`, "No. 12. Dark. The glass is whole and the mantle is new." `lamp_13`, "No. 13. Dark. There are fresh ladder marks on the post." `lamp_15`, "No. 15. Dark. It stands one lamp's distance from No. 13. There is no gap where a lamp could have been."
- `pell_in`: "Twelve, thirteen, fifteen. The stone says he went to see to No. 14." Options: **There is no No. 14** `handin number_fourteen` · **Fourteen, then** `handin number_fourteen`, `flag said_fourteen`
- The dog, next morning (`dog.lamps_counted`; placed below every `questReady` row and above the poke rows; shown once, sets `dog_heard_lamps`): "You went and counted. I did ask." / "Twelve, thirteen, fifteen. Yes. Fourteen was there the night Pell went up the road to it with his ladder. I watched him go. I did not watch him come back."
- If `said_fourteen`, the dog adds (`dog.lamps_fourteen`): "You told the stone fourteen. That was kind or it was careless. Out here they come to the same thing, {name}."

**Omen tie:** "The lamps on the east road go out at ten." Neighbouring claim, not confirmed. The plates are true on every seed: what is missing is a number, and a lamp's width of road that is not there.
**Co-op:** locations are shared. The dog speaks to whoever talks to it first.
**Why it is fun:** a gap of exactly one (K17) that she can check with her feet; a reason inside the fiction for the night-only rule; and the dog's strangest line, earned five pokes deep, turns out to have been directions.

### E2. `the_lampmans_brazier`: The Lampman's Brazier

| | |
| --- | --- |
| Chain | E, 2 of 2. **Reward: a fire lit somewhere new** |
| Plays at | `pell_brazier`, past `lamp_15` |
| Prerequisite | `number_fourteen` done |
| Danger | 1 by day, 2 by night. The difficulty is the materials, not the place |

```json
"the_lampmans_brazier": {
  "name": "The Lampman's Brazier",
  "description": "A parish notice under Pell's stone says the lampman's brazier stands past No. 15 and is to be kept laid: two planks and a fire stone. Any passer-by may light it and the parish will be obliged. Nobody has been obliged in some time.",
  "completion": "It catches at once, as if it had been waiting. From here you can see the last lit lamp behind you and nothing ahead. That is more than you could see before.",
  "requirements": [
    { "type": "acquire", "target": "wood", "qty": 2, "text": "Wood" },
    { "type": "acquire", "target": "fire_stone", "qty": 1, "text": "A fire stone" }
  ],
  "rewards": [{ "do": "hide", "prop": "pell_brazier_cold" }, { "do": "show", "prop": "pell_brazier" }]
}
```

**Text**

- `pell_second`: "A parish notice under the stone, newer: THE LAMPMAN'S BRAZIER stands past No. 15 and is to be kept laid. Two planks and a fire stone. Any passer-by may light it, and the parish will be obliged." Options: **I'll see to it** `quest the_lampmans_brazier` · **Walk on**
- `pell_after`: "A. PELL. LAMPMAN TO THIS PARISH 31 YEARS. Somebody has put a fire stone chip on top of the stone, the way you would a pebble."
- The cold brazier: prop `campfire_cold`, key `pell_brazier_cold`, tree `brazier_cold`. `brazier_empty`: "An iron basket on legs, swept out, with a dry place under it for planks. It has been kept ready by someone with nothing to put in it." `brazier_ready` (when `questReady`): "Two planks, one fire stone." Options: **Lay it and light it** `handin the_lampmans_brazier`, `take wood 2`, `take fire_stone 1` · **Not yet**
- `pell_brazier`: an ordinary `campfire` (tree `fire`), hidden at build, shown by the reward. It rests and saves like any other.

**The decision (K21):** the car in the wood holds exactly two planks and one fire stone. The mine's broken stairs want two planks (the dog says so). Until G1 pays out more wood, she chooses: a fire in the east Lowfields now, or the mine's stair now.
**Omen:** none.
**Co-op:** the materials may be spread across bags for `questReady`, but `take` draws from the one who lights it, so she should be carrying them (ASKS F4).
**Why it is fun:** the reward is the most valuable thing this game can give, a shorter run (K10, K19), she pays for it out of the same pocket the dungeon wants to pick, and afterwards it can be seen burning from the road (K9).

---

## Chain F. The Garden Book (Julie's yard)

Giver: Julie's garden book, a `note` prop, key `garden_book`, on the barrel by the stoop in the yard. Tree `garden_book`, speaker "Julie's Garden Book". Julie is always practical and always one fact short (`VOICE.md` rule 11).

### F1. `before_the_bell`: Before the Bell

| | |
| --- | --- |
| Chain | F, 1 of 2. **Giver absent** |
| Plays at | Area `sallow_bottom`: five `rose` props below anchor `sallow_jetty` |
| Prerequisite | `see_the_kitchen` done |
| Danger | 2 by day. 3 after the bell, and the book says not to |

```json
"before_the_bell": {
  "name": "Before the Bell",
  "description": "Julie's garden book says white water roses do better wild than in the cellar: at Sallow Bottom, below the jetty. Three is plenty. It says to pick them before the bell, and that she never learned what happens after.",
  "completion": "Three, picked in daylight. They are cold in the hand, like something from a cellar, though they grew in the sun.",
  "requirements": [{ "type": "acquire", "target": "white_water_rose", "qty": 3, "text": "White water roses from Sallow Bottom" }],
  "rewards": [{ "do": "give", "item": "rock", "qty": 2 }, { "do": "give", "item": "small_water", "qty": 2 }]
}
```

**Text**

- `book_first`: "{name}: the cellar bed is for winter. In summer the white water roses are better wild. Sallow Bottom, below the jetty, where the bank is soft." / "Three is plenty. Pick them before the bell. I never learned what happens after, and I never needed to. J." Options: **Three, before the bell** `quest before_the_bell` · **Close the book**
- `book_wait_1`: "Sallow Bottom, below the jetty. Three. Before the bell. J."
- `book_in_1`: "The next page has been turned down for you. There are two rocks and two vials of water on the barrel that you would swear were not there this morning." Options: **Turn the page** `handin before_the_bell` · **Not yet**
- The roses are kept: F2 needs one.

**Omen tie:** "Do not pick the white roses after dark. If true: something in the cellar notices." The M5 omen row may read `night` at the moment a rose prop is looted. This quest neither requires nor forbids a night picking; it only passes on the warning, in Julie's words, which admit she does not know.
**Co-op:** roses count across bags.
**Why it is fun:** Julie's voice gives an instruction and withholds the reason (K17), the cellar roses she already knows get a second, wilder source, and the player who picks one after the bell to see what happens is doing exactly what the county is for.

### F2. `rose_and_stone`: Rose and Stone

| | |
| --- | --- |
| Chain | F, 2 of 2. Handed to the dog, so **by day only** |
| Plays at | Julie's bench (the house) |
| Prerequisite | `before_the_bell` done |
| Danger | None |

```json
"rose_and_stone": {
  "name": "Rose and Stone",
  "description": "The turned-down page: rock breaks to stone at the bench. Stone, a vial of water and a white water rose make Stone Skin. Julie writes that one should be kept by the door, and that the dog will want to see it. She does not say why a dog would.",
  "completion": "The dog looked at it for some time. Then it told you where the next two things on the shelf used to be.",
  "requirements": [{ "type": "acquire", "target": "potion_stoneskin", "qty": 1, "text": "Small Stone Skin Potion" }],
  "rewards": [{ "do": "give", "item": "honeylace_lily", "qty": 2 }, { "do": "give", "item": "gold_dust", "qty": 1 }]
}
```

**Text**

- `book_second`: "Rock breaks to stone at the bench. Stone, water, white rose: Stone Skin. For when running is no longer an option." / "Keep one by the door. Show the dog. The dog will want to see it. J." Options: **I'll make one** `quest rose_and_stone` · **Close the book**
- `book_wait_2`: "Stone, water, white rose. Show the dog. J."
- `book_after`: "The rest of the book is planting dates. The last entry is for a Sunday and says only: bulbs in. she will be here before they are up."
- The dog, `dog.stoneskin_done` (a `questReady` row, placed with the other hand-in rows at the top of the dog's `start` list): "Rose and stone. Grey, and it does not settle. That is right." / "There were six on the shelf by the door. Then there were none, and then there was only the door." / "Lily and gold dust make the next one. There were lilies on that shelf as well." Options: **Here** `handin rose_and_stone` · **Not yet**
- `book_in_2` (fallback for the night, so the quest always has a home): "Show the dog, it says. The step is empty until six." (No hand-in here: this is a progress line only. The hand-in home is the dog.)

**Dog rules check:** no "I", "me" or "my" about anything Julie did; no "she" about anything only Julie could know. "There were six on the shelf" is what a dog in that house would have seen.
**Omen:** none.
**Co-op:** the potion may be in any bag; it is not taken.
**Why it is fun:** it teaches the second recipe family by handing her the missing two-thirds and letting her find the bench herself, the reward is the makings of a third, and the dog's three sentences are the saddest thing in the Lowfields and explain nothing.

---

## Chain G. The Company (the mine mouth and Quarry Steps)

### G1. `stood_down`: Stood Down

| | |
| --- | --- |
| Chain | G, 1 of 2. **Pure kill, with a reason. Givers dead** |
| Giver | The Company notice: a `sign`, key `company_notice`, tree `company_notice`, at the mine mouth beside a pay hatch with nobody behind it |
| Plays at | Area `quarry_steps` |
| Prerequisite | None |
| Danger | **3** by day, foothill, off the road: cliffs narrow the ways in and out. The mine mouth's fire is the run back |

```json
"stood_down": {
  "name": "Stood Down",
  "description": "The Goldskin Mining Co. stood its quarry gang down: five men. The notice says any man found working Quarry Steps will be paid off. The gang is still working. The pay hatch is open and there is nobody behind it.",
  "completion": "Five pay packets in the pigeonholes, never drawn. In each, in place of wages, a bar of iron or a plank, and a slip that says IN LIEU.",
  "requirements": [{ "type": "kill", "target": "quarryman", "qty": 5, "text": "Men found working Quarry Steps" }],
  "rewards": [{ "do": "give", "item": "iron", "qty": 4 }, { "do": "give", "item": "wood", "qty": 2 }]
}
```

**Text**

- `company_offer`: "GOLDSKIN MINING Co. QUARRY GANG (5 MEN) STOOD DOWN UNTIL FURTHER NOTICE. Any man found working Quarry Steps will be paid off at this hatch." / "The gang has been told. The gang is still working." Options: **Go up and tell them** `quest stood_down` · **Leave it**
- `company_wait`: "5 MEN. You can hear picks from here when the wind is right. They keep very good time."
- `company_in`: "The hatch is open. Five pigeonholes, five packets, five names. Nobody is behind the counter to stop you." Options: **Draw their pay** `handin stood_down` · **Not yet**
- `company_after`: "GOLDSKIN MINING Co. QUARRY GANG (5 MEN) STOOD DOWN. Someone has added in pencil: and stayed down. thank you."

**Omen tie:** neighbouring "No exit either, some nights" by voice only: the same Company, the same stencil.
**Co-op:** kills are shared. Each player draws iron and wood, which is generous on purpose: a party of four needs more Repair.
**Why it is fun:** "paid off" means what it says, flatly (`VOICE.md` rule 7); the patch is a real step up with a fire nearby; and the pay is exactly what the mine's broken track and stairs will ask for, which also eases the brazier decision (K10).

### G2. `down_at_five`: Down at Five: 4

| | |
| --- | --- |
| Chain | G, 2 of 2. **Giver dead** |
| Giver | The gang's tally slate: a `sign`, key `tally_slate`, tree `tally_slate`, at anchor `quarry_camp` (a cold camp at the foot of the Steps, on the side nearest the mine road) |
| Plays at | Rect `quarry_top`, at the head of `quarry_steps` |
| Prerequisite | `stood_down` done |
| Danger | 3, the far side of the patch she has just thinned. They come back (K8) |

```json
"down_at_five": {
  "name": "Down at Five: 4",
  "description": "A cold camp under Quarry Steps: five dinner tins laid out, a kettle, a tally slate. The slate says five men went up at seven and four came down at five, and that the foreman would see about it. There is no later entry.",
  "completion": "At the top, a rock the size of a shed lies across the mouth of an adit. It was placed, not fallen. In the foreman's tin beside it: a gilder's book with one leaf left. Behind the rock someone is working, steadily, with a hand pick.",
  "requirements": [{ "type": "location", "target": "quarry_top", "qty": 1, "text": "The top of Quarry Steps" }],
  "rewards": [{ "do": "grow", "stat": "spirit", "amount": 3, "id": "page_quarry" }]
}
```

**Text**

- `slate_offer`: "GANG OF 5. Dinners: 5. Up at seven: 5. Down at five: 4." / "Foreman to see about it." / "Five tins are laid out by the kettle. Four have been eaten from." Options: **Go up and see** `quest down_at_five` · **Leave it**
- `slate_wait`: "Up at seven: 5. Down at five: 4. The steps start behind the camp."
- `slate_in`: "There is room on the slate under the last line, and a stub of chalk." Options: **Write: found** `handin down_at_five` · **Not yet**
- `slate_after`: "Down at five: 4. Under it, in your writing: found. Under that, not in your writing, a tick."
- The adit: prop `adit_rock`, key `quarry_adit`. Tree `adit_rock`: "A rock across the adit mouth. Too big to shift and too neat to have fallen. It would take something sudden." (Later it `answers` Explosion: `the_adit`, K14.)
- The trigger on `quarry_top` runs `location quarry_top`. The foreman's tin is dressing only; the page arrives through the reward so that it is the world's.

**Omen:** none. The hand pick is a fact on every seed, and is checkable the day she learns Explosion.
**Co-op:** location is shared.
**Why it is fun:** one missing man is a gap of one (K17); the climb ends at a sealed thing she cannot open for two dungeons (K16, and `PLAN.md`'s verb gates); and the last line of the slate is the only time in the Lowfields that something writes back.

---

## Chain H. The Right of Way (Castle to the farm)

### H1. `footpath_three`: Footpath No. 3

| | |
| --- | --- |
| Chain | H, 1 of 2. **The hub's breadcrumb to its outpost** |
| Giver | The parish board (first notice shown) |
| Plays at | A footpath from stile `hedge_stile_town` (just outside Castle) through area `long_hedge` to stile `hedge_stile_farm` (in sight of the farm gate) |
| Prerequisite | None |
| Danger | 2 by day through the Long Hedge, no lamps. By road the same journey is 0 to 1 and takes about twice as long. After the bell the path is 3 and the road is lit |

```json
"footpath_three": {
  "name": "Footpath No. 3",
  "description": "The parish council must walk each right of way once a year or it lapses, and nobody has been free. Footpath No. 3 runs from Castle to Lowfield Farm by the Long Hedge. The notice gives it twelve minutes, and longer in the evening.",
  "completion": "Stile to stile. It is a good deal shorter than the road, and nothing on it is lit. You enter the date on the fingerpost card. The last date on it is in the same month, and not in this century.",
  "requirements": [
    { "type": "location", "target": "hedge_stile_town", "qty": 1, "text": "The stile outside Castle" },
    { "type": "location", "target": "hedge_stile_farm", "qty": 1, "text": "The stile by Lowfield Farm" }
  ],
  "rewards": [{ "do": "give", "item": "apple", "qty": 3 }, { "do": "flag", "flag": "footpath_walked" }]
}
```

**Text**

- `path_offer`: "CASTLE PARISH COUNCIL. PUBLIC FOOTPATH No. 3, Castle to Lowfield Farm by the Long Hedge. A right of way lapses if nobody walks it within the year. Nobody has been free." / "12 minutes. Longer in the evening." Options: **Take it down** `quest footpath_three` · **Leave it**
- `path_wait` (board): "Footpath No. 3. The stile is past the last house on the farm side."
- Each stile has a fingerpost: a `sign`, keys `fingerpost_town` and `fingerpost_farm`, tree `fingerpost`. The rects round the stiles run the `location`s.
- `fingerpost_plain`: "PUBLIC FOOTPATH No. 3. A card in a tin frame for walkers to date. It has been a long time between walkers."
- `fingerpost_in` (when `questReady`): "A card in a tin frame, and a pencil on a string." Options: **Date it** `handin footpath_three` · **Not yet**
- If she refuses, the board still moves on to its next notice (see the tree order in ASKS C), so one notice can never block the others.

**Omen (new, proposed):** `longer_in_the_evening`. If true, after the bell the far stile's rect only answers from the farm side: walked from the town it takes a second approach, by the hedge gap. If false, twelve minutes is twelve minutes. Fair either way: the road is always there.
**Co-op:** locations are shared. `flag` without `add` is safe to run per player.
**Why it is fun:** it gives her the region's first real route choice, safe and long against short and dark (K21), and having walked it once she owns a shortcut for the rest of the game (K1).

### H2. `if_found`: If Found

| | |
| --- | --- |
| Chain | H, 2 of 2. Breadcrumb back to Castle Halt |
| Giver | A walker's haversack in a hollow tree: a `lost_thing`, key `haversack_drop`, at anchor `hedge_tree` on the footpath inside `long_hedge`. Picking it up gives the quest |
| Hand-in home | The lost-property book at Castle Halt |
| Prerequisite | None. It is found, not offered |
| Danger | 2 where it lies. The rest is road |

```json
"if_found": {
  "name": "If Found",
  "description": "A walker's haversack, pushed into a hollow tree on Footpath No. 3 to keep it dry. The label says: if found, return to Lost Property, Castle Halt. Finder's due. It was pushed in from the inside of the tree.",
  "completion": "You enter it in the book. The finder's due is the contents, less the notebook, which stays. Hemshade root, water, and a line in the notebook: root and water, rub it on the blade and not the hand.",
  "requirements": [{ "type": "acquire", "target": "haversack", "qty": 1, "text": "A walker's haversack" }],
  "rewards": [{ "do": "give", "item": "hemshade_root", "qty": 3 }, { "do": "give", "item": "small_water", "qty": 2 }]
}
```

**Text**

- The prop: loot `haversack` 1; `use`: `quest if_found`.
- `haversack_in` (book): "There is a line for it already: one haversack, canvas, walker's. Reported lost by the owner, in person, on a date that has not happened yet." Options: **Hand it in** `handin if_found`, `take haversack` · **Not yet**

**Omen:** none.
**Co-op:** whoever is carrying it should hand it in.
**Why it is fun:** it is found, not given (K22); it turns the shortcut into a reason to go back to the station; and its reward is a recipe she has not been told, learned by reading someone else's notebook.

---

## 3.1 Coverage against the brief

| Asked for | Where |
| --- | --- |
| Two or more givers dead or absent | The nurse (D1, D2), A. Pell (E1, E2), the quarry gang (G1, G2), Julie (F1, F2) |
| Two or more available or completable only at night | `to_be_collected`, `plot_nine`, `after_the_bell`, `number_fourteen` |
| A patch well above base, with a clear warning | `not_turnips` (Top Field, 3); also `her_coat` and `stood_down` |
| Pays off something the dog said | `number_fourteen`: "Fourteen. It is a different number if you count." |
| Uses the car in the wood | Chain D; the car's planks also drive the decision in E2 |
| At the station | Chain A, and H2's hand-in |
| At the farm | Chain C |
| Crosses the map and comes back changed | `left_luggage`: platform, mine road, platform; returns with a page |
| A small number of pure kill quests, each with a reason | `rats_in_the_sheds` (the Society), `not_turnips` (the farmer), `stood_down` (the Company); one named kill, `plot_nine` |
| Growth by finding | Jars: `plot_nine`, `not_turnips`, `mrs_allens_dressing`. Pages: `left_luggage`, `down_at_five`, and the parcel in `to_be_collected` |
| A fire lit somewhere new | `the_lampmans_brazier` |
| Recipe-relevant materials | B1 (shed), F1, F2, H2; Repair materials in G1 |
| `STORY.md` respected | Nothing explains the county, names what the dog is, or shows a child. Julie appears as handwriting and as labels on jars |

Requirement mix across the nineteen: 4 kill, 6 acquire, 9 location. No chain repeats a type in consecutive links without changing the conditions.

## 3.2 Omens this region's quests touch

Existing (`STORY.md` section 6): the scarecrow (C), the east-road lamps (E, by neighbourhood), the white roses (F1), the train (A3). Proposed new rows, each an ordinary notice first, each fair:

| id | Text lives in | If true | If false | Fair because |
| --- | --- | --- | --- | --- |
| `plot_nine_not_let` | The Society's notice | The plot is freshly dug again the morning after | It stays as left | Nothing is blocked |
| `pike_after_nine` | Mr Pike's note | A knock after the bell is answered | Nothing answers | She was told not to knock |
| `longer_in_the_evening` | The footpath notice | After the bell the far stile must be reached from the farm side | Twelve minutes | The lit road is always open |

No quest text changes with an omen's truth. Only the world does.

---

# ASKS

Everything Part 3 needs that does not exist today. Ids are final unless John redlines them.

## A. New items (`items.json`)

| id | name | description | icon |
| --- | --- | --- | --- |
| `lost_glove` | Red Glove | "A child's glove, left hand. Hand it in at Castle Halt. The name tape inside says SCHOOL and nothing else." | NEW `item_glove`: a small red mitten |
| `lost_hat` | Felt Hat | "A man's hat. Hand it in at Castle Halt. He said he would only be a moment." | NEW `item_hat`: a grey trilby from the side |
| `lost_tin` | Dinner Tin | "A carter's dinner tin. Hand it in at Castle Halt. The sandwiches inside are fresh." | NEW `item_tin`: an oval tin with a wire handle |
| `key_left_luggage` | Trunk Key | "Stamped G.M.Co. Opens the trunk on the platform at Castle Halt. It is heavier than the lock needs." | reuse `item_key_gold` |
| `key_nurses_case` | Case Key | "A small key on a hospital tag. Opens the nurse's case. The tag gives a ward number and no hospital." | reuse `item_key_iron` |
| `nurses_parcel` | Parcel for Mrs Allen | "Dressings, made up and labelled in a neat hand. For the door facing the fire in Castle square." | reuse `item_present` |
| `haversack` | Walker's Haversack | "Canvas, dry inside. The label says: if found, return to Lost Property, Castle Halt." | NEW `item_haversack`: a canvas bag with one strap |

All are `maxStack: 1`, `usable: false`, `cooldown: 1`. The keys carry `opens: "left_luggage"` and `opens: "nurses_case"` and are not `bound` (they are consumed when they turn). The others become story items automatically because a quest acquires them.

## B. New unit rows (`units.json`). No new art

| id | name | behaviour | spawns | art |
| --- | --- | --- | --- | --- |
| `pumpkin_top` | Field Pumpkin | Exactly as `pumpkin` (strength 14, melee, aggro 10, leash 40, respawn 600; in `top_field` the ecology row stands it up at the next ten-minute mark instead) | 7 in area `top_field`, phase 3 | reuse sprite `pumpkin` |
| `quarryman` | Quarryman | As `skeleton` (strength 12, melee, aggro 10, leash 40, respawn 600; in `quarry_steps` the ecology row stands it up at the next ten-minute mark instead); loot `rock` 1 at 0.5, `iron` 1 at 0.25 | 6 in area `quarry_steps`, phase 3, in two groups of three | reuse sprite `skeleton` |
| `plot_tenant` | The Tenant of Plot 9 | As `skeleton_guard` (strength 16, aggro 12), `respawn: 0`, `nightOnly: true` (ASKS F2), leash 20 so he never leaves the allotments | 1 at mark `plot_nine`, key `plot_tenant`, phase 2 | reuse sprite `skeleton` |

The shared `rat` row serves B1: no new row, only placement.

## C. New props (`props.json`) and dialogue trees

**Prop rows**

| id | like | differs | art |
| --- | --- | --- | --- |
| `notice_board` | `sign` | w 3, h 1, prompt "Read" | NEW ART preferred (a board on two posts with papers). Fallback: reuse `sign` |
| `door_talk` | `door` | No `to`; never opens; prompt "Knock" | reuse sprite `door` |
| `lamp_dead` | `lamp_post` | No `light`, no `nightOnly`; prompt "Look" | reuse sprite `lamp_post` |
| `scarecrow` | `pillar` | w 1, h 1, solid, prompt "Look" | **NEW ART.** The approved `scarecrow` small place is drawn as a `pillar` today and needs this anyway |
| `lost_thing` | `herb` | prompt "Pick up"; `hideWhenUsed`; flat | reuse sprite `note` until a small bundle sprite exists |
| `campfire_cold` | `campfire` | No `light`, no `rest`; prompt "Look" | reuse sprite `campfire`, unlit frame if there is one |
| `adit_rock` | `pillar` | w 3, h 2, solid, blocks sight, prompt "Look" | reuse `rock` art scaled, or `root_wall` |

The shrine, the slate, the Company notice, the stake and the fingerposts are existing `pillar` and `sign` rows with a `talk` tree. The shelf, shed, trunk and case are existing `chest` and `boss_chest` rows. The brazier is an existing `campfire`, hidden at build. Two growth pickups, `preserve_jar` and `gold_leaf`, will be wanted for the loose upgrades in the dangerous patches (NEW ART, one small sprite each, `once`, `hideWhenUsed`, `use`: a `grow` row); no quest above depends on them.

**Dialogue trees** (`dialogue.json`). Node text is in Part 3. The `start` order matters: **every hand-in row (`questReady`) sits above every other row** (POSTMORTEM P0.4). The last row is unconditional. One deliberate exception is marked in `garden_book`, where the `questReady` row is a night-time progress line and the hand-in belongs to the dog.

`lost_property_book`, speaker "Lost Property":
1. `questReady if_found` → `haversack_in`
2. `questReady lost_property` → `lp_in`
3. `questReady left_luggage` → `ll_in`
4. `questReady to_be_collected` → `tbc_in`
5. `questActive lost_property` → `lp_wait`
6. not `questDone lost_property` → `lp_offer`
7. `questActive left_luggage` → `ll_wait`
8. not `questDone left_luggage` → `ll_offer`
9. `questActive to_be_collected` → `tbc_wait`
10. not `questDone to_be_collected` → `tbc_offer`
11. `flag took_parcel` → `closed_taken`
12. → `closed`

`parish_board`, speaker "Parish Board". A refused notice sets a `seen_` flag on its refusal option so the board moves on and comes round again:
1. `questReady rats_in_the_sheds` → `rats_in`
2. `questReady plot_nine` → `plot_in`
3. `questActive footpath_three`, not `flag board_path_told` → `path_wait` (its node sets `board_path_told`, so it shows once and the board moves on)
4. not `questDone footpath_three`, not `questActive footpath_three`, not `flag seen_path` → `path_offer` (**Leave it** sets `seen_path`)
5. `questActive rats_in_the_sheds` → `rats_wait`
6. not `questDone rats_in_the_sheds`, not `flag seen_rats` → `rats_offer` (**Leave it** sets `seen_rats`)
7. `questActive plot_nine` → `plot_wait`
8. `questDone rats_in_the_sheds`, not `questDone plot_nine`, not `flag seen_plot` → `plot_offer` (**Leave it** sets `seen_plot`)
9. → `board_idle`: "CASTLE PARISH COUNCIL. No further notices. The drawing pins have been left in rows." Its node clears the three `seen_` flags (`flag seen_path value 0`, and so on), so refused notices come round again.

`farmer`, speaker "The Farmer":
1. `questReady three_scarecrows` → `farmer_in_1`
2. `questReady after_the_bell` → `farmer_in_2`
3. `questReady not_turnips` → `farmer_in_3`
4. `questActive three_scarecrows` → `farmer_wait_1`
5. `questActive after_the_bell` → `farmer_wait_2`
6. `questActive not_turnips` → `farmer_wait_3`
7. not `questDone three_scarecrows` → `farmer_first`
8. not `questDone after_the_bell` → `farmer_second`
9. not `questDone not_turnips` → `farmer_third`
10. → `farmer_idle`

`scarecrow_gate`, `scarecrow_hedge`, `scarecrow_top` (three trees, speaker "Scarecrow"): 1. `night` → `night` node (runs `location <id>_night`); 2. → `day` node (runs `location <id>_day`). The twin `scarecrow_top_near` uses tree `scarecrow_top`.

`car_glovebox`, speaker "Glovebox":
1. `questReady the_nurses_round` → `glovebox_in_1` (its **Unfold it** option goes to `glovebox_second`)
2. `questReady her_coat` → `glovebox_in_2`
3. `questActive the_nurses_round` → `glovebox_wait_1`
4. `questActive her_coat` → `glovebox_wait_2`
5. not `questDone the_nurses_round` → `glovebox_first`
6. not `questDone her_coat` → `glovebox_second`
7. → `glovebox_empty`: "A road atlas of somewhere else. The clipboard has gone back under the seat where you found it, though you do not remember putting it there."

`note_allen`, `note_pike`, `note_crane` (speaker "Note on the Step"): one node each, running `location cottage_allen`, `cottage_pike`, `cottage_crane`.

`mrs_allen`, speaker "Mrs Allen": 1. `questReady mrs_allens_dressing` → `allen_in`; 2. `questDone mrs_allens_dressing` → `allen_after`; 3. → `allen_before`.

`pell_stone`, speaker "Stone":
1. `questReady number_fourteen` → `pell_in`
2. `questActive number_fourteen` → `pell_wait`
3. not `questDone number_fourteen` → `pell_first`
4. not `questDone the_lampmans_brazier`, not `questActive the_lampmans_brazier` → `pell_second`
5. → `pell_after`

`lamp_12`, `lamp_13`, `lamp_15` (speaker "Lamp Post"): 1. `night` → `night` node (runs `location`); 2. → `day` node.

`brazier_cold`, speaker "Brazier": 1. `questReady the_lampmans_brazier` → `brazier_ready`; 2. → `brazier_empty`.

`garden_book`, speaker "Julie's Garden Book":
1. `questReady before_the_bell` → `book_in_1`
2. `questActive before_the_bell` → `book_wait_1`
3. not `questDone before_the_bell` → `book_first`
4. `questReady rose_and_stone`, `night` → `book_in_2`
5. `questActive rose_and_stone` → `book_wait_2`
6. not `questDone rose_and_stone` → `book_second`
7. → `book_after`

`dog` (existing tree, three new rows):
- `questReady rose_and_stone` → `stoneskin_done`, inserted with the other `questReady` rows at the top.
- `questDone number_fourteen`, `flag said_fourteen`, not `flag dog_heard_lamps` → `lamps_counted_f` (the same two lines as `lamps_counted`, with `goto: "lamps_fourteen"`; `lamps_fourteen` sets `dog_heard_lamps`).
- `questDone number_fourteen`, not `flag dog_heard_lamps` → `lamps_counted` (sets `dog_heard_lamps`).
- Both are inserted **below** every `questReady`, `questActive` and `offer_` row and **above** the poke rows, so they can never stand in front of a hand-in. Neither adds to `dog_pokes`.

`company_notice`, speaker "Company Notice": 1. `questReady stood_down` → `company_in`; 2. `questActive stood_down` → `company_wait`; 3. not `questDone stood_down` → `company_offer`; 4. → `company_after`.

`tally_slate`, speaker "Tally Slate": 1. `questReady down_at_five` → `slate_in`; 2. `questActive down_at_five` → `slate_wait`; 3. `questDone stood_down`, not `questDone down_at_five` → `slate_offer`; 4. `questDone down_at_five` → `slate_after`; 5. → `slate_plain`: "GANG OF 5. Dinners: 5. Up at seven: 5. Down at five: 4. You can hear them working above."

`fingerpost`, speaker "Fingerpost": 1. `questReady footpath_three` → `fingerpost_in`; 2. → `fingerpost_plain`.

Single-node trees: `carters_note`, `plot_nine_stake` (night row first), `adit_rock`.

## D. Placement needs

**Anchors (the one new generator idea).** An anchor is a small place of a stated `pois.json` kind, placed by the skeleton under a constraint, guaranteed on every seed, and given a stable name. Each anchor yields a mark `<id>`, a rect `<id>` (13 x 11 cells round it) and the dressing listed. A skeleton that cannot place them all is re-rolled, as for sites. Anchors count toward the region's small-place budget.

| anchor id | kind | constraint | dressing |
| --- | --- | --- | --- |
| `halt_well` | `well` | Beside the `station` to `julie_house` road | `lost_thing` with loot `lost_glove` |
| `halt_signpost` | `signpost` | Beside the same road, at least 150 m from `halt_well` | `lost_thing` with loot `lost_hat` |
| `halt_cart` | `cart` | Beside the `julie_house` to `town` road | `lost_thing` with loot `lost_tin` |
| `carters_cart` | `cart` | Beside the `town` to `gold_mine` road, 150 to 350 m from `gold_mine` | `lost_thing` `carters_key_drop` (loot `key_left_luggage`, `use`: `location carters_cart`); `note` with tree `carters_note` |
| `scarecrow_gate` | `scarecrow` | 60 to 140 m from `farm`, beside its road | prop `scarecrow`, tree `scarecrow_gate` |
| `scarecrow_hedge` | `scarecrow` | Inside area `long_hedge` | prop `scarecrow`, tree `scarecrow_hedge` |
| `scarecrow_top` | `scarecrow` | On the rim of area `top_field` (radius minus 10 m), on the side nearest `farm` | prop `scarecrow` key `scarecrow_top`, tree `scarecrow_top`; and a second prop key `scarecrow_top_near`, hidden, 40 m nearer `farm` on the line to `farm_gate` |
| `cottage_allen` | `cottage` | The roadside cottage nearest area `sallow_bottom` | `note`, tree `note_allen`, on the step |
| `cottage_pike` | `cottage` | Nearest `long_hedge`; not `cottage_allen` | `note`, tree `note_pike` |
| `cottage_crane` | `cottage` | Nearest `quarry_steps`; not the other two | `note`, tree `note_crane`; five `crate` props as the parcels |
| `nurses_coat` | `hollow_tree` | Inside area `behind_the_car`, at least 80 m from `car_wood` | `lost_thing` `nurses_coat_drop` (loot `key_nurses_case`, `use` as in D2) |
| `hedge_tree` | `hollow_tree` | On the footpath inside `long_hedge` | `lost_thing` `haversack_drop` (loot `haversack`, `use`: `quest if_found`) |
| `pell_shrine` | `shrine` | Beside the **last lit lamp** of the Lowfields road whose unlit remainder is longest (at least 300 m). Not the station to Julie's to town roads | the shrine's `pillar` gets tree `pell_stone` |
| `sallow_jetty` | `jetty` | Inside area `sallow_bottom` | 5 x prop `rose` (loot `white_water_rose` 1) within 40 m, on the bank |
| `quarry_camp` | `camp` | On the rim of `quarry_steps`, on the side nearest the `town` to `gold_mine` road | `sign` key `tally_slate`, tree `tally_slate`; five `rock` props as tins |

**Along the dead run** (from `pell_shrine`, continuing away from the town, beside the road): `lamp_dead` props keys `lamp_12`, `lamp_13`, `lamp_15` at one, two and three lamp spacings (26 cells each, matching the lit runs), each with its tree; then, one spacing further and 4 cells off the road, `campfire_cold` key `pell_brazier_cold` (tree `brazier_cold`) and on the same cells `campfire` key `pell_brazier`, `hidden: true`, tree `fire`.

**The footpath**: a 2-cell `Dirt` stroke from a point 30 m outside the `town` gate nearest the farm, through the centre of `long_hedge`, to a point 30 m from `farm_gate`. At each end a `sign` (keys `fingerpost_town`, `fingerpost_farm`, tree `fingerpost`) and rects `hedge_stile_town`, `hedge_stile_farm` (9 x 9). Hedge bushes are cleared along the stroke.

**In set chunks**

- `station`: `table` key `lost_property_book`, tree `lost_property_book`, label "Lost Property", on the cobbles under the lamp. `chest` key `unclaimed_shelf`, locked `generic`, loot as A1. `boss_chest` key `left_luggage_trunk`, locked `left_luggage`, label "The trunk", loot and `use` as A2. Rect `platform`: the cobbled 18 x 19 block. Rect `halt_approach`: the chunk's box grown by 30 cells to the north, east and south. `lost_thing` key `night_parcel`, `hidden: true`, on the existing crate's south side, `use` as A3. Give the existing platform lamp the key `station_lamp`; add `lamp_dead` key `station_lamp_dead`, `hidden: true`, on the same cell.
- `town`: `notice_board` key `parish_board`, tree `parish_board`, at mark `town_square`. `door_talk` key `door_allen`, tree `mrs_allen`, label "Mrs Allen's sister's door", on the house of the lot nearest `town_fire`, facing it.
- `farm`: `door_talk` key `farm_door`, tree `farmer`, label "The farmhouse door", on the south wall of the farmhouse.
- `julie_house`: `note` key `garden_book`, tree `garden_book`, beside the barrel east of the stoop.
- `car_wood`: `car_wreck` gains `talk: "car_glovebox"` (its loot stays; USE loots first, then reads). `chest` key `nurses_case`, locked `nurses_case`, label "The nurse's case", loot and `use` as D2, on the chunk's east side.
- `gold_mine`: `sign` key `company_notice`, tree `company_notice`, west of the mouth. A 3 x 2 block of `Wall` with a gap as the pay hatch (dressing only).

**In areas**

- `allotments`: a 40 x 30 block of `Garden` plots with `Fence` rows; 8 x `rat` at phase 2; `chest` key `allotment_shed`, locked `generic`, loot as B1; one plot 8 x 6 with rect and mark `plot_nine`, a `sign` key `plot_nine_stake` (tree `plot_nine_stake`), and unit `plot_tenant`.
- `top_field`: 7 x `pumpkin_top` at phase 3, within 60 m of the centre, in rows; the field tiled `Garden` so it reads as planted.
- `quarry_steps`: 6 x `quarryman` at phase 3 in two groups; at the centre a `Cliff` face with prop `adit_rock` key `quarry_adit` (tree `adit_rock`), rect `quarry_top` (11 x 9) in front of it, and a `crate` as the foreman's tin.
- `behind_the_car`: no new units; the area's threat already sets the phase of its wildlife. Check that at least four creatures stand between `car_wood` and `nurses_coat`.

**Triggers** (`triggers.json`, all `zone: "county"`)

| id | rect | mode, once | when | actions |
| --- | --- | --- | --- | --- |
| `platform_night` | `platform` | while, true | `night` | `location platform_after_nine` |
| `halt_night` | `halt_approach` | enter, false | `night`; not `flag took_parcel` | `show night_parcel` |
| `halt_day` | `halt_approach` | enter, false | not `night` | `hide night_parcel` |
| `stile_town` | `hedge_stile_town` | enter, true | | `location hedge_stile_town` |
| `stile_farm` | `hedge_stile_farm` | enter, true | | `location hedge_stile_farm` |
| `quarry_top` | `quarry_top` | enter, true | | `location quarry_top` |
| `scarecrow_night` | `scarecrow_top_wide` | enter, false | `night`; `flag omen:scarecrow_closer` | `hide scarecrow_top`, `show scarecrow_top_near` |
| `scarecrow_day` | `scarecrow_top_wide` | enter, false | not `night` | `show scarecrow_top`, `hide scarecrow_top_near` |

`location` is idempotent and flag-backed, so a once-only trigger can never lose progress here (POSTMORTEM P0.5). `platform_night` is `while`, so a player who waits on the platform through the bell is credited at nine without having to step off and on.

**Nothing may be seen to change.** The view is 48 x 27 cells, so a prop shown or hidden by an `enter` trigger must be more than 24 cells across and 14 down from the rect's edge. Two wide rects do this: `halt_approach` (the `station` chunk's box grown by 30 cells on its three landward sides) and `scarecrow_top_wide` (a 120 x 90 rect centred midway between `scarecrow_top` and `scarecrow_top_near`). She walks away, comes back, and it is different: the dog's rule, applied to a parcel and a scarecrow.

## E. Wood

E2's decision only works if wood is scarce but not absent. Today the Lowfields hold two planks (the car) and whatever is in Julie's cellar store. After G1 there are two more per player. That is the intended curve. Do not add loose wood to the Lowfields without revisiting E2.

## F. Engine capabilities. Four, and the fourth is optional

1. **`grow`.** `{ do: "grow", stat: "strength" | "spirit", amount: number, id: string }`. Adds to `state.growth` (new fields `strength`, `spirit`, `found: string[]`), applies to every party body as `teach` does, and is a **no-op if `id` is already in `found`**. *Why:* `PLAN.md` 2.6 decided growth-by-finding and nothing can grant it yet; it is the whole game's reward, not one quest's. *Why the id:* quest rewards run once per player (`handIn`), so without it a party of four would eat the jar four times; it also makes every upgrade countable by the solver's "upgrades for phase N-1 are reachable" rule. Six rows above use it.
2. **`nightOnly` on `UnitDef`.** The mirror of `dayOnly`: hidden from six to nine, never seen arriving or leaving. *Why:* day and night is the spine; `plot_tenant` needs it now and every region's outline has a night quest. It is the three lines beside `dayOnly` in `sim.ts` with the test inverted.
3. **Anchors in the skeleton** (section D): named, constrained, guaranteed small places, as rows beside `pois.json`, yielding marks and rects. *Why:* quests must be written against names, `poi_<n>` is an index that means something different on every seed, and the kinds are rolled by weight so no kind is guaranteed today. This is generator work, not sim work, and it is the same constraint solver the sites already use.
4. **Optional: `take` falls through to the party.** When the talker lacks the item, take it from whoever in the party holds it, as `acquire` already counts across bags. *Why:* symmetry, and no stray parcels in a friend's bag. *Without it:* nothing breaks; a harmless item is left behind.

Not asked for, on purpose: no new requirement type, no conditional actions, no timer, no escort, no new zone, no currency, no quest markers. The omen flags (`omen:<id>`, set per seed at New Game) are `PLAN.md` section 5's own milestone; every quest above completes identically with every omen flag at zero.

**Two tests worth adding with the rows** (they are what stops this document becoming the Phaser build's 61 of 102): every quest id has at least one reachable `quest` action and one reachable `handin` action somewhere in dialogue, triggers or placed props; and every `location` target named by a quest is produced by at least one trigger, node or placed prop. Placed props carry action lists the catalog validator cannot see today, so the second test must walk a built county.
