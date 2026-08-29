import { describe, expect, it } from 'vitest'
import {
  describeEvent,
  describeMoveOutcome,
  describeSlideOutcome,
  destroyedSquares,
  lineName,
  squareName,
} from './log'
import { BOARD_SIZE } from './types'
import type { GameEvent, GameState, PieceKind, Square } from './types'

describe('board coordinates', () => {
  it('names squares with files left to right and ranks top to bottom', () => {
    expect(squareName({ row: 0, col: 0 })).toBe('a7')
    expect(squareName({ row: 6, col: 6 })).toBe('g1')
    expect(squareName({ row: 3, col: 3 })).toBe('d4')
  })

  it('names a token line by the kind of line it rides', () => {
    expect(lineName('row', 0)).toBe('rank 7')
    expect(lineName('column', 0)).toBe('file a')
  })
})

describe('describeEvent', () => {
  it('announces a volley by the line it comes from', () => {
    const event: GameEvent = { type: 'volley', player: 1, token: 'column', line: 3 }
    expect(describeEvent(event)).toEqual({ player: 1, text: 'volleys along file d' })
  })

  it('names the attacker and its one casualty', () => {
    const event: GameEvent = {
      type: 'struck',
      player: 1,
      token: 'column',
      kind: 'archer',
      from: { row: 3, col: 3 },
      target: { row: 1, col: 1 },
      casualty: { type: 'piece', piece: { kind: 'flail', owner: 0 } },
    }
    expect(describeEvent(event)).toEqual({
      player: 1,
      text: 'Archer d4 destroys Flail on b6',
      grave: true,
    })
  })

  it('names a razed castle by its square', () => {
    const event: GameEvent = {
      type: 'struck',
      player: 0,
      token: 'row',
      kind: 'trebuchet',
      from: { row: 3, col: 3 },
      target: { row: 6, col: 0 },
      casualty: { type: 'castle' },
    }
    expect(describeEvent(event)?.text).toBe('Trebuchet d4 destroys the castle on a1')
  })

  it('records an attacker that declined its shot', () => {
    const event: GameEvent = {
      type: 'heldFire',
      player: 0,
      token: 'row',
      kind: 'swordsman',
      from: { row: 3, col: 3 },
    }
    expect(describeEvent(event)?.text).toBe('Swordsman d4 holds its fire')
  })

  it('leaves the game-over line unattributed so it is not prefixed twice', () => {
    const entry = describeEvent({
      type: 'gameOver',
      outcome: { type: 'winner', player: 1 },
    })
    expect(entry).toEqual({ player: -1, text: 'Umber wins', grave: true })
  })

  it('drops turn boundaries, which the log does not show', () => {
    expect(describeEvent({ type: 'turnEnded', player: 0 })).toBeNull()
  })
})

describe('destroyedSquares', () => {
  it('collects the target of every shot in a batch', () => {
    const events: GameEvent[] = [
      { type: 'slid', player: 0, token: 'row', from: 0, to: 2 },
      { type: 'volley', player: 0, token: 'row', line: 2 },
      {
        type: 'struck',
        player: 0,
        token: 'row',
        kind: 'swordsman',
        from: { row: 2, col: 1 },
        target: { row: 1, col: 1 },
        casualty: { type: 'piece', piece: { kind: 'flail', owner: 1 } },
      },
      {
        type: 'struck',
        player: 0,
        token: 'row',
        kind: 'batteringRam',
        from: { row: 5, col: 6 },
        target: { row: 6, col: 6 },
        casualty: { type: 'castle' },
      },
    ]
    expect(destroyedSquares(events)).toEqual([
      { row: 1, col: 1 },
      { row: 6, col: 6 },
    ])
  })
})

/** A board with nothing on it but the pieces a test names. */
function position(pieces: [Square, PieceKind, number][]): GameState {
  const board = Array.from({ length: BOARD_SIZE }, () =>
    Array.from({ length: BOARD_SIZE }, () => null),
  ) as GameState['board']
  for (const [square, kind, owner] of pieces) board[square.row][square.col] = { kind, owner }
  return {
    board,
    castles: [
      [true, true, true],
      [true, true, true],
    ],
    tokens: [
      [
        { line: 0, face: 'movement' },
        { line: 0, face: 'attack' },
      ],
      [
        { line: 6, face: 'movement' },
        { line: 6, face: 'attack' },
      ],
    ],
    currentPlayer: 0,
    phase: 'activate',
    pending: ['row', 'column'],
    pendingLen: 2,
    attackIndex: 0,
    turn: 3,
    outcome: null,
  }
}

describe('describeMoveOutcome', () => {
  const state = position([[{ row: 1, col: 3 }, 'swordsman', 1]])

  it('names what the volley after the move would take', () => {
    expect(
      describeMoveOutcome(state, {
        from: { row: 0, col: 1 },
        to: { row: 1, col: 2 },
        threatenedPieces: [{ row: 1, col: 3 }],
        threatenedCastles: [{ row: 6, col: 0 }],
      }),
    ).toBe('b7 → c6, and your volley then bears on Swordsman d6 and the castle on a1.')
  })

  it('says only where the piece goes when nothing follows', () => {
    expect(
      describeMoveOutcome(state, {
        from: { row: 0, col: 1 },
        to: { row: 1, col: 0 },
        threatenedPieces: [],
        threatenedCastles: [],
      }),
    ).toBe('b7 → a6.')
  })
})

describe('describeSlideOutcome', () => {
  const state = position([[{ row: 2, col: 3 }, 'archer', 1]])

  it('reports both halves of a slide: the move now and the volley next turn', () => {
    expect(
      describeSlideOutcome(state, {
        token: 'row',
        line: 2,
        movers: [{ row: 2, col: 1 }],
        covered: [{ row: 2, col: 3 }],
        threatenedPieces: [{ row: 2, col: 3 }],
        threatenedCastles: [],
      }),
    ).toBe(
      '1 piece on rank 5 could move; next turn the volley there bears on Archer d5, as the board stands.',
    )
  })

  it('says plainly when a line offers neither', () => {
    expect(
      describeSlideOutcome(state, {
        token: 'column',
        line: 5,
        movers: [],
        covered: [],
        threatenedPieces: [],
        threatenedCastles: [],
      }),
    ).toBe(
      'Nothing on file f could move; next turn the volley there bears on nothing, as the board stands.',
    )
  })
})
