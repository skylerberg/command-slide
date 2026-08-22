//! Setup, legal-choice generation, and turn resolution.
//!
//! A turn is broken into the decisions a player actually makes — slide, order,
//! move — rather than handed to the search as one compound action. Enumerating
//! whole turns would mean a branching factor in the hundreds at every node;
//! this way each node branches by at most a few dozen and the search still sees
//! the same set of reachable positions.

use crate::types::*;

/// Directions a rook slides. The `Row` token's movement face uses these.
const ROOK_DIRS: [(i8, i8); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];
/// Directions a bishop slides. The `Column` token's movement face uses these.
const BISHOP_DIRS: [(i8, i8); 4] = [(-1, -1), (-1, 1), (1, -1), (1, 1)];

/// What a piece did, for the UI log and for replays. The search never builds
/// one: `apply` takes a sink that compiles away to nothing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum GameEvent {
    Slid {
        player: u8,
        token: TokenKind,
        from: u8,
        to: u8,
    },
    Moved {
        player: u8,
        token: TokenKind,
        kind: PieceKind,
        from: Square,
        to: Square,
    },
    /// A movement activation with no legal move behind it.
    Passed {
        player: u8,
        token: TokenKind,
    },
    Attacked {
        player: u8,
        token: TokenKind,
        line: u8,
        attackers: Vec<Square>,
        destroyed_pieces: Vec<(Square, Piece)>,
        destroyed_castles: Vec<Square>,
    },
    TurnEnded {
        player: u8,
    },
    GameOver {
        outcome: Outcome,
    },
}

/// Somewhere for `apply` to put events. `()` discards them without ever
/// building one, which is what keeps the search allocation-free.
pub trait EventSink {
    fn emit(&mut self, make: impl FnOnce() -> GameEvent);
}

impl EventSink for () {
    fn emit(&mut self, _make: impl FnOnce() -> GameEvent) {}
}

impl EventSink for Vec<GameEvent> {
    fn emit(&mut self, make: impl FnOnce() -> GameEvent) {
        self.push(make());
    }
}

pub fn initial_state() -> GameState {
    let mut board = [[None; BOARD_SIZE]; BOARD_SIZE];

    // Back row: spear, bow, trebuchet, bow, spear across the middle five
    // columns. The corner columns are castle squares and start empty; the
    // trebuchet starts on top of the middle castle.
    let back = [
        PieceKind::Spearman,
        PieceKind::Archer,
        PieceKind::Trebuchet,
        PieceKind::Archer,
        PieceKind::Spearman,
    ];
    let front = [
        PieceKind::Swordsman,
        PieceKind::Flail,
        PieceKind::BatteringRam,
        PieceKind::Flail,
        PieceKind::Swordsman,
    ];

    for player in 0..NUM_PLAYERS as u8 {
        let back_row = BACK_ROW[player as usize];
        let front_row = if player == 0 { back_row + 1 } else { back_row - 1 };
        for (i, (&back_kind, &front_kind)) in back.iter().zip(front.iter()).enumerate() {
            let col = i as u8 + 1;
            board[back_row as usize][col as usize] = Some(Piece {
                kind: back_kind,
                owner: player,
            });
            board[front_row as usize][col as usize] = Some(Piece {
                kind: front_kind,
                owner: player,
            });
        }
    }

    // Light rides the left and top edges, Dark the right and bottom, each
    // token starting beside its owner's back row and outer column.
    let start_line = [0u8, 6u8];
    let tokens = std::array::from_fn(|player| {
        [
            Token {
                line: start_line[player],
                face: TokenFace::Movement,
            },
            Token {
                line: start_line[player],
                face: TokenFace::Movement,
            },
        ]
    });

    GameState {
        board,
        castles: [[true; CASTLES_PER_PLAYER]; NUM_PLAYERS],
        tokens,
        current_player: 0,
        phase: Phase::Slide,
        pending: [TokenKind::Row, TokenKind::Column],
        pending_len: 0,
        turn: 0,
        outcome: None,
    }
}

/// Whether `line` holds a piece belonging to `player`. Castles do not count:
/// they are terrain, so a back row emptied of pieces is not a legal destination
/// even while its castles stand.
fn line_has_friendly(state: &GameState, player: u8, kind: TokenKind, line: u8) -> bool {
    GameState::line_squares(kind, line)
        .filter_map(|square| state.piece_at(square))
        .any(|piece| piece.owner == player)
}

