# The Roman campaign: design draft

*Status: implemented. Ten chapters (`ROME` in `crates/rr-game/src/missions.rs`), their holy places on the maps, the gods' favour and the beats (`crates/rr-game/src/story.rs`), opened one by one from the menu. What follows stays the place where the design is argued out; the decisions so far are listed below.*

## Decided (2026-10-27)

- Ten chapters; the gods are real but small; easy skill maps first, then harder.
- Each god is served in its own way (eight verbs), from a **holy place** on the map.
- Holy places can't be built, attacked, destroyed or burned.
- **Favour belongs to the player.** The holy place is only the channel.
- **You may use a holy place only while your land lies fully around it.** If the border runs
  through it, nobody can use it.
- When a holy place changes hands, its flag is recreated for the new holder, without a road.
- Concordia, Juno of Veii and Cybele keep their special rules.
- **The AI uses the gods too**, mostly through ordinary deliveries; Janus needs real AI logic.
- Omens are honest: sent when the real event starts, never guessed.
- Holy places are permanent obstacles, placed only on reachable land.
- Holy places are optional map data, so old maps stay compatible: most maps simply have none.
- Free games get a "holy places" option, after the campaign.
- **One god per chapter**, no choice from a pantheon.
- **Mars is served by drilling**, not by victories.
- **Medals** per chapter: bronze, silver, gold (see "Medals" below).
- **No carry-over**: every chapter stands on its own, as in the world campaign.
- **Rivals differ by colour**, using the peoples the game already has: Gauls look like the
  Vikings, Carthage and Numidia like the Nubians, as Carthage already does in Mare Nostrum.
  Etruscans, Sabines, Samnites and Greeks are Romans to the eye, in their own colours.
- **Real people appear as characters**, and the story stays plausible (see "The cast").
- **Chapter 2 is told as A with C**: the women end the war, Tarpeia is the twist.
- **Chapter 8 warns before Cannae** with one omen; Cannae happens by the clock either way.
- **Keep it simple**: no beat or goal can get stuck (see the rules below).
- A holy place never keeps a player alive, isn't a building in the statistics, and can't be a
  catapult or attack target.

A second campaign beside the world campaign: ten chapters of real Roman history, from the founding
of the city (753 BC) to Augustus closing the gates of Janus (29 BC). The gods are real, but small:
they lean in, they never win a battle for you.

The early chapters are small, gentle maps, and each teaches **one skill**. The later chapters are
large and hard, and ask for all of them at once.

## Ground rules

- **Our own story.** The 1996 original's campaign is also Roman: castaway legionaries opening gates
  from map to map. We use none of that: no shipwreck, no gates, no island hopping. Our chapters
  follow history as the Romans themselves told it (Livy, Polybius, Plutarch), legends included.
- **The gods are real but small.** A god's touch is worth about one good building, never a
  battle. Every divine effect has an ordinary counterpart in the economy, so a player who
  ignores the god can still win every chapter, only less comfortably.
- **Honest omens.** An omen always tells the truth about something the game will really do (an
  attack is planned, a vein is rich, a storm will close the strait). It may be vague, never false.
- **Deterministic.** Favour, omens and divine effects run in the simulation with its own random
  numbers, so saves, replays and LAN games stay exact.
- **One god per chapter, each in its own way.** Each chapter has one patron god, and every god
  is served differently: one is watched, one given gifts, one lured, one guarded, one switched,
  one drilled for, one given offerings, one carried home. No two chapters play alike.
- **The gods live in places, not buildings.** Their holy places are part of the map: you don't
  build them, and nothing can destroy them. They belong to whoever holds the land.

## Keep it simple: nothing may get stuck

A chapter that can't be won any more because one condition failed is the worst bug a campaign
can have. So every chapter follows these rules:

- **One goal per chapter**, from the kinds the world campaign already has: take a castle, hold
  land, hold out for a time, defeat a rival. At most one new kind in the whole campaign
  (connect by road, chapter 5).
- **Story beats fire on time**, or on one simple event that will happen anyway (a capture, an
  attack). Never on a combination of conditions.
- **The gods never gate anything.** Favour can make something come *earlier* or *easier*,
  never decide *whether* it comes.
- **A beat that acts on a building picks it at that moment**, for example "your military
  building nearest to the Sabines". If there's none, the beat is skipped, not stuck.
- **No unique items.** Nothing the goal depends on can be lost, carried off or destroyed.
- **The AI plays every chapter through** in the tests: on several seeds, as the player, to the
  end. A chapter that the AI can't finish isn't done.

## The gods as a system

### Holy places

Every chapter's map has its god's **holy place** already standing: a hilltop, a grove, a shrine on a
cape, a temple in a city, a field by the river. It is a fixed map object, like a harbour spot.

- **It can't be built, destroyed, burned or torn down.** No attack, catapult or lost border
  harms it. When the border moves, ordinary buildings on lost land burn, but a holy place
  simply **changes hands**. The Romans took enemy gods over rather than burning their temples
  (*evocatio*), and so do we.
- **It serves whoever holds the land fully around it**: the place itself and the ring of nodes
  around it must all be inside one player's border. If the border runs through it, nobody can use
  it until one side pushes the other back. Then the god's help goes to the new holder.
- **It has its own flag**, so carriers can bring what the god asks for (fish, grain, the stone
  of Cybele).
