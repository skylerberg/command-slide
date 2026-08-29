// Turns engine events into the sentences shown in the sidebar.

import { PIECE_NAMES, PLAYER_NAMES } from './types'
import type { GameEvent, Square, TokenKind } from './types'

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
