//! The game's side of the `mcts` contract, plus the evaluation the search
//! leans on.
//!
//! Command Slide is deterministic and perfect information, so most of the
//! trait falls away: `determinize_into` is a copy, `Side` is `()`, and the root
//! choices provably cannot vary between iterations. What does need care is the
//! leaf evaluation. Both win conditions sit far outside a random rollout's
//! reach — random play will not walk a trebuchet onto a hilltop or a ram up to
//! a castle wall — so an uninformed rollout returns a draw almost every time
//! and the search learns nothing. The evaluation below supplies the gradient
//! that random play cannot find.

use std::cell::RefCell;
use std::sync::atomic::AtomicBool;

use mcts::rand_core::Rng;
use mcts::{Config, Game, SearchResult, Searcher, Status};
use serde::{Deserialize, Serialize};

use crate::rules::{apply, legal_choices_into};
use crate::types::*;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvalParams {
    pub castle: f64,
    pub trebuchet: f64,
    pub battering_ram: f64,
    pub swordsman: f64,
    pub flail: f64,
    pub spearman: f64,
    pub archer: f64,
    /// A trebuchet standing on a hilltop is a live siege weapon; off one it is
    /// an expensive spectator.
    pub hilltop: f64,
    /// Per enemy castle a hilltop trebuchet currently bears on.
    pub castle_threat: f64,
    /// Per enemy castle a battering ram currently bears on.
    pub ram_threat: f64,
    /// Per square of distance from a trebuchet to the nearest hilltop.
    pub trebuchet_approach: f64,
    /// Per square of distance from a ram to the nearest standing enemy castle.
    pub ram_approach: f64,
    /// Charged to a side down to its last siege engine, which is one attack
    /// away from being unable to win at all.
    pub last_siege_engine: f64,
    /// Divides the score difference before it is squashed into `[0, 1]`.
    pub scale: f64,
}

impl Default for EvalParams {
    fn default() -> Self {
        Self {
            castle: 12.0,
            trebuchet: 9.0,
            battering_ram: 7.0,
            swordsman: 2.0,
            flail: 2.0,
            spearman: 2.4,
            archer: 2.4,
            hilltop: 4.0,
            castle_threat: 2.5,
            ram_threat: 3.0,
            trebuchet_approach: 0.6,
            ram_approach: 0.5,
            last_siege_engine: 4.0,
            scale: 14.0,
        }
    }
}

impl EvalParams {
    pub fn piece_value(&self, kind: PieceKind) -> f64 {
        match kind {
            PieceKind::Swordsman => self.swordsman,
            PieceKind::Flail => self.flail,
            PieceKind::Spearman => self.spearman,
            PieceKind::Archer => self.archer,
            PieceKind::Trebuchet => self.trebuchet,
            PieceKind::BatteringRam => self.battering_ram,
        }
    }
}

/// Standing enemy castles a piece of `kind` on `from` would hit right now.
pub fn castles_in_range(state: &GameState, from: Square, enemy: u8, kind: PieceKind) -> usize {
    if kind == PieceKind::Trebuchet && !GameState::is_hilltop(from) {
        return 0;
    }
    kind.attack_offsets()
        .iter()
        .filter_map(|&(drow, dcol)| from.offset(drow, dcol))
        .filter_map(GameState::castle_slot_at)
        .filter(|&(owner, index)| owner == enemy && state.castles[owner as usize][index])
        .count()
}

fn distance_to_hilltop(from: Square) -> u8 {
    HILLTOP_COLS
        .iter()
        .map(|&col| from.chebyshev(Square::new(HILLTOP_ROW, col)))
        .min()
        .expect("the board has hilltops")
}

fn distance_to_enemy_castle(state: &GameState, from: Square, enemy: u8) -> u8 {
    (0..CASTLES_PER_PLAYER)
        .filter(|&index| state.castles[enemy as usize][index])
        .map(|index| from.chebyshev(GameState::castle_square(enemy, index)))
        .min()
        .unwrap_or(0)
}

/// One side's standing in arbitrary units. Only the difference between the two
/// sides is ever used.
pub fn side_score(state: &GameState, player: u8, params: &EvalParams) -> f64 {
    let enemy = GameState::opponent(player);
    let mut score = params.castle * state.castles_standing(player) as f64;
    let mut siege_engines = 0;

    for (square, piece) in state.pieces_of(player) {
        score += params.piece_value(piece.kind);
        match piece.kind {
            PieceKind::Trebuchet => {
                siege_engines += 1;
                if GameState::is_hilltop(square) {
                    score += params.hilltop
                        + params.castle_threat
                            * castles_in_range(state, square, enemy, piece.kind) as f64;
                } else {
                    score -= params.trebuchet_approach * distance_to_hilltop(square) as f64;
                }
            }
            PieceKind::BatteringRam => {
                siege_engines += 1;
                let threatened = castles_in_range(state, square, enemy, piece.kind);
                if threatened > 0 {
                    score += params.ram_threat * threatened as f64;
                } else {
                    score -=
                        params.ram_approach * distance_to_enemy_castle(state, square, enemy) as f64;
                }
            }
            _ => {}
        }
    }

    if siege_engines == 1 {
        score -= params.last_siege_engine;
    }
    score
}

pub fn terminal_rewards(outcome: Outcome) -> [f64; 2] {
    match outcome {
        Outcome::Winner { player } => {
            let mut rewards = [0.0; 2];
            rewards[player as usize] = 1.0;
            rewards
        }
        Outcome::Draw => [0.5, 0.5],
    }
}

