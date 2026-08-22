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
        text: `has no move for the ${tokenName(event.token)} token`,
      }
    case 'attacked': {
      const kills = event.destroyedPieces.map(
        ([square, piece]) => `${PIECE_NAMES[piece.kind]} on ${squareName(square)}`,
      )
      const razed = event.destroyedCastles.map((square) => `the castle on ${squareName(square)}`)
      const casualties = [...kills, ...razed]
      const where = lineName(event.token, event.line)
      if (casualties.length === 0) {
        return { player: event.player, text: `volleys along ${where} — no casualties` }
      }
      return {
        player: event.player,
        text: `volleys along ${where}, destroying ${casualties.join(', ')}`,
        grave: true,
      }
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
  const squares: Square[] = []
  for (const event of events) {
    if (event.type !== 'attacked') continue
    for (const [square] of event.destroyedPieces) squares.push(square)
    for (const square of event.destroyedCastles) squares.push(square)
  }
  return squares
}
