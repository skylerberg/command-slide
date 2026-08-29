// Mirrors the serde output of `command-slide-core`. The shapes here are pinned
// on the Rust side by `command-slide-core/tests/wire_format.rs`; change one and
// change the other.

export const BOARD_SIZE = 7
export const CASTLE_COLS = [0, 3, 6]
export const BACK_ROW = [0, 6]
export const HILLTOP_ROW = 3
export const HILLTOP_COLS = [0, 3, 6]

export type PieceKind =
  | 'swordsman'
  | 'flail'
  | 'spearman'
  | 'archer'
  | 'trebuchet'
  | 'batteringRam'

export interface Piece {
  kind: PieceKind
  owner: number
}

export interface Square {
  row: number
  col: number
}

export type TokenKind = 'row' | 'column'
export type TokenFace = 'movement' | 'attack'

export interface Token {
  line: number
  face: TokenFace
}

export type Phase = 'slide' | 'order' | 'activate' | 'attack' | 'gameOver'

export type Choice =
  | { type: 'slide'; token: TokenKind; line: number }
  | { type: 'order'; first: TokenKind }
  | { type: 'move'; from: Square; to: Square }
  | { type: 'pass' }
  | { type: 'attack'; from: Square; target: Square }
  | { type: 'holdFire'; from: Square }

/** What one shot destroyed. A shot destroys exactly one thing. */
export type Casualty = { type: 'piece'; piece: Piece } | { type: 'castle' }

export type Outcome = { type: 'winner'; player: number } | { type: 'draw' }

export interface GameState {
  board: (Piece | null)[][]
  castles: boolean[][]
  /** Indexed by player, then by `tokenIndex` — 0 is the row token. */
  tokens: Token[][]
  currentPlayer: number
  phase: Phase
  pending: TokenKind[]
  pendingLen: number
  /** How far along its line a volley has got. Only meaningful in `attack`. */
  attackIndex: number
  turn: number
  outcome: Outcome | null
}

export type GameEvent =
  | { type: 'slid'; player: number; token: TokenKind; from: number; to: number }
  | {
      type: 'moved'
      player: number
      token: TokenKind
      kind: PieceKind
      from: Square
      to: Square
    }
  | { type: 'passed'; player: number; token: TokenKind }
  | { type: 'volley'; player: number; token: TokenKind; line: number }
  | {
      type: 'struck'
      player: number
      token: TokenKind
      kind: PieceKind
      from: Square
      target: Square
      casualty: Casualty
    }
  | {
      type: 'heldFire'
      player: number
      token: TokenKind
      kind: PieceKind
      from: Square
    }
  | { type: 'turnEnded'; player: number }
  | { type: 'gameOver'; outcome: Outcome }

/** A volley's reach: each attacker takes one of these, not all of them. */
export interface AttackPreview {
  attackers: Square[]
  threatenedPieces: Square[]
  threatenedCastles: Square[]
}

export const TOKEN_KINDS: TokenKind[] = ['row', 'column']

export function tokenIndex(kind: TokenKind): number {
  return kind === 'row' ? 0 : 1
}

export function otherToken(kind: TokenKind): TokenKind {
  return kind === 'row' ? 'column' : 'row'
}

export function tokenOf(state: GameState, player: number, kind: TokenKind): Token {
  return state.tokens[player][tokenIndex(kind)]
}

export function opponent(player: number): number {
  return 1 - player
}

export function sameSquare(a: Square, b: Square): boolean {
  return a.row === b.row && a.col === b.col
}

export function squareKey(square: Square): string {
  return `${square.row},${square.col}`
}

export function pieceAt(state: GameState, square: Square): Piece | null {
  return state.board[square.row][square.col]
}

/** Tokens still waiting to activate this turn, in the order they will act. */
export function pendingTokens(state: GameState): TokenKind[] {
  return state.pending.slice(0, state.pendingLen)
}

/** The single token this player will attack with, if one is armed. */
export function armedToken(state: GameState, player: number): TokenKind | null {
  return TOKEN_KINDS.find((kind) => tokenOf(state, player, kind).face === 'attack') ?? null
}

/** One of the printed three, or a razed castle with a hilltop token over it. */
export function isHilltop(state: GameState, square: Square): boolean {
  if (square.row === HILLTOP_ROW && HILLTOP_COLS.includes(square.col)) return true
  const slot = castleSlotAt(square)
  return slot !== null && !state.castles[slot.owner][slot.index]
}

/** The castle slot on `square`, whether or not that castle still stands. */
export function castleSlotAt(square: Square): { owner: number; index: number } | null {
  const owner = BACK_ROW.indexOf(square.row)
  const index = CASTLE_COLS.indexOf(square.col)
  if (owner < 0 || index < 0) return null
  return { owner, index }
}

export function castleStands(state: GameState, square: Square): boolean {
  const slot = castleSlotAt(square)
  return slot !== null && state.castles[slot.owner][slot.index]
}

export function castlesStanding(state: GameState, player: number): number {
  return state.castles[player].filter(Boolean).length
}

export function siegeEngines(state: GameState, player: number): PieceKind[] {
  const kinds: PieceKind[] = []
  for (const row of state.board) {
    for (const piece of row) {
      if (piece && piece.owner === player && isSiege(piece.kind)) kinds.push(piece.kind)
    }
  }
  return kinds
}

export function isSiege(kind: PieceKind): boolean {
  return kind === 'trebuchet' || kind === 'batteringRam'
}

const ATTACK_OFFSETS: Record<PieceKind, [number, number][]> = {
  swordsman: [
    [-1, 0],
    [1, 0],
    [0, -1],
    [0, 1],
  ],
  flail: [
    [-1, -1],
    [-1, 1],
    [1, -1],
    [1, 1],
  ],
  spearman: [
    [-2, 0],
    [2, 0],
    [0, -2],
    [0, 2],
  ],
  archer: [
    [-2, -2],
    [-2, 2],
    [2, -2],
    [2, 2],
  ],
  trebuchet: [
    [-3, 0],
    [3, 0],
    [0, -3],
    [0, 3],
    [-3, -3],
    [-3, 3],
    [3, -3],
    [3, 3],
  ],
  batteringRam: [
    [-1, 0],
    [1, 0],
    [0, -1],
    [0, 1],
  ],
}

export function attackOffsets(kind: PieceKind): [number, number][] {
  return ATTACK_OFFSETS[kind]
}

/** Every on-board square a piece of `kind` on `from` strikes. */
export function attackTargets(from: Square, kind: PieceKind): Square[] {
  return attackOffsets(kind)
    .map(([drow, dcol]) => ({ row: from.row + drow, col: from.col + dcol }))
    .filter(
      (square) =>
        square.row >= 0 && square.row < BOARD_SIZE && square.col >= 0 && square.col < BOARD_SIZE,
    )
}

export const PIECE_NAMES: Record<PieceKind, string> = {
  swordsman: 'Swordsman',
  flail: 'Flail',
  spearman: 'Spearman',
  archer: 'Archer',
  trebuchet: 'Trebuchet',
  batteringRam: 'Battering Ram',
}

export const PLAYER_NAMES = ['Ivory', 'Umber']

export function lineOf(kind: TokenKind, square: Square): number {
  return kind === 'row' ? square.row : square.col
}

export function squaresOnLine(kind: TokenKind, line: number): Square[] {
  return Array.from({ length: BOARD_SIZE }, (_, i) =>
    kind === 'row' ? { row: line, col: i } : { row: i, col: line },
  )
}
