//! Read-only questions the interface asks about a position.
//!
//! Every answer is produced by running the real rules forward, so the browser
//! never reasons about the game itself — it only asks "what would happen if"
//! and draws the reply.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::rules::{apply, attack_preview, legal_choices, AttackReach};
use crate::types::*;

/// Positions a single [`order_matters`] enumeration may visit before it gives
/// up. A volley is a shot per attacker, so a busy line multiplies out fast.
/// Past the budget the interface asks the question rather than guessing at it,
/// which costs the player one click and never a decision.
const ORDER_BUDGET: usize = 30_000;

/// Whether the two activation orders can reach different positions.
///
/// The player is asked to order their activations on every turn but the first,
/// and often the answer cannot matter: a volley with nothing to shoot at is
/// the same volley on either side of a move. Enumerating both orders to the
/// end of the turn settles it exactly, so the interface can drop the question
/// without ever dropping a real decision.
pub fn order_matters(state: &GameState) -> bool {
    if state.phase != Phase::Order {
        return false;
    }
    if volley_is_idle(state) {
        return false;
    }
    let mut budget = ORDER_BUDGET;
    let Some(row_first) = turn_endings(state, TokenKind::Row, &mut budget) else {
        return true;
    };
    let Some(column_first) = turn_endings(state, TokenKind::Column, &mut budget) else {
        return true;
    };
    row_first != column_first
}

/// Whether this turn's volley can end up with a shot to take at all.
///
/// A volley that never fires cannot be sequenced against — the move meets the
/// same board on either side of it — and that is the common case by a wide
/// margin. Settling it with a preview per move is worth a great deal over
/// enumerating every ordering of every shot to reach the same answer.
fn volley_is_idle(state: &GameState) -> bool {
    let player = state.current_player;
    let Some(armed) = TOKEN_KINDS
        .iter()
        .copied()
        .find(|&kind| state.token(player, kind).face == TokenFace::Attack)
    else {
        return true;
    };
    if attack_preview(state, player, armed).attackers != 0 {
        return false;
    }

    // Nothing on the line bears on anything, so only the move could give it a
    // target. Take the move first and see whether any destination does.
    let mut probe = *state;
    apply(
        &mut probe,
        &Choice::Order {
            first: armed.other(),
        },
    );
    legal_choices(&probe).into_iter().all(|choice| {
        if !matches!(choice, Choice::Move { .. }) {
            return true;
        }
        let mut next = probe;
        apply(&mut next, &choice);
        attack_preview(&next, player, armed).attackers == 0
    })
}

/// Every position this turn can end on once `first` activates first, or `None`
/// if the enumeration ran out of budget.
fn turn_endings(
    state: &GameState,
    first: TokenKind,
    budget: &mut usize,
) -> Option<HashSet<GameState>> {
    let mut next = *state;
    apply(&mut next, &Choice::Order { first });
    let mut endings = HashSet::new();
    collect_endings(&next, &mut endings, budget)?;
    Some(endings)
}

fn collect_endings(
    state: &GameState,
    out: &mut HashSet<GameState>,
    budget: &mut usize,
) -> Option<()> {
    *budget = budget.checked_sub(1)?;
    if state.outcome.is_some() || !matches!(state.phase, Phase::Activate | Phase::Attack) {
        out.insert(*state);
        return Some(());
    }
    for choice in legal_choices(state) {
        let mut next = *state;
        apply(&mut next, &choice);
        collect_endings(&next, out, budget)?;
    }
    Some(())
}

/// What one move puts within reach of the volley still to come this turn.
///
/// Moving a piece onto the line you armed last turn is the game's central
/// combination, and it is the one thing a player cannot read off the board:
/// whether it pays depends on where the piece lands. These are targets rather
/// than casualties — each attacker takes one shot, and the player picks which.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveOutcome {
    pub from: Square,
    pub to: Square,
    pub threatened_pieces: Vec<Square>,
    pub threatened_castles: Vec<Square>,
}

pub fn move_outcomes(state: &GameState) -> Vec<MoveOutcome> {
    if state.phase != Phase::Activate {
        return Vec::new();
    }
    let player = state.current_player;
    legal_choices(state)
        .into_iter()
        .filter_map(|choice| {
            let Choice::Move { from, to } = choice else {
                return None;
            };
            let mut next = *state;
            apply(&mut next, &choice);
            let (threatened_pieces, threatened_castles) = pending_threat(&next, player);
            Some(MoveOutcome {
                from,
                to,
                threatened_pieces,
                threatened_castles,
            })
        })
        .collect()
}

