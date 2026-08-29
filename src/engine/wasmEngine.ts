// Thin wrapper over the wasm bindings. Every rule lives in Rust; nothing here
// decides anything about the game, it only moves JSON across the boundary.

import init, {
  wasm_apply_choice,
  wasm_attack_preview,
  wasm_forced_order,
  wasm_initial_state,
  wasm_legal_choices,
  wasm_move_outcomes,
  wasm_pending_attackers,
  wasm_slide_outcomes,
} from '../wasm-pkg/command_slide_wasm.js'
import type {
  AttackPreview,
  Choice,
  GameEvent,
  GameState,
  MoveOutcome,
  SlideOutcome,
  Square,
  TokenKind,
} from '../data/types'

let ready: Promise<unknown> | null = null

export function initEngine(): Promise<unknown> {
  if (!ready) ready = init()
  return ready
}

export function initialState(): GameState {
  return JSON.parse(wasm_initial_state())
}

export function legalChoices(state: GameState): Choice[] {
  return JSON.parse(wasm_legal_choices(JSON.stringify(state)))
}

export interface Applied {
  state: GameState
  events: GameEvent[]
}

export function applyChoice(state: GameState, choice: Choice): Applied {
  return JSON.parse(wasm_apply_choice(JSON.stringify(state), JSON.stringify(choice)))
}

/** What this player's token bears on. Each attacker takes one of these. */
export function attackPreview(
  state: GameState,
  player: number,
  token: TokenKind,
): AttackPreview {
  return JSON.parse(
    wasm_attack_preview(JSON.stringify(state), player, JSON.stringify(token)),
  )
}

/** The activation order to take for the player, or null to put it to them. */
export function forcedOrder(state: GameState): TokenKind | null {
  return JSON.parse(wasm_forced_order(JSON.stringify(state)))
}

/** Every move in the current activation, with what it puts in reach. */
export function moveOutcomes(state: GameState): MoveOutcome[] {
  return JSON.parse(wasm_move_outcomes(JSON.stringify(state)))
}

/** Every slide available, with what it frees to move and what it arms. */
export function slideOutcomes(state: GameState): SlideOutcome[] {
  return JSON.parse(wasm_slide_outcomes(JSON.stringify(state)))
}

/** The attackers of the volley in progress that have still to fire. */
export function pendingAttackers(state: GameState): Square[] {
  return JSON.parse(wasm_pending_attackers(JSON.stringify(state)))
}
