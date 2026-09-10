//! Strategy evaluation: strategy-side agreement with annotated optimal actions, behavior
//! signatures, and assembly of the `evaluation.json` document. Game-agnostic.
//!
//! Two passes feed the [`EvaluationReport`]: [`run_tournament`] (opposition play against a
//! roster) and, when an annotation set is supplied, [`measure_agreement`] (engine-optimality of
//! the strategy's own choices at annotated positions, plus a deterministic behavior signature
//! used for archive novelty). Neither pass mutates game state shared across positions, so
//! [`measure_agreement`] plays every non-terminal position through rayon.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use rayon::prelude::*;
use serde::Serialize;

use crate::core::traits::{StrategyError, StrategyProvider};
use crate::discovery::agreement::AgreementCounts;
use crate::discovery::analyze::AnnRec;
use crate::discovery::annotate::{AnnotateMetadata, AnnotateMode};
use crate::discovery::bundle::GameBundle;
use crate::discovery::config::CorpusError;
use crate::discovery::match_engine::{game_seed, splitmix64};
use crate::discovery::tournament::{TournamentConfig, run_tournament};
use crate::io::schema::{
    ANNOTATE_FILE, ANNOTATIONS_FILE, AgreementSummary, AnnotationSource, BehaviorSignature, CorpusGame, EvaluationConfig,
    EvaluationReport, Headline, SCHEMA_VERSION, StrategyEvaluation, TournamentResult, check_schema_version,
};
use crate::io::{IoError, config_hash, read_json, read_jsonl};
use crate::strategy::engine::EngineGame;
use crate::strategy::registry::StrategyRegistry;
use crate::strategy::roster::{Roster, RosterEntry};

/// Salt mixed into the master seed before deriving per-position agreement seeds.
pub const AGREEMENT_SEED_SALT: u64 = 0x9E3779B97F4A7C15;
/// Maximum number of sampled positions in a behavior signature.
pub const SIGNATURE_SAMPLE: usize = 256;

/// An annotation set read from a directory holding `annotations.jsonl` and `annotate.json`.
pub struct AnnotationSet<G: EngineGame + CorpusGame> {
    /// The `annotate.json` manifest.
    pub metadata: AnnotateMetadata,
    /// Every annotation record, in file order.
    pub records: Vec<AnnRec<G>>,
}

/// Agreement summary plus the behavior signature from one pass over an annotation set.
#[derive(Debug, Clone, PartialEq)]
pub struct AgreementOutcome {
    /// Agreement with the annotated optimal actions.
    pub summary: AgreementSummary,
    /// Chosen actions at the fixed sample of positions.
    pub signature: BehaviorSignature,
}

/// Knobs for evaluating strategies: the tournament configuration plus the evaluator name.
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluateConfig {
    /// Evaluator name used to build every strategy (a key of `GameBundle::evaluators`).
    pub evaluator: String,
    /// Games per (opponent, seat) pairing.
    pub games_per_pairing: usize,
    /// Master seed for tournaments and the agreement pass.
    pub seed: u64,
    /// Ply cap per game, if any.
    pub max_plies: Option<usize>,
    /// Roster entry name the headline loss rate is measured against.
    pub reference: String,
    /// Private rayon pool size; `None` = the global pool.
    pub threads: Option<usize>,
    /// Play every game serially (takes precedence over `threads`).
    pub serial: bool,
}

/// The serialized name of an [`AnnotateMode`] (`corpus` or `exhaustive`).
pub fn mode_name(mode: AnnotateMode) -> &'static str {
    match mode {
        AnnotateMode::Corpus => "corpus",
        AnnotateMode::Exhaustive => "exhaustive",
    }
}

