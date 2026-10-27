#!/usr/bin/env python3
"""The manual's building catalog, from the game's own data: costs, sizes, workers and wares from
data/rules/classic.toml, German names from the game's translation table (crates/rr-app/src/i18n.rs),
pictures from assets/art/buildings. The descriptions are the manual's own.

    manual/gen.py de  ->  manual/build/de/buildings.tex
"""
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LANG = sys.argv[1] if len(sys.argv) > 1 else "de"
OUT = ROOT / "manual" / "build" / LANG
RULES = tomllib.loads((ROOT / "data/rules/classic.toml").read_text())["buildings"]

# the game's German table: ("English", "Deutsch") pairs
I18N = (ROOT / "crates/rr-app/src/i18n.rs").read_text()
PAIR = re.compile(r'\(\s*"((?:[^"\\]|\\.)*)",\s*"((?:[^"\\]|\\.)*)",?\s*\)')
DE = {a: b for a, b in PAIR.findall(I18N)}


def tr(en: str) -> str:
    return DE.get(en, en) if LANG == "de" else en


def split_camel(n: str) -> str:
    return re.sub(r"(?<!^)([A-Z])", lambda m: " " + m.group(1).lower(), n)


BUILDING_KEY = {"Headquarters": "Castle", "LookoutTower": "Lookout tower", "GraniteMine": "Granite mine",
                "CoalMine": "Coal mine", "IronMine": "Iron mine", "GoldMine": "Gold mine",
                "IronSmelter": "Iron smelter", "PigFarm": "Pig farm", "DonkeyBreeder": "Donkey breeder"}


def building(k: str) -> str:
    return tr(BUILDING_KEY.get(k, k))


def ware(w: str) -> str:
    return tr("Rolling pin" if w == "Rollingpin" else split_camel(w))


def job(j: str) -> str:
    en = split_camel(j)
    return DE.get(f"{en} (job)", tr(en)) if LANG == "de" else en


SIZE = {"Hut": "klein", "House": "mittel", "Castle": "groß", "Mine": "Bergwerk", "Harbor": "Hafenplatz"}

# (chapter section, buildings): the order the catalog tells them in
GROUPS = [
    ("Militär", ["Headquarters", "Barracks", "Guardhouse", "Watchtower", "Fortress", "LookoutTower", "Catapult"]),
    ("Rohstoffe", ["Woodcutter", "Forester", "Quarry", "Fishery", "Hunter", "Well"]),
    ("Holz, Nahrung und Tiere", ["Sawmill", "Farm", "Mill", "Bakery", "PigFarm", "Slaughterhouse", "Brewery",
                                 "DonkeyBreeder"]),
    ("Bergbau und Metall", ["GraniteMine", "CoalMine", "IronMine", "GoldMine", "IronSmelter", "Mint", "Armory",
                            "Metalworks"]),
    ("Lager und Seefahrt", ["Storehouse", "Harbor", "Shipyard"]),
]

