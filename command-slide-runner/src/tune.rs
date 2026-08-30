//! Tuning the leaf evaluation against itself.
//!
//! The weights in [`EvalParams`] were set by hand, and hand-set numbers in a
//! fourteen-dimensional space are never the best ones available. This wires
//! `mcts-tune` up to them: candidates are proposed, each plays a few hundred
//! games against the parameters the run started from, and the win rates drive
//! the search.
//!
//! Read the output with the noise in mind. A win rate over `n` games carries a
//! standard error of up to `sqrt(0.25 / n)` — 2.5 percentage points at 400
//! games — so a generation's reported best is a maximum over noisy
//! measurements and is biased upward by construction. `simulate --params-a`
//! against the seed file is what turns a shortlist into a result.

use std::fs;
use std::path::{Path, PathBuf};

use command_slide_core::rand_core::Rng;
use command_slide_core::search::{AiConfig, SearchContext};
use command_slide_core::types::GameState;
use command_slide_core::{initial_state, EvalParams};
use mcts::Config;
use mcts_tune::{
    CmaEs, CmaParams, Evaluation, Ga, GaParams, Match, Optimizer, Tunable, TuneConfig,
};

/// The genes, in order. `scale` is deliberately absent.
///
/// `evaluate` returns `tanh((light_score - dark_score) / scale)`, and both
/// scores are linear in these weights, so multiplying every weight *and*
/// `scale` by the same constant produces a bit-identical evaluation at every
/// position. Handing an optimizer both ends of that family gives it an exact
/// flat direction to wander along, spending hundreds of games per generation on
/// candidates that cannot play differently from one another. Pinning `scale`
/// at its seed value removes the redundancy and leaves fourteen real degrees of
/// freedom.
const GENES: &[&str] = &[
    "castle",
    "trebuchet",
    "batteringRam",
    "swordsman",
    "flail",
    "spearman",
    "archer",
    "hilltop",
    "castleThreat",
    "castleThreatExtra",
    "ramThreat",
    "trebuchetApproach",
    "ramApproach",
    "lastSiegeEngine",
];

/// Above this a weight saturates the `tanh` at every position, which flattens
/// the gradient the search climbs instead of steepening it. Nothing useful
/// lives out there, and candidates sent there are games spent learning that
/// twice.
const MAX_WEIGHT: f64 = 100.0;

/// The evaluation weights, as something this crate is allowed to implement a
/// foreign trait for.
///
/// [`EvalParams`] belongs to `command-slide-core` and [`Tunable`] to
/// `mcts-tune`, so the impl has to hang off a local type. A newtype here rather
/// than the trait in the core crate: `mcts-tune` pulls in threads and whole-game
/// play, and the core crate compiles to wasm.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weights(pub EvalParams);

impl Tunable for Weights {
    fn gene_names() -> &'static [&'static str] {
        GENES
    }

    fn to_genes(&self) -> Vec<f64> {
        let params = &self.0;
        vec![
            params.castle,
            params.trebuchet,
            params.battering_ram,
            params.swordsman,
            params.flail,
            params.spearman,
            params.archer,
            params.hilltop,
            params.castle_threat,
            params.castle_threat_extra,
            params.ram_threat,
            params.trebuchet_approach,
            params.ram_approach,
            params.last_siege_engine,
        ]
    }

    fn with_genes(&self, genes: &[f64]) -> Self {
        Weights(EvalParams {
            castle: genes[0],
            trebuchet: genes[1],
            battering_ram: genes[2],
            swordsman: genes[3],
            flail: genes[4],
            spearman: genes[5],
            archer: genes[6],
            hilltop: genes[7],
            castle_threat: genes[8],
            castle_threat_extra: genes[9],
            ram_threat: genes[10],
            trebuchet_approach: genes[11],
            ram_approach: genes[12],
            last_siege_engine: genes[13],
            // Pinned. See `GENES`.
            scale: self.0.scale,
        })
    }

    /// Every gene is the magnitude of an effect whose direction the formula
    /// already fixes — `trebuchet_approach` is subtracted, so a negative one
    /// would pay a trebuchet to walk away from the hilltops. Holding them at or
    /// above zero keeps the evaluation meaning what it says.
    fn repair(genes: &mut [f64]) {
        for gene in genes {
            *gene = gene.clamp(0.0, MAX_WEIGHT);
        }
    }
}

/// Self-play from the standard opening.
///
/// The opening is the same every game, which is not the limitation it looks
/// like: Command Slide has one starting position, so that *is* the distribution
/// these weights have to be good on. Games still differ, because the search's
/// own randomness differs with the seed.
pub struct EvalMatch {
    base: Weights,
    config: AiConfig,
}