/// Loads an annotation set from `dir`, which must hold [`ANNOTATIONS_FILE`] and [`ANNOTATE_FILE`]
/// — the output of `annotate --corpus DIR` or `annotate --exhaustive --out DIR`.
///
/// Fails with [`CorpusError::Precondition`] if either file is absent, and with
/// [`CorpusError::Config`] if the manifest's `game` does not match `bundle.name`.
pub fn load_annotations<G: EngineGame + CorpusGame>(bundle: &GameBundle<G>, dir: &Path) -> Result<AnnotationSet<G>, CorpusError> {
    let annotations_path = dir.join(ANNOTATIONS_FILE);
    if !annotations_path.exists() {
        return Err(CorpusError::Precondition(format!(
            "{} not found; run `annotate --exhaustive --game {} --out {}` or `annotate --corpus {}` first",
            annotations_path.display(),
            bundle.name,
            dir.display(),
            dir.display()
        )));
    }
    let annotate_path = dir.join(ANNOTATE_FILE);
    if !annotate_path.exists() {
        return Err(CorpusError::Precondition(format!(
            "{} not found; run `annotate --exhaustive --game {} --out {}` or `annotate --corpus {}` first",
            annotate_path.display(),
            bundle.name,
            dir.display(),
            dir.display()
        )));
    }

    let metadata: AnnotateMetadata = read_json(&annotate_path)?;
    check_schema_version(&annotate_path, metadata.schema_version)?;
    if metadata.game != bundle.name {
        return Err(CorpusError::Config(format!(
            "annotations at {} belong to game `{}`, not `{}`",
            dir.display(),
            metadata.game,
            bundle.name
        )));
    }

    let records: Vec<AnnRec<G>> = read_jsonl(&annotations_path)?;
    for record in &records {
        check_schema_version(&annotations_path, record.schema_version)?;
    }

    Ok(AnnotationSet { metadata, records })
}

/// Content-addressed key identifying the deterministic sample of positions in a behavior signature.
#[derive(Serialize)]
struct SampleKey {
    mode: String,
    run_id: Option<String>,
    nonterminal: usize,
    stride: usize,
}

/// Measures `provider`'s agreement with `annotations`' optimal actions, and its behavior
/// signature over the same pass.
///
/// For every non-terminal record (0-based file position `index`), a fresh strategy instance
/// `provider.create(game_seed(splitmix64(seed ^ AGREEMENT_SEED_SALT), index))` chooses among the
/// record's legal actions; it agrees iff the chosen action is one of `optimal_actions`. Positions
/// are played through rayon, in file order once collected; any [`StrategyError`] aborts the pass.
/// The signature samples up to [`SIGNATURE_SAMPLE`] of the chosen actions at a fixed stride over
/// the non-terminal positions, keyed by a hash of the annotation set's shape so signatures from
/// different annotation sets are never mistaken for comparable.
pub fn measure_agreement<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    provider: &dyn StrategyProvider<G>,
    annotations: &AnnotationSet<G>,
    seed: u64,
) -> Result<AgreementOutcome, CorpusError> {
    let rules = bundle.rules.as_ref();
    let agreement_seed = splitmix64(seed ^ AGREEMENT_SEED_SALT);

    let nonterminal_indices: Vec<usize> = annotations
        .records
        .iter()
        .enumerate()
        .filter(|(_, record)| !record.terminal)
        .map(|(index, _)| index)
        .collect();

    let outcomes: Vec<(usize, G::Action)> = nonterminal_indices
        .clone()
        .into_par_iter()
        .map(|index| -> Result<(usize, G::Action), StrategyError> {
            let record = &annotations.records[index];
            let legal = rules.legal_actions(&record.state);
            let mut strategy = provider.create(game_seed(agreement_seed, index));
            strategy.choose(&record.state, &legal).map(|action| (index, action))
        })
        .collect::<Result<Vec<_>, StrategyError>>()?;

    let positions = outcomes.len();
    let mut agreeing = 0usize;
    let mut by_value: BTreeMap<i8, AgreementCounts> = BTreeMap::new();
    for (index, action) in &outcomes {
        let record = &annotations.records[*index];
        let agrees = record.optimal_actions.contains(action);
        if agrees {
            agreeing += 1;
        }
        let counts = by_value.entry(record.value).or_default();
        counts.positions += 1;
        if agrees {
            counts.agreeing += 1;
        }
    }
    for counts in by_value.values_mut() {
        counts.rate = if counts.positions == 0 {
            0.0
        } else {
            counts.agreeing as f64 / counts.positions as f64
        };
    }
    let rate = if positions == 0 {
        0.0
    } else {
        agreeing as f64 / positions as f64
    };
    let summary = AgreementSummary {
        positions,
        agreeing,
        rate,
        by_value,
    };

    let stride = (positions / SIGNATURE_SAMPLE).max(1);
    let mut sampled = Vec::new();
    let mut ordinal = 0usize;
    while sampled.len() < SIGNATURE_SAMPLE && ordinal < positions {
        let value = serde_json::to_value(&outcomes[ordinal].1).map_err(|source| IoError::Json {
            path: PathBuf::from("<memory>"),
            line: 0,
            source,
        })?;
        sampled.push(value);
        ordinal += stride;
    }
    let sample_key = SampleKey {
        mode: mode_name(annotations.metadata.mode).to_string(),
        run_id: annotations.metadata.run_id.clone(),
        nonterminal: positions,
        stride,
    };
    let signature = BehaviorSignature {
        sample_id: config_hash(&sample_key)?,
        actions: sampled,
    };

    Ok(AgreementOutcome { summary, signature })
}

