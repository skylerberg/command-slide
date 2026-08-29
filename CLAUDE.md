# Command Slide

Command Slide is a two-player siege game on a 7×7 board. Complete rules,
including every interpretation chosen where the printed rules were silent, are
in `RULES.md`.

## Tech Stack

- Rust (game engine and AI), Svelte 5 + TypeScript (frontend), Vite (build)
- Monte Carlo tree search via the `mcts` crate from the sibling `monte_carlo`
  repository, consumed as a path dependency from the workspace root

## Project Structure

Rust workspace plus a Svelte frontend:

- `command-slide-core/` — the whole game engine: `types.rs` (board, pieces,
  tokens, state), `rules.rs` (setup, legal choices, turn resolution),
  `search.rs` (the `mcts::Game` impl and the leaf evaluation)
- `command-slide-wasm/` — wasm-bindgen bindings, JSON in and JSON out
- `command-slide-runner/` — CLI for simulation, replay and benchmarking
- `src/` — Svelte 5 + TypeScript frontend
  - `src/data/` — TypeScript mirror of the wire format, and the log formatter
  - `src/engine/` — wasm loader and wrappers
  - `src/ai/` — the Web Worker the search runs in, and its controller
  - `src/components/` — board, panels, rules sheet, menu
  - `src/assets/` — the designer's piece and token art, one PNG per side
  - `src/wasm-pkg/` — generated wasm output (gitignored)

## Build & Run Commands

- `pnpm install` — install frontend dependencies
- `pnpm run dev` — Vite dev server
- `pnpm run build:wasm` — rebuild the wasm bindings into `src/wasm-pkg/`
- `pnpm run build` — build wasm, then the production site
- `pnpm run check` — svelte-check
- `pnpm test` — frontend tests (Vitest)
- `cargo test` — Rust tests
- `pnpm run run-games -- <subcommand>` — the runner: `simulate`, `random`,
  `replay`, `bench`

## Architecture Notes

- **Every rule lives in `command-slide-core`.** The frontend asks the engine for
  legal choices and never decides anything about the game itself. A rule
  implemented in TypeScript is a bug.
- `GameState` is `Copy`: fixed arrays, no `Vec`, so a determinization is a
  memcpy. Keep it that way — the search clones one per iteration.
- A turn is modelled as separate decisions — slide, activation order, piece
  move, and one target per attacker in a volley — rather than one compound
  choice, to keep the branching factor down. `settle` runs the game past
  everything that carries no decision, so `Phase::Activate` always has a
  movement-face token at the head of the queue; an attack face goes to
  `Phase::Attack`, which walks the line and stops on each piece with a shot.
- `apply_with` is generic over an `EventSink` so the search path builds no
  events at all while the UI path gets a full log.
- The wire format between Rust and TypeScript is pinned by
  `command-slide-core/tests/wire_format.rs`. Change a serde attribute and change
  `src/data/types.ts` to match; the test is what catches the drift.
- Changes to the evaluation in `search.rs` are judged by
  `pnpm run run-games -- simulate`, playing the new numbers against the old.

## Testing

- **Rust**: inline `#[cfg(test)]` modules in `rules.rs` plus the wire-format
  integration test. Run with `cargo test`.
- **TypeScript**: Vitest, `src/**/*.test.ts`. Run with `pnpm test`.

## Code Conventions

- Default `rustfmt` for Rust; no explicit formatter config for TypeScript
- Comments are minimal and explain *why*, not what
- Serde JSON is the format across every boundary; structs use
  `rename_all = "camelCase"` and enums additionally `rename_all_fields`
- Svelte 5 runes. Do not name a variable `state` — it shadows the `$state` rune
  and Svelte will read `$state.snapshot(state)` as a store subscription. The
  game state is called `game` throughout.
