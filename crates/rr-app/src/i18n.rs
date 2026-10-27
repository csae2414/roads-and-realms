//! English/German UI texts. English strings are the keys; `tr` returns the German text when
//! German is active (from `$LANG` or the Options window).

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU8, Ordering};

static LANG: AtomicU8 = AtomicU8::new(0);

pub fn set_german(on: bool) {
    LANG.store(on as u8, Ordering::Relaxed);
}

pub fn is_german() -> bool {
    LANG.load(Ordering::Relaxed) == 1
}

/// German when $LANG/$LC_ALL starts with "de".
pub fn init_from_env() {
    let l = std::env::var("LC_ALL")
        .ok()
        .filter(|v| !v.is_empty())
        .or_else(|| std::env::var("LANG").ok())
        .unwrap_or_default();
    set_german(l.starts_with("de"));
}

/// Does a text have a German translation? (For texts that reach `tr` through a table.)
#[cfg(test)]
pub fn has_german(en: &str) -> bool {
    table().contains_key(en)
}

// a message's template and arguments (`rr_game::msg`: kept in English, shown in any language)
use rr_game::SEP;
pub use rr_game::msg;

/// A kept message in the current language. Plain texts (older saves) are translated if they
/// are a key, else shown as they are.
pub fn show(stored: &str) -> String {
    compose(stored, tr)
}

fn compose<'a>(stored: &'a str, tr: impl Fn(&'a str) -> &'a str) -> String {
    let mut parts = stored.split(SEP);
    let mut s = tr(parts.next().unwrap_or("")).to_string();
    for (i, a) in parts.enumerate() {
        let shown = a.strip_prefix(rr_game::PLAIN).unwrap_or_else(|| tr(a));
        s = s.replace(&format!("{{{}}}", i + 1), shown);
    }
    s
}

/// The message templates (checked by a test, as they don't go through `tr` literally).
#[cfg(test)]
const TEMPLATES: [&str; 91] = [
    "Held by {1}: its god is with them.",
    "{1} of {2}",
    "building site, {1} % done",
    "{1} wares wait here",
    "A geologist's sign: {1}, {2}.",
    "The mine for it: {1}.",
    "The land of {1}.",
    "{1} nodes of our land went to {2}: a post of theirs is nearer.",
    "Planned (⚒): the work starts when the game runs again ({1}).",
    "{1}'s castle fell to {2}: to win the mission now, defeat all rivals.",
    "{1} burned: {2}'s new territory took its land.",
    "On the way: {1}",
    "{1} to building sites",
    "{1} for: {2}",
    "{1} without a goal yet",
    "Rings mark the posts and warehouses of your last rivals ({1}).",
    "Following: {1} (Esc stops)",
    "{1} continues",
    "The attack on {1}'s {2} failed: its defenders held.",
    "{1} min ago",
    "{1} h ago",
    "{1} days ago",
    "{1} min",
    "Your rivals hold {1}× your land. {2}",
    "Your rival holds {1}× your land. {2}",
    "You have {1} military buildings, your rival {2}.",
    "The military settings send {1} soldiers here ({2}). Change it in Main selection, Military.",
    "The military settings send 1 soldier here ({1}). Change it in Main selection, Military.",
    "Belongs to {1}",
    "The truce ends in 5 minutes. 1 of your military buildings holds fewer soldiers than it could: Main selection, Military, raise the troops in the middle and at the border.",
    "Player {1}: only 1 tree near the castle: little wood to cut.",
    "Burned with the lost land: 1 building",
    "{1} sends one soldier against our {2} ({4} inside)",
    "Goal: mint {1} gold coins ({2} so far)",
    "Wheel, {1} / shift-{1}",
    "Ctrl+{1}",
    "Press a key for: {1} (Esc keeps the old one)",
    "{1} is the key of: {2}",
    "{1} can't be used: the game needs it (scrolling, cancelling, the map cursor, Tab, the action window)",
    "Map variant {1} (the seed below picks another)",
    "Player {1}: only {2} trees near the castle: little wood to cut.",
    "Player {1}: no stones near the castle: no stone to build with.",
    "The harbor spot at {1},{2} has no other harbor spot on its water: no ship can sail anywhere from it.",
    "Slot {1} is now slot {2}.",
    "Peace time: no catapults for {1}.",
    "first to mint {1} gold coins",
    "You minted {1} gold coins first: victory.",
    "{1} minted {2} gold coins first.",
    "Goal: mint {1} gold coins first ({2} so far)",
    "Waiting for: {1}.",
    "with {1} % of the land",
    "Victory! You hold {1} % of the land.",
    "Your best on this mission: {1} minutes, score {2}.",
    "Your best at this level: {1} minutes, score {2}",
    "The truce runs {1} more. Attacking ends it, with this rival and its allies.",
    "You broke the truce: {1} will fight back.",
    "{1} knows your coast: expect ships within the first hour.",
    "Time is up! {1} holds the most land.",
    "Peace time: no attacks for {1}.",
    "Goal: defeat all rivals ({1} left)",
    "Goal: take the castle of {1}",
    "Goal: hold {1} % of the land ({2} % now)",
    "Goal: hold out {1} minutes ({2} left)",
    "Goal: make {1} {2} ({3} so far)",
    "Your rivals leave you alone for the first {1} minutes.",
    "Your rivals leave you alone for the first {1} minutes (see the clock at the bottom right). Use them to build up and to man your borders.",
    "{1} {2}{3} {4}",
    "💡 {1}",
    "{1} finished",
    "{1} lost",
    "{1} lost to {2}: its wares and settlers are gone with it.",
    "{1} lost: its wares and settlers are gone with it.",
    "{1} lost to {2}: its wares and settlers are gone with it. You have no warehouse left: nothing new can be built.",
    "{1} lost: its wares and settlers are gone with it. You have no warehouse left: nothing new can be built.",
    "Geologist found {1}",
    "Geologist found {1} ({2})",
    "{1} has been defeated",
    "Not possible: {1}",
    "{1}: {2}",
    "{1} attacks our {2}",
    "{1} sends {3} soldiers against our {2} ({4} inside)",
    "{1} took our {2}",
    "Burned with the lost land: {1} buildings",
    "Your soldiers: {1} in warehouses (they defend only those), {2} in military buildings.",
    "By rank: {1}. Together {2} soldiers with those on the way, {3} points in the chart.",
    "💡 Your castle holds {1} soldiers, but they only defend the castle. A guardhouse holds 3, a watchtower 6, a fortress 9: put strong buildings where your rivals' land touches yours.",
    "We took {1}'s {2}",
    "You have {1} military buildings, your rivals about {2} each.",
    "{1} boards and {2} stones lie in store and nothing is being built: enough for {3} guardhouses. Place more buildings, military ones at the border first.",
    "{1} boards and {2} stones lie in store and nothing is being built. No soldier waits to man a new military building: brew beer, make swords and shields, and build workshops meanwhile.",
    "The truce ends in 5 minutes. {1} of your military buildings hold fewer soldiers than they could: Main selection, Military, raise the troops in the middle and at the border.",
];

fn table() -> &'static HashMap<&'static str, &'static str> {
    static T: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    T.get_or_init(|| DE.iter().copied().collect())
}

/// A percentage as the language writes it: "25%" in English, "25 %" in German.
pub fn pct(n: impl std::fmt::Display) -> String {
    pct_in(n, is_german())
}

fn pct_in(n: impl std::fmt::Display, german: bool) -> String {
    if german { format!("{n} %") } else { format!("{n}%") }
}

/// A template filled in now, in the current language (arguments that are keys are translated).
pub fn fill(template: &str, args: &[&str]) -> String {
    show(&msg(template, args))
}

/// The German text of a key, whatever the language now (for tests elsewhere).
#[cfg(test)]
pub fn german(en: &str) -> Option<&'static str> {
    table().get(en).copied()
}

pub fn tr(s: &str) -> &str {
    if !is_german() {
        return s;
    }
    table().get(s).copied().unwrap_or(s)
}

