# Command Slide

A playable implementation of Command Slide — a two-player siege game on a board
nine files across and seven ranks deep — with a browser interface and an
opponent driven by Monte Carlo tree search. The complete rules, including every reading chosen where the printed
rules were silent, are in [RULES.md](RULES.md).

## Running it

```sh
pnpm install
pnpm run build:wasm   # compiles the Rust engine to src/wasm-pkg/
pnpm run dev
```

`pnpm run build` does both steps and emits a static site into `dist/`.

Requires a Rust toolchain with the `wasm32-unknown-unknown` target, and
[`wasm-pack`](https://drager.github.io/wasm-pack/).

## Deploying

`.github/workflows/deploy.yml` builds the site on every push to `main` and
publishes it to GitHub Pages at `https://skylerberg.github.io/command-slide/`.
Nothing about the engine is configured here: `mcts` is a git dependency pinned
in the workspace `Cargo.toml`, so the workflow builds the commit that names.
Vite's `base` is pinned to `/command-slide/` to match the project-page URL;
serving from anywhere else means changing it.

Enable it under *Settings → Pages* by setting the source to *GitHub Actions*.

## Layout

A Rust workspace and a Svelte frontend:

* `command-slide-core/` — the whole game engine: board, rules, turn resolution,
  the `mcts::Game` implementation and the evaluation the search leans on.
* `command-slide-wasm/` — `wasm-bindgen` bindings. Every function takes and
  returns JSON, so TypeScript never mirrors a Rust memory layout.
* `command-slide-runner/` — CLI for batch simulation, replay and benchmarking.
* `src/` — Svelte 5 + TypeScript frontend.
  * `src/data/` — TypeScript mirror of the wire format, plus the log formatter.
  * `src/engine/` — the wasm loader and its wrappers.
  * `src/ai/` — the worker the search runs in, and the controller driving it.
  * `src/components/` — board, panels, rules sheet, menu.

Every rule lives in `command-slide-core`. The frontend asks the engine for legal
choices and never decides anything about the game itself.

## The AI

The opponent is [`mcts`](https://github.com/skylerberg/monte_carlo), the
`monte_carlo` crate, pinned to a commit in `Cargo.toml` — bump it deliberately
rather than tracking that repository's default branch. The game is deterministic
and perfect information, so most of the trait falls away: `determinize_into` is
a bitwise copy, `Side` is `()`, and `ROOT_CHOICES_INVARIANT` is provably true.

Two things about the integration are worth knowing:

**A turn is searched as three decisions, not one.** Enumerating whole turns —
slide × order × piece × destination — would branch in the hundreds at every
node. Splitting a turn into the decisions a player actually makes keeps each
node down to a few dozen children while leaving the set of reachable positions
identical.

**The leaf evaluation is load-bearing.** Both win conditions sit far outside a
random rollout's reach: random play will not walk a trebuchet onto a hilltop or
push a ram up to a castle wall. Left to an uninformed rollout the search learns
nothing. `search.rs` scores material, castles, a trebuchet's distance from the
nearest hilltop and a ram's from the nearest castle, and squashes the difference
into `[0, 1]` — which is what gives the search a gradient to climb.

Search runs at roughly 600k iterations a second natively at a small budget,
falling towards 425k as the tree grows. Every difficulty but the top one
answers inside a second in the browser; the top one trades a few seconds of
thinking for the strength.

## Checking it

```sh
cargo test                                    # 36 engine tests
pnpm test                                      # 18 frontend tests
pnpm run check                                 # svelte-check

pnpm run run-games -- bench                    # search throughput
pnpm run run-games -- random --games 400       # rules terminate, both wins reachable
pnpm run run-games -- replay --iterations 4000 # one game, printed move by move
pnpm run run-games -- simulate --games 40 --iterations-a 20000 --iterations-b 3000
pnpm run run-games -- tune --generations 50   # search the evaluation weights
```

`cargo test` covers each attack pattern against the printed diagrams, the turn
and token-flip structure, both win conditions, and the JSON wire format the
browser reads — a rename on the Rust side fails a test rather than breaking the
UI silently at runtime.

`simulate` is how any change to the evaluation gets judged: play the new numbers
against the old and count. The search scales monotonically with its budget —
3k iterations beats 200 by 85%, and 20k beats 3k outright — which is the signal
that it is doing real work. Weights load from a file with `--params-a`, which is
how a tuning run's output gets confirmed.

## Tuning the weights

`tune` searches the fourteen evaluation weights by self-play, on the `mcts-tune`
crate from the `monte_carlo` repository. Candidates are proposed by CMA-ES (or
`--strategy ga`), each plays a few hundred games against the weights the run
started from, and the win rates drive the search. Every generation writes its
best weights to `tuning/gen-NNN.json` and appends a line to
`tuning/history.jsonl`.

By default candidates are measured against the weights the run started from.
That gives fitness an absolute scale you can read progress from, and it
saturates: a 100-generation run here plateaued at generation 8 and by generation
43 the whole population sat between 0.88 and 0.94, closer together than the
error on measuring them, so selection was ranking noise. `--field round-robin`
plays the candidates against each other instead. The field improves with the
population, so there is nothing to saturate and no fixed opponent to specialise
against, and every game scores two candidates rather than one — at equal cost
each candidate is measured on twice as many games. The trade is that fitness
becomes relative: the population mean is 0.5 in every generation by
construction, so the column stops being a progress bar and `simulate
--params-a` is how you see whether the run is actually improving.

Three things about it are worth knowing before reading a result.

**Fitness is a win rate, so it is noisy, and that governs the whole design.**
The standard error over `n` games is at worst `sqrt(0.25 / n)` — 2.5 percentage
points at 400 games. Candidates that differ by less than that are ranked by
luck. `--games-per-eval` matters more than `--population`; below about 200 a run
is measuring noise. To push back on it, every candidate in a generation plays
the same seeds and every matchup is played from both seats, so a comparison
carries neither each candidate's own luck nor the first-move advantage.

**`scale` is not tuned.** `evaluate` returns `tanh(difference / scale)` and the
difference is linear in the weights, so multiplying every weight *and* `scale`
by the same constant gives a bit-identical evaluation. Tuning both ends of that
family would spend games wandering a direction that cannot change a game.

**A run is resumable.** Every generation writes `tuning/checkpoint.json`
alongside its parameters, through a temporary file and a rename so that a kill
landing mid-write cannot destroy the state it exists to protect. Continue with
the same command plus `--resume`:

```sh
pnpm run run-games -- --threads 10 tune --generations 100 --resume
```

Pass the same `--seed-params` the original run used. Fitness is a win rate
*against those weights*, so resuming against different ones — reaching for the
previous run's output is the natural mistake — would restart the scale at even
money and make every number after the resume incomparable with every number
before it. That is refused rather than allowed to happen quietly.

**A run's reported best is biased upward.** It is a maximum over noisy
measurements, so the luckiest candidate wins ties it would lose on a rerun.
Confirm it before adopting it, at the budget the game actually ships:

```sh
pnpm run run-games -- simulate --games 1000 \
  --params-a tuning/best.json --iterations-a 50000 --iterations-b 50000
```

That last point is the one to watch: weights matter most when the tree is
shallow, so weights tuned at `--eval-iterations 4000` are tuned for a regime the
browser's higher difficulties never play in.

## A note on the game

Games are short. Under equal search both sides reach a verdict in seven or eight
turns, and most of them end by eliminating a side's siege engines rather than by
razing castles — a piece strikes one target per volley, so three castles take at
least three turns to bring down, while two siege engines can be picked off much
sooner. The trebuchet is still what both sides play around: castles and hilltops
share files, so from the centre hilltop it bears on every castle on the board
and takes one a turn. The counterplay is the telegraph — a token is slid a full
turn before it fires, so an approach can be seen and answered — and the walls,
which pen every hilltop in on the middle rank and cannot be broken by the siege
train that wants to stand there.
