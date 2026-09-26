# Project Jane — How people will play

> **Partly superseded (September 2026).** The game is being rewritten natively in Rust (`PORT.md`). §1 (why lockstep), §2 (the co-op rules, all decided), §5 to §7 stand. §3 (browser, PWA, Electron, Tauri) and §4 (a WebSocket relay, guests open a URL) are void: everyone runs the native build, the host is `jane-app` or `jane serve` on any machine including a Pi, and the protocol is `ARCHITECTURE.md` §7. §8's two questions are answered there (Host / Join on the title and pause menus; the client token lives in `config.json` beside the saves).

Browser or app, alone or together, and what each costs. Written September 2026 from John's direction: **co-op would be a huge benefit (it is why 2020's engine was MMO-shaped), LAN only, and the game may run in a browser or as an app in a window.** Pair with `ENGINE.md` (the sim this leans on) and `PLAN.md` (where the work is scheduled).

The short version: **the engine is unusually well placed for LAN co-op, the app question is nearly free, and the expensive part is not networking. It is every place the sim says "the player", and that gets more expensive with every system built before it is fixed.**

---

## 1. What the engine already gives us

Co-op netcode comes in two families. The engine decides which one is cheap.

| | Host-authoritative | **Deterministic lockstep** |
| --- | --- | --- |
| Who simulates | The host. Clients send input, receive state | **Everyone.** Peers exchange only inputs |
| On the wire | Entity snapshots / deltas, every tick | One `InputFrame` per player per tick, plus commands. A few dozen bytes |
| Needs | Delta encoding, interest management, a replica state on clients | A sim that is **bit-identical** on every machine |
| Late join | Natural | Send a snapshot of the state, join at tick T |
| Weak point | Much more code; bandwidth | A slow peer stalls everyone; one non-deterministic line is a desync |

`ENGINE.md` §2 is a list of lockstep's requirements, already met and tested: fixed 60 Hz integer ticks, a seeded RNG inside the state, no runtime trigonometry, input quantised to 1/127, array-order iteration, the whole game in one serialisable tree, `hashState`, and a replay test that re-simulates a session to the same hash. Three things follow:

- **The replay format is the wire format.** `sim/replay.ts` already records exactly what lockstep sends: input frames (run-length encoded) and tick-stamped commands. A network session is a replay being written by several hands at once.
- **Late join is a save file.** A joiner receives the host's `GameState` (it is already the save), loads it, and starts consuming inputs from that tick.
- **Desync detection is `hashState`.** Peers compare hashes every second. The F2 overlay already prints it.

So: **lockstep.** Age of Empires and Factorio made the same choice for the same reason. On a LAN (1–10 ms round trip) an input delay of 2–3 ticks (33–50 ms) is below what anyone can feel in a game with a 1.5 s global cooldown, and the "slow peer stalls everyone" weakness barely exists on a LAN with 2–4 machines.

Host-authoritative would throw away everything in that list and need roughly ten times the networking code. It is only the better choice for internet play with strangers, which is out of scope (§5).

**The one real risk** is floating point across JS engines. The sim uses only `+ − × ÷` and `Math.sqrt`, which IEEE 754 and ECMAScript define exactly, and that was the point of banning `sin`/`cos`/`atan2`. It still has to be *proven*, not assumed: run `test/replay.test.ts` in Chromium, Firefox and WebKit and compare hashes (§4, step 2). If every player uses the same app build (§3), the risk disappears entirely.

---

## 2. What the engine assumed that co-op breaks — **done (M1b)**

*Built September 2026, before any more content was written on top of it. 97 tests, 23 of them with two to four seats in one sim (`jane/test/coop.test.ts`). No networking yet: this is the step that had to come first.*

**As built:**