/// German texts, by English key.
///
/// Glossary (German round 1, #1): one German word per concept, used in every text.
/// - rival, opponent: Gegner (never "Rivale")
/// - truce (the campaign's): Waffenruhe (never "Waffenstillstand"); the free game's peace time:
///   Friedenszeit
/// - castle / headquarters: Burg (the editor's texts included)
/// - guardhouse: Wachstube; watchtower: Wachturm; fortress: Festung; lookout tower: Spähturm
/// - fishery: Fischer (the building, as in the original); farm: Bauernhof; armory: Waffenschmiede
/// - scout: Späher; messages (window and key): Nachrichten
/// - map node: Punkt ("4 Punkte Abstand")
/// - ranks: Gefreiter, Obergefreiter, Unteroffizier, Offizier, General
/// - Bjorn: Björn
///
/// Style: "du" (never "ihr" or "Sie"); imperatives with -e ("greife", "schicke", "stelle",
/// "mache"); a capital after a colon when a full sentence follows; „…“ for quoted button names;
/// "25 %" with a space; "Min." and "Std.". Counts that can be 1 get a singular template of
/// their own, and templates filled with building names don't depend on their gender.
const DE: &[(&str, &str)] = &[
    (
        "Your land lies all around the Capitol now, and Picus climbs to the augurs' platform. Twelve vultures circle over the hill, twice the six Remus saw. \"Jupiter has answered,\" says Picus. \"From up here you will see far.\"",
        "Dein Land liegt jetzt rund um das Kapitol, und Picus steigt auf die Plattform der Auguren. Zwölf Geier kreisen über dem Hügel, doppelt so viele, wie Remus gesehen hat. „Jupiter hat geantwortet“, sagt Picus. „Von hier oben wirst du weit sehen.“",
    ),
    // the map's hover (hover.rs)
    ("Jupiter's augur platform, a holy place", "Jupiters Auguren-Plattform, ein heiliger Ort"),
    ("A holy place", "Ein heiliger Ort"),
    (
        "Yours: Jupiter's birds show you the land far around the hill.",
        "Deiner: Jupiters Vögel zeigen dir das Land weit um den Hügel.",
    ),
    ("Yours: its god is with you.", "Deiner: Sein Gott ist mit dir."),
    ("Held by {1}: its god is with them.", "Gehalten von {1}: Sein Gott steht auf dieser Seite."),
    (
        "Contested: a border runs through its ring, so nobody may use it.",
        "Umkämpft: Eine Grenze läuft durch seinen Ring, darum kann ihn niemand nutzen.",
    ),
    (
        "Nobody's: close your border all around it to win its god.",
        "Niemandes: Schließe deine Grenze ganz um ihn, um seinen Gott zu gewinnen.",
    ),
    ("A road", "Eine Straße"),
    ("A donkey road", "Eine Eselstraße"),
    ("A boat road", "Eine Bootsstraße"),
    ("its carrier is on the way", "ihr Träger ist unterwegs"),
    ("{1} of {2}", "{1} von {2}"),
    ("building site, {1} % done", "Baustelle, {1} % fertig"),
    ("1 ware waits here", "1 Ware wartet hier"),
    ("{1} wares wait here", "{1} Waren warten hier"),
    (
        "A young tree: a woodcutter fells it once it is grown",
        "Ein junger Baum: Ein Holzfäller fällt ihn, wenn er ausgewachsen ist",
    ),
    ("A tree: wood for a woodcutter", "Ein Baum: Holz für einen Holzfäller"),
    ("Rocks: stone for a quarry", "Felsen: Steine für einen Steinbruch"),
    ("A grain field", "Ein Getreidefeld"),
    ("A burning ruin", "Eine brennende Ruine"),
    ("A ship being built", "Ein Schiff im Bau"),
    ("Something that is in the way", "Etwas, das im Weg steht"),
    ("A geologist's sign: nothing found here.", "Ein Schild des Geologen: Hier wurde nichts gefunden."),
    (
        "A geologist's sign: water. A well finds it here.",
        "Ein Schild des Geologen: Wasser. Ein Brunnen findet es hier.",
    ),
    ("A geologist's sign: {1}, {2}.", "Ein Schild des Geologen: {1}, {2}."),
    ("The mine for it: {1}.", "Das Bergwerk dafür: {1}."),
    ("a lot", "viel"),
    ("some", "etwas"),
    ("a little", "wenig"),
    ("No one's land: push your border here to build.", "Niemandsland: Schiebe deine Grenze hierher, um zu bauen."),
    ("The land of {1}.", "Das Land von {1}."),
    ("An ally's land.", "Das Land eines Verbündeten."),
    ("Only a flag fits here.", "Hier passt nur eine Fahne."),
    ("Room for a small building (or a flag).", "Platz für ein kleines Gebäude (oder eine Fahne)."),
    ("Room for a medium or a small building.", "Platz für ein mittleres oder kleines Gebäude."),
    ("Room for a large building, or anything smaller.", "Platz für ein großes Gebäude oder alles Kleinere."),
    ("Room for a mine.", "Platz für ein Bergwerk."),
    ("Room for a harbor, or any other building.", "Platz für einen Hafen oder jedes andere Gebäude."),
    ("No military building: one stands too near.", "Kein Militärgebäude: Eines steht zu nah."),
    // the Roman campaign (rr_game::missions::ROME_MISSIONS, rr_game::story), chapter 1
    ("The Roman campaign", "Die römische Kampagne"),
    ("Win the chapter before it first.", "Gewinne zuerst das Kapitel davor."),
    ("The Augury", "Die Vogelschau"),
    ("Romulus", "Romulus"),
    ("Remus", "Remus"),
    ("Goal: hold 35 % of the land.", "Ziel: Halte 35 % des Landes."),
    (
        "The Roman campaign begins: build, connect and grow over the seven hills.",
        "Die römische Kampagne beginnt: Baue, verbinde und wachse über die sieben Hügel.",
    ),
    (
        "Two brothers stand on two hills above the Tiber, Romulus and Remus. Both want to found the city, and they leave it to the gods: each watches the sky from his own hill for a sign. You are Romulus, on the Palatine. Remus builds on the Aventine, at peace with you.\n\nHere you learn how a realm grows: woodcutters and a quarry, a sawmill, roads that carry everything, and guardhouses that push your border over the hills. Whoever holds the hills will hold the city.",
        "Zwei Brüder stehen auf zwei Hügeln über dem Tiber, Romulus und Remus. Beide wollen die Stadt gründen, und sie überlassen es den Göttern: Jeder beobachtet von seinem Hügel aus den Himmel und wartet auf ein Zeichen. Du bist Romulus, auf dem Palatin. Remus baut auf dem Aventin und hält Frieden mit dir.\n\nHier lernst du, wie ein Reich wächst: Holzfäller und ein Steinbruch, ein Sägewerk, Straßen, die alles tragen, und Wachstuben, die deine Grenze über die Hügel schieben. Wer die Hügel hält, wird die Stadt halten.",
    ),
    (
        "Rome is not yet a city. On the hills above the Tiber two brothers, raised by a shepherd, each want to found one: Romulus on the Palatine, Remus on the Aventine. They have agreed to let the gods decide. Each will watch the sky from his own hill, and whoever is sent the better sign will found the city. Picus, the old augur, stands beside you.",
        "Rom ist noch keine Stadt. Auf den Hügeln über dem Tiber will jeder von zwei Brüdern, die ein Hirte großgezogen hat, eine gründen: Romulus auf dem Palatin, Remus auf dem Aventin. Sie haben vereinbart, die Götter entscheiden zu lassen. Jeder beobachtet den Himmel von seinem eigenen Hügel aus, und wer das bessere Zeichen erhält, gründet die Stadt. Picus, der alte Augur, steht an deiner Seite.",
    ),
    (
        "Remus's banner stands on the Aventine. His herald calls across the valley: \"My brother builds on his hill, I on mine. Let the birds decide which one is Rome.\"",
        "Das Banner des Remus steht auf dem Aventin. Sein Herold ruft über das Tal: „Mein Bruder baut auf seinem Hügel, ich auf meinem. Die Vögel sollen entscheiden, welcher Rom ist.“",
    ),
    (
        "Picus leans on his curved staff: \"The gods answer from the sky, Romulus. But a sign is worth nothing on a hill that no one holds.\"",
        "Picus stützt sich auf seinen gekrümmten Stab: „Die Götter antworten vom Himmel, Romulus. Aber ein Zeichen ist nichts wert auf einem Hügel, den niemand hält.“",
    ),
    (
        "Picus points across the hills: \"Seven hills, and the river at their feet. Every hill you hold is one more that the city will stand on.\"",
        "Picus zeigt über die Hügel: „Sieben Hügel, und der Fluss zu ihren Füßen. Jeder Hügel, den du hältst, ist einer mehr, auf dem die Stadt stehen wird.“",
    ),
    (
        "Faustulus, the shepherd who raised you both, comes over from the Aventine: \"Remus watches the sky every morning. He has counted six vultures, he says, and swears he saw them first.\"",
        "Faustulus, der Hirte, der dich und Remus großgezogen hat, kommt vom Aventin herüber: „Remus beobachtet jeden Morgen den Himmel. Sechs Geier hat er gezählt, sagt er, und er schwört, er habe sie zuerst gesehen.“",
    ),
    (
        "The hills are yours, and the sign was yours: twelve vultures against six. The city takes your name: Roma. The old tellings disagree about what became of Remus; his people stay and build on the Aventine as Romans. Picus marks the day for the calendar, the twenty-first of April. Across the valley, the Sabines watch the new walls.",
        "Die Hügel gehören dir, und das Zeichen war deines: zwölf Geier gegen sechs. Die Stadt trägt deinen Namen: Roma. Was aus Remus wurde, erzählen die alten Geschichten verschieden; seine Leute bleiben und bauen als Römer auf dem Aventin. Picus trägt den Tag in den Kalender ein, den einundzwanzigsten April. Jenseits des Tals betrachten die Sabiner die neuen Mauern.",
    ),
    // the campaign's story (rr_game::story), in the order it is told
    (
        "Mira the map-maker is old, and every hall laughs at her map of the world: lands beyond the sea, mountains marked with little crowns of gold, a sea in the middle of the earth. Only your house believes her. Before you sail for the lands she drew, she asks for one thing: show her you can build a realm, here in the valley.",
        "Mira, die Kartenzeichnerin, ist alt, und jede Halle lacht über ihre Karte der Welt: Länder jenseits des Meeres, Berge mit kleinen Goldkronen, ein Meer in der Mitte der Erde. Nur dein Haus glaubt ihr. Bevor du zu den Ländern segelst, die sie gezeichnet hat, verlangt sie nur eines: Zeig ihr, dass du ein Reich aufbauen kannst, hier im Tal.",
    ),
    (
        "Aldric's banners on the ridge. His rider brings a friendly word: \"The valley is big enough for two, neighbour.\" Mira frowns: \"Aldric was never this friendly to anyone.\"",
        "Aldrics Banner auf dem Kamm. Sein Reiter bringt ein freundliches Wort: „Das Tal ist groß genug für zwei, Nachbar.“ Mira runzelt die Stirn: „So freundlich war Aldric noch zu niemandem.“",
    ),
    (
        "A letter for you, sealed with a black raven: \"I hear your house keeps an old woman's map. I collect such curiosities. Name your price; I pay in silver. Corvin, of the Raven Guild.\" Mira tears it in two.",
        "Ein Brief für dich, mit einem schwarzen Raben gesiegelt: „Wie ich höre, bewahrt dein Haus die Karte einer alten Frau. Ich sammle solche Kuriositäten. Nenne deinen Preis, ich zahle in Silber. Corvin, von der Rabengilde.“ Mira reißt ihn entzwei.",
    ),
    (
        "Mira, from her window: \"Aldric's new walls are stone from far away, and his men are paid in good silver. Where does a poor marcher lord find so much?\"",
        "Mira, an ihrem Fenster: „Aldrics neue Mauern sind aus Stein von weit her, und seine Leute werden in gutem Silber bezahlt. Woher hat ein armer Markgraf so viel?“",
    ),
    (
        "In the post you took lies a purse of silver, sealed with the same black raven as Corvin's letter.",
        "In dem Militärgebäude, das du erobert hast, liegt ein Beutel Silber, gesiegelt mit demselben schwarzen Raben wie Corvins Brief.",
    ),
    (
        "Aldric's keys lie on your table. In his chest are Corvin's letters: the Guild paid for every wall, and asked for one thing in return, Mira's map. Mira rolls it out and shows you why. Beside each little crown she wrote how deep the gold lies, and where the rivers can be crossed: \"Whoever holds this knows where every realm will grow rich, before the realm knows it. Get there first.\"",
        "Aldrics Schlüssel liegen auf deinem Tisch. In seiner Truhe liegen Corvins Briefe: Die Gilde hat jede Mauer bezahlt und dafür nur eines verlangt, Miras Karte. Mira rollt sie aus und zeigt dir, warum. Neben jede kleine Krone hat sie geschrieben, wie tief das Gold liegt und wo man die Flüsse überqueren kann: „Wer das besitzt, weiß, wo jedes Reich reich werden wird, bevor das Reich es selbst weiß. Sei als Erster dort.“",
    ),
    (
        "Mira's first mark lies on the hills of Italy, and the Guild got there before you. Bjorn of Germania, the Guild's sword, has sworn that no one else will rule beyond the Alps. Mira puts her finger on the little crown she drew in the mountains: \"Gold. Find it before he does.\"",
        "Miras erstes Zeichen liegt auf den Hügeln Italiens, und die Gilde war schneller. Björn von Germanien, das Schwert der Gilde, hat geschworen, dass jenseits der Alpen kein anderer herrscht. Mira legt den Finger auf die kleine Krone, die sie in die Berge gezeichnet hat: „Gold. Finde es vor ihm.“",
    ),
    (
        "Bjorn's scouts wear the black raven on their shields. Livia of Iberia sends a quiet word: she has no quarrel with you. Not yet.",
        "Björns Späher tragen den schwarzen Raben auf den Schilden. Livia von Iberien lässt dir leise ausrichten, dass sie keinen Streit mit dir hat. Noch nicht.",
    ),
    (
        "Hadrian of Sarmatia's banners fly beside Bjorn's raven. The Guild has bought him too, and for less.",
        "Die Banner von Hadrian von Sarmatien wehen neben Björns Raben. Auch ihn hat die Gilde gekauft, und billiger.",
    ),
    (
        "The first coin of Alpine gold rings on your table. Mira's little crown was right, to the depth she wrote beside it.",
        "Die erste Münze aus Alpengold klingt auf deinem Tisch. Miras kleine Krone hatte recht, bis auf die Tiefe, die sie danebengeschrieben hat.",
    ),
    (
        "Another raven letter: \"Leave Germania to Bjorn and the map to me, and the Guild will pay your soldiers for ten years. Refuse, and it will pay his. C.\"",
        "Wieder ein Rabenbrief: „Überlass Germanien Björn und die Karte mir, und die Gilde bezahlt deine Soldaten zehn Jahre lang. Lehnst du ab, bezahlt sie seine. C.“",
    ),
    (
        "A post of yours has fallen to men paid in Guild silver. Mira: \"Every soldier Bjorn sends is bought, except Bjorn. Take him, and the Guild has no sword left.\"",
        "Eines deiner Militärgebäude ist an Männer gefallen, die mit dem Silber der Gilde bezahlt werden. Mira: „Jeder Soldat, den Björn schickt, ist gekauft, nur Björn nicht. Nimm ihn, und die Gilde hat kein Schwert mehr.“",
    ),
    (
        "Bjorn's hall burns, and the Guild has lost its sword: no one in the north will fight under the raven now. Among Bjorn's maps lies a copy of Mira's own, line for line, every crown in its place. Someone gave the Guild her work. Mira is silent for a long time, then points across the ocean: \"My second mark. If they have my map, they are already there.\"",
        "Björns Halle brennt, und die Gilde hat ihr Schwert verloren: Im Norden kämpft niemand mehr unter dem Raben. Unter Björns Karten liegt eine Abschrift von Miras eigener, Linie für Linie, jede Krone an ihrem Platz. Jemand hat der Gilde ihre Arbeit gegeben. Mira schweigt lange, dann zeigt sie über den Ozean: „Mein zweites Zeichen. Wenn sie meine Karte haben, sind sie schon dort.“",
    ),
    (
        "Across the ocean, Mira's map was right: a wide land, rivers and plains, and Virginia's coast where your ships land. The Guild was here first, selling land it has never seen. Four of the five rulers hold Guild deeds, and every deed promises the gold of the West.",
        "Jenseits des Ozeans hatte Miras Karte recht: ein weites Land, Flüsse und Ebenen und die Küste Virginias, an der deine Schiffe landen. Die Gilde war vor dir hier und verkauft Land, das sie nie gesehen hat. Vier der fünf Herrscher haben Urkunden der Gilde, und jede Urkunde verspricht das Gold des Westens.",
    ),
    (
        "Ezra of New England holds up a deed with the raven seal: \"This coast was sold to me. Leave, or pay.\"",
        "Ezra von Neuengland hält eine Urkunde mit dem Rabensiegel hoch: „Diese Küste wurde mir verkauft. Geh, oder zahle.“",
    ),
    (
        "Santiago of Texas laughs at the Guild's deeds: \"Paper. Land is held, not bought.\" On that, at least, you agree.",
        "Santiago von Texas lacht über die Urkunden der Gilde: „Papier. Land wird gehalten, nicht gekauft.“ Wenigstens darin stimmst du ihm zu.",
    ),
    (
        "Every deed the Guild sold here is drawn from Mira's map. Mira looks at the coastlines for a long time: \"Whoever copied this learned to draw from me.\"",
        "Jede Urkunde, die die Gilde hier verkauft hat, ist nach Miras Karte gezeichnet. Mira betrachtet die Küstenlinien lange: „Wer das abgezeichnet hat, hat bei mir zeichnen gelernt.“",
    ),
    (
        "A raven letter, friendlier than the last: \"Why fight for land? The Guild has a deed for all of it. Buy one.\" The deed in the envelope is for Virginia. Your Virginia.",
        "Ein Rabenbrief, freundlicher als der letzte: „Warum um Land kämpfen? Die Gilde hat für alles eine Urkunde. Kauf eine.“ Die Urkunde im Umschlag ist für Virginia. Dein Virginia.",
    ),
    (
        "In the post you took, a Guild deed hangs framed on the wall. Its soldiers ran when the Guild's pay did not come. A deed, it turns out, holds nothing.",
        "In dem Militärgebäude, das du erobert hast, hängt eine Urkunde der Gilde gerahmt an der Wand. Seine Soldaten sind davongelaufen, als der Sold der Gilde ausblieb. Eine Urkunde, so zeigt sich, hält gar nichts.",
    ),
    (
        "A third of the continent is yours, and no one here will buy a Guild deed again. A captain of Ezra's tells you where the charts came from: a young map-maker called Tove, who sailed north to Svealand with Guild silver. Mira goes pale. Tove was her apprentice.",
        "Ein Drittel des Kontinents gehört dir, und hier kauft niemand mehr eine Urkunde der Gilde. Ein Kapitän Ezras erzählt dir, woher die Karten stammen: von einer jungen Kartenzeichnerin namens Tove, die mit dem Silber der Gilde nach Svealand gesegelt ist. Mira wird bleich. Tove war ihre Schülerin.",
    ),
    (
        "Winter, and Svealand. Tove is somewhere in these woods with Mira's charts, and the Guild has paid the northern lords to drive you out before the ice breaks. You don't have to win here: hold on until spring.",
        "Winter in Svealand. Tove ist irgendwo in diesen Wäldern, mit Miras Karten, und die Gilde hat die Herren des Nordens bezahlt, dich zu vertreiben, bevor das Eis bricht. Hier musst du nicht siegen: Halte durch bis zum Frühling.",
    ),
    (
        "Halvard of Norvegr's men carry Guild silver and Guild axes. They will come when the snow lies deepest.",
        "Die Leute von Halvard von Norvegr tragen Silber der Gilde und Äxte der Gilde. Sie kommen, wenn der Schnee am tiefsten liegt.",
    ),
    (
        "A raven letter with no greeting: \"Svealand is a cold place to die. Send me the old woman, and the snow can keep your village. C.\" Mira reads it and laughs for the first time in weeks.",
        "Ein Rabenbrief ohne Gruß: „Svealand ist ein kalter Ort zum Sterben. Schicke mir die alte Frau, und der Schnee lässt dir dein Dorf. C.“ Mira liest ihn und lacht zum ersten Mal seit Wochen.",
    ),
    (
        "A third of the winter is behind you. At night a fire burns alone on the hill across the river: someone is watching your village and not attacking it.",
        "Ein Drittel des Winters liegt hinter dir. Nachts brennt auf dem Hügel jenseits des Flusses ein einzelnes Feuer: Jemand beobachtet dein Dorf und greift es nicht an.",
    ),
    (
        "A post is lost in the snow. In the morning a sled stands at your gate with bread and no driver.",
        "Ein Militärgebäude ist im Schnee verloren. Am Morgen steht ein Schlitten mit Brot vor deinem Tor, ohne Fahrer.",
    ),
    (
        "The fire on the hill is Tove's. She sends a note: \"Corvin promised me my name on the map of the world. I see now he only wants the map.\"",
        "Das Feuer auf dem Hügel gehört Tove. Sie schickt eine Nachricht: „Corvin hat mir meinen Namen auf der Karte der Welt versprochen. Jetzt sehe ich, dass er nur die Karte will.“",
    ),
    (
        "Halvard's captains count their dead, and ask why they should freeze for a merchant who has never seen snow.",
        "Halvards Hauptleute zählen ihre Toten und fragen, warum sie für einen Händler frieren sollen, der nie Schnee gesehen hat.",
    ),
    (
        "The ice breaks, and the Guild's men go home: not one more would freeze for Corvin. Tove comes to your gate alone, with Mira's charts and Corvin's letters to her. They tell where his power still lies: in the banks of Jiangnan at the end of the silk road, which lend him all he spends, and in the harbors of the sea in the middle of the earth. Mira takes her back. \"A map-maker should know the way home.\"",
        "Das Eis bricht, und die Leute der Gilde gehen nach Hause: Kein einziger will mehr für Corvin frieren. Tove kommt allein an dein Tor, mit Miras Karten und Corvins Briefen an sie. Sie verraten, wo seine Macht noch liegt: in den Banken von Jiangnan am Ende der Seidenstraße, die ihm alles leihen, was er ausgibt, und in den Häfen des Meeres in der Mitte der Erde. Mira nimmt sie wieder auf. „Wer Karten zeichnet, sollte den Weg nach Hause kennen.“",
    ),
    (
        "Mira's map ends far in the east, on the Yellow River. Here is the Guild's purse: Lianhua of Jiangnan lends to Corvin, and every army he bought was bought with her money. Tove rides with you now. She knows the roads to Jiangnan; she drew them.",
        "Miras Karte endet weit im Osten, am Gelben Fluss. Hier ist der Geldbeutel der Gilde: Lianhua von Jiangnan leiht Corvin Geld, und jedes Heer, das er gekauft hat, wurde mit ihrem Geld gekauft. Tove reitet jetzt mit dir. Sie kennt die Wege nach Jiangnan, sie hat sie gezeichnet.",
    ),
    (
        "Lianhua's envoy brings silk and an offer: half of the south, if you turn back. Behind the envoy rides a man with a raven ring.",
        "Lianhuas Gesandter bringt Seide und ein Angebot: den halben Süden, wenn du umkehrst. Hinter dem Gesandten reitet ein Mann mit einem Rabenring.",
    ),
    (
        "Hideko of Yamato's ships are on the coast. Tove knows them: \"Yamato sails with the Guild's friends, but her soldiers can only come by sea. Guard your shores.\"",
        "Die Schiffe von Hideko von Yamato liegen vor der Küste. Tove kennt sie: „Yamato segelt mit den Freunden der Gilde, aber ihre Soldaten können nur übers Meer kommen. Bewache deine Küsten.“",
    ),
    (
        "A raven letter, for Tove this time: \"Come back, and your name goes on the map after all. In big letters.\" Tove burns it without a word.",
        "Ein Rabenbrief, diesmal für Tove: „Komm zurück, und dein Name kommt doch noch auf die Karte. In großen Buchstaben.“ Tove verbrennt ihn wortlos.",
    ),
    (
        "The first coin of Qinling gold. Tove draws a new little crown on Mira's map, in her own hand, and Mira lets her.",
        "Die erste Münze aus dem Gold des Qinling. Tove zeichnet eine neue kleine Krone auf Miras Karte, mit eigener Hand, und Mira lässt sie.",
    ),
    (
        "Your first ship is launched. Tove watches it go: \"Now Yamato is not the only one who can cross.\"",
        "Dein erstes Schiff läuft vom Stapel. Tove sieht ihm nach: „Jetzt ist Yamato nicht mehr das einzige Reich, das übers Meer kommt.“",
    ),
    (
        "In the post you took, a Guild courier has left his satchel behind: Lianhua's ledgers. Every loan she made to Corvin runs on to one place, the harbors of the sea in the middle of the earth.",
        "In dem Militärgebäude, das du erobert hast, hat ein Bote der Gilde seine Tasche zurückgelassen: Lianhuas Bücher. Jedes Darlehen, das sie Corvin gegeben hat, führt weiter an einen Ort, die Häfen des Meeres in der Mitte der Erde.",
    ),
    (
        "Jiangnan's banks are closed to the Guild: Corvin can borrow nothing more. Lianhua, captured, shrugs: \"He owes me more than you do. Go to the sea in the middle of the earth. He sits in its harbors, and he has never fought a battle in his life. He buys them, with what he has left.\"",
        "Die Banken von Jiangnan sind für die Gilde geschlossen: Corvin kann sich nichts mehr leihen. Die gefangene Lianhua zuckt mit den Schultern: „Er schuldet mir mehr als du. Geh ans Meer in der Mitte der Erde. Er sitzt in seinen Häfen, und er hat in seinem Leben noch keine Schlacht geschlagen. Er kauft sie, mit dem, was ihm geblieben ist.“",
    ),
    (
        "The last mark on Mira's map: the sea in the middle of the earth. Your ships land at Carthage, an old harbor the Guild overlooked. Corvin has no sword and no bank left, only the silver in his vaults, and every power on this sea still takes his coin. Swords will not beat him. Gold will: mint more than he has, and no one on this sea will want his.",
        "Das letzte Zeichen auf Miras Karte: das Meer in der Mitte der Erde. Deine Schiffe landen in Karthago, einem alten Hafen, den die Gilde übersehen hat. Corvin hat kein Schwert und keine Bank mehr, nur das Silber in seinen Gewölben, und jede Macht an diesem Meer nimmt noch seine Münze. Mit Schwertern besiegst du ihn nicht. Mit Gold schon: Präge mehr, als er hat, und an diesem Meer will niemand mehr seine.",
    ),
    (
        "Cornelia of Roma sends a merchant's greeting: \"The Guild pays well for Carthaginian harbors. Make me a better offer.\"",
        "Cornelia von Rom schickt einen Händlergruß: „Die Gilde zahlt gut für karthagische Häfen. Mache mir ein besseres Angebot.“",
    ),
    (
        "Nefret of Aegyptus has taken Corvin's silver to land her soldiers on your coast. He is emptying his vaults to stop you.",
        "Nefret von Ägypten hat Corvins Silber genommen, um ihre Soldaten an deiner Küste zu landen. Er leert seine Gewölbe, um dich aufzuhalten.",
    ),
    (
        "Corvin writes in his own hand now, without the raven: \"I could have bought you once. What do you cost today?\" Mira answers for you: \"The map is not for sale.\"",
        "Corvin schreibt jetzt mit eigener Hand, ohne den Raben: „Einst hätte ich dich kaufen können. Was kostest du heute?“ Mira antwortet für dich: „Die Karte ist nicht zu verkaufen.“",
    ),
    (
        "Tove copies Mira's map one last time, clean, every crown and every ford: \"When this is over, someone has to keep it.\"",
        "Tove zeichnet Miras Karte ein letztes Mal ab, sauber, jede Krone und jede Furt: „Wenn das vorbei ist, muss jemand sie hüten.“",
    ),
    (
        "In every harbor the merchants weigh your coins against the Guild's, and take yours. Corvin's ravens come home unpaid.",
        "In jedem Hafen wiegen die Händler deine Münzen gegen die der Gilde und nehmen deine. Corvins Raben kehren unbezahlt heim.",
    ),
    (
        "Your sixtieth coin, and the Guild's coin is worth nothing on this sea. Corvin comes to your gate himself: a small man in a black cloak, with nothing left to offer. He asks only to see the map, once. Mira, very old now, shows it to him. Then she draws one last line on it, round your realm, and gives the pen to Tove.",
        "Deine sechzigste Münze, und die Münze der Gilde ist an diesem Meer nichts mehr wert. Corvin kommt selbst an dein Tor: ein kleiner Mann in einem schwarzen Mantel, der nichts mehr zu bieten hat. Er bittet nur darum, die Karte einmal zu sehen. Mira, nun sehr alt, zeigt sie ihm. Dann zieht sie eine letzte Linie darauf, um dein Reich, und gibt Tove die Feder.",
    ),
    // bar and windows
    ("🏠 Build help", "🏠 Bauhilfe"),
    ("📦 Inventory", "📦 Inventur"),
    ("⚖ Distribution", "⚖ Verteilung"),
    ("🚚 Transport", "🚚 Transport"),
    ("🔨 Tools", "🔨 Werkzeuge"),
    ("⚔ Military", "⚔ Militär"),
    ("📈 Statistics", "📈 Statistik"),
    ("✉ Messages", "✉ Nachrichten"),
    ("🗺 Map", "🗺 Karte"),
    ("❓ Help", "❓ Hilfe"),
    ("⚙ Options", "⚙ Optionen"),
    ("Menu", "Menü"),
    ("Save", "Speichern"),
    ("Load", "Laden"),
    ("Inventory", "Inventur"),
    ("Distribution of goods", "Warenverteilung"),
    ("Transport", "Transport"),
    ("Tools", "Werkzeuge"),
    ("Military", "Militär"),
    ("Statistics", "Statistik"),
    ("Messages", "Nachrichten"),
    ("Map", "Karte"),
    ("Help", "Hilfe"),
    ("Options", "Optionen"),
    ("Music", "Musik"),
    ("Sounds", "Geräusche"),
    ("Volume", "Lautstärke"),
    (
        "Each gold coin promotes one soldier of every rank present here by one rank.",
        "Jede Goldmünze befördert hier je einen Soldaten jedes vorhandenen Rangs um einen Rang.",
    ),
    ("How the economy works", "So funktioniert die Wirtschaft"),
    (
        "Wood goes to a sawmill and becomes boards; stones from a quarry: every building needs boards and stones.",
        "Holz wird im Sägewerk zu Brettern; Steine aus dem Steinbruch: Jedes Gebäude braucht Bretter und Steine.",
    ),
    (
        "Food (fish, meat, bread) feeds the mines; coal and iron make tools and weapons.",
        "Nahrung (Fisch, Fleisch, Brot) versorgt die Minen; Kohle und Eisen ergeben Werkzeuge und Waffen.",
    ),
    (
        "Beer, a sword and a shield make a soldier; gold coins promote soldiers.",
        "Bier, ein Schwert und ein Schild machen einen Soldaten; Goldmünzen befördern Soldaten.",
    ),
    (
        "Military buildings are greyed out: they keep 4 nodes from the castle and from each other. Try further out.",
        "Militärgebäude sind ausgegraut: Zur Burg und untereinander brauchen sie 4 Punkte Abstand. Versuche es weiter draußen.",
    ),
    ("no enemy near", "kein Feind in der Nähe"),
    ("an enemy within reach", "ein Feind in Reichweite"),
    ("a harbor to guard", "ein Hafen zu schützen"),
    ("an enemy close by", "ein Feind ganz nah"),
    ("(one mark per soldier, by rank)", "(ein Zeichen je Soldat, nach Rang)"),
    (
        "Dark: land nobody of yours has seen yet. Dots: where your rivals rule.",
        "Dunkel: Land, das noch keiner deiner Leute gesehen hat. Punkte: wo deine Gegner herrschen.",
    ),
    (
        "Tip: wares pile up at a flag: its roads are too long or too busy. Put more flags on long roads (each part gets its own carrier), or build a second road.",
        "Tipp: An einer Fahne stauen sich Waren. Die Straßen dort sind zu lang oder überlastet. Setze mehr Fahnen auf lange Straßen (jedes Stück bekommt einen eigenen Träger) oder baue eine zweite Straße.",
    ),
    (
        "Tip: swords and shields wait, but there is no beer: a brewery (grain and water) turns them into soldiers.",
        "Tipp: Schwerter und Schilde warten, aber es gibt kein Bier. Eine Brauerei (Getreide und Wasser) macht daraus Soldaten.",
    ),
    ("Why is it slow?", "Warum geht es so langsam?"),
    (
        "Every road between two flags has one carrier. Long roads with few flags carry little: put a flag every two or three steps.",
        "Jede Straße zwischen zwei Fahnen hat einen Träger. Lange Straßen mit wenigen Fahnen tragen wenig: Setze alle zwei, drei Schritte eine Fahne.",
    ),
    (
        "Wares piling up at a flag mean its roads are busy: build a second road around it.",
        "Stauen sich Waren an einer Fahne, sind ihre Straßen überlastet: Baue eine zweite Straße daran vorbei.",
    ),
    (
        "Far from the castle everything takes long: a storehouse in the middle of your land keeps wares near.",
        "Weit weg von der Burg dauert alles lange: Ein Lagerhaus mitten in deinem Land hält Waren nah.",
    ),
    (
        "A building that waits for wares says so in its window (click it); O shows how well buildings work.",
        "Ein Gebäude, das auf Waren wartet, sagt es in seinem Fenster (anklicken); O zeigt, wie gut Gebäude arbeiten.",
    ),
    ("To the end", "Bis zum Ende"),
    ("Skip ten minutes", "Zehn Minuten überspringen"),
    ("Player names", "Spielernamen"),
    ("minutes of peace", "Minuten Frieden"),
    ("Briefing", "Einweisung"),
    ("Goal", "Ziel"),
    ("Title", "Titel"),
    (
        "Shown before a game on this map; saved next to it.",
        "Wird vor einem Spiel auf dieser Karte gezeigt; neben ihr gespeichert.",
    ),
    ("Mission", "Mission"),
    ("Rocks", "Felsen"),
    ("Flowers", "Blumen"),
    ("Bushes", "Büsche"),
    ("Decoration", "Deko"),
    ("One triangle at a time", "Ein Dreieck nach dem anderen"),
    ("You play:", "Du spielst:"),
    ("Open a map", "Karte öffnen"),
    ("No map files yet.", "Noch keine Kartendateien."),
    ("Export .swd", "Als .swd exportieren"),
    (
        "The original's format, for Return to the Roots (fish amounts are lost)",
        "Das Format des Originals, für Return to the Roots (Fischmengen gehen verloren)",
    ),
    ("Back to the editor", "Zurück zum Editor"),
    ("Author", "Autor"),
    ("Sheep", "Schaf"),
    ("Duck", "Ente"),
    ("Polar bear", "Eisbär"),
    ("Deer", "Reh"),
    ("Stag", "Hirsch"),
    ("Fox", "Fuchs"),
    ("Rabbit", "Hase"),
    ("density", "Dichte"),
    ("pile size", "Haufengröße"),
    ("mixed", "gemischt"),
    ("Smooth", "Glätten"),
    ("Alpine pasture", "Almwiese"),
    ("Dark steppe", "Dunkle Steppe"),
    ("Desert (2)", "Wüste (2)"),
    ("Flat mountain", "Flaches Gebirge"),
    ("Flower pasture", "Blumenweide"),
    ("Flower tundra", "Blumentundra"),
    ("Ice", "Eis"),
    ("Ice (2)", "Eis (2)"),
    ("Ice floe", "Eisscholle"),
    ("Ice floes", "Eisschollen"),
    ("Lava", "Lava"),
    ("Lava (2)", "Lava (2)"),
    ("Lava (3)", "Lava (3)"),
    ("Lava (4)", "Lava (4)"),
    ("Lava (5)", "Lava (5)"),
    ("Lava with few stones", "Lava mit wenig Steinen"),
    ("Lava with many stones", "Lava mit vielen Steinen"),
    ("Light steppe", "Helle Steppe"),
    ("Moor", "Moor"),
    ("Pasture", "Weide"),
    ("Pasture (2)", "Weide (2)"),
    ("Pasture (3)", "Weide (3)"),
    ("Reef moor", "Riffmoor"),
    ("Reef water", "Riffwasser"),
    ("Shallow moor", "Flaches Moor"),
    ("Shallow water", "Flaches Wasser"),
    ("Taiga", "Taiga"),
    ("Tundra", "Tundra"),
    ("Tundra (2)", "Tundra (2)"),
    ("Tundra (3)", "Tundra (3)"),
    ("Tundra (4)", "Tundra (4)"),
    ("Wasteland (2)", "Ödland (2)"),
    ("Show resources", "Bodenschätze zeigen"),
    ("Where buildings fit (Space)", "Wo Gebäude passen (Leertaste)"),
    ("height", "Höhe"),
    (
        "A map with this name exists. Save again to replace it.",
        "Eine Karte mit diesem Namen gibt es schon. Nochmals speichern ersetzt sie.",
    ),
    ("⟳ Redo", "⟳ Wiederholen"),
    ("The changes since the last save will be lost.", "Die Änderungen seit dem letzten Speichern gehen verloren."),
    ("Discard them", "Verwerfen"),
    ("A harbor spot goes on the coast.", "Ein Hafenplatz gehört an die Küste."),
    ("Harbor spots not on the coast:", "Hafenplätze nicht an der Küste:"),
    ("Trees or stones in water:", "Bäume oder Steine im Wasser:"),
    ("The map has no headquarters.", "Die Karte hat keine Burg."),
    ("This file is no map that can be read.", "Diese Datei ist keine lesbare Karte."),
    (
        "Without a mouse: Enter shows a cursor on the map. Arrows move it, Home / PageUp / End / PageDown go diagonally. Enter clicks there. In the action window 1-9 build, F places a flag, T shows the next tab. Backspace takes back a road step. Ctrl+M: mute.",
        "Ohne Maus: Enter zeigt einen Cursor auf der Karte. Die Pfeile bewegen ihn, Pos1 / Bild auf / Ende / Bild ab gehen schräg. Enter klickt dort. Im Aktionsfenster bauen 1-9, F setzt eine Fahne, T zeigt den nächsten Reiter. Rücktaste nimmt einen Straßenschritt zurück. Strg+M: stumm.",
    ),
    ("Ambience", "Umgebungsgeräusche"),
    ("Mute (Ctrl+M)", "Stumm (Strg+M)"),
    ("Silent while the window is in the background", "Stumm, solange das Fenster im Hintergrund ist"),
    ("Toggle fullscreen (F11)", "Vollbild an/aus (F11)"),
    ("No audio device.", "Kein Audiogerät."),
    ("Building a road: click nodes, a flag to finish", "Straßenbau: Punkte anklicken, Fahne zum Beenden"),
    ("Flag here", "Fahne hier"),
    ("Cancel", "Abbrechen"),
    // action window
    ("Action", "Aktion"),
    ("Flag", "Fahne"),
    ("➡ Build road", "➡ Straße bauen"),
    ("✖ Remove flag", "✖ Fahne abreißen"),
    ("✖ Remove road", "✖ Straße abreißen"),
    ("⛏ Geologist", "⛏ Geologe"),
    ("👁 Scout", "👁 Späher"),
    ("🚩 Place flag", "🚩 Fahne setzen"),
    ("Small", "Klein"),
    ("Medium", "Mittel"),
    ("Large", "Groß"),
    ("Mines", "Bergwerke"),
    ("You can only build on your own land.", "Du kannst nur auf deinem Land bauen."),
    ("Nothing can be built here.", "Hier kann nichts gebaut werden."),
    ("An enemy flag.", "Eine feindliche Fahne."),
    ("wares waiting", "Waren warten"),
    ("⚔ Attack!", "⚔ Angriff!"),
    ("🔥 Demolish", "🔥 Abreißen"),
    ("▶ Start production", "▶ Produktion starten"),
    ("⏸ Stop production", "⏸ Produktion anhalten"),
    ("Waiting for a worker.", "Wartet auf einen Arbeiter."),
    ("Waiting for a builder.", "Wartet auf einen Bauarbeiter."),
    ("Waiting for the planer to level the ground.", "Wartet auf den Planierer."),
    ("No more resources here.", "Hier gibt es nichts mehr."),
    ("No trees in reach: build a forester nearby.", "Keine Bäume in Reichweite: einen Förster in der Nähe bauen."),
    (
        "No stone in reach: tear it down and build a quarry by other rocks, or a granite mine in the mountains (send a geologist first; miners need food).",
        "Keine Steine in Reichweite: abreißen und einen Steinbruch an anderen Felsen bauen, oder ein Granitbergwerk in den Bergen (zuerst einen Geologen schicken; Bergleute brauchen Nahrung).",
    ),
    ("No fish in reach.", "Keine Fische in Reichweite."),
    ("No game in reach.", "Kein Wild in Reichweite."),
    ("No room for fields in reach.", "Kein Platz für Felder in Reichweite."),
    ("No room to plant trees in reach.", "Kein Platz zum Pflanzen in Reichweite."),
    ("Nothing to do in reach.", "Nichts zu tun in Reichweite."),
    ("Waiting for wares.", "Wartet auf Waren."),
    ("How to start", "So fängst du an"),
    ("Small building", "Kleines Gebäude"),
    ("Medium building", "Mittleres Gebäude"),
    ("Large building", "Großes Gebäude"),
    ("Mine", "Bergwerk"),
    ("host address", "Adresse des Gastgebers"),
    ("Others join at", "Mitspieler verbinden sich mit"),
    ("Players:", "Spieler:"),
    ("Held up", "Warenstau"),
    ("Black: land nobody of yours has seen yet.", "Schwarz: Land, das noch keiner deiner Leute gesehen hat."),
    (
        "The truce ends in 5 minutes. Your border buildings already take all the soldiers they can: watchtowers and fortresses hold more, and beer, swords and shields make new soldiers.",
        "Die Waffenruhe endet in 5 Minuten. Deine Grenzgebäude nehmen schon so viele Soldaten auf, wie sie können: Wachtürme und Festungen fassen mehr, und Bier, Schwerter und Schilde machen neue Soldaten.",
    ),
    (
        "Tip: next, food: mines need it for coal, iron and gold. Build a fishery by the water or a hunter, later farms with a mill and a bakery.",
        "Tipp: Als Nächstes Nahrung. Bergwerke brauchen sie für Kohle, Eisen und Gold. Baue einen Fischer am Wasser oder einen Jäger, später Bauernhöfe mit Mühle und Bäckerei.",
    ),
    // (the old "behind" message, in older saves: "{1} {2}{3} {4}" with these two)
    ("Your rivals hold", "Deine Gegner besitzen"),
    ("× your land.", "× dein Land."),
    (
        "Make soldiers before you grow: beer, a sword and a shield make one in the castle. New military buildings now would stand empty.",
        "Mache erst Soldaten, dann wachse: Bier, ein Schwert und ein Schild ergeben einen in der Burg. Neue Militärgebäude stünden jetzt leer.",
    ),
    (
        "Hold what you have: a watchtower or fortress facing your attackers, and no new guardhouses until soldiers wait in store (beer, a sword and a shield make one).",
        "Halte, was du hast: einen Wachturm oder eine Festung gegenüber den Angreifern, und keine neuen Wachstuben, bis Soldaten im Lager warten (Bier, ein Schwert und ein Schild machen einen).",
    ),
    (
        "Grow: put military buildings at the edge of your land (Space shows where), keep wood and stone coming, and feed miners for iron and gold.",
        "Wachse: Stelle Militärgebäude an den Rand deines Landes (Leertaste zeigt wo), halte Holz und Steine im Fluss und versorge Bergleute mit Essen für Eisen und Gold.",
    ),
    (
        "Tip: stones are running low. Build a quarry next to rocks, or a granite mine in the mountains (send a geologist first).",
        "Tipp: Die Steine gehen zur Neige. Baue einen Steinbruch an Felsen oder ein Granitbergwerk in den Bergen (schicke zuerst einen Geologen).",
    ),
    (
        "Too close to the castle or another military building: military buildings keep 4 nodes apart.",
        "Zu nah an der Burg oder einem anderen Militärgebäude: Militärgebäude halten 4 Punkte Abstand.",
    ),
    (
        "The truce is over: your rivals may attack now.",
        "Die Waffenruhe ist vorbei: Deine Gegner dürfen jetzt angreifen.",
    ),
    (
        "The truce ends in 5 minutes. Man your border buildings: Main selection, Military, troops at the border.",
        "Die Waffenruhe endet in 5 Minuten. Besetze deine Grenzgebäude: Hauptauswahl, Militär, Truppen an der Grenze.",
    ),
    ("Truce", "Waffenruhe"),
    ("Until then your rivals leave you alone", "Bis dahin lassen dich deine Gegner in Ruhe"),
    (
        "Your rivals leave you alone for the first {1} minutes (see the clock at the bottom right). Use them to build up and to man your borders.",
        "Deine Gegner lassen dich die ersten {1} Minuten in Ruhe (siehe die Uhr unten rechts). Nutze sie zum Aufbau und um deine Grenzen zu besetzen.",
    ),
    ("The military settings allow", "Die Militäreinstellungen erlauben"),
    ("buildings", "Gebäude"),
    ("inland", "im Landesinneren"),
    ("in the middle", "im Mittelfeld"),
    ("near a harbor", "nahe einem Hafen"),
    ("at the border", "an der Grenze"),
    ("Change it in Main selection, Military.", "Ändern unter Hauptauswahl, Militär."),
    (
        "No room is left for a military building, and none of your soldiers can attack. From here the land can't be won back: try the mission again (Main selection, Give up).",
        "Für ein Militärgebäude ist kein Platz mehr, und keiner deiner Soldaten kann angreifen. Von hier aus lässt sich das Land nicht zurückgewinnen: Versuche die Mission erneut (Hauptauswahl, Aufgeben).",
    ),
    ("An ally.", "Ein Verbündeter."),
    (
        "Tip: your shipyard builds boats for river roads. Switch it to Ships in its window to sail to other shores.",
        "Tipp: Deine Werft baut Boote für Wasserwege. Stelle sie in ihrem Fenster auf Schiffe um, um zu anderen Küsten zu segeln.",
    ),
    ("Reserve soldiers", "Soldaten in Reserve"),
    ("A flag can't stand right next to another flag.", "Eine Fahne kann nicht direkt neben einer anderen stehen."),
    ("A catapult is firing upon us!", "Ein Katapult beschießt uns!"),
    ("Boats", "Boote"),
    ("Ships", "Schiffe"),
    (
        "Not attackable: nobody has moved in yet, or you can't see it.",
        "Nicht angreifbar: noch niemand eingezogen, oder du siehst es nicht.",
    ),
    ("No road to this building site.", "Keine Straße zu dieser Baustelle."),
    ("Go there (G)", "Hingehen (G)"),
    ("Next mission:", "Nächste Mission:"),
    ("Show statistics", "Statistik zeigen"),
    ("Allied against you:", "Gegen dich verbündet:"),
    ("Each on their own, at war with all:", "Jeder für sich, im Krieg mit allen:"),
    ("At peace with you:", "In Frieden mit dir:"),
    ("Goal: defeat all rivals ({1} left)", "Ziel: alle Gegner besiegen (noch {1})"),
    ("Goal: take the castle of {1}", "Ziel: die Burg von {1} einnehmen"),
    ("Goal: hold {1} % of the land ({2} % now)", "Ziel: {1} % des Landes (jetzt {2} %)"),
    ("Goal: hold out {1} minutes ({2} left)", "Ziel: {1} Minuten durchhalten (noch {2})"),
    ("Goal: make {1} {2} ({3} so far)", "Ziel: {1} {2} herstellen (bisher {3})"),
    ("Goal reached! The mission is won.", "Ziel erreicht! Die Mission ist gewonnen."),
    ("Continue: conquer all", "Weiterspielen: alles erobern"),
    ("Goal: take the castle of Bjorn of Germania.", "Ziel: Nimm die Burg von Björn von Germanien ein."),
    (
        "Goal: mint 60 gold coins: rule the sea by trade.",
        "Ziel: Präge 60 Goldmünzen und beherrsche so das Meer durch Handel.",
    ),
    ("Goal: hold 30 % of the continent.", "Ziel: Halte 30 % des Kontinents."),
    (
        "Goal: take the castle of Lianhua of Jiangnan, in the south.",
        "Ziel: Nimm im Süden die Burg von Lianhua von Jiangnan ein.",
    ),
    ("Goal: hold out 90 minutes in the snow.", "Ziel: Halte 90 Minuten im Schnee durch."),
    ("Paused", "Angehalten"),
    ("{1} continues", "{1} spielt weiter"),
    ("Leave the game", "Spiel verlassen"),
    (
        "Leave the network game? You are the host: the game ends for everyone, and nobody can come back.",
        "Das Netzwerkspiel verlassen? Du bist der Gastgeber: Das Spiel endet für alle, und niemand kann zurück.",
    ),
    (
        "Leave the network game? The others play on without you, and you can't come back.",
        "Das Netzwerkspiel verlassen? Die anderen spielen ohne dich weiter, und du kannst nicht zurück.",
    ),
    (
        "A network game can't be paused: the others play on.",
        "Ein Netzwerkspiel lässt sich nicht anhalten: Die anderen spielen weiter.",
    ),
    (
        "Find your rivals: grow toward them, or reach them by sea (a harbor, a shipyard, an expedition)",
        "Finde deine Gegner: Wachse auf sie zu, oder erreiche sie über das Meer (Hafen, Werft, Expedition)",
    ),
    (
        "Your rivals live far away, across the sea or beyond the desert: the war comes late. Grow toward them, or build a harbor, a shipyard and an expedition to reach them sooner.",
        "Deine Gegner leben weit weg, über dem Meer oder hinter der Wüste: Der Krieg kommt spät. Wachse auf sie zu, oder baue Hafen, Werft und Expedition, um sie früher zu erreichen.",
    ),
    (
        "The campaign's first war: rivals by land on every side.",
        "Der erste Krieg des Feldzugs: Gegner zu Land auf allen Seiten.",
    ),
    (
        "Rivals far across the sea and the desert: harbors and expeditions; the war comes late.",
        "Gegner weit über dem Meer und der Wüste: Häfen und Expeditionen; der Krieg kommt spät.",
    ),
    (
        "A wide continent: rivals by land, little wood on the plains.",
        "Ein weiter Kontinent: Gegner zu Land, wenig Holz in den Ebenen.",
    ),
    (
        "Lianhua in the south is your goal; Yamato across the sea fills its coasts early.",
        "Lianhua im Süden ist dein Ziel; Yamato jenseits des Meeres besetzt seine Küsten früh.",
    ),
    (
        "Snow: wood everywhere, little grain; rivals close by land.",
        "Schnee: überall Holz, wenig Getreide; Gegner nah zu Land.",
    ),
    ("⏸ Paused: P continues", "⏸ Angehalten: P spielt weiter"),
    (
        "A farm needs open land around it for its fields; grain and water feed the mill, the brewery and the pigs.",
        "Ein Bauernhof braucht freies Land für seine Felder; Getreide und Wasser versorgen Mühle, Brauerei und Schweine.",
    ),
    (
        "Grain makes flour, flour and water make bread: bread feeds the miners.",
        "Aus Getreide wird Mehl, aus Mehl und Wasser Brot: Brot ernährt die Bergleute.",
    ),
    (
        "On mountains where a geologist found coal and iron; miners need food (fish, meat or bread).",
        "Auf Bergen, wo ein Geologe Kohle und Eisen fand; Bergleute brauchen Essen (Fisch, Fleisch oder Brot).",
    ),
    (
        "Grain and water make beer. Beer, a sword and a shield make a soldier.",
        "Aus Getreide und Wasser wird Bier. Bier, ein Schwert und ein Schild machen einen Soldaten.",
    ),
    (
        "Iron ore and coal make iron; iron and coal make swords and shields in the armory.",
        "Aus Eisenerz und Kohle wird Eisen; aus Eisen und Kohle macht die Waffenschmiede Schwerter und Schilde.",
    ),
    (
        "Once you have soldiers to spare: a rival post near your border, weaker than what you can send.",
        "Sobald Soldaten übrig sind: ein gegnerischer Posten nahe deiner Grenze, schwächer als das, was du schicken kannst.",
    ),
    (
        "💡 Two soldiers stay in this warehouse to defend it: rivals attack empty warehouses first. Change it in its window (Reserve).",
        "💡 Zwei Soldaten bleiben zur Verteidigung in diesem Lagerhaus: Gegner greifen leere Lagerhäuser zuerst an. Ändern im Fenster des Gebäudes (Reserve).",
    ),
    ("Load the quick save?", "Den Schnellspeicherstand laden?"),
    ("What happened since the last save is lost.", "Was seit dem letzten Speichern geschah, geht verloren."),
    ("Camera", "Kamera"),
    ("Scroll", "Verschieben"),
    ("Zoom", "Zoomen"),
    ("To the castle", "Zur Burg"),
    ("To the latest alert", "Zur letzten Warnung"),
    ("Windows", "Fenster"),
    ("Showing", "Anzeigen"),
    ("Building help", "Bauhilfe"),
    ("Building names", "Gebäudenamen"),
    ("Game", "Spiel"),
    ("Pause", "Pause"),
    ("Speed", "Tempo"),
    ("Quick save / load", "Schnellspeichern / Schnellladen"),
    ("Fullscreen", "Vollbild"),
    ("Mute", "Stumm"),
    ("Pause when attacked", "Bei Angriff anhalten"),
    ("The scout keeps watch: the tower sees far around.", "Der Späher hält Wache: Der Turm sieht weit umher."),
    (
        "Works little (under 25%): too few wares, or its work is far away.",
        "Arbeitet wenig (unter 25 %): zu wenige Waren, oder die Arbeit ist weit weg.",
    ),
    ("With names", "Mit Namen"),
    (
        "Tip: a forester plants trees on all free land around it. Keep foresters away from where you still want to build.",
        "Tipp: Ein Förster pflanzt auf allem freien Land um sich herum Bäume. Baue Förster nicht dort, wo du noch bauen willst.",
    ),
    ("/path/to/MAP.WLD", "/pfad/zu/KARTE.WLD"),
    (
        "You hold Italy. Five rulers share Europe with you.",
        "Du hältst Italien. Fünf Herrscher teilen sich Europa mit dir.",
    ),
    ("Go there", "Hingehen"),
    ("Show on the map", "Auf der Karte zeigen"),
    ("Islands", "Inseln"),
    (
        "⛵ By sea: soldiers from the posts by your harbor sail over and land nearby.",
        "⛵ Übers Meer: Soldaten aus den Posten bei deinem Hafen segeln hinüber und landen in der Nähe.",
    ),
    ("🔭 Explore the coasts", "🔭 Küsten erkunden"),
    (
        "A ship sails to coasts you haven't seen yet, looking far around, and comes back.",
        "Ein Schiff segelt zu Küsten, die du noch nicht gesehen hast, schaut weit umher und kehrt zurück.",
    ),
    ("⛵ Sail to the nearest free coast", "⛵ Zur nächsten freien Küste segeln"),
    (
        "Every realm on its own island, with islands of ore and forest between: harbors and expeditions reach the rest.",
        "Jedes Reich auf seiner eigenen Insel, dazwischen Inseln mit Erz und Wald: Häfen und Expeditionen erreichen den Rest.",
    ),
    (
        "A ship takes a builder, the harbor's materials and a starter kit to a free harbor spot on another coast: a new colony.",
        "Ein Schiff bringt einen Bauarbeiter, die Baustoffe des Hafens und eine Grundausstattung zu einem freien Hafenplatz an einer anderen Küste: eine neue Kolonie.",
    ),
    (
        "No free harbor spot known across this sea: explore its coasts.",
        "Kein freier Hafenplatz an diesem Meer bekannt: Erkunde seine Küsten.",
    ),
    ("Free harbor spots across the sea: {1}", "Freie Hafenplätze übers Meer: {1}"),
    ("★ Priority: on", "★ Vorrang: an"),
    ("☆ Priority: off", "☆ Vorrang: aus"),
    (
        "Built and supplied before your other buildings, and gets its people first.",
        "Wird vor deinen anderen Gebäuden gebaut und beliefert und bekommt seine Leute zuerst.",
    ),
    ("Stopped.", "Angehalten."),
    (
        "No worker yet: a settler and the right tool are needed.",
        "Noch kein Arbeiter: Ein Siedler und das passende Werkzeug fehlen.",
    ),
    (
        "Click the ground near your castle and choose a building; a road starts from its flag — click the castle to connect it.",
        "Klicke neben deine Burg und wähle ein Gebäude; von seiner Fahne beginnt eine Straße — klicke die Burg an, um sie anzuschließen.",
    ),
    (
        "First woodcutters, a forester at the edge of the woods (away from the castle: it plants trees all round it), a quarry and a sawmill; then guardhouses to grow your land.",
        "Zuerst Holzfäller, einen Förster am Waldrand (weg von der Burg: er pflanzt rundum Bäume), einen Steinbruch und ein Sägewerk; dann Wachstuben, um dein Land zu vergrößern.",
    ),
    (
        "Space shows where you can build, O how well buildings work. Right-click on a window or the map closes the window.",
        "Leertaste zeigt, wo du bauen kannst, O wie gut Gebäude arbeiten. Rechtsklick auf ein Fenster oder die Karte schließt das Fenster.",
    ),
    ("Settlers", "Siedler"),
    // menu
    ("⚔  The Europe mission", "⚔  Die Europa-Mission"),
    ("The Europe mission", "Die Europa-Mission"),
    ("Free game on a random map", "Freies Spiel auf einer Zufallskarte"),
    ("players", "Spieler"),
    ("map size", "Kartengröße"),
    ("Start free game", "Freies Spiel starten"),
    ("Load quick save", "Schnellspeicherstand laden"),
    ("Continue", "Weiterspielen"),
    ("Quit", "Beenden"),
    ("Opponents:", "Gegner:"),
    ("easy", "leicht"),
    ("normal", "normal"),
    ("hard", "schwer"),
    ("build · carry · conquer", "bauen · tragen · erobern"),
    ("Begin", "Los geht's"),
    ("Goal: defeat all five rivals.", "Ziel: Besiege alle fünf Gegner."),
    ("Back to the menu", "Zurück zum Menü"),
    ("Game over", "Spielende"),
    ("Victory! All enemies are defeated.", "Sieg! Alle Gegner sind besiegt."),
    ("You have been defeated.", "Du wurdest besiegt."),
    (
        "Your people have settled in the hills of Italy. Beyond the Alps, across the Pyrenees and far in the north, five rulers watch your rise: Livia of Iberia, Sigrid of Britain, Ragnar of Scandinavia, Bjorn of Germania and Hadrian of Sarmatia.\n\nBuild roads, gather wood and stone, and push your borders north. The Alps hold the richest gold. Feed your miners, forge swords and shields, brew beer — and train an army strong enough to take Europe.",
        "Dein Volk hat sich in den Hügeln Italiens niedergelassen. Jenseits der Alpen, hinter den Pyrenäen und weit im Norden beobachten fünf Herrscher deinen Aufstieg: Livia von Iberien, Sigrid von Britannien, Ragnar von Skandinavien, Björn von Germanien und Hadrian von Sarmatien.\n\nBaue Straßen, sammle Holz und Stein und schiebe deine Grenzen nach Norden. In den Alpen liegt das meiste Gold. Versorge deine Bergleute, schmiede Schwerter und Schilde, braue Bier — und bilde ein Heer aus, das stark genug ist, Europa zu erobern.",
    ),
    // messages
    ("finished", "fertig"),
    ("{1} finished", "{1} fertig"),
    ("{1} lost", "{1} verloren"),
    (
        "{1} lost to {2}: its wares and settlers are gone with it.",
        "{1} an {2} verloren: Alle Waren und Siedler darin sind fort.",
    ),
    ("{1} lost: its wares and settlers are gone with it.", "{1} verloren: Alle Waren und Siedler darin sind fort."),
    (
        "{1} lost to {2}: its wares and settlers are gone with it. You have no warehouse left: nothing new can be built.",
        "{1} an {2} verloren: Alle Waren und Siedler darin sind fort. Du hast kein Lagerhaus mehr: Nichts Neues kann gebaut werden.",
    ),
    (
        "{1} lost: its wares and settlers are gone with it. You have no warehouse left: nothing new can be built.",
        "{1} verloren: Alle Waren und Siedler darin sind fort. Du hast kein Lagerhaus mehr: Nichts Neues kann gebaut werden.",
    ),
    (
        "{1} boards and {2} stones lie in store and nothing is being built: enough for {3} guardhouses. Place more buildings, military ones at the border first.",
        "{1} Bretter und {2} Steine liegen im Lager, und nichts wird gebaut: genug für {3} Wachstuben. Stelle mehr Gebäude auf, zuerst Militärgebäude an der Grenze.",
    ),
    (
        "{1} boards and {2} stones lie in store and nothing is being built. No soldier waits to man a new military building: brew beer, make swords and shields, and build workshops meanwhile.",
        "{1} Bretter und {2} Steine liegen im Lager, und nichts wird gebaut. Kein Soldat wartet auf ein neues Militärgebäude: Braue Bier, schmiede Schwerter und Schilde und baue derweil Werkstätten.",
    ),
    ("Next steps", "Nächste Schritte"),
    ("A farm and a well", "Ein Bauernhof und ein Brunnen"),
    ("A mill and a bakery", "Eine Mühle und eine Bäckerei"),
    ("A coal mine and an iron mine", "Ein Kohlebergwerk und ein Eisenbergwerk"),
    ("An iron smelter and an armory", "Eine Eisenschmelze und eine Waffenschmiede"),
    ("A brewery", "Eine Brauerei"),
    (
        "Not sure what's inside? Place a flag on the mountain and send a geologist from it.",
        "Unklar, was im Berg steckt? Setze eine Fahne auf den Berg und schicke von dort einen Geologen.",
    ),
    ("Geologists found here:", "Geologen fanden hier:"),
    ("Near trees: Space shows where buildings fit.", "In der Nähe von Bäumen: Leertaste zeigt, wo Gebäude passen."),
    (
        "Next to the woodcutters, away from the land you still want to build on.",
        "Neben den Holzfällern, abseits vom Land, auf dem du noch bauen willst.",
    ),
    ("Next to grey rocks.", "Neben grauen Felsen."),
    (
        "A medium spot (the bigger house marker): it turns wood into boards.",
        "Ein mittlerer Bauplatz (das größere Haussymbol): Es macht aus Holz Bretter.",
    ),
    (
        "At the edge of your land: each one makes it bigger and guards it.",
        "Am Rand deines Landes: Jedes vergrößert und schützt es.",
    ),
    (
        "A fishery by the water, a hunter near forests with game.",
        "Ein Fischer am Wasser, ein Jäger bei Wäldern mit Wild.",
    ),
    (
        "On a mountain inside your land (the dark triangles); send a geologist first to find out what is inside.",
        "Auf einem Berg in deinem Land (die dunklen Dreiecke); schicke zuerst einen Geologen, um herauszufinden, was darin ist.",
    ),
    (
        "A flag can't go here, next to another flag: click that flag, or one step further on.",
        "Hier passt keine Fahne, direkt neben einer anderen: Klicke auf diese Fahne oder einen Schritt weiter.",
    ),
    (
        "The truce ends in 5 minutes. {1} of your military buildings hold fewer soldiers than they could: Main selection, Military, raise the troops in the middle and at the border.",
        "Die Waffenruhe endet in 5 Minuten. {1} deiner Militärgebäude haben weniger Soldaten, als sie fassen können: Hauptauswahl, Militär, die Truppen in der Mitte und an der Grenze erhöhen.",
    ),
    ("A player's game went apart from ours (desync):", "Das Spiel eines Mitspielers lief auseinander (Desync):"),
    (
        "A player's game went apart from the host's (desync):",
        "Das Spiel eines Mitspielers lief vom Spiel des Gastgebers auseinander (Desync):",
    ),
    ("report:", "Bericht:"),
    ("Continue against the computer", "Gegen den Computer weiterspielen"),
    ("Build id", "Version"),
    (
        "This replay played differently from the recorded game.",
        "Diese Wiederholung lief anders als das aufgezeichnete Spiel.",
    ),
    ("Replays can't be saved.", "Wiederholungen lassen sich nicht speichern."),
    (
        "Attacks start from your military buildings nearby (barracks, guardhouse, watchtower, fortress); the castle's soldiers only defend. Build one close to it.",
        "Angriffe gehen von deinen Militärgebäuden in der Nähe aus (Baracke, Wachstube, Wachturm, Festung); die Soldaten in der Burg verteidigen nur. Baue eines in der Nähe.",
    ),
    (
        "Your military buildings near it keep their last soldier at home. Send them more soldiers, or raise the attack setting (Military).",
        "Deine Militärgebäude in der Nähe behalten ihren letzten Soldaten. Schicke ihnen mehr Soldaten oder erhöhe die Angriffseinstellung (Militär).",
    ),
    (
        "Only a flag fits here: a building needs more room (too close to another building, a tree, rocks or a slope).",
        "Hier passt nur eine Fahne: Ein Gebäude braucht mehr Platz (zu nah an einem Gebäude, Baum, Felsen oder Hang).",
    ),
    (
        "No saved game there yet. Press F5 during a game to make a quick save.",
        "Dort ist noch kein Spielstand. Drücke im Spiel F5 für einen Schnellspeicherstand.",
    ),
    (
        "No quick save yet. Press F5 during a game to make one.",
        "Noch kein Schnellspeicherstand. Drücke im Spiel F5, um einen anzulegen.",
    ),
    (
        "You have {1} military buildings, your rivals about {2} each.",
        "Du hast {1} Militärgebäude, deine Gegner je etwa {2}.",
    ),
    ("First steps", "Erste Schritte"),
    ("Two woodcutters", "Zwei Holzfäller"),
    ("A forester", "Ein Förster"),
    ("A quarry", "Ein Steinbruch"),
    ("A sawmill", "Ein Sägewerk"),
    ("Three military buildings", "Drei Militärgebäude"),
    ("A fishery or a hunter", "Ein Fischer oder ein Jäger"),
    ("A mine", "Ein Bergwerk"),
    (
        "The rivals build slowly and attack late: for your first games.",
        "Die Gegner bauen langsam und greifen spät an: für die ersten Spiele.",
    ),
    (
        "The rivals expand fast and attack once the truce is over.",
        "Die Gegner breiten sich schnell aus und greifen nach der Waffenruhe an.",
    ),
    ("The rivals build and attack at full speed.", "Die Gegner bauen und greifen mit voller Kraft an."),
    ("{1} attacks our {2}", "{1} greift an: {2}"),
    ("{1} sends {3} soldiers against our {2} ({4} inside)", "{1} schickt {3} Soldaten zum Angriff auf {2} ({4} drin)"),
    ("{1} took our {2}", "{1} hat erobert: {2}"),
    (
        "💡 Your castle holds {1} soldiers, but they only defend the castle. A guardhouse holds 3, a watchtower 6, a fortress 9: put strong buildings where your rivals' land touches yours.",
        "💡 Deine Burg hat {1} Soldaten, aber die verteidigen nur die Burg. Eine Wachstube fasst 3, ein Wachturm 6, eine Festung 9: Stelle starke Gebäude dorthin, wo das Land der Gegner an deins grenzt.",
    ),
    ("We took {1}'s {2}", "Erobert: {2} von {1}"),
    ("Geologist found {1}", "Der Geologe fand {1}"),
    ("Geologist found {1} ({2})", "Der Geologe fand {1} ({2})"),
    ("{1} has been defeated", "{1} wurde besiegt"),
    ("Not possible: {1}", "Nicht möglich: {1}"),
    ("{1}: {2}", "{1}: {2}"),
    (
        "Your rivals leave you alone for the first {1} minutes.",
        "Deine Gegner lassen dich die ersten {1} Minuten in Ruhe.",
    ),
    ("{1} {2}{3} {4}", "{1} {2}{3} {4}"),
    // whole sentences, so that German can order its own words and count (German round 1, #3/#4)
    ("{1} min ago", "vor {1} Min."),
    ("{1} h ago", "vor {1} Std."),
    ("1 day ago", "vor 1 Tag"),
    ("{1} days ago", "vor {1} Tagen"),
    ("{1} min", "{1} Min."),
    ("Your rivals hold {1}× your land. {2}", "Deine Gegner haben im Schnitt {1}-mal so viel Land wie du. {2}"),
    ("Your rival holds {1}× your land. {2}", "Dein Gegner hat {1}-mal so viel Land wie du. {2}"),
    ("You have {1} military buildings, your rival {2}.", "Du hast {1} Militärgebäude, dein Gegner {2}."),
    (
        "The military settings send {1} soldiers here ({2}). Change it in Main selection, Military.",
        "Laut Militäreinstellungen kommen {1} Soldaten hierher ({2}). Ändern unter Hauptauswahl, Militär.",
    ),
    (
        "The military settings send 1 soldier here ({1}). Change it in Main selection, Military.",
        "Laut Militäreinstellungen kommt 1 Soldat hierher ({1}). Ändern unter Hauptauswahl, Militär.",
    ),
    ("Belongs to {1}", "Besitzer: {1}"),
    (
        "The truce ends in 5 minutes. 1 of your military buildings holds fewer soldiers than it could: Main selection, Military, raise the troops in the middle and at the border.",
        "Die Waffenruhe endet in 5 Minuten. Eines deiner Militärgebäude hat weniger Soldaten, als es fassen kann: Hauptauswahl, Militär, die Truppen in der Mitte und an der Grenze erhöhen.",
    ),
    (
        "Player {1}: only 1 tree near the castle: little wood to cut.",
        "Spieler {1}: nur 1 Baum bei der Burg, also wenig Holz zum Fällen.",
    ),
    ("Burned with the lost land: 1 building", "Mit dem verlorenen Land abgebrannt: 1 Gebäude"),
    ("{1} sends one soldier against our {2} ({4} inside)", "{1} schickt einen Soldaten zum Angriff auf {2} ({4} drin)"),
    ("Goal: mint {1} gold coins ({2} so far)", "Ziel: {1} Goldmünzen prägen (bisher {2})"),
    // the help's key column, and key names (German round 1, #6)
    ("Right drag, arrows, WASD", "Rechts ziehen, Pfeile, WASD"),
    ("Wheel, Z / shift-Z", "Mausrad, Z / Umschalt+Z"),
    ("Wheel, {1} / shift-{1}", "Mausrad, {1} / Umschalt+{1}"),
    ("Ctrl+M", "Strg+M"),
    ("Ctrl+{1}", "Strg+{1}"),
    ("Space", "Leertaste"),
    ("Esc", "Esc"),
    ("Escape", "Esc"),
    ("Backspace", "Rücktaste"),
    ("Home", "Pos1"),
    ("End", "Ende"),
    ("PageUp", "Bild auf"),
    ("PageDown", "Bild ab"),
    ("Insert", "Einfg"),
    ("Delete", "Entf"),
    ("Plus", "Plus"),
    ("Minus", "Minus"),
    ("My map", "Meine Karte"),
    ("💡 {1}", "💡 {1}"),
    ("lost", "verloren"),
    ("We are under attack!", "Wir werden angegriffen!"),
    ("Geologist found", "Der Geologe fand"),
    ("We captured a building!", "Wir haben ein Gebäude erobert!"),
    ("A building was captured by the enemy", "Der Feind hat ein Gebäude erobert"),
    ("A mine is exhausted", "Ein Bergwerk ist erschöpft"),
    ("A mine is running low", "Ein Bergwerk geht zur Neige"),
    (
        "Its ore is running low: it finds something ever less often. Build a new mine where a geologist finds more.",
        "Sein Erz geht zur Neige: Es fördert immer seltener etwas. Baue ein neues Bergwerk, wo ein Geologe mehr findet.",
    ),
    ("has been defeated", "wurde besiegt"),
    ("Not possible:", "Nicht möglich:"),
    ("Game saved:", "Gespeichert:"),
    ("Promotes soldiers with:", "Befördert Soldaten mit:"),
    ("Game loaded", "Spiel geladen"),
    ("coal", "Kohle"),
    ("iron ore", "Eisenerz"),
    ("gold", "Gold"),
    ("granite", "Granit"),
    // military sliders, statistics
    ("Recruits (fewer … more)", "Rekruten (weniger … mehr)"),
    ("Defense (weak … strong soldiers first)", "Verteidigung (schwache … starke zuerst)"),
    ("Defenders sent to help", "Verteidiger zur Hilfe"),
    ("Attackers per building", "Angreifer je Gebäude"),
    ("Troops in the interior", "Besatzung im Landesinneren"),
    ("Troops in the middle", "Besatzung in der Mitte"),
    ("Troops near harbors", "Besatzung an Häfen"),
    ("Troops at the border", "Besatzung an der Grenze"),
    ("Land", "Land"),
    ("Buildings", "Gebäude"),
    ("Soldiers", "Soldaten"),
    ("Produced", "Produziert"),
    ("Your buildings", "Deine Gebäude"),
    ("Player", "Spieler"),
    ("Carriers take the higher wares first.", "Träger nehmen die oberen Waren zuerst."),
    ("Land owned over time", "Land im Verlauf"),
    ("Productivity", "Produktivität"),
    (
        "Left click: select a spot, flag or building. Pick an action in its window.",
        "Linksklick: Platz, Fahne oder Gebäude wählen. Im Fenster die Aktion auswählen.",
    ),
    (
        "Road: choose 'Build road' at a flag, then click nodes. Click a flag to finish, or 'Flag here'.",
        "Straße: an einer Fahne „Straße bauen“ wählen, dann Punkte anklicken. Mit einer Fahne beenden oder „Fahne hier“.",
    ),
    (
        "Right drag / arrows / WASD: scroll. Wheel or Z / shift-Z: zoom. Space: building help. O: productivity. H: to the HQ. C: building names. I: inventory. L: map. M: main selection. N: messages. P: pause. +/-: speed. Esc: cancel. F5/F9: quick save/load. F11: fullscreen.",
        "Rechts ziehen / Pfeile / WASD: scrollen. Rad oder Z / Umschalt-Z: zoomen. Leertaste: Bauhilfe. O: Produktivität. H: zur Burg. C: Gebäudenamen. I: Inventur. L: Karte. M: Hauptauswahl. N: Nachrichten. P: Pause. +/-: Tempo. Esc: abbrechen. F5/F9: Schnellspeichern/-laden. F11: Vollbild.",
    ),
    (
        "Goal: defeat all other players by taking their military buildings and castles.",
        "Ziel: Besiege alle anderen Spieler, indem du ihre Militärgebäude und Burgen einnimmst.",
    ),
    ("Time played:", "Spielzeit:"),
    ("boards", "Bretter"),
    ("stones", "Steine"),
    ("soldiers, available:", "Soldaten, verfügbar:"),
    ("Under construction:", "Im Bau:"),
    ("Settlers:", "Siedler:"),
    ("Soldiers:", "Soldaten:"),
    ("wanted", "gewünscht"),
    ("Coins:", "Münzen:"),
    ("Productivity:", "Produktivität:"),
    ("defeated", "besiegt"),
    ("Worker:", "Arbeiter:"),
    ("Needs:", "Braucht:"),
    ("Makes:", "Erzeugt:"),
    ("Gold: on", "Gold: an"),
    ("UI scale", "Skalierung"),
    ("Weapons", "Waffen"),
    ("Top", "Ganz nach oben"),
    ("Up", "Nach oben"),
    ("Down", "Nach unten"),
    ("Bottom", "Ganz nach unten"),
    ("Default", "Standard"),
    ("Inhabitants", "Einwohner"),
    ("Merchandise", "Waren"),
    ("Vanquished", "Besiegte Gegner"),
    ("Main selection", "Hauptauswahl"),
    ("Faster", "Schneller"),
    ("Slower", "Langsamer"),
    ("💾 Save", "💾 Speichern"),
    ("📂 Load", "📂 Laden"),
    ("🚪 Menu", "🚪 Menü"),
    ("Overview map", "Übersichtskarte"),
    ("Post office", "Postamt"),
    ("Overview map (L)", "Übersichtskarte (L)"),
    ("Main selection (M)", "Hauptauswahl (M)"),
    ("Messages (N)", "Nachrichten (N)"),
    ("Search the nearby mountains for resources", "Sucht in den Bergen in der Nähe nach Bodenschätzen"),
    ("Starting goods:", "Startwaren:"),
    ("low", "wenig"),
    ("medium", "mittel"),
    ("high", "viel"),
    ("Your wares", "Deine Waren"),
    ("Made so far:", "Bisher hergestellt:"),
    ("per 30 s", "pro 30 s"),
    ("one bar:", "ein Balken:"),
    ("⬆ Send the strongest home", "⬆ Den Stärksten heimschicken"),
    (
        "Your strongest soldier here goes to the castle and from there to where soldiers are wanted",
        "Dein stärkster Soldat hier geht in die Burg und von dort dorthin, wo Soldaten gebraucht werden",
    ),
    ("rival league", "Bündnis"),
    ("Take a rival post: click it, then ⚔ Attack", "Erobere einen gegnerischen Posten: anklicken, dann ⚔ Angriff"),
    (
        "These rivals send every soldier they can spare. After the truce they attack a post every few minutes: keep your border posts full, and have beer, swords and shields ready.",
        "Diese Gegner schicken jeden Soldaten, den sie entbehren können. Nach der Waffenruhe greifen sie alle paar Minuten einen Posten an: Halte deine Grenzposten voll besetzt und habe Bier, Schwerter und Schilde bereit.",
    ),
    ("None made yet", "Noch nichts hergestellt"),
    ("Burned with the lost land: {1} buildings", "Mit dem verlorenen Land abgebrannt: {1} Gebäude"),
    ("Rival league together", "Bündnis gesamt"),
    ("Soldiers counted by rank: a general counts 5.", "Soldaten nach Rang gezählt: Ein General zählt 5."),
    ("Gold coins minted so far.", "Bisher geprägte Goldmünzen."),
    (
        "Your soldiers: {1} in warehouses (they defend only those), {2} in military buildings.",
        "Deine Soldaten: {1} in Lagerhäusern (sie verteidigen nur diese), {2} in Militärgebäuden.",
    ),
    (
        "By rank: {1}. Together {2} soldiers with those on the way, {3} points in the chart.",
        "Nach Rang: {1}. Zusammen {2} Soldaten mit denen unterwegs, {3} Punkte in der Kurve.",
    ),
    ("Territory", "Gebiet"),
    ("Buildings and roads", "Gebäude und Straßen"),
    ("Next track", "Nächstes Stück"),
    ("Shuffle", "Zufällige Reihenfolge"),
    ("The computer players are allies against you", "Die Computerspieler verbünden sich gegen dich"),
    ("Win:", "Sieg:"),
    ("when all rivals are defeated", "wenn alle Gegner besiegt sind"),
    ("with {1} % of the land", "mit {1} % des Landes"),
    ("Victory! You hold {1} % of the land.", "Sieg! Du hast {1} % des Landes."),
    ("Peace time", "Friedenszeit"),
    ("⚔ Attack! (ends the truce)", "⚔ Angriff! (beendet die Waffenruhe)"),
    ("Your rivals keep the truce as long as you do", "Deine Gegner halten die Waffenruhe, solange du sie hältst"),
    ("End the truce and attack", "Waffenruhe beenden und angreifen"),
    ("Keep the truce", "Waffenruhe halten"),
    ("At war with you:", "Im Krieg mit dir:"),
    ("At peace:", "In Frieden:"),
    ("as above", "wie oben"),
    ("Leave the network game first.", "Verlasse zuerst das Netzwerkspiel."),
    ("Not saved, the game kept the last good save:", "Nicht gespeichert (der letzte gute Spielstand bleibt erhalten):"),
    (
        "You are under attack (\"Pause when attacked\" in the Options).",
        "Du wirst angegriffen („Bei Angriff anhalten“ in den Optionen).",
    ),
    ("Following: {1} (Esc stops)", "Ansicht folgt: {1} (Esc beendet)"),
    (
        "The view goes with them; moving the map stops it too",
        "Die Ansicht bewegt sich mit; auch Verschieben der Karte beendet das",
    ),
    (
        "The lookout tower isn't one: it holds no soldiers and claims no land, it only watches far.",
        "Der Spähturm ist kein Militärgebäude: Er hat keine Soldaten und nimmt kein Land ein, er hält nur Ausschau.",
    ),
    (
        "The attack on {1}'s {2} failed: its defenders held.",
        "Der Angriff auf {2} von {1} ist gescheitert: Die Verteidiger haben standgehalten.",
    ),
    ("Follow a soldier", "Einem Soldaten folgen"),
    (
        "The view goes with the soldier; move the map to stop",
        "Die Ansicht folgt dem Soldaten; Verschieben der Karte beendet das",
    ),
    ("Keys", "Tasten"),
    ("Press a key for: {1} (Esc keeps the old one)", "Drücke eine Taste für: {1} (Esc behält die alte)"),
    ("{1} is the key of: {2}", "{1} ist die Taste für: {2}"),
    (
        "{1} can't be used: the game needs it (scrolling, cancelling, the map cursor, Tab, the action window)",
        "{1} ist nicht belegbar: Das Spiel braucht diese Taste (Scrollen, Abbrechen, Kartencursor, Tab, Aktionsfenster)",
    ),
    ("All keys as they were", "Alle Tasten zurücksetzen"),
    ("Quick save", "Schnellspeichern"),
    ("Quick load", "Schnellladen"),
    ("Coins minted", "Geprägte Münzen"),
    ("Map variant {1} (the seed below picks another)", "Kartenvariante {1} (der Startwert unten wählt eine andere)"),
    ("Show the game's scenery", "Die Kulisse des Spiels zeigen"),
    (
        "Bushes, stumps and boulders the game draws; they aren't in the map",
        "Büsche, Stümpfe und Felsbrocken, die das Spiel zeichnet; sie gehören nicht zur Karte",
    ),
    (
        "Player {1}: only {2} trees near the castle: little wood to cut.",
        "Spieler {1}: nur {2} Bäume bei der Burg, also wenig Holz zum Fällen.",
    ),
    (
        "Player {1}: no stones near the castle: no stone to build with.",
        "Spieler {1}: keine Steine bei der Burg, also kein Stein zum Bauen.",
    ),
    (
        "The harbor spot at {1},{2} has no other harbor spot on its water: no ship can sail anywhere from it.",
        "Der Hafenplatz bei {1},{2} hat keinen zweiten Hafenplatz am selben Gewässer: Von dort kann kein Schiff irgendwohin fahren.",
    ),
    ("only the text above", "nur der Text oben"),
    ("defeat all", "alle besiegen"),
    ("defeat the player in slot", "einen Spieler besiegen, Platz"),
    ("hold this % of the land", "Landanteil halten, in %"),
    ("survive this many minutes", "überstehen, in Minuten"),
    ("mint this many gold coins", "Goldmünzen prägen, Anzahl"),
    ("Goal the game checks:", "Ziel, das das Spiel prüft:"),
    ("The human plays slot", "Menschlicher Spieler auf Platz"),
    ("Players renumbered without gaps.", "Spieler ohne Lücken neu nummeriert."),
    ("Slot {1} is now slot {2}.", "Platz {1} ist jetzt Platz {2}."),
    (
        "The mission file can't be read: it is left as it is, and the mission fields stay empty. Fix the file, or remove it.",
        "Die Missionsdatei lässt sich nicht lesen: Die Datei bleibt, wie sie ist, und die Missionsfelder bleiben leer. Repariere die Datei oder entferne sie.",
    ),
    (
        "Fix the mission file, or remove it to play without it.",
        "Repariere die Missionsdatei oder entferne sie, um ohne sie zu spielen.",
    ),
    ("Peace for good: nobody attacks in this game.", "Frieden für immer: In diesem Spiel greift niemand an."),
    ("Peace for good", "Frieden für immer"),
    ("Peace time: no catapults for {1}.", "Friedenszeit: keine Katapulte für {1}."),
    ("Nobody attacks in this game", "In diesem Spiel greift niemand an"),
    ("for good", "für immer"),
    ("first to mint {1} gold coins", "wer zuerst {1} Goldmünzen prägt"),
    ("You minted {1} gold coins first: victory.", "Du hast zuerst {1} Goldmünzen geprägt: Sieg."),
    ("{1} minted {2} gold coins first.", "{1} hat zuerst {2} Goldmünzen geprägt."),
    ("Goal: mint {1} gold coins first ({2} so far)", "Ziel: zuerst {1} Goldmünzen prägen (bisher {2})"),
    ("Follow the worker", "Dem Arbeiter folgen"),
    (
        "The view goes with the worker; move the map to stop",
        "Die Ansicht folgt dem Arbeiter; verschiebe die Karte, um aufzuhören",
    ),
    ("You send:", "Du schickst:"),
    ("They hold:", "Verteidiger:"),
    ("nobody", "niemand"),
    ("Rough odds: in your favour.", "Grob geschätzt: Vorteil für dich."),
    ("Rough odds: even.", "Grob geschätzt: ausgeglichen."),
    ("Rough odds: against you.", "Grob geschätzt: Nachteil für dich."),
    (
        "This rival keeps the peace as long as you do. Attacking ends it, with its allies too.",
        "Dieser Gegner hält Frieden, solange du es tust. Ein Angriff beendet ihn, auch mit seinen Verbündeten.",
    ),
    (
        "A quiet valley, and one neighbour: Aldric of the Marches. Here you learn how a realm grows: woodcutters and a quarry, a sawmill, roads that carry everything, guardhouses that push the border out, and at last soldiers who take a castle.\n\nAldric keeps the peace as long as you do: nothing here is a race. Follow the First steps at the top right, and attack when your army is ready.",
        "Ein stilles Tal und ein Nachbar: Aldric aus der Mark. Hier lernst du, wie ein Reich wächst: Holzfäller und ein Steinbruch, ein Sägewerk, Straßen, die alles tragen, Wachstuben, die die Grenze hinausschieben, und zuletzt Soldaten, die eine Burg einnehmen.\n\nAldric hält Frieden, solange du es tust: Hier drängt nichts. Folge den Ersten Schritten oben rechts und greife an, wenn dein Heer bereit ist.",
    ),
    ("Goal: take the castle of Aldric of the Marches.", "Ziel: Nimm die Burg von Aldric aus der Mark ein."),
    (
        "Learn to play: one neighbour, a small valley, a castle to take.",
        "Spielen lernen: ein Nachbar, ein kleines Tal, eine Burg zum Einnehmen.",
    ),
    ("You (the Valley)", "Du (das Tal)"),
    ("The first valley", "Das erste Tal"),
    ("Aldric of the Marches", "Aldric aus der Mark"),
    ("Productivity and crowded flags", "Produktivität und Warenstau"),
    ("Not connected: no road leads from its flag.", "Nicht verbunden: Von seiner Fahne führt keine Straße weg."),
    (
        "Its flag is full: the carriers don't take the wares away fast enough. Another road or a donkey helps.",
        "An seiner Fahne stauen sich die Waren: Die Träger bringen sie nicht schnell genug weg. Eine zweite Straße oder ein Esel hilft.",
    ),
    ("Waiting for: {1}.", "Wartet auf: {1}."),
    (
        "Your best on this mission: {1} minutes, score {2}.",
        "Deine Bestzeit in dieser Mission: {1} Minuten, Punktzahl {2}.",
    ),
    ("Your best at this level: {1} minutes, score {2}", "Deine Bestzeit auf dieser Stufe: {1} Minuten, Punktzahl {2}"),
    ("silver", "Silber"),
    ("bronze", "Bronze"),
    (
        "Rings mark the posts and warehouses of your last rivals ({1}).",
        "Ringe zeigen Posten und Lagerhäuser deiner letzten Gegner ({1}).",
    ),
    (
        "The truce holds both ways: if you attack a rival, it and its allies fight back at once.",
        "Die Waffenruhe gilt für beide Seiten: Greifst du einen Gegner an, schlagen er und seine Verbündeten sofort zurück.",
    ),
    (
        "The truce runs {1} more. Attacking ends it, with this rival and its allies.",
        "Die Waffenruhe läuft noch {1}. Ein Angriff beendet sie, mit diesem Gegner und seinen Verbündeten.",
    ),
    ("You broke the truce: {1} will fight back.", "Du hast die Waffenruhe gebrochen: {1} wird zurückschlagen."),
    (
        "{1} knows your coast: expect ships within the first hour.",
        "{1} kennt deine Küste: Rechne in der ersten Stunde mit Schiffen.",
    ),
    ("Time limit", "Zeitlimit"),
    ("none", "keine"),
    ("Peace", "Frieden"),
    ("Until then nobody can attack", "Bis dahin kann niemand angreifen"),
    ("Time left", "Restzeit"),
    ("Then the side with the most land wins", "Dann gewinnt die Seite mit dem meisten Land"),
    ("Time is up! You hold the most land: victory.", "Die Zeit ist um! Du hast das meiste Land: Sieg."),
    ("Time is up! {1} holds the most land.", "Die Zeit ist um! {1} hat das meiste Land."),
    (
        "Time is up! The land is shared equally: a draw.",
        "Die Zeit ist um! Das Land ist gleich verteilt: unentschieden.",
    ),
    (
        "The first gold coin is minted: coins promote the soldiers of the military buildings they reach.",
        "Die erste Goldmünze ist geprägt: Münzen befördern die Soldaten der Militärgebäude, die sie erreichen.",
    ),
    (
        "A general at last, your strongest soldier. Send generals where the fighting is.",
        "Endlich ein General, dein stärkster Soldat. Schicke Generäle dorthin, wo gekämpft wird.",
    ),
    (
        "Your first ship is on the water: harbors can now ship wares across the sea and send out expeditions.",
        "Dein erstes Schiff ist auf dem Wasser: Häfen können jetzt Waren über das Meer schicken und Expeditionen aussenden.",
    ),
    ("Peace time: no attacks for {1}.", "Friedenszeit: keine Angriffe für {1}."),
    ("💾 Save / load", "💾 Speichern / Laden"),
    ("Quick save F5, quick load F9", "Schnellspeichern F5, Schnellladen F9"),
    ("⚡ Quick save", "⚡ Schnellspeichern"),
    ("Save / load", "Speichern / Laden"),
    ("Name", "Name"),
    ("No saved games yet.", "Noch keine gespeicherten Spiele."),
    ("Load a saved game", "Gespeichertes Spiel laden"),
    ("Greenland", "Grünland"),
    ("Wasteland", "Ödland"),
    ("Winter world", "Winterwelt"),
    ("Building", "Gebäude"),
    ("+", "+"),
    ("−", "−"),
    ("Wares and settlers in your warehouses", "Waren und Siedler in deinen Lagerhäusern"),
    ("Which buildings get scarce wares", "Welche Gebäude knappe Waren bekommen"),
    ("Order in which carriers move wares", "Reihenfolge, in der Träger Waren befördern"),
    ("What the metalworks makes", "Was die Schlosserei herstellt"),
    ("Military settings", "Militäreinstellungen"),
    ("Compare the players", "Die Spieler vergleichen"),
    ("Sound and display", "Ton und Anzeige"),
    ("Controls", "Steuerung"),
    (
        "(on a road — too close to a flag for another one)",
        "(auf einer Straße — zu nah an einer Fahne für eine weitere)",
    ),
    ("title", "Titel"),
    ("Roads & Realms", "Roads & Realms"),
    ("water", "Wasser"),
    ("fish", "Fisch"),
    ("nothing", "nichts"),
    ("Pause (P)", "Pause (P)"),
    ("Fewer soldiers", "Weniger Soldaten"),
    ("More soldiers", "Mehr Soldaten"),
    ("seed ", "Startwert "),
    ("Save failed:", "Speichern fehlgeschlagen:"),
    ("Load failed:", "Laden fehlgeschlagen:"),
    ("Donkey breeder (job)", "Eselzüchter"),
    ("You", "Du"),
    ("AI", "KI"),
    ("You (Italy)", "Du (Italien)"),
    ("You (Carthago)", "Du (Karthago)"),
    ("You (Virginia)", "Du (Virginia)"),
    ("You (Zhongyuan)", "Du (Zhongyuan)"),
    ("You (Svealand)", "Du (Svealand)"),
    ("Livia of Iberia", "Livia von Iberien"),
    ("Sigrid of Britain", "Sigrid von Britannien"),
    ("Ragnar of Scandinavia", "Ragnar von Skandinavien"),
    ("Bjorn of Germania", "Björn von Germanien"),
    ("Hadrian of Sarmatia", "Hadrian von Sarmatien"),
    ("Viriato of Hispania", "Viriato von Hispanien"),
    ("Cornelia of Roma", "Cornelia von Rom"),
    ("Leonidas of Graecia", "Leonidas von Griechenland"),
    ("Tarhun of Anatolia", "Tarhun von Anatolien"),
    ("Nefret of Aegyptus", "Nefret von Ägypten"),
    ("Ezra of New England", "Ezra von Neuengland"),
    ("Mary of the Great Lakes", "Mary von den Großen Seen"),
    ("Santiago of Texas", "Santiago von Texas"),
    ("Rosa of California", "Rosa von Kalifornien"),
    ("Elias of Cascadia", "Elias von Kaskadien"),
    ("Temur of Manchuria", "Temur von der Mandschurei"),
    ("Min-ho of Korea", "Min-ho von Korea"),
    ("Hideko of Yamato", "Hideko von Yamato"),
    ("Lianhua of Jiangnan", "Lianhua von Jiangnan"),
    ("Wei of Lingnan", "Wei von Lingnan"),
    ("Halvard of Norvegr", "Halvard von Norvegr"),
    ("Aino of Suomi", "Aino von Suomi"),
    ("Talis of Livonia", "Talis von Livland"),
    ("Yaroslav of Novgorod", "Jaroslaw von Nowgorod"),
    ("Niila of Sapmi", "Niila von Sápmi"),
    ("not your land", "nicht dein Land"),
    ("no room to build there", "dort ist kein Platz zum Bauen"),
    ("there is no flag", "dort ist keine Fahne"),
    ("the road can't go there", "die Straße kann dort nicht entlang"),
    ("nothing there to do it with", "dafür ist nichts da"),
    ("not allowed", "nicht erlaubt"),
    ("The expedition found the harbor spot taken", "Die Expedition fand den Hafenplatz besetzt"),
    ("No messages.", "Keine Nachrichten."),
    ("Delete this message", "Diese Nachricht löschen"),
    ("🗑 Delete all", "🗑 Alle löschen"),
    ("Wares", "Waren"),
    (
        "Gold coins promote the soldiers here; switch deliveries on or off",
        "Goldmünzen befördern die Soldaten hier; Lieferung an- oder abschalten",
    ),
    ("15 min", "15 Min."),
    ("1 h", "1 Std."),
    ("4 h", "4 Std."),
    ("all", "alles"),
    ("under construction", "im Bau"),
    ("🏠 Buildings", "🏠 Gebäude"),
    ("How many of each building you have", "Wie viele Gebäude jeder Art du hast"),
    ("⚑ Give up", "⚑ Aufgeben"),
    ("Give up this game", "Dieses Spiel aufgeben"),
    ("Give up", "Aufgeben"),
    ("Really give up this game?", "Dieses Spiel wirklich aufgeben?"),
    ("Players", "Spieler"),
    ("Romans", "Römer"),
    ("Vikings", "Wikinger"),
    ("Nubians", "Nubier"),
    ("Japanese", "Japaner"),
    ("no team", "kein Team"),
    ("Team", "Team"),
    ("Stock settings", "Lagereinstellungen"),
    (
        "· normal  ⬇ collect here  ⛔ don't store  ⬆ move out",
        "· normal  ⬇ hier sammeln  ⛔ nicht einlagern  ⬆ auslagern",
    ),
    ("🗺 Map editor", "🗺 Karteneditor"),
    ("Give the map a name.", "Gib der Karte einen Namen."),
    ("Map editor", "Karteneditor"),
    ("brush", "Pinsel"),
    ("amount", "Menge"),
    ("Click to set, click again to remove.", "Klicken zum Setzen, nochmal klicken zum Entfernen."),
    ("Saved to", "Gespeichert unter"),
    ("▶ Try it", "▶ Ausprobieren"),
    ("Save, then play it against computer players", "Speichern und gegen Computerspieler spielen"),
    ("Set at least one headquarters first.", "Setze zuerst mindestens eine Burg."),
    ("New map", "Neue Karte"),
    ("Start a new map", "Neue Karte anfangen"),
    ("Terrain", "Gelände"),
    ("Raise", "Anheben"),
    ("Lower", "Absenken"),
    ("Flatten", "Einebnen"),
    ("Trees", "Bäume"),
    ("Clear", "Räumen"),
    ("Resources", "Bodenschätze"),
    ("Harbor spot", "Hafenplatz"),
    ("Meadow", "Wiese"),
    ("Meadow (2)", "Wiese (2)"),
    ("Meadow (3)", "Wiese (3)"),
    ("Flower meadow", "Blumenwiese"),
    ("Steppe", "Steppe"),
    ("Savannah", "Savanne"),
    ("Mountain meadow", "Bergwiese"),
    ("Desert", "Wüste"),
    ("Swamp", "Sumpf"),
    ("Mountain", "Berg"),
    ("Mountain (2)", "Berg (2)"),
    ("Mountain (3)", "Berg (3)"),
    ("Mountain (4)", "Berg (4)"),
    ("Snow", "Schnee"),
    ("Nothing", "Nichts"),
    ("Granite", "Granit"),
    ("No room for a headquarters of player", "Kein Platz für die Burg von Spieler"),
    ("Network game (LAN)", "Netzwerkspiel (LAN)"),
    ("Your name", "Dein Name"),
    ("Host a game", "Ein Spiel eröffnen"),
    ("Join", "Beitreten"),
    ("Hosting a network game", "Netzwerkspiel eröffnet"),
    ("Port", "Port"),
    (
        "The map, landscape and start goods: the menu's free game; the other slots go to computer players at normal level.",
        "Karte, Landschaft und Startwaren kommen aus dem freien Spiel im Menü; die übrigen Plätze bekommen Computerspieler auf „normal“.",
    ),
    (
        "Not in network games: peace time, time limit, land and coin goals, the Players table's levels, allied computer players.",
        "Nicht im Netzwerkspiel: Friedenszeit, Zeitlimit, Land- und Münzziele, die Stufen der Spielertabelle, verbündete Computerspieler.",
    ),
    (
        "Network games play the classic rules, not those of --rules.",
        "Netzwerkspiele laufen nach den klassischen Regeln, nicht nach denen von --rules.",
    ),
    ("Start", "Starten"),
    ("Joined a network game", "Einem Netzwerkspiel beigetreten"),
    ("Waiting for the host to start…", "Warten, bis der Gastgeber startet…"),
    ("Waiting for the other players…", "Warten auf die anderen Spieler…"),
    ("Network game ended:", "Netzwerkspiel beendet:"),
    ("Not in a network game.", "Nicht in einem Netzwerkspiel."),
    ("Credits", "Mitwirkende"),
    (
        "A non-commercial remake in the spirit of the 1996 classic. No original game data is included.",
        "Ein nichtkommerzielles Remake im Geist des Klassikers von 1996. Es enthält keine Daten des Originalspiels.",
    ),
    (
        "Some rules and timings come from Return to the Roots (GPL), with thanks to its authors.",
        "Einige Regeln und Zeiten stammen aus Return to the Roots (GPL), mit Dank an seine Autoren.",
    ),
    (
        "Music: RandomMind and Woli34 (OpenGameArt, CC0). Ambience: Spring Spring, RandomMind (CC0).",
        "Musik: RandomMind und Woli34 (OpenGameArt, CC0). Umgebungsgeräusche: Spring Spring, RandomMind (CC0).",
    ),
    (
        "Music: RandomMind, Woli34 · Sounds: Kenney (CC0) · Coastlines: Natural Earth",
        "Musik: RandomMind, Woli34 · Geräusche: Kenney (CC0) · Küsten: Natural Earth",
    ),
    ("Sounds: Kenney (kenney.nl, CC0).", "Geräusche: Kenney (kenney.nl, CC0)."),
    ("Coastlines: Natural Earth (public domain).", "Küstenlinien: Natural Earth (gemeinfrei)."),
    (
        "Title painting and three buildings: made with Google Gemini (agy); all other art: our own code.",
        "Titelbild und drei Gebäude: mit Google Gemini (agy) erstellt; alle übrige Grafik: unser eigener Code.",
    ),
    ("Code: GPL-3.0-or-later. Art pack: CC-BY-SA-4.0.", "Code: GPL-3.0-or-later. Grafikpaket: CC-BY-SA-4.0."),
    (
        "The host sent a game that can't be played.",
        "Der Gastgeber hat ein Spiel geschickt, das sich nicht spielen lässt.",
    ),
    ("A player left the game:", "Ein Spieler hat das Spiel verlassen:"),
    ("Animals", "Tiere"),
    ("Settler settings", "Einstellungen für Siedler"),
    ("Tips for beginners (in the messages)", "Tipps für Einsteiger (in den Nachrichten)"),
    (
        "Tip: build a woodcutter near trees. Click a spot with a house marker (Space shows where buildings fit).",
        "Tipp: Baue einen Holzfäller nahe bei Bäumen. Klicke auf einen Platz mit Hausmarkierung (Leertaste zeigt, wo Gebäude passen).",
    ),
    (
        "Tip: a new building needs a road. Click its flag, choose 'Build road' and click your way to another flag.",
        "Tipp: Ein neues Gebäude braucht eine Straße. Klicke auf seine Fahne, wähle „Straße bauen“ und klicke dich zu einer anderen Fahne.",
    ),
    ("Tip: stones come from a quarry built close to rocks.", "Tipp: Steine liefert ein Steinbruch nahe bei Felsen."),
    (
        "Tip: a sawmill turns wood into boards, which every building needs.",
        "Tipp: Ein Sägewerk macht aus Holz Bretter, die jedes Gebäude braucht.",
    ),
    (
        "Tip: guardhouses and barracks near the border make your land bigger.",
        "Tipp: Wachstuben und Baracken nahe der Grenze vergrößern dein Land.",
    ),
    (
        "Tip: boards are running low. More woodcutters, foresters and a second sawmill help.",
        "Tipp: Die Bretter gehen aus. Mehr Holzfäller, Förster und ein zweites Sägewerk helfen.",
    ),
    ("⟲ Undo", "⟲ Rückgängig"),
    ("As fast as possible", "So schnell wie möglich"),
    ("Back to normal speed", "Zurück zur normalen Geschwindigkeit"),
    ("👁 Show all", "👁 Alles zeigen"),
    (
        "The whole map: every realm's land, buildings and people",
        "Die ganze Karte: Land, Gebäude und Leute aller Reiche",
    ),
    ("All", "Alle"),
    ("Economy", "Wirtschaft"),
    ("Tips", "Tipps"),
    ("Other", "Sonstiges"),
    ("🏷 Names", "🏷 Namen"),
    ("Building names on the map (C)", "Gebäudenamen auf der Karte (C)"),
    ("just now", "gerade eben"),
    ("No road there: not your land.", "Keine Straße möglich: nicht dein Land."),
    ("No road there: something is in the way.", "Keine Straße möglich: Etwas steht im Weg."),
    ("Too far for one road: click a node closer by.", "Zu weit für eine Straße: einen näheren Punkt anklicken."),
    (
        "A road is in the way: click a node of it to put a flag there and join.",
        "Eine Straße ist im Weg: einen ihrer Punkte anklicken, um dort eine Fahne zu setzen und anzuschließen.",
    ),
    (
        "No way for a road there. Right-click or Cancel ends road building.",
        "Kein Weg für eine Straße. Rechtsklick oder Abbrechen beendet den Straßenbau.",
    ),
    ("Strong soldiers", "Starke Soldaten"),
    ("Weak soldiers", "Schwache Soldaten"),
    ("Really demolish?", "Wirklich abreißen?"),
    ("Yes", "Ja"),
    ("No", "Nein"),
    ("Start a road after placing a building", "Nach dem Bauen gleich eine Straße beginnen"),
    ("Fog of war (land out of sight is dimmed)", "Kriegsnebel (Land außer Sicht wird abgedunkelt)"),
    ("% Productivity", "% Produktivität"),
    ("Productivity and construction progress (O)", "Produktivität und Baufortschritt (O)"),
    ("Show where buildings fit (Space)", "Zeigt, wo Gebäude passen (Leertaste)"),
    ("Watch the last game", "Letztes Spiel ansehen"),
    (
        "The last game's replay can't be played: it was recorded with another version of the game, or the file is damaged.",
        "Die Aufzeichnung des letzten Spiels lässt sich nicht abspielen, denn sie stammt aus einer anderen Version des Spiels, oder die Datei ist beschädigt.",
    ),
    ("Replay", "Wiederholung"),
    ("Mare Nostrum", "Mare Nostrum"),
    ("North America", "Nordamerika"),
    ("The Far East", "Ferner Osten"),
    ("The North", "Der Norden"),
    (
        "Winter lies over Svealand, and your people must build in the snow. Five rulers of the North watch you: Halvard of Norvegr, Aino of Suomi, Talis of Livonia, Yaroslav of Novgorod and Niila of Sapmi.\n\nThe taiga gives all the wood you want, but little grows in the fields. The Scandes hide gold; the Khibiny in the far north hold iron and more.",
        "Winter liegt über Svealand, und dein Volk muss im Schnee bauen. Fünf Herrscher des Nordens beobachten dich: Halvard von Norvegr, Aino von Suomi, Talis von Livland, Jaroslaw von Nowgorod und Niila von Sápmi.\n\nDie Taiga gibt Holz in Hülle und Fülle, doch auf den Feldern wächst wenig. Die Skanden bergen Gold; die Chibinen im hohen Norden Eisen und mehr.",
    ),
    (
        "Your people farm the Central Plain on the Yellow River. Five rulers share the land with you: Temur of Manchuria, Min-ho of Korea, Hideko of Yamato, Lianhua of Jiangnan and Wei of Lingnan.\n\nThe Gobi in the north is barren; the Qinling and the Khingan hide gold. Yamato lies across the sea — only a harbor and a ship will bring your soldiers there.",
        "Dein Volk bestellt die Zentralebene am Gelben Fluss. Fünf Herrscher teilen das Land mit dir: Temur von der Mandschurei, Min-ho von Korea, Hideko von Yamato, Lianhua von Jiangnan und Wei von Lingnan.\n\nDie Gobi im Norden ist karg; Qinling und Khingan bergen Gold. Yamato liegt jenseits des Meeres — nur ein Hafen und ein Schiff bringen deine Soldaten dorthin.",
    ),
    (
        "Carthage rules the harbors of Africa, but the sea is crowded. Around its shores five powers each go their own way: Viriato of Hispania, Cornelia of Roma, Leonidas of Graecia, Tarhun of Anatolia and Nefret of Aegyptus.\n\nThe Sahara gives nothing; your wealth lies in the coastal plains and in the Atlas. Gibraltar, Messina and the Bosporus can be crossed on foot — or build harbors and send your ships.",
        "Karthago beherrscht die Häfen Afrikas, doch auf dem Meer herrscht Gedränge. Rings um seine Küsten geht jede der fünf Mächte ihren eigenen Weg: Viriato von Hispanien, Cornelia von Rom, Leonidas von Griechenland, Tarhun von Anatolien und Nefret von Ägypten.\n\nDie Sahara gibt nichts her; dein Reichtum liegt in den Küstenebenen und im Atlas. Gibraltar, Messina und der Bosporus lassen sich zu Fuß überqueren — oder baue Häfen und schicke deine Schiffe.",
    ),
    (
        "Your settlers have landed in Virginia, between the Appalachians and the sea. The continent is wide, and five rivals are already marching: Ezra of New England, Mary of the Great Lakes, Santiago of Texas, Rosa of California and Elias of Cascadia.\n\nThe plains feed many farms but little wood grows there. The Sierra Nevada and the Black Hills hide gold; the Appalachians are full of coal.",
        "Deine Siedler sind in Virginia gelandet, zwischen den Appalachen und dem Meer. Der Kontinent ist weit, und fünf Gegner sind schon unterwegs: Ezra von Neuengland, Mary von den Großen Seen, Santiago von Texas, Rosa von Kalifornien und Elias von Kaskadien.\n\nDie Prärie ernährt viele Höfe, aber dort wächst wenig Holz. In der Sierra Nevada und den Black Hills liegt Gold; die Appalachen sind voller Kohle.",
    ),
    (
        "Play a map file (.rrmap from the editor, .wld/.swd e.g. from your own copy)",
        "Kartendatei spielen (.rrmap aus dem Editor, .wld/.swd z. B. aus deinem Original)",
    ),
    ("Look for map files again", "Nochmals nach Kartendateien suchen"),
    ("Play this map", "Diese Karte spielen"),
    ("⛵ Send the expedition here", "⛵ Expedition hierher schicken"),
    ("⛵ Prepare an expedition", "⛵ Expedition vorbereiten"),
    (
        "Expedition ready: click a free harbor spot on another coast.",
        "Expedition bereit: einen freien Hafenplatz an einer anderen Küste anklicken.",
    ),
    ("Expedition:", "Expedition:"),
    ("builder", "Bauarbeiter"),
    ("Waiting for a ship.", "Wartet auf ein Schiff."),
    ("Ships here:", "Schiffe hier:"),
    ("Gold: off", "Gold: aus"),
    // buildings (classic German names)
    // (shown as "Castle"; kept for messages in older saves)
    ("Headquarters", "Burg"),
    ("Castle", "Burg"),
    ("Barracks", "Baracke"),
    ("Guardhouse", "Wachstube"),
    ("Watchtower", "Wachturm"),
    ("Fortress", "Festung"),
    ("Lookout tower", "Spähturm"),
    ("Catapult", "Katapult"),
    ("Granite mine", "Granitbergwerk"),
    ("Coal mine", "Kohlebergwerk"),
    ("Iron mine", "Eisenbergwerk"),
    ("Gold mine", "Goldbergwerk"),
    ("Woodcutter", "Holzfäller"),
    ("Fishery", "Fischer"),
    ("Quarry", "Steinbruch"),
    ("Forester", "Förster"),
    ("Slaughterhouse", "Schlachterei"),
    ("Hunter", "Jäger"),
    ("Brewery", "Brauerei"),
    ("Armory", "Waffenschmiede"),
    ("Metalworks", "Schlosserei"),
    ("Iron smelter", "Eisenschmelze"),
    ("Pig farm", "Schweinezucht"),
    ("Storehouse", "Lagerhaus"),
    ("Mill", "Mühle"),
    ("Bakery", "Bäckerei"),
    ("Sawmill", "Sägewerk"),
    ("Mint", "Münzprägerei"),
    ("Well", "Brunnen"),
    ("Shipyard", "Werft"),
    ("Farm", "Bauernhof"),
    ("Donkey breeder", "Eselzucht"),
    ("Harbor", "Hafen"),
    ("Construction", "Baustellen"),
    ("Food", "Nahrung"),
    // wares
    ("Beer", "Bier"),
    ("Tongs", "Zange"),
    ("Hammer", "Hammer"),
    ("Axe", "Axt"),
    ("Saw", "Säge"),
    ("Pick axe", "Spitzhacke"),
    ("Shovel", "Schaufel"),
    ("Crucible", "Schmelztiegel"),
    ("Rod and line", "Angel"),
    ("Scythe", "Sense"),
    ("Water", "Wasser"),
    ("Cleaver", "Beil"),
    ("Rolling pin", "Nudelholz"),
    ("Bow", "Bogen"),
    ("Boat", "Boot"),
    ("Sword", "Schwert"),
    ("Iron", "Eisen"),
    ("Flour", "Mehl"),
    ("Fish", "Fisch"),
    ("Bread", "Brot"),
    ("Shield", "Schild"),
    ("Wood", "Holz"),
    ("Boards", "Bretter"),
    ("Stones", "Steine"),
    ("Grain", "Getreide"),
    ("Coins", "Münzen"),
    ("Gold", "Gold"),
    ("Iron ore", "Eisenerz"),
    ("Coal", "Kohle"),
    ("Meat", "Fleisch"),
    // (the pig farm's ware goes to the slaughterhouse alive: "Schinken" before "Fleisch" read
    // backwards; owner, 2026-10-27)
    ("Ham", "Schwein"),
    // jobs
    ("Helper", "Träger"),
    ("Fisher", "Fischer"),
    ("Carpenter", "Schreiner"),
    ("Stonemason", "Steinmetz"),
    ("Farmer", "Bauer"),
    ("Miller", "Müller"),
    ("Baker", "Bäcker"),
    ("Butcher", "Metzger"),
    ("Miner", "Bergmann"),
    ("Brewer", "Brauer"),
    ("Pig breeder", "Schweinezüchter"),
    ("Iron founder", "Schmelzer"),
    ("Minter", "Münzpräger"),
    ("Metalworker", "Schlosser"),
    ("Armorer", "Schmied"),
    ("Builder", "Bauarbeiter"),
    ("Planer", "Planierer"),
    ("Today's map", "Karte des Tages"),
    (
        "{1} nodes of our land went to {2}: a post of theirs is nearer.",
        "{1} Felder unseres Landes gingen an {2}: Ein Militärgebäude von dort liegt näher.",
    ),
    (
        "{1}'s castle fell to {2}: to win the mission now, defeat all rivals.",
        "Die Burg von {1} ist an {2} gefallen: Für den Sieg musst du jetzt alle Gegner besiegen.",
    ),
    (
        "The truce holds both ways: if you attack your rival, it fights back at once.",
        "Die Waffenruhe gilt für beide Seiten: Greifst du deinen Gegner an, schlägt er sofort zurück.",
    ),
    (
        "Planned (⚒): the work starts when the game runs again ({1}).",
        "Geplant (⚒): Die Arbeit beginnt, wenn das Spiel weiterläuft ({1}).",
    ),
    (
        "{1} burned: {2}'s new territory took its land.",
        "{1} abgebrannt: Der Platz liegt jetzt im neuen Gebiet von {2}.",
    ),
    (
        "The truce ends in 5 minutes, and you have no military building yet: build guardhouses or barracks toward your rivals.",
        "Die Waffenruhe endet in 5 Minuten, und du hast noch kein Militärgebäude: Baue Wachstuben oder Baracken in Richtung deiner Gegner.",
    ),
    (
        "Fast until something needs you: an attack, a loss, the truce, a pause",
        "Schnell, bis du gebraucht wirst: ein Angriff, ein Verlust, die Waffenruhe, eine Pause",
    ),
    ("On the way: {1}", "Unterwegs: {1}"),
    ("{1} to building sites", "{1} zu Baustellen"),
    ("{1} for: {2}", "{1} für: {2}"),
    ("{1} without a goal yet", "{1} noch ohne Ziel"),
    (
        "The same map for everyone who plays today (4 players, size 128, Greenland): compare your game with friends",
        "Heute spielen alle dieselbe Karte (4 Spieler, Größe 128, Grünland): Vergleiche dein Spiel mit Freunden",
    ),
    ("Private", "Gefreiter"),
    ("Private first class", "Obergefreiter"),
    ("Sergeant", "Unteroffizier"),
    ("Officer", "Offizier"),
    ("General", "General"),
    ("Geologist", "Geologe"),
    ("Shipwright", "Schiffsbauer"),
    ("Scout", "Späher"),
    ("Pack donkey", "Packesel"),
];

