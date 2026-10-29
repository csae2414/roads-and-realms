//! The campaigns' stories: one thread through the world campaign's six missions (Mira's map of
//! the world, and the Raven Guild that wants it), and the Roman campaign's chapters told by
//! Picus the augur; each in a prologue at the briefing, a few beats in the news when something
//! happens, and an epilogue when the mission is won.

/// What sets a beat off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Beat {
    /// The first building of this rival (player slot) seen.
    Contact(u8),
    /// The game is this many minutes old.
    Minute(u32),
    /// Our first capture of a rival's building.
    FirstCapture,
    /// The first military building we lose.
    FirstLoss,
    /// We have minted this many gold coins.
    Minted(u32),
    /// Our first ship is on the water.
    FirstShip,
    /// We hold a holy place (the Roman campaign: our land lies fully around it).
    HolyPlace,
    /// A god's favour with us has reached this much (the Roman campaign's Concordia).
    Favour(u32),
}

pub struct Tale {
    /// Above the briefing: where the story stands.
    pub prologue: &'static str,
    pub beats: &'static [(Beat, &'static str)],
    /// On the end screen of a won mission: what it led to.
    pub epilogue: &'static str,
    /// What the story does in the game when its moment comes (the Roman campaign's Tarpeia):
    /// each once, told apart by its own bits (`deed_bit`).
    pub deeds: &'static [(Beat, Deed)],
}

/// Something a story beat does in the game, beside its words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deed {
    /// Our military building nearest to this rival's castle goes over to it (`Cmd::HandOver`),
    /// and the story tells `line`; nothing if we have none but the castle.
    HandOver { to: u8, line: &'static str },
    /// This rival's truce ends: it is at war with us.
    War { with: u8 },
    /// The chapter's god answers (`Cmd::Bless` at the holy place we hold): `n` of our privates
    /// are promoted, or (`recruit`) `n` new privates come, and the story tells `line`; nothing if
    /// we hold no holy place.
    Bless { n: u32, recruit: bool, line: &'static str },
}

/// The bit a tale's deed `i` keeps in a game's done deeds.
pub fn deed_bit(i: usize) -> u32 {
    1 << i
}

/// The deeds due now and not done yet (`done`): their bits and what to do.
pub fn due_deeds(tale: &Tale, seen: &Seen, done: u32) -> Vec<(u32, Deed)> {
    tale.deeds
        .iter()
        .enumerate()
        .filter(|(i, (beat, _))| done & deed_bit(*i) == 0 && beat_due(*beat, seen))
        .map(|(i, (_, d))| (deed_bit(i), *d))
        .collect()
}

/// Where a hand-over strikes: `me`'s finished military building (not a warehouse) nearest to
/// `to`'s castle, if there is one.
pub fn hand_over_spot(sim: &rr_sim::Sim, me: u8, to: u8) -> Option<rr_core::MapCoord> {
    let g = sim.world().grid();
    // a lost castle leaves a dead id in `Player::hq`, so ask where it stands (it may stand nowhere)
    let target = sim.hq_pos(to)?;
    sim.buildings()
        .iter()
        .filter(|(_, b)| {
            b.player == me && b.is_ready() && !b.kind.is_warehouse() && sim.rules().building(b.kind).is_military()
        })
        .min_by_key(|(_, b)| (g.distance(b.pos, target), b.pos.y, b.pos.x))
        .map(|(_, b)| b.pos)
}

/// What the game shows now, for the beats.
#[derive(Clone, Debug, Default)]
pub struct Seen {
    pub minute: u32,
    /// Rivals (player slots) whose buildings we have seen.
    pub rivals: Vec<u8>,
    pub captured: bool,
    pub lost: bool,
    /// Gold coins we minted so far.
    pub minted: u32,
    /// We have a ship.
    pub ship: bool,
    /// We hold a holy place.
    pub holy: bool,
    /// The chapter's god's favour with us (0-100).
    pub favour: u32,
}

/// The bit a mission's beat `i` keeps in a game's told stories (the first eight bits are the
/// small stories of every game).
pub fn beat_bit(i: usize) -> u32 {
    1 << (8 + i)
}

/// The beats due now and not told yet (`told`): their bits and lines.
pub fn due(tale: &Tale, seen: &Seen, told: u32) -> Vec<(u32, &'static str)> {
    tale.beats
        .iter()
        .enumerate()
        .filter(|(i, _)| told & beat_bit(*i) == 0)
        .filter(|(_, (beat, _))| beat_due(*beat, seen))
        .map(|(i, (_, line))| (beat_bit(i), *line))
        .collect()
}

/// How near Concordia's shrine both lands must reach for her favour to grow.
pub const CONCORDIA_REACH: u16 = 3;

/// Concordia (the Roman campaign's chapter II): her shrine works while `me`'s land and a
/// rival's both lie within `CONCORDIA_REACH` of it, at war or at peace (where the lands meet,
/// the women carry gifts across). A radius rule, not a holder's.
pub fn concordia_near(sim: &rr_sim::Sim, me: u8) -> bool {
    let w = sim.world();
    w.holy_places().iter().filter(|(_, god)| *god == 1).any(|(at, _)| {
        let owners: Vec<u8> = w.grid().within(*at, CONCORDIA_REACH).filter_map(|q| w.owner(q)).collect();
        owners.contains(&me) && owners.iter().any(|o| *o != me)
    })
}

/// How many soldiers must wait in the warehouses for Mars's drill.
pub const DRILL_SOLDIERS: u32 = 6;

/// Mars (chapters VI and IX): his favour grows while `me` holds his field and at least
/// `DRILL_SOLDIERS` soldiers wait in the warehouses, drilling there (the owner's choice:
/// drilling, not victories).
pub fn mars_drill(sim: &rr_sim::Sim, me: u8) -> bool {
    let w = sim.world();
    let field = w.holy_places().iter().any(|(at, god)| *god == 5 && w.holy_holder(*at) == rr_world::Holder::Player(me));
    field && sim.player_inventory(me).soldiers() >= DRILL_SOLDIERS
}

/// The favour after frame `gf` (called every 100 frames): one point every 10 seconds while
/// the lands meet at the shrine, back one a minute while they don't; 0 to 100.
pub fn favour_step(favour: u32, near: bool, gf: u32) -> u32 {
    if near && gf.is_multiple_of(200) {
        (favour + 1).min(100)
    } else if !near && gf.is_multiple_of(1200) {
        favour.saturating_sub(1)
    } else {
        favour
    }
}

/// Whether what a beat waits for has happened.
fn beat_due(beat: Beat, seen: &Seen) -> bool {
    match beat {
        Beat::Contact(p) => seen.rivals.contains(&p),
        Beat::Minute(m) => seen.minute >= m,
        Beat::FirstCapture => seen.captured,
        Beat::FirstLoss => seen.lost,
        Beat::Minted(n) => seen.minted >= n,
        Beat::FirstShip => seen.ship,
        Beat::HolyPlace => seen.holy,
        Beat::Favour(n) => seen.favour >= n,
    }
}

/// Juno's answers in chapter III, told only when they come.
const JUNO_FIRST: &str = "Picus comes running from the altar: \"Juno has heard you. She is leaving Veii for Rome.\" Soldiers waiting in your warehouses swear they saw her in a dream. They train with new heart: some of them are privates first class now.";
const JUNO_SECOND: &str = "In Veii, the priests find Juno's statue turned towards Rome. More of your waiting soldiers rise a rank. Picus: \"A city its goddess has left is only walls.\"";

/// Juno's geese in chapter IV, told only when they wake the guards.
const GEESE: &str = "At night the Gauls climb the rock of the Capitol, so quietly that not even the dogs wake. But Juno's geese cackle and beat their wings. You leap up and throw the first Gaul off the cliff, and the others fall with him. At dawn five men who were thought too old or too young to fight join the defenders.";

/// Where a god's answer is given: a holy place `me` holds.
pub fn held_holy_place(sim: &rr_sim::Sim, me: u8) -> Option<rr_core::MapCoord> {
    let w = sim.world();
    w.holy_places().into_iter().map(|(p, _)| p).find(|p| w.holy_holder(*p) == rr_world::Holder::Player(me))
}

/// Chapter II's hand-over, told only when there was a post to give.
const TARPEIA: &str = "At night Tarpeia, the daughter of the commander of your post nearest the Sabines, opens its gate to them. She asked for what they wore on their left arms: she meant their gold bracelets, and they gave her their shields. (Another telling says she meant to trick them out of their shields.) The post is theirs, and its soldiers come home to you.";

pub fn tale(mission: &str) -> Option<&'static Tale> {
    TALES.iter().find(|(k, _)| *k == mission).map(|(_, t)| t)
}

