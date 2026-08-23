//! Versioned record schema, JSONL writers/readers and replay parser for corpus persistence.
//! Game-agnostic: generic over the persisted state/action/player/outcome types.

pub mod hash;
pub mod jsonl;
pub mod replay;
pub mod schema;

pub use hash::{config_hash, fnv1a64, hex16};
pub use jsonl::{JsonlReader, JsonlWriter, read_json, read_jsonl, write_json_pretty, write_jsonl};
pub use replay::{final_state, replay};
pub use schema::{
    ANALYZE_FILE, ANNOTATE_FILE, ANNOTATIONS_FILE, ARCHIVE_ENTRIES_FILE, ARCHIVE_FILE, AgreementSummary, AnnotationRecord,
    AnnotationSource, ArchiveEntry, ArchiveIndex, ArchiveIndexEntry, BehaviorSignature, CandidateParams, CellKey, CorpusGame,
    DATASET_FILE, DATASET_MANIFEST_FILE, DISCOVER_FILE, DatasetColumn, DatasetManifest, DatasetRow, DiscoverCandidate,
    DiscoverManifest, DiscoveryProvenance, EVALUATION_FILE, EvaluationConfig, EvaluationReport, GAMES_FILE, GameRecord,
    HEURISTICS_FILE, Headline, MINE_ENGINE_CART, MINE_ENGINE_LINFA_TREES, MineParams, MinedHeuristic, MiningReport, Novelty,
    OpponentResult, OutcomeTally, POSITIONS_FILE, PairingResult, PositionRecord, Provenance, RUN_FILE, RuleEvidence,
    SCHEMA_VERSION, SUMMARY_FILE, StrategyEvaluation, Tally, TournamentResult, check_schema_version,
};

use std::path::PathBuf;
use thiserror::Error;

/// Errors from reading or writing corpus files.
#[derive(Debug, Error)]
pub enum IoError {
    /// Filesystem error on `path`.
    #[error("{}: {source}", path.display())]
    Io {
        /// File involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// JSON (de)serialization error at 1-based `line` of `path` (`0` for whole-document files).
    #[error("{}:{line}: {source}", path.display())]
    Json {
        /// File involved.
        path: PathBuf,
        /// 1-based line number; `0` when the error concerns a whole `.json` document.
        line: usize,
        /// Underlying error.
        #[source]
        source: serde_json::Error,
    },
    /// TOML parse error in `path`.
    #[error("{}: {message}", path.display())]
    Toml {
        /// File involved (`<string>` when parsed from memory).
        path: PathBuf,
        /// Parser message.
        message: String,
    },
    /// A record or document carries a `schema_version` other than the one this build reads.
    #[error("{}: schema_version {found} does not match expected {expected}", path.display())]
    SchemaVersion {
        /// File involved.
        path: PathBuf,
        /// Version this build reads.
        expected: u32,
        /// Version found in the file.
        found: u32,
    },
    /// A required file is absent.
    #[error("{}: missing", path.display())]
    Missing {
        /// The absent file.
        path: PathBuf,
    },
    /// Any other invalid content.
    #[error("{0}")]
    Invalid(String),
}
