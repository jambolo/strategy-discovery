//! Sweep configuration for corpus generation.
//!
//! A [`GenerateConfig`] is the human-authored, TOML-loadable description of a corpus sweep:
//! search depth/epsilon variants live in [`GenerateConfig::strategies`] as named
//! [`RosterEntry`] specs, tie-break seeding derives from [`GenerateConfig::seed`], evaluator
//! variants are named in [`GenerateConfig::evaluators`], forced starts are named
//! [`NamedOpening`]s crossed with [`GenerateConfig::random_opening_plies`], and opponent
//! pairings are named in [`GenerateConfig::pairings`] (empty meaning "all ordered pairs",
//! filled in by a later resolution step). The resolved form — defaults filled in, plus the
//! enumerated [`Cell`]s — is [`ResolvedConfig`]; it is what gets hashed and persisted
//! alongside a run so the run is reproducible from its config alone. This module defines the
//! data model and TOML/seed plumbing only; enumeration into [`ResolvedConfig`] and validation
//! are added by a later step.

use crate::core::traits::{MatchError, RulesError, StrategyError};
use crate::discovery::bundle::GameBundle;
use crate::discovery::match_engine::{Opening, splitmix64};
use crate::discovery::solver::SolverError;
use crate::io::IoError;
use crate::io::replay::replay;
use crate::io::schema::{CellKey, CorpusGame, SCHEMA_VERSION};
use crate::strategy::engine::EngineGame;
use crate::strategy::registry::StrategyRegistry;
use crate::strategy::roster::RosterEntry;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Strategy names aligned to a game's players, naming one opposing pairing. TOML:
/// `pairings = [["random","perfect"]]`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(transparent)]
pub struct Pairing(
    /// Strategy entry names, in turn order.
    pub Vec<String>,
);

/// A named fixed opening: an action prefix applied verbatim before strategies take over. TOML:
/// `[[openings]]` with `name = "center"` and `actions = [4]`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct NamedOpening<A> {
    /// Name this opening is referenced by from a sweep cell.
    pub name: String,
    /// Actions applied verbatim from the initial state.
    #[serde(default = "Vec::new")]
    pub actions: Vec<A>,
}

/// Human-authored sweep configuration (TOML in, resolved JSON out).
///
/// Every `Vec` field defaults to empty when absent from TOML; empty means "derive from the
/// game bundle" (see each field's doc), which a later resolution step fills in to produce a
/// [`ResolvedConfig`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GenerateConfig<A> {
    /// Schema version this config was authored against; must equal
    /// [`crate::io::schema::SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Game name; must equal the target game bundle's name.
    pub game: String,
    /// Master seed this sweep's per-cell and per-game seeds are derived from.
    pub seed: u64,
    /// Number of games played per enumerated cell; must be `> 0`.
    pub games_per_cell: usize,
    /// Optional cap on plies per game; `None` means play to completion.
    #[serde(default)]
    pub max_plies: Option<usize>,
    /// Named strategy specs available to [`Self::pairings`]; empty means the game bundle's
    /// default strategies.
    #[serde(default = "Vec::new")]
    pub strategies: Vec<RosterEntry>,
    /// Opponent pairings to sweep, by strategy entry name; empty means all ordered pairs of
    /// [`Self::strategies`].
    #[serde(default = "Vec::new")]
    pub pairings: Vec<Pairing>,
    /// Evaluator variant names to sweep; empty means the game bundle's single default
    /// evaluator.
    #[serde(default = "Vec::new")]
    pub evaluators: Vec<String>,
    /// Named openings to sweep; empty means a single no-op opening named `"none"`.
    #[serde(default = "Vec::new")]
    pub openings: Vec<NamedOpening<A>>,
    /// Random-opening-ply counts to sweep, crossed with [`Self::openings`]; empty means `[0]`.
    #[serde(default = "Vec::new")]
    pub random_opening_plies: Vec<u32>,
}