- `GameState.players[]`, up to four `PlayerState`s: seat, client token, body, zone, bar, craft row, conversation, respawn clock, stats, god flag. **What is hers is her bags and her bar. Everything else is the world's**: the heroine's name, quests, flags, what has been learned, and the last fire anyone rested at.
- `Sim.tick(inputs[])` and `Sim.command(seat, c)`. Sitting down and getting up are commands (`join`, `leave`) in the same stream, so a session with people coming and going replays exactly. **That stream is the network protocol.**
- A `World` is now *one zone plus an optional acting player*. The scheduler keeps a context per live zone; every zone with a connected player in it ticks, in fixed order; empty ones are dropped (their state stays). `asPlayer(w, p, fn)` scopes anything done on someone's behalf.
- The load ring is the union of every player's ring. Triggers fire for whoever trips them; a lock-in holds while anyone is alive inside and resets when the last one falls.
- Alone, dialogue still freezes the world (2020's rule). With company nothing pauses: she stands still and reads while the county carries on.
- **The co-op penalty** (John's rule): the world is not rebalanced; the people are weakened, by *how many are connected*, wherever they stand.

  | players | each deals | party together | each takes |
  | ---: | ---: | ---: | ---: |
  | 1 | 100% | 100% | 100% |
  | 2 | 62% | 124% | 115% |
  | 3 | 46% | 138% | 130% |
  | 4 | 38% | 152% | 145% |

  Together is a little better than alone. **Split up, each of you is at 38% in a party of four, by design, until you regroup.** Applied in one place (the damage flush), never to healing. `PARTY_DEALT` / `PARTY_TAKEN` in `sim/constants.ts` are the two rows to tune. The county says so when the head count changes.
- Quest rewards pay **everyone** (nobody is left without the house key because a friend did the talking). Chest loot belongs to whoever opens it, and enemy loot lies where the enemy fell. `acquire` counts what the party holds between them.
- **One heroine.** The host names her at New Game; everyone who joins plays her under that name. Nobody has a name of her own and none is drawn. They are told apart by the coat, and the coat is the seat: plum, teal, moss, ochre. Same drawing, three palette characters swapped (`SEAT_COATS` in `art/units.ts`).
- **Growth is the world's; items are hers.** One of them touches the orb and all of them know Icebolt, including whoever is away and whoever sits down next month (`state.growth`). Found upgrades will live in the same place, so everyone at the table is always as far along as everyone else. Bags never are shared.
- **One fire.** `state.rest` is the last bed or fire *anyone* rested at. Whoever dies wakes there. Whoever sits down arrives there, new or returning, so nobody joins a friend from ten minutes away. Nobody can call anybody back from the dead any other way.
- **A guest's rest saves.** The `rest` event goes to everyone, because the world lives on the host's machine.
- **Open and closed.** A world is single-player until the host opens it (`open` command, seat 0 only). Closing sends nobody home; it stops anyone new. A save always loads closed. The penalty follows the head count, not the door: when the last guest leaves it is an ordinary single-player game again.
- **A needed thing never leaves with a guest.** When someone gets up, what the story cannot go on without (keys, anything a quest asks the party to acquire: `catalog.storyItems`, derived at boot) is handed to a player who is staying, host first. What does not fit lands at that player's feet, and story items on the ground never age out. *Not* dropped where the leaver stood, which was the first idea: that spot can be behind a lock-in, or ten minutes' walk away in a game with no fast travel, and a LAN drop-out chooses its moment badly.
- **Numbers are yours.** Damage and heal events say who did it; a client floats a number only if she dealt it or took it. A friend's fight shows its sparks and the enemy's bar, never her arithmetic. (Numbers stay even though enemy health is never a number: it is a feeling thing, as in classic WoW.)
- **Healing a friend: mouse-over.** Spell kind `ally`. One button, no target frame, no modifier key.
  - *Mouse:* the cast carries the unit under the cursor (`on` in the `bar` / `cast` command; the client works out the hover, the sim checks it). Over a friend: her. Over yourself, an enemy or the grass: yourself. If the friend you pointed at is out of range or out of sight the cast **fails and says why**; it does not quietly fall on you, because you pointed at someone and meant it.
  - *Pad:* no cursor, so the friend nearest the right stick's line (in range, in sight, within 16 px of the line); stick at rest, yourself.
  - The friend under the cursor gets corner brackets at her feet in her coat colour. That is the whole targeting UI.
  - It never fails for want of a friend, so it is the same spell alone. Healing is not scaled by the penalty. No heal spell is placed in the county yet; tests prove the verb on a row of their own.
- A bed only sleeps the clock to morning when the whole party is resting.
- Leaving parks the body with what she owned; the same client token (`who`, kept by her browser, never shown) gets it back. A save is the host's world: seat 0 sits back down, the others wait.
- Events carry routing (`to` a seat, `inZone`), so a client shows only what is hers, her zone's, or the party's.
- The UI and the renderer are built against `PlayerView`, one seat's view. They never see the whole sim, so the same code serves seat 1 alone and seat 4 of four.
- Console: `open`, `close`, `party`, `join [who]`, `leave <seat>` sit an idle body down, for feeling the penalty before there is a network.
- Saves migrate (v2 → v3 moves the old single-player fields into `players[0]`; v3 → v4 moves name, rest point and learning from the person to the world). Saves written before the rename from junqi still open.

What follows is the list that drove it, kept for the reasoning:

| Today | For co-op |
| --- | --- |
| `state.playerId`, `sim.player`, `playerOf(w)` | `state.players[]`: each with a unit, a name, a bar, a craft row, a dialogue, a rest point, a respawn clock. System functions take *the acting player* or iterate *players in this zone* |
| `tick(input)`, `command(c)` | `tick(inputs[])`, `command(player, c)`. The recorder does the same. That **is** the network protocol |
| One current zone: `sim.zone`, `sim.rt` | **Every zone with a player in it ticks.** `World` becomes a per-zone view; the sim holds a small map of live zone runtimes. One player can be in the mine while another is in Julie's kitchen |
| The load ring follows the player | The ring is the union of rings around every player in the zone |
| Dialogue freezes the sim; bags and menus pause it | **Nothing pauses in co-op.** Dialogue, bags and the pause menu become per-player and the world keeps running. (Solo keeps its pauses: it is one `if`) |
| Triggers ask "is the player in the rect" | "Is any player in the rect"; lock-ins count everyone inside; `reset` fires when the *last* player in it dies |
| Quests, flags: one set | **Shared by the party.** One story, one log. Kills and locations count for everyone. (Bags, spellbooks, stats, rest points are per player) |
| A bed sleeps the clock | Only when every player is resting, as in Minecraft |
| `god`, dev commands | Per player |
| The UI and renderer read `sim.player` | They read *the local player's index*. Each machine renders its own camera; lockstep makes that free |

None of this is hard. All of it is *everywhere*, and every system added on top of `playerOf(w)` (omens, threat, side quests, the dungeon generator, fires) adds more places. **The refactor costs about the same today as the networking itself, and roughly doubles with each milestone it is postponed past.** That is the argument for doing it now, before M2, even if the first networked session is months away.

It also pays off at once without any network: a test with **two bots in one sim** proves party rules, shared quests and multi-zone ticking headless, the same way the first five minutes are proven today.

---

## 3. Browser, app, or both

*Void since the Rust decision: the product is the native build, one dedicated binary per target (`PORT.md` §1, §3). Kept as the record of the web-era reasoning.*

**Both, and the browser build stays the product.** The game is a static site: `dist/` is 290 kB and needs no server to play alone.

| Way to play | What it takes | Verdict |
| --- | --- | --- |
| **Browser, hosted** (any static host) | Nothing new | Always. It is the dev loop and the easiest way for anyone to try it |
| **Installed PWA** ("app in a window") | A web manifest + a service worker for offline. A day. No runtime, no installer, updates itself | **Do this.** It is the app-in-a-window for free. It cannot host a LAN game (browsers cannot listen on a socket) |
| **Desktop app (Electron)** | A thin shell around the same `dist/`. ~100 MB, bundles Chromium and Node | **Later, and this is what hosts LAN games** (§4). Also the normal route to Steam. Identical engine on every machine, which retires the float risk in §1 |
| Desktop app (Tauri) | Smaller (~10 MB), but uses the OS webview: WebView2 on Windows, **WebKit on macOS and Linux** | Not for this game. Three different JS engines is exactly what lockstep does not want, and the LAN server would have to be written in Rust |

Saves are gzip in IndexedDB (done, `ENGINE.md` §3), with `localStorage` as the fallback; that works identically in all three.

---

## 4. LAN co-op: the shape

*Void since the Rust decision: the shape is now `ARCHITECTURE.md` §7 (TCP star, host relays and paces, input delay 3, hash every 60 frames, join by snapshot, `jane serve` headless). Step 1 below is done in the TS build and is what the Rust sim carries; steps 2 to 5 are replaced by `PORT.md` P8.*

Browsers cannot accept connections, so somebody has to run something that can. The clean LAN story:

> **The host runs the app (or one command). Everyone else opens a URL.**

- The host process does two dumb things: serves the game's static files over HTTP on the LAN (`http://192.168.1.20:7777`), and relays WebSocket messages between connected players. **It does not run the sim.** Lockstep means the relay never needs to understand the game. About 150 lines of Node.
- Guests need nothing installed: a browser on the same network. Because the page came from the host, the WebSocket is same-origin, and there is no HTTPS / mixed-content problem.
- Peer-to-peer WebRTC was considered and rejected: on a LAN with no internet it needs a signalling step anyway (pasting connection codes between machines), for no benefit over the relay at LAN latency.
- The Electron app is that same host script plus a window. Before it exists, `npm run host` is the host.

**Protocol (all of it):**

```
join      -> name                      <- player index, snapshot (the save), tick T
every tick -> my InputFrame + commands for tick T+delay
             <- everyone's for tick T+delay         (sim advances only when all have arrived)
every 60   -> hashState                 <- mismatch? host re-sends a snapshot to the odd one out
leave / timeout: that player's unit stands idle, then is removed; the party carries on
```

**Steps, each shippable on its own:**

1. **Co-op-ready sim** (§2). No network. Proven by multi-seat tests. ***Done.***
2. **Cross-engine determinism proof**: the replay test run under Playwright in Chromium, Firefox and WebKit, hashes compared. Small, and it also gives the project its first browser test (`SYSTEMS.md` has wanted one since Rule 0).
3. **Host relay + lockstep client**: input delay, stall handling, join-by-snapshot, hash checks, a "Host game / Join game" pair on the title screen.
4. **Co-op rules pass**: enemy health scaling per extra player, shared vs. personal loot, revive-at-a-fire with a living partner, party omens.
5. **Electron shell**, when there is a game worth installing.

---

## 5. Scope

| In | Out |
| --- | --- |
| 2–4 players on one LAN | Internet play, NAT traversal, matchmaking, accounts |
| One shared world and story; drop in, drop out | Dedicated servers, persistence without the host |
| Host's save is the world; guests keep their character in it by name | Characters that travel between worlds |
| Trust: it is your friends on your network | Anti-cheat, authoritative validation |
| Browser guests, app or script host | Rollback netcode, client prediction (LAN latency does not need them) |
| Windows / macOS / Linux desktop | Phones and touch (the UI is pointer-ready, the controls are not) |

Lockstep *would* work over the internet between friends with a higher input delay and a port forward. It is not promised and nothing is designed around it.

## 6. What co-op does to the design

Worth knowing before it is built, because each is a creative call:

- **No fast travel + split parties** means someone can be ten minutes away when you die. That is either the best or the worst thing about it. A living partner at a fire being able to call you back there is the obvious kindness (§4, step 4).
- **The dog is gone after nine for everyone**, and the bell rings for everyone. Shared dread is the point.
- **Omens are per world, not per player**, so the party argues about whether the sign was true. Good.
- **Nothing pauses.** Reading a sign while something walks up behind you is very much in tone.
- **Difficulty**: threat stays a map (`PLAN.md` §2.6) and the world is never rebalanced. More players means weaker players (§2), so the party is pulled together by arithmetic, not by a rule.

## 7. Decided

| | |
| --- | --- |
| Co-op-ready sim | Now. Done (§2) |
| Party size | Four, hard cap (`MAX_PLAYERS`) |
| Splitting across zones | Yes. Every zone with someone in it ticks |
| Balance | No world rebalancing. A server-wide penalty by head count, so a split party is very weak until it regroups |

| Revive | No. The living cannot call the dead back. Everyone wakes at the party's last fire |
| Rest point | Shared. New and returning players arrive at it |
| Saving | A guest resting saves the host's world |
| Name | One, the host's choice. Guests share it |
| Telling players apart | Coat colour by seat, nothing else. No name labels |
| Numbers | Floating damage and healing stay, and you see only your own |
| Growth | Shared by the world, so everyone is equally far along. Items personal |
| Story items | Never lost to a disconnect: handed to a staying player |
| Chest loot | First come. Decided |
| Healing others | Mouse-over: the friend under the cursor, else yourself. Pad: along the right stick |
| Opening a game | Any single-player world can be opened to co-op and goes back to normal, with no penalty, when the guests leave |
| Who hosts | *(2026-09-27)* A player's own game: she hosts and plays seat 0, the others join her. A headless host (`jane serve`) is the secondary path, for a Pi. LAN only for now; the transport is a trait so internet play, a relay or NAT traversal can be added later without touching the lockstep logic (`ARCHITECTURE.md` §7) |

**As built (P8, 2026-09-27).** The Rust session honours every row above, and a test plays each rule the network touches through lockstep peers (`crates/jane-net/tests/coop_rules.rs`): the penalty by head count as seats connect and disconnect, keys surviving a disconnect, the world single-player again when the guests leave, a guest's rest writing the save on the host, arrival at the party's fire, numbers and coats per seat.

## 8. Still to decide, when the network step comes

1. **Where Host / Join live in the UI**: the title screen for Join; the pause menu for "Open this world". *(The calls behind both exist, `ARCHITECTURE.md` §7 "Hooks for the menus"; the command line is `--host` and `--join` until the screens land.)*
2. **A guest's client token**: random, kept beside the saves (`%APPDATA%\Jane\client-token` on Windows, `~/.config/jane/client-token` elsewhere; `--token N` overrides). Deleting it makes her a new arrival with the starting kit; her old body stays parked in the save. Acceptable on a LAN among friends. *(Decided 2026-09-27.)*