/// Tokens the current player may slide: both on their first turn, otherwise
/// just the one showing its movement face.
fn slidable_tokens(state: &GameState) -> Vec<TokenKind> {
    TOKEN_KINDS
        .iter()
        .copied()
        .filter(|&kind| state.token(state.current_player, kind).face == TokenFace::Movement)
        .collect()
}

/// Destinations for a piece under the movement face of `kind`: rook lines for
/// the row token, bishop lines for the column token. Movement never captures,
/// so a slide stops before any occupied square. Castles are terrain and do not
/// block.
fn slide_destinations_into(state: &GameState, from: Square, kind: TokenKind, out: &mut Vec<Choice>) {
    let dirs = match kind {
        TokenKind::Row => &ROOK_DIRS,
        TokenKind::Column => &BISHOP_DIRS,
    };
    for &(drow, dcol) in dirs {
        let mut square = from;
        while let Some(next) = square.offset(drow, dcol) {
            if state.piece_at(next).is_some() {
                break;
            }
            out.push(Choice::Move { from, to: next });
            square = next;
        }
    }
}

fn movement_choices_into(state: &GameState, kind: TokenKind, out: &mut Vec<Choice>) {
    let player = state.current_player;
    let line = state.token(player, kind).line;
    for square in GameState::line_squares(kind, line) {
        match state.piece_at(square) {
            Some(piece) if piece.owner == player => {
                slide_destinations_into(state, square, kind, out)
            }
            _ => {}
        }
    }
}

pub fn legal_choices_into(state: &GameState, out: &mut Vec<Choice>) {
    out.clear();
    match state.phase {
        Phase::Slide => {
            let player = state.current_player;
            let slidable = slidable_tokens(state);
            for &kind in &slidable {
                let current = state.token(player, kind).line;
                for line in 0..BOARD_SIZE as u8 {
                    if line != current && line_has_friendly(state, player, kind, line) {
                        out.push(Choice::Slide { token: kind, line });
                    }
                }
            }
            // A player whose every piece sits on the token's current line has
            // nowhere legal to slide. Rather than deadlock, the token holds
            // its line; the turn otherwise proceeds normally.
            if out.is_empty() {
                for &kind in &slidable {
                    out.push(Choice::Slide {
                        token: kind,
                        line: state.token(player, kind).line,
                    });
                }
            }
        }
        Phase::Order => {
            out.push(Choice::Order {
                first: TokenKind::Row,
            });
            out.push(Choice::Order {
                first: TokenKind::Column,
            });
        }
        Phase::Activate => {
            // Attack activations carry no decision and are already resolved by
            // `settle`, so the head of the queue always shows a movement face.
            let kind = state.pending[0];
            movement_choices_into(state, kind, out);
            if out.is_empty() {
                out.push(Choice::Pass);
            }
        }
        Phase::GameOver => {}
    }
}

pub fn legal_choices(state: &GameState) -> Vec<Choice> {
    let mut out = Vec::new();
    legal_choices_into(state, &mut out);
    out
}

/// Everything one attack activation destroys, gathered before anything is
/// removed. Every attacker belongs to the attacking player and every casualty
/// to their opponent, so resolution order cannot matter — but gathering first
/// makes that fact structural rather than incidental.
#[derive(Default)]
pub struct AttackResult {
    /// Friendly pieces in the line that struck, as a square bitboard.
    pub attackers: u64,
    /// Enemy pieces struck, as a square bitboard.
    pub pieces: u64,
    /// Enemy castles struck, indexed by slot.
    pub castles: [bool; CASTLES_PER_PLAYER],
}

impl AttackResult {
    pub fn squares(bits: u64) -> impl Iterator<Item = Square> {
        GameState::squares().filter(move |square| bits >> square.index() & 1 == 1)
    }

    pub fn is_empty(&self) -> bool {
        self.pieces == 0 && !self.castles.iter().any(|&c| c)
    }
}