- **It's a goal on the map.** "Push your border to the grove on the hill" is clearer than
  another building in the menu. The AI wants it too: it is a military target, and taking it
  hurts the rival twice.
- **Map makers can use it.** The editor places holy places like harbour spots, so they work in
  free games and custom maps too.

### Eight gods, eight ways

Each god has one verb: what you do for it. It also has its own holy place, and one small way of
helping.

| God | Chapters | Verb | Holy place | How it works | What it gives (small) |
|---|---|---|---|---|---|
| **Jupiter** | 1 | *watch* | The highest of the seven hills | No offerings. Whoever holds the hill sees from it: the augur reads the birds there | Far sight from the hill, and the first omens |
| **Concordia** | 2 | *unite* | A shrine in the valley between the two realms | Where both lands reach her shrine, the women carry gifts across: her favour grows | At 50 favour, one battle fewer brings the peace |
| **Juno of Veii** | 3 | *lure* | Her altar on your side of the valley, facing Veii | Offerings of bread call her out of the enemy's city | At 50 and 80 favour, three of your waiting soldiers each rise a rank |
| **Juno's geese** | 4 | *guard* | Juno's temple on the Capitol, inside your first land | Feed the sacred geese grain (an offering, as at Veii) | At 50 favour the geese wake Manlius: five new privates join the defence |
| **Janus** | 5, 10 | *switch* | His gates in your city | You open or close the gates. Open means war: soldiers are recruited faster. Closed means peace: workshops work faster, but you can't attack. Switching takes a few minutes | A choice to make, not a bonus |
| **Mars** | 6, 9 | *drill* | The Field of Mars by the river | Soldiers from your warehouses drill on his field. Every finished drill raises favour | Drilled soldiers are promoted without coins |
| **Neptune** | 7 | *offer* | A shrine on a cape across the strait (Carthage's coast) | Fish at the altar | Faster ships, and once a storm that keeps Carthage's fleet in port |
| **Cybele** | 8 | *receive* | Her temple on the Palatine | Her stone arrives by the story at minute 35; from then on her temple serves its holder | Fields and mills work better, for good |

The god fits the chapter's skill each time: Jupiter goes with land, Concordia with diplomacy, Juno
of Veii with mines, the geese with defence, Janus with economy versus war, Mars with soldier
quality, Neptune with the sea, and Cybele with transport.

The artwork for the ten holy places (`assets/art/holy/`, made like the Roman buildings):

![The holy places](images/holy_places.jpg)

### How strong the help is

Where a god has favour (Concordia, Juno of Veii, the geese, Mars, Neptune), it fills from 0 to 100
and slowly fades, roughly 1 point a minute. The help comes in three steps (25, 50 and 75), and is
never stronger than this:

- a small production bonus in one chain (about +10–15 %);
- an earlier or clearer warning;
- one single event in the chapter: a storm, a vein, a peace offer.

There are no lightning bolts and no spawned armies. Gods only ever **help** their holder; ignoring
them is never punished, it just gives less. A god whose place the rival holds helps the rival
the same way, which is why it's worth taking.

**Example: Neptune in chapter 7.** A carrier brings fish to the cape shrine, which holds up to 3.
Every 60–90 seconds one is burned at the altar: smoke rises, a short sound plays, and favour goes
up by about 4. One fishery feeding the shrine makes favour climb slowly, two quickly.

| Favour | Neptune's touch |
|---|---|
| 25 | Omens tell where Carthage's fleet sails ("dolphins follow the ships to Panormus") |
| 50 | Your ships sail about 15 % faster: expeditions and sea attacks arrive sooner |
| 75 | Once in the chapter, a storm keeps Carthage's ships in port for about 5 minutes, announced beforehand: your window to cross |

Every fish on the altar is one no miner eats. The real game is that choice: more swords, or the
god's help with the crossing. The shrine only orders what its god likes, so nothing is wasted by
mistake.

### Implications of holy places

What "a fixed place, indestructible, owned by whoever holds the land" means in our engine, and
what has to be decided before building it.

**1. Whose is the favour: the player's or the place's?**
If favour belongs to the *place*, a conqueror takes over everything the rival built up. That fits
*evocatio*, but it's an exploit: let the AI fill the shrine, then take it. If favour belongs to the
*player*, holding the place is the channel: only the holder can raise favour and gets the help;
a lost place lets your favour fade as usual, and winning it back resumes where you were.
**Proposal: favour belongs to the player; the place is the channel.**

**2. A holy place holds no land, and serves only a holder fully around it. (Decided.)**
If it held land it would be a military building with a garrison. It doesn't: land around it is
owned by the military buildings nearby (RTTR's nearest-building rule). You may use it only while
the place **and every node of the ring around it** (radius 2, so no border line can run through
it) is your land, away from the border line. You never attack a holy place: you take the posts
that hold its land, until your border closes around it. Consequence: the AI must learn that posts near a holy place are worth more, as
targets and as posts to keep manned. That is one value in its target scoring, not a new system.

**3. Changing hands touches the road net.**
When a border moves, our sim destroys the loser's flags and roads on lost land (RTTR
DestroyPlayerRests). The holy place itself survives, but its flag must be **recreated for the
new holder**, unconnected. Wares on the way to it are lost like any others on lost land. The new
holder must build a road to it before anything can be delivered: one more step, and a fair one.

**4. A place on the border line belongs to nobody.**
With the "fully around" rule, a shrine between two posts doesn't flip with every capture: while
the border cuts through its ring, nobody can use it, and the help pauses for both sides. That is
calmer than flipping back and forth and makes the goal clear: push the border past it. Messages
come only when someone gains or loses the use (one each), and the place's window shows its state:
"yours", "theirs" or "contested".

The pause matters for the gods' rules: favour doesn't rise while contested, but it fades as usual,
so a long fight over a shrine costs both sides.

**5. One god breaks the simple rule on purpose.**
*Concordia* stands "half in two lands", the opposite of "fully around". She takes gifts while
both players' land lies within a few nodes of her, at war or at peace (in chapter 2 the gifts
come from the women during the war). That's a radius rule, not ownership.

