import { describe, expect, it } from 'vitest'
import { attackTargets, castleSlotAt, isHilltop, squareKey } from './types'
import type { GameState } from './types'

/** Only the castles matter to the terrain rules under test. */
function board(castles: boolean[][]): GameState {
  return { castles } as GameState
}

describe('attack patterns', () => {
  it('matches the printed diagrams', () => {
    const centre = { row: 3, col: 3 }
    const keys = (kind: Parameters<typeof attackTargets>[1]) =>
      attackTargets(centre, kind).map(squareKey).sort()

    expect(keys('swordsman')).toEqual(['2,3', '3,2', '3,4', '4,3'].sort())
    expect(keys('flail')).toEqual(['2,2', '2,4', '4,2', '4,4'].sort())
    expect(keys('spearman')).toEqual(['1,3', '3,1', '3,5', '5,3'].sort())
    expect(keys('archer')).toEqual(['1,1', '1,5', '5,1', '5,5'].sort())
  })

  it('clips targets that fall off the board', () => {
    expect(attackTargets({ row: 0, col: 0 }, 'archer').map(squareKey)).toEqual(['2,2'])
  })

  it('gives the centre trebuchet a line on every castle', () => {
    const castles = attackTargets({ row: 3, col: 3 }, 'trebuchet')
      .filter((square) => castleSlotAt(square) !== null)
      .map(squareKey)
      .sort()
    expect(castles).toEqual(['0,0', '0,3', '0,6', '6,0', '6,3', '6,6'])
  })
})

describe('terrain', () => {
  it('puts the printed hilltops on the middle row only', () => {
    const intact = board([
      [true, true, true],
      [true, true, true],
    ])
    expect(isHilltop(intact, { row: 3, col: 0 })).toBe(true)
    expect(isHilltop(intact, { row: 3, col: 3 })).toBe(true)
    expect(isHilltop(intact, { row: 3, col: 6 })).toBe(true)
    expect(isHilltop(intact, { row: 0, col: 0 })).toBe(false)
    expect(isHilltop(intact, { row: 3, col: 1 })).toBe(false)
  })

  it('puts a hilltop token over every razed castle', () => {
    const razed = board([
      [false, true, true],
      [true, true, false],
    ])
    expect(isHilltop(razed, { row: 0, col: 0 })).toBe(true)
    expect(isHilltop(razed, { row: 6, col: 6 })).toBe(true)
    expect(isHilltop(razed, { row: 0, col: 3 })).toBe(false)
  })

  it('finds a castle slot on each back-row corner and middle', () => {
    expect(castleSlotAt({ row: 0, col: 3 })).toEqual({ owner: 0, index: 1 })
    expect(castleSlotAt({ row: 6, col: 6 })).toEqual({ owner: 1, index: 2 })
    expect(castleSlotAt({ row: 0, col: 1 })).toBeNull()
    expect(castleSlotAt({ row: 3, col: 3 })).toBeNull()
  })
})