#[cfg(test)]
mod tests {
    /// New-player round 10, #8: one name for the castle; "headquarters" only in the editor's
    /// texts, and the German attack line without a stray colon.
    /// Each English text has one German entry (a doubled "gold" slipped in with item 466; the
    /// map keeps only one of two, so a second one is dead or, worse, overrides the first).
    #[test]
    fn no_text_is_translated_twice() {
        let mut seen = std::collections::HashSet::new();
        for (en, _) in DE {
            assert!(seen.insert(*en), "translated twice: {en:?}");
        }
    }

    #[test]
    fn the_castle_has_one_name() {
        let editor = [
            "The map has no headquarters.",
            "Set at least one headquarters first.",
            "No room for a headquarters of player",
            "Headquarters",
        ];
        for (en, de) in DE {
            if editor.contains(en) {
                continue;
            }
            assert!(!en.to_lowercase().contains("headquarters"), "{en}");
            assert!(!de.contains("Hauptquartier"), "{de}");
        }
        let line = DE.iter().find(|(en, _)| *en == "{1} sends {3} soldiers against our {2} ({4} inside)").unwrap().1;
        assert!(!line.contains(": {2}"), "{line}");
    }

    use super::*;

    /// a11y roast #7: messages are kept in English and shown in the language chosen now.
    #[test]
    fn messages_follow_the_language() {
        let m = msg("{1} finished", &["Woodcutter"]);
        assert_eq!(compose(&m, |k| k), "Woodcutter finished");
        assert_eq!(compose(&m, tr_de), format!("{} fertig", tr_de("Woodcutter")));
        // a player's name is never looked up (architect round 3, #12: "Wood" showed as "Holz")
        let named = msg("{1} has been defeated", &[&rr_game::plain("Wood")]);
        assert!(compose(&named, tr_de).starts_with("Wood "), "{}", compose(&named, tr_de));
        assert!(!compose(&named, |k| k).contains(rr_game::PLAIN));
        // older saves keep their text
        assert_eq!(show("Holzfäller fertig"), "Holzfäller fertig");
        for t in TEMPLATES {
            assert!(table().contains_key(t), "{t} has no German");
        }
    }