Juno of Veii and Cybele were special too (a tunnel, a stone carried by road). Both are now plain
holy places: a tunnel and a carried stone were two more things that could fail.

**6. The AI must serve the gods too, or the gods are only the player's.**
Eight verbs could mean eight bits of AI. The cheap way out: everything that's a **delivery** (gifts,
offerings, grain for the geese) goes through the normal distribution system, so the
AI's economy feeds a holy place without any new logic, exactly as it feeds a mine. Only two verbs
needs a real decision: the **Janus switch** (open or close, a rule of thumb from war and peace).
Mars's field asks for soldiers the way a military building does, so the AI's soldiers go there
by themselves; the AI only has to leave it enough soldiers. Jupiter's hill and the geese need
nothing: holding is enough.

**7. Honest omens need the sim, not foresight.**
"An attack comes from the north within 5 minutes" is only honest if it's already decided. The
sim knows it the moment the attack command is given: soldiers take minutes to walk. So an omen is
**sent when the real event starts**, before the player could see it, and never guesses about the
future. That also works in LAN games, against human rivals: no one's plans are read, only what
already happens. Scripted events (the storm, Cybele's stone) are announced from the chapter's
script.

**8. A place is an obstacle forever.**
It blocks its node and the building space around it, like a large building that can never be
removed. Roads go around it. The editor and the map generator must keep it off water, away from
harbour spots, and reachable by land.

**9. Saves, maps and replays.**
Holy places are new world data: one SAVE bump, one map-format entry (old maps simply have none),
and new commands only at the end of the list (the Janus switch). Favour, deliveries and omens run
in the sim with its own random numbers, so replays and LAN games stay exact.

**10. Free games and multiplayer.**
Once built, holy places would work on any map. Free games could get an option, "holy places",
where the generator puts one or two between the players with random gods from the "plain" list
(Jupiter, the geese, Mars, Neptune, Janus). That's a reason to fight over the middle of the map,
and it's fair, because every player can reach them.

**11. What a holy place never does.**
It never keeps a player alive (losing every military building still ends the game). It doesn't
count as a building in the statistics, can't be hit by catapults, and can't be chosen as an
attack target. Its geese and figures are never fought.

### Omens

The chapter's advisor, an **augur**, reads the sky now and then: about one omen in 8–12 minutes,
more often at high favour. An omen is a short message with a hint at something real:

> *"Ravens circle the north. Someone there is counting his swords."*
> (the AI has decided to attack from the north within about 5 minutes)

At low favour the omen is vague: a direction or a kind of thing. At high favour it names a time
and a place. Clicking the omen's message jumps to the place, like other messages.

### The cast

Real people speak in the briefings and messages, and the story follows what the sources say about
them. Where the sources are legend (Romulus, the geese), we tell the legend as the Romans did,
without claiming more. Where they are history, we don't change who won, who died or when.

| # | You lead Rome as | Others who appear | Picus, the augur |
|---|---|---|---|
| 1 | Romulus | Remus, Faustulus the shepherd who raised them | Picus the elder, who watches the birds with Romulus |
| 2 | Romulus | Titus Tatius, king of the Sabines; Hersilia; Tarpeia | Picus, now Romulus's augur |
| 3 | Marcus Furius Camillus | The king of Veii (unnamed in the sources) | A Picus of the fourth century, who suggests the *evocatio* |
| 4 | Marcus Manlius on the Capitol, then Camillus | Brennus, the Gauls' leader ("Vae victis") | A Picus who keeps Juno's geese |
| 5 | Appius Claudius Caecus, the censor | Samnite leaders (Gaius Pontius of the Caudine Forks) | A Picus who reads the road's omens |
| 6 | Manius Curius Dentatus (who beat Pyrrhus at Beneventum, 275 BC; Laevinus had lost at Heraclea) | Pyrrhus of Epirus; Cineas, his envoy | A Picus on the Field of Mars |
| 7 | Gaius Duilius | Hannibal Gisco, the Carthaginian admiral | A Picus at sea for the first time |
| 8 | Quintus Fabius Maximus, then Publius Cornelius Scipio | Hannibal Barca | The Picus who reads the Sibylline books |
| 9 | Scipio Aemilianus | Cato in the senate; Hasdrubal, the city's last general | A Picus who doubts |
| 10 | Octavian, later Augustus | Agrippa, his admiral; Antony and Cleopatra | The last Picus, at the gates of Janus |

**The rule of plausibility.** The house of Picus is our invention and stays in the margins: an
augur beside the leader, never the one who decides a battle. The gods' touches are small enough
that a Roman could believe in them and a sceptic could explain them away; the story never says
which is right. Dates, places and outcomes are the real ones.

### The advisor: the house of Picus

A fictional family of augurs serves Rome through all ten chapters, one generation per chapter or
two. Their name comes from *picus*, the woodpecker, the bird of Mars and an old king of Latium in
legend. They play the role Mira plays in the world campaign.

The question that carries the story: **do the gods guide Rome, or does Rome use the gods?** Each
Picus answers it a little differently. The last one, in chapter 10, sees both are true and says so
quietly as the gates of Janus close.

## The ten chapters

Difficulty: ★ gentle · ★★ medium · ★★★ hard · ★★★★ very hard. The ★ chapters keep their rivals at
peace until you attack (as *The first valley* does now), so nothing can be lost while learning.

| # | Chapter | Year | Skill it teaches | God | Map | Difficulty |
|---|---|---|---|---|---|---|
| 1 | The Augury | 753 BC | Building, roads, land | Jupiter | Seven hills, small | ★ |
| 2 | The Sabine Peace | ~750 BC | Soldiers and a first attack | Concordia | Two valleys, small | ★ |
| 3 | Veii | 396 BC | Mines, geologists, weapons | Juno of Veii | Etruscan hills | ★★ |
| 4 | The Geese of the Capitol | 390 BC | Defence, military settings | Juno's geese | Rome, invaded | ★★ |
| 5 | The Appian Way | 312 BC | Long roads, donkeys, warehouses | Janus | Latium to Campania | ★★ |
| 6 | A Pyrrhic Victory | 280 BC | Promotions, gold, soldier quality | Mars | Magna Graecia | ★★★ |
| 7 | Mylae | 260 BC | Harbours, shipyards, sea attacks | Neptune | The strait of Sicily | ★★★ |
| 8 | Hannibal at the Gates | 218–204 BC | Holding out | Cybele | Italy and the Alps | ★★★★ |
| 9 | Carthago delenda est | 149–146 BC | Everything, at war | Mars | The African coast | ★★★★ |
| 10 | The Gates of Janus | 31–29 BC | Everything, then peace | Janus | The whole Mediterranean | ★★★★ |

### 1 · The Augury ★

**History.** Romulus and Remus quarrel over where to found the city. They let the gods decide:
Remus on the Aventine sees six vultures first, Romulus on the Palatine sees twelve.

**Play.** A small map, 56 nodes, with seven hills east of the Tiber and the Alban hills (a little
mountain with ore) to the south-east. Remus builds on the Aventine, peaceful all chapter. You
learn to build, to connect roads and to push out the border.

**Goal.** Hold 35 % of the map's land. Remus builds too, at peace, but stops at 35 %, so the goal
always stays within reach; when you reach it his people join you in the closing message, without
any fighting. Because nobody attacks, the game mans each of your military buildings with one
soldier: the 52 soldiers of the start hold the goal with plenty to spare. (Built and tested: the
AI as the player reaches the goal on 8 of 8 seeds in 21–36 minutes. With full garrisons on a
64-node map it ran out of soldiers at 27–36 %, there being no ore for weapons.)

**God: Jupiter.** His place is the augurs' platform on the Capitol, by the river, neutral between
you and Remus (12–13 nodes from both castles). Whoever's border first closes fully around it sees
twelve vultures (a story moment), and from then on sees 14 nodes far from the hill; while it is
yours, vultures circle over it. It's the first holy place of the campaign, and a lesson in where
to push the border. *(Built: a holy place is a map decoration from id 0xF0 on, which blocks like
a rock and that nothing removes, so maps and saves keep their formats; its holder is computed
from the land, `World::holy_holder`; Jupiter's sight is in the fog, `JUPITER_SIGHT`.)*

### 2 · The Sabine Peace ★

*Told as A with C: the women end the war, Tarpeia is the twist. Sources: Livy, book 1, 9–13;
Plutarch, Romulus 14–19.*

**History.** Rome is a town of men. At the festival of Consus, Romulus's men carry off the
daughters of their guests. Three small neighbouring towns strike back first and are beaten; Romulus
kills the king of Caenina with his own hand and dedicates his armour to Jupiter. Then the
Sabines under Titus Tatius come. Tarpeia, daughter of the Capitol's commander, lets them into the
citadel; they crush her under their shields. The two armies meet in the marshy valley between the
hills, where the Forum will be. The women, now wives and mothers in Rome, run between the lines
and stop the battle. Romulus and Tatius rule together, and the two peoples become one.

**The map.** About 64 nodes. Your hills (the Palatine with the castle, the Capitol with a
ready guardhouse) on one side; the Sabine land on the hills to the north-east; the marshy
valley between them, with **Concordia's shrine** at its edge. In a corner, the small town of
**Caenina**: weak, peaceful until attacked.

**The cast.**
- *Romulus* (you).
- *Hersilia*, his wife, speaks for the women: their voice, not their victimhood.
- *Picus*, the augur, as before.
- *Titus Tatius*, the Sabine king, in a few short messages: a proud man, not a villain.
- *Tarpeia*, in one scene only.

**As built (2026-10-27).** Simpler than the plan below, by the keep-it-simple rules:

- *Map:* the seven hills at 64 nodes; Romulus on the Palatine, the Sabines on their own hill far
  north of the Quirinal (28 nodes apart), Concordia's shrine in the valley halfway (14 from
  each castle, so both realms must push out to meet there).
- *Concordia:* no deliveries yet (they come with the offerings of chapter III). Her favour grows
  one point every 10 seconds while your land and the Sabines' both lie within 3 nodes of her
  shrine, and fades one a minute otherwise (`story::concordia_near`, `favour_step`; kept in the
  save). At 50 the goal needs one building fewer.
- *Caenina:* only in the briefing's spirit; a third realm would have muddled the goal's count.
- *Tarpeia (minute 20):* your military building nearest the Sabine castle goes over to them
  (`Cmd::HandOver`: its soldiers walk home, it keeps its land, the Sabines man it), told only
  when it happens; the Sabines' truce ends either way (`story::Deed`).
- *Goal:* take or burn two Sabine military buildings (`Objective::Vanquish`). When it is met,
  the women step in, the Sabines keep the peace for good, and the chapter is won.
- *Tested:* the goals soak wins 8 of 8 games (9-42 minutes), the story's deeds played by the
  harness as by the app.

**How it plays: four beats** *(the plan)*

1. **The quiet weeks (from the start).** The briefing tells the festival in one sentence, in the
   past: *"At the feast of Consus your men seized the Sabines' daughters and made them their wives. Their fathers
   have not forgiven it."* Nothing of it is shown. You build the soldier chain. Hersilia asks for
   a road to Concordia's shrine: *"Our families are across the valley. Let us at least send them
   bread."* Food or beer delivered there raises Concordia's favour.
2. **Caenina (optional).** Picus suggests the weak town as the place to learn war. If you take
   its post, a message tells how Romulus dedicated the king's armour to Jupiter. Nothing later
   depends on it.
3. **Tarpeia (minute 20, fixed).** At night your military building nearest to the Sabines **goes
   over to them**, and its soldiers come home to your castle. If you have none but the castle,
   the beat is skipped. Picus tells it in two sentences: *"Tarpeia opened the gate for what the
   Sabines wore on their left arms. She meant their gold bracelets; they gave her their
   shields."* He mentions the older second telling Livy reports, where she meant to trick them
   out of their shields: the campaign's question in small. From now on the Sabines are at war,
   and they attack modestly.
4. **The women step in.** The goal on screen: **take two Sabine military buildings**. With
   Concordia's favour at 50 or more, **one** is enough. The moment it's met, the women run
   between the armies, Hersilia speaks, the Sabines become allies, and the chapter is won.

**Goal.** Take two Sabine military buildings (one with Concordia's favour). Any building counts,
the one Tarpeia gave away too.

**Medal.** Time to the peace. Gold needs the gifts started early: Concordia rewards the player
who thought of the families before the war.

**Why this stays plausible.** Every beat is in Livy or Plutarch. The abduction isn't played,
counted or rewarded, only remembered. The women act, and their choice ends the war, which is how
the Romans themselves told it. Tarpeia is shown briefly and without gore, with both tellings.

**What it needs (new):**
- a timed beat that hands one building to a rival (Tarpeia);
- the peace: a rival becomes an ally when the goal is met.

### 3 · Veii ★★

**As built (2026-10-27).**

- *Map:* the random map at 80 nodes with more mountains (ore), Veii across it; Juno's altar 40 %
  of the way from your castle to Veii's (else 30 or 50 %), on buildable land with its ring
  clear (`rome::holy_places` for `RomeMap::Veii`).
- *Offerings:* click the altar (held: your land all round it) and offer 5 bread from your
  warehouses (`Cmd::Offer`); each bread is 4 favour, fading one a minute. No carriers walk to the
  altar: robust, and bread is still the miners' food, so the choice is real.
- *Juno's answer:* a story deed at favour 50 and again at 80 (`Deed::Bless`, `Cmd::Bless`): three
  privates in your warehouses become privates first class, told by Picus. In place of "Veii's
  soldiers are promoted more slowly", which would have needed lasting state in the simulation.
- *Goal:* take Veii's castle. The goals soak wins 6 of 6 games in 61-69 minutes (medals 60, 90,
  120).

**History.** Veii, the rich Etruscan city, withstands a Roman siege for ten years. Camillus
digs a tunnel under its walls into the temple of Juno, and asks the goddess to leave Veii for a
new home in Rome (*evocatio*). Veii falls in 396 BC.

**Play.** Hilly Etruscan country, rich in ore. Veii sits behind a ring of strong posts and is
too strong to storm with privates. You need geologists, mines, food for the miners, smelting
and an armory: the weapons chain.

**Goal.** Take the castle of Veii.

**God: Juno of Veii**, called *out* of the enemy's city. Her altar stands on your side of the
valley, facing Veii; offerings of bread there call her over, as Camillus called her. With
favour, Veii's soldiers are promoted more slowly. That is a small edge, felt only in a long
siege. (The tunnel of the story is told in the briefing, not played.)

### 4 · The Geese of the Capitol ★★

**As built (2026-10-27).**

- *Map:* the seven hills, Rome on the Palatine, two allied Gaulish realms (Brennus and the
  Senones, drawn as the northern people) on hills far to the north; Juno's temple with the geese
  on the Capitol's slope, 6 nodes from the castle, Rome's from the start (`RomeMap::Gauls`).
- *Goal:* hold out 45 minutes. The Gauls' truce is 10 minutes (15 on easy; `Mission::truce`):
  with the usual 30 there would have been almost no war.
- *The geese:* the game already warns of every attack the moment it is sent, with its numbers;
  earlier would not be honest. So the geese do what the legend says they did: fed with grain
  (offerings in the temple's window), at favour 50 they wake Manlius, and five new privates join
  the castle (`Deed::Bless` with `recruit`).
- *Medal:* a stand of 45 minutes is won in time or not at all, so every win is gold for now; a
  medal by buildings lost needs a counter of its own.
- *Tested:* the goals soak holds out in 6 of 6 games; seen live: the briefing's 10-minute truce,
  the temple's window with the grain offering.

**History.** In 390 BC the Gauls of Brennus rout the Romans at the Allia and sack Rome. Only the
Capitol holds. One night the Gauls climb the rock; the sacred geese of Juno cackle, Marcus
Manlius wakes and throws them back. Camillus returns with an army.

**Play.** You start with a ready but small realm on a hill. A much stronger Gaulish invader
arrives early from the north and attacks again and again. This chapter is about **defence**:
the military settings window, garrisons at the right border, catapults, and posts placed
where the attacks come.

**Goal.** Hold out 45 minutes. Then Camillus arrives, and the closing message tells how he drove
the Gauls out.

**God: Juno's geese**, on the Capitol, the last hill you hold. The geese eat grain, and the flock
walks along your border. With favour every attack is announced earlier and with its
direction: the game's attack warning, given by the gods.

### 5 · The Appian Way ★★

**History.** During the Samnite wars, the censor Appius Claudius builds a paved road from Rome to
Capua (312 BC), so the legions can reach the south in days instead of weeks. The Samnites in the
mountains had trapped a Roman army at the Caudine Forks once; they won't again.

**Play.** A long map running from Latium down to Campania. Three allied towns lie along it, far
apart. You learn **logistics**: long roads, road upgrades for donkeys, warehouses on the way,
and transport priorities.

**Goal.** New: connect all three allied towns to your castle by road. Samnite raids try to cut
the road; the goal counts the moment all three are connected.

**God: Janus**, the god of passages and beginnings. His gates stand in your city. Open them, and
soldiers are recruited faster to guard the road; close them, and the workshops and donkey
breeders work faster, but you can't attack. Switching takes a few minutes, so you must guess
the next Samnite raid.

### 6 · A Pyrrhic Victory ★★★

**History.** King Pyrrhus of Epirus crosses to Italy with elephants and a trained army. He beats
the Romans at Heraclea (280 BC) and Asculum (279 BC), but loses so many men that he says
another such victory will undo him.

**Play.** Pyrrhus's soldiers are few but highly ranked. Your privates lose every duel. This
chapter is about **soldier quality**: gold mines, the mint, coins for promotions, and
attacking with the strongest soldiers first.

**Goal.** Defeat Pyrrhus. The medal counts the soldiers you lose: winning by numbers is allowed,
but earns only bronze.

**God: Mars.** His field lies by the river, in your land. He wants no wares, he wants your
soldiers' time. The field asks for up to six soldiers from your warehouses; they march there,
drill for a few minutes and go home, and every drill raises his favour. With favour, a drilled
soldier is sometimes promoted without a coin. The cost: soldiers on the field defend nothing,
so drilling while Pyrrhus attacks is a gamble.

### 7 · Mylae ★★★

**History.** Rome has no navy. In the First Punic War the Romans copy a stranded Carthaginian
warship and build a hundred in sixty days. At Mylae (260 BC) the consul Gaius Duilius lays
boarding bridges onto the enemy decks and wins Rome's first sea battle.

**Play.** The strait between Italy and Sicily. Carthage holds western Sicily and is out of
reach by land. You learn **seafaring**: a harbour on a free coast, the shipyard switched to ships,
expeditions, and attacks from the sea (all built already).

**Goal.** Take Carthage's harbour town in Sicily.

**God: Neptune.** His shrine stands on a cape across the strait, on the coast Carthage holds:
you must land to take it. Fish at the altar raise his favour. Then ships sail faster, and once
in the chapter a storm keeps the Carthaginian fleet in port for a few minutes, announced as an
omen (see the example above).

### 8 · Hannibal at the Gates ★★★★

*Sources: Polybius, book 3; Livy, books 21–30; Plutarch, Fabius Maximus.*

**History.** Hannibal crosses the Alps (218 BC) and beats the Romans at the Trebia and at Lake
Trasimene. Rome names Quintus Fabius Maximus dictator. He refuses battle, shadows Hannibal from
the hills and lets hunger and distance wear the invader down; Rome mocks him as *Cunctator*, the
Delayer. In 216 BC the new consuls want a battle and get one: at Cannae Hannibal encircles and
destroys the largest army Rome ever raised. Rome doesn't make peace. It raises new legions, even
from slaves who are promised freedom, and goes back to Fabius's way. The Sibylline books say the
invader leaves only when the Great Mother comes to Rome: her stone arrives from Pessinus in 204
BC. That year Scipio lands in Africa, Hannibal is called home, and Scipio beats him at Zama (202
BC).

**The map.** A region map cut from our Mediterranean: northern Italy with the Alps, Rome in the
middle, the sea to the south, and the African coast with Carthage across it, at first out of
reach (no free harbour spot known).

**The cast.**
- *Fabius* (you), then *Scipio* (you, from beat 5).
- *Varro*, the impatient consul, the voice of the crowd.
- *Hannibal*, only through omens and reports: never seen, always near.
- *Picus*, who reads the Sibylline books.

**The invader's weakness.** This makes Fabius's way right in the game, not only in the story.
Hannibal arrives with a strong, highly ranked army (many officers and generals) but **without a
home economy in Italy**: he recruits no new soldiers and gets no coins, so every soldier he loses
is gone for good. (In history he won some Gauls and Italian towns over, and few reinforcements
came from Carthage; the game keeps the essence: he can't replace his veterans.) You keep
recruiting. Time is on your side, as it was on Rome's.

**How it plays: everything on the clock**

| Minute | Beat |
|---|---|
| 0 | **The Alps.** Fabius's advice in the briefing: *"Do not meet him in the open. Hold the strong places, let him come to you, and let Italy itself fight him."* Hannibal appears in the north |
| 10 | **The temptation.** Varro and the crowd mock you as *Cunctator*. A message only |
| 25 | **Cannae.** Varro marches without you, and the message tells of the disaster. Rome raises new legions: a batch of privates arrives at your castle (the slaves who fought for freedom) |
| 35 | **The Great Mother.** Her stone arrives from Pessinus; her temple on the Palatine now serves whoever holds its land: farms and mills work better |
| 50 | **Scipio lands in Africa.** Hannibal is called home. The chapter is won |

**One warning, once.** The first time you order an attack on a post whose defenders clearly
outrank your attackers, an omen comes first: *"The chickens will not eat. The augurs say: not
today."* (The Romans really did read feeding chickens before a battle.) It's only a message:
the attack goes ahead if you want it, and if it never fires, nothing is missing.

**Goal.** Hold out 50 minutes: your castle still stands when Scipio sails.

**Medal.** Land kept: the share of your starting land you still held at the worst moment. A
player who held back gets gold; one who threw the army at Hannibal's generals learns why Fabius
was right.

**Why this stays plausible.** The battles, the dictator, the mockery, Cannae, the slave legions,
the Great Mother and Scipio's landing are all in the sources and in order. Zama itself is told in
the closing message: chapter 9 takes the war to Africa. The omen of the feeding chickens is a
real Roman custom.

**What it needs (new):**
- an invader without recruiting or coins;
- timed beats: messages, privates for your castle;
- the rank-gap omen at an attack order (a message, nothing depends on it).

### 9 · Carthago delenda est ★★★★

**History.** Cato ends every speech in the senate with "Carthage must be destroyed." In the Third
Punic War, Scipio Aemilianus besieges the city for three years and takes it in 146 BC.

**Play.** The African coast and the islands before it. Carthage is rich, fortified and allied
with Numidian neighbours. It's full war on land and sea with everything learned so far.

**Goal.** Take the castle of Carthage.

**God: Mars**, harder this time. Carthage holds a holy place of its own, the temple of its god,
inside its walls. Mars's favour gives early warnings of counterattacks, and taking Carthage's
temple takes its god's help from Carthage. The chapter's story asks openly whether Rome needs
the gods for this, or only says so.

### 10 · The Gates of Janus ★★★★

**History.** After Actium (31 BC), where Octavian beats Antony and Cleopatra at sea, Rome is at
peace for the first time in two hundred years. In 29 BC the doors of the temple of Janus are
closed, which happens only when Rome is at peace everywhere.

**Play.** The whole Mediterranean. Rivals at every coast, some hostile, some neutral, some
ready to make peace.

**Goal.** Hold 75 % of the land. Then the gates close: the chapter ends in peace, not in a final
massacre. Rivals still standing make peace in the closing message.

**God: Janus**, who opens and closes, the same gates as in chapter 5: open for faster recruits,
closed for faster workshops. At the end you close them for good.

## Chapters V-X as built (2026-10-27)

All ten chapters are playable. V-X use the same few building blocks, simpler than the plans
above where those would have needed new lasting state in the simulation:

| # | Map | Goal | God, as built |
|---|---|---|---|
| V The Appian Way | random, 96 | reach the three allied towns (Capua, Cumae, Neapolis: allied **with** you, `Stance::Friend`): your land touches theirs (`Objective::Reach`); the Samnites at war | Janus's gates on the way to the first town: holding them, three helpers come (no gates switch yet) |
| VI A Pyrrhic Victory | random, 80 | take Pyrrhus's castle | the Field of Mars near Rome: his favour grows while six or more soldiers wait in the warehouses (drilling, `story::mars_drill`); at 50 and 80 three are promoted |
| VII Mylae | islands, 96 | take a Carthaginian military building across the sea (the castle was beyond the computer stand-in in 3 hours) | Neptune's shrine on whatever island lies halfway: fish offered, at 50 five sailors join |
| VIII Hannibal at the Gates | random, 96, more mountains | hold out 50 minutes (truce 15) | Cybele's temple near Rome; at minute 25 (Cannae) eight freed slaves, at 35 (her stone) five more join, while Rome holds it (no "Hannibal can't recruit" yet) |
| IX Carthago delenda est | random, 112 (the siege was by land; the sea is VII's) | take Carthage's castle; Numidia fights on your side | the Field of Mars as in VI; Carthage's temple near its walls |
| X The Gates of Janus | Europe (the world campaign's region, Rome in Italy: in the Mediterranean's Rome's slot was boxed in) | take Mark Antony's castle (his legions across the Alps; Cleopatra in the story) | Janus's gates near Rome: five veterans when held |

The goals soak (the computer playing each chapter as the player, 6 seeds, 3 hours at most)
wins 57-58 of 60: every chapter 6/6 except Mylae 5/6 and Carthage 5/6 (a game too long).

## The deeper check (2026-10-27)

All ten chapters were checked once more: every chapter started in the game and played for three
minutes; the goals soak on normal as well as easy (normal: 37 of 40, Pyrrhus beating the
computer stand-in in 2 of 6, a fair defeat for a ★★★ chapter); where each holy place lands, on
which landmass; and a reading of every text against the code and the sources. Fixed after it:
six years from Veii to the Allia (not thirty); 217 BC for Trasimene and Fabius; the last Picus no
longer the grandson's grandson of the first; Antony's fleet in Greece; chapter I's ending true
whether or not the Capitol was held; Tarpeia before the dawn of the war; blessings without fixed
numbers; "you" in the beats, not the commander's name; Manius Curius Dentatus, who beat Pyrrhus,
in place of Laevinus, who lost to him; the abduction named plainly; Cybele's temple inside Rome's
first land (Cannae's freed slaves come in every game); Neptune's shrine "on a coast"; and German
wording.

## Medals

Every chapter gives a bronze, silver or gold medal when won. Each chapter measures its own skill,
so a medal says what you did well, not only how fast. The menu shows the best medal per chapter
and level, like the best times in the world campaign now.

| # | Chapter | The medal measures |
|---|---|---|
| 1 | The Augury | Time to hold 40 % of the land |
| 2 | The Sabine Peace | Time to the peace |
| 3 | Veii | Time to take Veii |
| 4 | The Geese of the Capitol | Buildings lost while holding out |
| 5 | The Appian Way | Time the road was cut by raids |
| 6 | A Pyrrhic Victory | Soldiers lost |
| 7 | Mylae | Time to take Carthage's harbour town |
| 8 | Hannibal at the Gates | Land kept during the invasion |
| 9 | Carthago delenda est | Time to take Carthage |
| 10 | The Gates of Janus | Time to hold 75 % of the land |

The thresholds come from playtests: gold should take a good plan, not a perfect one. On easy the
thresholds are looser, so every level has all three medals to earn.

## The difficulty ramp

| | Map size | Rivals | At war | Start goods | Needed skills |
|---|---|---|---|---|---|
| 1–2 ★ | ~64 | 1 | Not until you attack | Plenty | 1 |
| 3–5 ★★ | ~96 | 1–2 | One, from the start or the story | Normal | 1 new + earlier ones |
| 6–7 ★★★ | ~128 | 2–3 | All but one | Normal | 1 new + all earlier |
| 8–10 ★★★★ | Region | 3–5 | Most, plus an invader | Few | All |

The menu's difficulty levels still apply on top. *Easy* keeps at most two rivals at war, as in the
world campaign; *hard* makes peace with no one.

## What it would take

**Already there:** missions with briefings and goals, story beats, peaceful and allied rivals,
invaders, survival and land goals, harbours, expeditions, sea attacks, mines that run low, the
statistics.

**New in the simulation (one SAVE bump, new enum variants only at the end):**

- holy places: a new map object with an owner taken from the land, and a flag; it never burns
  or breaks, so no new building type is needed (the map format gains an entry, as harbour
  spots have);
- favour per player and god, deliveries to a holy place, favour fading;
- the eight gods' verbs: watch (sight from a node), gifts across a border, offerings, a goose
  flock, the Janus switch, drill without coins, and a place the story opens (Cybele);
- the touches: small multipliers and one-off events per chapter;
- omens: an event with a hint, tied to real AI plans (the AI already decides attacks ahead).

**New in the campaign (rr-game):**

- a second campaign list beside `MISSIONS`;
- new story beats, all timed or on one event: a building handed over (Tarpeia), a stance change
  (the Sabine peace), soldiers sent to your castle (after Cannae);
- a "chapter played by the AI" test for each chapter;
- a new goal: connect towns by road;
- medals: a measure per chapter, the thresholds per level, the best medal kept per chapter;
- ten maps: 1–7 as hand-tuned generated maps with fixed features; 8–10 cut from the
  Mediterranean region we already have.

**New art:** eight holy places (the hilltop, the border shrine, Juno's temple, the Capitol, the
gates of Janus, the Field of Mars, the cape shrine, Cybele's temple and stone), the geese, and a
bird flock for omens. Each place is a chance for its own look; about ten sprites.

**Manual:** a new chapter in the handbook, with each chapter's map and the gods' table.

## Suggested order of work

1. **Chapter 1 alone** with holy places and Jupiter's hill, the smallest whole thing: does a
   holy place make a good goal?
2. **Chapters 2 and 4**: the two new story beats (stance change, ally arrives) and the geese
   warnings. If omens and warnings feel good, the system works.
3. **Chapter 7**: the sea, mostly built already.
4. The rest in order, each with a playtest before the next.

## Open questions for discussion

None at the moment. Next: chapter 1 as the first thing to build, after a last read of this page.
