# The Bell at Nine: Voice

How Castle talks, and how {name} thinks. Every sign, note, book, item description, diary entry, toast, loading line and line of dialogue is written against this file. Pair with `STORY.md` (what is true), `WORLD.md` (who stands where, and when) and `PLAN.md` §5 (how claims come true).

**Status: v2, draft for the owner's approval, 9 October 2026.** Nothing in `data/` has been rewritten to it yet. The sample pack is `progress/2026-10-09_65_voice/samples.md`; the open questions are at the end of that file and listed in §12 here.

---

## 0. What changed from v1

The owner's verdict on v1's text was "pretty average": **flat and terse**, **everyone sounds alike**, and **the mood doesn't land** (the unease is generic, not specific or memorable). v1's own rules caused most of it.

| v1 said | v2 says | Why |
| --- | --- | --- |
| Plain, short, declarative. One or two sentences | **Quiet, not flat.** Length is set by the speaker, not by a rule (§2.4) | "Short" drained the life out of people who would talk longer. Mrs Fenn runs on; Mr Coker does not |
| One wrong detail per text | **One wrong thing at the centre**, but it is the speaker's own (§2.3) | v1's single wrong detail became the town's shared tic: lamps, the bell, the window, in every mouth |
| Jane notices; she does not emote | **Jane has an inner voice**: first person, physical, a beat behind, grief she will not name (§5) | She was a camera. Now she is a person who looks away at the right moment |
| The quest log records what Jane did | **The quest log is her diary**, first person, past tense (§6) | The same rule (what she did, not what it meant), in her own hand |
| Townsfolk: small talk with one hole in it | **A voice sheet for every recurring person** (§8), each with a want, a thing they avoid, and a habit you can hear | Cover the name and you should still know who is speaking (§2.6) |
| Registers in one table | A register for every kind of text (§4), including official notices, hand signs, carved stone, books, the dog and Julie | So a notice, a shop card and a confession never share a cadence |
| (nothing) | A list of banned habits (§10) | The tidy closer, the triad and the not-X-but-Y were most of what read as machine-written |

**Kept from v1, unchanged:** checkable claims; the bell at nine as the turn; no fantasy diction; no text ever confirms another; proper English, never broken English; oddness earned by context and poke chains; specific numbers and times; `{name}` in every row and never "Jane"; no en or em dashes; the phrase budgets in `crates/jane-data/tests/voice.rs`; what a person points at is placed (§9).

---

## 1. Tone

**Silent Hill 2, written for Castle.** Quiet dread. A county where ordinary people say slightly wrong things, kindly, and leave most of it unsaid. Not horror: a town with a second town underneath it, and a bell that tells you which one you are in.

What we take from it, as technique and not as pastiche:

- **Evasive and specific at once.** People dodge the big thing and are exact about a small one. Mrs Fenn will not say what she thinks the bell is; she will tell you it went at ten to, because the kettle had not boiled.
- **They answer a different question.** Ask Mr Hale about his son and he tells you what the boy wanted to be called.
- **Grief and guilt under small talk.** The weather, the milk, the post, the paper. What is under it surfaces once, sideways, and goes back down.
- **Distinct damaged voices.** Each person has a habit that is also a symptom: the one who checks, the one who closes every subject, the one who repeats the news to be sure you heard, the one who speaks of the dead in the present tense. Habits are kept up across every visit, and vary.
- **The protagonist is a beat behind.** {name} sees the thing, and says the thing next to it.
- **Letters are written by someone with a reason to write.** A confession, an order, a note to a nurse who has not come. They can run long. They are personal, and they do not explain the town.
- **The uncanny is ordinary detail, slightly wrong.** Bread for forty, sold to nine. A cup of tea by the bed, cold. A date on a stone, next Sunday.
- **Silence carries weight.** Pauses, a sentence that stops, a door that does not open. Ellipses are rare and therefore heard.
- **Repetition with variation.** The same words come back changed: "in case" from Mr Dunn, then from the constable, then on a card in a window.
- **Nothing explains the town.** Not a sign, not a person, not the dog, not the diary.

What we leave: jump scares, gore, cruelty shown on screen, harm to children on the page, radio static as a gimmick, nonsense for its own sake. If a line is merely weird, cut it.

## 2. Core rules

**2.1 Quiet, not flat.** A line can be calm and still have a pulse. The pulse comes from rhythm (a long sentence, then a short one), from a person's habit, and from what they nearly say. Flat is when every sentence is the same length and says only what it says. If a line could be spoken by anybody, it is flat.

**2.2 Subtext over statement.** Say the thing next to the thing. Not "I miss him" but "I've put his tea on again". Not "the children never came back" but "I used to have six a week with a temperature, in the winter". The player completes it, and remembers it because they did. A line that states its own meaning is cut back to the part that does not.