/// Every line of the story (for the translation's test).
pub fn all_lines() -> impl Iterator<Item = &'static str> {
    let deed_lines = TALES.iter().flat_map(|(_, t)| {
        t.deeds.iter().filter_map(|(_, d)| match d {
            Deed::HandOver { line, .. } | Deed::Bless { line, .. } => Some(*line),
            Deed::War { .. } => None,
        })
    });
    TALES
        .iter()
        .flat_map(|(_, t)| [t.prologue, t.epilogue].into_iter().chain(t.beats.iter().map(|b| b.1)))
        .chain(deed_lines)
}

const TALES: [(&str, Tale); 16] = [
    (
        "first",
        Tale {
            prologue: "Mira the map-maker is old, and every hall laughs at her map of the world: lands beyond the sea, mountains marked with little crowns of gold, a sea in the middle of the earth. Only your house believes her. Before you sail for the lands she drew, she asks for one thing: show her you can build a realm, here in the valley.",
            beats: &[
                (
                    Beat::Contact(1),
                    "Aldric's banners on the ridge. His rider brings a friendly word: \"The valley is big enough for two, neighbour.\" Mira frowns: \"Aldric was never this friendly to anyone.\"",
                ),
                (
                    Beat::Minute(15),
                    "A letter for you, sealed with a black raven: \"I hear your house keeps an old woman's map. I collect such curiosities. Name your price; I pay in silver. Corvin, of the Raven Guild.\" Mira tears it in two.",
                ),
                (
                    Beat::Minute(30),
                    "Mira, from her window: \"Aldric's new walls are stone from far away, and his men are paid in good silver. Where does a poor marcher lord find so much?\"",
                ),
                (
                    Beat::FirstCapture,
                    "In the post you took lies a purse of silver, sealed with the same black raven as Corvin's letter.",
                ),
            ],
            epilogue: "Aldric's keys lie on your table. In his chest are Corvin's letters: the Guild paid for every wall, and asked for one thing in return, Mira's map. Mira rolls it out and shows you why. Beside each little crown she wrote how deep the gold lies, and where the rivers can be crossed: \"Whoever holds this knows where every realm will grow rich, before the realm knows it. Get there first.\"",
            deeds: &[],
        },
    ),
    (
        "europe",
        Tale {
            prologue: "Mira's first mark lies on the hills of Italy, and the Guild got there before you. Bjorn of Germania, the Guild's sword, has sworn that no one else will rule beyond the Alps. Mira puts her finger on the little crown she drew in the mountains: \"Gold. Find it before he does.\"",
            beats: &[
                (
                    Beat::Contact(4),
                    "Bjorn's scouts wear the black raven on their shields. Livia of Iberia sends a quiet word: she has no quarrel with you. Not yet.",
                ),
                (
                    Beat::Contact(5),
                    "Hadrian of Sarmatia's banners fly beside Bjorn's raven. The Guild has bought him too, and for less.",
                ),
                (
                    Beat::Minted(1),
                    "The first coin of Alpine gold rings on your table. Mira's little crown was right, to the depth she wrote beside it.",
                ),
                (
                    Beat::Minute(30),
                    "Another raven letter: \"Leave Germania to Bjorn and the map to me, and the Guild will pay your soldiers for ten years. Refuse, and it will pay his. C.\"",
                ),
                (
                    Beat::FirstLoss,
                    "A post of yours has fallen to men paid in Guild silver. Mira: \"Every soldier Bjorn sends is bought, except Bjorn. Take him, and the Guild has no sword left.\"",
                ),
            ],
            epilogue: "Bjorn's hall burns, and the Guild has lost its sword: no one in the north will fight under the raven now. Among Bjorn's maps lies a copy of Mira's own, line for line, every crown in its place. Someone gave the Guild her work. Mira is silent for a long time, then points across the ocean: \"My second mark. If they have my map, they are already there.\"",
            deeds: &[],
        },
    ),
    (
        "northamerica",
        Tale {
            prologue: "Across the ocean, Mira's map was right: a wide land, rivers and plains, and Virginia's coast where your ships land. The Guild was here first, selling land it has never seen. Four of the five rulers hold Guild deeds, and every deed promises the gold of the West.",
            beats: &[
                (
                    Beat::Contact(1),
                    "Ezra of New England holds up a deed with the raven seal: \"This coast was sold to me. Leave, or pay.\"",
                ),
                (
                    Beat::Contact(3),
                    "Santiago of Texas laughs at the Guild's deeds: \"Paper. Land is held, not bought.\" On that, at least, you agree.",
                ),
                (
                    Beat::Minute(25),
                    "Every deed the Guild sold here is drawn from Mira's map. Mira looks at the coastlines for a long time: \"Whoever copied this learned to draw from me.\"",
                ),
                (
                    Beat::Minute(50),
                    "A raven letter, friendlier than the last: \"Why fight for land? The Guild has a deed for all of it. Buy one.\" The deed in the envelope is for Virginia. Your Virginia.",
                ),
                (
                    Beat::FirstCapture,
                    "In the post you took, a Guild deed hangs framed on the wall. Its soldiers ran when the Guild's pay did not come. A deed, it turns out, holds nothing.",
                ),
            ],
            epilogue: "A third of the continent is yours, and no one here will buy a Guild deed again. A captain of Ezra's tells you where the charts came from: a young map-maker called Tove, who sailed north to Svealand with Guild silver. Mira goes pale. Tove was her apprentice.",
            deeds: &[],
        },
    ),
    (
        "north",
        Tale {
            prologue: "Winter, and Svealand. Tove is somewhere in these woods with Mira's charts, and the Guild has paid the northern lords to drive you out before the ice breaks. You don't have to win here: hold on until spring.",
            beats: &[
                (
                    Beat::Contact(1),
                    "Halvard of Norvegr's men carry Guild silver and Guild axes. They will come when the snow lies deepest.",
                ),
                (
                    Beat::Minute(20),
                    "A raven letter with no greeting: \"Svealand is a cold place to die. Send me the old woman, and the snow can keep your village. C.\" Mira reads it and laughs for the first time in weeks.",
                ),
                (
                    Beat::Minute(30),
                    "A third of the winter is behind you. At night a fire burns alone on the hill across the river: someone is watching your village and not attacking it.",
                ),
                (
                    Beat::FirstLoss,
                    "A post is lost in the snow. In the morning a sled stands at your gate with bread and no driver.",
                ),
                (
                    Beat::Minute(60),
                    "The fire on the hill is Tove's. She sends a note: \"Corvin promised me my name on the map of the world. I see now he only wants the map.\"",
                ),
                (
                    Beat::Minute(75),
                    "Halvard's captains count their dead, and ask why they should freeze for a merchant who has never seen snow.",
                ),
            ],
            epilogue: "The ice breaks, and the Guild's men go home: not one more would freeze for Corvin. Tove comes to your gate alone, with Mira's charts and Corvin's letters to her. They tell where his power still lies: in the banks of Jiangnan at the end of the silk road, which lend him all he spends, and in the harbors of the sea in the middle of the earth. Mira takes her back. \"A map-maker should know the way home.\"",
            deeds: &[],
        },
    ),
    (
        "fareast",
        Tale {
            prologue: "Mira's map ends far in the east, on the Yellow River. Here is the Guild's purse: Lianhua of Jiangnan lends to Corvin, and every army he bought was bought with her money. Tove rides with you now. She knows the roads to Jiangnan; she drew them.",
            beats: &[
                (
                    Beat::Contact(4),
                    "Lianhua's envoy brings silk and an offer: half of the south, if you turn back. Behind the envoy rides a man with a raven ring.",
                ),
                (
                    Beat::Contact(3),
                    "Hideko of Yamato's ships are on the coast. Tove knows them: \"Yamato sails with the Guild's friends, but her soldiers can only come by sea. Guard your shores.\"",
                ),
                (
                    Beat::Minute(30),
                    "A raven letter, for Tove this time: \"Come back, and your name goes on the map after all. In big letters.\" Tove burns it without a word.",
                ),
                (
                    Beat::Minted(1),
                    "The first coin of Qinling gold. Tove draws a new little crown on Mira's map, in her own hand, and Mira lets her.",
                ),
                (
                    Beat::FirstShip,
                    "Your first ship is launched. Tove watches it go: \"Now Yamato is not the only one who can cross.\"",
                ),
                (
                    Beat::FirstCapture,
                    "In the post you took, a Guild courier has left his satchel behind: Lianhua's ledgers. Every loan she made to Corvin runs on to one place, the harbors of the sea in the middle of the earth.",
                ),
            ],
            epilogue: "Jiangnan's banks are closed to the Guild: Corvin can borrow nothing more. Lianhua, captured, shrugs: \"He owes me more than you do. Go to the sea in the middle of the earth. He sits in its harbors, and he has never fought a battle in his life. He buys them, with what he has left.\"",
            deeds: &[],
        },
    ),
    (
        "mediterranean",
        Tale {
            prologue: "The last mark on Mira's map: the sea in the middle of the earth. Your ships land at Carthage, an old harbor the Guild overlooked. Corvin has no sword and no bank left, only the silver in his vaults, and every power on this sea still takes his coin. Swords will not beat him. Gold will: mint more than he has, and no one on this sea will want his.",
            beats: &[
                (
                    Beat::Contact(2),
                    "Cornelia of Roma sends a merchant's greeting: \"The Guild pays well for Carthaginian harbors. Make me a better offer.\"",
                ),
                (
                    Beat::Contact(5),
                    "Nefret of Aegyptus has taken Corvin's silver to land her soldiers on your coast. He is emptying his vaults to stop you.",
                ),
                (
                    Beat::Minted(20),
                    "Corvin writes in his own hand now, without the raven: \"I could have bought you once. What do you cost today?\" Mira answers for you: \"The map is not for sale.\"",
                ),
                (
                    Beat::Minute(40),
                    "Tove copies Mira's map one last time, clean, every crown and every ford: \"When this is over, someone has to keep it.\"",
                ),
                (
                    Beat::Minted(40),
                    "In every harbor the merchants weigh your coins against the Guild's, and take yours. Corvin's ravens come home unpaid.",
                ),
            ],
            epilogue: "Your sixtieth coin, and the Guild's coin is worth nothing on this sea. Corvin comes to your gate himself: a small man in a black cloak, with nothing left to offer. He asks only to see the map, once. Mira, very old now, shows it to him. Then she draws one last line on it, round your realm, and gives the pen to Tove.",
            deeds: &[],
        },
    ),
    // the Roman campaign (docs/CAMPAIGN-ROME.md): Picus the augur tells it
    (
        "augury",
        Tale {
            prologue: "Rome is not yet a city. On the hills above the Tiber two brothers, raised by a shepherd, each want to found one: Romulus on the Palatine, Remus on the Aventine. They have agreed to let the gods decide. Each will watch the sky from his own hill, and whoever is sent the better sign will found the city. Picus, the old augur, stands beside you.",
            beats: &[
                (
                    Beat::Contact(1),
                    "Remus's banner stands on the Aventine. His herald calls across the valley: \"My brother builds on his hill, I on mine. Let the birds decide which one is Rome.\"",
                ),
                (
                    Beat::Minute(5),
                    "Picus leans on his curved staff: \"The gods answer from the sky, Romulus. But a sign is worth nothing on a hill that no one holds.\"",
                ),
                (
                    Beat::Minute(15),
                    "Picus points across the hills: \"Seven hills, and the river at their feet. Every hill you hold is one more that the city will stand on.\"",
                ),
                (
                    Beat::Minute(10),
                    "Faustulus, the shepherd who raised you both, comes over from the Aventine: \"Remus watches the sky every morning. He has counted six vultures, he says, and swears he saw them first.\"",
                ),
                // (last in the list: the told bits of older saves keep their beats)
                (
                    Beat::HolyPlace,
                    "Your land lies all around the Capitol now, and Picus climbs to the augurs' platform. Twelve vultures circle over the hill. \"Jupiter has answered,\" says Picus. \"From up here you will see far.\"",
                ),
            ],
            epilogue: "The hills are yours, and the sky gave its sign: twelve vultures for Romulus, six for Remus. The city takes your name: Roma. The old tellings disagree about what became of Remus; his people stay and build on the Aventine as Romans. Picus marks the day for the calendar, the twenty-first of April. Across the valley, the Sabines watch the new walls.",
            deeds: &[],
        },
    ),
    (
        "sabines",
        Tale {
            prologue: "Rome stands, but it is a town of men. At the feast of Consus your men seized the daughters of their Sabine guests and made them their wives, and their fathers have not forgiven it. Across the valley, on the Quirinal, the Sabine king Titus Tatius gathers his people. Between the two hills stands an old shrine of Concordia. Picus, the augur, has served you since the vultures.",
            beats: &[
                (
                    Beat::Contact(1),
                    "Sabine banners on the Quirinal. Titus Tatius sends no herald. Picus: \"A king who sends no word is counting his spears.\"",
                ),
                (
                    Beat::Minute(4),
                    "Hersilia, your wife, comes to you: \"Our families are across the valley. Where your land meets theirs by Concordia's shrine, we can carry them bread, and they will see that we are well.\"",
                ),
                (
                    Beat::Favour(50),
                    "The women have carried bread across the valley for many days now. Hersilia: \"Our fathers ask for us by name. One battle, and they will listen to us.\"",
                ),
                (
                    Beat::Minute(21),
                    "Sabine spears in the valley at dawn: Titus Tatius is at war with you. Picus: \"Now we will learn who keeps your gates.\"",
                ),
                (
                    Beat::FirstCapture,
                    "You have taken a Sabine post. Picus: \"Tatius has lost ground before. He has never lost his pride.\"",
                ),
            ],
            epilogue: "The armies meet in the marshy valley below the Capitol. Then the women run between the lines, their hair loose, their children in their arms: \"If you must fight, fight us, not each other. We are wives on one side and daughters on the other.\" The fathers lower their spears. Romulus and Titus Tatius make peace and rule together: Romans and Sabines, one people on two hills. Picus marks the place where the women stood. One day it will be the Forum.",
            deeds: &[
                (Beat::Minute(20), Deed::HandOver { to: 1, line: TARPEIA }),
                (Beat::Minute(20), Deed::War { with: 1 }),
            ],
        },
    ),
    (
        "veii",
        Tale {
            prologue: "Rome has grown for three hundred years, and one city has always stood in its way: Veii, the rich Etruscan city on its rock across the valley, behind walls no army has broken. The senate sends you, Marcus Furius Camillus. Picus, of the augurs' house, rides with you: \"Veii's walls are stone. Its goddess is Juno. Take her, and the walls are nothing.\"",
            beats: &[
                (
                    Beat::Contact(1),
                    "Veii's towers on the rock. Its soldiers have trained for generations: you will need iron, and many swords. Picus: \"Geologists first. The hills here are full of ore.\"",
                ),
                (
                    Beat::Minute(6),
                    "Picus shows you an old altar of Juno on your side of the valley, facing Veii: \"When your land lies all around it, offer bread there. Juno is the city's goddess; we will ask her to come to Rome.\"",
                ),
                (
                    Beat::Favour(25),
                    "Smoke from Juno's altar drifts towards Veii. Picus: \"She listens. Keep the bread coming, though the miners grumble.\"",
                ),
                (
                    Beat::FirstCapture,
                    "You have taken one of Veii's posts. Its soldiers fought like veterans, and they were. Picus: \"Every post we take is a gate less around the rock.\"",
                ),
            ],
            epilogue: "Your miners dig a tunnel through the rock, right up under the temple of Juno in the citadel. Camillus asks the goddess aloud to leave the city and come to a temple worthy of her in Rome, and the story says she nodded. Veii falls after ten years of war. Juno's statue is carried to the Aventine. Picus writes in the augurs' book: \"We did not take her. She came.\"",
            deeds: &[
                (Beat::Favour(50), Deed::Bless { n: 3, recruit: false, line: JUNO_FIRST }),
                (Beat::Favour(80), Deed::Bless { n: 3, recruit: false, line: JUNO_SECOND }),
            ],
        },
    ),
    (
        "gauls",
        Tale {
            prologue: "Six years after Veii the Gauls come over the mountains: the Senones under their chief Brennus. At the river Allia they scatter Rome's army in a single afternoon, and the road to the city lies open. Rome cannot fight them in the field. You hold the hills, and above all the Capitol, where Juno's sacred geese are kept. Picus: \"Hold out until Camillus comes. He is raising an army.\"",
            beats: &[
                (
                    Beat::Contact(1),
                    "Gaulish horns in the north. The Senones are taller than any Roman, and they fight with long swords. Picus: \"Do not go out to them. Let them come to the walls.\"",
                ),
                (
                    Beat::Minute(3),
                    "Picus at Juno's temple: \"The geese are hungry, and so are we. But a fed goose is a watchful goose. Some grain for them, now and then.\"",
                ),
                (
                    Beat::FirstLoss,
                    "A post has fallen to the Gauls. Picus: \"Let them have the plain. Keep the hills.\"",
                ),
                (
                    Beat::Minute(30),
                    "A messenger slips through the Gaulish lines: Camillus is coming with an army. Hold out a little longer.",
                ),
            ],
            epilogue: "Camillus arrives as Brennus is weighing the gold Rome must pay to be left alone, with false weights, and his sword thrown onto the scale: \"Vae victis\", woe to the defeated. Camillus answers that Rome pays with iron, not with gold, and drives the Gauls from the city. From that year on a goose is carried through Rome on a purple cushion. Picus writes: \"Rome was saved by birds, and by the men they woke.\"",
            deeds: &[(Beat::Favour(50), Deed::Bless { n: 5, recruit: true, line: GEESE })],
        },
    ),
    (
        "appian",
        Tale {
            prologue: "312 BC. The war with the Samnites drags on in the mountains of the south, and Rome's allies there wait weeks for every legion. The censor Appius Claudius has a plan the senate laughs at: a road of stone from Rome to Capua, straight as a spear. Picus of the augurs' house marks the first stone: \"Janus opens every road. Ask him well.\"",
            beats: &[
                (
                    Beat::Contact(4),
                    "Samnite riders on the ridges. Picus: \"They remember the Caudine Forks. So do we.\"",
                ),
                (
                    Beat::Minute(8),
                    "The first miles are paved. Picus: \"Roads are slow to build and quick to cut. Warehouses on the way, and donkeys where the carriers wait.\"",
                ),
                (
                    Beat::FirstLoss,
                    "The Samnites take a post by the road. Your eyes are already failing, and you only say: \"Rebuild it, and pave further.\"",
                ),
            ],
            epilogue: "The Appian Way reaches Capua, and the legions march south in five days. The Samnites fight on for years, but they never trap a Roman army again. Appius Claudius goes blind in his old age; the road he built still carries Rome. Picus writes: \"Some gods answer with birds. Janus answered with a road.\"",
            deeds: &[(
                Beat::HolyPlace,
                Deed::Bless {
                    n: 3,
                    recruit: true,
                    line: "Your land closes around the old gates of Janus by the road. Picus opens them: \"Janus looks both ways, back to Rome and on to the south.\" Three men from the road's camps take up arms for you.",
                },
            )],
        },
    ),
    (
        "pyrrhus",
        Tale {
            prologue: "280 BC. The Greek city of Tarentum has called a king from across the sea: Pyrrhus of Epirus, a cousin of Alexander, with elephants no Roman has ever seen. At Heraclea and Asculum his phalanx and his elephants break Rome's legions. Now the command is yours, Manius Curius Dentatus. Picus walks with you on the Field of Mars by the river: \"We cannot buy soldiers like his. We can make them.\"",
            beats: &[
                (
                    Beat::Contact(1),
                    "Elephants at the edge of your land. Your carriers run from them, and so do some of your soldiers. Picus: \"Beasts are only beasts. Soldiers are made.\"",
                ),
                (
                    Beat::Minute(10),
                    "Picus: \"Keep soldiers in the castle, and let them drill on Mars's field. Coins for the posts at the front: a soldier who has been promoted does not run.\"",
                ),
                (
                    Beat::FirstCapture,
                    "You have taken a post from Pyrrhus. His envoy Cineas comes with gifts and talk of peace. The senate sends him home: no peace while a foreign king stands in Italy.",
                ),
                (
                    Beat::FirstLoss,
                    "A post falls to Pyrrhus, but his own losses are heavy. Picus: \"He wins every battle, and his army grows smaller with each.\"",
                ),
            ],
            epilogue: "At Beneventum, in 275 BC, you beat Pyrrhus at last, and he leaves Italy, his army spent. Another such victory, he is said to have told his friends, and I am lost. Rome has learned that it can lose battles and still win the war, as long as it keeps making soldiers. Picus writes: \"Mars does not love the bravest army. He loves the one that drills.\"",
            deeds: &[
                (
                    Beat::Favour(50),
                    Deed::Bless {
                        n: 3,
                        recruit: false,
                        line: "The soldiers drilling on the Field of Mars see a woodpecker, Mars's bird, land on their standard. Some of them are promoted that day: privates first class now.",
                    },
                ),
                (
                    Beat::Favour(80),
                    Deed::Bless {
                        n: 3,
                        recruit: false,
                        line: "Mars is pleased with the drill. More soldiers rise a rank, and the whole army hears of it.",
                    },
                ),
            ],
        },
    ),
    (
        "mylae",
        Tale {
            prologue: "260 BC. The war with Carthage has come to Sicily, and Carthage's ships sail where they like. Rome has never built a warship. Then a Carthaginian ship runs aground on the coast, and Roman carpenters take it apart plank by plank and build a hundred like it in sixty days. Picus, who has never been to sea, looks at the waves: \"Neptune decides. We had better ask him.\"",
            beats: &[
                (
                    Beat::Minute(5),
                    "Picus: \"A harbour first, on a coast with an anchor sign. Then a shipyard, and switch it to ships. Carthage will not come to us.\"",
                ),
                (
                    Beat::FirstShip,
                    "Your first ship is on the water. Its rowers are farmers who learned to row on benches on the beach. You have a boarding bridge fixed to every bow: \"We fight at sea as we fight on land.\"",
                ),
                (
                    Beat::Contact(1),
                    "Carthaginian sails on the horizon. Their crews laugh at your heavy ships. Picus: \"Let them laugh until the bridges fall.\"",
                ),
                (
                    Beat::FirstCapture,
                    "You have taken a post on Carthage's shore. Picus: \"The sea is a road too, it seems.\"",
                ),
            ],
            epilogue: "At Mylae Duilius' boarding bridges fall onto the Carthaginian decks, and Roman soldiers fight there as on land. Carthage loses its first battle at sea. Rome gives Duilius a column decorated with the bronze rams of the captured ships, and a flute player to walk before him at night for the rest of his life. Picus writes: \"Neptune let us cross. I still do not like the sea.\"",
            deeds: &[(
                Beat::Favour(50),
                Deed::Bless {
                    n: 5,
                    recruit: true,
                    line: "Fishermen from every coast have seen the smoke of your offerings at Neptune's shrine. Five of them come to row and to fight for Rome: privates in your castle now.",
                },
            )],
        },
    ),
    (
        "hannibal",
        Tale {
            prologue: "217 BC. Hannibal of Carthage has done what no one thought possible: he has crossed the Alps with an army and elephants, and beaten Rome at the Trebia and at Lake Trasimene. The senate names you dictator, Quintus Fabius Maximus. Picus, who has heard the Sibylline books read, tells you: \"Do not meet him in the open. Let him come to you.\"",
            beats: &[
                (
                    Beat::Contact(1),
                    "Hannibal's army in the north: veterans of Spain and the Alps, who know their trade. Picus: \"Every soldier he loses is one he cannot replace. Ours we can.\"",
                ),
                (
                    Beat::Minute(10),
                    "The people of Rome call you Cunctator, the Delayer, and mean it as an insult. Picus: \"One day they will mean it as a title.\"",
                ),
                (
                    Beat::Minute(25),
                    "News from Apulia: the consul Varro has given battle at Cannae without you, and Hannibal has surrounded and destroyed the largest army Rome ever raised. Rome does not ask for peace.",
                ),
                (
                    Beat::Minute(45),
                    "Scipio is gathering ships in Sicily. A few more minutes, and Rome strikes at Africa.",
                ),
            ],
            epilogue: "Scipio lands in Africa, and Carthage calls Hannibal home. After fifteen years in Italy he leaves without ever having taken Rome, and at Zama Scipio beats him. The name Cunctator becomes a title of honour. Picus writes: \"Fabius won no battle. He won the war.\"",
            deeds: &[
                (
                    Beat::Minute(25),
                    Deed::Bless {
                        n: 8,
                        recruit: true,
                        line: "Rome arms eight thousand slaves, promising them their freedom. Eight new privates come to your castle.",
                    },
                ),
                (
                    Beat::Minute(35),
                    Deed::Bless {
                        n: 5,
                        recruit: true,
                        line: "The Great Mother's stone arrives at Ostia, a black stone from Pessinus, and is carried to her temple while the people line the road. Five men who watched it pass join your army.",
                    },
                ),
            ],
        },
    ),
    (
        "carthage",
        Tale {
            prologue: "149 BC. Carthage has paid its debts to Rome and grown rich on trade. In the senate Cato holds up a fresh fig from Carthage: \"Picked three days ago, that close is Carthage.\" Every speech he ends the same way: Carthage must be destroyed. Picus is old now, and he doubts: \"Do the gods want this, or do we only say they do?\"",
            beats: &[
                (
                    Beat::Minute(5),
                    "Numidian horsemen ride beside your columns. King Massinissa is almost ninety years old and still in the saddle.",
                ),
                (
                    Beat::Contact(1),
                    "The walls of Carthage, the greatest you have seen, and behind them a city that will not surrender.",
                ),
                (Beat::FirstCapture, "A Carthaginian post falls. The city behind it does not ask for terms."),
            ],
            epilogue: "After three years of siege Carthage falls and burns for many days. Scipio Aemilianus watches the fire and weeps; he quotes Homer on the fall of Troy and says he fears the same day will come for Rome. Picus closes the augurs' book for this war: \"We asked the gods every morning. I am no longer sure they answered.\"",
            deeds: &[
                (
                    Beat::Favour(50),
                    Deed::Bless {
                        n: 3,
                        recruit: false,
                        line: "Before Carthage, the soldiers who wait in your warehouses drill as they did on the Field of Mars at home. Some of them are promoted: privates first class now.",
                    },
                ),
                (
                    Beat::Favour(80),
                    Deed::Bless {
                        n: 3,
                        recruit: false,
                        line: "Mars stays with the army before Carthage. More of your soldiers rise a rank.",
                    },
                ),
            ],
        },
    ),
    (
        "janus",
        Tale {
            prologue: "31 BC. Julius Caesar is dead, and the Republic with him. Octavian, his adopted son, holds Rome; Mark Antony, with Cleopatra of Egypt as his ally, gathers his legions in the north and his fleet in Greece. The doors of the temple of Janus stand open, as they have stood for two hundred years: Rome is at war. The last of the house of Picus, a young augur, stands beside you: \"The first of my house saw the vultures over Rome's hills. I would like to see the gates close.\"",
            beats: &[
                (
                    Beat::Contact(4),
                    "Antony's legions in the north, and Cleopatra's gold behind them. Your friend Agrippa: \"Give me soldiers, and I will give you Antony.\"",
                ),
                (
                    Beat::Contact(1),
                    "The Cantabri in their mountains, who have fought every Roman army for a hundred years. Picus: \"Antony first. Spain will wait.\"",
                ),
                (
                    Beat::FirstCapture,
                    "You have taken one of your rivals' posts. Picus: \"Every gate that falls is a step towards the one that closes.\"",
                ),
            ],
            epilogue: "At Actium Agrippa's ships break Antony's fleet; Antony and Cleopatra die in Egypt. In 29 BC the doors of the temple of Janus are closed: Rome is at peace on every coast. Octavian will soon be called Augustus. The last Picus writes the last line in the augurs' book: \"The gods guided Rome, and Rome used the gods. Both were true, and the gates are closed.\"",
            deeds: &[(
                Beat::HolyPlace,
                Deed::Bless {
                    n: 5,
                    recruit: true,
                    line: "Your land closes around the old gates of Janus. The young Picus touches the bronze: \"When Rome is at peace everywhere, we close them.\" Veterans of every war come to see them, and five stay to serve you.",
                },
            )],
        },
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    /// Every mission of both campaigns has its part of the story.
    #[test]
    fn every_mission_has_its_tale() {
        for key in crate::missions::CAMPAIGN.into_iter().chain(crate::missions::ROME) {
            let t = tale(key).unwrap_or_else(|| panic!("{key}"));
            assert!(!t.prologue.is_empty() && !t.epilogue.is_empty() && t.beats.len() >= 3, "{key}");
            // the told bits must fit beside the small stories
            assert!(t.beats.len() <= 24, "{key}");
        }
    }

    /// A contact beat names one of the mission's rivals.
    #[test]
    fn contacts_are_real_rivals() {
        for key in crate::missions::CAMPAIGN.into_iter().chain(crate::missions::ROME) {
            let m = crate::missions::mission(key).unwrap();
            for (beat, _) in tale(key).unwrap().beats {
                if let Beat::Contact(p) = beat {
                    assert!((1..=m.rivals.len() as u8).contains(p), "{key}: rival {p}");
                }
            }
        }
    }

    #[test]
    fn beats_come_once_when_due() {
        let t = tale("first").unwrap();
        let mut seen = Seen::default();
        assert!(due(t, &seen, 0).is_empty(), "nothing yet");
        seen.rivals.push(1);
        let now = due(t, &seen, 0);
        assert_eq!(now.len(), 1);
        assert!(now[0].1.starts_with("Aldric's banners"));
        let told = now[0].0;
        assert!(due(t, &seen, told).is_empty(), "told once");
        seen.minute = 30;
        seen.captured = true;
        assert_eq!(due(t, &seen, told).len(), t.beats.len() - 1);
        // the small stories' bits are not the beats'
        assert_eq!(due(t, &seen, 0b111).len(), t.beats.len());
    }

    /// Chapter II (docs/CAMPAIGN-ROME.md): Tarpeia's deeds come once at minute 20, and
    /// Concordia's favour grows while the lands meet, fades slowly otherwise, within 0-100.
    #[test]
    fn tarpeia_comes_once_and_concordia_grows() {
        let t = tale("sabines").unwrap();
        let mut seen = Seen { minute: 19, ..Default::default() };
        assert!(due_deeds(t, &seen, 0).is_empty());
        seen.minute = 20;
        let now = due_deeds(t, &seen, 0);
        assert_eq!(
            now.iter().map(|d| d.1).collect::<Vec<_>>(),
            vec![Deed::HandOver { to: 1, line: TARPEIA }, Deed::War { with: 1 }]
        );
        let done = now.iter().fold(0, |a, d| a | d.0);
        assert!(due_deeds(t, &seen, done).is_empty(), "once");
        assert_eq!(favour_step(10, true, 200), 11);
        assert_eq!(favour_step(10, true, 100), 10, "every 10 s");
        assert_eq!(favour_step(100, true, 200), 100);
        assert_eq!(favour_step(10, false, 1200), 9);
        assert_eq!(favour_step(0, false, 1200), 0);
        // the favour beat
        seen.favour = 50;
        assert!(due(t, &seen, 0).iter().any(|(_, l)| l.starts_with("The women have carried")));
    }

    /// The finale counts the coins: Corvin writes at 20, his ravens come home unpaid at 40.
    #[test]
    fn coins_and_ships_tell_their_beats() {
        let t = tale("mediterranean").unwrap();
        let mut seen = Seen { minted: 19, ..Default::default() };
        assert!(due(t, &seen, 0).is_empty());
        seen.minted = 20;
        assert!(due(t, &seen, 0).iter().any(|(_, l)| l.starts_with("Corvin writes")));
        seen.minted = 40;
        assert_eq!(due(t, &seen, 0).len(), 2);
        let east = tale("fareast").unwrap();
        let ship = Seen { ship: true, ..Default::default() };
        assert!(due(east, &ship, 0).iter().any(|(_, l)| l.starts_with("Your first ship")));
    }
}
