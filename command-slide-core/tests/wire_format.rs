//! The browser reads these shapes directly, so they are part of the interface
//! rather than an implementation detail. A rename that slips through here
//! breaks the UI silently at runtime.

use command_slide_core::rules::{apply_logged, GameEvent};
use command_slide_core::types::*;
use command_slide_core::{initial_state, settle_state};
use serde_json::{json, Value};

fn field(value: &Value, key: &str) -> Value {
    value
        .get(key)
        .unwrap_or_else(|| panic!("missing {key} in {value}"))
        .clone()
}

#[test]
fn game_state_serializes_with_the_keys_the_ui_reads() {
    let state = initial_state();
    let value: Value = serde_json::to_value(state).unwrap();

    for key in [
        "board",
        "castles",
        "tokens",
        "currentPlayer",
        "phase",
        "pending",
        "pendingLen",
        "attackIndex",
        "turn",
        "outcome",
    ] {
        assert!(value.get(key).is_some(), "GameState is missing {key}");
    }

    assert_eq!(field(&value, "phase"), json!("slide"));
    assert_eq!(field(&value, "outcome"), json!(null));
    assert_eq!(
        field(&value, "board")[0][3],
        json!({ "kind": "trebuchet", "owner": 0 }),
    );
    assert_eq!(field(&value, "board")[0][0], json!(null));
    assert_eq!(field(&value, "castles"), json!([[true, true, true], [true, true, true]]));
    // Index 0 is the row token, index 1 the column token, matching
    // `TokenKind::index`.
    assert_eq!(
        field(&value, "tokens")[0],
        json!([
            { "line": 0, "face": "movement" },
            { "line": 0, "face": "movement" },
        ]),
    );

    let round_tripped: GameState = serde_json::from_value(value).unwrap();
    assert_eq!(round_tripped, initial_state());
}

#[test]
fn choices_are_a_tagged_union() {
    let cases = [
        (
            Choice::Slide {
                token: TokenKind::Row,
                line: 4,
            },
            json!({ "type": "slide", "token": "row", "line": 4 }),
        ),
        (
            Choice::Order {
                first: TokenKind::Column,
            },
            json!({ "type": "order", "first": "column" }),
        ),
        (
            Choice::Move {
                from: Square::new(1, 2),
                to: Square::new(3, 2),
            },
            json!({
                "type": "move",
                "from": { "row": 1, "col": 2 },
                "to": { "row": 3, "col": 2 },
            }),
        ),
        (Choice::Pass, json!({ "type": "pass" })),
        (
            Choice::Attack {
                from: Square::new(3, 3),
                target: Square::new(6, 0),
            },
            json!({
                "type": "attack",
                "from": { "row": 3, "col": 3 },
                "target": { "row": 6, "col": 0 },
            }),
        ),
        (
            Choice::HoldFire {
                from: Square::new(3, 3),
            },
            json!({ "type": "holdFire", "from": { "row": 3, "col": 3 } }),
        ),
    ];

    for (choice, expected) in cases {
        assert_eq!(serde_json::to_value(choice).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<Choice>(expected.clone()).unwrap(),
            choice
        );
    }
}

#[test]
fn events_use_camel_case_field_names() {
    let mut state = initial_state();
    let events = apply_logged(
        &mut state,
        &Choice::Slide {
            token: TokenKind::Row,
            line: 1,
        },
    );
    assert_eq!(
        serde_json::to_value(&events[0]).unwrap(),
        json!({ "type": "slid", "player": 0, "token": "row", "from": 0, "to": 1 }),
    );

    // A volley is announced once, and every shot in it is its own event.
    let mut state = initial_state();
    state.board = [[None; BOARD_SIZE]; BOARD_SIZE];
    for (row, col, kind, owner) in [
        (3, 3, PieceKind::Trebuchet, 0),
        (3, 4, PieceKind::Swordsman, 0),
        (0, 1, PieceKind::BatteringRam, 0),
        (2, 4, PieceKind::Archer, 1),
        (6, 1, PieceKind::Trebuchet, 1),
        (6, 2, PieceKind::BatteringRam, 1),
    ] {
        state.set_piece(Square::new(row, col), Some(Piece { kind, owner }));
    }
    *state.token_mut(0, TokenKind::Row) = Token {
        line: 3,
        face: TokenFace::Attack,
    };
    state.pending = [TokenKind::Row, TokenKind::Column];
    state.pending_len = 1;
    state.phase = Phase::Activate;

    let events = settle_state(&mut state);
    assert_eq!(
        serde_json::to_value(&events[0]).unwrap(),
        json!({ "type": "volley", "player": 0, "token": "row", "line": 3 }),
    );

    // The trebuchet on the centre hilltop bears on all three enemy castles and
    // brings down one of them.
    let events = apply_logged(
        &mut state,
        &Choice::Attack {
            from: Square::new(3, 3),
            target: Square::new(6, 0),
        },
    );
    assert_eq!(
        serde_json::to_value(&events[0]).unwrap(),
        json!({
            "type": "struck",
            "player": 0,
            "token": "row",
            "kind": "trebuchet",
            "from": { "row": 3, "col": 3 },
            "target": { "row": 6, "col": 0 },
            "casualty": { "type": "castle" },
        }),
    );

    // Then the swordsman further along the rank takes the archer.
    let events = apply_logged(
        &mut state,
        &Choice::Attack {
            from: Square::new(3, 4),
            target: Square::new(2, 4),
        },
    );
    assert_eq!(
        serde_json::to_value(&events[0]).unwrap(),
        json!({
            "type": "struck",
            "player": 0,
            "token": "row",
            "kind": "swordsman",
            "from": { "row": 3, "col": 4 },
            "target": { "row": 2, "col": 4 },
            "casualty": { "type": "piece", "piece": { "kind": "archer", "owner": 1 } },
        }),
    );
}

#[test]
fn holding_fire_is_its_own_event() {
    let mut state = initial_state();
    state.board = [[None; BOARD_SIZE]; BOARD_SIZE];
    for (row, col, kind, owner) in [
        (3, 3, PieceKind::Swordsman, 0),
        (0, 1, PieceKind::Trebuchet, 0),
        (2, 3, PieceKind::Archer, 1),
        (6, 1, PieceKind::Trebuchet, 1),
    ] {
        state.set_piece(Square::new(row, col), Some(Piece { kind, owner }));
    }
    *state.token_mut(0, TokenKind::Row) = Token {
        line: 3,
        face: TokenFace::Attack,
    };
    state.pending = [TokenKind::Row, TokenKind::Column];
    state.pending_len = 1;
    state.phase = Phase::Activate;
    settle_state(&mut state);

    let events = apply_logged(
        &mut state,
        &Choice::HoldFire {
            from: Square::new(3, 3),
        },
    );
    assert_eq!(
        serde_json::to_value(&events[0]).unwrap(),
        json!({
            "type": "heldFire",
            "player": 0,
            "token": "row",
            "kind": "swordsman",
            "from": { "row": 3, "col": 3 },
        }),
    );
}

#[test]
fn outcomes_are_tagged() {
    assert_eq!(
        serde_json::to_value(Outcome::Winner { player: 1 }).unwrap(),
        json!({ "type": "winner", "player": 1 }),
    );
    assert_eq!(
        serde_json::to_value(Outcome::Draw).unwrap(),
        json!({ "type": "draw" }),
    );
}