/// What an attack with `player`'s `kind` token, at its current line, would
/// destroy. Pure: the UI calls it to preview a threat.
pub fn attack_preview(state: &GameState, player: u8, kind: TokenKind) -> AttackResult {
    let line = state.token(player, kind).line;
    let enemy = GameState::opponent(player);
    let mut result = AttackResult::default();

    for square in GameState::line_squares(kind, line) {
        let Some(piece) = state.piece_at(square) else {
            continue;
        };
        if piece.owner != player {
            continue;
        }
        // A trebuchet is a siege weapon only while it stands on a hilltop.
        // Off one it still occupies a square and still dies like anything else,
        // but it throws nothing.
        if piece.kind == PieceKind::Trebuchet && !GameState::is_hilltop(square) {
            continue;
        }
        result.attackers |= 1 << square.index();

        for &(drow, dcol) in piece.kind.attack_offsets() {
            let Some(target) = square.offset(drow, dcol) else {
                continue;
            };
            if piece.kind.is_siege() {
                if let Some((owner, index)) = GameState::castle_slot_at(target) {
                    if owner == enemy && state.castles[owner as usize][index] {
                        result.castles[index] = true;
                    }
                }
            } else if let Some(victim) = state.piece_at(target) {
                if victim.owner == enemy {
                    result.pieces |= 1 << target.index();
                }
            }
        }
    }

    result
}

fn resolve_attack(state: &mut GameState, kind: TokenKind, sink: &mut impl EventSink) {
    let player = state.current_player;
    let enemy = GameState::opponent(player);
    let line = state.token(player, kind).line;
    let result = attack_preview(state, player, kind);

    let destroyed_pieces: Vec<(Square, Piece)> = AttackResult::squares(result.pieces)
        .filter_map(|square| state.piece_at(square).map(|piece| (square, piece)))
        .collect();
    for &(square, _) in &destroyed_pieces {
        state.set_piece(square, None);
    }

    let mut destroyed_castles = Vec::new();
    for (index, &hit) in result.castles.iter().enumerate() {
        if hit {
            state.castles[enemy as usize][index] = false;
            destroyed_castles.push(GameState::castle_square(enemy, index));
        }
    }

    sink.emit(|| GameEvent::Attacked {
        player,
        token: kind,
        line,
        attackers: AttackResult::squares(result.attackers).collect(),
        destroyed_pieces,
        destroyed_castles,
    });
}

/// Both win conditions, checked against both players. Only the side that was
/// just attacked can have lost — attacks never touch the attacker's own
/// material — but checking both keeps the rule in one place.
fn check_outcome(state: &mut GameState) {
    if state.outcome.is_some() {
        return;
    }
    for player in 0..NUM_PLAYERS as u8 {
        let razed = state.castles_standing(player) == 0;
        let disarmed = state.siege_engines(player) == 0;
        if razed || disarmed {
            state.outcome = Some(Outcome::Winner {
                player: GameState::opponent(player),
            });
            return;
        }
    }
}

fn pop_pending(state: &mut GameState) {
    state.pending[0] = state.pending[1];
    state.pending_len -= 1;
}

fn end_turn(state: &mut GameState, sink: &mut impl EventSink) {
    let player = state.current_player;
    sink.emit(|| GameEvent::TurnEnded { player });
    state.current_player = GameState::opponent(player);
    state.pending_len = 0;
    state.phase = Phase::Slide;
    if state.current_player == 0 {
        state.turn += 1;
        if state.turn >= MAX_TURNS {
            state.outcome = Some(Outcome::Draw);
        }
    }
}

/// Run the game forward past everything that carries no decision, so that
/// `status` and `legal_choices` are only ever asked about a real choice.
fn settle(state: &mut GameState, sink: &mut impl EventSink) {
    loop {
        if let Some(outcome) = state.outcome {
            if state.phase != Phase::GameOver {
                state.phase = Phase::GameOver;
                sink.emit(|| GameEvent::GameOver { outcome });
            }
            return;
        }
        if state.phase != Phase::Activate {
            return;
        }
        if state.pending_len == 0 {
            end_turn(state, sink);
            continue;
        }
        let kind = state.pending[0];
        if state.token(state.current_player, kind).face == TokenFace::Movement {
            return;
        }
        resolve_attack(state, kind, sink);
        let token = state.token_mut(state.current_player, kind);
        token.face = token.face.flipped();
        pop_pending(state);
        check_outcome(state);
    }
}