impl EvalMatch {
    pub fn new(base: EvalParams, iterations: u32, rollout_plies: u32) -> Self {
        Self {
            base: Weights(base),
            config: AiConfig {
                iterations,
                context: SearchContext {
                    params: base,
                    rollout_plies,
                },
                ..AiConfig::default()
            },
        }
    }
}

impl Match for EvalMatch {
    type Game = GameState;
    type Params = Weights;

    fn base(&self) -> &Weights {
        &self.base
    }

    fn initial_state<R: Rng + ?Sized>(&self, _rng: &mut R) -> GameState {
        initial_state()
    }

    fn context(&self, params: &Weights) -> SearchContext {
        SearchContext {
            params: params.0,
            rollout_plies: self.config.context.rollout_plies,
        }
    }

    /// The game's own configuration, so a run measures candidates against the
    /// search that will actually use them.
    fn config(&self) -> Config {
        self.config.mcts_config()
    }
}

/// Which strategy proposes candidates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Strategy {
    /// Covariance Matrix Adaptation. Learns which weights move together, which
    /// is most of what there is to learn about fourteen correlated weights.
    CmaEs,
    /// Elitism, tournament selection, uniform crossover, Gaussian mutation.
    Ga,
}

pub struct TuneArgs {
    pub strategy: Strategy,
    pub generations: usize,
    pub games_per_eval: usize,
    pub population: usize,
    pub eval_iterations: u32,
    pub rollout_plies: u32,
    pub seed: u64,
    pub reseed_each_generation: bool,
    pub seed_params: Option<PathBuf>,
    pub output: PathBuf,
    pub threads: usize,
}

pub fn load_params(path: &Path) -> EvalParams {
    let text = fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
    serde_json::from_str(&text)
        .unwrap_or_else(|error| panic!("cannot parse {} as EvalParams: {error}", path.display()))
}

fn write_params(path: &Path, params: &EvalParams) {
    let json = serde_json::to_string_pretty(params).expect("EvalParams serializes");
    fs::write(path, json)
        .unwrap_or_else(|error| panic!("cannot write {}: {error}", path.display()));
}