/// What `player`'s volley still to come bears on. Empty once it has fired, or
/// once the turn has passed on: there is then nothing left to set up.
fn pending_threat(state: &GameState, player: u8) -> (Vec<Square>, Vec<Square>) {
    let none = (Vec::new(), Vec::new());
    if state.current_player != player || state.pending_len == 0 {
        return none;
    }
    let kind = state.pending[0];
    if state.token(player, kind).face != TokenFace::Attack {
        return none;
    }
    let reach = attack_preview(state, player, kind);
    (
        AttackReach::squares(reach.pieces).collect(),
        threatened_castles(&reach, GameState::opponent(player)),
    )
}

/// What sliding a token to a line sets up.
///
/// A slide decides two things a turn apart — which pieces may move now, and
/// where the token will volley from next turn — and the second is the one
/// players miss. Both are reported for the line as it stands today.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlideOutcome {
    pub token: TokenKind,
    pub line: u8,
    /// Pieces on the line that would have a legal move under this token.
    pub movers: Vec<Square>,
    /// Squares the line would shoot an enemy standing on them, occupied today
    /// or not.
    pub covered: Vec<Square>,
    /// Enemy pieces standing in that reach right now.
    pub threatened_pieces: Vec<Square>,
    /// Enemy castles that reach takes in right now.
    pub threatened_castles: Vec<Square>,
}

pub fn slide_outcomes(state: &GameState) -> Vec<SlideOutcome> {
    if state.phase != Phase::Slide {
        return Vec::new();
    }
    let player = state.current_player;
    legal_choices(state)
        .into_iter()
        .filter_map(|choice| {
            let Choice::Slide { token, line } = choice else {
                return None;
            };

            let mut probe = *state;
            probe.token_mut(player, token).line = line;
            let reach = attack_preview(&probe, player, token);

            Some(SlideOutcome {
                token,
                line,
                movers: movers_after(state, &choice, token),
                covered: AttackReach::squares(reach.covered).collect(),
                threatened_pieces: AttackReach::squares(reach.pieces).collect(),
                threatened_castles: threatened_castles(&reach, GameState::opponent(player)),
            })
        })
        .collect()
}

/// The distinct pieces that could move once `slide` is played and `token`
/// activates first. Taking the token first is what the board is showing — a
/// volley resolved ahead of the move can only free more squares, never fewer.
fn movers_after(state: &GameState, slide: &Choice, token: TokenKind) -> Vec<Square> {
    let mut next = *state;
    apply(&mut next, slide);
    if next.phase == Phase::Order {
        apply(&mut next, &Choice::Order { first: token });
    }
    let mut movers = Vec::new();
    for choice in legal_choices(&next) {
        if let Choice::Move { from, .. } = choice {
            if !movers.contains(&from) {
                movers.push(from);
            }
        }
    }
    movers
}

/// The attackers of the volley in progress that have still to be asked for a
/// target, the one choosing now first.
///
/// Read as the shots left to take if none of them lands: a shot that empties a
/// square can leave a later attacker with nothing to fire at, and the volley
/// then skips it.
pub fn pending_attackers(state: &GameState) -> Vec<Square> {
    let mut squares = Vec::new();
    let mut probe = *state;
    while probe.phase == Phase::Attack && probe.outcome.is_none() {
        let Some(choice) = legal_choices(&probe)
            .into_iter()
            .find(|choice| matches!(choice, Choice::HoldFire { .. }))
        else {
            break;
        };
        let Choice::HoldFire { from } = choice else {
            break;
        };
        squares.push(from);
        apply(&mut probe, &choice);
    }
    squares
}