/// Play `choice` and run on to the next real decision.
///
/// Panics in debug on an illegal choice; in release an illegal choice quietly
/// produces a nonsense position, so callers enumerate first.
pub fn apply_with<S: EventSink>(state: &mut GameState, choice: &Choice, sink: &mut S) {
    debug_assert!(
        legal_choices(state).contains(choice),
        "command-slide: {choice:?} is not legal in {:?}",
        state.phase
    );
    let player = state.current_player;

    match *choice {
        Choice::Slide { token, line } => {
            let first_turn = state.is_first_turn();
            let from = state.token(player, token).line;
            state.token_mut(player, token).line = line;
            sink.emit(|| GameEvent::Slid {
                player,
                token,
                from,
                to: line,
            });

            if first_turn {
                // Only the token that was slid acts; the other stays where it
                // is, still showing its movement face, and is slid next turn.
                state.pending = [token, token.other()];
                state.pending_len = 1;
                state.phase = Phase::Activate;
            } else {
                state.pending = [token, token.other()];
                state.pending_len = 2;
                state.phase = Phase::Order;
            }
        }
        Choice::Order { first } => {
            state.pending = [first, first.other()];
            state.pending_len = 2;
            state.phase = Phase::Activate;
        }
        Choice::Move { from, to } => {
            let token = state.pending[0];
            let piece = state.piece_at(from).expect("moved a piece that is not there");
            state.set_piece(from, None);
            state.set_piece(to, Some(piece));
            sink.emit(|| GameEvent::Moved {
                player,
                token,
                kind: piece.kind,
                from,
                to,
            });
            let slot = state.token_mut(player, token);
            slot.face = slot.face.flipped();
            pop_pending(state);
        }
        Choice::Pass => {
            let token = state.pending[0];
            sink.emit(|| GameEvent::Passed { player, token });
            let slot = state.token_mut(player, token);
            slot.face = slot.face.flipped();
            pop_pending(state);
        }
    }

    settle(state, sink);
}

pub fn apply(state: &mut GameState, choice: &Choice) {
    apply_with(state, choice, &mut ());
}

/// Run a state forward past everything that carries no decision.
///
/// States produced by [`apply`] are already settled. A state assembled by hand
/// — a test position, a puzzle, a saved position from an older build — is not,
/// and must be settled before it is asked for a legal choice: `Phase::Activate`
/// assumes the head of the queue shows a movement face, because the attack that
/// would otherwise be there has already been resolved.
pub fn settle_state(state: &mut GameState) -> Vec<GameEvent> {
    let mut events = Vec::new();
    settle(state, &mut events);
    events
}

