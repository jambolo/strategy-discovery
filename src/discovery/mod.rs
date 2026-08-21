//! Discovery pipeline runtime: the rayon match engine (Phase 4), the exhaustive solver, the
//! game bundle and the sweep configuration (Phase 5). Corpus generation, annotation and
//! summary stages follow in later steps.

pub mod annotate;
pub mod bundle;
pub mod config;
pub mod corpus;
pub mod match_engine;
pub mod solver;
pub mod summary;

pub use match_engine::{RayonMatchEngine, game_seed, play_game, player_seed, splitmix64};

pub use annotate::{AnnotateMetadata, AnnotateMode, AnnotateOptions, annotate_corpus, annotate_exhaustive, annotate_states};
pub use bundle::GameBundle;
pub use config::{Cell, CorpusError, GenerateConfig, NamedOpening, Pairing, ResolvedConfig, cell_seed, resolve};
pub use corpus::{GenerateOptions, RunMetadata, expand_game, generate};
pub use solver::{ExhaustiveSolver, Solved, SolverError, reachable_states};
pub use summary::{CorpusSummary, DiversityThresholds, OutcomeCounts, analyze_corpus, summarize, verify_corpus};
