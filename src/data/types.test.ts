import { describe, expect, it } from 'vitest'
import {
  attackTargets,
  castleSlotAt,
  isHilltop,
  squareKey,
  squaresOnLine,
  wallSlotAt,
  wallStands,
} from './types'
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
    // Nine files, so col 8 is the last one and col 7 keeps both diagonals.
    expect(attackTargets({ row: 3, col: 8 }, 'archer').map(squareKey).sort()).toEqual(
      ['1,6', '5,6'].sort(),
    )
  })

  it('gives the centre trebuchet a line on every castle', () => {
    const castles = attackTargets({ row: 3, col: 4 }, 'trebuchet')
      .filter((square) => castleSlotAt(square) !== null)
      .map(squareKey)
      .sort()
    expect(castles).toEqual(['0,1', '0,4', '0,7', '6,1', '6,4', '6,7'])
  })
})

describe('lines', () => {
  it('runs a rank across nine files and a file down seven ranks', () => {
    expect(squaresOnLine('row', 3)).toHaveLength(9)
    expect(squaresOnLine('column', 3)).toHaveLength(7)
  })
})

describe('terrain', () => {
  it('puts the printed hilltops on the middle row only', () => {
    const intact = board([
      [true, true, true],
      [true, true, true],
    ])
    expect(isHilltop(intact, { row: 3, col: 1 })).toBe(true)
    expect(isHilltop(intact, { row: 3, col: 4 })).toBe(true)
    expect(isHilltop(intact, { row: 3, col: 7 })).toBe(true)
    expect(isHilltop(intact, { row: 0, col: 1 })).toBe(false)
    expect(isHilltop(intact, { row: 3, col: 2 })).toBe(false)
  })

  it('puts a hilltop token over every razed castle', () => {
    const razed = board([
      [false, true, true],
      [true, true, false],
    ])
    expect(isHilltop(razed, { row: 0, col: 1 })).toBe(true)
    expect(isHilltop(razed, { row: 6, col: 7 })).toBe(true)
    expect(isHilltop(razed, { row: 0, col: 4 })).toBe(false)
  })

  it('finds a castle slot on the three back-row castle files', () => {
    expect(castleSlotAt({ row: 0, col: 4 })).toEqual({ owner: 0, index: 1 })
    expect(castleSlotAt({ row: 6, col: 7 })).toEqual({ owner: 1, index: 2 })
    expect(castleSlotAt({ row: 0, col: 2 })).toBeNull()
    expect(castleSlotAt({ row: 0, col: 0 })).toBeNull()
    expect(castleSlotAt({ row: 3, col: 4 })).toBeNull()
  })

  it('fills the middle row between the hilltops with walls', () => {
    expect(wallSlotAt({ row: 3, col: 2 })).toBe(0)
    expect(wallSlotAt({ row: 3, col: 6 })).toBe(3)
    expect(wallSlotAt({ row: 3, col: 4 })).toBeNull()
    expect(wallSlotAt({ row: 2, col: 2 })).toBeNull()

    const standing = { walls: [true, false, true, true] } as GameState
    expect(wallStands(standing, { row: 3, col: 2 })).toBe(true)
    expect(wallStands(standing, { row: 3, col: 3 })).toBe(false)
    expect(wallStands(standing, { row: 3, col: 4 })).toBe(false)
  })
})
