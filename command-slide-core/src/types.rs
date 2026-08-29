//! Board geometry, pieces, command tokens, and the game state they live in.
//!
//! `GameState` is `Copy` on purpose: the search clones one per determinization
//! and per rollout step, and a memcpy of ~130 bytes is cheaper than anything
//! that chases a pointer. That is why the board is a fixed array and the
//! activation queue is an array plus a length rather than a `Vec`.

use serde::{Deserialize, Serialize};

pub const BOARD_SIZE: usize = 7;
pub const NUM_PLAYERS: usize = 2;
pub const CASTLES_PER_PLAYER: usize = 3;

/// Back row for each player: where their castles stand.
pub const BACK_ROW: [u8; NUM_PLAYERS] = [0, 6];

/// Columns carrying a castle on a player's back row.
pub const CASTLE_COLS: [u8; CASTLES_PER_PLAYER] = [0, 3, 6];

/// The middle row, and the columns on it that are hilltops. Castles and
/// hilltops share columns, which is what puts every castle exactly three
/// squares from a hilltop — straight for the near ones, diagonal for the rest.
pub const HILLTOP_ROW: u8 = 3;
pub const HILLTOP_COLS: [u8; 3] = [0, 3, 6];

/// Turns each side may take before the game is called a draw. Nothing in the
/// rules forces progress, and a search needs every line to terminate.
pub const MAX_TURNS: u32 = 300;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PieceKind {
    Swordsman,
    Flail,
    Spearman,
    Archer,
    Trebuchet,
    BatteringRam,
}

