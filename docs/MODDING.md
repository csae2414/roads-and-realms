# Modding the rules

The game's numbers live in one file, `data/rules/classic.toml`: the classic values, some of them
from the open-source Return to the Roots. The game has a copy of it built in. To try other numbers, copy the file,
change values, and start with it:

```
rr-app --rules my.toml            # the game
rr-cli soak --rules my.toml ...   # headless AI games (also ladder and fairness)
```

What `--rules` changes, and what it doesn't:

- Games on this machine play by the file: new games, loaded saves and replays. A save made under
  other rules is refused (saves keep the hash of their rules).
- **Network games always play the classic rules**, since every machine has them.
- The file has the classic file's shape: every section, every job and every building must be
  there. A key the game doesn't know, a missing one or a wrong type is refused with the key's name
  and the line (`unknown field fight_kk ...`).
- Numbers only. The list of wares, jobs and buildings, the terrain, nature and the building
  programs are in the code. A mod can tune them, but it can't add a building.

Times are in game frames (GF): 20 a second at normal speed. Distances are in nodes.

## [military]

| Key | Unit | What it does |
|---|---|---|
| `hp` | hit points | the hit points of the five ranks, private to general |
| `fight_k` | — | a strike lands on `rand(rank + fight_k)`; RTTR's "medium strength" is 10 |
| `heal_every` | GF | a soldier at home heals 1 HP every `heal_every` + rand(`heal_rand`) |
| `heal_rand` | GF | see `heal_every` |
| `promote_wait` | GF | after a coin arrives, a promotion follows in `promote_wait` + rand(`promote_rand`) |
| `promote_rand` | GF | see `promote_wait` |
| `attack_radius` | nodes | how far from a post its soldiers may be sent to attack (plus 10 for the posts' reach) |
| `attack_max_walk` | steps | the longest walk an attacker takes to its target |
| `recruit_wait` | GF | a warehouse makes a recruit every `recruit_wait` + rand(`recruit_rand`) |
| `recruit_rand` | GF | see `recruit_wait` |

## [economy]

| Key | Unit | What it does |
|---|---|---|
| `flag_capacity` | wares | how many wares wait at one flag |
| `helper_spawn` | GF | a warehouse gets a helper every `helper_spawn` + rand(`helper_spawn_rand`) |
| `helper_spawn_rand` | GF | see `helper_spawn` |
| `helper_max` | people | no new helpers while this many wait in store |
| `donkey_busy_percent` | % | a road busier than this gets a donkey |
| `donkey_window` | GF | over how long a road's business is counted |
| `woodcutter_radius` | nodes | how far a woodcutter looks for trees |
| `forester_radius` | nodes | how far a forester plants |
| `stonemason_radius` | nodes | how far a stonemason looks for stone |
| `fisher_radius` | nodes | how far a fisher looks for fish |
| `hunter_radius` | nodes | how far a hunter looks for game |
| `farmer_radius` | nodes | how far a farmer sows and reaps |
| `geologist_radius` | nodes | how far a geologist walks from his flag |
| `geologist_flags` | flags | how many signs one geologist puts up |

## [jobs]

One line per job, for example `Woodcutter = { tool = "Axe", work = 148, wait1 = 789, wait2 = 5 }`.

| Key | Unit | What it does |
|---|---|---|
| `tool` | ware | the tool a helper needs to take the job (none: no tool) |
| `work` | GF | how long one piece of work takes |
| `wait1` | GF | the pause before the work |
| `wait2` | GF | the pause after it |

## [buildings]

One line per building, for example `Woodcutter = { size = "Hut", boards = 2, worker = "Woodcutter", output = "Wood" }`.

| Key | Unit | What it does |
|---|---|---|
| `size` | — | `Hut`, `House`, `Castle`, `Mine` or `Harbor`: the building spot it needs |
| `boards` | wares | boards it costs |
| `stones` | wares | stones it costs |
| `worker` | job | who works there |
| `inputs` | wares | what it consumes |
| `slots_per_input` | wares | how many of each input it stores (6 if left out) |
| `use_one_each` | — | `true`: one of each input per cycle; `false`: one of whichever it holds most of |
| `output` | ware | what it makes |
| `military` | table | for posts: see below |

`military = { soldiers = 2, coins = 1, radius = 8, sight = 11 }`:

| Key | Unit | What it does |
|---|---|---|
| `soldiers` | people | how many soldiers it holds |
| `coins` | wares | how many coins it stores for promotions |
| `radius` | nodes | the land it claims |
| `sight` | nodes | how far it sees |

## [start.wares] and [start.jobs]

What a headquarters starts with at the "medium" start goods: a count per ware and per job, by
name. "Low" halves them, "high" doubles them.