**2.3 Specificity, and whose it is.** Every speaker's unease comes from their own life and trade: the baker's forty loaves, the postmistress's second post, the milkman's three bottles, the constable's paces, the smith's unused anvil. The county's shared signals (the bell, the lamps, the School's window) belong to people with a personal stake in them (Mrs Tace's husband went up that hill). Nobody mentions them as weather. Numbers, times, names, weights and days beat adjectives every time: "ten to nine on Tuesday" is frightening; "early, lately" is not.

**2.4 Length by character, not by rule.** Some people talk: Mrs Fenn, Mr Cobb, Mrs Wakes, Mr Tolly, the vicar. Some do not: Mrs Wick, Mr Pask, Mr Coker, the sweeper. The dog says as much as it chooses and no more. A written note runs as long as its writer needed. **The only hard limits are the screen's:** a spoken line about 160 characters at most (split a longer speech into the node's next line), four lines to a node as a habit, a readable note paged at the same size. Toasts and loading lines stay short because they are glanced at, not because short is good.

**2.5 How to earn unease.**
1. **Ordinary first.** The wrong thing needs an ordinary room to stand in. Most of the line is the person's day.
2. **One wrong thing at the centre.** Not a scatter of oddities. One, and it is theirs.
3. **Checkable.** "The lamps go out at ten." "It went at ten to." "I'm in the Arms from nine and I keep my eye on the street" (and the Arms' curtains are drawn after nine). A claim the player can test is worth ten atmospheres. Atmosphere is rationed: one line in five at most may be untestable mood.
4. **Withheld.** Leave out the sentence that would explain. If the player can infer it from what is placed, leave it out.
5. **Earned by order.** Oddness comes on the second or third visit, behind a question she chooses to ask, or at night. Never the first thing a stranger says (v1 rule 0, kept).
6. **It comes back changed.** A phrase or a fact heard once returns later from another mouth or another angle. Plan the return when you write the first.

**2.6 The blind test.** Cover the speaker's name. If you cannot tell who is talking from the line alone, rewrite it in their voice sheet's habit (§8). Two people never share a line, and no two adjacent people share a habit.

**2.7 Uncertain about itself, honest about it.** "Some nights." "Usually." "As far as I know." The writer does not have all the facts and does not pretend to.

**2.8 Never confirm.** No text says whether another text was true. The diary records what she did and saw and what she was told, never what it meant. Nothing in the UI says which omens are true. The dog will not say.

**2.9 Proper English.** Correct grammar and spelling, British spelling throughout. Contractions are normal speech. Dialect is a person's, not the county's (§3). Unease comes from what is said, never from how badly.

**2.10 No fantasy diction, no winks.** No "beware", "thou", "ancient evil", "dark forces", "adventurer", "quest", "dungeon" in the world's own words. Castle has a post office and a train timetable. Dry humour is fine (the dog is dry, the children are blunt); nothing acknowledges a game; no pop culture.

## 3. The county's English

- **Spelling:** British ("colour", "neighbour", "grey", "realise"). Times in speech are words ("ten to nine", "half six"); on notices and timetables, figures with a point ("22.00", "13.02").
- **Speech is rural English, lightly marked, by person.** The markers below are allowed, each to the people whose sheet lists it, so the county does not speak in one accent:
  - "I'll not", "I'd not": the older working people (the Farmer, Mr Hale, Mrs Fludd, Mr Rook, Miss Hanney).
  - "our Peter", "our Walter": family speech, for the families (Mrs Tace, Ernest Dunn, Mrs Pollard).
  - "love": the hatch, Miss Dray, Mrs Garland to strangers. "dear": Mrs Fenn alone.
  - "mind" as a tag ("Keep to the cobbles, mind"): the sweeper, the Farmer, Mrs Wakes. Nobody else, and no more than once a person.
- **The dog and Julie** use no contractions. Jane uses them. That difference is one of the tells (§7).
- **Nobody swears.** The strongest is "God help him", and it belongs to one person (the vicar would not say it; Mrs Garland might, once).

## 4. Registers

| Kind | Whose voice | Sounds like | Length | Example (new) |
| --- | --- | --- | --- | --- |
| **Dialogue** | The speaker's sheet (§8) | A person talking to a stranger, with something under it | By character (§2.4) | Mrs Wick: "Settled. The milk's been, the lamps'll be on at half six. I don't know what else you'd want me to say." |
| **Jane's inner voice** (examining a thing, a door, a bed, a sight) | {name}'s (§5) | First person, present tense, physical, a beat behind | One to three short lines | "A tin bowl by the step, licked so clean it shines. The label's in the same hand as my letter." |
| **The diary** (quest log) | {name}'s (§6) | First person, past tense, an evening's account | Entry: 2 to 5 sentences; a long one now and then | "Found Tilly's cat on the new stone in the churchyard, the one with a date and no name." |
| **Letters, notes, diaries found** | Their writer's, with a reason to write | Personal, confessional, addressed to someone, dated by weekday | Can run long; paged | The foreman: "If the office asks, he was never on." |
| **Official notices** (council, Company, constabulary, railway) | An office | Capitals for the heading, liability, "residents are advised", the wrong thing in the same register | Two to four sentences | "Persons seen on the unlit roads after nine who do not answer when spoken to should not be spoken to again." |
| **Shop and hand signs, cards in windows** | A tired, polite person, and later hands | Hand-lettered, apologetic, additions in other pens (Jane says so) | One card | "CLOSED FOR STOCKTAKING. OPEN MONDAY. SORRY." |
| **Carved stone** | A mason paid by the letter; later scratches by others | Capitals, few words, older grammar not needed; scratches in lower case | One line cut, one scratched | "FOUR TO KEEP THE CORNERS. READ THEM IN TURN." |
| **Books** (the library's last page, planting books, registers) | The author's trade voice, then Julie's margin | Instructional or ledger prose; the margin is the only personal thing | A page | "it will not come up in the dark. Do it by day." |
| **Item descriptions** | First sentence the game's (the rule, second person); second, optional, Jane's | One fact you can use; sometimes one you would rather not have | Two sentences, about 120 characters | "Opens Julie's front door. A little wet from the dog. The ring has room for two keys, and there's only this one on it." |
| **Toasts and system lines** | Nobody's: neutral narration | Present tense, impersonal, one physical detail, never cute, no "I" | Under about 90 characters | "Iron Knuckles comes apart at the rivets. All through the mine, the picks stop." |
| **Loading lines** | The county being got ready, as a worker would put it | A routine task in the passive or the third person; one in three slightly wrong | Under about 60 characters | "A kettle is filled for two and left on the stove." |
| **The dog** | §7 | Dry, formal, no contractions, impatient, answers a different question | Short to medium | "Rust, and lamp oil, and humbugs. You have been through his drawer." |
| **Julie's notes** | §7 | Lists, quantities, imperatives, signed J., one fact short | A card or half a page | "Gold dust, one vial of water, one pansy from the pantry chest." |

**Notes on the registers.**

- **Notices are where the register never changes when the subject does.** That is their horror. Keep the liability language exact ("at their own risk", "are asked not to", "until further notice"). The wrong thing is stated as policy.
- **Hand signs have history.** A card turned round so often its corners are soft; a word written over in two pens; a pencilled line under a painted one. Jane names the second hand; she does not say whose it is unless it is a hand she knows (Julie's, from the letter).
- **Carved stone does not explain itself.** Its later scratches may: Julie's "leave it there" under DO NOT. The scratch is in Julie's hand, and the text only ever says "the hand from the letter".
- **Books are long only where the book would be.** A ledger is columns; a planting book is dates; a time book is names and times. The one personal line is in the margin.
- **Toasts never use "I" or "you"** and never feel anything. They are what happened, physically: "The rope goes slack and stays slack."

## 5. Jane's inner voice

{name} is twenty, from the city, practical and tired, carrying a present for an aunt she has never met (`STORY.md` §5). She is not told what the county is. She notices things.

**Where it is used:** every examine line (a prop, a door, a stone, a bed, a fire), every narrated beat where she acts ("I put the letter through"), the endings' pages, and the diary (§6). Not toasts (§4).

**Rules.**

1. **First person, present tense, contractions.** "The kettle's warm." "I put my hand on it." The "I" is light: most lines are what she sees, and "I" appears when she does something or catches herself.
2. **Physical first.** Temperature, weight, smell, damp, what her hands are doing, what is missing. She registers the body before the meaning: "It's heavier than it looks, and cold right through."
3. **A beat behind.** She sees the thing and says the thing next to it. The implication is left one step away, for the player: "The date on it is next Sunday. I check what today is." Not: "The stone is waiting for someone."
4. **She counts.** Bottles, lamps, coats, names. It is how she holds on, and the county counts too. Use it, not every time.
5. **Grief she will not name.** Something in the city is behind her (§12, question 3). It never appears as a statement. It shows in what she looks at for a second too long: families waiting, a place laid at a table, a kept room. She does not write about home.
6. **Dry, not cute.** She can be wry once in a while, at herself, never at the county or the player. No jokes that wink.
7. **She does not know the county's words at first.** The first evening it is "a bell, a long way off"; later it is "the bell". The dog is "the dog", never "Julie", never "Auntie". Julie is "Julie", as she signed the letter; "my aunt" only when she has to explain herself to someone.
8. **Never** "I have a bad feeling", "something's wrong", "creepy", "eerie", "ominous", "I shiver", "a chill runs down my spine", "I can't shake the feeling". If the line needs one of these, the detail before it was not good enough.

**Before and after (her voice):**

| Before (v1) | After (v2) |
| --- | --- |
| "Straw, still warm. Three eggs in a row, and a card propped behind them: MRS BEX. The writing is small and neat." | "Straw, still warm. Three eggs in a row, and a card propped behind them: MRS BEX. Small, neat writing. I've seen her hand on her list. It's the same." |
| "You sleep. Nothing wakes you. When you open your eyes the blanket has been pulled up." | "I sleep. Nothing wakes me. When I open my eyes the blanket's been pulled up to my chin, the way nobody's done since I was small." |

## 6. The diary (the quest log)

The quest log is {name}'s diary. She writes it in the evening, or when she sits down, about what she did that day.

**Which fields are the diary:**
- `description`: the entry she wrote when she took the thing on. What she was asked, by whom, where, and what she noticed about the asking.
- `completion`: the entry she wrote when it was done. What she did, what she saw, what was said.
- **Not** `requirements[].text` or `returnTo`. Those are the margin: plain place labels ("Sixpence, in the churchyard behind the church"), unchanged in register, because the player reads them to find their way.

**Rules.**

1. **First person, past tense, contractions.** "Tilly asked me to look for her cat." The present tense is allowed for what is still true when she writes: "He's in the churchyard, she thinks."
2. **What she did and noticed, never what it meant.** v1's rule 5 kept: the diary does not confirm, conclude or foreshadow. It can record what someone said, accurately, in reported speech or a short quote.
3. **Directions are kept.** An entry still tells the player where to go, in landmarks ("up the farm track", "at the end of the mine road out of Castle"), because the Lost player rebuilds their objective from it (`EXPERIENCE.md` §5). Write them as she would remember them.
4. **One personal aside at most, and not in every entry.** A thing she noticed about herself or the person, a beat behind: "I read every name. I don't know why." Most entries have none.
5. **Dated by the journal, not by the row.** The journal already keeps a day line ("Day 3, Tuesday. Rested at the Reedcutters' fire.", `WORLD.md` §9.2). The heading over an entry is stamped by the journal when the entry is written (day, weekday, and the hour where the sim has it). Rows never write a date or a day number themselves, so they stay true whenever she does the thing.
6. **{name} and co-op.** The diary has one writer, "I", the county's heroine, whichever seat reads it. It never says "we", never names a seat, and never says who of two did a thing. `{name}` appears in the diary only inside someone's quoted words.
7. **No closers.** An entry ends on the last thing she did or saw. Never a summary, a moral or a line that tells the reader how to feel.
8. **Length:** a description about 2 to 5 sentences (under about 450 characters); a completion 1 to 4 (under about 350). A spine completion can run longer: it is the night she writes the most.

**The spine's lines that other files quote.** `STORY.md` and `EXPERIENCE.md` quote some completions word for word (End of Term's, Yours to Say's). When the diary rewrite reaches them, the quotes are updated in the same commit.

## 7. The three who are not townsfolk

### The dog

What is left of Julie (`STORY.md` §3). Nobody is told. The writing must make it findable, never said.

- **Voice:** dry, formal, patient and a little impatient with her. No contractions. Sentences that stop when the information stops.
- **Answers a different question.** Asked where Julie is, it tells her where Julie is not. Asked "Who sent them?", the slip.
- **Smell first.** It knows where she has been before she says: "You smell of hot wire." Make the smell specific and true to the place (humbugs from the Headmaster's drawer, plaster from the painted wing).
- **Julie leaks through habits, not facts.** It tells her to eat, to sit down to it, to keep the potion by the door. It says things twice, the way an aunt does. It never says "I" or "me" about anything Julie did, and never "she" about anything it could only know by having been her, except the two slips (`STORY.md` §3), which are frozen.
- **Never reassuring, never cute.** It does not say "good girl" or "don't worry". Its kindness is practical: "There is a bed. Sleep in it."
- **Never seen after nine.** No line explains it.
- **Avoids:** her name for itself, Goldskin before she has seen him, what the Ball will do before the School.
- **Sample:** "Rust, and lamp oil, and humbugs. You have been through his drawer."

### Julie (handwriting only)

- **Who:** a kind, gentle witch who held the shield until it used her up, and went ahead of {name} leaving what she would need where she would need it.
- **Wants:** {name} to get there, prepared. **Avoids:** why; herself; the dog; anything that would frighten her before she has to know it.
- **Voice:** lists, quantities, imperatives, times ("before the bell, not after"), signed **J.** No contractions. Addresses `{name}` by name at the top of a note and nowhere else.
- **Always one fact short.** She tells you how, never why.
- **One warm line per note at most,** and it is practical in form: "I am sorry there is nobody to show you." Never "love", never "dear", never "I miss".
- **Later notes are shorter.** Her time was running out; her pages along the spine get barer (the Shot-Firer's glove: four instructions and "Keep it dry"). Jane may notice the hand changing; the text never says why.
- **Sample:** "Stone, water, white rose: Stone Skin. Drink it when you cannot get away. Keep one by the door. J."

### Jane

Her sheet is §5 and §6.
- **Wants:** to arrive, give the present, be told something. **Avoids:** home; what she felt in the tunnel; the word "witch" about herself.
- **Wound:** a loss behind her in the city, never named (§12, question 3).
- **Habits:** counts; touches things to check them; says the thing next to the thing.
- **Sample:** "The date on it is next Sunday. I check what today is."

## 8. Voice sheets

One row per recurring named person in the data. **Want** is what they are after in conversation; **avoids** is what they talk around; **habit** is what you should hear in every line. Their one true thing and one false thing are `STORY.md` §5.1; the sheet is how they say them.

### 8.1 The mine, the School and the Company

| Who | Who they are | Wants | Avoids | Wound | Vocabulary, rhythm, habits | Sample |
| --- | --- | --- | --- | --- | --- | --- |
| **The Headmaster** | Still taking the register in the mine (`units.json` `headmaster`) | Order; every child accounted for | That none of them are there | He did it because the bell said in | A schoolmaster's address: full sentences, never raised, "Quietly, please", names by surname and initial, rules stated as kindness. Speaks to her as a pupil who is late. If he is ever given spoken lines (§12 q. 9), he takes the register | "HALE, R.? Present. Thank you. Quietly, please. We are not going up until we are told." |
| **Amos Noone, the Timekeeper** | Rang the county in and out | The shift kept | Nothing: he does not speak | None he would name | Writing only: times in figures, "9 o'c.", "no time out", a signature, A. NOONE, TIMEKEEPER. No adjectives, ever | "JULIE, 9 o'c., the night of the bell. No time out." |
| **Gnox Goldskin, the Company** | Never seen until the Burial; known by notices and plates | Ownership | That the county was ever not his | None | The Company's voice: capitals, "the Company", "BENEFACTOR", "by order", proprietorial possessives, gifts announced as policy. Under it, workers' later hands in pencil | "GOLDSKIN MINING Co. IN CASE OF FIRE assemble in the SHAFT HALL. Do not use the cage. The cage is not in use." |
| **The foreman** (his diary, Factory and Sidings) | Ran the Works' lines | To be told he did right | That he took the lamps out because the dark made it quieter | He took a man's card out of the rack | Diary entries by weekday: "Had the lamps out...", clipped, managerial, then a confession in one sentence; later only sums | "If the office asks, he was never on." |
| **Iron Knuckles** | The pit boss; rust and habit | | | | No lines. Toasts only, physical | "Iron Knuckles comes apart at the rivets." |

### 8.2 Castle: the square, the Arms, the lanes

| Who | Who they are | Wants | Avoids | Wound | Vocabulary, rhythm, habits | Sample |
| --- | --- | --- | --- | --- | --- | --- |
| **Mrs Fenn** | Neighbour on the square | Somebody to agree it went early | What she thinks the bell is | Widowed, unsaid (her clock was a wedding present) | Runs on. Gives the evidence, then the evidence for the evidence. Kitchen objects (kettle, clock). "dear" (hers alone). Ends by putting something back where it was | "I had the clock down off the wall after, and the back off it. There's nothing wrong with it. It was a wedding present. I've put it back up." |
| **Mrs Wick** | Neighbour; the town's denial | To end the conversation | Any question | None she would admit | Short, final, defensive. Never asks a question. Lists the routine as proof. "What else is there to know?" (once, budgeted) | "Settled. The milk's been, the lamps'll be on at half six. I don't know what else you'd want me to say." |
| **Mr Cobb** | Sits outside the Arms by day, inside from nine | To be the one who saw it | That he sees nothing after nine (the curtains are drawn) | None; he is the town's eyes and proud of it | Long, pleased sentences. Tallies who came up the street and when. Repeats news to be sure you heard. "I like to see who comes up the street" | "Nothing happens on this street after the bell. I'd have seen it. I'm in the Arms from nine, and I keep my eye on the street all night, same as out here." |
| **Mrs Garland** | Landlady of the Castle Arms | The bar kept, the door locked, nobody fussing | The back room's silence | She leaves Ernest's tray every night | Brisk, sharp with gossip, turns a sentence back on the asker ("Now I'm telling you"). Tells you what she tells herself. "love" to strangers | "I leave his tray at the door, and it comes back empty. Tell Walter he's all right. That's what I tell myself." |
| **Walter Dunn** (Mr Dunn) | Waits by the telephone box | His brother | That Ernest has been in a public house since the spring | Ernest, the night of the bell | Chapel-proper, a little stiff, defends Ernest's character before anyone accuses it. Always the time: "Is that the time?", "on the hour". Waits "in case" | "He's chapel, same as me. He's never set foot in a public house in his life, and I'll thank you not to say otherwise." |
| **Ernest Dunn** | In the back room since the night of the bell; heard only on the telephone | Walter to come before the lamps | Time: he does not know how long it has been | | Urgent, a long way off, family speech ("our Walter"). Never more than two sentences; the line goes | "Tell our Walter I'm at the Arms. Tell him to come before the lamps." |
| **Mr Hale** | Smith at the Forge on Cross Lane | To keep Robert upstairs | Reading the name; the past tense | Robert, HALE, R., the last on the memorial | Few words, hands and metal ("keeps its heat"). Corrects himself back into the present tense. Answers a question about Robert with what Robert wanted, or wore | "He wanted Bob, at school, and I wouldn't have it. Robert, I said. It's a good name." |
| **Mrs Tace** | On her step, later at the top of Church Lane | Her husband and Peter home | That it has been since the spring | Both of them | Future tense, patient, exact about times and the latch. "our Peter". Never says "if" | "They'll be back before the lamps. I've left the door on the latch, so he needn't knock." |
| **Mr Tolly** | On the bench by the memorial | Company on the bench | The newest name; Mrs Hobb's kindness | He "never heard he'd gone" | Old man's meander, "near enough", dates by what was built when. Talks to the bench as much as to her | "I've sat on this bench since the memorial went up, near enough. Before that I sat on the wall, and before that I was working." |
| **Mrs Hobb** | Orchard behind the house on Back Lane | Something to do with her hands | Why Mr Tolly stopped picking | | Hands and watering; short practical offers; lets other people be stubborn ("Let him") | "I water them morning and evening. They don't need it. It's something to do with my hands." |
| **Miss Dray** | Postmistress | The post to go where it is addressed | That nobody here gets post | | Procedural and cheerful: times, rates, "second post at four". Sorting language for everything. "love" | "There's always post on Sunday. The sacks are never for anyone here. I sort them by street anyway." |
| **The Milkman** | The round, seven to nine | The round done | No. 7 | | Numbers of bottles, doors, minutes. "on a round". Brisk, then one sentence too many | "No. 7 hasn't taken theirs in all week. Three bottles on the step. You don't stop for one house, though." |
| **The constable** | Walks the High Street end to end | The book kept | What is on the roads after nine | That the street is longer than it was | Officialese softened by a decent man: "the book", "nothing to report", paces counted. Writes things down he cannot explain | "I've put it in the book. I didn't know what else to put." |
| **The sweeper** | Sweeps the square twice a day | The square clean | Looking | | Laconic; two-clause sentences; leaves in the morning, salt in the evening. "mind" | "I've never seen anything on the cobbles. I've never looked, mind." |
| **The vicar** | The church; evensong at six | Somebody to come in | Goldskin's name; the bell at nine | He buried Goldskin and wrote it in the book | Educated, gentle, careful. Says a true thing that sounds like a pun and does not hear it ("I've stopped answering it"). Keeps the book | "I ring for evensong at six. Something else rings at nine. I've stopped answering it." |
| **Miss Orme and Mr Lyle** | Sit where they can see the Doctor's | Each other's company | That they are courting | | A duet that contradicts itself. She keeps count and corrects; he is sure and wrong. Each quotes the other | Mr Lyle: "We're not courting. We're sitting." |
| **Dr Vane** | The Doctor's, Pound Lane | Somebody ill | The list, until she brings it up | He signed them fit and gave Julie the list | Clinical exactness ("signed fit", "a temperature"). Admits shame plainly, once. Never pretends | "I sit out here in case. If somebody came up the lane coughing, I think I'd run to meet them." |
| **Mr Ince** | Reads Sunday's paper all week | It to be in the paper | That the paper never changes | | Page numbers, cuttings, "Sunday's". Treats the paper as the only record | "It'll be on page four, if it's anywhere." |
| **Mrs Oddie** | Baker | The bread taken | The thirty-one loaves | | Quantities: bakes for forty, sells nine. Practical generosity, "It's only bread" | "I bake for forty. I've always baked for forty, and I sell about nine." |
| **Mrs Crewe** | Seed stall | A fair swap | What Julie wanted the lot for | | Trader's terms ("two for one", "I'll swap"). Discretion as a virtue: "You didn't, with Julie" | "Julie used to have the lot off me. She never said what for, and I never asked. You didn't, with Julie." |
| **Mrs Marsh** | Her washing line, No. 5 High Street | Her washing back | The Arms yard | | Housework, weather, a mother's habits she used to laugh at | "I bring it in before the lamps now. Mother always did, and I used to think she was being silly." |
| **Mrs Bex** | Waits for eggs | To be out of the house | Her own handwriting on a card | | Short, a little lost, buys too much | "That's my writing. I don't remember writing it." |
| **Mr Pound** | Fruit stall | Trade | Why people buy for one day | | Market patter, then candour | "People buy for one day at a time now. I do it myself." |
| **Mr Sallis** | Edges and roses | His wife's grave kept | Going in | Edith | Gardener's talk; "I might" | "I might go in and see her on Sunday. I might." |
| **Mr Quill, Mr Ennis** | Regulars in the Arms | To stay in | Going out | Mr Ennis missed a train | One line each, a loop | Mr Quill: "Mrs Garland keeps the curtains drawn, so I go by how many I've had." |
| **A Woman with a Case** | Waiting since Sunday | The Sunday train | That it has not been Sunday since | | Polite, packed, questions she has asked before | "I have been here since Sunday. It has not been Sunday since." |
| **The children** (Tilly, Robin, Nell, Dot) | Castle's children | To play, to count, a lost cat | Nothing: they have not learned to | | Blunt. Town rules as games ("We're playing lamps"). "Mum says". They say aloud what the adults will not, without knowing it is hard | Dot: "Four lanes. I don't count that one out loud. You can, if you like. You're not from here." |

### 8.3 The Lowfields, the Waters, the Works, the roads

| Who | Who they are | Wants | Avoids | Habit | Sample |
| --- | --- | --- | --- | --- | --- |
| **The Farmer** (Lowfield Farm) | Three scarecrows, a back kitchen | Somebody else to count | The front of the house, which faces the School | Slow, counting, weather; "I'd not ask a stranger" | "You'd tell me if it was four." |
| **Mrs Allen** | Behind her door, waiting for the nurse | The nurse on Tuesday | Opening the door | Polite formulas through a door; "my sister says" | "Nurse will be along on Tuesday, I expect. She always is." |
| **The nurse, E.M.** | Never met: cards, initials, a bag | | | Initials in registers, "NURSE CALLED. TUESDAY." | |
| **Mrs Fludd** | Keeps the reedcutters' fire | Somebody to come up to the fire | The far bank | Sees everybody; speaks for the silent men; "I keep the fire, that's all" | "I'd have seen. I see everybody." |
| **Mrs Loveday** | Rings her husband in at the landing | Company down to the water | That he has not come | Ringing, calling, small domestic memories in the past tense | "I'd shout him in at five and he'd come up grumbling." |
| **Mr Pask** | Watchman, Hut Two | The watch kept | What the lodge book says | Clipped watchman's speech, "six to six", the book | "Nobody's come to take it off me, so I keep it." |
| **Mr Breen** | Watchman, Hut One | Somebody to say good evening to | The cold | Lonely hospitality | "It's a long night in a hut with nobody to say good evening to." |
| **The hatch** (the canteen) | A woman behind a shutter | The dinners eaten | Who they are for | "love", dinners and tins, "I only read the rota" | "They go back on the shelf at one and I do five again in the morning." |
| **Miss Hanney** | Tells the bees | It done plainly | Sentiment | Blunt, practical, first sentence an assessment of you | "You're not family. There isn't any. You'll do." |
| **Mrs Wakes** | Knits a row for everyone at the door | Your name | Pulling a row | Rambling, warm, instructions mixed with gossip, "mind" | "I knit a row for everybody who comes up to the door. Always have." |
| **Mr Rook, Mary Rook** | A boarded cottage | Mary kept in / out | What she came back as | He repeats "Don't knock"; she is flat and final | "Don't knock. Whatever you do." |
| **Mrs Pollard, Mr Pollard** | Plates on a stone | Him home | Tom, at the School | She: curlers, plates, "our Tom"; he: "Don't mind me" | "He never would eat what they gave him." |
| **Mr Pargeter** | Horace the tortoise | Horace kept for winter | North | Fond grumbling at an animal | "Horace. You old fool." |
| **Mr Coker** | Walls with headstones | Three more stones | What they are | Clipped to the bone: "Coker. I wall." | "Face down, they're only stone." |
| **Mr Denholm** | Company diver in his helmet | His spanner, his bolts | The knocking at the grille | Muffled, matter-of-fact, Company rules | "There's a good deal of glass between us." |
| **Mr Sorrell** | Vermin man | Paid by the head | What lives under the hatch | Trade talk, bargains, "I'd not. But you're not me." | "I'll split it with you, and I'll not say which half." |
| **Mr and Mrs Voysey** | The Halt's lamps | He: red. She: green | Each other | He keeps lamps; she keeps a packed case | "It hasn't stopped here in eleven years. Not once." |
| **Country folk** (the Farmer, a Woman in an Apron, an Old Man, a Woman in a Brown Coat, the Woodcutter, a Reedcutter, the Innkeeper, a Man in a Grey Coat) | Generated; one tree serves many | Their work done before dark | The School | Each tree turns its lines over (`said:farmer` 0 to 4); each archetype keeps its own trade's words | Reedcutter: "I lost a boot off the end of this landing in May, and I've cut here twenty years." |
| **The 27 story households** (`data/dialogue/stories.json`) | One household, one small errand chain | Their own small thing | Their own small grief | Each is given one habit from the list in §8.4, never the same as a neighbour on the same road | Mrs Vosper: "I've read them all now, in order. I know when he stopped writing. I'm still waiting for Sunday." |

### 8.4 Habits to hand out

When a new person is written, give them one habit from here (or a new one) that nobody within a screen of them has: checks and re-checks; closes every subject; repeats news; speaks of the gone in the present tense; future tense only; answers with a fact about the asker; counts; talks to an animal instead of you; quotes someone else's opinion as their own; corrects themselves; apologises before anything is wrong; asks a question they already know the answer to; describes a routine in great detail and skips one step.

## 9. Truth (unchanged in substance from v1)

- **What they point at is there.** A named object near a speaker is placed near them. Open the chunk or the generator and look before writing.
- **Out in the country, only what is guaranteed.** Country folk are generated; one tree serves every farmer on the map. A line may name only what that kind of place always has, or what is true everywhere: the bell at nine, the School on its hill (north, in the Lowfields), the lamps at dusk, the creatures off the road after dark, the Sunday train.

  | Speaker | Stands at | Always there |
  | --- | --- | --- |
  | The Farmer | a farmstead, sometimes an orchard (Lowfields) | fences, the field or trees; at a farmstead a chained barn with a card, a pump, a hen house, a sheep pen |
  | A Woman in an Apron | a farmstead, a hamlet or a cottage | a house, a garden or field, the road |
  | An Old Man, A Woman in a Brown Coat | a hamlet or a cottage | a house, a garden plot, the road |
  | A Man in a Grey Coat | a hamlet, a cottage or an inn | the road |
  | The Innkeeper | an inn | THE HALFWAY HOUSE sign, the lamp by the door, a trough, a cart, barrels |
  | The Woodcutter | a clearing (Lowfields) | stumps, logs, woodpiles, a shed or a tent |
  | A Reedcutter | a reed hut (the Waters) | the hut, the cut reed stacked, a plank landing, wet ground |

- **Say different things across visits.** A person with more than one thing to say says it over several visits (a counter flag: `talked_x` 1, 2, ...); country trees turn over their lines (`said:farmer` 0 to 4).
- **A small claim about the town is true.** The false thing a person holds is their own, not a fact about the street (`STORY.md` §5.1).
- **Omens** are a text plus a consequence (`PLAN.md` §5). Template, unchanged:

```
where     a sign on the east road, first lamp post past the farm
text      "STREET LIGHTING. These lamps are switched off at 22.00 to save the county money.
           Residents are advised that this has always been the case."
claim     the lamps on this road go out at ten
if true   at 22:00 every lamp on this road segment goes dark; night spawns apply
if false  the lamps burn all night; the sign is just a sign
fair      the road is still walkable; the next lit hub is under two minutes away
tell      none. Optional: a townsperson who says "they never used to"
```

## 10. Banned habits

These read as machine-written, and most of v1's flatness came from them. A reviewer cuts on sight.

1. **Aphorisms and fortune-cookie closers.** "Habit is a kind of bell." "Everything here keeps." A last line that sums up, moralises or tells you how to feel.
2. **Triads.** Three parallel items for rhythm's sake ("She left fruit. She left a bench. She left a note."). Lists of three are fine when there are three things (three scarecrows); never as a cadence.
3. **Not X but Y.** "It isn't the drink. It's that...", "late, not lost", "I'm not saying... I'm saying...". Say Y.
4. **Tidy symmetry.** Balanced pairs, chiasmus, "we are both right".
5. **Abstract nouns as atmosphere.** "a stillness", "an absence", "the weight of it", "something old", "a wrongness". Name a thing.
6. **Uniform cadence.** Every sentence 8 to 12 words, full stop, next. Vary: a long sentence, a fragment, an interruption.
7. **Dash tics.** No en or em dashes at all (a test holds it). Do not fake them with commas or " - ".
8. **Over-ellipsis.** At most one "..." in a node, for a real pause or a sentence that stops. Never "...!" or "?...". The dog's silent poke is the one place a line is only "...".
9. **"As if it had been waiting"** and its family ("as though it knew", "as if expecting you"). Objects do not wait.
10. **Telling the feeling.** "unsettling", "eerie", "uneasy", "a chill", "somehow", "strangely".
11. **The echo-closer.** Repeating the line's key word at the end for effect ("He's upstairs. Upstairs.").
12. **Everyone talks about the bell and the lamps.** They belong to people with a stake (§2.3).
13. **Rhetorical questions to the player** ("Who could have done this?").
14. **Over-explaining a joke or a horror.** If the line works, stop.
15. **The phrase budgets.** "all the same", "never once", "No bell last night.", "halfway between", "thirty years": see `voice.rs`. Raise one only by cutting a use elsewhere.

## 11. Redlines

John's corrections, dated, as rules.

- **2026-09-21** "I have already draw a map" is not eerie, it is poor English. → §2.9.
- **2026-09-21** "I am not a city dog" is random where it was. Rework it, or make it something found by clicking a few times. → §2.5.5; the poke chain in `dialogue.json` (`dog.poke_1` to `poke_loop`).
- **2026-09-21** "We're almost getting a slight Silent Hill vibe, and I love it. Lean into that a little." → §1.
- **2026-09-21** Every `{name}`: no row may have "Jane" baked in (a test enforces it).
- **2026-09-23** "Many would be better feeling uneasy with the situation like the player may feel and pointing out something they've noticed (that is probably real in game) ... mostly in more normal talk tone/voice but some can be a little off." → §2.3, §8, §9.
- **2026-10-09** The text is "pretty average": flat and terse; everyone sounds alike; the mood does not land. Target Silent Hill 2. Jane gets an inner voice; the quest log becomes her first-person diary; every sign, notice, book and note is in scope. → this v2.

## 12. Open questions

Asked in `progress/2026-10-09_65_voice/samples.md` §Questions, each with a recommended default; the guide is written to the defaults until answered.

1. Examine lines in Jane's first person (default: yes, present tense).
2. Jane's age and relation to Julie, as felt in her voice (default: twenty, Julie's niece who never met her).
3. Jane's past: how much the diary may reveal (default: a loss in the city, never named).
4. Spelling and dialect (default: British spelling; dialect by person, §3).
5. How dark the letters may go (default: grief, guilt, fear and death yes; harm to children and self-harm never on the page).
6. The diary's dates (default: stamped by the journal, never written in the row).
7. Co-op: "I" or "we" in the diary (default: "I").
8. The screen's limits for a line and a node (default: about 160 characters, four lines).
9. Whether the Headmaster speaks (default: three or four lines in his fight, his sheet's voice).
10. Whether the dog's voice drifts toward Julie's over the game (default: very slightly, in habits only).
11. Which quoted lines are frozen (default: the endings, the two slips, "She is not dead, {name}. I would know.", the Hoar Stone and the Company's mine sign).