pub fn apply_logged(state: &mut GameState, choice: &Choice) -> Vec<GameEvent> {
    let mut events = Vec::new();
    apply_with(state, choice, &mut events);
    events
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The opening position with every piece swept off, so a test can state
    /// exactly the position it means. Castles and tokens are left as dealt.
    fn empty_board() -> GameState {
        let mut state = initial_state();
        state.board = [[None; BOARD_SIZE]; BOARD_SIZE];
        state
    }

    fn place(state: &mut GameState, row: u8, col: u8, kind: PieceKind, owner: u8) {
        state.set_piece(Square::new(row, col), Some(Piece { kind, owner }));
    }

    /// Point `player`'s token at `line` with its attack face up, and queue it
    /// as the only activation left this turn.
    fn arm_attack(state: &mut GameState, player: u8, kind: TokenKind, line: u8) {
        state.current_player = player;
        *state.token_mut(player, kind) = Token {
            line,
            face: TokenFace::Attack,
        };
        state.pending = [kind, kind.other()];
        state.pending_len = 1;
        state.phase = Phase::Activate;
    }

    fn destroyed_by_attack(state: &GameState, player: u8, kind: TokenKind) -> (Vec<Square>, Vec<Square>) {
        let result = attack_preview(state, player, kind);
        let enemy = GameState::opponent(player);
        let pieces = AttackResult::squares(result.pieces).collect();
        let castles = result
            .castles
            .iter()
            .enumerate()
            .filter(|(_, &hit)| hit)
            .map(|(index, _)| GameState::castle_square(enemy, index))
            .collect();
        (pieces, castles)
    }

    #[test]
    fn opening_position_matches_the_printed_board() {
        let state = initial_state();

        // Ten pieces a side: two each of spear, bow, sword and flail, plus the
        // two siege engines.
        for player in 0..NUM_PLAYERS as u8 {
            assert_eq!(state.pieces_of(player).count(), 10);
            assert_eq!(state.siege_engines(player), 2);
            assert_eq!(state.castles_standing(player), 3);
        }

        let back = [
            (1, PieceKind::Spearman),
            (2, PieceKind::Archer),
            (3, PieceKind::Trebuchet),
            (4, PieceKind::Archer),
            (5, PieceKind::Spearman),
        ];
        for (col, kind) in back {
            assert_eq!(
                state.piece_at(Square::new(0, col)),
                Some(Piece { kind, owner: 0 })
            );
            assert_eq!(
                state.piece_at(Square::new(6, col)),
                Some(Piece { kind, owner: 1 })
            );
        }

        // The trebuchet starts standing on its own middle castle, and the
        // castle corners hold no piece at all.
        assert_eq!(GameState::castle_slot_at(Square::new(0, 3)), Some((0, 1)));
        assert!(state.piece_at(Square::new(0, 0)).is_none());
        assert!(state.piece_at(Square::new(6, 6)).is_none());
    }

    #[test]
    fn hilltops_are_the_three_middle_row_squares() {
        let hilltops: Vec<Square> = GameState::squares()
            .filter(|&square| GameState::is_hilltop(square))
            .collect();
        assert_eq!(
            hilltops,
            vec![Square::new(3, 0), Square::new(3, 3), Square::new(3, 6)]
        );
    }

    #[test]
    fn each_piece_strikes_its_printed_pattern() {
        let cases = [
            (PieceKind::Swordsman, vec![(2, 3), (4, 3), (3, 2), (3, 4)]),
            (PieceKind::Flail, vec![(2, 2), (2, 4), (4, 2), (4, 4)]),
            (PieceKind::Spearman, vec![(1, 3), (5, 3), (3, 1), (3, 5)]),
            (PieceKind::Archer, vec![(1, 1), (1, 5), (5, 1), (5, 5)]),
        ];

        for (kind, targets) in cases {
            let mut state = empty_board();
            place(&mut state, 3, 3, kind, 0);
            // Ring the attacker with enemies at every square within two, so
            // only the pattern decides who dies.
            for row in 1..=5u8 {
                for col in 1..=5u8 {
                    if (row, col) != (3, 3) {
                        place(&mut state, row, col, PieceKind::Swordsman, 1);
                    }
                }
            }
            state.token_mut(0, TokenKind::Row).line = 3;
            let (dead, castles) = destroyed_by_attack(&state, 0, TokenKind::Row);
            let expected: Vec<Square> = targets
                .iter()
                .map(|&(row, col)| Square::new(row, col))
                .collect();
            let mut expected = expected;
            expected.sort();
            let mut dead = dead;
            dead.sort();
            assert_eq!(dead, expected, "{kind:?} struck the wrong squares");
            assert!(castles.is_empty(), "{kind:?} must not damage a castle");
        }
    }

    #[test]
    fn a_trebuchet_on_the_centre_hilltop_reaches_every_enemy_castle() {
        let mut state = empty_board();
        place(&mut state, 3, 3, PieceKind::Trebuchet, 0);
        state.token_mut(0, TokenKind::Row).line = 3;

        let (pieces, castles) = destroyed_by_attack(&state, 0, TokenKind::Row);
        assert!(pieces.is_empty(), "a trebuchet does not shoot at pieces");
        assert_eq!(
            castles,
            vec![Square::new(6, 0), Square::new(6, 3), Square::new(6, 6)],
        );
    }

    #[test]
    fn a_side_hilltop_trebuchet_reaches_two_castles() {
        let mut state = empty_board();
        place(&mut state, 3, 0, PieceKind::Trebuchet, 0);
        state.token_mut(0, TokenKind::Row).line = 3;

        let (_, castles) = destroyed_by_attack(&state, 0, TokenKind::Row);
        // Straight down to (6,0), and diagonally to (6,3).
        assert_eq!(castles, vec![Square::new(6, 0), Square::new(6, 3)]);
    }

    #[test]
    fn a_trebuchet_off_a_hilltop_throws_nothing() {
        let mut state = empty_board();
        place(&mut state, 2, 3, PieceKind::Trebuchet, 0);
        state.token_mut(0, TokenKind::Row).line = 2;

        let result = attack_preview(&state, 0, TokenKind::Row);
        assert!(result.is_empty());
        assert_eq!(result.attackers, 0, "it does not even count as firing");
    }

    #[test]
    fn a_battering_ram_breaks_an_adjacent_castle_but_no_pieces() {
        let mut state = empty_board();
        place(&mut state, 5, 3, PieceKind::BatteringRam, 0);
        place(&mut state, 5, 4, PieceKind::Swordsman, 1);
        state.token_mut(0, TokenKind::Row).line = 5;

        let (pieces, castles) = destroyed_by_attack(&state, 0, TokenKind::Row);
        assert_eq!(castles, vec![Square::new(6, 3)]);
        assert!(pieces.is_empty(), "a ram does not fight pieces");
    }

    #[test]
    fn a_ram_diagonal_to_a_castle_reaches_nothing() {
        let mut state = empty_board();
        place(&mut state, 5, 2, PieceKind::BatteringRam, 0);
        state.token_mut(0, TokenKind::Row).line = 5;
        let (_, castles) = destroyed_by_attack(&state, 0, TokenKind::Row);
        assert!(castles.is_empty(), "the ram attacks straight only");
    }

    #[test]
    fn attacks_spare_friendly_pieces_and_already_ruined_castles() {
        let mut state = empty_board();
        place(&mut state, 3, 3, PieceKind::Swordsman, 0);
        place(&mut state, 3, 4, PieceKind::Swordsman, 0);
        place(&mut state, 2, 3, PieceKind::Archer, 1);
        place(&mut state, 3, 0, PieceKind::Trebuchet, 0);
        state.castles[1] = [false, true, false];
        state.token_mut(0, TokenKind::Row).line = 3;

        let (pieces, castles) = destroyed_by_attack(&state, 0, TokenKind::Row);
        assert_eq!(pieces, vec![Square::new(2, 3)]);
        // (6,0) is already rubble; only the standing middle castle is hit.
        assert_eq!(castles, vec![Square::new(6, 3)]);
    }

    #[test]
    fn the_column_token_attacks_down_its_column() {
        let mut state = empty_board();
        place(&mut state, 2, 4, PieceKind::Flail, 0);
        place(&mut state, 3, 5, PieceKind::Spearman, 1);
        place(&mut state, 3, 3, PieceKind::Spearman, 1);
        state.token_mut(0, TokenKind::Column).line = 4;

        let (pieces, _) = destroyed_by_attack(&state, 0, TokenKind::Column);
        assert_eq!(pieces, vec![Square::new(3, 3), Square::new(3, 5)]);
        // The same piece is not in the row token's line, so that token hits
        // nothing.
        state.token_mut(0, TokenKind::Row).line = 0;
        assert!(attack_preview(&state, 0, TokenKind::Row).is_empty());
    }

    #[test]
    fn the_first_turn_activates_only_the_token_that_was_slid() {
        let mut state = initial_state();
        assert!(state.is_first_turn());
        assert_eq!(state.phase, Phase::Slide);

        apply(
            &mut state,
            &Choice::Slide {
                token: TokenKind::Row,
                line: 1,
            },
        );

        // Straight to the movement activation: there is no order to choose
        // when only one token acts.
        assert_eq!(state.phase, Phase::Activate);
        assert_eq!(state.pending_tokens(), &[TokenKind::Row]);

        apply(
            &mut state,
            &Choice::Move {
                from: Square::new(1, 1),
                to: Square::new(2, 1),
            },
        );

        assert_eq!(state.current_player, 1);
        assert_eq!(state.phase, Phase::Slide);
        assert_eq!(
            state.token(0, TokenKind::Row).face,
            TokenFace::Attack,
            "the slid token flips to its attack side"
        );
        assert_eq!(
            state.token(0, TokenKind::Column).face,
            TokenFace::Movement,
            "the other token has not acted yet"
        );
    }

    #[test]
    fn later_turns_slide_one_token_and_activate_both() {
        let mut state = initial_state();
        // Both sides take their opening turn.
        for player in 0..2u8 {
            let row = if player == 0 { 1 } else { 5 };
            let step = if player == 0 { 2 } else { 4 };
            apply(
                &mut state,
                &Choice::Slide {
                    token: TokenKind::Row,
                    line: row,
                },
            );
            apply(
                &mut state,
                &Choice::Move {
                    from: Square::new(row, 1),
                    to: Square::new(step, 1),
                },
            );
        }

        assert!(!state.is_first_turn());
        // Only the column token still shows a movement face, so it is the one
        // that may slide.
        let slides = legal_choices(&state);
        assert!(slides.iter().all(|choice| matches!(
            choice,
            Choice::Slide {
                token: TokenKind::Column,
                ..
            }
        )));

        apply(
            &mut state,
            &Choice::Slide {
                token: TokenKind::Column,
                line: 2,
            },
        );
        assert_eq!(state.phase, Phase::Order);
        assert_eq!(
            legal_choices(&state),
            vec![
                Choice::Order {
                    first: TokenKind::Row
                },
                Choice::Order {
                    first: TokenKind::Column
                },
            ]
        );

        // Attacking first resolves the row token's volley on the way to the
        // column token's movement activation.
        apply(
            &mut state,
            &Choice::Order {
                first: TokenKind::Row,
            },
        );
        assert_eq!(state.phase, Phase::Activate);
        assert_eq!(state.pending_tokens(), &[TokenKind::Column]);
        assert_eq!(state.token(0, TokenKind::Row).face, TokenFace::Movement);
    }

    #[test]
    fn tokens_alternate_faces_every_turn_after_the_first() {
        let mut state = initial_state();
        let mut rng_index = 0usize;
        let mut choices = Vec::new();

        for _ in 0..60 {
            if state.outcome.is_some() {
                break;
            }
            if state.phase == Phase::Slide && state.current_player == 0 && !state.is_first_turn() {
                let faces: Vec<TokenFace> = TOKEN_KINDS
                    .iter()
                    .map(|&kind| state.token(0, kind).face)
                    .collect();
                assert_ne!(
                    faces[0], faces[1],
                    "exactly one token shows a movement face at the start of a turn"
                );
            }
            legal_choices_into(&state, &mut choices);
            let choice = choices[rng_index % choices.len()];
            rng_index += 7;
            apply(&mut state, &choice);
        }
    }

    #[test]
    fn a_slide_must_change_line_and_needs_a_friendly_piece_there() {
        let state = initial_state();
        let choices = legal_choices(&state);

        for choice in &choices {
            let Choice::Slide { token, line } = *choice else {
                panic!("the opening decision is a slide");
            };
            assert_ne!(line, state.token(0, token).line, "the token must move");
            assert!(
                GameState::line_squares(token, line)
                    .filter_map(|square| state.piece_at(square))
                    .any(|piece| piece.owner == 0),
                "{choice:?} names a line with no friendly piece"
            );
        }

        // Light's pieces occupy rows 0 and 1 and columns 1 through 5. The row
        // token sits on row 0, so row 1 is its only destination; the column
        // token sits on column 0, so all five occupied columns are open.
        assert_eq!(choices.len(), 1 + 5);
    }

    #[test]
    fn movement_never_captures_and_never_jumps() {
        let mut state = empty_board();
        place(&mut state, 3, 3, PieceKind::Swordsman, 0);
        place(&mut state, 3, 5, PieceKind::Swordsman, 1);
        place(&mut state, 1, 3, PieceKind::Flail, 0);
        state.current_player = 0;
        state.phase = Phase::Activate;
        state.pending = [TokenKind::Row, TokenKind::Column];
        state.pending_len = 1;
        state.token_mut(0, TokenKind::Row).line = 3;

        let destinations: Vec<Square> = legal_choices(&state)
            .into_iter()
            .filter_map(|choice| match choice {
                Choice::Move { from, to } if from == Square::new(3, 3) => Some(to),
                _ => None,
            })
            .collect();

        assert!(destinations.contains(&Square::new(3, 4)), "up to the blocker");
        assert!(
            !destinations.contains(&Square::new(3, 5)),
            "a move never captures"
        );
        assert!(
            !destinations.contains(&Square::new(3, 6)),
            "and never jumps the blocker"
        );
        assert!(destinations.contains(&Square::new(2, 3)));
        assert!(
            !destinations.contains(&Square::new(1, 3)),
            "friendly pieces block too"
        );
    }

    #[test]
    fn the_row_token_moves_like_a_rook_and_the_column_token_like_a_bishop() {
        let mut state = empty_board();
        place(&mut state, 3, 3, PieceKind::Swordsman, 0);
        state.current_player = 0;
        state.phase = Phase::Activate;
        state.pending_len = 1;

        for (kind, straight) in [(TokenKind::Row, true), (TokenKind::Column, false)] {
            state.pending = [kind, kind.other()];
            state.token_mut(0, kind).line = 3;
            let destinations: Vec<Square> = legal_choices(&state)
                .into_iter()
                .filter_map(|choice| match choice {
                    Choice::Move { to, .. } => Some(to),
                    _ => None,
                })
                .collect();

            // A rook from the centre of an empty 7x7 board reaches 12 squares;
            // so does a bishop from (3,3), which sits on the long diagonals.
            assert_eq!(destinations.len(), 12, "{kind:?}");
            let has_straight = destinations.contains(&Square::new(3, 6));
            let has_diagonal = destinations.contains(&Square::new(6, 6));
            assert_eq!(has_straight, straight, "{kind:?} straight");
            assert_eq!(has_diagonal, !straight, "{kind:?} diagonal");
        }
    }

    #[test]
    fn a_piece_may_leave_the_line_its_token_named() {
        // "Move a piece in the row like a Rook" picks the piece from the row;
        // the rook move itself may carry it out of that row entirely.
        let mut state = empty_board();
        place(&mut state, 3, 3, PieceKind::Swordsman, 0);
        state.current_player = 0;
        state.phase = Phase::Activate;
        state.pending = [TokenKind::Row, TokenKind::Column];
        state.pending_len = 1;
        state.token_mut(0, TokenKind::Row).line = 3;

        assert!(legal_choices(&state).contains(&Choice::Move {
            from: Square::new(3, 3),
            to: Square::new(0, 3),
        }));
    }

    #[test]
    fn razing_the_last_castle_ends_the_game() {
        let mut state = empty_board();
        place(&mut state, 5, 3, PieceKind::BatteringRam, 0);
        place(&mut state, 0, 3, PieceKind::Trebuchet, 0);
        place(&mut state, 6, 1, PieceKind::Trebuchet, 1);
        place(&mut state, 6, 2, PieceKind::BatteringRam, 1);
        state.castles[1] = [false, true, false];
        arm_attack(&mut state, 0, TokenKind::Row, 5);

        // The queued attack is the only thing left this turn, so settling
        // resolves it and ends the turn — and the game.
        settle_state(&mut state);
        assert_eq!(state.outcome, Some(Outcome::Winner { player: 0 }));
        assert_eq!(state.phase, Phase::GameOver);
        assert!(legal_choices(&state).is_empty());
    }

    #[test]
    fn losing_both_siege_engines_ends_the_game() {
        let mut state = empty_board();
        place(&mut state, 3, 3, PieceKind::Swordsman, 0);
        place(&mut state, 0, 1, PieceKind::Trebuchet, 0);
        place(&mut state, 0, 2, PieceKind::BatteringRam, 0);
        place(&mut state, 3, 2, PieceKind::Trebuchet, 1);
        place(&mut state, 2, 3, PieceKind::BatteringRam, 1);
        arm_attack(&mut state, 0, TokenKind::Row, 3);

        settle_state(&mut state);
        assert_eq!(
            state.outcome,
            Some(Outcome::Winner { player: 0 }),
            "a side with no siege engine can never raze a castle, so it has lost"
        );
    }

    #[test]
    fn one_surviving_siege_engine_is_not_a_loss() {
        let mut state = empty_board();
        place(&mut state, 3, 3, PieceKind::Swordsman, 0);
        place(&mut state, 3, 2, PieceKind::Trebuchet, 1);
        place(&mut state, 0, 0, PieceKind::BatteringRam, 1);
        place(&mut state, 0, 1, PieceKind::Trebuchet, 0);
        place(&mut state, 0, 2, PieceKind::BatteringRam, 0);
        arm_attack(&mut state, 0, TokenKind::Row, 3);

        settle_state(&mut state);
        assert_eq!(state.outcome, None);
        assert_eq!(state.siege_engines(1), 1);
    }

    #[test]
    fn a_move_activation_with_nowhere_to_go_passes() {
        let mut state = empty_board();
        // Boxed into a corner by pieces on both rook lines.
        place(&mut state, 0, 0, PieceKind::Swordsman, 0);
        place(&mut state, 0, 1, PieceKind::Swordsman, 1);
        place(&mut state, 1, 0, PieceKind::Swordsman, 1);
        state.current_player = 0;
        state.phase = Phase::Activate;
        state.pending = [TokenKind::Row, TokenKind::Column];
        state.pending_len = 1;
        state.token_mut(0, TokenKind::Row).line = 0;

        assert_eq!(legal_choices(&state), vec![Choice::Pass]);
    }

    #[test]
    fn every_position_reachable_by_random_play_offers_a_legal_choice() {
        let mut seed = 0x5EEDu64;
        for _ in 0..200 {
            let mut state = initial_state();
            let mut choices = Vec::new();
            let mut steps = 0;
            while state.outcome.is_none() {
                legal_choices_into(&state, &mut choices);
                assert!(
                    !choices.is_empty(),
                    "no legal choice in {:?} for player {}",
                    state.phase,
                    state.current_player
                );
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                let choice = choices[(seed >> 33) as usize % choices.len()];
                apply(&mut state, &choice);
                steps += 1;
                assert!(steps < 20_000, "a random game failed to terminate");
            }
            // Whatever ended it, the position agrees with the verdict.
            match state.outcome.unwrap() {
                Outcome::Winner { player } => {
                    let loser = GameState::opponent(player);
                    assert!(
                        state.castles_standing(loser) == 0 || state.siege_engines(loser) == 0
                    );
                }
                Outcome::Draw => assert!(state.turn >= MAX_TURNS),
            }
        }
    }
}
