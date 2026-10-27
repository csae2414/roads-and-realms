# The `.rrmap` map format, version 1

The map editor writes `.rrmap` files. The game also reads the original's `.wld`/`.swd` maps, and
can write `.swd` for Return to the Roots (see the end of this page).

A version-1 file is pinned byte for byte by `crates/rr-formats/tests/data/sample-v1.rrmap` and the
test `version_1_files_keep_loading`. If the layout ever changes, the version number goes up, and
version-1 files must still load.

## Layout

```
"RRMAP\0"        6 bytes, magic
version          u16, little endian (1)
body             the map, postcard-encoded (below)
```

The body uses [postcard](https://postcard.jamesmunk.ai/wire-format) encoding:

- Integers wider than a byte (`u16`, lengths) are LEB128 varints, so 8 is one byte, `08`.
- `u8` is one plain byte.
- A string or list is a varint length followed by its items.
- An enum is a varint variant number followed by its fields.
- An `Option` is `00` (none) or `01` followed by the value.

The fields come in this order:

| Field | Type | Meaning |
|---|---|---|
| name | string | map name |
| author | string | who made it |
| grid | `u16` width, `u16` height | nodes; height is even, sides at most 1024 |
| landscape | enum: 0 greenland, 1 wasteland, 2 winter | |
| altitude | list of `u8`, one per node | 0..=60 |
| terrain_rsu | list of `u8` | terrain id of the triangle pointing down-right of each node (original ids, 0..=34) |
| terrain_usd | list of `u8` | terrain id of the triangle pointing down |
| resources | list of enum | 0 none, 1 water, 2 fish(n), 3 coal(n), 4 iron(n), 5 gold(n), 6 granite(n); n is a `u8`, 1..=7 |
| objects | list of enum | 0 none, 1 tree(species 0..=8), 2 granite(kind 0/1, size 1..=6), 3 decoration(original id) |
| animals | list of `Option<enum>` | 0 rabbit, 1 fox, 2 stag, 3 deer, 4 duck, 5 sheep, 6 polar bear |
| hq | list of `Option<(u16 x, u16 y)>` | headquarters per player slot (up to 7). Files from the editor have no gaps |
| harbors | list of `(u16 x, u16 y)` | harbor spots |

Nodes go row by row: index = y × width + x. Odd rows sit half a node to the right. Every list
of nodes has width × height entries. A file with a different count, or with a headquarters or
harbor off the map, is refused when it is read.

## Exporting for Return to the Roots

`rr-cli convert MAP.rrmap MAP.swd` writes the original's format, and so does the editor's
"Export .swd" button. What gets lost:

- Fish amounts: the original format has no fish amount, and RTTR always uses 4.
- Polar bears: the original has none; they become stags.
- Map names and authors longer than the original's fixed fields (19 and 19 characters).

`rr-cli convert MAP.swd MAP.rrmap` goes the other way.

## A mission for a map

A map can come with its own mission, written in the editor under "Mission" or by hand. It is a
JSON file next to the map, with the same name and the ending `.mission.json`:

```json
{
  "title": "Two valleys",
  "briefing": "Hold the pass before the snow comes.",
  "goal": "Goal: defeat the north.",
  "truce_minutes": 20,
  "names": ["You", "Hrothgar of the North"],
  "objective": {"defeat_rival": 1},
  "human": 0
}
```

Every field may be left out. Before the game starts, the title, briefing and goal are shown, from
the menu and from the command line (`--map FILE`). For `truce_minutes`, the computer players leave
the human player alone; if the human attacks one of them first, the truce with that player and its
allies ends. `names` names the players in slot order. The menu's free-game settings (opponents'
level, start goods, the Players table, peace time, time limit) apply to a map with a mission too.

`objective` is the goal the game checks (the `goal` text only describes it): `"defeat_all"`,
`{"defeat_rival": p}` (take the castle of the player in slot `p`, counted from 0), `{"hold_land": percent}`,
`{"survive": minutes}` or `{"mint_coins": n}`. Met, the game says so and offers to go on. `human`
is the slot the human plays (counted from 0); without it the menu's choice counts, and `--me N`
overrides both. The editor sets both under "Mission".

These seven keys are all there are: a file with any other key (a typo such as `"truce"`), or one
that isn't valid JSON, is refused. The menu then says which file and where it broke, and doesn't
start the map until the file is fixed or removed. The editor leaves such a file as it is when it
saves the map.