const SWORDSMAN_ATTACKS: [(i8, i8); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
const FLAIL_ATTACKS: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];
const SPEARMAN_ATTACKS: [(i8, i8); 4] = [(-2, 0), (2, 0), (0, -2), (0, 2)];
const ARCHER_ATTACKS: [(i8, i8); 4] = [(-2, -2), (-2, 2), (2, -2), (2, 2)];
const TREBUCHET_ATTACKS: [(i8, i8); 8] = [
    (-3, 0),
    (3, 0),
    (0, -3),
    (0, 3),
    (-3, -3),
    (-3, 3),
    (3, -3),
    (3, 3),
];
const RAM_ATTACKS: [(i8, i8); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

impl PieceKind {
    /// Squares this piece strikes, relative to where it stands. Nothing blocks
    /// an attack: a spearman reaches over the square between it and its target.
    pub fn attack_offsets(self) -> &'static [(i8, i8)] {
        match self {
            PieceKind::Swordsman => &SWORDSMAN_ATTACKS,
            PieceKind::Flail => &FLAIL_ATTACKS,
            PieceKind::Spearman => &SPEARMAN_ATTACKS,
            PieceKind::Archer => &ARCHER_ATTACKS,
            PieceKind::Trebuchet => &TREBUCHET_ATTACKS,
            PieceKind::BatteringRam => &RAM_ATTACKS,
        }
    }

    /// Siege engines damage castles and nothing else; every other piece damages
    /// pieces and nothing else. No piece does both.
    pub fn is_siege(self) -> bool {
        matches!(self, PieceKind::Trebuchet | PieceKind::BatteringRam)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Piece {
    pub kind: PieceKind,
    pub owner: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Square {
    pub row: u8,
    pub col: u8,
}

impl Square {
    pub const fn new(row: u8, col: u8) -> Self {
        Self { row, col }
    }

    /// The square `(drow, dcol)` away, or `None` if that falls off the board.
    pub fn offset(self, drow: i8, dcol: i8) -> Option<Square> {
        let row = self.row as i8 + drow;
        let col = self.col as i8 + dcol;
        let in_range = |v: i8| (0..BOARD_SIZE as i8).contains(&v);
        (in_range(row) && in_range(col)).then(|| Square::new(row as u8, col as u8))
    }

    pub fn index(self) -> usize {
        self.row as usize * BOARD_SIZE + self.col as usize
    }

    pub fn chebyshev(self, other: Square) -> u8 {
        let drow = self.row.abs_diff(other.row);
        let dcol = self.col.abs_diff(other.col);
        drow.max(dcol)
    }
}

/// Which of a player's two command tokens.
///
/// The `Row` token rides the left or right edge, names a row, and its movement
/// face moves a piece from that row like a rook. The `Column` token rides the
/// top or bottom edge, names a column, and its movement face moves a piece from
/// that column like a bishop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TokenKind {
    Row,
    Column,
}

pub const TOKEN_KINDS: [TokenKind; 2] = [TokenKind::Row, TokenKind::Column];

impl TokenKind {
    pub fn index(self) -> usize {
        match self {
            TokenKind::Row => 0,
            TokenKind::Column => 1,
        }
    }

    pub fn other(self) -> TokenKind {
        match self {
            TokenKind::Row => TokenKind::Column,
            TokenKind::Column => TokenKind::Row,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TokenFace {
    Movement,
    Attack,
}

impl TokenFace {
    pub fn flipped(self) -> TokenFace {
        match self {
            TokenFace::Movement => TokenFace::Attack,
            TokenFace::Attack => TokenFace::Movement,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Token {
    /// Row index for a `Row` token, column index for a `Column` token.
    pub line: u8,
    pub face: TokenFace,
}

/// Where a turn is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    /// Slide a command token to a new line.
    Slide,
    /// Choose which of the two tokens activates first.
    Order,
    /// Spend the movement face at the head of the queue.
    Activate,
    /// Aim one attacker of a volley. Each piece on the line fires in turn, so
    /// an attack face costs as many decisions as it has pieces with a shot.
    Attack,
    GameOver,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Choice {
    /// Slide `token` to `line`. On a player's first turn this also picks which
    /// of the two tokens is the one that acts.
    Slide { token: TokenKind, line: u8 },
    /// Activate `first` before the other token.
    Order { first: TokenKind },
    /// Spend a movement activation moving a piece.
    Move { from: Square, to: Square },
    /// Spend a movement activation without moving.
    Pass,
    /// Fire the attacker on `from` at `target`. A piece commanded to attack
    /// strikes one square, not everything its pattern covers.
    Attack { from: Square, target: Square },
    /// Leave the attacker on `from` unfired and move on down the line.
    HoldFire { from: Square },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Outcome {
    Winner { player: u8 },
    Draw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GameState {
    pub board: [[Option<Piece>; BOARD_SIZE]; BOARD_SIZE],
    /// `castles[player][i]` is whether the castle on `CASTLE_COLS[i]` still
    /// stands. Castles are terrain, not pieces: they never move, never attack,
    /// and a piece may stand on one.
    pub castles: [[bool; CASTLES_PER_PLAYER]; NUM_PLAYERS],
    /// Indexed by player, then by `TokenKind::index`.
    pub tokens: [[Token; 2]; NUM_PLAYERS],
    pub current_player: u8,
    pub phase: Phase,
    /// Tokens still to activate this turn, head first. Only the first
    /// `pending_len` entries are meaningful.
    pub pending: [TokenKind; 2],
    pub pending_len: u8,
    /// Under `Phase::Attack`, how far along the token's line the volley has
    /// got: the index of the piece now choosing its target. Meaningless in
    /// every other phase.
    pub attack_index: u8,
    /// Full turns taken by both sides so far.
    pub turn: u32,
    pub outcome: Option<Outcome>,
}

impl GameState {
    pub fn piece_at(&self, square: Square) -> Option<Piece> {
        self.board[square.row as usize][square.col as usize]
    }

    pub fn set_piece(&mut self, square: Square, piece: Option<Piece>) {
        self.board[square.row as usize][square.col as usize] = piece;
    }

    pub fn token(&self, player: u8, kind: TokenKind) -> Token {
        self.tokens[player as usize][kind.index()]
    }

    pub fn token_mut(&mut self, player: u8, kind: TokenKind) -> &mut Token {
        &mut self.tokens[player as usize][kind.index()]
    }

    pub fn opponent(player: u8) -> u8 {
        1 - player
    }

    pub fn squares() -> impl Iterator<Item = Square> {
        (0..BOARD_SIZE as u8)
            .flat_map(|row| (0..BOARD_SIZE as u8).map(move |col| Square::new(row, col)))
    }

    /// The squares of `line`, which is a row for a `Row` token and a column for
    /// a `Column` token.
    pub fn line_squares(kind: TokenKind, line: u8) -> impl Iterator<Item = Square> {
        (0..BOARD_SIZE as u8).map(move |index| Self::line_square(kind, line, index))
    }

    /// The `index`th square of `line`, counting along the line.
    pub fn line_square(kind: TokenKind, line: u8, index: u8) -> Square {
        match kind {
            TokenKind::Row => Square::new(line, index),
            TokenKind::Column => Square::new(index, line),
        }
    }

    /// One of the three hilltops printed on the board.
    pub fn is_natural_hilltop(square: Square) -> bool {
        square.row == HILLTOP_ROW && HILLTOP_COLS.contains(&square.col)
    }

    /// A hilltop as the board stands: one of the printed three, or a castle
    /// that has been destroyed and taken a hilltop token in its place.
    pub fn is_hilltop(&self, square: Square) -> bool {
        Self::is_natural_hilltop(square) || self.razed_castle_at(square)
    }

    fn razed_castle_at(&self, square: Square) -> bool {
        matches!(
            Self::castle_slot_at(square),
            Some((owner, index)) if !self.castles[owner as usize][index]
        )
    }

    /// Who owns the castle still standing on `square`, if one does.
    pub fn standing_castle_at(&self, square: Square) -> Option<u8> {
        match Self::castle_slot_at(square) {
            Some((owner, index)) if self.castles[owner as usize][index] => Some(owner),
            _ => None,
        }
    }

    /// Whether a standing enemy castle sits on `square`. A piece may neither
    /// enter one nor slide through it.
    pub fn blocks_slide(&self, square: Square, player: u8) -> bool {
        self.standing_castle_at(square)
            .is_some_and(|owner| owner != player)
    }

    /// The castle slot `square` belongs to, if any, as `(owner, index)`.
    /// Reports the slot whether or not that castle still stands.
    pub fn castle_slot_at(square: Square) -> Option<(u8, usize)> {
        let owner = BACK_ROW.iter().position(|&r| r == square.row)? as u8;
        let index = CASTLE_COLS.iter().position(|&c| c == square.col)?;
        Some((owner, index))
    }

    pub fn castle_square(owner: u8, index: usize) -> Square {
        Square::new(BACK_ROW[owner as usize], CASTLE_COLS[index])
    }

    pub fn castles_standing(&self, player: u8) -> usize {
        self.castles[player as usize].iter().filter(|&&c| c).count()
    }

    pub fn pieces(&self) -> impl Iterator<Item = (Square, Piece)> + '_ {
        Self::squares().filter_map(|square| self.piece_at(square).map(|piece| (square, piece)))
    }

    pub fn pieces_of(&self, player: u8) -> impl Iterator<Item = (Square, Piece)> + '_ {
        self.pieces().filter(move |(_, piece)| piece.owner == player)
    }

    pub fn siege_engines(&self, player: u8) -> usize {
        self.pieces_of(player)
            .filter(|(_, piece)| piece.kind.is_siege())
            .count()
    }

    /// True on a player's very first turn, which is the one turn where both
    /// tokens still show their movement face. Every later turn flips both, so
    /// the pair stays opposite forever after.
    pub fn is_first_turn(&self) -> bool {
        TOKEN_KINDS
            .iter()
            .all(|&kind| self.token(self.current_player, kind).face == TokenFace::Movement)
    }

    pub fn pending_tokens(&self) -> &[TokenKind] {
        &self.pending[..self.pending_len as usize]
    }
}
