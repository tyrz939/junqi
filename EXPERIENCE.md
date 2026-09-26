# Jane: the Experience

What she meets, in order, and what holds it true. Written 26 September 2026 from John's direction: *"the whole experience needs to be distilled into highly detailed documents and carefully verified against what will actually happen in the game world. Gameplay and the player's long-running context of what's happening is very much key."* Pair with `WORLD.md` (the living county), `README.md` (the first hour in six lines), `PLAN.md`, `QUESTS.md`, `QUEST-TREE.md`, `DUNGEONS.md` §3 and `VERIFICATION.md` §5 (how this file is checked).

**Status: design for the Rust build. Nothing here is built.** Every claim below is either held by a check or marked *proposed*.

---

## 1. The rule and the format

**Rule: the experience is specified minute by minute for the first hour and hour by hour after, per seed-invariant.** A claim that is true on one seed and not another is not a claim; it goes under *Varies*.

Each beat is a block:

| Field | Holds |
| --- | --- |
| **Sees / hears / can do** | What is on screen and in the speakers, and every verb open to her |
| **Knows / does not** | The journal's facts at this moment (`WORLD.md` §7), and the one thing she does not yet know that the beat is built on |
| **Meanwhile** | What the world is doing that she cannot see (`WORLD.md` §2 to §5) |
| **Checks** | The tags that hold the claim, keyed to `VERIFICATION.md`'s layers: `[L1:…]` it exists on every seed, `[L2:…]` density and structure, `[L4:…]` an experience metric from a trace (time to a place, the first fight, deaths), `[L5:…]` a claim, the journal or cohesion, `[L6:…]` the living world (schedules, presence, weather, ecology, consequences, rumours, the bell, the dog), `[L7:…]` the dossier a person reads |
| **Varies** | By seed (distances inside their bands, which omens are true, the weather) and by player model (below) |

Times are **real minutes after New Game**, then the clock. One game hour is two real minutes; the lamps come on at real minute 3, the bell at real minute 8, the morning bell at 26. Walking is 360 m a real minute on a dry road.

**Player models** are `VERIFICATION.md` L3's: the **Reader** (takes every quest, reads every sign, rests when the journal knows a bed and she is hurt), the **Cautious** (the Reader who rests at every fire and sleeps every night), the **Rusher** (spine only, sprints, never rests until forced), the **Explorer** (frontier walk, quests incidental), the **Co-op pair**, the **Lost** (rebuilds its objective from the journal alone).

---

## 2. The first hour, worked in full

### 0:00 to 0:45 · 17:00 · The platform

- **Sees / hears / can do.** Castle Halt: the platform, a canopy, a fire burning in a brazier, a sign (CASTLE HALT), the lost-property book on its table, the trunk, the timetable, a crate at the platform's end. One road leaves east. The sun low and orange. North over the fields, on the hill, one lit window. The train is gone. She can read (the sign, the book, the timetable), take Lost Property, rest at the fire, walk.
- **Knows.** The letter (in her bag): an aunt, a house at the end of the station road, a map that "was right when I drew it", a dog. **Does not:** that the dog talks; what the window is.
- **Meanwhile.** The town at its doors talking about the lamps. The farmer taking his last look at the scarecrows. The reedcutters stacking reed. The night shift not yet moving.
- **Checks.** `[L1:station on the west edge, one road out, a fire]` `[L6:clock 17:00 at New Game]` `[L6:the School's window lit from 17:00]` `[L5:claim name CASTLE HALT]` `[L5:the book's offer says what QUESTS.md A1 says]`
- **Varies.** The Reader takes A1 here and reads the trunk (locked, "The trunk"). The Rusher is off the platform in ten seconds. By seed: nothing on the platform moves.

### 0:45 to 3:30 · 17:20 to 18:45 · The lit road