fn threatened_castles(reach: &AttackReach, enemy: u8) -> Vec<Square> {
    reach
        .castles
        .iter()
        .enumerate()
        .filter(|(_, &hit)| hit)
        .map(|(index, _)| GameState::castle_square(enemy, index))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{initial_state, settle_state};

    /// The opening position swept clear, so a test can state exactly the
    /// position it means. Castles and tokens are left as dealt.
    fn empty_board() -> GameState {
        let mut state = initial_state();
        state.board = [[None; BOARD_SIZE]; BOARD_SIZE];
        state
    }

    fn place(state: &mut GameState, row: u8, col: u8, kind: PieceKind, owner: u8) {
        state.set_piece(Square::new(row, col), Some(Piece { kind, owner }));
    }

    /// Both sides keep their siege engines out of the way, so no test position
    /// is accidentally already won.
    fn spare_engines(state: &mut GameState) {
        place(state, 5, 5, PieceKind::Trebuchet, 0);
        place(state, 5, 4, PieceKind::BatteringRam, 0);
        place(state, 6, 6, PieceKind::Trebuchet, 1);
        place(state, 6, 5, PieceKind::BatteringRam, 1);
    }

    fn token(state: &mut GameState, player: u8, kind: TokenKind, line: u8, face: TokenFace) {
        *state.token_mut(player, kind) = Token { line, face };
    }

    /// Mid-turn, slide already made, both activations still to come.
    fn order_phase(state: &mut GameState, player: u8) {
        state.current_player = player;
        state.pending = [TokenKind::Row, TokenKind::Column];
        state.pending_len = 2;
        state.phase = Phase::Order;
    }

    /// Mid-turn with `first` at the head of the queue and the other behind it.
    fn activate_phase(state: &mut GameState, player: u8, first: TokenKind) {
        state.current_player = player;
        state.pending = [first, first.other()];
        state.pending_len = 2;
        state.phase = Phase::Activate;
    }

    /// A swordsman that can leave the volley line but can never carry it onto
    /// anything: the order it activates in cannot change the position.
    fn barren_volley() -> GameState {
        let mut state = empty_board();
        spare_engines(&mut state);
        place(&mut state, 0, 1, PieceKind::Swordsman, 0);
        token(&mut state, 0, TokenKind::Row, 0, TokenFace::Attack);
        token(&mut state, 0, TokenKind::Column, 1, TokenFace::Movement);
        order_phase(&mut state, 0);
        state
    }

    #[test]
    fn order_is_no_choice_when_the_volley_has_nothing_to_shoot() {
        assert!(!order_matters(&barren_volley()));
    }

    #[test]
    fn order_is_a_real_choice_when_moving_away_spares_a_victim() {
        let mut state = barren_volley();
        // The swordsman on b7 now bears on b6 — and stepping off the rank to
        // reach b6's diagonal is exactly what spares the piece standing there.
        place(&mut state, 1, 1, PieceKind::Swordsman, 1);

        assert!(order_matters(&state));
    }

    #[test]
    fn order_is_no_choice_when_the_movement_line_is_empty() {
        // The movement token names a line with nothing of ours on it, so that
        // activation can only pass and the volley is all there is to sequence.
        let mut state = barren_volley();
        place(&mut state, 1, 1, PieceKind::Swordsman, 1);
        token(&mut state, 0, TokenKind::Column, 6, TokenFace::Movement);

        assert!(!order_matters(&state));
    }

    #[test]
    fn coverage_includes_squares_no_one_is_standing_on() {
        let mut state = empty_board();
        spare_engines(&mut state);
        place(&mut state, 3, 2, PieceKind::Swordsman, 0);
        token(&mut state, 0, TokenKind::Row, 3, TokenFace::Attack);

        let reach = attack_preview(&state, 0, TokenKind::Row);
        let covered: Vec<Square> = AttackReach::squares(reach.covered).collect();

        assert_eq!(
            covered,
            vec![
                Square::new(2, 2),
                Square::new(3, 1),
                Square::new(3, 3),
                Square::new(4, 2),
            ],
        );
        // Nothing is standing in it, so the volley has no shot to take.
        assert_eq!(reach.pieces, 0);
        assert_eq!(reach.attackers, 0);
    }

    #[test]
    fn a_siege_engine_covers_no_square_a_piece_could_stand_on() {
        let mut state = empty_board();
        spare_engines(&mut state);
        place(&mut state, 1, 3, PieceKind::BatteringRam, 0);
        token(&mut state, 0, TokenKind::Row, 1, TokenFace::Attack);

        assert_eq!(attack_preview(&state, 0, TokenKind::Row).covered, 0);
    }

    #[test]
    fn move_outcomes_report_what_each_destination_puts_in_reach() {
        let mut state = empty_board();
        spare_engines(&mut state);
        place(&mut state, 0, 1, PieceKind::Swordsman, 0);
        place(&mut state, 1, 3, PieceKind::Swordsman, 1);
        token(&mut state, 0, TokenKind::Row, 1, TokenFace::Attack);
        token(&mut state, 0, TokenKind::Column, 1, TokenFace::Movement);
        activate_phase(&mut state, 0, TokenKind::Column);

        let outcomes = move_outcomes(&state);
        let threatens = |to: Square| {
            outcomes
                .iter()
                .find(|outcome| outcome.to == to)
                .unwrap_or_else(|| panic!("no move to {to:?}"))
                .threatened_pieces
                .clone()
        };

        // Landing on c6 puts the swordsman beside d6 and on the armed rank.
        assert_eq!(threatens(Square::new(1, 2)), vec![Square::new(1, 3)]);
        // a6 is on the same rank but two squares short of anything.
        assert_eq!(threatens(Square::new(1, 0)), Vec::<Square>::new());
        // Leaving the rank altogether disarms the volley entirely.
        assert_eq!(threatens(Square::new(2, 3)), Vec::<Square>::new());
    }

    #[test]
    fn move_outcomes_are_empty_once_the_volley_has_fired() {
        let mut state = empty_board();
        spare_engines(&mut state);
        place(&mut state, 0, 1, PieceKind::Swordsman, 0);
        place(&mut state, 1, 3, PieceKind::Swordsman, 1);
        token(&mut state, 0, TokenKind::Row, 1, TokenFace::Attack);
        token(&mut state, 0, TokenKind::Column, 1, TokenFace::Movement);
        state.current_player = 0;
        state.pending = [TokenKind::Column, TokenKind::Row];
        state.pending_len = 1;
        state.phase = Phase::Activate;

        assert!(move_outcomes(&state)
            .iter()
            .all(|outcome| outcome.threatened_pieces.is_empty()));
    }

    #[test]
    fn slide_outcomes_name_the_movers_and_the_line_they_arm() {
        let mut state = empty_board();
        spare_engines(&mut state);
        place(&mut state, 2, 2, PieceKind::Swordsman, 0);
        place(&mut state, 2, 3, PieceKind::Swordsman, 1);
        token(&mut state, 0, TokenKind::Row, 0, TokenFace::Movement);
        token(&mut state, 0, TokenKind::Column, 6, TokenFace::Attack);
        state.current_player = 0;
        state.phase = Phase::Slide;

        let outcomes = slide_outcomes(&state);
        let at = |line: u8| {
            outcomes
                .iter()
                .find(|outcome| outcome.token == TokenKind::Row && outcome.line == line)
                .unwrap_or_else(|| panic!("no slide to rank {line}"))
        };

        let rank = at(2);
        assert_eq!(rank.movers, vec![Square::new(2, 2)]);
        assert!(rank.covered.contains(&Square::new(2, 3)));
        assert_eq!(rank.threatened_pieces, vec![Square::new(2, 3)]);

        // The siege rank arms nothing a piece could be standing on, and the
        // ram is out of reach of any castle.
        let siege = at(5);
        assert_eq!(siege.covered, Vec::<Square>::new());
        assert_eq!(siege.threatened_castles, Vec::<Square>::new());
        assert_eq!(siege.movers, vec![Square::new(5, 4), Square::new(5, 5)]);
    }

    #[test]
    fn pending_attackers_lists_the_shots_still_to_take() {
        let mut state = empty_board();
        spare_engines(&mut state);
        place(&mut state, 3, 1, PieceKind::Swordsman, 0);
        place(&mut state, 3, 4, PieceKind::Swordsman, 0);
        place(&mut state, 3, 2, PieceKind::Swordsman, 1);
        place(&mut state, 3, 5, PieceKind::Swordsman, 1);
        token(&mut state, 0, TokenKind::Row, 3, TokenFace::Attack);
        state.current_player = 0;
        state.pending = [TokenKind::Row, TokenKind::Column];
        state.pending_len = 1;
        state.phase = Phase::Activate;
        settle_state(&mut state);

        assert_eq!(state.phase, Phase::Attack);
        assert_eq!(
            pending_attackers(&state),
            vec![Square::new(3, 1), Square::new(3, 4)],
        );

        // Once the first has fired, only the second is still to be asked.
        let shot = legal_choices(&state)
            .into_iter()
            .find(|choice| matches!(choice, Choice::Attack { .. }))
            .expect("the first attacker has a shot");
        apply(&mut state, &shot);
        assert_eq!(pending_attackers(&state), vec![Square::new(3, 4)]);
    }

    /// The shortcut in `order_matters` claims to be exact, not conservative.
    /// Random play is where that claim is cheap to keep honest.
    #[test]
    fn the_idle_shortcut_agrees_with_enumerating_every_ordering() {
        use crate::rand_core::{Rng, SeedableRng};

        let mut rng = wyrand::WyRand::seed_from_u64(11);
        let mut compared = 0;

        for _ in 0..8 {
            let mut state = initial_state();
            while state.outcome.is_none() {
                if state.phase == Phase::Order {
                    let mut budget = 400_000;
                    let row = turn_endings(&state, TokenKind::Row, &mut budget);
                    let column = turn_endings(&state, TokenKind::Column, &mut budget);
                    if let (Some(row), Some(column)) = (row, column) {
                        assert_eq!(order_matters(&state), row != column);
                        compared += 1;
                    }
                }
                let choices = legal_choices(&state);
                let index = ((rng.next_u64() as u128 * choices.len() as u128) >> 64) as usize;
                apply(&mut state, &choices[index]);
            }
        }

        assert!(compared > 100, "only {compared} orders were checked");
    }

    #[test]
    fn nothing_is_pending_outside_a_volley() {
        assert_eq!(pending_attackers(&initial_state()), Vec::<Square>::new());
    }
}