TEXT = {
    "Headquarters": ("Das Herz deines Reiches. Hier wohnen zu Beginn alle Siedler, hier lagern Werkzeuge, "
                     "Baustoffe und Waffen, und von hier aus beginnt jede Straße. Die Burg hält Land um sich "
                     "herum und wird nie abgerissen. Fällt sie, ist das Spiel meist verloren.",
                     "Halte immer ein paar Soldaten in Reserve in der Burg."),
    "Barracks": ("Das kleinste Militärgebäude: zwei Soldaten, schnell gebaut, nur aus Brettern. Gut, um Land "
                 "im Hinterland zu sichern oder schnell eine Lücke zu schließen.",
                 "An der Front ist eine Baracke leichte Beute."),
    "Guardhouse": ("Das Arbeitstier der Ausbreitung: drei Soldaten, ein großer Landkreis und geringe Kosten. "
                   "Mit Wachstuben schiebst du deine Grenze Schritt für Schritt nach außen.",
                   "Militärgebäude brauchen untereinander und zur Burg vier Punkte Abstand."),
    "Watchtower": ("Ein mittleres Militärgebäude mit sechs Soldaten und weiter Sicht. Es trägt die Grenze ein "
                   "gutes Stück weiter als die Wachstube und hält einem Angriff länger stand.", ""),
    "Fortress": ("Die stärkste Stellung: neun Soldaten und der größte Landkreis. Teuer, aber an einer heißen "
                 "Grenze der beste Schutz und der beste Ausgangspunkt für eigene Angriffe.",
                 "Mit Münzen beförderte Soldaten machen eine Festung fast uneinnehmbar."),
    "LookoutTower": ("Kein Militärgebäude: Ein Späher hält von hier aus weit Ausschau. Der Turm nimmt kein Land "
                     "ein und hat keine Soldaten, aber er zeigt dir, was hinter dem Nebel liegt.",
                     "Baue ihn dort, wo du einen Gegner vermutest, ihn aber nicht siehst."),
    "Catapult": ("Wirft Steine auf feindliche Militärgebäude in Reichweite, die du sehen kannst. Sieben von "
                 "zehn Würfen treffen, und jeder Treffer tötet den schwächsten Soldaten darin. Ist der letzte "
                 "gefallen, brennt das Gebäude.",
                 "Ein Wurf auf einen Gegner in der Waffenruhe beendet sie, wie ein Angriff."),
    "Woodcutter": ("Der Holzfäller fällt ausgewachsene Bäume in der Nähe und bringt die Stämme an die Fahne. "
                   "Holz ist der erste Rohstoff jedes Reiches.",
                   "Stelle ihm einen Förster zur Seite, sonst ist der Wald bald leer."),
    "Forester": ("Der Förster pflanzt rund um seine Hütte junge Bäume. Nach einer Weile wachsen sie heran und "
                 "der Holzfäller kann sie fällen.",
                 "Nicht neben Bauland setzen: Er pflanzt auch dort, wo du noch bauen willst."),
    "Quarry": ("Der Steinmetz schlägt Steine aus den Felsen in der Nähe. Steine braucht fast jedes größere "
               "Gebäude, dazu das Katapult.",
               "Sind die Felsen aufgebraucht, reiße den Steinbruch ab oder baue ein Granitbergwerk."),
    "Fishery": ("Der Fischer angelt in Seen, Flüssen und an der Küste in Reichweite. Fisch ist eine der drei "
                "Nahrungen für die Bergleute.",
                "Er braucht eine Angel; die macht der Schlosser."),
    "Hunter": ("Der Jäger pirscht Rehen und anderem Wild nach und bringt das Fleisch heim.",
               "Wild wird weniger: Für viele Bergwerke reicht der Jäger allein nicht."),
    "Well": ("Am Brunnen schöpft ein Träger Wasser. Bäckerei, Brauerei, Schweinezucht und Eselzucht brauchen es.",
             ""),
    "Sawmill": ("Der Schreiner sägt aus Holz Bretter. Bretter sind der wichtigste Baustoff: Ohne Sägewerk "
                "kommt dein Reich nicht weit.",
                "Zwei Holzfäller halten ein Sägewerk gut beschäftigt."),
    "Farm": ("Der Bauer sät und erntet Getreide auf dem freien Land um seinen Hof. Getreide geht an die Mühle, "
             "die Brauerei, die Schweinezucht und die Eselzucht.",
             "Lass rund um den Hof Platz: Auf bebautem Land wächst nichts."),
    "Mill": ("Der Müller mahlt Getreide zu Mehl, das die Bäckerei braucht.", ""),
    "Bakery": ("Der Bäcker backt aus Mehl und Wasser Brot, die beste Nahrung für die Bergwerke.", ""),
    "PigFarm": ("Der Schweinezüchter mästet mit Getreide und Wasser Schweine. Die Schweine verarbeitet der "
                "Metzger in der Schlachterei zu Fleisch.", ""),
    "Slaughterhouse": ("Der Metzger macht aus Schweinen Fleisch, eine Nahrung für die Bergleute.", ""),
    "Brewery": ("Der Brauer braut aus Getreide und Wasser Bier. Jeder neue Soldat braucht ein Bier, dazu ein "
                "Schwert und einen Schild.", ""),
    "DonkeyBreeder": ("Der Eselzüchter zieht mit Getreide und Wasser Packesel auf. Auf viel befahrenen Straßen "
                      "hilft ein Esel dem Träger und trägt doppelt so viel weg.",
                      "Esel helfen erst, wenn eine Straße wirklich voll ist."),
    "GraniteMine": ("Bergleute brechen im Berg Granit. Das Bergwerk liefert Steine, wenn die Felsen auf dem "
                    "Land aufgebraucht sind.",
                    "Schicke vorher einen Geologen: Er zeigt mit Schildern, was im Berg liegt."),
    "CoalMine": ("Kohle braucht die Eisenschmelze, die Münzprägerei und die Waffenschmiede. Ohne Kohle keine "
                 "Waffen und kein Gold.",
                 "Jede Arbeit kostet die Bergleute eine Nahrung: Fisch, Fleisch oder Brot."),
    "IronMine": ("Das Eisenbergwerk fördert Eisenerz, das die Eisenschmelze zu Eisen schmilzt.", ""),
    "GoldMine": ("Das Goldbergwerk fördert Gold. Die Münzprägerei macht daraus Münzen, mit denen deine "
                 "Soldaten befördert werden.",
                 "Gold liegt meist tief in den Bergen; in der Kampagne zeigen es die Briefings."),
    "IronSmelter": ("Der Schmelzer schmilzt Eisenerz mit Kohle zu Eisen, für den Schlosser und die "
                    "Waffenschmiede.", ""),
    "Mint": ("Der Münzpräger prägt aus Gold und Kohle Münzen. Eine Münze in einem Militärgebäude befördert "
             "dort einen Soldaten jedes Ranges um einen Rang.",
             "In Mare Nostrum ist das Prägen das Ziel der Mission."),
    "Armory": ("Der Schmied schmiedet aus Eisen und Kohle abwechselnd Schwerter und Schilde, die Waffen deiner "
               "neuen Soldaten.", ""),
    "Metalworks": ("Der Schlosser macht aus Eisen und Brettern Werkzeuge: Axt, Säge, Spitzhacke, Hammer und "
                   "die anderen. Welche, stellst du im Fenster Werkzeuge ein.",
                   "Fehlt einem Gebäude der Arbeiter, fehlt meist sein Werkzeug."),
    "Storehouse": ("Ein Lager für Waren und Siedler fern der Burg. Träger müssen dann nicht jede Ware durch "
                   "das ganze Reich tragen.",
                   "In den Lagereinstellungen legst du fest, was ein Lager aufnimmt oder abgibt."),
    "Harbor": ("Ein Lagerhaus an der Küste mit Anlegestelle. Schiffe bringen von Hafen zu Hafen Waren und "
               "Siedler und fahren Expeditionen zu fremden Küsten, wo sie eine Kolonie gründen.",
               "Ein Hafen geht nur auf einem Hafenplatz am Wasser."),
    "Shipyard": ("Der Schiffsbauer baut aus Brettern Boote für die Fischer und, wenn du umstellst, "
                 "Seeschiffe für deine Häfen.", ""),
}