    fn tr_de(k: &str) -> &str {
        table().get(k).copied().unwrap_or(k)
    }

    /// German round 1, #1: one German word per concept (see the glossary above `DE`).
    #[test]
    fn german_terms_follow_the_glossary() {
        let banned = ["Rivale", "Waffenstillstand", "Wachhaus", "Fischerei", "Kundschafter", "Farmen", "Bjorn"];
        for (en, de) in DE {
            for b in banned {
                assert!(!de.contains(b), "{b:?} in the German of {en:?}: {de}");
            }
            // "Schmiede" only as "Waffenschmiede" (the verb "schmiede" is lower case)
            for (i, _) in de.match_indices("Schmiede") {
                assert!(de[..i].ends_with("Waffen"), "\"Schmiede\" in the German of {en:?}: {de}");
            }
        }
        // the old fragments that glued sentences in English order are gone (#3)
        for k in ["min ago", "h ago", "days ago", "Belongs to", "The military settings send", "Build"] {
            assert!(!table().contains_key(k), "{k}");
        }
        assert!(!include_str!("app.rs").contains("\"× your land.\""));
    }

    /// German round 1, #5: "du" throughout, imperatives with -e, capitals after a colon.
    #[test]
    fn german_keeps_one_register() {
        let word = |de: &str, w: &str| {
            de.match_indices(w).any(|(i, _)| {
                let before = de[..i].chars().next_back().is_none_or(|c| !c.is_alphabetic());
                let after = de[i + w.len()..].chars().next().is_none_or(|c| !c.is_alphabetic());
                before && after
            })
        };
        let banned = [
            "euch", "eure", "euren", "eurem", "eurer", "eures", "greif", "schick", "Schick", "Stell", "stell", "Mach",
            "mach",
        ];
        for (en, de) in DE {
            for w in banned {
                assert!(!word(de, w), "{w:?} in the German of {en:?}: {de}");
            }
            for formal in [": Sie ", ": Ihre ", ": Ihr ", ": Ihnen "] {
                assert!(!de.contains(formal), "{formal:?} in the German of {en:?}: {de}");
            }
        }
        for de in ["Die Waffenruhe ist vorbei: Deine", "anhalten: Die anderen", "Wache: Der Turm", "Wüste: Der Krieg"]
        {
            assert!(DE.iter().any(|(_, d)| d.contains(de)), "{de}");
        }
    }