impl<A: DeserializeOwned> GenerateConfig<A> {
    /// Parses a [`GenerateConfig`] from an in-memory TOML document. Parse errors are reported
    /// as [`IoError::Toml`] with `path` set to `"<string>"`.
    pub fn from_toml_str(text: &str) -> Result<Self, CorpusError> {
        toml::from_str(text)
            .map_err(|e| IoError::Toml {
                path: PathBuf::from("<string>"),
                message: e.to_string(),
            })
            .map_err(CorpusError::from)
    }

    /// Parses a [`GenerateConfig`] from a TOML file on disk. A read failure is reported as
    /// [`IoError::Io`]; a parse failure is reported as [`IoError::Toml`], both carrying `path`.
    pub fn from_toml_file(path: &Path) -> Result<Self, CorpusError> {
        let text = std::fs::read_to_string(path).map_err(|source| IoError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&text)
            .map_err(|e| IoError::Toml {
                path: path.to_path_buf(),
                message: e.to_string(),
            })
            .map_err(CorpusError::from)
    }
}

/// One enumerated sweep cell: a resolved strategy pairing, evaluator, and opening, with its
/// derived seed.
#[derive(Clone, Debug, PartialEq)]
pub struct Cell<A> {
    /// Identifying key for this cell, persisted onto every record it produces.
    pub key: CellKey,
    /// Resolved strategy entries, aligned to the game's players.
    pub entries: Vec<RosterEntry>,
    /// Forced opening this cell's games start from.
    pub opening: Opening<A>,
    /// Master seed for this cell, derived via [`cell_seed`].
    pub seed: u64,
}

/// A [`GenerateConfig`] with defaults filled in, plus its enumerated [`Cell`]s. Hashed and
/// persisted into a run's manifest so the run is reproducible from its config alone.
#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedConfig<A> {
    /// The config this was resolved from, with defaults filled in.
    pub config: GenerateConfig<A>,
    /// Every cell enumerated from `config`.
    pub cells: Vec<Cell<A>>,
}

/// Salt mixed into the master seed to derive per-cell seeds.
pub const CELL_SEED_SALT: u64 = 0x5851F42D4C957F2D;
/// Multiplier applied to the cell index when deriving per-cell seeds.
pub const CELL_SEED_MUL: u64 = 0xD1B54A32D192ED03;

/// Per-cell master seed, derived from a sweep's master seed and the cell's index:
/// `splitmix64(master ^ CELL_SEED_SALT ^ (cell_index as u64).wrapping_mul(CELL_SEED_MUL))`.
pub fn cell_seed(master: u64, cell_index: usize) -> u64 {
    splitmix64(master ^ CELL_SEED_SALT ^ (cell_index as u64).wrapping_mul(CELL_SEED_MUL))
}

