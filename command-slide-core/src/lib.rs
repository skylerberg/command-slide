//! Command Slide: the complete game engine.
//!
//! Every rule lives here. The wasm bindings, the runner, and the browser all
//! consume this crate rather than reimplementing any part of it.

/// Re-exported so consumers can seed a search without depending on `mcts`
/// or on a particular `rand` version directly.
pub use mcts::rand_core;

pub mod rules;
pub mod search;
pub mod types;

pub use rules::{
    apply, apply_logged, apply_with, attack_preview, initial_state, legal_choices,
    legal_choices_into, settle_state, AttackResult, EventSink, GameEvent,
};
pub use search::{evaluate, side_score, Ai, AiConfig, EvalParams, SearchContext};
pub use types::*;
