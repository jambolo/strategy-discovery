//! Discovery pipeline runtime: the rayon match engine (Phase 4), the exhaustive solver, the
//! game bundle, the sweep configuration, the analyzer registry backing the `analyze` stage
//! (Phase 5), and the Milestone 1 evaluation harness (`tournament`, `evaluate`, `archive`).

pub mod agreement;
pub mod analyze;
pub mod annotate;
pub mod archive;
pub mod bundle;
pub mod config;
pub mod corpus;
pub mod evaluate;
pub mod experiment;
pub mod match_engine;
pub mod solver;
pub mod summary;
pub mod tournament;

pub use match_engine::{RayonMatchEngine, game_seed, play_game, player_seed, splitmix64};

pub use agreement::{AGREEMENT_FILE, AgreementAnalyzer, AgreementCounts, AgreementReport};
pub use analyze::{
    AnalyzeContext, AnalyzeMetadata, AnalyzeOptions, Analyzer, AnalyzerEntry, AnalyzerOutput, AnalyzerRegistry, LoadedCorpus,
    RunHeader, builtin_registry, load_corpus,
};
pub use annotate::{AnnotateMetadata, AnnotateMode, AnnotateOptions, annotate_corpus, annotate_exhaustive, annotate_states};
pub use archive::{Appended, Archive, NOVELTY_METHOD, NewEntry, behavior_distance, entry_id, novelty, spec_distance};
pub use bundle::GameBundle;
pub use config::{Cell, CorpusError, GenerateConfig, NamedOpening, Pairing, ResolvedConfig, cell_seed, resolve};
pub use corpus::{GenerateOptions, RunMetadata, expand_game, generate};
pub use evaluate::{
    AGREEMENT_SEED_SALT, AgreementOutcome, AnnotationSet, EvaluateConfig, SIGNATURE_SAMPLE, evaluate_strategies, evaluate_strategy,
    evaluation_id, headline, load_annotations, measure_agreement, mode_name, strict_failures,
};
pub use experiment::{
    AnalyzeSection, AnnotateSection, ExperimentConfig, GenerateSection, ReportSection, ResolvedExperiment, StrategyFile,
    SweepSource, load_experiment, resolve_experiment,
};
pub use solver::{ExhaustiveSolver, Solved, SolverError, reachable_states};
pub use summary::{CorpusSummary, DiversityThresholds, OutcomeCounts, SummaryAnalyzer, analyze_corpus, summarize, verify_corpus};
pub use tournament::{TournamentConfig, TournamentRun, default_reference, run_tournament, run_tournament_detailed};
