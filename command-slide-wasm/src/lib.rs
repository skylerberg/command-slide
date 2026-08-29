//! Browser bindings. Every function takes and returns JSON so the TypeScript
//! side never has to mirror a Rust memory layout.

use command_slide_core::rand_core::SeedableRng;
use command_slide_core::preview::{forced_order, move_outcomes, pending_attackers, slide_outcomes};
use command_slide_core::rules::{apply_logged, attack_preview, GameEvent};
use command_slide_core::search::{Ai, AiConfig};
use command_slide_core::types::{Choice, GameState, Square, TokenKind, WALL_SQUARES};
use command_slide_core::{initial_state, legal_choices, AttackReach};
use serde::Serialize;
use wasm_bindgen::prelude::*;
use wyrand::WyRand;

/// `getrandom` needs build-time configuration to reach the browser's entropy;
/// `Math.random` is already here and a search seed does not need to be
/// unpredictable to an adversary.
fn seed_rng() -> WyRand {
    let high = (js_sys::Math::random() * u32::MAX as f64) as u64;
    let low = (js_sys::Math::random() * u32::MAX as f64) as u64;
    WyRand::seed_from_u64((high << 32) | low)
}

fn parse_state(json: &str) -> GameState {
    serde_json::from_str(json).expect("failed to parse game state")
}

fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("failed to serialize")
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StateWithEvents {
    state: GameState,
    events: Vec<GameEvent>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AttackPreview {
    attackers: Vec<Square>,
    covered: Vec<Square>,
    threatened_pieces: Vec<Square>,
    threatened_castles: Vec<Square>,
    threatened_walls: Vec<Square>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiTurn {
    choices: Vec<Choice>,
    state: GameState,
    events: Vec<GameEvent>,
}

#[wasm_bindgen]
pub fn wasm_initial_state() -> String {
    to_json(&initial_state())
}

#[wasm_bindgen]
pub fn wasm_legal_choices(state_json: &str) -> String {
    to_json(&legal_choices(&parse_state(state_json)))
}

#[wasm_bindgen]
pub fn wasm_apply_choice(state_json: &str, choice_json: &str) -> String {
    let mut state = parse_state(state_json);
    let choice: Choice = serde_json::from_str(choice_json).expect("failed to parse choice");
    let events = apply_logged(&mut state, &choice);
    to_json(&StateWithEvents { state, events })
}

/// What an attack with this player's token bears on right now. Each attacker
/// takes one of these, so it is the volley's reach rather than its casualty
/// list. The UI draws it as a threat overlay.
#[wasm_bindgen]
pub fn wasm_attack_preview(state_json: &str, player: u8, token_json: &str) -> String {
    let state = parse_state(state_json);
    let token: TokenKind = serde_json::from_str(token_json).expect("failed to parse token");
    let reach = attack_preview(&state, player, token);
    let enemy = GameState::opponent(player);
    to_json(&AttackPreview {
        attackers: AttackReach::squares(reach.attackers).collect(),
        covered: AttackReach::squares(reach.covered).collect(),
        threatened_pieces: AttackReach::squares(reach.pieces).collect(),
        threatened_castles: reach
            .castles
            .iter()
            .enumerate()
            .filter(|(_, &hit)| hit)
            .map(|(index, _)| GameState::castle_square(enemy, index))
            .collect(),
        threatened_walls: reach
            .walls
            .iter()
            .enumerate()
            .filter(|(_, &hit)| hit)
            .map(|(index, _)| WALL_SQUARES[index])
            .collect(),
    })
}

/// The activation order the interface should take on the player's behalf, or
/// `null` when which token goes first is a decision worth putting to them.
#[wasm_bindgen]
pub fn wasm_forced_order(state_json: &str) -> String {
    to_json(&forced_order(&parse_state(state_json)))
}

/// Every move in the current activation, paired with what it puts within reach
/// of the volley still to come.
#[wasm_bindgen]
pub fn wasm_move_outcomes(state_json: &str) -> String {
    to_json(&move_outcomes(&parse_state(state_json)))
}

/// Every slide available, paired with the pieces it would free to move and the
/// squares it would arm for next turn.
#[wasm_bindgen]
pub fn wasm_slide_outcomes(state_json: &str) -> String {
    to_json(&slide_outcomes(&parse_state(state_json)))
}

/// The attackers of the volley in progress that have still to fire.
#[wasm_bindgen]
pub fn wasm_pending_attackers(state_json: &str) -> String {
    to_json(&pending_attackers(&parse_state(state_json)))
}

/// One search for the single decision in front of the player.
#[wasm_bindgen]
pub fn wasm_ai_choose(state_json: &str, player: u8, iterations: u32) -> String {
    let state = parse_state(state_json);
    let config = AiConfig::with_iterations(iterations);
    let mut rng = seed_rng();
    let choice = Ai::new(&state)
        .search(&state, player, &config, None, &mut rng)
        .choice;
    to_json(&choice)
}

/// Play the AI's whole turn — slide, activation order, piece move — and hand
/// back every choice it made along with the resulting position.
///
/// A turn is three decisions, so the alternative is three round trips through
/// the worker for one move. Searching them here also lets each decision inherit
/// the previous one's subtree, which a round trip through JSON cannot.
#[wasm_bindgen]
pub fn wasm_ai_take_turn(state_json: &str, iterations: u32) -> String {
    let mut state = parse_state(state_json);
    let player = state.current_player;
    let config = AiConfig::with_iterations(iterations);
    let mut rng = seed_rng();
    let mut ai = Ai::new(&state);

    let mut choices = Vec::new();
    let mut events = Vec::new();

    while state.outcome.is_none() && state.current_player == player {
        let choice = ai.search(&state, player, &config, None, &mut rng).choice;
        events.extend(apply_logged(&mut state, &choice));
        ai.advance(&choice);
        choices.push(choice);
    }

    to_json(&AiTurn {
        choices,
        state,
        events,
    })
}
