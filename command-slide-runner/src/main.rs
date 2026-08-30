//! Batch simulation for measuring the AI against itself.
//!
//! The evaluation in `command-slide-core` is hand-set, so the only honest way
//! to change it is to play the new numbers against the old ones and count.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

use clap::{Parser, Subcommand};
use command_slide_core::rand_core::{Rng, SeedableRng};
use command_slide_core::rules::{apply, apply_logged, legal_choices_into, Casualty, GameEvent};
use command_slide_core::search::{Ai, AiConfig, SearchContext};
use command_slide_core::types::{
    Choice, GameState, Outcome, PieceKind, Square, TokenFace, TokenKind, BOARD_COLS, BOARD_ROWS,
};
use command_slide_core::{initial_state, EvalParams};
use wyrand::WyRand;

mod tune;

#[derive(Parser)]
#[command(name = "command-slide-runner", about = "Command Slide batch simulation")]
struct Cli {
    /// Games to play in parallel.
    #[arg(long, default_value_t = 8, global = true)]
    threads: usize,
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Play two AI configurations head to head, alternating who moves first.
    Simulate {
        #[arg(long, default_value_t = 20)]
        games: usize,
        /// Search iterations per decision for configuration A.
        #[arg(long, default_value_t = 4_000)]
        iterations_a: u32,
        /// Search iterations per decision for configuration B.
        #[arg(long, default_value_t = 4_000)]
        iterations_b: u32,
        /// Rollout plies for configuration A.
        #[arg(long, default_value_t = 8)]
        rollout_a: u32,
        /// Rollout plies for configuration B.
        #[arg(long, default_value_t = 8)]
        rollout_b: u32,
        /// Evaluation weights for configuration A, as JSON. Defaults to the
        /// built-in weights, so this is how a tuning run's output gets judged.
        #[arg(long)]
        params_a: Option<std::path::PathBuf>,
        /// Evaluation weights for configuration B, as JSON.
        #[arg(long)]
        params_b: Option<std::path::PathBuf>,
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
    /// Search the evaluation weights for stronger ones by self-play.
    Tune {
        /// Which optimizer proposes candidates.
        #[arg(long, value_enum, default_value_t = tune::Strategy::CmaEs)]
        strategy: tune::Strategy,
        /// Who candidates are measured against.
        #[arg(long, value_enum, default_value_t = tune::Field::Baseline)]
        field: tune::Field,
        #[arg(long, default_value_t = 50)]
        generations: usize,
        /// Games behind each candidate's win rate. The most important number
        /// here: below about 200 a generation is ranking luck, because a win
        /// rate over n games carries a standard error of up to sqrt(0.25 / n).
        #[arg(long, default_value_t = 400)]
        games_per_eval: usize,
        /// Candidates per generation. Zero lets CMA-ES pick the standard
        /// 4 + floor(3 ln n); the GA falls back to 20.
        #[arg(long, default_value_t = 0)]
        population: usize,
        /// Search iterations in each evaluation game. Weights matter most when
        /// the tree is shallow, so a run at a budget far below the shipping one
        /// is tuning for a regime the game does not play in.
        #[arg(long, default_value_t = 4_000)]
        eval_iterations: u32,
        #[arg(long, default_value_t = 8)]
        rollout_plies: u32,
        /// Draw fresh game seeds each generation, so a candidate cannot be
        /// selected for suiting one fixed set of games.
        #[arg(long)]
        reseed: bool,
        /// Weights to start from, as JSON. Defaults to the built-in ones.
        #[arg(long)]
        seed_params: Option<std::path::PathBuf>,
        #[arg(long, default_value = "tuning")]
        output: std::path::PathBuf,
        /// Continue the run whose checkpoint is in --output, rather than
        /// starting one. Pass the same --seed-params the original run used:
        /// fitness is a win rate against those weights, so resuming against
        /// different ones is refused.
        #[arg(long)]
        resume: bool,
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
    /// Uniformly random self-play. A cheap check that the rules terminate and
    /// that both win conditions are reachable without a search.
    Random {
        #[arg(long, default_value_t = 500)]
        games: usize,
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
    /// Play a single game and print every decision and position.
    Replay {
        #[arg(long, default_value_t = 3_000)]
        iterations: u32,
        #[arg(long, default_value_t = 1)]
        seed: u64,
    },
    /// Search throughput at the opening position.
    Bench {
        #[arg(long, default_value_t = 20_000)]
        iterations: u32,
    },
}

/// How a finished game finished.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum EndReason {
    #[default]
    TurnLimit,
    CastlesRazed,
    SiegeEliminated,
}

#[derive(Clone, Copy, Default)]
struct GameResult {
    outcome: Option<Outcome>,
    reason: EndReason,
    turns: u32,
    decisions: u32,
}

impl GameResult {
    fn of(state: &GameState, decisions: u32) -> Self {
        let reason = match state.outcome {
            Some(Outcome::Winner { player }) => {
                let loser = GameState::opponent(player);
                if state.castles_standing(loser) == 0 {
                    EndReason::CastlesRazed
                } else {
                    EndReason::SiegeEliminated
                }
            }
            _ => EndReason::TurnLimit,
        };
        Self {
            outcome: state.outcome,
            reason,
            turns: state.turn,
            decisions,
        }
    }
}

fn config(iterations: u32, rollout_plies: u32, params: EvalParams) -> AiConfig {
    AiConfig {
        iterations,
        context: SearchContext {
            params,
            rollout_plies,
        },
        ..AiConfig::default()
    }
}

/// One complete game. `configs[p]` drives board player `p`.
fn play_game(configs: &[AiConfig; 2], seed: u64) -> GameResult {
    let mut state = initial_state();
    let mut rng = WyRand::seed_from_u64(seed);
    let mut ais = [Ai::new(&state), Ai::new(&state)];
    let mut decisions = 0;

    while state.outcome.is_none() {
        let player = state.current_player;
        let choice = ais[player as usize]
            .search(&state, player, &configs[player as usize], None, &mut rng)
            .choice;
        apply(&mut state, &choice);
        for ai in &mut ais {
            ai.advance(&choice);
        }
        decisions += 1;
    }
    GameResult::of(&state, decisions)
}

fn play_random(seed: u64) -> GameResult {
    let mut state = initial_state();
    let mut rng = WyRand::seed_from_u64(seed);
    let mut choices: Vec<Choice> = Vec::new();
    let mut decisions = 0;

    while state.outcome.is_none() {
        legal_choices_into(&state, &mut choices);
        if choices.is_empty() {
            break;
        }
        let index = ((rng.next_u64() as u128 * choices.len() as u128) >> 64) as usize;
        let choice = choices[index];
        apply(&mut state, &choice);
        decisions += 1;
    }
    GameResult::of(&state, decisions)
}

/// Run `total` independent jobs across `threads` workers, in index order.
fn run_parallel<T, F>(total: usize, threads: usize, job: F) -> Vec<T>
where
    T: Send,
    F: Fn(usize) -> T + Sync,
{
    let next = AtomicUsize::new(0);
    let job = &job;
    let next = &next;

    let mut collected: Vec<(usize, T)> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads.max(1))
            .map(|_| {
                scope.spawn(move || {
                    let mut local = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        if index >= total {
                            return local;
                        }
                        local.push((index, job(index)));
                    }
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("worker panicked"))
            .collect()
    });

    collected.sort_by_key(|&(index, _)| index);
    collected.into_iter().map(|(_, value)| value).collect()
}

fn summarize(results: &[GameResult], swapped_from: usize) {
    let mut wins = [0usize; 2];
    let mut draws = 0usize;
    let mut razed = 0usize;
    let mut disarmed = 0usize;

    for (index, result) in results.iter().enumerate() {
        // Past `swapped_from` the configurations changed seats, so a win by
        // board player 1 is a win for configuration A.
        let swapped = index >= swapped_from;
        match result.outcome {
            Some(Outcome::Winner { player }) => {
                let config = if swapped { GameState::opponent(player) } else { player };
                wins[config as usize] += 1;
                match result.reason {
                    EndReason::CastlesRazed => razed += 1,
                    EndReason::SiegeEliminated => disarmed += 1,
                    EndReason::TurnLimit => {}
                }
            }
            _ => draws += 1,
        }
    }

    let games = results.len() as f64;
    let percent = |count: usize| 100.0 * count as f64 / games;
    println!(
        "A {} ({:.1}%)   B {} ({:.1}%)   draws {} ({:.1}%)",
        wins[0],
        percent(wins[0]),
        wins[1],
        percent(wins[1]),
        draws,
        percent(draws),
    );
    println!(
        "won by razing castles: {razed}   by eliminating siege engines: {disarmed}",
    );
    println!(
        "mean turns {:.1}   mean decisions {:.1}",
        results.iter().map(|r| r.turns as f64).sum::<f64>() / games,
        results.iter().map(|r| r.decisions as f64).sum::<f64>() / games,
    );
}

/// One letter per piece. Light is upper case, Dark lower.
fn glyph(kind: PieceKind, owner: u8) -> char {
    let letter = match kind {
        PieceKind::Swordsman => 'S',
        PieceKind::Flail => 'F',
        PieceKind::Spearman => 'P',
        PieceKind::Archer => 'A',
        PieceKind::Trebuchet => 'T',
        PieceKind::BatteringRam => 'R',
    };
    if owner == 0 {
        letter
    } else {
        letter.to_ascii_lowercase()
    }
}

/// The board, with each side's tokens shown on the edges they ride: Light on
/// the left and top, Dark on the right and bottom.
fn render(state: &GameState) -> String {
    let face = |player: u8, kind: TokenKind| {
        match state.token(player, kind).face {
            TokenFace::Movement => if kind == TokenKind::Row { "R>" } else { "B>" },
            TokenFace::Attack => "!!",
        }
    };

    let mut out = String::new();
    let column_marker = |player: u8, col: u8| {
        if state.token(player, TokenKind::Column).line == col {
            face(player, TokenKind::Column)
        } else {
            "  "
        }
    };

    out.push_str("      ");
    for col in 0..BOARD_COLS as u8 {
        out.push_str(column_marker(0, col));
        out.push(' ');
    }
    out.push('\n');

    for row in 0..BOARD_ROWS as u8 {
        let left = if state.token(0, TokenKind::Row).line == row {
            face(0, TokenKind::Row)
        } else {
            "  "
        };
        out.push_str(&format!("{left} {row} |"));
        for col in 0..BOARD_COLS as u8 {
            let square = Square::new(row, col);
            let cell = match state.piece_at(square) {
                Some(piece) => glyph(piece.kind, piece.owner),
                None if state.standing_wall_at(square).is_some() => 'O',
                None => match GameState::castle_slot_at(square) {
                    Some((owner, index)) if state.castles[owner as usize][index] => '#',
                    Some(_) => 'x',
                    None if state.is_hilltop(square) => '^',
                    None => '.',
                },
            };
            out.push(' ');
            out.push(cell);
        }
        let right = if state.token(1, TokenKind::Row).line == row {
            face(1, TokenKind::Row)
        } else {
            "  "
        };
        out.push_str(&format!(" | {right}\n"));
    }

    out.push_str("      ");
    for col in 0..BOARD_COLS as u8 {
        out.push_str(column_marker(1, col));
        out.push(' ');
    }
    out.push('\n');
    out.push_str(
        "      # castle  x razed castle, now a hilltop  ^ hilltop  O wall  R>/B> movement face  !! attack face\n",
    );
    out
}

fn describe(event: &GameEvent) -> String {
    match event {
        GameEvent::Slid { player, token, from, to } => {
            format!("  P{player} slides its {token:?} token from {from} to {to}")
        }
        GameEvent::Moved { player, kind, from, to, .. } => format!(
            "  P{player} moves {kind:?} ({},{}) -> ({},{})",
            from.row, from.col, to.row, to.col
        ),
        GameEvent::Passed { player, token } => {
            format!("  P{player} takes no action with its {token:?} token")
        }
        GameEvent::Volley { player, token, line } => {
            format!("  P{player} volleys with {token:?} {line}")
        }
        GameEvent::Struck { player, kind, from, target, casualty, .. } => {
            let shot = format!("  P{player} {kind:?} ({},{})", from.row, from.col);
            match casualty {
                Casualty::Piece { piece } => format!(
                    "{shot} kills {:?} at ({},{})",
                    piece.kind, target.row, target.col
                ),
                Casualty::Castle => {
                    format!("{shot} razes the castle at ({},{})", target.row, target.col)
                }
                Casualty::Wall => {
                    format!("{shot} breaks the wall at ({},{})", target.row, target.col)
                }
            }
        }
        GameEvent::HeldFire { player, kind, from, .. } => {
            format!("  P{player} {kind:?} ({},{}) holds its fire", from.row, from.col)
        }
        GameEvent::TurnEnded { .. } => String::new(),
        GameEvent::GameOver { outcome } => format!("  game over: {outcome:?}"),
    }
}

fn replay(iterations: u32, seed: u64) {
    let mut state = initial_state();
    let mut rng = WyRand::seed_from_u64(seed);
    let config = config(iterations, 8, EvalParams::default());
    let mut ais = [Ai::new(&state), Ai::new(&state)];

    println!("{}", render(&state));

    while state.outcome.is_none() {
        let player = state.current_player;
        let turn_start = state.turn;
        while state.current_player == player && state.outcome.is_none() {
            let choice = ais[player as usize]
                .search(&state, player, &config, None, &mut rng)
                .choice;
            for event in apply_logged(&mut state, &choice) {
                let line = describe(&event);
                if !line.is_empty() {
                    println!("{line}");
                }
            }
            for ai in &mut ais {
                ai.advance(&choice);
            }
        }
        println!("--- after turn {turn_start}, player {player} ---");
        println!("{}", render(&state));
    }

    println!("{:?} in {} turns", state.outcome.unwrap(), state.turn);
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Simulate {
            games,
            iterations_a,
            iterations_b,
            rollout_a,
            rollout_b,
            params_a,
            params_b,
            seed,
        } => {
            let weights = |path: &Option<std::path::PathBuf>| {
                path.as_deref().map(tune::load_params).unwrap_or_default()
            };
            let a = config(iterations_a, rollout_a, weights(&params_a));
            let b = config(iterations_b, rollout_b, weights(&params_b));
            // Half the games with A moving first, half with B, so the
            // first-move advantage cancels rather than being measured.
            let half = games / 2;
            let started = Instant::now();
            let results = run_parallel(games, cli.threads, |index| {
                let configs = if index < half { [a, b] } else { [b, a] };
                play_game(&configs, seed.wrapping_add(index as u64))
            });
            let named = |path: &Option<std::path::PathBuf>| match path {
                Some(path) => path.display().to_string(),
                None => String::from("built-in weights"),
            };
            println!(
                "A: {iterations_a} iterations, {rollout_a} rollout plies, {}",
                named(&params_a)
            );
            println!(
                "B: {iterations_b} iterations, {rollout_b} rollout plies, {}",
                named(&params_b)
            );
            summarize(&results, half);
            println!("{games} games in {:.1}s", started.elapsed().as_secs_f64());
        }
        Command::Random { games, seed } => {
            let started = Instant::now();
            let results = run_parallel(games, cli.threads, |index| {
                play_random(seed.wrapping_add(index as u64))
            });
            summarize(&results, games);
            println!("{games} games in {:.1}s", started.elapsed().as_secs_f64());
        }
        Command::Tune {
            strategy,
            field,
            generations,
            games_per_eval,
            population,
            eval_iterations,
            rollout_plies,
            reseed,
            seed_params,
            output,
            resume,
            seed,
        } => tune::run(&tune::TuneArgs {
            strategy,
            field,
            generations,
            games_per_eval,
            population,
            eval_iterations,
            rollout_plies,
            seed,
            reseed_each_generation: reseed,
            seed_params,
            output,
            threads: cli.threads,
            resume,
        }),
        Command::Replay { iterations, seed } => replay(iterations, seed),
        Command::Bench { iterations } => {
            let state = initial_state();
            let config = config(iterations, 8, EvalParams::default());
            let mut rng = WyRand::seed_from_u64(0xC0FFEE);
            let started = Instant::now();
            let result = Ai::new(&state).search(&state, 0, &config, None, &mut rng);
            let elapsed = started.elapsed().as_secs_f64();
            println!(
                "{} iterations in {:.2}s ({:.0}/s), stopped on {:?}",
                result.iterations_used,
                elapsed,
                result.iterations_used as f64 / elapsed,
                result.stop_reason,
            );
            println!(
                "best {:?} at mean reward {:.3} over {} visits",
                result.choice, result.best_mean_reward, result.best_visits,
            );
        }
    }
}
