import { describe, expect, it } from 'vitest'
import { describeEvent, destroyedSquares, lineName, squareName } from './log'
import type { GameEvent } from './types'

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
  it('lists every casualty of a volley', () => {
    const event: GameEvent = {
      type: 'attacked',
      player: 1,
      token: 'column',
      line: 3,
      attackers: [{ row: 3, col: 3 }],
      destroyedPieces: [[{ row: 2, col: 3 }, { kind: 'archer', owner: 0 }]],
      destroyedCastles: [
        { row: 0, col: 0 },
        { row: 0, col: 3 },
      ],
    }
    expect(describeEvent(event)).toEqual({
      player: 1,
      text: 'volleys along file d, destroying Archer on d5, the castle on a7, the castle on d7',
      grave: true,
    })
  })

  it('says so when a volley hits nothing', () => {
    const event: GameEvent = {
      type: 'attacked',
      player: 0,
      token: 'row',
      line: 6,
      attackers: [],
      destroyedPieces: [],
      destroyedCastles: [],
    }
    expect(describeEvent(event)?.text).toBe('volleys along rank 1 — no casualties')
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
  it('collects pieces and castles from every volley in a batch', () => {
    const events: GameEvent[] = [
      { type: 'slid', player: 0, token: 'row', from: 0, to: 2 },
      {
        type: 'attacked',
        player: 0,
        token: 'row',
        line: 2,
        attackers: [],
        destroyedPieces: [[{ row: 1, col: 1 }, { kind: 'flail', owner: 1 }]],
        destroyedCastles: [{ row: 6, col: 6 }],
      },
    ]
    expect(destroyedSquares(events)).toEqual([
      { row: 1, col: 1 },
      { row: 6, col: 6 },
    ])
  })
})