def tex(s: str) -> str:
    return s.replace("&", r"\&").replace("%", r"\%").replace("#", r"\#")


def entry(k: str) -> str:
    d = RULES[k]
    rows = [("Größe", SIZE[d["size"]])]
    cost = [f"{d[w]}~{ware(n)}" for w, n in (("boards", "Boards"), ("stones", "Stones")) if d.get(w)]
    rows.append(("Baukosten", ", ".join(cost) or "—"))
    if d.get("worker"):
        rows.append(("Arbeiter", job(d["worker"])))
    if d.get("inputs") and not d.get("military"):
        sep = " oder " if d.get("use_one_each") is False else ", "
        rows.append(("Braucht", sep.join(ware(w) for w in d["inputs"])))
    out = {"Metalworks": "Werkzeuge", "Armory": f"{ware('Sword')}, {ware('Shield')}",
           "DonkeyBreeder": job("PackDonkey"), "Shipyard": f"{ware('Boat')}, Schiffe",
           "Forester": "junge Bäume"}.get(k, ware(d["output"]) if d.get("output") else "")
    if out:
        rows.append(("Liefert", out))
    m = d.get("military")
    if m and m.get("soldiers"):
        rows.append(("Soldaten", f"{m['soldiers']} (Münzen: {m['coins']})"))
    if m:
        rows.append(("Landkreis", f"{m['radius']} Punkte, Sicht {m['sight']}"))
    text, tip = TEXT[k]
    facts = "\n".join(rf"\fact{{{a}}}{{{tex(b)}}}" for a, b in rows)
    tip_tex = rf"\tip{{{tex(tip)}}}" if tip else ""
    img = ROOT / "assets/art/buildings" / f"{k.lower()}.png"
    return (rf"\building{{{tex(building(k))}}}{{{img}}}{{%" "\n" + facts + "\n}{" + tex(text) + "}{" + tip_tex
            + "}\n")


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    parts = []
    for title, keys in GROUPS:
        parts.append(rf"\section{{{title}}}")
        parts.extend(entry(k) for k in keys)
    missing = set(RULES) - {k for _, ks in GROUPS for k in ks}
    assert not missing, f"buildings left out of the manual: {missing}"
    (OUT / "buildings.tex").write_text("\n".join(parts))
    print(f"{OUT / 'buildings.tex'}: {sum(len(k) for _, k in GROUPS)} buildings")


if __name__ == "__main__":
    main()