pub fn run(args: &TuneArgs) {
    let base = args
        .seed_params
        .as_deref()
        .map(load_params)
        .unwrap_or_default();

    fs::create_dir_all(&args.output)
        .unwrap_or_else(|error| panic!("cannot create {}: {error}", args.output.display()));

    let seed_weights = Weights(base);
    let arena = EvalMatch::new(base, args.eval_iterations, args.rollout_plies);
    let mut optimizer: Box<dyn Optimizer> = match args.strategy {
        Strategy::CmaEs => Box::new(CmaEs::new(
            &seed_weights,
            CmaParams {
                population: args.population,
                seed: args.seed,
                ..CmaParams::default()
            },
        )),
        Strategy::Ga => Box::new(Ga::new(
            &seed_weights,
            GaParams {
                population: if args.population == 0 {
                    20
                } else {
                    args.population
                },
                seed: args.seed,
                ..GaParams::default()
            },
        )),
    };

    let population = optimizer.population();
    let total_games = population * args.games_per_eval * args.generations;
    println!(
        "{} over {} genes: {} generations x {} candidates x {} games = {} games",
        optimizer.name(),
        GENES.len(),
        args.generations,
        population,
        args.games_per_eval,
        total_games,
    );
    println!(
        "each game at {} iterations, {} rollout plies; win rates carry a standard error of \
         up to {:.1} points",
        args.eval_iterations,
        args.rollout_plies,
        100.0 * (0.25 / args.games_per_eval as f64).sqrt(),
    );

    // The fitness history, which is what makes a finished run readable. Keeping
    // only the best parameters per generation — and not the numbers behind them
    // — leaves no way to tell afterwards whether a run climbed or wandered.
    let log_path = args.output.join("history.jsonl");
    let mut history = String::new();

    let report = mcts_tune::run(
        &arena,
        &mut *optimizer,
        &TuneConfig {
            generations: args.generations,
            evaluation: Evaluation {
                games: args.games_per_eval,
                seed: args.seed,
                threads: args.threads,
            },
            reseed_each_generation: args.reseed_each_generation,
        },
        |generation| {
            let best = seed_weights.with_genes(generation.best_genes).0;
            write_params(
                &args
                    .output
                    .join(format!("gen-{:03}.json", generation.generation)),
                &best,
            );

            history.push_str(&format!(
                "{{\"generation\":{},\"best\":{:.4},\"mean\":{:.4},\"worst\":{:.4},\
                 \"incumbent\":{:.4},\"games\":{},\"seconds\":{:.1}}}\n",
                generation.generation,
                generation.best_fitness,
                generation.mean_fitness,
                generation.worst_fitness,
                generation.incumbent_fitness,
                generation.games,
                generation.elapsed.as_secs_f64(),
            ));
            fs::write(&log_path, &history).expect("cannot write the history log");

            println!(
                "gen {:>3}: best {:.4}  mean {:.4}  worst {:.4}  incumbent {:.4}  {:.1}s",
                generation.generation,
                generation.best_fitness,
                generation.mean_fitness,
                generation.worst_fitness,
                generation.incumbent_fitness,
                generation.elapsed.as_secs_f64(),
            );
        },
    );

    let best = seed_weights.with_genes(&report.best_genes).0;
    let final_path = args.output.join("best.json");
    write_params(&final_path, &best);

    println!(
        "\n{} games over {} generations. Best measured win rate {:.4}, written to {}",
        report.games,
        report.generations,
        report.best_fitness,
        final_path.display(),
    );
    println!(
        "That number is a maximum over noisy measurements and is biased upward. Confirm it:\n  \
         run-games -- simulate --games 1000 --params-a {} --iterations-a <shipping budget>",
        final_path.display(),
    );

    for (name, (seed, tuned)) in GENES.iter().zip(
        seed_weights
            .to_genes()
            .into_iter()
            .zip(Weights(best).to_genes()),
    ) {
        let change = if seed.abs() > 1e-9 {
            format!("{:+.1}%", 100.0 * (tuned - seed) / seed)
        } else {
            String::from("n/a")
        };
        println!("  {name:<20} {seed:>8.3} -> {tuned:>8.3}  {change:>8}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn as_map(params: &EvalParams) -> serde_json::Map<String, serde_json::Value> {
        match serde_json::to_value(params).expect("EvalParams serializes") {
            serde_json::Value::Object(map) => map,
            other => panic!("expected an object, got {other:?}"),
        }
    }

    /// The gene list has to cover every field but `scale`.
    ///
    /// This is the test that earns its place. A hand-written mapping between a
    /// struct and a flat vector rots the moment a field is added to the struct
    /// and not to the vector, and nothing else notices: the tuner simply never
    /// moves that parameter, which is indistinguishable from a run deciding the
    /// parameter was already right.
    #[test]
    fn every_weight_but_scale_is_a_gene() {
        let fields = as_map(&EvalParams::default()).len();
        assert_eq!(
            GENES.len(),
            fields - 1,
            "EvalParams has {fields} fields, so {} genes are expected, but GENES lists {}. \
             Add the new field to GENES, `to_genes` and `with_genes`, or pin it deliberately.",
            fields - 1,
            GENES.len()
        );
    }

    #[test]
    fn genes_round_trip_exactly() {
        let seed = Weights(EvalParams::default());
        let genes = seed.to_genes();
        assert_eq!(genes.len(), GENES.len());
        assert_eq!(seed.with_genes(&genes), seed);
    }

    /// Every gene has to reach a distinct field. A gene written to the wrong
    /// field, or dropped, leaves its target holding the base value.
    #[test]
    fn each_gene_moves_its_own_field() {
        let seed = Weights(EvalParams::default());
        // Values nothing in the defaults holds, so "unchanged" is unambiguous.
        let genes: Vec<f64> = (0..GENES.len()).map(|index| 90.0 - index as f64).collect();
        let moved = seed.with_genes(&genes).0;

        let before = as_map(&seed.0);
        let after = as_map(&moved);
        for (field, original) in &before {
            let updated = &after[field];
            if field == "scale" {
                assert_eq!(original, updated, "`scale` is pinned and must not move");
            } else {
                assert_ne!(
                    original, updated,
                    "`{field}` kept its seed value, so no gene reaches it"
                );
            }
        }

        // And distinct genes stay distinct, which a duplicated index would break.
        let written: Vec<f64> = Weights(moved).to_genes();
        assert_eq!(written, genes);
    }

    #[test]
    fn repair_holds_weights_in_range() {
        let mut genes = vec![-5.0, 0.0, 3.0, 1e9];
        Weights::repair(&mut genes);
        assert_eq!(genes, vec![0.0, 0.0, 3.0, MAX_WEIGHT]);
    }

    /// `gene_scales` sets the initial step per coordinate, so a zero would
    /// freeze that weight for the whole run.
    #[test]
    fn every_gene_scale_is_usable() {
        for (name, scale) in GENES
            .iter()
            .zip(Weights(EvalParams::default()).gene_scales())
        {
            assert!(scale > 0.0 && scale.is_finite(), "{name} has scale {scale}");
        }
    }
}
