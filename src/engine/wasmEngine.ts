// Thin wrapper over the wasm bindings. Every rule lives in Rust; nothing here
// decides anything about the game, it only moves JSON across the boundary.

import init, {
  wasm_apply_choice,
  wasm_attack_preview,
  wasm_initial_state,
  wasm_legal_choices,
} from '../wasm-pkg/command_slide_wasm.js'
import type { AttackPreview, Choice, GameEvent, GameState, TokenKind } from '../data/types'

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

/** What this player's token would destroy if its attack resolved right now. */
export function attackPreview(
  state: GameState,
  player: number,
  token: TokenKind,
): AttackPreview {
  return JSON.parse(
    wasm_attack_preview(JSON.stringify(state), player, JSON.stringify(token)),
  )
}
