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

/// What one shot destroyed. A shot destroys exactly one thing: siege engines
/// only ever fire at castles, and everything else fires at a piece or a wall —
/// and no square ever holds both, because a standing wall cannot be stood on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Casualty {
    Piece { piece: Piece },
    Castle,
    Wall,
}

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
    /// A movement activation spent without moving.
    Passed {
        player: u8,
        token: TokenKind,
    },
    /// An attack activation beginning. Every shot in it follows as its own
    /// event, because every shot is its own decision.
    Volley {
        player: u8,
        token: TokenKind,
        line: u8,
    },
    /// One attacker firing at its one target.
    Struck {
        player: u8,
        token: TokenKind,
        kind: PieceKind,
        from: Square,
        target: Square,
        casualty: Casualty,
    },
    /// An attacker that had a shot and did not take it.
    HeldFire {
        player: u8,
        token: TokenKind,
        kind: PieceKind,
        from: Square,
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
    let mut board = [[None; BOARD_COLS]; BOARD_ROWS];

    // Back row: spear, bow, trebuchet, bow, spear across the middle five
    // columns. The castle columns either side of them start empty; the
    // trebuchet starts on top of the middle castle. The outermost file on each
    // wing is empty ground.
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
            // The five pieces sit between the outer two castles.
            let col = CASTLE_COLS[0] + 1 + i as u8;
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
    let start_line = |player: usize, kind: TokenKind| match player {
        0 => 0,
        _ => GameState::line_count(kind) - 1,
    };
    let tokens = std::array::from_fn(|player| {
        TOKEN_KINDS.map(|kind| Token {
            line: start_line(player, kind),
            face: TokenFace::Movement,
        })
    });

    GameState {
        board,
        castles: [[true; CASTLES_PER_PLAYER]; NUM_PLAYERS],
        walls: [true; NUM_WALLS],
        tokens,
        current_player: 0,
        phase: Phase::Slide,
        pending: [TokenKind::Row, TokenKind::Column],
        pending_len: 0,
        attack_index: 0,
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
/// so a slide stops before any occupied square, and a standing enemy castle
/// stops it too: you may neither enter one nor pass through it. Your own
/// castles are open ground — the trebuchet opens the game standing on one.
fn slide_destinations_into(
    state: &GameState,
    player: u8,
    from: Square,
    kind: TokenKind,
    out: &mut Vec<Choice>,
) {
    let dirs = match kind {
        TokenKind::Row => &ROOK_DIRS,
        TokenKind::Column => &BISHOP_DIRS,
    };
    for &(drow, dcol) in dirs {
        let mut square = from;
        while let Some(next) = square.offset(drow, dcol) {
            if state.piece_at(next).is_some() || state.blocks_slide(next, player) {
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
                slide_destinations_into(state, player, square, kind, out)
            }
            _ => {}
        }
    }
}

/// Every square the piece on `from` could strike as the board stands: enemy
/// pieces and standing walls for an ordinary piece, standing enemy castles for
/// a siege engine. A trebuchet off a hilltop throws nothing at all.
///
/// Walls fall to the infantry alone. The siege train is strictly anti-castle,
/// so clearing a lane through the middle row is never something a ram or a
/// trebuchet can do for itself.
fn strikes(state: &GameState, from: Square, piece: Piece) -> impl Iterator<Item = Square> + '_ {
    let enemy = GameState::opponent(piece.owner);
    let blind = piece.kind == PieceKind::Trebuchet && !state.is_hilltop(from);
    let offsets: &'static [(i8, i8)] = if blind { &[] } else { piece.kind.attack_offsets() };

    offsets
        .iter()
        .filter_map(move |&(drow, dcol)| from.offset(drow, dcol))
        .filter(move |&target| {
            if piece.kind.is_siege() {
                state.standing_castle_at(target) == Some(enemy)
            } else {
                state.standing_wall_at(target).is_some()
                    || state
                        .piece_at(target)
                        .is_some_and(|victim| victim.owner == enemy)
            }
        })
}

/// Every on-board square a piece's pattern reaches, whoever is standing there.
/// [`strikes`] narrows this to the squares that hold a legal target; the board
/// wants the wider set to show where a move would not survive.
fn covered_squares(state: &GameState, from: Square, piece: Piece) -> impl Iterator<Item = Square> {
    let blind = piece.kind == PieceKind::Trebuchet && !state.is_hilltop(from);
    let offsets: &'static [(i8, i8)] = if blind { &[] } else { piece.kind.attack_offsets() };
    offsets
        .iter()
        .filter_map(move |&(drow, dcol)| from.offset(drow, dcol))
}

/// The next piece of the current player's on `kind`'s line, at or after index
/// `from`, with a shot to take. Pieces with nothing to fire at are skipped:
/// they carry no decision, so the volley never stops on them.
fn next_attacker(state: &GameState, kind: TokenKind, from: u8) -> Option<u8> {
    let player = state.current_player;
    let line = state.token(player, kind).line;
    (from..GameState::line_len(kind)).find(|&index| {
        let square = GameState::line_square(kind, line, index);
        state.piece_at(square).is_some_and(|piece| {
            piece.owner == player && strikes(state, square, piece).next().is_some()
        })
    })
}

/// The attacker now choosing a target, and the piece standing on it.
fn current_attacker(state: &GameState) -> (Square, Piece) {
    let kind = state.pending[0];
    let line = state.token(state.current_player, kind).line;
    let square = GameState::line_square(kind, line, state.attack_index);
    let piece = state
        .piece_at(square)
        .expect("a volley only ever stops on a piece with a shot");
    (square, piece)
}

pub fn legal_choices_into(state: &GameState, out: &mut Vec<Choice>) {
    out.clear();
    match state.phase {
        Phase::Slide => {
            let player = state.current_player;
            let slidable = slidable_tokens(state);
            for &kind in &slidable {
                let current = state.token(player, kind).line;
                for line in 0..GameState::line_count(kind) {
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
            // `settle` takes every attack face into `Phase::Attack`, so the
            // head of the queue always shows a movement face here.
            let kind = state.pending[0];
            movement_choices_into(state, kind, out);
            // "You may also flip the token and take no action."
            out.push(Choice::Pass);
        }
        Phase::Attack => {
            let (from, piece) = current_attacker(state);
            for target in strikes(state, from, piece) {
                out.push(Choice::Attack { from, target });
            }
            out.push(Choice::HoldFire { from });
        }
        Phase::GameOver => {}
    }
}

pub fn legal_choices(state: &GameState) -> Vec<Choice> {
    let mut out = Vec::new();
    legal_choices_into(state, &mut out);
    out
}

/// Everything one attack activation could strike from its line. Each attacker
/// fires at most one of these, so this is the volley's reach and not its
/// casualty list — which is exactly what a threat overlay wants to show.
#[derive(Default)]
pub struct AttackReach {
    /// Friendly pieces in the line with a shot to take, as a square bitboard.
    pub attackers: u64,
    /// Every square this line's pieces could shoot an enemy standing on,
    /// occupied or not. An attacker with nothing to fire at today is still
    /// counted: walking into its pattern is what gives it a target. Siege
    /// engines add nothing — they break castles and leave what stands on one
    /// alone.
    pub covered: u64,
    /// Enemy pieces under threat, as a square bitboard.
    pub pieces: u64,
    /// Enemy castles under threat, indexed by slot.
    pub castles: [bool; CASTLES_PER_PLAYER],
    /// Walls under threat, indexed by slot.
    pub walls: [bool; NUM_WALLS],
}

impl AttackReach {
    pub fn squares(bits: u64) -> impl Iterator<Item = Square> {
        GameState::squares().filter(move |square| bits >> square.index() & 1 == 1)
    }

    pub fn is_empty(&self) -> bool {
        self.pieces == 0
            && !self.castles.iter().any(|&c| c)
            && !self.walls.iter().any(|&w| w)
    }
}

/// What an attack with `player`'s `kind` token, at its current line, bears on.
/// Pure: the UI calls it to preview a threat.
pub fn attack_preview(state: &GameState, player: u8, kind: TokenKind) -> AttackReach {
    let line = state.token(player, kind).line;
    let mut reach = AttackReach::default();

    for square in GameState::line_squares(kind, line) {
        let Some(piece) = state.piece_at(square) else {
            continue;
        };
        if piece.owner != player {
            continue;
        }
        if !piece.kind.is_siege() {
            for target in covered_squares(state, square, piece) {
                reach.covered |= 1 << target.index();
            }
        }
        for target in strikes(state, square, piece) {
            reach.attackers |= 1 << square.index();
            if piece.kind.is_siege() {
                let (_, index) =
                    GameState::castle_slot_at(target).expect("a siege engine fires at castles");
                reach.castles[index] = true;
            } else if let Some(index) = state.standing_wall_at(target) {
                reach.walls[index] = true;
            } else {
                reach.pieces |= 1 << target.index();
            }
        }
    }

    reach
}

/// Resolve one shot. Shots land as they are chosen rather than all at once, so
/// an attacker later down the line cannot spend itself on a target an earlier
/// one has already taken.
fn fire(state: &mut GameState, from: Square, target: Square, sink: &mut impl EventSink) {
    let player = state.current_player;
    let token = state.pending[0];
    let piece = state
        .piece_at(from)
        .expect("fired with a piece that is not there");

    let casualty = if piece.kind.is_siege() {
        let (owner, index) =
            GameState::castle_slot_at(target).expect("a siege engine fires at castles");
        state.castles[owner as usize][index] = false;
        Casualty::Castle
    } else if let Some(index) = state.standing_wall_at(target) {
        state.walls[index] = false;
        Casualty::Wall
    } else {
        let victim = state.piece_at(target).expect("fired at an empty square");
        state.set_piece(target, None);
        Casualty::Piece { piece: victim }
    };

    sink.emit(|| GameEvent::Struck {
        player,
        token,
        kind: piece.kind,
        from,
        target,
        casualty,
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
    state.attack_index = 0;
}

fn end_turn(state: &mut GameState, sink: &mut impl EventSink) {
    let player = state.current_player;
    sink.emit(|| GameEvent::TurnEnded { player });
    state.current_player = GameState::opponent(player);
    state.pending_len = 0;
    // Reset rather than leave the drained queue and the volley's cursor
    // behind: two turns that reached the same board by different activation
    // orders are the same position, and stale fields would say otherwise.
    state.pending = [TokenKind::Row, TokenKind::Column];
    state.attack_index = 0;
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
        if !matches!(state.phase, Phase::Activate | Phase::Attack) {
            return;
        }
        if state.pending_len == 0 {
            end_turn(state, sink);
            continue;
        }
        let player = state.current_player;
        let kind = state.pending[0];
        if state.token(player, kind).face == TokenFace::Movement {
            state.phase = Phase::Activate;
            return;
        }

        // An attack face walks its line, stopping on each piece with a shot so
        // that piece can pick its single target. Entering the volley announces
        // it once; the shots themselves are separate decisions.
        if state.phase != Phase::Attack {
            let line = state.token(player, kind).line;
            sink.emit(|| GameEvent::Volley {
                player,
                token: kind,
                line,
            });
            state.phase = Phase::Attack;
            state.attack_index = 0;
        }

        match next_attacker(state, kind, state.attack_index) {
            Some(index) => {
                state.attack_index = index;
                return;
            }
            None => {
                let token = state.token_mut(player, kind);
                token.face = token.face.flipped();
                pop_pending(state);
                state.phase = Phase::Activate;
            }
        }
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
        Choice::Attack { from, target } => {
            fire(state, from, target, sink);
            state.attack_index += 1;
            check_outcome(state);
        }
        Choice::HoldFire { from } => {
            let token = state.pending[0];
            let kind = state
                .piece_at(from)
                .expect("held fire with a piece that is not there")
                .kind;
            sink.emit(|| GameEvent::HeldFire {
                player,
                token,
                kind,
                from,
            });
            state.attack_index += 1;
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

    /// The opening position with every piece and wall swept off, so a test can
    /// state exactly the position it means. Castles and tokens are left as
    /// dealt; a test that wants a wall puts it back with `raise_wall`.
    fn empty_board() -> GameState {
        let mut state = initial_state();
        state.board = [[None; BOARD_COLS]; BOARD_ROWS];
        state.walls = [false; NUM_WALLS];
        state
    }

    fn raise_wall(state: &mut GameState, square: Square) {
        let index = GameState::wall_slot_at(square).expect("no wall stands there");
        state.walls[index] = true;
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

    /// The pieces and castles an attack with `kind` bears on. Each attacker
    /// fires once, so this is the reach of the volley, not its body count.
    fn threatened_by(state: &GameState, player: u8, kind: TokenKind) -> (Vec<Square>, Vec<Square>) {
        let reach = attack_preview(state, player, kind);
        let enemy = GameState::opponent(player);
        let pieces = AttackReach::squares(reach.pieces).collect();
        let castles = reach
            .castles
            .iter()
            .enumerate()
            .filter(|(_, &hit)| hit)
            .map(|(index, _)| GameState::castle_square(enemy, index))
            .collect();
        (pieces, castles)
    }

    /// Play a whole volley out, taking the first shot offered at every stop.
    fn fire_at_will(state: &mut GameState) {
        while state.phase == Phase::Attack {
            let shot = legal_choices(state)
                .into_iter()
                .find(|choice| matches!(choice, Choice::Attack { .. }))
                .expect("a volley only stops where there is a shot to take");
            apply(state, &shot);
        }
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
            (2, PieceKind::Spearman),
            (3, PieceKind::Archer),
            (4, PieceKind::Trebuchet),
            (5, PieceKind::Archer),
            (6, PieceKind::Spearman),
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
        // castle squares either side of the line hold no piece at all.
        assert_eq!(GameState::castle_slot_at(Square::new(0, 4)), Some((0, 1)));
        assert!(state.piece_at(Square::new(0, 1)).is_none());
        assert!(state.piece_at(Square::new(6, 7)).is_none());

        // The outer file on each wing is empty ground, top to bottom.
        for row in 0..BOARD_ROWS as u8 {
            for col in [0, BOARD_COLS as u8 - 1] {
                assert!(state.piece_at(Square::new(row, col)).is_none());
            }
        }

        assert_eq!(state.walls_standing(), NUM_WALLS);
    }

    #[test]
    fn the_walls_fill_the_middle_row_between_the_hilltops() {
        let state = initial_state();
        let walls: Vec<Square> = GameState::squares()
            .filter(|&square| state.standing_wall_at(square).is_some())
            .collect();
        assert_eq!(
            walls,
            vec![
                Square::new(3, 2),
                Square::new(3, 3),
                Square::new(3, 5),
                Square::new(3, 6),
            ],
        );
        // Hilltop, wall, wall, hilltop, wall, wall, hilltop across the middle.
        for square in walls {
            assert!(!state.is_hilltop(square));
        }
    }

    #[test]
    fn a_standing_wall_stops_a_slide_and_a_broken_one_does_not() {
        let mut state = empty_board();
        raise_wall(&mut state, Square::new(3, 3));
        place(&mut state, 3, 1, PieceKind::Swordsman, 0);
        state.current_player = 0;
        state.phase = Phase::Activate;
        state.pending = [TokenKind::Row, TokenKind::Column];
        state.pending_len = 1;
        state.token_mut(0, TokenKind::Row).line = 3;

        let destinations = |state: &GameState| -> Vec<Square> {
            legal_choices(state)
                .into_iter()
                .filter_map(|choice| match choice {
                    Choice::Move { to, .. } => Some(to),
                    _ => None,
                })
                .collect()
        };

        let blocked = destinations(&state);
        assert!(blocked.contains(&Square::new(3, 2)), "up to the wall");
        assert!(!blocked.contains(&Square::new(3, 3)), "never onto it");
        assert!(!blocked.contains(&Square::new(3, 4)), "and never through it");

        state.walls = [false; NUM_WALLS];
        let cleared = destinations(&state);
        assert!(cleared.contains(&Square::new(3, 3)), "rubble is open ground");
        assert!(cleared.contains(&Square::new(3, 4)));
    }

    #[test]
    fn an_ordinary_piece_breaks_a_wall_and_a_siege_engine_cannot() {
        let mut state = empty_board();
        raise_wall(&mut state, Square::new(3, 3));
        place(&mut state, 3, 2, PieceKind::BatteringRam, 0);
        place(&mut state, 3, 4, PieceKind::Trebuchet, 0);
        // Siege engines for the other side too, off in the empty wing: without
        // them player 1 has already lost and the shot below ends the game.
        place(&mut state, 0, 0, PieceKind::Trebuchet, 1);
        place(&mut state, 6, 0, PieceKind::BatteringRam, 1);
        state.token_mut(0, TokenKind::Row).line = 3;

        // (3,4) is the centre hilltop, so that trebuchet is a live siege
        // weapon — and still has nothing to say to a wall one square away,
        // any more than the ram touching it does.
        assert!(state.is_hilltop(Square::new(3, 4)));
        assert_eq!(
            attack_preview(&state, 0, TokenKind::Row).walls,
            [false; NUM_WALLS],
            "the siege train leaves walls alone"
        );

        place(&mut state, 2, 3, PieceKind::Swordsman, 0);
        state.token_mut(0, TokenKind::Column).line = 3;
        let reach = attack_preview(&state, 0, TokenKind::Column);
        assert_eq!(reach.walls, [false, true, false, false]);

        arm_attack(&mut state, 0, TokenKind::Column, 3);
        settle_state(&mut state);
        assert_eq!(
            legal_choices(&state),
            vec![
                Choice::Attack {
                    from: Square::new(2, 3),
                    target: Square::new(3, 3),
                },
                Choice::HoldFire {
                    from: Square::new(2, 3),
                },
            ],
        );

        let events = apply_logged(
            &mut state,
            &Choice::Attack {
                from: Square::new(2, 3),
                target: Square::new(3, 3),
            },
        );
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::Struck {
                casualty: Casualty::Wall,
                ..
            }
        )));
        assert_eq!(state.walls_standing(), 0);
        assert_eq!(state.outcome, None, "a wall is nobody's to lose");
    }

    #[test]
    fn hilltops_are_the_three_middle_row_squares() {
        let state = initial_state();
        let hilltops: Vec<Square> = GameState::squares()
            .filter(|&square| state.is_hilltop(square))
            .collect();
        assert_eq!(
            hilltops,
            vec![Square::new(3, 1), Square::new(3, 4), Square::new(3, 7)]
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
            let (dead, castles) = threatened_by(&state, 0, TokenKind::Row);
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
    fn a_volley_takes_one_target_per_attacker() {
        // Two flails on the same row, and four enemies each of them covers.
        let mut state = empty_board();
        place(&mut state, 3, 1, PieceKind::Flail, 0);
        place(&mut state, 3, 3, PieceKind::Flail, 0);
        for (row, col) in [(2, 0), (2, 2), (4, 2), (4, 4)] {
            place(&mut state, row, col, PieceKind::Swordsman, 1);
        }
        // Siege engines for both sides, off the line: without them a side has
        // already lost and the volley ends the game mid-way.
        for (col, kind) in [(2, PieceKind::Trebuchet), (3, PieceKind::BatteringRam)] {
            place(&mut state, 0, col, kind, 1);
            place(&mut state, 6, col, kind, 0);
        }
        arm_attack(&mut state, 0, TokenKind::Row, 3);
        settle_state(&mut state);

        assert_eq!(state.phase, Phase::Attack);
        fire_at_will(&mut state);

        // Both flails covered three of the four, and each took one.
        assert_eq!(
            state
                .pieces_of(1)
                .filter(|(_, piece)| piece.kind == PieceKind::Swordsman)
                .count(),
            2,
        );
    }

    #[test]
    fn an_attacker_may_hold_its_fire() {
        let mut state = empty_board();
        place(&mut state, 3, 3, PieceKind::Swordsman, 0);
        place(&mut state, 2, 3, PieceKind::Swordsman, 1);
        for (col, kind) in [(2, PieceKind::Trebuchet), (3, PieceKind::BatteringRam)] {
            place(&mut state, 0, col, kind, 1);
            place(&mut state, 6, col, kind, 0);
        }
        arm_attack(&mut state, 0, TokenKind::Row, 3);
        settle_state(&mut state);

        let choices = legal_choices(&state);
        assert!(choices.contains(&Choice::Attack {
            from: Square::new(3, 3),
            target: Square::new(2, 3),
        }));
        assert!(choices.contains(&Choice::HoldFire {
            from: Square::new(3, 3)
        }));

        apply(
            &mut state,
            &Choice::HoldFire {
                from: Square::new(3, 3),
            },
        );
        assert!(
            state.piece_at(Square::new(2, 3)).is_some(),
            "holding fire spares the target"
        );
        assert_ne!(state.phase, Phase::Attack, "and ends the volley");
    }

    #[test]
    fn the_centre_trebuchet_razes_one_castle_a_turn() {
        let mut state = empty_board();
        place(&mut state, 3, 4, PieceKind::Trebuchet, 0);
        place(&mut state, 6, 3, PieceKind::BatteringRam, 0);
        place(&mut state, 0, 2, PieceKind::Trebuchet, 1);
        place(&mut state, 0, 3, PieceKind::BatteringRam, 1);
        arm_attack(&mut state, 0, TokenKind::Row, 3);
        settle_state(&mut state);

        // It bears on all three, and picks one of them.
        assert_eq!(legal_choices(&state).len(), 3 + 1);
        fire_at_will(&mut state);

        assert_eq!(state.castles_standing(1), 2);
        assert_eq!(state.outcome, None, "one volley cannot raze three castles");
    }

    #[test]
    fn a_razed_castle_becomes_a_hilltop() {
        let mut state = empty_board();
        state.castles[1] = [false, true, true];
        let rubble = GameState::castle_square(1, 0);
        assert!(state.is_hilltop(rubble));

        // From the rubble it made, a trebuchet bears on the middle castle
        // three squares along the back rank.
        place(&mut state, rubble.row, rubble.col, PieceKind::Trebuchet, 0);
        state.token_mut(0, TokenKind::Row).line = rubble.row;
        let (_, castles) = threatened_by(&state, 0, TokenKind::Row);
        assert_eq!(castles, vec![GameState::castle_square(1, 1)]);

        // And a piece may stand there, which it could not while it stood.
        assert!(!state.blocks_slide(rubble, 0));
        assert!(state.blocks_slide(GameState::castle_square(1, 1), 0));
    }

    #[test]
    fn a_trebuchet_on_the_centre_hilltop_reaches_every_enemy_castle() {
        let mut state = empty_board();
        place(&mut state, 3, 4, PieceKind::Trebuchet, 0);
        state.token_mut(0, TokenKind::Row).line = 3;

        let (pieces, castles) = threatened_by(&state, 0, TokenKind::Row);
        assert!(pieces.is_empty(), "a trebuchet does not shoot at pieces");
        assert_eq!(
            castles,
            vec![Square::new(6, 1), Square::new(6, 4), Square::new(6, 7)],
        );
    }

    #[test]
    fn a_side_hilltop_trebuchet_reaches_two_castles() {
        let mut state = empty_board();
        place(&mut state, 3, 1, PieceKind::Trebuchet, 0);
        state.token_mut(0, TokenKind::Row).line = 3;

        let (_, castles) = threatened_by(&state, 0, TokenKind::Row);
        // Straight down to (6,1), and diagonally to (6,4).
        assert_eq!(castles, vec![Square::new(6, 1), Square::new(6, 4)]);
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
        place(&mut state, 5, 4, PieceKind::BatteringRam, 0);
        place(&mut state, 5, 5, PieceKind::Swordsman, 1);
        state.token_mut(0, TokenKind::Row).line = 5;

        let (pieces, castles) = threatened_by(&state, 0, TokenKind::Row);
        assert_eq!(castles, vec![Square::new(6, 4)]);
        assert!(pieces.is_empty(), "a ram does not fight pieces");
    }

    #[test]
    fn a_ram_diagonal_to_a_castle_reaches_nothing() {
        let mut state = empty_board();
        place(&mut state, 5, 2, PieceKind::BatteringRam, 0);
        state.token_mut(0, TokenKind::Row).line = 5;
        let (_, castles) = threatened_by(&state, 0, TokenKind::Row);
        // (6,1) is a castle one square away, but on the diagonal.
        assert!(castles.is_empty(), "the ram attacks straight only");
    }

    #[test]
    fn attacks_spare_friendly_pieces_and_already_ruined_castles() {
        let mut state = empty_board();
        place(&mut state, 3, 3, PieceKind::Swordsman, 0);
        place(&mut state, 3, 4, PieceKind::Swordsman, 0);
        place(&mut state, 2, 3, PieceKind::Archer, 1);
        place(&mut state, 3, 1, PieceKind::Trebuchet, 0);
        state.castles[1] = [false, true, false];
        state.token_mut(0, TokenKind::Row).line = 3;

        let (pieces, castles) = threatened_by(&state, 0, TokenKind::Row);
        assert_eq!(pieces, vec![Square::new(2, 3)]);
        // (6,1) is already rubble; only the standing middle castle is hit.
        assert_eq!(castles, vec![Square::new(6, 4)]);
    }

    #[test]
    fn the_column_token_attacks_down_its_column() {
        let mut state = empty_board();
        place(&mut state, 2, 4, PieceKind::Flail, 0);
        place(&mut state, 3, 5, PieceKind::Spearman, 1);
        place(&mut state, 3, 3, PieceKind::Spearman, 1);
        state.token_mut(0, TokenKind::Column).line = 4;

        let (pieces, _) = threatened_by(&state, 0, TokenKind::Column);
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
                from: Square::new(1, 2),
                to: Square::new(2, 2),
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
                    from: Square::new(row, 2),
                    to: Square::new(step, 2),
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

        // Light's pieces occupy rows 0 and 1 and columns 2 through 6. The row
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
    fn an_enemy_castle_stops_a_slide_and_a_friendly_one_does_not() {
        let mut state = empty_board();
        // On the enemy back rank, between their outer and middle castles.
        place(&mut state, 6, 2, PieceKind::Swordsman, 0);
        state.current_player = 0;
        state.phase = Phase::Activate;
        state.pending = [TokenKind::Row, TokenKind::Column];
        state.pending_len = 1;
        state.token_mut(0, TokenKind::Row).line = 6;

        let destinations: Vec<Square> = legal_choices(&state)
            .into_iter()
            .filter_map(|choice| match choice {
                Choice::Move { to, .. } => Some(to),
                _ => None,
            })
            .collect();

        assert!(!destinations.contains(&Square::new(6, 1)), "cannot enter it");
        assert!(destinations.contains(&Square::new(6, 3)));
        assert!(
            !destinations.contains(&Square::new(6, 4)),
            "the middle castle is enemy ground too"
        );
        assert!(
            !destinations.contains(&Square::new(6, 5)),
            "and a slide does not pass through it"
        );

        // Its own castles are open ground: the trebuchet opens standing on one.
        let mut own = empty_board();
        place(&mut own, 0, 2, PieceKind::Swordsman, 0);
        own.current_player = 0;
        own.phase = Phase::Activate;
        own.pending = [TokenKind::Row, TokenKind::Column];
        own.pending_len = 1;
        own.token_mut(0, TokenKind::Row).line = 0;
        assert!(legal_choices(&own).contains(&Choice::Move {
            from: Square::new(0, 2),
            to: Square::new(0, 1),
        }));
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

            // From (3,3) on an empty 9x7 board a rook reaches 8 squares along
            // the rank and 6 up the file; a bishop runs three squares to each
            // of the four edges. Neither line meets a castle on the way.
            assert_eq!(destinations.len(), if straight { 14 } else { 12 }, "{kind:?}");
            let has_straight = destinations.contains(&Square::new(3, 8));
            let has_diagonal = destinations.contains(&Square::new(0, 0));
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
        place(&mut state, 5, 4, PieceKind::BatteringRam, 0);
        place(&mut state, 0, 3, PieceKind::Trebuchet, 0);
        place(&mut state, 6, 2, PieceKind::Trebuchet, 1);
        place(&mut state, 6, 3, PieceKind::BatteringRam, 1);
        state.castles[1] = [false, true, false];
        arm_attack(&mut state, 0, TokenKind::Row, 5);

        // The queued attack is the only thing left this turn, so taking its
        // one shot ends the turn — and the game.
        settle_state(&mut state);
        fire_at_will(&mut state);
        assert_eq!(state.outcome, Some(Outcome::Winner { player: 0 }));
        assert_eq!(state.phase, Phase::GameOver);
        assert!(legal_choices(&state).is_empty());
    }

    #[test]
    fn losing_both_siege_engines_ends_the_game() {
        // One attacker per engine: a single piece could only take one of them.
        let mut state = empty_board();
        place(&mut state, 3, 1, PieceKind::Swordsman, 0);
        place(&mut state, 3, 3, PieceKind::Swordsman, 0);
        place(&mut state, 0, 2, PieceKind::Trebuchet, 0);
        place(&mut state, 0, 3, PieceKind::BatteringRam, 0);
        place(&mut state, 3, 2, PieceKind::Trebuchet, 1);
        place(&mut state, 3, 4, PieceKind::BatteringRam, 1);
        arm_attack(&mut state, 0, TokenKind::Row, 3);

        settle_state(&mut state);
        fire_at_will(&mut state);
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
        place(&mut state, 6, 2, PieceKind::BatteringRam, 1);
        place(&mut state, 0, 2, PieceKind::Trebuchet, 0);
        place(&mut state, 0, 3, PieceKind::BatteringRam, 0);
        arm_attack(&mut state, 0, TokenKind::Row, 3);

        settle_state(&mut state);
        fire_at_will(&mut state);
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
    fn a_movement_activation_may_always_decline_to_move() {
        let mut state = empty_board();
        place(&mut state, 3, 3, PieceKind::Swordsman, 0);
        state.current_player = 0;
        state.phase = Phase::Activate;
        state.pending = [TokenKind::Row, TokenKind::Column];
        state.pending_len = 1;
        state.token_mut(0, TokenKind::Row).line = 3;

        let choices = legal_choices(&state);
        assert!(choices.len() > 1, "the piece has somewhere to go");
        assert!(
            choices.contains(&Choice::Pass),
            "and may still flip the token and take no action"
        );
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