/// Rewards in `[0, 1]`, which is the range `Config::min_reward` and
/// `Config::max_reward` are set to.
pub fn evaluate(state: &GameState, params: &EvalParams) -> [f64; 2] {
    if let Some(outcome) = state.outcome {
        return terminal_rewards(outcome);
    }
    let difference = side_score(state, 0, params) - side_score(state, 1, params);
    let light = 0.5 + 0.5 * (difference / params.scale).tanh();
    [light, 1.0 - light]
}

/// Constant for a whole search. Held out of `GameState` so it is not copied on
/// every determinization.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SearchContext {
    pub params: EvalParams,
    /// Plies of random play before the leaf is evaluated. Zero evaluates the
    /// leaf directly.
    pub rollout_plies: u32,
}

impl Default for SearchContext {
    fn default() -> Self {
        Self {
            params: EvalParams::default(),
            rollout_plies: 8,
        }
    }
}

thread_local! {
    /// Reused across rollouts so the hot path does not allocate. Nothing
    /// reached from a rollout touches this again, so the borrow cannot nest.
    static ROLLOUT_CHOICES: RefCell<Vec<Choice>> = const { RefCell::new(Vec::new()) };
}

#[inline]
fn uniform_index<R: Rng + ?Sized>(rng: &mut R, len: usize) -> usize {
    ((rng.next_u64() as u128 * len as u128) >> 64) as usize
}

impl Game for GameState {
    type Choice = Choice;
    type Rewards = [f64; 2];
    type Context = SearchContext;
    type Side = ();

    /// Safe here in the strong sense rather than the plausible one:
    /// `determinize_into` is a bitwise copy, so every iteration searches the
    /// identical root position and the legal set is the same list every time.
    const ROOT_CHOICES_INVARIANT: bool = true;

    fn status(&self, _ctx: &SearchContext) -> Status<[f64; 2]> {
        match self.outcome {
            Some(outcome) => Status::Terminal(terminal_rewards(outcome)),
            None => Status::Active {
                player: self.current_player,
            },
        }
    }

    fn choices_into(&self, _ctx: &SearchContext, out: &mut Vec<Choice>) {
        legal_choices_into(self, out);
    }

    fn apply_choice<R: Rng + ?Sized>(
        &mut self,
        _ctx: &SearchContext,
        choice: &Choice,
        _rng: &mut R,
    ) {
        apply(self, choice);
    }

    fn rollout<R: Rng + ?Sized>(&mut self, ctx: &SearchContext, rng: &mut R) -> [f64; 2] {
        ROLLOUT_CHOICES.with_borrow_mut(|buf| {
            for _ in 0..ctx.rollout_plies {
                if self.outcome.is_some() {
                    break;
                }
                legal_choices_into(self, buf);
                if buf.is_empty() {
                    break;
                }
                let choice = buf[uniform_index(rng, buf.len())];
                apply(self, &choice);
            }
        });
        evaluate(self, &ctx.params)
    }

    fn new_buffer(&self) -> Self {
        *self
    }

    fn determinize_into<R: Rng + ?Sized>(
        &self,
        dest: &mut Self,
        _ctx: &SearchContext,
        _perspective: u8,
        _rng: &mut R,
    ) {
        *dest = *self;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub iterations: u32,
    /// Only honoured when `command-slide-core` is built with the `time`
    /// feature; `wasm32` builds are not, because `Instant` panics there.
    pub time_limit_ms: Option<u64>,
    pub exploration_constant: f64,
    pub context: SearchContext,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            iterations: 20_000,
            time_limit_ms: None,
            exploration_constant: 0.75,
            context: SearchContext::default(),
        }
    }
}

impl AiConfig {
    pub fn with_iterations(iterations: u32) -> Self {
        Self {
            iterations,
            ..Self::default()
        }
    }

    fn mcts_config(&self) -> Config {
        Config {
            iterations: self.iterations,
            time_limit_ms: self.time_limit_ms,
            exploration_constant: self.exploration_constant,
            progressive_bias_weight: 0.0,
            early_termination: true,
            max_reward: 1.0,
            min_reward: 0.0,
            ..Config::default()
        }
    }
}

/// A searcher that keeps its tree between moves.
///
/// Worth using from the runner, where one process plays a whole game. The
/// browser rebuilds the state from JSON on every call and so gets no reuse;
/// [`choose_move`] is the entry point there.
pub struct Ai {
    searcher: Searcher<GameState>,
}

impl Ai {
    pub fn new(state: &GameState) -> Self {
        Self {
            searcher: Searcher::new(state),
        }
    }

    pub fn search<R: Rng + ?Sized>(
        &mut self,
        state: &GameState,
        player: u8,
        cfg: &AiConfig,
        cancel: Option<&AtomicBool>,
        rng: &mut R,
    ) -> SearchResult<Choice> {
        self.searcher.search(
            state,
            &cfg.context,
            player,
            &cfg.mcts_config(),
            cancel,
            rng,
        )
    }

    /// Keep the subtree under `choice` for the next search.
    pub fn advance(&mut self, choice: &Choice) {
        self.searcher.reuse_subtree(choice);
    }
}

/// One search from scratch. Every decision within a turn — the slide, the
/// activation order, the piece move — is a separate call.
pub fn choose_move<R: Rng + ?Sized>(
    state: &GameState,
    player: u8,
    cfg: &AiConfig,
    rng: &mut R,
) -> Choice {
    Ai::new(state).search(state, player, cfg, None, rng).choice
}