/// Headline metrics for one strategy's tournament (and optional agreement) results: loss rate
/// against `tournament.reference`, overall win/draw/loss rates, and the agreement rate if
/// `agreement` is supplied.
pub fn headline(tournament: &TournamentResult, agreement: Option<&AgreementSummary>) -> Headline {
    let loss_rate_vs_reference = tournament
        .opponents
        .iter()
        .find(|opponent| opponent.opponent == tournament.reference)
        .map(|opponent| opponent.tally.loss_rate)
        .unwrap_or(0.0);

    Headline {
        reference: tournament.reference.clone(),
        loss_rate_vs_reference,
        win_rate: tournament.totals.win_rate,
        draw_rate: tournament.totals.draw_rate,
        loss_rate: tournament.totals.loss_rate,
        games: tournament.totals.games,
        unfinished: tournament.totals.unfinished,
        agreement_rate: agreement.map(|summary| summary.rate),
    }
}

/// Identity key an evaluation is hashed from: a change to game, roster, config or the evaluated
/// strategies is a different evaluation.
#[derive(Serialize)]
struct EvaluationKey<'a> {
    game: &'a str,
    roster: &'a Roster,
    config: &'a EvaluationConfig,
    strategies: &'a [RosterEntry],
}

/// Computes an evaluation's identity: `config_hash` of `(game, roster, config, strategies)`.
pub fn evaluation_id(
    game: &str,
    roster: &Roster,
    config: &EvaluationConfig,
    strategies: &[RosterEntry],
) -> Result<String, CorpusError> {
    Ok(config_hash(&EvaluationKey {
        game,
        roster,
        config,
        strategies,
    })?)
}

/// Evaluates one strategy: builds it, runs its tournament against `roster`, and — when
/// `annotations` are supplied — measures its agreement with the annotated optimal actions and
/// samples its behavior signature.
pub fn evaluate_strategy<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    registry: &StrategyRegistry<G>,
    entry: &RosterEntry,
    roster: &Roster,
    annotations: Option<&AnnotationSet<G>>,
    config: &EvaluateConfig,
) -> Result<StrategyEvaluation, CorpusError> {
    let provider = registry.build(&entry.spec)?;

    let tournament_config = TournamentConfig {
        games_per_pairing: config.games_per_pairing,
        seed: config.seed,
        max_plies: config.max_plies,
        reference: config.reference.clone(),
        threads: config.threads,
        serial: config.serial,
    };
    let tournament = run_tournament(bundle, registry, entry, roster, &tournament_config)?;

    let (agreement, signature) = match annotations {
        Some(set) => {
            let outcome = measure_agreement(bundle, provider.as_ref(), set, config.seed)?;
            (Some(outcome.summary), Some(outcome.signature))
        }
        None => (None, None),
    };

    let strategy_headline = headline(&tournament, agreement.as_ref());

    Ok(StrategyEvaluation {
        name: entry.name.clone(),
        kind: entry.spec.kind().to_string(),
        spec: entry.spec.clone(),
        spec_hash: config_hash(&entry.spec)?,
        tournament,
        headline: strategy_headline,
        agreement,
        signature,
    })
}