    /// German round 1, #9/#10: one colon per sentence, „…“ for quotes, "25 %".
    #[test]
    fn german_typography() {
        for (en, de) in DE {
            let first = de.split(['.', '!', '?', ';', '·', '\n']).next().unwrap_or("");
            assert!(first.matches(':').count() < 2, "two colons in one sentence, German of {en:?}: {de}");
            if *en != "Begin" {
                assert!(!de.contains('\''), "a straight quote in the German of {en:?}: {de}");
            }
        }
        assert_eq!(pct_in(25, true), "25 %");
        assert_eq!(pct_in(25, false), "25%");
        assert_eq!(tr_de("{1} min").replace("{1}", "30"), "30 Min.");
    }

    /// German round 1, #4: templates filled with one and with the castle don't assume a gender
    /// or a plural ("Burg … seine Waren … mit ihm").
    #[test]
    fn german_templates_fit_any_building() {
        for t in TEMPLATES {
            let m = msg(t, &["Castle", "1", "1", "1"]);
            let de = compose(&m, tr_de);
            // (counts of one have templates of their own: the callers' tests check they are used)
            // (and "sein Land": a Mühle or Burg is no "sein"; German round 2, #1)
            for b in ["seine Waren", "sein Land", "mit ihm", "ihn ", "ihm "] {
                assert!(!de.contains(b), "{b:?} in {de}");
            }
        }
        let one = tr_de("Player {1}: only 1 tree near the castle: little wood to cut.");
        assert!(one.contains("nur 1 Baum "), "{one}");
        let lost = compose(&msg("{1} lost to {2}: its wares and settlers are gone with it.", &["Castle", "X"]), tr_de);
        assert_eq!(lost, "Burg an X verloren: Alle Waren und Siedler darin sind fort.");
    }