- **Sees / hears / can do.** A lamp-lit road, lamps unlit yet, hedges, fields, the well with a plate (THE WELL ON THE STATION ROAD) and a red glove by its wall; further on a signpost with a felt hat at its foot. Rabbits. Crows that follow her a way and turn back. Nothing out here starts a fight with her: by day the Lowfields' own things on their gentlest ground leave her be until the bell (`PLAN.md` §2.6, *Day*), on the road or off it across the fields; she can start one. At real minute 3 (18:30) every lamp on this road comes on within a few seconds, north end first. She can pick things up, read the plate, look north.
- **Knows.** Nothing new unless she reads the well: "a big building on a hill, with one window lit. It is the only light in that direction." **Does not:** that the crows' turning point is the edge of a threat pocket.
- **Meanwhile.** Doors shutting in Castle, north end first. The mine's skeletons still lying. The dog on the left-hand board.
- **Checks.** `[L1:julie_house 520 to 850 m from station by road]` `[L1:halt_well 100 to 280 m and halt_signpost 200 to 420 m from the station, on this road]` `[L1:nothing above threat 1 on this road by day; no true omen on it]` `[L6:wary: no Damage to her before the stoop, Reader and Rusher, on or off the road]` `[L2:the first walk is never empty]` `[L4:time_to_place gate 2 to 4 min, Reader]` `[L6:lamps at 18:30 on the station road on every seed]` `[L6:weather clear on the first walk]` `[L5:claim beside the glove by the well wall]` `[L5:knows school, by the well, not its name]`
- **Varies.** By seed the well and signpost swap order and the road wanders. The Reader reaches the gate as the lamps come on; the Rusher before them (1.5 to 3 min), straight across the fields past bats and spiders that watch her go.

### 2:00 to 4:30 · 18:00 to 19:15 · Julie's gate and stoop

- **Sees / hears / can do.** A gate, a fence, a yard, a house with one lit window downstairs, the stoop with two boards, a barrel east of the step with the garden book on it, a dog sitting on the left-hand board. Along the fence line behind the house, something walking. Reaching the stoop completes the letter (a toast). She can talk to the dog, read the garden book, walk on.
- **Knows.** "Auntie Julie's house": done. **Does not:** whether the thing on the fence line has seen her. It has not; it walks the fence.
- **Meanwhile.** Evensong finishing in Castle. The Voyseys lighting nothing.
- **Checks.** `[L1:the stoop rect, dog, yard_skeleton and house_door on every seed]` `[L6:the skeleton keeps to the fence line until aggro]` `[L5:the garden book offers F1 and says the dog's hours]` `[L4:no death on the first walk; health lost under 15]`
- **Varies.** The Explorer walks past: the road goes on to Castle and the dog says nothing to her back.

### 4:00 to 5:30 · 19:00 to 19:45 · The dog

