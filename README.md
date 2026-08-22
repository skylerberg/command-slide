# Command Slide

A playable implementation of Command Slide — a two-player siege game on a 7×7
board — with a browser interface and an opponent driven by Monte Carlo tree
search. The complete rules, including every reading chosen where the printed
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
The workflow checks out `monte_carlo` alongside this repository so the `mcts`
path dependency resolves, at the commit named in the `ref:` there — bump it
deliberately rather than tracking that repository's default branch. Vite's
`base` is pinned to `/command-slide/` to match the project-page URL; serving
from anywhere else means changing it.

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

The opponent is [`mcts`](https://github.com/skylerberg/monte_carlo), the sibling
`monte_carlo` crate, consumed as a path dependency. The game is deterministic
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

Search runs at roughly 600k iterations a second natively, so even the top
difficulty answers well inside a second in the browser.

## Checking it

```sh
cargo test                                    # 26 engine tests
pnpm test                                      # 12 frontend tests
pnpm run check                                 # svelte-check

pnpm run run-games -- bench                    # search throughput
pnpm run run-games -- random --games 400       # rules terminate, both wins reachable
pnpm run run-games -- replay --iterations 4000 # one game, printed move by move
pnpm run run-games -- simulate --games 40 --iterations-a 20000 --iterations-b 3000
```

`cargo test` covers each attack pattern against the printed diagrams, the turn
and token-flip structure, both win conditions, and the JSON wire format the
browser reads — a rename on the Rust side fails a test rather than breaking the
UI silently at runtime.

`simulate` is how any change to the evaluation gets judged: play the new numbers
against the old and count. The search scales monotonically with its budget —
3k iterations beats 200 by 85%, and 20k beats 3k outright — which is the signal
that it is doing real work.

## A note on the game

Games are short. Under equal search both sides reach a verdict in seven or eight
turns, and a trebuchet that reaches the centre hilltop unanswered razes all
three enemy castles in a single volley. That is faithful to the rules rather than
a quirk of the implementation: castles and hilltops share files, so the centre
hilltop is exactly three squares from every castle on the board. The counterplay
is the telegraph — a token is slid a full turn before it fires, so an approach
can be seen and answered.
