import AiWorkerModule from './aiWorker?worker'
import type { AiRequest, AiResponse, AiTurn } from './aiWorker'
import type { GameState } from '../data/types'

export interface Difficulty {
  name: string
  iterations: number
  blurb: string
}

/// Iterations are per decision, and a turn is up to three decisions.
/// Throughput falls as the tree grows — a little over 500k iterations a second
/// natively at Squire, about 425k at Warlord, and less again in wasm — so
/// Warlord takes a second or more per decision and several over a whole turn.
/// Everything below it still answers inside a second.
export const DIFFICULTIES: Difficulty[] = [
  { name: 'Squire', iterations: 4_000, blurb: 'Sees the next exchange, little more.' },
  { name: 'Knight', iterations: 30_000, blurb: 'Plays the siege race honestly.' },
  { name: 'Marshal', iterations: 120_000, blurb: 'Punishes a loose siege engine.' },
  { name: 'Warlord', iterations: 500_000, blurb: 'Takes a moment, and takes the game.' },
]

/// One worker for the whole session. Each request supersedes the last, so a
/// reply whose id has gone stale — the player started a new game mid-search —
/// is dropped rather than applied to a position it was never about.
export class AiController {
  private worker: Worker
  private nextId = 1
  private pending: {
    id: number
    resolve: (turn: AiTurn) => void
    reject: (error: Error) => void
  } | null = null

  constructor() {
    this.worker = new AiWorkerModule()
    this.worker.onmessage = (event: MessageEvent<AiResponse>) => {
      const waiting = this.pending
      if (!waiting || waiting.id !== event.data.id) return
      this.pending = null
      if (event.data.type === 'error') {
        waiting.reject(new Error(event.data.message))
      } else {
        const { choices, state, events } = event.data
        waiting.resolve({ choices, state, events })
      }
    }
  }

  takeTurn(state: GameState, iterations: number): Promise<AiTurn> {
    const id = this.nextId++
    return new Promise((resolve, reject) => {
      this.pending = { id, resolve, reject }
      const request: AiRequest = { id, state, iterations }
      this.worker.postMessage(JSON.parse(JSON.stringify(request)))
    })
  }

  /** Abandon any reply still in flight. */
  cancel(): void {
    this.pending = null
  }

  dispose(): void {
    this.worker.terminate()
  }
}
