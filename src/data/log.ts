// Turns engine events into the sentences shown in the sidebar.

import { PIECE_NAMES, PLAYER_NAMES, pieceAt } from './types'
import type { GameEvent, GameState, MoveOutcome, SlideOutcome, Square, TokenKind } from './types'

export interface LogEntry {
  player: number
  text: string
  /** Set on entries that removed something, so the log can mark them. */
  grave?: boolean
}

/** Files run a-g left to right; ranks run 7-1 from the top row down. */
export function squareName(square: Square): string {
  return `${'abcdefg'[square.col]}${7 - square.row}`
}

export function lineName(kind: TokenKind, line: number): string {
  return kind === 'row' ? `rank ${7 - line}` : `file ${'abcdefg'[line]}`
}

function tokenName(kind: TokenKind): string {
  return kind === 'row' ? 'rank' : 'file'
}

export function describeEvent(event: GameEvent): LogEntry | null {
  switch (event.type) {
    case 'slid':
      return {
        player: event.player,
        text: `slides the ${tokenName(event.token)} token to ${lineName(event.token, event.to)}`,
      }
    case 'moved':
      return {
        player: event.player,
        text: `${PIECE_NAMES[event.kind]} ${squareName(event.from)} → ${squareName(event.to)}`,
      }
    case 'passed':
      return {
        player: event.player,
        text: `takes no action with the ${tokenName(event.token)} token`,
      }
    case 'volley':
      return {
        player: event.player,
        text: `volleys along ${lineName(event.token, event.line)}`,
      }
    case 'struck': {
      const victim =
        event.casualty.type === 'castle'
          ? `the castle on ${squareName(event.target)}`
          : `${PIECE_NAMES[event.casualty.piece.kind]} on ${squareName(event.target)}`
      return {
        player: event.player,
        text: `${PIECE_NAMES[event.kind]} ${squareName(event.from)} destroys ${victim}`,
        grave: true,
      }
    }
    case 'heldFire':
      return {
        player: event.player,
        text: `${PIECE_NAMES[event.kind]} ${squareName(event.from)} holds its fire`,
      }
    case 'gameOver':
      // No speaker prefix: the sentence already names the winner.
      return {
        player: -1,
        text:
          event.outcome.type === 'winner'
            ? `${PLAYER_NAMES[event.outcome.player]} wins`
            : 'the campaign ends in a draw',
        grave: true,
      }
    case 'turnEnded':
      return null
  }
}

/** Squares emptied by these events, for the strike flash on the board. */
export function destroyedSquares(events: GameEvent[]): Square[] {
  return events.filter((event) => event.type === 'struck').map((event) => event.target)
}

function joinList(items: string[]): string {
  if (items.length < 2) return items.join('')
  return `${items.slice(0, -1).join(', ')} and ${items[items.length - 1]}`
}

/** Names what stands on each square, for a sentence about shooting at it. */
function targetNames(state: GameState, pieces: Square[], castles: Square[]): string[] {
  return [
    ...pieces.map((square) => {
      const piece = pieceAt(state, square)
      return piece ? `${PIECE_NAMES[piece.kind]} ${squareName(square)}` : squareName(square)
    }),
    ...castles.map((square) => `the castle on ${squareName(square)}`),
  ]
}

/** The sentence under the board while a destination is under the cursor. */
export function describeMoveOutcome(state: GameState, outcome: MoveOutcome): string {
  const journey = `${squareName(outcome.from)} → ${squareName(outcome.to)}`
  const targets = targetNames(state, outcome.threatenedPieces, outcome.threatenedCastles)
  if (targets.length === 0) return `${journey}.`
  return `${journey}, and your volley then bears on ${joinList(targets)}.`
}

/** The sentence under the board while a slide destination is under the cursor. */
export function describeSlideOutcome(state: GameState, outcome: SlideOutcome): string {
  const where = lineName(outcome.token, outcome.line)
  const movers =
    outcome.movers.length === 0
      ? `Nothing on ${where} could move`
      : `${outcome.movers.length} piece${outcome.movers.length === 1 ? '' : 's'} on ${where} could move`
  const targets = targetNames(state, outcome.threatenedPieces, outcome.threatenedCastles)
  const volley =
    targets.length === 0
      ? 'next turn the volley there bears on nothing, as the board stands'
      : `next turn the volley there bears on ${joinList(targets)}, as the board stands`
  return `${movers}; ${volley}.`
}