/// Errors from the corpus pipeline stages.
#[derive(Debug, Error)]
pub enum CorpusError {
    /// A sweep configuration is invalid (fails validation, not TOML/JSON parsing).
    #[error("invalid configuration: {0}")]
    Config(String),
    /// A required input is absent; the message names the missing file and the stage to run first.
    #[error("precondition failed: {0}")]
    Precondition(String),
    /// A match failed to play out.
    #[error(transparent)]
    Match(#[from] MatchError),
    /// A strategy could not be built or failed to choose an action.
    #[error(transparent)]
    Strategy(#[from] StrategyError),
    /// A file could not be read, written, or parsed.
    #[error(transparent)]
    Io(#[from] IoError),
    /// The rules rejected an action or state.
    #[error(transparent)]
    Rules(#[from] RulesError),
    /// The exhaustive solver's state-space limit was exceeded.
    #[error(transparent)]
    Solver(#[from] SolverError),
}

/// Fills defaults from `bundle`, validates, and enumerates the sweep cells in the documented order.
pub fn resolve<G: EngineGame + CorpusGame>(
    mut config: GenerateConfig<G::Action>,
    bundle: &GameBundle<G>,
) -> Result<ResolvedConfig<G::Action>, CorpusError> {
    if config.schema_version != SCHEMA_VERSION {
        return Err(CorpusError::Config(format!(
            "schema_version {} is not the supported {SCHEMA_VERSION}",
            config.schema_version
        )));
    }
    if config.game != bundle.name {
        return Err(CorpusError::Config(format!(
            "game `{}` does not match bundle `{}`",
            config.game, bundle.name
        )));
    }
    if config.games_per_cell == 0 {
        return Err(CorpusError::Config("games_per_cell must be > 0".to_string()));
    }

    if config.strategies.is_empty() {
        config.strategies = bundle.default_strategies.clone();
    }
    if config.pairings.is_empty() {
        let names: Vec<String> = config.strategies.iter().map(|entry| entry.name.clone()).collect();
        config.pairings = all_pairings(&names, bundle.players.len());
    }
    if config.evaluators.is_empty() {
        config.evaluators = vec![bundle.default_evaluator.clone()];
    }
    if config.openings.is_empty() {
        config.openings = vec![NamedOpening {
            name: "none".to_string(),
            actions: Vec::new(),
        }];
    }
    if config.random_opening_plies.is_empty() {
        config.random_opening_plies = vec![0];
    }

    let registry = StrategyRegistry::new(bundle.engine_bundle(&bundle.default_evaluator)?);
    let mut seen_strategy_names: Vec<&str> = Vec::with_capacity(config.strategies.len());
    for entry in &config.strategies {
        if entry.name.is_empty() {
            return Err(CorpusError::Config("strategy name must not be empty".to_string()));
        }
        if seen_strategy_names.contains(&entry.name.as_str()) {
            return Err(CorpusError::Config(format!("duplicate strategy name `{}`", entry.name)));
        }
        seen_strategy_names.push(entry.name.as_str());
        registry
            .build(&entry.spec)
            .map_err(|err| CorpusError::Config(format!("strategy `{}`: {err}", entry.name)))?;
    }

    for pairing in &config.pairings {
        if pairing.0.len() != bundle.players.len() {
            return Err(CorpusError::Config(format!(
                "pairing {:?} must name {} strategies (one per player)",
                pairing.0,
                bundle.players.len()
            )));
        }
        for name in &pairing.0 {
            if !config.strategies.iter().any(|entry| &entry.name == name) {
                return Err(CorpusError::Config(format!(
                    "pairing {:?} names unknown strategy `{name}`",
                    pairing.0
                )));
            }
        }
    }

    for evaluator in &config.evaluators {
        bundle.engine_bundle(evaluator)?;
    }

    let mut seen_opening_names: Vec<&str> = Vec::with_capacity(config.openings.len());
    for opening in &config.openings {
        if opening.name.is_empty() {
            return Err(CorpusError::Config("opening name must not be empty".to_string()));
        }
        if seen_opening_names.contains(&opening.name.as_str()) {
            return Err(CorpusError::Config(format!("duplicate opening name `{}`", opening.name)));
        }
        seen_opening_names.push(opening.name.as_str());
        replay(bundle.rules.as_ref(), &opening.actions)
            .map_err(|err| CorpusError::Config(format!("opening `{}` is not a legal action sequence: {err}", opening.name)))?;
    }

    let mut seen_plies: Vec<u32> = Vec::with_capacity(config.random_opening_plies.len());
    for &plies in &config.random_opening_plies {
        if seen_plies.contains(&plies) {
            return Err(CorpusError::Config(format!("duplicate random_opening_plies value {plies}")));
        }
        seen_plies.push(plies);
    }

    let mut cells = Vec::new();
    let mut index = 0usize;
    for pairing in &config.pairings {
        for evaluator in &config.evaluators {
            for opening in &config.openings {
                for &plies in &config.random_opening_plies {
                    let entries: Vec<RosterEntry> = pairing
                        .0
                        .iter()
                        .map(|name| {
                            config
                                .strategies
                                .iter()
                                .find(|entry| &entry.name == name)
                                .expect("pairing names were validated above")
                                .clone()
                        })
                        .collect();
                    cells.push(Cell {
                        key: CellKey {
                            index,
                            strategies: pairing.0.clone(),
                            evaluator: evaluator.clone(),
                            opening: opening.name.clone(),
                            random_opening_plies: plies,
                        },
                        entries,
                        opening: Opening {
                            actions: opening.actions.clone(),
                            random_plies: plies,
                        },
                        seed: cell_seed(config.seed, index),
                    });
                    index += 1;
                }
            }
        }
    }

    Ok(ResolvedConfig { config, cells })
}

/// Ordered tuples of `names`, `slots` long: `names` crossed with itself `slots` times, the
/// earliest slot varying slowest (outer loop) and the latest slot varying fastest (inner loop).
fn all_pairings(names: &[String], slots: usize) -> Vec<Pairing> {
    let mut acc: Vec<Vec<String>> = vec![vec![]];
    for _ in 0..slots {
        let mut next = Vec::with_capacity(acc.len() * names.len());
        for prefix in &acc {
            for name in names {
                let mut extended = prefix.clone();
                extended.push(name.clone());
                next.push(extended);
            }
        }
        acc = next;
    }
    acc.into_iter().map(Pairing).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::tictactoe::{Move, game_bundle};
    use crate::strategy::minimax::{MinimaxConfig, TieBreak};
    use crate::strategy::registry::StrategySpec;
    use std::collections::BTreeSet;
    use std::path::Path;

    const SMALL: &str = r#"
schema_version = 1
game = "tictactoe"
seed = 20260821
games_per_cell = 4
evaluators = ["default"]
random_opening_plies = [0, 2]

[[strategies]]
name = "random"
[strategies.spec]
kind = "random"

[[strategies]]
name = "depth-2"
[strategies.spec]
kind = "minimax"
depth = 2

[[strategies]]
name = "perfect"
[strategies.spec]
kind = "minimax"
depth = 9
tie_break = "engine"

[[openings]]
name = "none"

[[openings]]
name = "center"
actions = [4]
"#;

    /// [`SMALL`] with `extra_top_level` inserted right after `games_per_cell = 4`.
    fn small_with(extra_top_level: &str) -> String {
        SMALL.replace("games_per_cell = 4\n", &format!("games_per_cell = 4\n{extra_top_level}"))
    }

    /// Parses `text` and resolves it against the tic-tac-toe [`game_bundle`].
    fn resolve_text(text: &str) -> Result<ResolvedConfig<Move>, CorpusError> {
        resolve(GenerateConfig::<Move>::from_toml_str(text)?, &game_bundle())
    }

    #[test]
    fn documented_toml_shape_parses() {
        let config = GenerateConfig::<Move>::from_toml_str(SMALL).unwrap();
        assert_eq!(config.schema_version, 1);
        assert_eq!(config.game, "tictactoe");
        assert_eq!(config.seed, 20260821);
        assert_eq!(config.games_per_cell, 4);
        assert_eq!(config.max_plies, None);

        let names: Vec<&str> = config.strategies.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["random", "depth-2", "perfect"]);
        assert_eq!(config.strategies[0].spec, StrategySpec::Random);
        assert_eq!(
            config.strategies[1].spec,
            StrategySpec::Minimax(MinimaxConfig {
                depth: 2,
                epsilon: 0.0,
                tie_break: TieBreak::SeededUniform,
            })
        );
        match &config.strategies[2].spec {
            StrategySpec::Minimax(cfg) => {
                assert_eq!(cfg.depth, 9);
                assert_eq!(cfg.tie_break, TieBreak::Engine);
            }
            other => panic!("expected Minimax, got {other:?}"),
        }

        assert!(config.pairings.is_empty());
        assert_eq!(config.evaluators, vec!["default".to_string()]);
        assert_eq!(
            config.openings,
            vec![
                NamedOpening {
                    name: "none".to_string(),
                    actions: vec![],
                },
                NamedOpening {
                    name: "center".to_string(),
                    actions: vec![Move(4)],
                },
            ]
        );
        assert_eq!(config.random_opening_plies, vec![0, 2]);
    }

    #[test]
    fn pairings_parse_as_nested_string_arrays() {
        let text = SMALL.replace(
            "games_per_cell = 4",
            "games_per_cell = 4\npairings = [[\"random\", \"perfect\"], [\"perfect\", \"random\"]]",
        );
        let config = GenerateConfig::<Move>::from_toml_str(&text).unwrap();
        assert_eq!(
            config.pairings,
            vec![
                Pairing(vec!["random".to_string(), "perfect".to_string()]),
                Pairing(vec!["perfect".to_string(), "random".to_string()]),
            ]
        );
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let text = SMALL.replace("games_per_cell = 4", "games_per_cell = 4\nbogus = 1");
        match GenerateConfig::<Move>::from_toml_str(&text) {
            Err(CorpusError::Io(IoError::Toml { message, .. })) => assert!(message.contains("bogus")),
            other => panic!("expected Err(Io(Toml {{ .. }})), got {other:?}"),
        }
    }

    #[test]
    fn unknown_strategy_kind_is_rejected() {
        let text = SMALL.replace("kind = \"random\"", "kind = \"mcts\"");
        match GenerateConfig::<Move>::from_toml_str(&text) {
            Err(CorpusError::Io(IoError::Toml { .. })) => {}
            other => panic!("expected Err(Io(Toml {{ .. }})), got {other:?}"),
        }
    }

    #[test]
    fn missing_file_is_an_io_error() {
        match GenerateConfig::<Move>::from_toml_file(Path::new("definitely-missing-dir/cfg.toml")) {
            Err(CorpusError::Io(IoError::Io { .. })) => {}
            other => panic!("expected Err(Io(Io {{ .. }})), got {other:?}"),
        }
    }

    #[test]
    fn minimal_config_uses_empty_defaults() {
        let config =
            GenerateConfig::<Move>::from_toml_str("schema_version = 1\ngame = \"tictactoe\"\nseed = 1\ngames_per_cell = 1\n")
                .unwrap();
        assert!(config.strategies.is_empty());
        assert!(config.pairings.is_empty());
        assert!(config.evaluators.is_empty());
        assert!(config.openings.is_empty());
        assert!(config.random_opening_plies.is_empty());
        assert_eq!(config.max_plies, None);
    }

    #[test]
    fn config_round_trips_through_json() {
        let config = GenerateConfig::<Move>::from_toml_str(SMALL).unwrap();
        let json = serde_json::to_string(&config).unwrap();
        let round_tripped: GenerateConfig<Move> = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, config);
    }

    #[test]
    fn cell_seed_reference_values() {
        assert_eq!(cell_seed(0, 0), 0xBA8894FA3BE59747);
        assert_eq!(cell_seed(0, 1), 0x2366A2894CCB62DB);
        assert_eq!(cell_seed(20260821, 0), 0xF7AB10D4F37DAD2A);
        assert_eq!(cell_seed(20260821, 1), 0x45120386E5756643);
        assert_ne!(cell_seed(0, 0), cell_seed(0, 1));
    }

    #[test]
    fn corpus_error_messages() {
        assert_eq!(CorpusError::Config("x".to_string()).to_string(), "invalid configuration: x");
        assert!(
            CorpusError::from(SolverError::TooLarge { limit: 3 })
                .to_string()
                .contains("limit of 3 states")
        );
    }

    #[test]
    fn resolve_fills_defaults_from_the_bundle() {
        let resolved = resolve_text("schema_version = 1\ngame = \"tictactoe\"\nseed = 1\ngames_per_cell = 1\n").unwrap();

        let names: Vec<&str> = resolved.config.strategies.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["random", "depth-1", "depth-2", "depth-3", "depth-4", "depth-6", "perfect"]
        );
        assert_eq!(resolved.config.pairings.len(), 49);
        assert_eq!(resolved.config.evaluators, vec!["default".to_string()]);
        assert_eq!(
            resolved.config.openings,
            vec![NamedOpening {
                name: "none".to_string(),
                actions: vec![],
            }]
        );
        assert_eq!(resolved.config.random_opening_plies, vec![0]);

        assert_eq!(resolved.cells.len(), 49);
        assert_eq!(
            resolved.cells[0].key,
            CellKey {
                index: 0,
                strategies: vec!["random".to_string(), "random".to_string()],
                evaluator: "default".to_string(),
                opening: "none".to_string(),
                random_opening_plies: 0,
            }
        );
        assert_eq!(
            resolved.cells[1].key.strategies,
            vec!["random".to_string(), "depth-1".to_string()]
        );
        assert_eq!(
            resolved.cells[7].key.strategies,
            vec!["depth-1".to_string(), "random".to_string()]
        );
        assert_eq!(
            resolved.cells[48].key.strategies,
            vec!["perfect".to_string(), "perfect".to_string()]
        );
        assert_eq!(resolved.cells[0].opening, Opening::default());
    }

    #[test]
    fn three_strategies_enumerate_cells_in_documented_order() {
        let resolved = resolve_text(SMALL).unwrap();

        assert_eq!(
            resolved.config.pairings,
            vec![
                Pairing(vec!["random".to_string(), "random".to_string()]),
                Pairing(vec!["random".to_string(), "depth-2".to_string()]),
                Pairing(vec!["random".to_string(), "perfect".to_string()]),
                Pairing(vec!["depth-2".to_string(), "random".to_string()]),
                Pairing(vec!["depth-2".to_string(), "depth-2".to_string()]),
                Pairing(vec!["depth-2".to_string(), "perfect".to_string()]),
                Pairing(vec!["perfect".to_string(), "random".to_string()]),
                Pairing(vec!["perfect".to_string(), "depth-2".to_string()]),
                Pairing(vec!["perfect".to_string(), "perfect".to_string()]),
            ]
        );
        assert_eq!(resolved.cells.len(), 36);
        for (i, cell) in resolved.cells.iter().enumerate() {
            assert_eq!(cell.key.index, i);
        }

        assert_eq!(
            resolved.cells[0].key.strategies,
            vec!["random".to_string(), "random".to_string()]
        );
        assert_eq!(resolved.cells[0].key.evaluator, "default");
        assert_eq!(resolved.cells[0].key.opening, "none");
        assert_eq!(resolved.cells[0].key.random_opening_plies, 0);

        assert_eq!(resolved.cells[1].key.strategies, resolved.cells[0].key.strategies);
        assert_eq!(resolved.cells[1].key.opening, "none");
        assert_eq!(resolved.cells[1].key.random_opening_plies, 2);

        assert_eq!(resolved.cells[2].key.strategies, resolved.cells[0].key.strategies);
        assert_eq!(resolved.cells[2].key.opening, "center");
        assert_eq!(resolved.cells[2].key.random_opening_plies, 0);

        assert_eq!(resolved.cells[3].key.opening, "center");
        assert_eq!(resolved.cells[3].key.random_opening_plies, 2);

        assert_eq!(
            resolved.cells[4].key.strategies,
            vec!["random".to_string(), "depth-2".to_string()]
        );

        assert_eq!(
            resolved.cells[2].opening,
            Opening {
                actions: vec![Move(4)],
                random_plies: 0,
            }
        );
        assert_eq!(resolved.cells[3].opening.random_plies, 2);

        let entry_names: Vec<&str> = resolved.cells[4].entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(entry_names, vec!["random", "depth-2"]);
        assert_eq!(resolved.cells[4].entries[1].spec, resolved.config.strategies[1].spec);
    }

    #[test]
    fn cell_seeds_follow_the_formula_and_are_distinct() {
        let resolved = resolve_text(SMALL).unwrap();

        for (i, cell) in resolved.cells.iter().enumerate() {
            assert_eq!(cell.seed, cell_seed(20260821, i));
        }

        let distinct: BTreeSet<u64> = resolved.cells.iter().map(|c| c.seed).collect();
        assert_eq!(distinct.len(), 36);
    }

    #[test]
    fn defaulted_and_spelled_out_configs_resolve_equal() {
        let defaulted = resolve_text(SMALL).unwrap();
        let spelled_out = resolve_text(&small_with(
            "pairings = [[\"random\",\"random\"], [\"random\",\"depth-2\"], [\"random\",\"perfect\"], [\"depth-2\",\"random\"], [\"depth-2\",\"depth-2\"], [\"depth-2\",\"perfect\"], [\"perfect\",\"random\"], [\"perfect\",\"depth-2\"], [\"perfect\",\"perfect\"]]\n",
        ))
        .unwrap();
        assert_eq!(defaulted, spelled_out);
    }

    #[test]
    fn invalid_configs_are_rejected() {
        let cases: Vec<(String, &str)> = vec![
            (SMALL.replace("schema_version = 1", "schema_version = 2"), "schema_version 2"),
            (SMALL.replace("game = \"tictactoe\"", "game = \"chess\""), "chess"),
            (SMALL.replace("games_per_cell = 4", "games_per_cell = 0"), "games_per_cell"),
            (small_with("pairings = [[\"random\",\"nope\"]]\n"), "nope"),
            (small_with("pairings = [[\"random\"]]\n"), "2 strategies"),
            (
                SMALL.replace("evaluators = [\"default\"]", "evaluators = [\"nope\"]"),
                "unknown evaluator `nope`",
            ),
            (
                format!("{SMALL}\n[[strategies]]\nname = \"random\"\n[strategies.spec]\nkind = \"random\"\n"),
                "duplicate strategy name `random`",
            ),
            (SMALL.replace("name = \"depth-2\"", "name = \"\""), "must not be empty"),
            (
                SMALL.replace("name = \"center\"", "name = \"none\""),
                "duplicate opening name `none`",
            ),
            (SMALL.replace("actions = [4]", "actions = [4, 4]"), "opening `center`"),
            (
                SMALL.replace("random_opening_plies = [0, 2]", "random_opening_plies = [0, 0]"),
                "duplicate random_opening_plies",
            ),
            (SMALL.replace("depth = 2", "depth = 0"), "strategy `depth-2`"),
            (
                format!("{SMALL}\n[[strategies]]\nname = \"reserved\"\n[strategies.spec]\nkind = \"llm\"\n"),
                "strategy `reserved`",
            ),
        ];

        for (text, expected) in cases {
            match resolve_text(&text) {
                Err(CorpusError::Config(msg)) => {
                    assert!(msg.contains(expected), "case {expected:?}: message was {msg:?}");
                }
                other => panic!("case {expected:?}: expected Err(Config(_)), got {other:?}"),
            }
        }

        assert!(matches!(resolve_text(&small_with("bogus = 1\n")), Err(CorpusError::Io(_))));
    }

    #[test]
    fn checked_in_configs_resolve() {
        let bundle = game_bundle();
        let cases: [(&str, usize, usize); 4] = [
            ("tests/fixtures/generate-small.toml", 36, 4),
            ("tests/fixtures/generate-small-explicit.toml", 36, 4),
            ("tests/fixtures/generate-draws.toml", 1, 16),
            ("configs/tictactoe-default.toml", 49, 20),
        ];

        for (path, cells, games_per_cell) in cases {
            let config = GenerateConfig::<Move>::from_toml_file(Path::new(path)).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert_eq!(config.games_per_cell, games_per_cell, "{path}");
            let resolved = resolve(config, &bundle).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert_eq!(resolved.cells.len(), cells, "{path}");
        }

        let small = GenerateConfig::<Move>::from_toml_file(Path::new("tests/fixtures/generate-small.toml")).unwrap();
        let explicit = GenerateConfig::<Move>::from_toml_file(Path::new("tests/fixtures/generate-small-explicit.toml")).unwrap();
        assert_eq!(resolve(small, &bundle).unwrap(), resolve(explicit, &bundle).unwrap());
    }
}
