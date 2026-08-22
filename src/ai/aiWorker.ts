// Runs the search off the main thread so the board stays responsive while the
// opponent thinks.

import init, { wasm_ai_take_turn } from '../wasm-pkg/command_slide_wasm.js'
import type { Choice, GameEvent, GameState } from '../data/types'

export interface AiRequest {
  id: number
  state: GameState
  iterations: number
}

export interface AiTurn {
  choices: Choice[]
  state: GameState
  events: GameEvent[]
}

export type AiResponse =
  | ({ type: 'success'; id: number } & AiTurn)
  | { type: 'error'; id: number; message: string }

let ready: Promise<unknown> | null = null

function ensureInit(): Promise<unknown> {
  if (!ready) ready = init()
  return ready
}

self.onmessage = async (event: MessageEvent<AiRequest>) => {
  const { id, state, iterations } = event.data
  try {
    await ensureInit()
    const turn: AiTurn = JSON.parse(wasm_ai_take_turn(JSON.stringify(state), iterations))
    self.postMessage({ type: 'success', id, ...turn } satisfies AiResponse)
  } catch (error) {
    self.postMessage({
      type: 'error',
      id,
      message: String(error),
    } satisfies AiResponse)
  }
}