    /// Every `tr("…")` literal in the UI has a German text.
    /// QA round 5, #7: a tip's bulb is kept in German.
    #[test]
    fn german_keeps_the_bulb() {
        for (en, de) in DE {
            assert_eq!(en.starts_with('💡'), de.starts_with('💡'), "{en}");
        }
    }

    /// The campaign's story (story round 1): every line of it has its German.
    #[test]
    fn the_story_is_told_in_german() {
        for line in rr_game::story::all_lines() {
            assert!(table().contains_key(line), "no German: {line}");
        }
    }

    #[test]
    fn every_ui_text_has_a_german_translation() {
        let sources = [
            include_str!("ui.rs"),
            include_str!("menu.rs"),
            include_str!("app.rs"),
            include_str!("main.rs"),
            include_str!("missions.rs"),
            include_str!("hover.rs"),
            include_str!("editor.rs"),
            include_str!("netui.rs"),
        ];
        let table = super::table();
        let mut missing = Vec::new();
        for src in sources {
            let mut rest = src;
            while let Some(i) = rest.find("tr(\"") {
                rest = &rest[i + 4..];
                let mut key = String::new();
                let mut chars = rest.char_indices();
                let mut end = 0;
                while let Some((j, c)) = chars.next() {
                    match c {
                        '\\' => match chars.next() {
                            Some((_, 'n')) => key.push('\n'),
                            Some((_, x)) => key.push(x),
                            None => {}
                        },
                        '"' => {
                            end = j;
                            break;
                        }
                        c => key.push(c),
                    }
                }
                rest = &rest[end..];
                if !table.contains_key(key.as_str()) && !missing.contains(&key) {
                    missing.push(key);
                }
            }
        }
        assert!(missing.is_empty(), "{} texts without German:\n{}", missing.len(), missing.join("\n"));
    }
}

#[cfg(test)]
mod glyph_tests {
    /// Every symbol the UI uses exists in egui's fonts (no "missing glyph" boxes).
    #[test]
    fn every_symbol_has_a_glyph() {
        let ctx = eframe::egui::Context::default();
        let _ = ctx.run(Default::default(), |_| {});
        let sources = [
            include_str!("ui.rs"),
            include_str!("menu.rs"),
            include_str!("app.rs"),
            include_str!("i18n.rs"),
            include_str!("editor.rs"),
        ];
        let mut chars: Vec<char> = sources
            .iter()
            .flat_map(|s| s.lines())
            .filter(|l| !l.trim_start().starts_with("//"))
            .flat_map(|l| l.chars())
            .filter(|c| !c.is_ascii() && !c.is_alphabetic() && !"—–·…’“”„«»×°".contains(*c))
            .collect();
        chars.sort();
        chars.dedup();
        let font = eframe::egui::FontId::proportional(14.0);
        let missing: Vec<char> = ctx.fonts(|f| chars.iter().copied().filter(|c| !f.has_glyph(&font, *c)).collect());
        assert!(missing.is_empty(), "no glyph for: {missing:?}");
    }
}