/// Evaluates every entry of `strategies` against `roster` (defaulting to `bundle.roster` when
/// `None`), assembling the [`EvaluationReport`].
///
/// Validates the roster, then builds every provider — each strategies entry, then each roster
/// entry — once up front and discards them, so a build error surfaces before any game is played.
/// Strategies are then evaluated in `strategies` order via [`evaluate_strategy`].
pub fn evaluate_strategies<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    strategies: &[RosterEntry],
    roster: Option<&Roster>,
    annotations: Option<&AnnotationSet<G>>,
    config: &EvaluateConfig,
) -> Result<EvaluationReport, CorpusError> {
    let roster = roster.unwrap_or(&bundle.roster);
    roster.validate()?;

    let registry = StrategyRegistry::new(bundle.engine_bundle(&config.evaluator)?);

    for entry in strategies {
        registry.build(&entry.spec)?;
    }
    for entry in &roster.entries {
        registry.build(&entry.spec)?;
    }

    let mut evaluations = Vec::with_capacity(strategies.len());
    for entry in strategies {
        evaluations.push(evaluate_strategy(bundle, &registry, entry, roster, annotations, config)?);
    }

    let eval_config = EvaluationConfig {
        evaluator: config.evaluator.clone(),
        games_per_pairing: config.games_per_pairing,
        seed: config.seed,
        max_plies: config.max_plies,
        reference: config.reference.clone(),
        annotations: annotations.map(|set| AnnotationSource {
            mode: mode_name(set.metadata.mode).to_string(),
            run_id: set.metadata.run_id.clone(),
            records: set.records.len(),
        }),
    };

    let id = evaluation_id(&bundle.name, roster, &eval_config, strategies)?;

    Ok(EvaluationReport {
        schema_version: SCHEMA_VERSION,
        evaluation_id: id,
        game: bundle.name.clone(),
        roster: roster.clone(),
        config: eval_config,
        strategies: evaluations,
    })
}