- **Sees / hears / can do.** "{name}. You came on the Sunday train. She said you would. Then she stopped saying anything." The offer: bones on the fence line; "Space swings." She can accept, ask about a talking dog, refuse, poke it (the chain begins).
- **Knows.** Julie expected her; the dog knows her name; the bell is at nine ("Before nine, if you can"). **Does not:** what the dog is.
- **Meanwhile.** In Castle, Mr Cobb goes in. The reedcutters build the fire up.
- **Checks.** `[L5:claim beside: the bones on the fence line, and there they are]` `[L5:knows: the intro assumes only the letter]` `[L5:no row says Jane; every row says {name}]` `[L6:schedule: the dog on the step at any hour until the bones are down]`
- **Varies.** A kill before accepting does not count (2020's rule): the Rusher who swung first swings again.

### 5:00 to 6:30 · 19:30 to 20:15 · The yard skeleton

- **Sees / hears / can do.** One skeleton, phase 1, walking the fence. By day it minds the fence and not her: she starts it (after the bell it comes for her). Melee only. A bar over its head and no number. She wins in a few swings; she can lose.
- **Knows.** What a swing costs and what a hit costs. **Does not:** that dying now wakes her at the Halt fire on the platform: the party's fire from New Game, until she rests somewhere else.
- **Meanwhile.** Nothing else in the yard. Nothing comes through the fence.
- **Checks.** `[L4:first_fight is the yard skeleton inside Julie's fence]` `[L1:one skeleton inside the fence, none within a screen outside it]` `[L6:wake at the Halt fire before the first rest]` `[L5:enemy health is never a number]`
- **Varies.** The Co-op pair fights it at 62% each: §3.

### 6:00 to 7:30 · 20:00 to 20:45 · The key

- **Sees / hears / can do.** "Down. Good. The house key was always for you." The key. "There is a bed. Sleep in it. If you die out there you wake in the last place you rested." **E** on the door twice: unlock, enter. The banner: Julie's House.
- **Knows.** There is a bed; death costs a walk; the key is hers. The journal: Julie's Kitchen is the next step, and the dog is where to come back to. **Does not:** that the dog is about to leave.
- **Meanwhile.** Doors barring along every road. The Tenant standing up at Plot 9.
- **Checks.** `[L4:the house key and the kitchen by 8 min, Reader]` `[L6:the key paid to everyone, once]` `[L6:consequences: the yard skeleton gone at hand-in, for good]` `[L6:the door takes the key and becomes a nightLock she holds]` `[L5:knows: the reward line and the returnTo]`
- **Varies.** The Explorer, back from Castle after nine, finds the skeleton walking and the step empty: she can put the bones down; she cannot hand them in until six; the house is locked; the platform fire is her rest.

### 7:00 to 10:00 · 20:30 to 22:00 · The kitchen, and the bell

- **Sees / hears / can do.** A kitchen: the note on the table ("Dust, water, pansy. Make it before you go down any stairs…"), the pantry chest, the bench, the stove, the orb beside it, two hatches chalked in Julie's hand (KEYS, the nearer, by the stove; OUT), the stairs up to the bed. At real minute 8 the toast: **"A school bell, a long way off. Nine o'clock."** She can read, open, craft (bags with **I**, three things into the row, take the potion), touch the orb (Icebolt), go up, go down.
- **Knows.** Julie is a witch who wrote lists; the potion; Icebolt; the bell has rung. The journal: Dust, Water, Pansy, back to the dog. **Does not:** that the step is empty now.
- **Meanwhile.** The dog `Absent`. The Lowfields' own ground stops leaving her be; night creatures up in every patch of threat 2 and above; skeletons at the edge of the unlit roads; the mine's two or three on the mine road; the night shift in file on Cinder Walk. The station road's lamps hold: nothing on it. Mist on the water.
- **Checks.** `[L6:the bell at 21:00, heard in the house zone]` `[L6:the dog absent 21:00 to 06:00 once the bones are down]` `[L6:havens: nothing leaked on the station road or inside Julie's fence]` `[L4:Icebolt by 12 min, Reader]` `[L4:a bed known to the journal before 21:00]` `[L5:claim behind: the note's recipe and the pantry's pansy agree]` `[L1:the orb, the bench, the chest and two hatches in the kitchen chunk]`
- **Varies.** A Reader who lingered is on the stoop when the bell goes and sees the step empty a minute later, having looked away. Nobody sees the dog leave (`STORY.md` §3; the presence rule).

### 10:00 to 17:00 · 22:00 to 01:30 · The cellar

- **Sees / hears / can do.** Down either hatch. KEYS comes down in room A: a chest with two iron keys, barrels to push. An iron door that takes one. OUT comes down in room B, the other end, behind the second iron door, with a note by it: "THE OTHER WAY OUT. The keys are in the chest at the first stair: the hatch chalked KEYS, by the stove." She goes back up and down the other. A corridor: the rat room (four rats, a chest with a plain key), the potion room with a bench, the study with a clean ring in the dust, the rose alcove (white roses, gatherable), the storage gate (the plain key: wood x4, iron x4, a small jar *proposed*). A second iron door, room B, the other stair. She can fight, push, unlock, gather, craft, read the desk.
- **Knows.** Rat meat x3 (the dog's next ask is not given yet; the meat counts when it is). One kind of iron key opens either door and is spent. **Does not:** what the ring in the dust was. What the roses are for after dark.
- **Meanwhile.** 22:00: the east road's lamps go out *if that omen is true* (*proposed*). Mr Pollard walks to the Dole Stone. The Arms lit. Mr Dunn ringing from across the square.
- **Checks.** `[L1:the cellar's mission graph as DUNGEONS.md 3.0; a way out from either door]` `[L5:the note by the second iron door says where the keys are]` `[L4:the rats and the meat by 20 min, at most one death, Reader]` `[L6:ecology: rats x4 in the cellar, recovery per WORLD.md 4.5]` `[L5:omen: the rose omen true at the seeded rate; when true, one soldier in the study on a rose picked after dark, and the solver proves the stair]` `[L5:claim behind: the wood and iron in the locked storage room]`
- **Varies.** The Rusher skips the storage room and regrets it at the mine's stair. The rose omen is true on about a third of seeds, and never from behind: the soldier is in the study, not the stair.

### 16:00 to 18:00 · 01:00 to 02:00 · The empty step

- **Sees / hears / can do.** Up the stairs, out of the door: the stoop. The left-hand board empty. The lamps on the road. The fence line quiet. Nothing says anything. She can read the journal: *Dust, Water, Pansy: ready. Back to the dog, on Julie's step, by day.* The map: the house, the road, the platform, and fog.
- **Knows.** "By day." The bed is upstairs. **Does not:** where the dog goes. Nobody ever tells her.
- **Meanwhile.** The quietest hour. What is up stays up and does not come nearer.
- **Checks.** `[L5:knows: the returnTo says "by day" on every quest the dog takes back]` `[L5:no row explains the dog's absence]` `[L6:the step's talk is empty from the bell to six]` `[L4:night exposure, first night, under 3 min outside lamplight, Reader]`
- **Varies.** A player who goes looking (the Explorer) finds the road to Castle unlit past the last lamp and a skeleton at its edge: warned by the farmer's line if she read it, by the dog's "before nine" if not.

### 17:00 to 19:00 · 01:30 to 06:00 · The first rest

- **Sees / hears / can do.** Julie's bed, "made, recently and badly, by someone without the hands for it." **E**: rest, save, sleep. The clock runs to 06:00. **"The bell again. It is only morning."** "When you open your eyes the blanket has been pulled up."
- **Knows.** Where she wakes from now. Day 2, Monday. **Does not:** who pulled the blanket.
- **Meanwhile.** Everything that came up goes back over the hour. The mine's door, if barred, opened at five. The milk round begins at seven. The dog on the step.
- **Checks.** `[L6:a bed sleeps to 06:00 and sets the rest point]` `[L6:the morning bell in the house]` `[L6:presence: by 07:00 no night-only unit stands; none hid in view]` `[L4:the first rest before 21:00, Cautious]` `[L5:the bed's two lines]`
- **Varies.** The Rusher does not rest and is on the mine road at 02:00: the road unlit past the last lamp, the mine's approach no harder than the mine's own rooms (phase 1: skeletons at 100 HP, the phase the ground gives them by day), but in the dark they notice her from further off and follow her further; "No exit either, some nights" is true on a third of seeds and the door is barred until five; the mine's fire is outside it. That is the design working, not failing.

### 19:00 to 22:00 · 06:00 to 07:30 · The morning, the dog

- **Sees / hears / can do.** Daylight, town smoke over Castle. The dog: "Pansy on your fingers. You made one. She was a witch, {name}. A kind, gentle one. So are you." Then: "There are rats under the house… Bring me three pieces of meat." Handed in at once (the meat is already in the bag). "Water and snakeroot makes Stranglethorn…" Then the mine: "at the end of the mine road out of Castle." She can hand in, take, poke, refuse.
- **Knows.** She is a witch, said by a dog. Stranglethorn. The mine is at the end of the mine road out of Castle; the Headmaster and Iron Knuckles are in it. **Does not:** what the Headmaster is doing there.
- **Meanwhile.** The Milkman on High Street. The sweeper on the square. The farmer at the pump. Her arrival known to Mr Cobb since last evening if she passed before six; otherwise from now.
- **Checks.** `[L5:knows: bench_done, rats_done and offer_mine in that order from one visit, each assuming only what the journal holds]` `[L5:claim direction: the mine at the end of the mine road out of Castle]` `[L6:acquire counts what is already in the bag]` `[L6:rumours: her arrival reaches Mr Cobb per WORLD.md 7.2]`
- **Varies.** The Reader hands in Lost Property later, two of three found by accident. The Co-op pair hears the same lines: §3.

### 22:00 to 26:00 · 07:30 to 09:30 · Through Castle

- **Sees / hears / can do.** The road from Julie's to Castle (a minute or two): the overturned cart with the dinner tin in the road beside it. Castle: the square, the fire, the fountain, the memorial, Mr Tolly's bench, the parish board (one notice pinned over the others), the Arms, the Stores, lanes with signs at their west ends. People on steps. The Milkman finishing on Pound Lane; three bottles at No. 7. She can rest at the fire, read the board, knock, talk, shop when the Stores open at nine.
- **Knows.** Castle by day looks normal. One fact per person, and Mr Cobb's "I like to see who comes up the street." **Does not:** which of each person's two things is the false one.
- **Meanwhile.** Mr Cobb walking to his seat for nine. Mrs Tace at the top of Church Lane. The night shift back in the cooling yard.
- **Checks.** `[L1:town 260 to 650 m from julie_house by road; threat 0 inside the fence; a fire]` `[L1:halt_cart on this road]` `[L5:claim beside: the tin by the tailboard]` `[L5:claim count: three bottles at No. 7]` `[L6:schedules: every town person at their 07:30 slot]` `[L5:each town line names a thing within a screen or true everywhere]` `[L5:no two neighbours share a line]`
- **Varies.** Which notice is on the board first (Footpath No. 3). Rain keeps the steps empty and the knock lines change.

### 26:00 to 33:00 · 09:30 to 13:00 · The mine road and the mouth

- **Sees / hears / can do.** Out of Castle by the mine road: five to seven minutes, the longest walk yet. Fields thinning to foothill. The quarry turning with its fingerpost (QUARRY STEPS). The carter's cart short of the mine, a key under it, a note: "Horse would not go past the quarry turning." Then the mouth: a lamp, a fire, the pay hatch, and the sign. **GOLDSKIN MINING Co. CLOSED. No entry. No exit either, some nights.** The Company notice beside it offers Stood Down. She can rest, read, take G1, go in.
- **Knows.** The sign's claim, recorded as *read*. The carter went up on foot. **Does not:** whether the claim is true this seed. Nothing will tell her.
- **Meanwhile.** The mine's skeletons lying by day. The quarry gang at the face. Dinner in Castle. Five tins put out at the canteen hatch nobody comes to.
- **Checks.** `[L1:gold_mine at least 700 m from julie_house; the mine road from town; carters_cart 100 to 280 m short of the mine on it; a fire at the mouth]` `[L1:quarry_steps 290 to 500 m from the mine; the track from the yard]` `[L5:claim name: QUARRY STEPS at the fingerpost]` `[L5:the sign is the model line, unchanged]` `[L5:omen: its truth readable from no row]` `[L4:quest legibility: the Reader reaches the mouth from the text alone]`
- **Varies.** The walk is 1,900 to 2,500 steps from the yard by seed. The Rusher arrived here nine clock hours ago and may have found the door barred; it is open now.

### 33:00 to 60:00 · 13:00 to the evening · Into the mine

- **Sees / hears / can do.** The mine (`DUNGEONS.md` §3.1): a shaft hall, timbered galleries, carts on rails, the store, the first aid room, the pay office made into a classroom with a register and a hand-bell. The broken thing shown first: a stair with two treads gone. A plate under a barrel. The clerk with the keys. The Headmaster, who lifts his hand-bell before it rings: where it rings, whoever is still standing there stands still. She can fight, push, Repair (with the cellar's wood), key doors, find a jar.
- **Knows.** Repair costs what the thing is made of. The children came down here. The Headmaster is keeping order, and the bell is his tell: step out of it and he is standing still with it raised. **Does not:** what happened to them; the register never says.
- **Meanwhile.** The clock runs; the bell comes at real minute 56 (21:00, day 2) and she hears it underground. If the mine omen is true the front door bars behind her; the second way out is there.
- **Checks.** `[L1:the mine's mission graph in its fixed order; a second exit always]` `[L6:the bell heard inside the mine]` `[L5:omen: if true, the front door barred 21:00 to 05:00 from outside only]` `[L5:the register lists names and no fates]` `[L4:the verb's first use is in safety; the rest room before the mini-boss]`
- **Varies.** The whole layout by seed; room variants; whether she brought wood. The Co-op pair enters together; nothing in the mine asks them to split.

---

## 3. The first evening in co-op

A second seat joins at real minute 5, while seat 1 stands at the stoop.

- **Sees / hears / can do.** The guest appears at the party's last fire. Nobody has rested, so that is the fire the party started by: **the Halt fire on the platform at Castle Halt**, 17:15 by the clock, the same sunset. A toast to both: the head count has changed. She has the same name and a different coat. She walks the same road, alone, at 62%: still threat 1, still nothing on it. Seat 1 reads the dog's offer while the world runs; nothing pauses. The skeleton is fought together at 124%, or by one at 62%.
- **Knows.** The journal is the world's: the guest opens it and sees the letter done and the bones offered. **Does not:** the guest has no pansy: bags are hers. One potion is made, by one of them; the orb teaches both.
- **Meanwhile.** The same lamps at real minute 3 for the host; the guest arrives after them. The same bell at real minute 8 for both. The dog leaves for both.
- **Checks.** `[L6:a joiner arrives at the party's rest point, the Halt fire before the first rest]` `[L6:the penalty by head count, everywhere, never on healing]` `[L6:quests, flags, growth and the rest point are the world's; bags are the seat's]` `[L6:the key paid to both]` `[L5:the journal reads the same from either seat]` `[L4:the pair together survives the yard; split, each is warned]`
- **Varies.** Until one of them rests somewhere else, the party's rest point is the Halt fire, and whoever dies in the yard walks back from there. The bed sleeps the clock only when both are resting. A guest leaving hands the house key to the host.

---

## 4. Hour 2 onward: the dungeons, one beat each

Headers and key claims; later passes expand each to the §2 format.

### The Gold Mine · hours 1 to 3 · Repair

- **Sees / hears / can do.** Two nested cycles; the plate, the plain key, the clerk, the Headmaster's key, the Headmaster, the vault, Repair (2 wood), the boss key, Iron Knuckles. A big jar. The Museum key.
- **Knows after.** What Goldskin took; where the children went; the Museum is across the river. **Journal:** `WORLD.md` §9.2, after 1.
- **Meanwhile.** The mine road's skeletons stop walking after hand-in; the Cranes' window sees her come back; the Arms hears by the day after.
- **Checks.** `[L1:the graph; the solver on every seed]` `[L6:consequences: the notice overpainted, the road quiet]` `[L6:rumours: the act rumour at the Arms within its delay]` `[L4:deaths per attempt in band; finishes at phase 1]`
- **Varies.** Layout; variants; whether the front door was barred on the way in.

### The Museum · hours 3 to 5 · Explosion

- **Sees / hears / can do.** Across the river by the tollhouse bridge (the toll board, the omen). A hub and four wings, two states: lights on, a museum; lights off, empty plinths. The MAGIC wing. A portrait with the face scratched out. The forest key.
- **Knows after.** What the gold was for. **Does not:** who scratched the portrait.
- **Meanwhile.** Nobody in the Waters talks to the town. The wing's lights stay on after, seen from the bridge at night.
- **Checks.** `[L1:museum across the river from town, 300 to 850 m by road]` `[L5:omen, proposed: the bridge counts; when true the third crossing of a night is different and survivable]` `[L6:consequences: the wing lit after]` `[L5:cohesion: the portrait is Goldskin's and no line says so]`
- **Varies.** The bridge count (*proposed*). The Waters' mist weight after dark.

### The ruined library and Butterfly Forest · hours 5 to 7 · Grow

- **Sees / hears / can do.** The library's portico (a fire), the returns box, the stacks, Julie's margins, the last page: Grow. The forest: no doors, a ring of glades, things grow where the light comes down, eight butterflies. The Amulet.
- **Knows after.** Where the well were sent, and that they are not coming back.
- **Meanwhile.** Rare herbs in Julie's garden if W7 is done. The butterflies gone from the forest road after.
- **Checks.** `[L1:library 380 to 1000 m from the museum; the forest 260 to 950 m from the library]` `[L6:the canopy light layer keyed to the forest]` `[L6:consequences: the butterflies leave the road at hand-in]` `[L5:cohesion: the margins are Julie's hand, the same as the letter]`
- **Varies.** The glade ring's order.

### The pipes and the Factory · hours 7 to 10 · Electric

- **Sees / hears / can do.** Cinder Walk at night: the night shift in file. The canteen (a fire, the hatch, the rota). The pipes: a drop, a reversal, a way back. The Factory: light is how the machines see you. Electric: store it, release it, turn the machines. The foreman's diary.
- **Knows after.** The Works ran on the same gold. The dead run in the Lowfields can be relit (K13).
- **Meanwhile.** The Factory's lights out for good after; the Works darker, not safer. The night shift stops walking only if K3 is done.
- **Checks.** `[L1:canteen 150 to 400 m from the factory; the graveyard road from town]` `[L6:schedules: the night shift walks Cinder Walk 21:00 to 06:00 and goes home at six, until K3]` `[L6:weather: storm weight highest here at night]` `[L5:claim promise: the rota says "Not relieved" until it is]`
- **Varies.** Storm nights: lamps flicker, bats grounded.

### The Burial Chamber · hours 10 to 13 · Fire

- **Sees / hears / can do.** The graveyard, the footpath behind its gate, the stone that says DO NOT, the way down. A hub and four corners in any order: the Snake, the Spider, the Flower, the Soldier. Cold light shows what is there; warm light keeps it off. The small snakes fed, not fought. The big snake starts again if it loses sight of her. Goldskin, and the Ball.
- **Knows after.** Goldskin is the wizard. The shield was Julie's. The town says his name.
- **Meanwhile.** The graveyard's ground fog gone after. Chapel Rise's keepers up.
- **Checks.** `[L1:burial 120 to 260 m from the graveyard, off road, not seen from any road]` `[L6:the ground fog layer 21:00 to 06:00 until the Burial is done]` `[L5:claim direction: the dog names the graveyard road and "north, toward the School. Do not go as far as the School."]`
- **Varies.** Corner order. The reference build has this dungeon second; the plan has it fifth (`WORLD.md` §9.1).

### The School · hours 13 to 15 · everything

- **Sees / hears / can do.** Up the stair out of the Burial, the one behind Goldskin, into the boiler room: the caretaker's bin, and on it the key to the front doors, which are bolted from inside. After, those doors are the way down to the Bellfield and back. The Bellfield at nine. The building follows the timetable: six lessons, a tower, the bell. Every verb. The choice: re-make the shield and hold it; put the Ball back in the hill; take the Sunday train.
- **Knows after.** What the bell was for. **Does not:** which choice was right. The game does not say.
- **Meanwhile.** The two bells, if K12 was done, falling out of time.
- **Checks.** `[L1:school on the crown, at least 1300 m from the station by road, north of every Lowfields place]` `[L1:the way in is the Burial's stair; the front doors' key is inside, in the boiler room]` `[L6:the window lit every hour of every day until the end]` `[L5:no line calls any choice right]` `[L5:the children are never shown and never enemies]` `[L7:dossier: the School in every first-hour dossier, seen or read, before the bell]`
- **Varies.** The ending. Whether the dog is on the step after.

---

## 5. The long-running context

**Rule: from any save, a returning player or a joining guest can carry on from the journal, the map and the nearest person who tells, within one conversation.** No other source is allowed to be necessary. The Lost model is this rule as a check.

### 5.1 What the journal must hold at each act boundary

| After act | Quests | Facts read or seen | People, last said | The day line |
| --- | --- | --- | --- | --- |
| 0 | The mine, offered; A1 if taken | The bell at nine; the dog not on the step after nine; Julie expected her; the mine is at the end of the mine road out of Castle | The dog: the mine offer | "Day 2, Monday. Slept in Julie's bed. The bell went at nine." |
| 1 | The Museum, offered | The Headmaster keeps a register in the mine; the Company shipped gold; the Museum is across the river from Castle | The dog; the Cranes; the carter's note | "Day 3. Rested at the mine's fire. The notice says OPEN." |
| 2 | The pipes, offered | The MAGIC wing; where the well were sent; Grow, from the last page | The reedcutters' slate; the toll board | "Day 5. Rested at the library. Mist all night." |
| 3 | The Burial, offered | The Works ran on the gold; the foreman's diary; the four markers; the dead run relit | The rota; the hatch; the Voyseys | "Day 7. Rested at the canteen. The night shift did not walk." |
| 4 | The School, offered | Goldskin is the wizard; the Ball; the shield was Julie's | The dog, one slip | "Day 8, Sunday. The train stopped." |
| 5 | None | The choice | | The last line |

Every quest carries its `returnTo`; every fact its key, where and when; every person where they stand by the hour, as she last saw it. Checks: `[L5:knows at each boundary per WORLD.md 9.2]` `[L3:the Lost rebuilds its objective from the journal alone every 20 min]`.

### 5.2 The "I am lost" path

| She does | She gets |
| --- | --- |
| Opens the journal | The active quests, each with its `returnTo` and its steps' landmarks in words ("up the farm track", "at the end of the mine road out of Castle") |
| Opens the map | The ground she has seen; every fire she has rested at; every sign she has read as a dot she can re-read; the School's window as the one fixed light; fog beyond |
| Walks to the nearest fire | A rest point and, at every hub fire, one thing that tells: the parish board, the lost-property book, the order slate, the rota |
| Asks the nearest person by day | Their `tells` line assumes only what her journal holds; it says the next thing, not the whole story. The dog says "It is in your log. Check it." |
| Reads the last day line | Where she slept, what the bell did, what changed |

**Rule: no step of this path needs a marker, a bearing or a number.** If the path fails on any seed at any save, the failure is a bug in L5, not in the player. Checks: `[L4:time lost under 6 min, Reader; under 12, Lost]` `[L5:knows: every step's landmark is in the journal or posted at the place]`.

---

## 6. How this document is verified and maintained

**Verified.** `VERIFICATION.md` §5 runs this file: each beat's tags are checks, and a headless bot plays the first hour on every seed under each player model, writing a **sweep report** in the same beat order with what it saw, heard, could do and had in its journal at each band. `jane sweep --report` diffs the report against §2 to §5. A beat whose claim the report contradicts on any seed fails; a beat whose *Varies* line does not cover the difference fails; a tag naming no check, or a check no beat names, is listed at the foot of the report and fails a gate.

**Maintained.**

| When | What changes |
| --- | --- |
| The owner plays and disagrees with a beat | The beat's claim changes, or the world does; not both. The redline goes to `VOICE.md` or `WORLD.md`; the beat is rewritten to what was decided |
| The sweep report differs from a beat | The world is wrong or the beat is; the diff says which line; one of them is fixed before anything else is added (`PLAN.md` §7) |
| A quest, person, layer or consequence is added | Its beat is added here in the format, with tags, before its rows are written |
| `VERIFICATION.md` renumbers a layer or renames a metric | Tags are re-keyed by a script; no prose changes |
| An act boundary changes | §5.1's row changes and the journal check with it |

**Rule: a beat without a tag is a wish.** It may stay in the file for one milestone, marked *proposed*, and then it gets a check or it goes.
