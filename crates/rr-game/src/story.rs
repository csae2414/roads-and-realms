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
}

pub struct Tale {
    /// Above the briefing: where the story stands.
    pub prologue: &'static str,
    pub beats: &'static [(Beat, &'static str)],
    /// On the end screen of a won mission: what it led to.
    pub epilogue: &'static str,
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
        .filter(|(_, (beat, _))| match *beat {
            Beat::Contact(p) => seen.rivals.contains(&p),
            Beat::Minute(m) => seen.minute >= m,
            Beat::FirstCapture => seen.captured,
            Beat::FirstLoss => seen.lost,
            Beat::Minted(n) => seen.minted >= n,
            Beat::FirstShip => seen.ship,
            Beat::HolyPlace => seen.holy,
        })
        .map(|(i, (_, line))| (beat_bit(i), *line))
        .collect()
}

pub fn tale(mission: &str) -> Option<&'static Tale> {
    TALES.iter().find(|(k, _)| *k == mission).map(|(_, t)| t)
}

/// Every line of the story (for the translation's test).
pub fn all_lines() -> impl Iterator<Item = &'static str> {
    TALES.iter().flat_map(|(_, t)| [t.prologue, t.epilogue].into_iter().chain(t.beats.iter().map(|b| b.1)))
}

const TALES: [(&str, Tale); 7] = [
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
                    Beat::Minute(30),
                    "Faustulus, the shepherd who raised you both, comes over from the Aventine: \"Remus watches the sky every morning. He has counted six vultures, he says, and swears he saw them first.\"",
                ),
                // (last in the list: the told bits of older saves keep their beats)
                (
                    Beat::HolyPlace,
                    "Your land lies all around the Capitol now, and Picus climbs to the augurs' platform. Twelve vultures circle over the hill, twice the six Remus saw. \"Jupiter has answered,\" says Picus. \"From up here you will see far.\"",
                ),
            ],
            epilogue: "The hills are yours, and the sign was yours: twelve vultures against six. The city takes your name: Roma. The old tellings disagree about what became of Remus; his people stay and build on the Aventine as Romans. Picus marks the day for the calendar, the twenty-first of April. Across the valley, the Sabines watch the new walls.",
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
