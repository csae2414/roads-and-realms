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
const TEMPLATES: [&str; 98] = [
    "Goal: reach {1} allied towns ({2} so far)",
    "Favour: {1} of 100",
    "Offer {1} {2}",
    "In store: {1}",
    "Goal: take one rival military building ({1} so far)",
    "Goal: take {1} rival military buildings ({2} so far)",
    "Concordia's favour: {1} of 100. At 50, one battle fewer brings the peace.",
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
        "Your land lies all around the Capitol now, and Picus climbs to the augurs' platform. Twelve vultures circle over the hill. \"Jupiter has answered,\" says Picus. \"From up here you will see far.\"",
        "Dein Land liegt jetzt rund um das Kapitol, und Picus steigt auf die Plattform der Auguren. Zwölf Geier kreisen über dem Hügel. „Jupiter hat geantwortet“, sagt Picus. „Von hier oben wirst du weit sehen.“",
    ),
    // the Roman campaign, chapter II
    (
        "Sabine spears in the valley at dawn: Titus Tatius is at war with you. Picus: \"Now we will learn who keeps your gates.\"",
        "Sabinische Speere im Tal bei Tagesanbruch: Titus Tatius führt Krieg gegen dich. Picus: „Jetzt werden wir sehen, wer deine Tore hütet.“",
    ),
    ("The Sabine Peace", "Der Sabinerfrieden"),
    ("Titus Tatius", "Titus Tatius"),
    ("Goal: take two Sabine military buildings.", "Ziel: Nimm zwei Militärgebäude der Sabiner ein."),
    (
        "Chapter II: soldiers, a betrayal, and a first war that ends in peace.",
        "Kapitel II: Soldaten, ein Verrat und ein erster Krieg, der mit Frieden endet.",
    ),
    (
        "Rome has walls now, and too few women. At the feast of Consus your men seized the Sabines' daughters and made them their wives, and their fathers have not forgiven it. The Sabine king Titus Tatius rules the Quirinal across the valley, at peace with you for now.\n\nHere you learn war: a brewery for beer, swords and shields from your store, soldiers recruited in the castle, and your first attack. Keep your posts manned: Tatius will not wait forever. Between the two hills stands Concordia's shrine: where your land meets the Sabines' there, the women carry gifts across, and the peace comes sooner.",
        "Rom hat jetzt Mauern, aber zu wenige Frauen. Beim Fest des Consus haben deine Männer die Töchter der Sabiner geraubt und zu ihren Frauen gemacht, und ihre Väter haben es nicht verziehen. Der Sabinerkönig Titus Tatius herrscht auf dem Quirinal jenseits des Tals und hält vorerst Frieden mit dir.\n\nHier lernst du den Krieg: eine Brauerei für Bier, Schwerter und Schilde aus deinem Lager, Soldaten, die in der Burg ausgebildet werden, und deinen ersten Angriff. Halte deine Posten besetzt: Tatius wird nicht ewig warten. Zwischen den beiden Hügeln steht ein Heiligtum der Concordia: Wo dein Land dort an das der Sabiner stößt, tragen die Frauen Geschenke hinüber, und der Frieden kommt früher.",
    ),
    (
        "Rome stands, but it is a town of men. At the feast of Consus your men seized the daughters of their Sabine guests and made them their wives, and their fathers have not forgiven it. Across the valley, on the Quirinal, the Sabine king Titus Tatius gathers his people. Between the two hills stands an old shrine of Concordia. Picus, the augur, has served you since the vultures.",
        "Rom steht, aber es ist eine Stadt der Männer. Beim Fest des Consus haben deine Männer die Töchter ihrer sabinischen Gäste geraubt und zu ihren Frauen gemacht, und deren Väter haben es nicht verziehen. Jenseits des Tals, auf dem Quirinal, sammelt der Sabinerkönig Titus Tatius sein Volk. Zwischen den beiden Hügeln steht ein altes Heiligtum der Concordia. Picus, der Augur, dient dir seit den Geiern.",
    ),
    (
        "Sabine banners on the Quirinal. Titus Tatius sends no herald. Picus: \"A king who sends no word is counting his spears.\"",
        "Sabinische Banner auf dem Quirinal. Titus Tatius schickt keinen Herold. Picus: „Ein König, der kein Wort schickt, zählt seine Speere.“",
    ),
    (
        "Hersilia, your wife, comes to you: \"Our families are across the valley. Where your land meets theirs by Concordia's shrine, we can carry them bread, and they will see that we are well.\"",
        "Hersilia, deine Frau, kommt zu dir: „Unsere Familien sind jenseits des Tals. Wo dein Land beim Heiligtum der Concordia an ihres stößt, können wir ihnen Brot bringen, und sie sehen, dass es uns gut geht.“",
    ),
    (
        "The women have carried bread across the valley for many days now. Hersilia: \"Our fathers ask for us by name. One battle, and they will listen to us.\"",
        "Seit vielen Tagen tragen die Frauen Brot über das Tal. Hersilia: „Unsere Väter fragen nach uns, jeden beim Namen. Eine Schlacht, und sie werden auf uns hören.“",
    ),
    (
        "At night Tarpeia, the daughter of the commander of your post nearest the Sabines, opens its gate to them. She asked for what they wore on their left arms: she meant their gold bracelets, and they gave her their shields. (Another telling says she meant to trick them out of their shields.) The post is theirs, and its soldiers come home to you.",
        "In der Nacht öffnet Tarpeia, die Tochter des Befehlshabers deines Postens, der den Sabinern am nächsten liegt, ihnen das Tor. Sie verlangte, was die Sabiner am linken Arm trugen, und meinte damit ihre goldenen Armreifen; doch die Sabiner gaben ihr ihre Schilde. (Eine andere Erzählung sagt, sie wollte sie um ihre Schilde betrügen.) Der Posten gehört nun ihnen, und seine Soldaten kommen zu dir heim.",
    ),
    (
        "You have taken a Sabine post. Picus: \"Tatius has lost ground before. He has never lost his pride.\"",
        "Du hast einen Posten der Sabiner eingenommen. Picus: „Tatius hat schon früher Land verloren. Seinen Stolz hat er nie verloren.“",
    ),
    (
        "The armies meet in the marshy valley below the Capitol. Then the women run between the lines, their hair loose, their children in their arms: \"If you must fight, fight us, not each other. We are wives on one side and daughters on the other.\" The fathers lower their spears. Romulus and Titus Tatius make peace and rule together: Romans and Sabines, one people on two hills. Picus marks the place where the women stood. One day it will be the Forum.",
        "Die Heere treffen sich im sumpfigen Tal unter dem Kapitol. Da laufen die Frauen zwischen die Reihen, mit offenem Haar, die Kinder auf dem Arm: „Wenn ihr kämpfen müsst, dann kämpft gegen uns, nicht gegeneinander. Auf der einen Seite sind wir Ehefrauen, auf der anderen Töchter.“ Die Väter senken die Speere. Romulus und Titus Tatius schließen Frieden und herrschen gemeinsam: Römer und Sabiner, ein Volk auf zwei Hügeln. Picus markiert die Stelle, an der die Frauen standen. Eines Tages wird hier das Forum sein.",
    ),
    (
        "Goal: take one rival military building ({1} so far)",
        "Ziel: Nimm ein gegnerisches Militärgebäude ein ({1} bisher)",
    ),
    (
        "Goal: take {1} rival military buildings ({2} so far)",
        "Ziel: Nimm {1} gegnerische Militärgebäude ein ({2} bisher)",
    ),
    ("Concordia's shrine, a holy place", "Das Heiligtum der Concordia, ein heiliger Ort"),
    (
        "Concordia's favour: {1} of 100. At 50, one battle fewer brings the peace.",
        "Die Gunst der Concordia: {1} von 100. Ab 50 bringt eine Schlacht weniger den Frieden.",
    ),
    (
        "Your land and your rival's meet here: the women carry gifts across.",
        "Dein Land und das deines Gegners treffen sich hier: Die Frauen tragen Geschenke hinüber.",
    ),
    (
        "Bring your land up to her shrine, beside your rival's, and the women carry gifts across.",
        "Bring dein Land bis an ihr Heiligtum, neben das deines Gegners, und die Frauen tragen Geschenke hinüber.",
    ),
    // the Roman campaign, chapter III
    ("Veii", "Veji"),
    ("Camillus", "Camillus"),
    ("Goal: take the castle of Veii.", "Ziel: Nimm die Burg von Veji ein."),
    (
        "Chapter III: geologists, mines and the weapons chain against a fortified city.",
        "Kapitel III: Geologen, Bergwerke und die Waffenkette gegen eine befestigte Stadt.",
    ),
    (
        "Veii, the rich Etruscan city, stands on its rock across the valley, behind posts manned by trained soldiers. Its king will not wait for you: after the truce, Veii fights.\n\nHere you learn the weapons chain: send geologists into the hills, build mines and feed their miners (fish, meat or bread), smelt iron with coal, and let the armory make swords and shields. Only an army of many soldiers takes Veii. Juno's old altar stands on your side of the valley: where your land lies all around it, offerings of bread win her over.",
        "Veji, die reiche Stadt der Etrusker, steht auf ihrem Felsen jenseits des Tals, hinter Posten mit erfahrenen Soldaten. Ihr König wird nicht auf dich warten: Nach der Waffenruhe kämpft Veji.\n\nHier lernst du die Waffenkette: Schicke Geologen in die Hügel, baue Bergwerke und versorge ihre Bergleute (Fisch, Fleisch oder Brot), schmelze Eisen mit Kohle, und lass die Waffenschmiede Schwerter und Schilde machen. Nur ein Heer aus vielen Soldaten nimmt Veji ein. Junos alter Altar steht auf deiner Seite des Tals: Wo dein Land ganz um ihn liegt, gewinnen Opfergaben aus Brot die Göttin.",
    ),
    (
        "Rome has grown for three hundred years, and one city has always stood in its way: Veii, the rich Etruscan city on its rock across the valley, behind walls no army has broken. The senate sends you, Marcus Furius Camillus. Picus, of the augurs' house, rides with you: \"Veii's walls are stone. Its goddess is Juno. Take her, and the walls are nothing.\"",
        "Rom ist dreihundert Jahre lang gewachsen, und eine Stadt stand ihm immer im Weg: Veji, die reiche Stadt der Etrusker auf ihrem Felsen jenseits des Tals, hinter Mauern, die kein Heer gebrochen hat. Der Senat schickt dich, Marcus Furius Camillus. Picus aus dem Haus der Auguren reitet mit dir: „Vejis Mauern sind aus Stein. Seine Göttin ist Juno. Gewinne sie, und die Mauern sind nichts.“",
    ),
    (
        "Veii's towers on the rock. Its soldiers have trained for generations: you will need iron, and many swords. Picus: \"Geologists first. The hills here are full of ore.\"",
        "Vejis Türme auf dem Felsen. Seine Soldaten üben seit Generationen: Du wirst Eisen brauchen und viele Schwerter. Picus: „Zuerst Geologen. Die Hügel hier sind voller Erz.“",
    ),
    (
        "Picus shows you an old altar of Juno on your side of the valley, facing Veii: \"When your land lies all around it, offer bread there. Juno is the city's goddess; we will ask her to come to Rome.\"",
        "Picus zeigt dir einen alten Altar der Juno auf deiner Seite des Tals, mit Blick auf Veji: „Wenn dein Land ganz um ihn liegt, opfere dort Brot. Juno ist die Göttin der Stadt; wir werden sie bitten, nach Rom zu kommen.“",
    ),
    (
        "Smoke from Juno's altar drifts towards Veii. Picus: \"She listens. Keep the bread coming, though the miners grumble.\"",
        "Rauch von Junos Altar zieht nach Veji hinüber. Picus: „Sie hört zu. Bring weiter Brot, auch wenn die Bergleute murren.“",
    ),
    (
        "You have taken one of Veii's posts. Its soldiers fought like veterans, and they were. Picus: \"Every post we take is a gate less around the rock.\"",
        "Du hast einen Posten von Veji eingenommen. Seine Soldaten kämpften wie Veteranen, und das waren sie. Picus: „Jeder Posten, den wir nehmen, ist ein Tor weniger um den Felsen.“",
    ),
    (
        "Your miners dig a tunnel through the rock, right up under the temple of Juno in the citadel. Camillus asks the goddess aloud to leave the city and come to a temple worthy of her in Rome, and the story says she nodded. Veii falls after ten years of war. Juno's statue is carried to the Aventine. Picus writes in the augurs' book: \"We did not take her. She came.\"",
        "Deine Bergleute graben einen Tunnel durch den Felsen, bis unter den Tempel der Juno in der Zitadelle. Camillus bittet die Göttin laut, die Stadt zu verlassen und in einen Tempel nach Rom zu kommen, der ihrer würdig ist, und die Geschichte sagt, sie habe genickt. Veji fällt nach zehn Jahren Krieg. Junos Statue wird auf den Aventin getragen. Picus schreibt in das Buch der Auguren: „Wir haben sie nicht genommen. Sie ist gekommen.“",
    ),
    (
        "Picus comes running from the altar: \"Juno has heard you. She is leaving Veii for Rome.\" Soldiers waiting in your warehouses swear they saw her in a dream. They train with new heart: some of them are privates first class now.",
        "Picus kommt vom Altar gelaufen: „Juno hat dich gehört. Sie verlässt Veji und kommt nach Rom.“ Soldaten, die in deinen Lagern warten, schwören, sie im Traum gesehen zu haben. Sie üben mit neuem Mut: Einige von ihnen sind jetzt Obergefreite.",
    ),
    (
        "In Veii, the priests find Juno's statue turned towards Rome. More of your waiting soldiers rise a rank. Picus: \"A city its goddess has left is only walls.\"",
        "In Veji finden die Priester Junos Statue nach Rom gewandt. Weitere deiner wartenden Soldaten steigen einen Rang auf. Picus: „Eine Stadt, die ihre Göttin verlassen hat, ist nur noch Mauern.“",
    ),
    ("Juno's altar, a holy place", "Junos Altar, ein heiliger Ort"),
    (
        "Yours: click it to offer bread, and Juno may leave Veii for Rome.",
        "Deiner: Klicke ihn an, um Brot zu opfern, und Juno kommt vielleicht von Veji nach Rom.",
    ),
    ("Favour: {1} of 100", "Gunst: {1} von 100"),
    ("Offer {1} {2}", "Opfere {1} {2}"),
    ("Your land must lie all around it first.", "Zuerst muss dein Land ganz um ihn liegen."),
    ("From your warehouses, the castle first.", "Aus deinen Lagern, zuerst aus der Burg."),
    ("In store: {1}", "Im Lager: {1}"),
    // the Roman campaign, chapter IV
    ("The Geese of the Capitol", "Die Gänse des Kapitols"),
    ("Marcus Manlius", "Marcus Manlius"),
    ("Brennus", "Brennus"),
    ("The Senones", "Die Senonen"),
    ("Goal: hold out 45 minutes until Camillus comes.", "Ziel: Halte 45 Minuten aus, bis Camillus kommt."),
    (
        "Chapter IV: defence. Hold the hills against a stronger invader.",
        "Kapitel IV: Verteidigung. Halte die Hügel gegen einen stärkeren Eindringling.",
    ),
    (
        "The Gauls have beaten Rome's army at the Allia and are coming for the city: two war bands of the Senones, stronger than you and soon here. You cannot beat them in the field. Hold the hills until Camillus comes with an army.\n\nHere you learn defence: man the posts that face the north, set the military window so the border holds more soldiers than the inland, keep beer, swords and shields coming for new recruits, and use catapults. Juno's temple on the Capitol is yours: feed her sacred geese with grain, and they may wake the guards.",
        "Die Gallier haben Roms Heer an der Allia geschlagen und ziehen gegen die Stadt: zwei Kriegerscharen der Senonen, stärker als du und bald hier. Im Feld kannst du sie nicht schlagen. Halte die Hügel, bis Camillus mit einem Heer kommt.\n\nHier lernst du die Verteidigung: Besetze die Posten, die nach Norden blicken, stelle im Militärfenster ein, dass an der Grenze mehr Soldaten stehen als im Inneren, halte Bier, Schwerter und Schilde für neue Rekruten bereit und nutze Katapulte. Junos Tempel auf dem Kapitol gehört dir: Füttere ihre heiligen Gänse mit Getreide, und sie wecken vielleicht die Wachen.",
    ),
    (
        "Six years after Veii the Gauls come over the mountains: the Senones under their chief Brennus. At the river Allia they scatter Rome's army in a single afternoon, and the road to the city lies open. Rome cannot fight them in the field. You hold the hills, and above all the Capitol, where Juno's sacred geese are kept. Picus: \"Hold out until Camillus comes. He is raising an army.\"",
        "Sechs Jahre nach Veji kommen die Gallier über die Berge: die Senonen unter ihrem Anführer Brennus. Am Fluss Allia zerstreuen sie Roms Heer an einem einzigen Nachmittag, und der Weg zur Stadt liegt offen. Im Feld kann Rom sie nicht schlagen. Du hältst die Hügel, vor allem das Kapitol, wo Junos heilige Gänse gehalten werden. Picus: „Halte aus, bis Camillus kommt. Er sammelt ein Heer.“",
    ),
    (
        "Gaulish horns in the north. The Senones are taller than any Roman, and they fight with long swords. Picus: \"Do not go out to them. Let them come to the walls.\"",
        "Gallische Hörner im Norden. Die Senonen sind größer als jeder Römer und kämpfen mit langen Schwertern. Picus: „Zieh nicht hinaus zu ihnen. Lass sie an die Mauern kommen.“",
    ),
    (
        "Picus at Juno's temple: \"The geese are hungry, and so are we. But a fed goose is a watchful goose. Some grain for them, now and then.\"",
        "Picus an Junos Tempel: „Die Gänse sind hungrig, und wir auch. Aber eine gefütterte Gans ist eine wachsame Gans. Ab und zu etwas Getreide für sie.“",
    ),
    (
        "A post has fallen to the Gauls. Picus: \"Let them have the plain. Keep the hills.\"",
        "Ein Posten ist an die Gallier gefallen. Picus: „Lass ihnen die Ebene. Halte die Hügel.“",
    ),
    (
        "A messenger slips through the Gaulish lines: Camillus is coming with an army. Hold out a little longer.",
        "Ein Bote schlüpft durch die gallischen Linien: Camillus kommt mit einem Heer. Halte noch ein wenig aus.",
    ),
    (
        "Camillus arrives as Brennus is weighing the gold Rome must pay to be left alone, with false weights, and his sword thrown onto the scale: \"Vae victis\", woe to the defeated. Camillus answers that Rome pays with iron, not with gold, and drives the Gauls from the city. From that year on a goose is carried through Rome on a purple cushion. Picus writes: \"Rome was saved by birds, and by the men they woke.\"",
        "Camillus kommt, als Brennus das Gold wiegt, das Rom zahlen muss, um in Ruhe gelassen zu werden, mit falschen Gewichten und seinem Schwert auf der Waage: „Vae victis“, wehe den Besiegten. Camillus antwortet, Rom zahle mit Eisen, nicht mit Gold, und vertreibt die Gallier aus der Stadt. Von diesem Jahr an wird eine Gans auf einem purpurnen Kissen durch Rom getragen. Picus schreibt: „Rom wurde von Vögeln gerettet, und von den Männern, die sie weckten.“",
    ),
    (
        "At night the Gauls climb the rock of the Capitol, so quietly that not even the dogs wake. But Juno's geese cackle and beat their wings. You leap up and throw the first Gaul off the cliff, and the others fall with him. At dawn five men who were thought too old or too young to fight join the defenders.",
        "In der Nacht klettern die Gallier den Felsen des Kapitols hinauf, so leise, dass nicht einmal die Hunde aufwachen. Doch Junos Gänse schnattern und schlagen mit den Flügeln. Du springst auf und stößt den ersten Gallier vom Felsen, und die anderen stürzen mit ihm. Im Morgengrauen schließen sich fünf Männer, die man für zu alt oder zu jung zum Kämpfen hielt, den Verteidigern an.",
    ),
    ("Juno's temple and her sacred geese, a holy place", "Junos Tempel und ihre heiligen Gänse, ein heiliger Ort"),
    (
        "Yours: click it to feed the sacred geese grain. Well fed, they may wake the guards.",
        "Deiner: Klicke ihn an, um den heiligen Gänsen Getreide zu geben. Gut gefüttert, wecken sie vielleicht die Wachen.",
    ),
    // the Roman campaign, chapters V-X
    (
        "Before Carthage, the soldiers who wait in your warehouses drill as they did on the Field of Mars at home. Some of them are promoted: privates first class now.",
        "Vor Karthago üben die Soldaten, die in deinen Lagern warten, wie daheim auf dem Marsfeld. Einige von ihnen werden befördert: Jetzt sind sie Obergefreite.",
    ),
    (
        "Mars stays with the army before Carthage. More of your soldiers rise a rank.",
        "Mars bleibt beim Heer vor Karthago. Weitere deiner Soldaten steigen einen Rang auf.",
    ),
    ("The Britons", "Die Britannier"),
    ("The Suebi", "Die Sueben"),
    ("The Dacians", "Die Daker"),
    ("The Appian Way", "Die Via Appia"),
    (
        "Chapter V: long roads, donkeys and warehouses to reach three allied towns.",
        "Kapitel V: lange Straßen, Esel und Lagerhäuser, um drei verbündete Städte zu erreichen.",
    ),
    (
        "Goal: reach the three allied towns: your land must touch theirs.",
        "Ziel: Erreiche die drei verbündeten Städte; dein Land muss an ihres stoßen.",
    ),
    (
        "The Samnites hold the mountains, and once they trapped a Roman army in the pass of the Caudine Forks. Never again, says the censor Appius Claudius: Rome will build a road, paved and straight, to its allies in the south, so that the legions reach them in days, not weeks.\n\nHere you learn logistics: long roads, flags close together, donkeys on the busy roads (a donkey breeder), warehouses on the way, and the transport window's order. Reach the allied towns of Capua, Cumae and Neapolis: your land must touch theirs. The Samnites will try to cut your way. Janus's gates stand on the road: hold them, and the god of beginnings sends you helpers.",
        "Die Samniten halten die Berge, und einmal haben sie ein römisches Heer im Pass der Caudinischen Gabeln gefangen. Nie wieder, sagt der Zensor Appius Claudius: Rom baut eine Straße, gepflastert und gerade, zu seinen Verbündeten im Süden, damit die Legionen sie in Tagen erreichen statt in Wochen.\n\nHier lernst du den Nachschub: lange Straßen, Fahnen dicht beieinander, Esel auf den belebten Straßen (ein Eselzüchter), Lagerhäuser unterwegs und die Reihenfolge im Transportfenster. Erreiche die verbündeten Städte Capua, Cumae und Neapolis: Dein Land muss an ihres stoßen. Die Samniten werden versuchen, deinen Weg zu durchschneiden. An der Straße stehen die Tore des Janus: Halte sie, und der Gott der Anfänge schickt dir Helfer.",
    ),
    (
        "312 BC. The war with the Samnites drags on in the mountains of the south, and Rome's allies there wait weeks for every legion. The censor Appius Claudius has a plan the senate laughs at: a road of stone from Rome to Capua, straight as a spear. Picus of the augurs' house marks the first stone: \"Janus opens every road. Ask him well.\"",
        "312 v. Chr. Der Krieg mit den Samniten zieht sich in den Bergen des Südens hin, und Roms Verbündete dort warten Wochen auf jede Legion. Der Zensor Appius Claudius hat einen Plan, über den der Senat lacht: eine Straße aus Stein von Rom nach Capua, gerade wie ein Speer. Picus aus dem Haus der Auguren setzt den ersten Stein: „Janus öffnet jeden Weg. Bitte ihn gut.“",
    ),
    (
        "The Appian Way reaches Capua, and the legions march south in five days. The Samnites fight on for years, but they never trap a Roman army again. Appius Claudius goes blind in his old age; the road he built still carries Rome. Picus writes: \"Some gods answer with birds. Janus answered with a road.\"",
        "Die Via Appia erreicht Capua, und die Legionen marschieren in fünf Tagen nach Süden. Die Samniten kämpfen noch jahrelang, aber nie wieder fangen sie ein römisches Heer. Appius Claudius erblindet im Alter; die Straße, die er gebaut hat, trägt Rom noch immer. Picus schreibt: „Manche Götter antworten mit Vögeln. Janus antwortete mit einer Straße.“",
    ),
    (
        "Samnite riders on the ridges. Picus: \"They remember the Caudine Forks. So do we.\"",
        "Samnitische Reiter auf den Kämmen. Picus: „Sie erinnern sich an die Caudinischen Gabeln. Wir auch.“",
    ),
    (
        "The first miles are paved. Picus: \"Roads are slow to build and quick to cut. Warehouses on the way, and donkeys where the carriers wait.\"",
        "Die ersten Meilen sind gepflastert. Picus: „Straßen baut man langsam und durchschneidet sie schnell. Lagerhäuser unterwegs, und Esel, wo die Träger warten.“",
    ),
    (
        "The Samnites take a post by the road. Your eyes are already failing, and you only say: \"Rebuild it, and pave further.\"",
        "Die Samniten nehmen einen Posten an der Straße. Deine Augen werden schon schwach, und du sagst nur: „Baut ihn wieder auf, und pflastert weiter.“",
    ),
    (
        "Your land closes around the old gates of Janus by the road. Picus opens them: \"Janus looks both ways, back to Rome and on to the south.\" Three men from the road's camps take up arms for you.",
        "Dein Land schließt sich um die alten Tore des Janus an der Straße. Picus öffnet sie: „Janus blickt in beide Richtungen, zurück nach Rom und weiter nach Süden.“ Drei Männer aus den Lagern an der Straße greifen für dich zu den Waffen.",
    ),
    ("Appius Claudius", "Appius Claudius"),
    ("Capua", "Capua"),
    ("Cumae", "Cumae"),
    ("Neapolis", "Neapolis"),
    ("The Samnites", "Die Samniten"),
    ("A Pyrrhic Victory", "Ein Pyrrhussieg"),
    (
        "Chapter VI: soldier quality, gold and promotions against a master of war.",
        "Kapitel VI: Qualität der Soldaten, Gold und Beförderungen gegen einen Meister des Krieges.",
    ),
    ("Goal: take the castle of Pyrrhus.", "Ziel: Nimm die Burg des Pyrrhus ein."),
    (
        "King Pyrrhus of Epirus has crossed to Italy to fight for the Greek cities of the south, with elephants and an army trained in every war since Alexander. His soldiers are few, but each is worth three of yours.\n\nHere you learn soldier quality: gold mines and a mint, coins for the military buildings so their soldiers are promoted, and attacks with your strongest soldiers. The Field of Mars lies near Rome: while six or more soldiers wait in your warehouses, they drill there, Mars's favour grows, and he promotes some of them.",
        "König Pyrrhus von Epirus ist nach Italien übergesetzt, um für die griechischen Städte des Südens zu kämpfen, mit Elefanten und einem Heer, das in jedem Krieg seit Alexander geübt hat. Er hat wenige Soldaten, aber jeder wiegt drei der deinen auf.\n\nHier lernst du die Qualität der Soldaten: Goldbergwerke und eine Münzprägerei, Münzen für die Militärgebäude, damit ihre Soldaten befördert werden, und Angriffe mit deinen stärksten Soldaten. Das Marsfeld liegt nahe bei Rom: Solange sechs oder mehr Soldaten in deinen Lagern warten, üben sie dort, die Gunst des Mars wächst, und er befördert einige von ihnen.",
    ),
    (
        "280 BC. The Greek city of Tarentum has called a king from across the sea: Pyrrhus of Epirus, a cousin of Alexander, with elephants no Roman has ever seen. At Heraclea and Asculum his phalanx and his elephants break Rome's legions. Now the command is yours, Manius Curius Dentatus. Picus walks with you on the Field of Mars by the river: \"We cannot buy soldiers like his. We can make them.\"",
        "280 v. Chr. Die griechische Stadt Tarent hat einen König von jenseits des Meeres gerufen: Pyrrhus von Epirus, einen Vetter Alexanders, mit Elefanten, die noch kein Römer gesehen hat. Bei Herakleia und Asculum zerschlagen seine Phalanx und seine Elefanten Roms Legionen. Jetzt hast du den Befehl, Manius Curius Dentatus. Picus geht mit dir über das Marsfeld am Fluss: „Soldaten wie seine können wir nicht kaufen. Wir können sie machen.“",
    ),
    (
        "At Beneventum, in 275 BC, you beat Pyrrhus at last, and he leaves Italy, his army spent. Another such victory, he is said to have told his friends, and I am lost. Rome has learned that it can lose battles and still win the war, as long as it keeps making soldiers. Picus writes: \"Mars does not love the bravest army. He loves the one that drills.\"",
        "Bei Beneventum, 275 v. Chr., schlägst du Pyrrhus endlich, und er verlässt Italien, sein Heer ist aufgerieben. Noch so ein Sieg, soll er zu seinen Freunden gesagt haben, und ich bin verloren. Rom hat gelernt, dass es Schlachten verlieren und doch den Krieg gewinnen kann, solange es weiter Soldaten macht. Picus schreibt: „Mars liebt nicht das tapferste Heer. Er liebt das, das übt.“",
    ),
    (
        "Elephants at the edge of your land. Your carriers run from them, and so do some of your soldiers. Picus: \"Beasts are only beasts. Soldiers are made.\"",
        "Elefanten am Rand deines Landes. Deine Träger fliehen vor ihnen, und manche deiner Soldaten auch. Picus: „Tiere sind nur Tiere. Soldaten werden gemacht.“",
    ),
    (
        "Picus: \"Keep soldiers in the castle, and let them drill on Mars's field. Coins for the posts at the front: a soldier who has been promoted does not run.\"",
        "Picus: „Halte Soldaten in der Burg und lass sie auf dem Marsfeld üben. Münzen für die Posten an der Front: Ein beförderter Soldat läuft nicht davon.“",
    ),
    (
        "You have taken a post from Pyrrhus. His envoy Cineas comes with gifts and talk of peace. The senate sends him home: no peace while a foreign king stands in Italy.",
        "Du hast Pyrrhus einen Posten abgenommen. Sein Gesandter Kineas kommt mit Geschenken und Reden vom Frieden. Der Senat schickt ihn heim: kein Frieden, solange ein fremder König in Italien steht.",
    ),
    (
        "A post falls to Pyrrhus, but his own losses are heavy. Picus: \"He wins every battle, and his army grows smaller with each.\"",
        "Ein Posten fällt an Pyrrhus, doch seine eigenen Verluste sind schwer. Picus: „Er gewinnt jede Schlacht, und mit jeder wird sein Heer kleiner.“",
    ),
    (
        "The soldiers drilling on the Field of Mars see a woodpecker, Mars's bird, land on their standard. Some of them are promoted that day: privates first class now.",
        "Die Soldaten, die auf dem Marsfeld üben, sehen einen Specht, den Vogel des Mars, auf ihrer Standarte landen. Einige von ihnen werden an diesem Tag befördert: Jetzt sind sie Obergefreite.",
    ),
    (
        "Mars is pleased with the drill. More soldiers rise a rank, and the whole army hears of it.",
        "Mars ist mit den Übungen zufrieden. Weitere Soldaten steigen einen Rang auf, und das ganze Heer hört davon.",
    ),
    ("Curius", "Curius"),
    ("Pyrrhus", "Pyrrhus"),
    ("Mylae", "Mylae"),
    (
        "Chapter VII: harbours, ships and attacks across the sea.",
        "Kapitel VII: Häfen, Schiffe und Angriffe über das Meer.",
    ),
    (
        "Goal: take a Carthaginian military building across the sea.",
        "Ziel: Nimm ein Militärgebäude Karthagos jenseits des Meeres ein.",
    ),
    (
        "Carthage rules the sea, and its fleet has never lost. Rome has no ships at all. You, Gaius Duilius, will change that: build a harbour on a free coast, switch a shipyard to sea ships, and carry your soldiers across.\n\nHere you learn seafaring: harbour spots (the anchors), shipyards, expeditions that found a colony on another coast, and attacks from the sea. Neptune's shrine stands on a coast between you and Carthage: offer fish there, and he may send you sailors.",
        "Karthago beherrscht das Meer, und seine Flotte hat nie verloren. Rom hat überhaupt keine Schiffe. Du, Gaius Duilius, wirst das ändern: Baue einen Hafen an einer freien Küste, stelle eine Werft auf Seeschiffe um und bring deine Soldaten hinüber.\n\nHier lernst du die Seefahrt: Hafenplätze (die Anker), Werften, Expeditionen, die an einer anderen Küste eine Kolonie gründen, und Angriffe vom Meer aus. An einer Küste zwischen dir und Karthago steht ein Heiligtum Neptuns: Opfere dort Fisch, und er schickt dir vielleicht Seeleute.",
    ),
    (
        "260 BC. The war with Carthage has come to Sicily, and Carthage's ships sail where they like. Rome has never built a warship. Then a Carthaginian ship runs aground on the coast, and Roman carpenters take it apart plank by plank and build a hundred like it in sixty days. Picus, who has never been to sea, looks at the waves: \"Neptune decides. We had better ask him.\"",
        "260 v. Chr. Der Krieg mit Karthago hat Sizilien erreicht, und Karthagos Schiffe fahren, wohin sie wollen. Rom hat noch nie ein Kriegsschiff gebaut. Da läuft ein karthagisches Schiff an der Küste auf Grund, und römische Zimmerleute nehmen es Planke für Planke auseinander und bauen in sechzig Tagen hundert wie dieses. Picus, der noch nie auf See war, betrachtet die Wellen: „Neptun entscheidet. Wir sollten ihn besser fragen.“",
    ),
    (
        "At Mylae Duilius' boarding bridges fall onto the Carthaginian decks, and Roman soldiers fight there as on land. Carthage loses its first battle at sea. Rome gives Duilius a column decorated with the bronze rams of the captured ships, and a flute player to walk before him at night for the rest of his life. Picus writes: \"Neptune let us cross. I still do not like the sea.\"",
        "Bei Mylae fallen die Enterbrücken des Duilius auf die karthagischen Decks, und römische Soldaten kämpfen dort wie an Land. Karthago verliert seine erste Seeschlacht. Rom schenkt Duilius eine Säule, geschmückt mit den bronzenen Rammspornen der erbeuteten Schiffe, und einen Flötenspieler, der ihm für den Rest seines Lebens nachts vorangeht. Picus schreibt: „Neptun hat uns übersetzen lassen. Ich mag das Meer trotzdem nicht.“",
    ),
    (
        "Picus: \"A harbour first, on a coast with an anchor sign. Then a shipyard, and switch it to ships. Carthage will not come to us.\"",
        "Picus: „Zuerst einen Hafen, an einer Küste mit einem Ankerzeichen. Dann eine Werft, und stelle sie auf Schiffe um. Karthago wird nicht zu uns kommen.“",
    ),
    (
        "Your first ship is on the water. Its rowers are farmers who learned to row on benches on the beach. You have a boarding bridge fixed to every bow: \"We fight at sea as we fight on land.\"",
        "Dein erstes Schiff ist auf dem Wasser. Seine Ruderer sind Bauern, die auf Bänken am Strand rudern gelernt haben. Du lässt an jedem Bug eine Enterbrücke befestigen: „Wir kämpfen auf See wie an Land.“",
    ),
    (
        "Carthaginian sails on the horizon. Their crews laugh at your heavy ships. Picus: \"Let them laugh until the bridges fall.\"",
        "Karthagische Segel am Horizont. Ihre Mannschaften lachen über deine schweren Schiffe. Picus: „Lass sie lachen, bis die Brücken fallen.“",
    ),
    (
        "You have taken a post on Carthage's shore. Picus: \"The sea is a road too, it seems.\"",
        "Du hast einen Posten an Karthagos Küste eingenommen. Picus: „Das Meer ist also auch eine Straße.“",
    ),
    (
        "Fishermen from every coast have seen the smoke of your offerings at Neptune's shrine. Five of them come to row and to fight for Rome: privates in your castle now.",
        "Fischer von allen Küsten haben den Rauch deiner Opfer an Neptuns Heiligtum gesehen. Fünf von ihnen kommen, um für Rom zu rudern und zu kämpfen: Jetzt sind sie Gefreite in deiner Burg.",
    ),
    ("Gaius Duilius", "Gaius Duilius"),
    ("Carthage", "Karthago"),
    ("Hannibal at the Gates", "Hannibal vor den Toren"),
    (
        "Chapter VIII: hold out against Hannibal, and let time fight for you.",
        "Kapitel VIII: Halte gegen Hannibal aus, und lass die Zeit für dich kämpfen.",
    ),
    (
        "Goal: hold out 50 minutes, until Scipio sails for Africa.",
        "Ziel: Halte 50 Minuten aus, bis Scipio nach Afrika segelt.",
    ),
    (
        "Hannibal has crossed the Alps with his army and his elephants and beaten every army Rome sent against him. You are Quintus Fabius Maximus, dictator: do not meet him in the open. Hold the strong places, let him come to you, and let Italy itself wear him down.\n\nHold out fifty minutes, until Scipio is ready to sail for Africa. Cybele's temple stands near Rome: the Sibylline books say the invader leaves Italy only when the Great Mother comes to Rome. Hold it, and Rome's people rally to you.",
        "Hannibal hat mit seinem Heer und seinen Elefanten die Alpen überquert und jedes Heer geschlagen, das Rom gegen ihn schickte. Du bist Quintus Fabius Maximus, Diktator: Stelle dich ihm nicht im offenen Feld. Halte die festen Plätze, lass ihn zu dir kommen, und lass Italien selbst ihn zermürben.\n\nHalte fünfzig Minuten aus, bis Scipio bereit ist, nach Afrika zu segeln. Nahe bei Rom steht der Tempel der Kybele: Die Sibyllinischen Bücher sagen, der Eindringling verlässt Italien erst, wenn die Große Mutter nach Rom kommt. Halte ihn, und Roms Volk sammelt sich bei dir.",
    ),
    (
        "217 BC. Hannibal of Carthage has done what no one thought possible: he has crossed the Alps with an army and elephants, and beaten Rome at the Trebia and at Lake Trasimene. The senate names you dictator, Quintus Fabius Maximus. Picus, who has heard the Sibylline books read, tells you: \"Do not meet him in the open. Let him come to you.\"",
        "217 v. Chr. Hannibal von Karthago hat getan, was niemand für möglich hielt: Er hat mit einem Heer und Elefanten die Alpen überquert und Rom an der Trebia und am Trasimenischen See geschlagen. Der Senat ernennt dich zum Diktator, Quintus Fabius Maximus. Picus, der die Sibyllinischen Bücher hat vorlesen hören, sagt dir: „Stelle dich ihm nicht im offenen Feld. Lass ihn zu dir kommen.“",
    ),
    (
        "Scipio lands in Africa, and Carthage calls Hannibal home. After fifteen years in Italy he leaves without ever having taken Rome, and at Zama Scipio beats him. The name Cunctator becomes a title of honour. Picus writes: \"Fabius won no battle. He won the war.\"",
        "Scipio landet in Afrika, und Karthago ruft Hannibal heim. Nach fünfzehn Jahren in Italien zieht er ab, ohne Rom je eingenommen zu haben, und bei Zama schlägt Scipio ihn. Der Name Cunctator wird zum Ehrentitel. Picus schreibt: „Fabius hat keine Schlacht gewonnen. Er hat den Krieg gewonnen.“",
    ),
    (
        "Hannibal's army in the north: veterans of Spain and the Alps, who know their trade. Picus: \"Every soldier he loses is one he cannot replace. Ours we can.\"",
        "Hannibals Heer im Norden: Veteranen aus Spanien und den Alpen, die ihr Handwerk verstehen. Picus: „Jeden Soldaten, den er verliert, kann er nicht ersetzen. Unsere können wir ersetzen.“",
    ),
    (
        "The people of Rome call you Cunctator, the Delayer, and mean it as an insult. Picus: \"One day they will mean it as a title.\"",
        "Das Volk von Rom nennt dich Cunctator, den Zauderer, und meint es als Beleidigung. Picus: „Eines Tages werden sie es als Ehrentitel meinen.“",
    ),
    (
        "News from Apulia: the consul Varro has given battle at Cannae without you, and Hannibal has surrounded and destroyed the largest army Rome ever raised. Rome does not ask for peace.",
        "Nachricht aus Apulien: Der Konsul Varro hat ohne dich bei Cannae die Schlacht gesucht, und Hannibal hat das größte Heer, das Rom je aufgestellt hat, eingeschlossen und vernichtet. Rom bittet nicht um Frieden.",
    ),
    (
        "Scipio is gathering ships in Sicily. A few more minutes, and Rome strikes at Africa.",
        "Scipio sammelt Schiffe auf Sizilien. Noch ein paar Minuten, und Rom schlägt in Afrika zu.",
    ),
    (
        "Rome arms eight thousand slaves, promising them their freedom. Eight new privates come to your castle.",
        "Rom bewaffnet achttausend Sklaven und verspricht ihnen die Freiheit. Acht neue Gefreite kommen in deine Burg.",
    ),
    (
        "The Great Mother's stone arrives at Ostia, a black stone from Pessinus, and is carried to her temple while the people line the road. Five men who watched it pass join your army.",
        "Der Stein der Großen Mutter kommt in Ostia an, ein schwarzer Stein aus Pessinus, und wird zu ihrem Tempel getragen, während das Volk die Straße säumt. Fünf Männer, die ihn vorbeiziehen sahen, schließen sich deinem Heer an.",
    ),
    ("Fabius", "Fabius"),
    ("Hannibal", "Hannibal"),
    ("Carthago delenda est", "Carthago delenda est"),
    (
        "Chapter IX: everything you have learned, in a long war on land.",
        "Kapitel IX: alles, was du gelernt hast, in einem langen Krieg an Land.",
    ),
    ("Goal: take the castle of Carthage.", "Ziel: Nimm die Burg von Karthago ein."),
    (
        "Fifty years after Zama, Carthage is rich again, and in the senate old Cato ends every speech with the same words: Carthage must be destroyed. The senate sends you, Scipio Aemilianus. King Massinissa of Numidia fights on your side.\n\nThis chapter asks for everything at once: an economy that feeds an army, soldiers of high rank, and a long siege. The Field of Mars near Rome drills your waiting soldiers. Carthage's temple stands near its walls: where your land closes around it, its god no longer guards the city.",
        "Fünfzig Jahre nach Zama ist Karthago wieder reich, und im Senat beendet der alte Cato jede Rede mit denselben Worten: Karthago muss zerstört werden. Der Senat schickt dich, Scipio Aemilianus. König Massinissa von Numidien kämpft auf deiner Seite.\n\nDieses Kapitel verlangt alles auf einmal: eine Wirtschaft, die ein Heer ernährt, Soldaten von hohem Rang und eine lange Belagerung. Das Marsfeld nahe bei Rom lässt deine wartenden Soldaten üben. Karthagos Tempel steht nahe seinen Mauern: Wo dein Land ihn umschließt, hütet sein Gott die Stadt nicht mehr.",
    ),
    (
        "149 BC. Carthage has paid its debts to Rome and grown rich on trade. In the senate Cato holds up a fresh fig from Carthage: \"Picked three days ago, that close is Carthage.\" Every speech he ends the same way: Carthage must be destroyed. Picus is old now, and he doubts: \"Do the gods want this, or do we only say they do?\"",
        "149 v. Chr. Karthago hat seine Schulden bei Rom bezahlt und ist durch den Handel reich geworden. Im Senat hält Cato eine frische Feige aus Karthago hoch: „Vor drei Tagen gepflückt, so nah ist Karthago.“ Jede Rede beendet er gleich: Karthago muss zerstört werden. Picus ist jetzt alt, und er zweifelt: „Wollen die Götter das, oder sagen wir nur, dass sie es wollen?“",
    ),
    (
        "After three years of siege Carthage falls and burns for many days. Scipio Aemilianus watches the fire and weeps; he quotes Homer on the fall of Troy and says he fears the same day will come for Rome. Picus closes the augurs' book for this war: \"We asked the gods every morning. I am no longer sure they answered.\"",
        "Nach drei Jahren Belagerung fällt Karthago und brennt viele Tage lang. Scipio Aemilianus sieht dem Feuer zu und weint; er zitiert Homer über den Fall Trojas und sagt, er fürchte, derselbe Tag werde einst für Rom kommen. Picus schließt das Buch der Auguren für diesen Krieg: „Wir haben die Götter jeden Morgen gefragt. Ich bin nicht mehr sicher, dass sie geantwortet haben.“",
    ),
    (
        "Numidian horsemen ride beside your columns. King Massinissa is almost ninety years old and still in the saddle.",
        "Numidische Reiter reiten neben deinen Kolonnen. König Massinissa ist fast neunzig Jahre alt und sitzt noch im Sattel.",
    ),
    (
        "The walls of Carthage, the greatest you have seen, and behind them a city that will not surrender.",
        "Die Mauern Karthagos, die größten, die du je gesehen hast, und dahinter eine Stadt, die sich nicht ergeben wird.",
    ),
    (
        "A Carthaginian post falls. The city behind it does not ask for terms.",
        "Ein karthagischer Posten fällt. Die Stadt dahinter bittet nicht um Bedingungen.",
    ),
    ("Scipio Aemilianus", "Scipio Aemilianus"),
    ("Numidia", "Numidien"),
    ("The Gates of Janus", "Die Tore des Janus"),
    (
        "Chapter X: the whole Mediterranean, and at the end, peace.",
        "Kapitel X: das ganze Mittelmeer, und am Ende Frieden.",
    ),
    (
        "Goal: take Mark Antony's castle. Then the gates of Janus close.",
        "Ziel: Nimm die Burg des Marcus Antonius ein. Dann schließen sich die Tore des Janus.",
    ),
    (
        "After a century of civil wars two men are left: you, Octavian, in Rome, and Mark Antony, Cleopatra's ally, his legions gathered in the north and his fleet in Greece. The Cantabri in Spain have never been tamed, and beyond the Rhine and the Channel the Germans and the Britons watch.\n\nEverything you have learned, on the whole Mediterranean. When Antony falls, the doors of the temple of Janus can close: Rome at peace everywhere, for the first time in two hundred years. Janus's gates stand near Rome: hold them, and veterans of every war come to serve you.",
        "Nach einem Jahrhundert der Bürgerkriege sind zwei Männer übrig: du, Octavian, in Rom, und Marcus Antonius, der Verbündete Kleopatras, seine Legionen im Norden versammelt und seine Flotte vor Griechenland. Die Kantabrer in Spanien wurden nie unterworfen, und jenseits des Rheins und des Kanals schauen Germanen und Britannier zu.\n\nAlles, was du gelernt hast, auf dem ganzen Mittelmeer. Wenn Antonius fällt, können sich die Türen des Janustempels schließen: Rom überall im Frieden, zum ersten Mal seit zweihundert Jahren. Nahe bei Rom stehen die Tore des Janus: Halte sie, und Veteranen aller Kriege kommen, um dir zu dienen.",
    ),
    (
        "31 BC. Julius Caesar is dead, and the Republic with him. Octavian, his adopted son, holds Rome; Mark Antony, with Cleopatra of Egypt as his ally, gathers his legions in the north and his fleet in Greece. The doors of the temple of Janus stand open, as they have stood for two hundred years: Rome is at war. The last of the house of Picus, a young augur, stands beside you: \"The first of my house saw the vultures over Rome's hills. I would like to see the gates close.\"",
        "31 v. Chr. Julius Caesar ist tot, und mit ihm die Republik. Octavian, sein Adoptivsohn, hält Rom; Marcus Antonius, mit Kleopatra von Ägypten verbündet, sammelt seine Legionen im Norden und seine Flotte vor Griechenland. Die Türen des Janustempels stehen offen, wie seit zweihundert Jahren: Rom ist im Krieg. Der letzte aus dem Haus des Picus, ein junger Augur, steht neben dir: „Der Erste meines Hauses sah die Geier über Roms Hügeln. Ich würde gern sehen, wie sich die Tore schließen.“",
    ),
    (
        "At Actium Agrippa's ships break Antony's fleet; Antony and Cleopatra die in Egypt. In 29 BC the doors of the temple of Janus are closed: Rome is at peace on every coast. Octavian will soon be called Augustus. The last Picus writes the last line in the augurs' book: \"The gods guided Rome, and Rome used the gods. Both were true, and the gates are closed.\"",
        "Bei Actium zerschlagen Agrippas Schiffe die Flotte des Antonius; Antonius und Kleopatra sterben in Ägypten. 29 v. Chr. werden die Türen des Janustempels geschlossen: Rom ist an allen Küsten im Frieden. Octavian wird bald Augustus genannt werden. Der letzte Picus schreibt die letzte Zeile in das Buch der Auguren: „Die Götter haben Rom geführt, und Rom hat die Götter benutzt. Beides war wahr, und die Tore sind geschlossen.“",
    ),
    (
        "Antony's legions in the north, and Cleopatra's gold behind them. Your friend Agrippa: \"Give me soldiers, and I will give you Antony.\"",
        "Die Legionen des Antonius im Norden, und Kleopatras Gold hinter ihnen. Dein Freund Agrippa: „Gib mir Soldaten, und ich gebe dir Antonius.“",
    ),
    (
        "The Cantabri in their mountains, who have fought every Roman army for a hundred years. Picus: \"Antony first. Spain will wait.\"",
        "Die Kantabrer in ihren Bergen, die seit hundert Jahren gegen jedes römische Heer kämpfen. Picus: „Zuerst Antonius. Spanien kann warten.“",
    ),
    (
        "You have taken one of your rivals' posts. Picus: \"Every gate that falls is a step towards the one that closes.\"",
        "Du hast einen Posten deiner Gegner eingenommen. Picus: „Jedes Tor, das fällt, ist ein Schritt zu dem, das sich schließt.“",
    ),
    (
        "Your land closes around the old gates of Janus. The young Picus touches the bronze: \"When Rome is at peace everywhere, we close them.\" Veterans of every war come to see them, and five stay to serve you.",
        "Dein Land schließt sich um die alten Tore des Janus. Der junge Picus berührt die Bronze: „Wenn Rom überall im Frieden ist, schließen wir sie.“ Veteranen aller Kriege kommen, um sie zu sehen, und fünf bleiben, um dir zu dienen.",
    ),
    ("Octavian", "Octavian"),
    ("The Cantabri", "Die Kantabrer"),
    ("Mark Antony", "Marcus Antonius"),
    ("Parthia", "Parthien"),
    ("Cleopatra", "Kleopatra"),
    ("Goal: reach {1} allied towns ({2} so far)", "Ziel: Erreiche {1} verbündete Städte ({2} bisher)"),
    ("Allied with you:", "Mit dir verbündet:"),
    ("The gates of Janus, a holy place", "Die Tore des Janus, ein heiliger Ort"),
    ("The Field of Mars, a holy place", "Das Marsfeld, ein heiliger Ort"),
    ("Neptune's shrine, a holy place", "Neptuns Heiligtum, ein heiliger Ort"),
    ("Cybele's temple, a holy place", "Der Tempel der Kybele, ein heiliger Ort"),
    ("Carthage's temple, a holy place", "Karthagos Tempel, ein heiliger Ort"),
    (
        "Yours: Janus, the god of gates and roads, watches over your way.",
        "Deiner: Janus, der Gott der Tore und Wege, wacht über deinen Weg.",
    ),
    (
        "Yours: while six or more soldiers wait in your warehouses, they drill here, and Mars's favour grows.",
        "Deiner: Solange sechs oder mehr Soldaten in deinen Lagern warten, üben sie hier, und die Gunst des Mars wächst.",
    ),
    (
        "Yours: click it to offer fish, and Neptune may send you sailors.",
        "Deiner: Klicke ihn an, um Fisch zu opfern, und Neptun schickt dir vielleicht Seeleute.",
    ),
    ("Yours: the Great Mother watches over Rome.", "Deiner: Die Große Mutter wacht über Rom."),
    (
        "Yours: Carthage's own god no longer guards its walls.",
        "Deiner: Karthagos eigener Gott hütet seine Mauern nicht mehr.",
    ),
    // the map's hover (hover.rs)
    ("Jupiter's augur platform, a holy place", "Jupiters Auguren-Plattform, ein heiliger Ort"),
    ("A holy place", "Ein heiliger Ort"),
    (
        "Yours: Jupiter's birds show you the land far around the hill.",
        "Deiner: Jupiters Vögel zeigen dir das Land weit um den Hügel.",
    ),
    ("Yours: its god is with you.", "Deiner: Sein Gott ist mit dir."),
    ("Held by {1}: its god is with them.", "Gehalten von {1}: Sein Gott steht auf ihrer Seite."),
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
        "The hills are yours, and the sky gave its sign: twelve vultures for Romulus, six for Remus. The city takes your name: Roma. The old tellings disagree about what became of Remus; his people stay and build on the Aventine as Romans. Picus marks the day for the calendar, the twenty-first of April. Across the valley, the Sabines watch the new walls.",
        "Die Hügel gehören dir, und der Himmel gab sein Zeichen: zwölf Geier für Romulus, sechs für Remus. Die Stadt trägt deinen Namen: Roma. Was aus Remus wurde, erzählen die alten Geschichten verschieden; seine Leute bleiben und bauen als Römer auf dem Aventin. Picus trägt den Tag in den Kalender ein, den einundzwanzigsten April. Jenseits des Tals betrachten die Sabiner die neuen Mauern.",
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
    ("Calm map (no swaying, birds or sparkles)", "Ruhige Karte (kein Wiegen, keine Vögel, kein Glitzern)"),
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
        "Title painting, the Roman buildings, the holy places and the ships: painted with an AI image tool from our own prompts; all other art: our own code.",
        "Titelbild, die römischen Gebäude, die heiligen Stätten und die Schiffe: mit einem KI-Bildwerkzeug aus eigenen Textvorgaben gemalt; alle übrige Grafik: unser eigener Code.",
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
