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