/// Strategies from `report` whose loss rate against the reference exceeds `max_loss_rate`, as one
/// message per offending strategy in report order; empty when every strategy is within bounds.
pub fn strict_failures(report: &EvaluationReport, max_loss_rate: f64) -> Vec<String> {
    report
        .strategies
        .iter()
        .filter(|evaluation| evaluation.headline.loss_rate_vs_reference > max_loss_rate)
        .map(|evaluation| {
            format!(
                "reference loss rate exceeded: {} {:.3} > {:.3}",
                evaluation.name, evaluation.headline.loss_rate_vs_reference, max_loss_rate
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::dsl::HeuristicStrategy;
    use crate::discovery::annotate::{AnnotateOptions, annotate_corpus};
    use crate::discovery::config::GenerateConfig;
    use crate::discovery::corpus::{GenerateOptions, generate};
    use crate::games::tictactoe::{Move, TicTacToe, game_bundle};
    use crate::io::schema::{OpponentResult, Tally};
    use crate::strategy::registry::StrategySpec;

    fn temp_dir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("m1-evaluate-")
            .tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/target"))
            .unwrap()
    }

    fn corpus_annotations(bundle: &GameBundle<TicTacToe>) -> (tempfile::TempDir, AnnotationSet<TicTacToe>) {
        let dir = temp_dir();
        let config = GenerateConfig::<Move>::from_toml_str(include_str!("../../tests/fixtures/generate-small.toml")).unwrap();
        generate(bundle, config, &GenerateOptions::default(), dir.path()).unwrap();
        annotate_corpus(bundle, dir.path(), &AnnotateOptions::default()).unwrap();
        let annotations = load_annotations(bundle, dir.path()).unwrap();
        (dir, annotations)
    }

    fn tiny_roster() -> Roster {
        toml::from_str(include_str!("../../tests/fixtures/roster-tiny.toml")).unwrap()
    }

    fn config(seed: u64, serial: bool, threads: Option<usize>) -> EvaluateConfig {
        EvaluateConfig {
            evaluator: "default".to_string(),
            games_per_pairing: 2,
            seed,
            max_plies: None,
            reference: "perfect".to_string(),
            threads,
            serial,
        }
    }

    #[test]
    fn perfect_agrees_fully_and_random_does_not() {
        let bundle = game_bundle();
        let (_dir, annotations) = corpus_annotations(&bundle);
        let registry = StrategyRegistry::new(bundle.engine_bundle("default").unwrap());
        let roster = tiny_roster();

        let perfect_provider = registry.build(&roster.get("perfect").unwrap().spec).unwrap();
        let outcome = measure_agreement(&bundle, perfect_provider.as_ref(), &annotations, 0).unwrap();
        assert_eq!(outcome.summary.positions, 390);
        assert_eq!(outcome.summary.agreeing, 390);
        assert_eq!(outcome.summary.rate, 1.0);
        let keys: Vec<i8> = outcome.summary.by_value.keys().copied().collect();
        assert_eq!(keys, vec![-1, 0, 1]);
        assert_eq!(outcome.summary.by_value[&-1].positions, 48);
        assert_eq!(outcome.summary.by_value[&0].positions, 200);
        assert_eq!(outcome.summary.by_value[&1].positions, 142);
        for counts in outcome.summary.by_value.values() {
            assert_eq!(counts.rate, 1.0);
        }

        let random_provider = registry.build(&roster.get("random").unwrap().spec).unwrap();
        let random_outcome = measure_agreement(&bundle, random_provider.as_ref(), &annotations, 0).unwrap();
        assert_eq!(random_outcome.summary.agreeing, 223);
        assert_eq!(random_outcome.summary.positions, 390);
        assert_eq!(random_outcome.summary.rate, 223.0 / 390.0);
    }

    #[test]
    fn signature_is_deterministic_and_sized() {
        let bundle = game_bundle();
        let (_dir, annotations) = corpus_annotations(&bundle);
        let registry = StrategyRegistry::new(bundle.engine_bundle("default").unwrap());
        let roster = tiny_roster();
        let random_provider = registry.build(&roster.get("random").unwrap().spec).unwrap();

        let a = measure_agreement(&bundle, random_provider.as_ref(), &annotations, 0).unwrap();
        let b = measure_agreement(&bundle, random_provider.as_ref(), &annotations, 0).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.signature.sample_id, "07a51aa7499cf679");
        assert_eq!(a.signature.actions.len(), 256);

        let c = measure_agreement(&bundle, random_provider.as_ref(), &annotations, 1).unwrap();
        assert_eq!(c.signature.sample_id, a.signature.sample_id);
        assert_ne!(c.signature.actions, a.signature.actions);
    }

    #[test]
    fn missing_annotations_is_a_precondition_error() {
        let bundle = game_bundle();
        let dir = temp_dir();

        let err = match load_annotations(&bundle, dir.path()) {
            Ok(_) => panic!("expected a precondition error"),
            Err(err) => err,
        };
        assert!(matches!(&err, CorpusError::Precondition(msg) if msg.contains("annotations.jsonl") && msg.contains("not found")));
    }

    #[test]
    fn headline_reads_the_reference_loss_rate() {
        let tournament = TournamentResult {
            roster_id: "tiny-v1".to_string(),
            reference: "perfect".to_string(),
            games_per_pairing: 2,
            seed: 0,
            opponents: vec![
                OpponentResult {
                    opponent: "a".to_string(),
                    tally: Tally::from_counts(1, 1, 2, 0),
                    by_seat: Vec::new(),
                },
                OpponentResult {
                    opponent: "perfect".to_string(),
                    tally: Tally::from_counts(0, 3, 1, 0),
                    by_seat: Vec::new(),
                },
            ],
            totals: Tally::from_counts(1, 4, 3, 0),
        };

        let h = headline(&tournament, None);
        assert_eq!(h.loss_rate_vs_reference, 0.25);
        assert_eq!(h.loss_rate, 0.375);
        assert_eq!(h.games, 8);
        assert_eq!(h.agreement_rate, None);

        let agreement = AgreementSummary {
            positions: 4,
            agreeing: 3,
            rate: 0.75,
            by_value: BTreeMap::new(),
        };
        let h2 = headline(&tournament, Some(&agreement));
        assert_eq!(h2.agreement_rate, Some(0.75));
    }

    #[test]
    fn evaluate_strategies_assembles_a_deterministic_report() {
        let bundle = game_bundle();
        let (_dir, annotations) = corpus_annotations(&bundle);
        let roster = tiny_roster();
        let strategies = vec![roster.get("perfect").unwrap().clone(), roster.get("random").unwrap().clone()];

        let cfg_a = config(20260822, true, None);
        let report_a = evaluate_strategies(&bundle, &strategies, Some(&roster), Some(&annotations), &cfg_a).unwrap();

        assert_eq!(report_a.schema_version, 1);
        assert_eq!(report_a.game, "tictactoe");
        assert_eq!(report_a.roster.id(), "tiny-v1");
        assert_eq!(report_a.evaluation_id.len(), 16);
        assert_eq!(
            report_a.config.annotations,
            Some(AnnotationSource {
                mode: "corpus".to_string(),
                run_id: Some("5eafa65f76637df3".to_string()),
                records: 390,
            })
        );
        assert_eq!(report_a.strategies.len(), 2);

        let perfect_eval = &report_a.strategies[0];
        assert_eq!(perfect_eval.kind, "minimax");
        assert_eq!(perfect_eval.headline.loss_rate_vs_reference, 0.0);
        assert_eq!(perfect_eval.headline.loss_rate, 0.0);
        assert_eq!(perfect_eval.headline.games, 12);
        assert_eq!(perfect_eval.headline.agreement_rate, Some(1.0));
        assert_eq!(perfect_eval.tournament.opponents.len(), 3);
        assert!(perfect_eval.signature.is_some());

        let random_eval = &report_a.strategies[1];
        assert_eq!(random_eval.headline.loss_rate_vs_reference, 0.75);
        assert!(random_eval.headline.agreement_rate < Some(1.0));

        assert_eq!(
            evaluation_id(&report_a.game, &report_a.roster, &report_a.config, &strategies).unwrap(),
            report_a.evaluation_id
        );

        assert_eq!(
            strict_failures(&report_a, 0.0),
            vec!["reference loss rate exceeded: random 0.750 > 0.000".to_string()]
        );
        assert!(strict_failures(&report_a, 1.0).is_empty());

        let cfg_b = config(20260822, false, Some(2));
        let report_b = evaluate_strategies(&bundle, &strategies, Some(&roster), Some(&annotations), &cfg_b).unwrap();
        assert_eq!(report_a, report_b);
    }

    #[test]
    fn heuristic_rules_plays_and_is_evaluated() {
        let bundle = game_bundle();
        let roster = tiny_roster();
        let strategies = vec![RosterEntry {
            name: "empty".to_string(),
            spec: StrategySpec::HeuristicRules {
                heuristic: HeuristicStrategy::new("empty"),
            },
        }];
        let cfg = config(0, true, None);

        let report = evaluate_strategies(&bundle, &strategies, Some(&roster), None, &cfg).unwrap();

        assert_eq!(report.strategies.len(), 1);
        let eval = &report.strategies[0];
        assert_eq!(eval.name, "empty");
        assert_eq!(eval.kind, "heuristic-rules");
        assert_eq!(eval.headline.games, 12);
        assert_eq!(eval.headline.unfinished, 0);
        assert_eq!(eval.tournament.totals.games, 12);
        assert_eq!(eval.tournament.totals.unfinished, 0);
        assert!(eval.agreement.is_none());
        assert!(eval.signature.is_none());
        assert!(eval.headline.agreement_rate.is_none());
    }
}
