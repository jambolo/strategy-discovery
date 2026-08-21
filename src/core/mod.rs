//! Game-agnostic framework core. Nothing in this module may reference a specific game.
//!
//! - [`traits`]: `GameDomain`, `GameRules`, `GamePrimitives`, `FeatureExtractor`,
//!   `Canonicalize`, `StateEvaluator`, `Strategy`, `StrategyGenerator`, `MatchEngine`.
//! - [`symmetry`]: permutation groups and mechanical orbit derivation (tier-1 primitives).
//! - [`features`]: tiered feature vocabulary and the feature-expression algebra.
//! - [`derived`]: tier-1 feature extractor built mechanically from `GamePrimitives`.
//! - [`dsl`]: heuristic-strategy DSL (ordered decision lists).
//! - [`interpreter`]: generic rule-interpreter `Strategy` over the DSL.

pub mod derived;
pub mod dsl;
pub mod features;
pub mod interpreter;
pub mod symmetry;
pub mod traits;

/// Reserved strategy kind names used by the strategy registry (Phase 4) and persistence.
pub mod kinds {
    /// Engine-backed minimax strategy.
    pub const MINIMAX: &str = "minimax";
    /// Uniform random strategy.
    pub const RANDOM: &str = "random";
    /// Generic rule-interpreter over a [`crate::core::dsl::HeuristicStrategy`].
    pub const HEURISTIC_RULES: &str = "heuristic-rules";
    /// Scripted (pre-recorded action list) strategy; test/debug aid, not spec-constructible.
    pub const SCRIPTED: &str = "scripted";
    /// Reserved: evolutionary strategy generator (plan.md Phase 8).
    pub const EVOLUTIONARY: &str = "evolutionary";
    /// Reserved: LLM-generated strategy (plan.md Phase 8).
    pub const LLM: &str = "llm";
}
