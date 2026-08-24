//! Versioned record schema for corpus persistence: file-name constants, the [`CorpusGame`]
//! bound that makes a game's associated types serializable, and the three record shapes
//! (`GameRecord`, `PositionRecord`, `AnnotationRecord`) written by the pipeline stages.

use crate::core::dsl::HeuristicStrategy;
use crate::core::features::{FeatureDef, Tier};
use crate::core::traits::GameDomain;
use crate::discovery::agreement::AgreementCounts;
use crate::discovery::config::CorpusError;
use crate::io::IoError;
use crate::strategy::registry::StrategySpec;
use crate::strategy::roster::Roster;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Schema version written into every record and top-level document.
pub const SCHEMA_VERSION: u32 = 1;
/// File name for a run's top-level configuration/manifest document.
pub const RUN_FILE: &str = "run.json";
/// File name for the per-game JSONL corpus file.
pub const GAMES_FILE: &str = "games.jsonl";
/// File name for the per-position JSONL corpus file.
pub const POSITIONS_FILE: &str = "positions.jsonl";
/// File name for the per-position annotation JSONL file.
pub const ANNOTATIONS_FILE: &str = "annotations.jsonl";
/// File name for the annotation stage's configuration document.
pub const ANNOTATE_FILE: &str = "annotate.json";
/// File name for a run's summary document.
pub const SUMMARY_FILE: &str = "summary.json";
/// File name for the analyze stage's manifest document.
pub const ANALYZE_FILE: &str = "analyze.json";
/// File name for the `evaluate` stage's evaluation document.
pub const EVALUATION_FILE: &str = "evaluation.json";
/// File name for a strategy archive's index document.
pub const ARCHIVE_FILE: &str = "archive.json";
/// File name for a strategy archive's per-entry JSONL file (append order = `sequence`).
pub const ARCHIVE_ENTRIES_FILE: &str = "entries.jsonl";
/// File name for the `dataset` analyzer's per-row JSONL output.
pub const DATASET_FILE: &str = "dataset.jsonl";
/// File name for the `dataset` analyzer's manifest document (the registered analyzer output).
pub const DATASET_MANIFEST_FILE: &str = "dataset.json";
/// File name for the `mine` analyzer's mining report (candidate heuristics with evidence).
pub const HEURISTICS_FILE: &str = "heuristics.json";
/// File name for the `discover` stage's manifest document.
pub const DISCOVER_FILE: &str = "discover.json";

/// File name of the `concepts` analyzer's output document.
pub const CONCEPTS_FILE: &str = "concepts.json";
/// File name of the `vocabulary` analyzer's output document.
pub const VOCABULARY_FILE: &str = "vocabulary.json";
/// `MineParams::engine` value selecting the in-crate deterministic CART engine (the default).
pub const MINE_ENGINE_CART: &str = "cart";
/// `MineParams::engine` value selecting the opt-in `linfa-trees` engine.
pub const MINE_ENGINE_LINFA_TREES: &str = "linfa-trees";

/// Games whose state/action/player/outcome types can be persisted. Blanket-implemented.
pub trait CorpusGame:
    GameDomain<
        State: Serialize + DeserializeOwned,
        Action: Serialize + DeserializeOwned,
        Player: Serialize + DeserializeOwned,
        Outcome: Serialize + DeserializeOwned,
    >
{
}

impl<G> CorpusGame for G
where
    G: GameDomain,
    G::State: Serialize + DeserializeOwned,
    G::Action: Serialize + DeserializeOwned,
    G::Player: Serialize + DeserializeOwned,
    G::Outcome: Serialize + DeserializeOwned,
{
}

/// Checks a record's or document's `schema_version` against [`SCHEMA_VERSION`].
pub fn check_schema_version(path: &Path, found: u32) -> Result<(), IoError> {
    if found == SCHEMA_VERSION {
        Ok(())
    } else {
        Err(IoError::SchemaVersion {
            path: path.to_path_buf(),
            expected: SCHEMA_VERSION,
            found,
        })
    }
}

/// One sweep cell: a strategy pairing x evaluator variant x opening x random-opening-plies.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CellKey {
    /// Index of this cell within the sweep, used to build `game_id`.
    pub index: usize,
    /// Strategy entry names, aligned to turn order.
    pub strategies: Vec<String>,
    /// Evaluator variant name used by this cell.
    pub evaluator: String,
    /// Opening name used by this cell.
    pub opening: String,
    /// Number of random-opening plies played after the fixed opening.
    pub random_opening_plies: u32,
}

/// One played game: the full action sequence and its outcome, keyed by cell and index within it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GameRecord<S, A, P, O> {
    /// Schema version this record was written under.
    pub schema_version: u32,
    /// Identifier of the run that produced this record.
    pub run_id: String,
    /// `format!("{}:{}", cell.index, game_index)`.
    pub game_id: String,
    /// Sweep cell this game belongs to.
    pub cell: CellKey,
    /// Index of this game within its cell.
    pub game_index: usize,
    /// Per-game seed, equal to `game_seed(cell.seed, game_index)`.
    pub seed: u64,
    /// Players in turn order.
    pub players: Vec<P>,
    /// Strategy specs, aligned to `players`.
    pub specs: Vec<StrategySpec>,
    /// Plies played by the opening (fixed plies plus random-opening plies).
    pub opening_plies: usize,
    /// Full action sequence of the game.
    pub actions: Vec<A>,
    /// Terminal outcome; `None` iff the game was aborted by `max_plies`.
    pub outcome: Option<O>,
    /// `actions.len()`.
    pub length: usize,
    /// State after the last action.
    pub final_state: S,
    /// Canonical representative of `final_state`.
    pub final_canonical_state: S,
}

/// One position within a game: the state before a chosen action, its legal actions, and the
/// action actually taken.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PositionRecord<S, A, P, O> {
    /// Schema version this record was written under.
    pub schema_version: u32,
    /// `game_id` of the [`GameRecord`] this position belongs to.
    pub game_id: String,
    /// 0-based index into the game's `actions`; `state` is the position before `chosen_action`.
    pub ply: usize,
    /// Player to move at this position.
    pub side_to_move: P,
    /// Raw game state.
    pub state: S,
    /// Canonical representative of `state`; equals `state` when the game has no canonicalizer.
    pub canonical_state: S,
    /// `Permutation::as_slice()` mapping `state` to `canonical_state`; empty means identity/none.
    pub canonical_transform: Vec<usize>,
    /// `rules.legal_actions(state)`, in rules order.
    pub legal_actions: Vec<A>,
    /// The action actually chosen at this position.
    pub chosen_action: A,
    /// `"opening"`, `"random-opening"`, or the strategy entry name of the side to move.
    pub chosen_by: String,
    /// The game's outcome, denormalized onto every position.
    pub outcome: Option<O>,
    /// Length of the game this position belongs to, in plies.
    pub game_length: usize,
}

/// Engine ground truth for one position, keyed by the raw (not canonical) state.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AnnotationRecord<S, A, P> {
    /// Schema version this record was written under.
    pub schema_version: u32,
    /// The key: raw (not canonical) state.
    pub state: S,
    /// Canonical representative of `state`.
    pub canonical_state: S,
    /// Player to move at this position.
    pub side_to_move: P,
    /// Whether `state` is terminal.
    pub terminal: bool,
    /// Value from the side-to-move's perspective: `1` win, `0` draw, `-1` loss.
    pub value: i8,
    /// Every legal action achieving `value`, in legal order; empty iff terminal.
    pub optimal_actions: Vec<A>,
    /// `game-player` search result at `engine_depth`; `None` iff terminal.
    pub engine_action: Option<A>,
    /// `Some(optimal_actions.contains(engine_action))` iff non-terminal.
    pub engine_agrees: Option<bool>,
}

/// Win/draw/loss/unfinished counts over a set of games, with rates (`count / games`, exactly
/// `0.0` when `games == 0`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Tally {
    /// Games played (`wins + draws + losses + unfinished`).
    pub games: usize,
    /// Games the strategy under test won.
    pub wins: usize,
    /// Drawn games.
    pub draws: usize,
    /// Games the strategy under test lost.
    pub losses: usize,
    /// Games aborted by `max_plies`; never counted as losses.
    pub unfinished: usize,
    /// `wins / games`, or `0.0` when `games == 0`.
    pub win_rate: f64,
    /// `draws / games`, or `0.0` when `games == 0`.
    pub draw_rate: f64,
    /// `losses / games`, or `0.0` when `games == 0`.
    pub loss_rate: f64,
}

impl Tally {
    /// Builds a tally from counts: `games` is their sum and every rate is `count / games` (`0.0`
    /// when `games == 0`).
    pub fn from_counts(wins: usize, draws: usize, losses: usize, unfinished: usize) -> Tally {
        let games = wins + draws + losses + unfinished;
        let rate = |count: usize| if games == 0 { 0.0 } else { count as f64 / games as f64 };
        Tally {
            games,
            wins,
            draws,
            losses,
            unfinished,
            win_rate: rate(wins),
            draw_rate: rate(draws),
            loss_rate: rate(losses),
        }
    }
}

/// Results of the strategy under test against one roster opponent from one seat.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PairingResult {
    /// Roster entry name of the opponent.
    pub opponent: String,
    /// Seat index the strategy under test occupied (`bundle.players[seat]`).
    pub seat: usize,
    /// `format!("{:?}", bundle.players[seat])`.
    pub player: String,
    /// Counts and rates for this pairing, inlined into the document.
    #[serde(flatten)]
    pub tally: Tally,
}

/// Results of the strategy under test against one roster opponent over every seat.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct OpponentResult {
    /// Roster entry name of the opponent.
    pub opponent: String,
    /// Counts and rates summed over `by_seat`, inlined into the document.
    #[serde(flatten)]
    pub tally: Tally,
    /// Per-seat results, in seat order.
    pub by_seat: Vec<PairingResult>,
}

/// Full tournament of one strategy under test against a roster.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct TournamentResult {
    /// `Roster::id()` of the opposition.
    pub roster_id: String,
    /// Roster entry name the headline loss rate is measured against.
    pub reference: String,
    /// Games played per (opponent, seat) pairing.
    pub games_per_pairing: usize,
    /// Master seed of the tournament.
    pub seed: u64,
    /// Per-opponent results, in roster order.
    pub opponents: Vec<OpponentResult>,
    /// Counts and rates over every pairing.
    pub totals: Tally,
}

/// Strategy-side agreement with annotated optimal actions.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AgreementSummary {
    /// Non-terminal annotated positions evaluated.
    pub positions: usize,
    /// Of `positions`, how many chose an action in the annotation's optimal set.
    pub agreeing: usize,
    /// `agreeing / positions`, or `0.0` when `positions == 0`.
    pub rate: f64,
    /// Tallies keyed by the annotation's side-to-move value (`-1`, `0`, `1`).
    pub by_value: BTreeMap<i8, AgreementCounts>,
}

/// Chosen actions at a fixed deterministic sample of annotated positions; input to novelty.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BehaviorSignature {
    /// Hash identifying the annotation sample; signatures are comparable only when equal.
    pub sample_id: String,
    /// `serde_json::to_value(chosen_action)` per sampled position, in sample order.
    pub actions: Vec<serde_json::Value>,
}

/// Headline metrics of one evaluated strategy.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Headline {
    /// Roster entry name the headline loss rate is measured against.
    pub reference: String,
    /// Loss rate over every pairing against `reference`.
    pub loss_rate_vs_reference: f64,
    /// Win rate over all pairings.
    pub win_rate: f64,
    /// Draw rate over all pairings.
    pub draw_rate: f64,
    /// Loss rate over all pairings.
    pub loss_rate: f64,
    /// Games over all pairings.
    pub games: usize,
    /// Unfinished games over all pairings.
    pub unfinished: usize,
    /// `AgreementSummary::rate`, when annotations were supplied.
    pub agreement_rate: Option<f64>,
}

/// Everything measured about one strategy under test.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StrategyEvaluation {
    /// Strategy entry name.
    pub name: String,
    /// `spec.kind()`.
    pub kind: String,
    /// The evaluated strategy spec.
    pub spec: StrategySpec,
    /// `config_hash(&spec)`.
    pub spec_hash: String,
    /// Tournament results against the roster.
    pub tournament: TournamentResult,
    /// Headline metrics.
    pub headline: Headline,
    /// Agreement with annotations, when supplied.
    pub agreement: Option<AgreementSummary>,
    /// Behavior signature, when annotations were supplied.
    pub signature: Option<BehaviorSignature>,
}

/// Provenance of the annotation set an evaluation used.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct AnnotationSource {
    /// `AnnotateMetadata::mode` as its serialized name (`corpus` or `exhaustive`).
    pub mode: String,
    /// `AnnotateMetadata::run_id` (`None` for exhaustive annotation).
    pub run_id: Option<String>,
    /// Number of annotation records read.
    pub records: usize,
}

/// Configuration an evaluation ran with.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EvaluationConfig {
    /// Evaluator name used to build every strategy.
    pub evaluator: String,
    /// Games per (opponent, seat) pairing.
    pub games_per_pairing: usize,
    /// Master seed.
    pub seed: u64,
    /// Ply cap per game, if any.
    pub max_plies: Option<usize>,
    /// Reference roster entry name.
    pub reference: String,
    /// Annotation set used for agreement, if any.
    pub annotations: Option<AnnotationSource>,
}

/// The `evaluate` stage's document (`evaluation.json`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EvaluationReport {
    /// Schema version this document was written under.
    pub schema_version: u32,
    /// Hash identifying this evaluation (game, roster, config, strategies).
    pub evaluation_id: String,
    /// Game name.
    pub game: String,
    /// The opposition roster, in full.
    pub roster: Roster,
    /// Configuration used.
    pub config: EvaluationConfig,
    /// One entry per evaluated strategy, in strategies-file order.
    pub strategies: Vec<StrategyEvaluation>,
}

/// One line of a strategy archive's index.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ArchiveIndexEntry {
    /// Entry identity (hash of game, spec and evaluation id).
    pub entry_id: String,
    /// Strategy entry name.
    pub name: String,
    /// Strategy kind.
    pub kind: String,
    /// 0-based append order.
    pub sequence: usize,
}

/// A strategy archive's index document (`archive.json`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ArchiveIndex {
    /// Schema version this document was written under.
    pub schema_version: u32,
    /// Game every entry belongs to.
    pub game: String,
    /// Entries in append order.
    pub entries: Vec<ArchiveIndexEntry>,
}

/// Where an archive entry came from.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Provenance {
    /// Game name.
    pub game: String,
    /// Producing stage (`evaluate` or `discover`).
    pub source: String,
    /// `EvaluationReport::evaluation_id`.
    pub evaluation_id: String,
    /// `Roster::id()` of the opposition.
    pub roster_id: String,
    /// Master seed.
    pub seed: u64,
    /// Games per pairing.
    pub games_per_pairing: usize,
    /// Evaluator name.
    pub evaluator: String,
    /// Corpus run the strategy was mined from (`None` for evaluated-only entries).
    pub corpus_run_id: Option<String>,
    /// `run_id` of the annotation set used for agreement, if any.
    pub annotations_run_id: Option<String>,
    /// Mode of the annotation set used for agreement, if any.
    pub annotations_mode: Option<String>,
    /// Mining provenance of a `discover`-archived entry; absent for `evaluate` entries.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovery: Option<DiscoveryProvenance>,
}

/// Novelty of an entry relative to the entries already archived.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Novelty {
    /// Novelty method identifier (`m1-v1`).
    pub method: String,
    /// `entry_id` of the nearest existing entry, if any.
    pub nearest_entry_id: Option<String>,
    /// `name` of the nearest existing entry, if any.
    pub nearest_name: Option<String>,
    /// Minimum spec distance to existing entries; `1.0` when the archive was empty.
    pub spec_distance: f64,
    /// Minimum behavior distance over entries with a comparable signature.
    pub behavior_distance: Option<f64>,
    /// `behavior_distance.unwrap_or(spec_distance)`.
    pub distance: f64,
    /// `distance > 0.0`.
    pub is_novel: bool,
}

/// One archived strategy with its evaluation (`entries.jsonl`, one per line).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ArchiveEntry {
    /// Schema version this record was written under.
    pub schema_version: u32,
    /// Entry identity (hash of game, spec and evaluation id).
    pub entry_id: String,
    /// 0-based append order.
    pub sequence: usize,
    /// Strategy entry name.
    pub name: String,
    /// Strategy kind.
    pub kind: String,
    /// The archived strategy spec.
    pub spec: StrategySpec,
    /// `config_hash(&spec)`.
    pub spec_hash: String,
    /// Where the entry came from.
    pub provenance: Provenance,
    /// The evaluation that produced it.
    pub evaluation: StrategyEvaluation,
    /// Novelty relative to earlier entries.
    pub novelty: Novelty,
}

/// One feature column of a dataset, in vocabulary order.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DatasetColumn {
    /// Feature name.
    pub name: String,
    /// Vocabulary tier of the feature.
    pub tier: Tier,
    /// Value kind encoded in the column: `"bool"`, `"int"`, `"float"` or `"set"`.
    pub kind: String,
    /// Feature description from its definition.
    pub description: String,
}

/// Manifest of a feature dataset (`dataset.json`): columns, classes and row statistics.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DatasetManifest {
    /// Schema version this document was written under.
    pub schema_version: u32,
    /// Game name.
    pub game: String,
    /// Corpus run the annotations came from, if any.
    pub run_id: Option<String>,
    /// Annotation mode of the input (`corpus` or `exhaustive`).
    pub annotations_mode: String,
    /// Serde names of the tiers present among the columns, e.g. `["primitive", "supplied"]`.
    pub tiers: Vec<String>,
    /// Feature columns, in vocabulary order.
    pub columns: Vec<DatasetColumn>,
    /// Action classes = names of every `set`-kind column, in vocabulary order.
    pub classes: Vec<String>,
    /// Rows written (one per non-terminal canonical state).
    pub rows: usize,
    /// Annotation records read.
    pub annotated: usize,
    /// Non-terminal annotation records.
    pub nonterminal: usize,
    /// `nonterminal - rows`: records collapsed into an existing canonical row.
    pub collapsed: usize,
    /// Rows per label (including `"none"`).
    pub label_counts: BTreeMap<String, usize>,
    /// Rows per side-to-move value, keys `"-1"`, `"0"`, `"1"`.
    pub value_counts: BTreeMap<String, usize>,
    /// Rows per `side_to_move` slot, keys are the slot indices as strings.
    pub side_to_move_counts: BTreeMap<String, usize>,
    /// Rows whose label is `"none"` (fallback-risk population).
    pub unlabeled: usize,
}

/// One dataset row (`dataset.jsonl`): a non-terminal canonical state with its encoded features.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DatasetRow<S> {
    /// Schema version this record was written under.
    pub schema_version: u32,
    /// The canonical state.
    pub canonical_state: S,
    /// Slot of the player to move in turn order.
    pub side_to_move: usize,
    /// Annotated value from the mover's perspective: `1` win, `0` draw, `-1` loss.
    pub value: i8,
    /// Annotation records collapsed into this row.
    pub occurrences: usize,
    /// Legal action positions in the canonical frame, ascending, deduplicated.
    pub legal: Vec<usize>,
    /// Optimal action positions in the canonical frame, ascending, deduplicated.
    pub optimal: Vec<usize>,
    /// Action classes that qualify for this row, in vocabulary order.
    pub qualifying: Vec<String>,
    /// Preferred qualifying class, or `"none"`.
    pub label: String,
    /// Encoded feature values, one per column in column order.
    pub values: Vec<f64>,
}

/// Miner parameters: the `[mine]` experiment table, the `analyze --mine-*` flags, and the
/// `params` of a mining report. Every field defaults.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MineParams {
    /// Induction engine: `"cart"` (default, deterministic) or `"linfa-trees"`.
    #[serde(default = "default_mine_engine")]
    pub engine: String,
    /// One candidate per entry; `0` = unlimited depth.
    #[serde(default = "default_mine_depths")]
    pub depths: Vec<usize>,
    /// Minimum rows per leaf (`>= 1`).
    #[serde(default = "default_mine_min_leaf")]
    pub min_leaf: usize,
    /// Seed of the train/holdout shuffle.
    #[serde(default)]
    pub seed: u64,
    /// Fraction of rows held out for evaluation, in `[0, 1)`; `0.0` = no holdout.
    #[serde(default)]
    pub holdout_fraction: f64,
}

/// `"cart"`, the default for [`MineParams::engine`].
fn default_mine_engine() -> String {
    MINE_ENGINE_CART.to_string()
}

/// `[8]`, the default for [`MineParams::depths`].
fn default_mine_depths() -> Vec<usize> {
    vec![8]
}

/// `1`, the default for [`MineParams::min_leaf`].
fn default_mine_min_leaf() -> usize {
    1
}

impl Default for MineParams {
    fn default() -> Self {
        MineParams {
            engine: default_mine_engine(),
            depths: default_mine_depths(),
            min_leaf: default_mine_min_leaf(),
            seed: 0,
            holdout_fraction: 0.0,
        }
    }
}

impl MineParams {
    /// Rejects an unknown engine, empty or duplicate `depths`, `min_leaf == 0`, or a
    /// `holdout_fraction` outside `[0, 1)`, each as [`CorpusError::Config`].
    pub fn validate(&self) -> Result<(), CorpusError> {
        if self.engine != MINE_ENGINE_CART && self.engine != MINE_ENGINE_LINFA_TREES {
            return Err(CorpusError::Config(format!(
                "[mine] engine `{}` is not one of: {MINE_ENGINE_CART}, {MINE_ENGINE_LINFA_TREES}",
                self.engine
            )));
        }
        if self.depths.is_empty() {
            return Err(CorpusError::Config("[mine] depths must list at least one depth".to_string()));
        }
        for (i, depth) in self.depths.iter().enumerate() {
            if self.depths[..i].contains(depth) {
                return Err(CorpusError::Config(format!("[mine] depths lists {depth} more than once")));
            }
        }
        if self.min_leaf == 0 {
            return Err(CorpusError::Config("[mine] min_leaf must be >= 1, got 0".to_string()));
        }
        if !(0.0..1.0).contains(&self.holdout_fraction) {
            return Err(CorpusError::Config(format!(
                "[mine] holdout_fraction must be in [0, 1), got {}",
                self.holdout_fraction
            )));
        }
        Ok(())
    }
}

/// Concept-induction parameters: the `[induction]` experiment table. Every field defaults.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InductionParams {
    /// Master switch for concept induction.
    #[serde(default)]
    pub enabled: bool,
    /// Exclude the game-supplied tier-2 extractor from the dataset and vocabulary.
    #[serde(default)]
    pub withhold_tier2: bool,
    /// Mint the extended mechanical tier-1 families when induction is enabled.
    #[serde(default = "default_induction_extended_tier1")]
    pub extended_tier1: bool,
    /// Maximum promotion rounds.
    #[serde(default = "default_induction_rounds")]
    pub rounds: usize,
    /// Thresholds minted per numeric atom.
    #[serde(default = "default_induction_max_thresholds")]
    pub max_thresholds: usize,
    /// Level-1 predicates kept for combination.
    #[serde(default = "default_induction_beam")]
    pub beam: usize,
    /// Shortlist size handed to the downstream probe.
    #[serde(default = "default_induction_top_k")]
    pub top_k: usize,
    /// Maximum concepts promoted per round.
    #[serde(default = "default_induction_max_promoted")]
    pub max_promoted: usize,
    /// Minimum train information-gain to shortlist a candidate.
    #[serde(default = "default_induction_min_train_gain")]
    pub min_train_gain: f64,
    /// Minimum holdout information-gain to shortlist a candidate.
    #[serde(default = "default_induction_min_holdout_gain")]
    pub min_holdout_gain: f64,
    /// Fraction of induction rows held out for evaluation, in `[0, 1)`; `0.0` = no holdout.
    #[serde(default = "default_induction_holdout_fraction")]
    pub holdout_fraction: f64,
    /// Downstream CART probe depth limit; `0` = unlimited.
    #[serde(default = "default_induction_probe_depth")]
    pub probe_depth: usize,
    /// Allowed soundness drop when the downstream probe's rule count shrinks.
    #[serde(default = "default_induction_soundness_tolerance")]
    pub soundness_tolerance: f64,
    /// Required soundness rise when the downstream probe's rule count only ties.
    #[serde(default = "default_induction_min_soundness_gain")]
    pub min_soundness_gain: f64,
    /// Enable atom-pair comparison predicates during enumeration.
    #[serde(default)]
    pub compare_atoms: bool,
    /// Seed of the induction train/holdout shuffle.
    #[serde(default)]
    pub seed: u64,
    /// Optional TOML file mapping concept names to human labels, applied at render time only.
    #[serde(default)]
    pub label_map: Option<std::path::PathBuf>,
}

/// `true`, the default for [`InductionParams::extended_tier1`].
fn default_induction_extended_tier1() -> bool {
    true
}

/// `2`, the default for [`InductionParams::rounds`].
fn default_induction_rounds() -> usize {
    2
}

/// `4`, the default for [`InductionParams::max_thresholds`].
fn default_induction_max_thresholds() -> usize {
    4
}

/// `64`, the default for [`InductionParams::beam`].
fn default_induction_beam() -> usize {
    64
}

/// `8`, the default for [`InductionParams::top_k`].
fn default_induction_top_k() -> usize {
    8
}

/// `4`, the default for [`InductionParams::max_promoted`].
fn default_induction_max_promoted() -> usize {
    4
}

/// `0.01`, the default for [`InductionParams::min_train_gain`].
fn default_induction_min_train_gain() -> f64 {
    0.01
}

/// `0.005`, the default for [`InductionParams::min_holdout_gain`].
fn default_induction_min_holdout_gain() -> f64 {
    0.005
}

/// `0.25`, the default for [`InductionParams::holdout_fraction`].
fn default_induction_holdout_fraction() -> f64 {
    0.25
}

/// `6`, the default for [`InductionParams::probe_depth`].
fn default_induction_probe_depth() -> usize {
    6
}

/// `0.005`, the default for [`InductionParams::soundness_tolerance`].
fn default_induction_soundness_tolerance() -> f64 {
    0.005
}

/// `0.001`, the default for [`InductionParams::min_soundness_gain`].
fn default_induction_min_soundness_gain() -> f64 {
    0.001
}

impl Default for InductionParams {
    fn default() -> Self {
        InductionParams {
            enabled: false,
            withhold_tier2: false,
            extended_tier1: default_induction_extended_tier1(),
            rounds: default_induction_rounds(),
            max_thresholds: default_induction_max_thresholds(),
            beam: default_induction_beam(),
            top_k: default_induction_top_k(),
            max_promoted: default_induction_max_promoted(),
            min_train_gain: default_induction_min_train_gain(),
            min_holdout_gain: default_induction_min_holdout_gain(),
            holdout_fraction: default_induction_holdout_fraction(),
            probe_depth: default_induction_probe_depth(),
            soundness_tolerance: default_induction_soundness_tolerance(),
            min_soundness_gain: default_induction_min_soundness_gain(),
            compare_atoms: false,
            seed: 0,
            label_map: None,
        }
    }
}

impl InductionParams {
    /// Rejects `withhold_tier2` without `enabled`, any zero count bound, a negative gain or
    /// tolerance bound, or a `holdout_fraction` outside `[0, 1)`, each as [`CorpusError::Config`].
    pub fn validate(&self) -> Result<(), CorpusError> {
        if self.withhold_tier2 && !self.enabled {
            return Err(CorpusError::Config(
                "[induction] withhold_tier2 requires enabled = true".to_string(),
            ));
        }
        if self.rounds == 0 {
            return Err(CorpusError::Config(format!(
                "[induction] rounds must be >= 1, got {}",
                self.rounds
            )));
        }
        if self.max_thresholds == 0 {
            return Err(CorpusError::Config(format!(
                "[induction] max_thresholds must be >= 1, got {}",
                self.max_thresholds
            )));
        }
        if self.beam == 0 {
            return Err(CorpusError::Config(format!(
                "[induction] beam must be >= 1, got {}",
                self.beam
            )));
        }
        if self.top_k == 0 {
            return Err(CorpusError::Config(format!(
                "[induction] top_k must be >= 1, got {}",
                self.top_k
            )));
        }
        if self.max_promoted == 0 {
            return Err(CorpusError::Config(format!(
                "[induction] max_promoted must be >= 1, got {}",
                self.max_promoted
            )));
        }
        if self.min_train_gain < 0.0 {
            return Err(CorpusError::Config(format!(
                "[induction] min_train_gain must be >= 0, got {}",
                self.min_train_gain
            )));
        }
        if self.min_holdout_gain < 0.0 {
            return Err(CorpusError::Config(format!(
                "[induction] min_holdout_gain must be >= 0, got {}",
                self.min_holdout_gain
            )));
        }
        if self.soundness_tolerance < 0.0 {
            return Err(CorpusError::Config(format!(
                "[induction] soundness_tolerance must be >= 0, got {}",
                self.soundness_tolerance
            )));
        }
        if self.min_soundness_gain < 0.0 {
            return Err(CorpusError::Config(format!(
                "[induction] min_soundness_gain must be >= 0, got {}",
                self.min_soundness_gain
            )));
        }
        if !(0.0..1.0).contains(&self.holdout_fraction) {
            return Err(CorpusError::Config(format!(
                "[induction] holdout_fraction must be in [0, 1), got {}",
                self.holdout_fraction
            )));
        }
        Ok(())
    }
}

/// Win/draw/loss counts of the rows reaching a rule, from the mover's perspective.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct OutcomeTally {
    /// Rows with value `1`.
    pub win: usize,
    /// Rows with value `0`.
    pub draw: usize,
    /// Rows with value `-1`.
    pub loss: usize,
}

/// Training evidence for one emitted rule.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RuleEvidence {
    /// Rule name (`rule{k}`).
    pub rule: String,
    /// Action class the rule targets.
    pub class: String,
    /// Train rows reaching the leaf.
    pub support: usize,
    /// Of `support`, rows whose `qualifying` list contains `class`.
    pub sound: usize,
    /// `sound / support`.
    pub soundness: f64,
    /// Of `support`, rows whose `label` equals `class`.
    pub label_matches: usize,
    /// Sum of `occurrences` over the rows reaching the leaf.
    pub occurrences: usize,
    /// Outcome counts over the rows reaching the leaf.
    pub outcomes: OutcomeTally,
}

/// One mined candidate: the self-contained heuristic plus its training metrics.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MinedHeuristic {
    /// Candidate name (`mined-d{depth}-l{min_leaf}`, or `mined-linfa-d{depth}-l{min_leaf}`).
    pub name: String,
    /// Engine that induced the tree.
    pub engine: String,
    /// Depth limit requested (`0` = unlimited).
    pub max_depth: usize,
    /// Minimum rows per leaf requested.
    pub min_leaf: usize,
    /// The emitted, validated heuristic.
    pub heuristic: HeuristicStrategy,
    /// Rules emitted.
    pub rules: usize,
    /// Leaves of the induced tree (rules plus `none` leaves).
    pub leaves: usize,
    /// Depth actually reached.
    pub depth: usize,
    /// Training rows.
    pub train_rows: usize,
    /// Holdout rows.
    pub holdout_rows: usize,
    /// Fraction of train rows whose predicted class equals their label.
    pub train_accuracy: f64,
    /// Fraction of train rows whose predicted class qualifies (`none` never qualifies).
    pub train_soundness: f64,
    /// Holdout accuracy, when a holdout exists.
    pub holdout_accuracy: Option<f64>,
    /// Holdout soundness, when a holdout exists.
    pub holdout_soundness: Option<f64>,
    /// Train rows falling to the fallback (`none` leaves).
    pub fallback_rows: usize,
    /// Per-rule evidence, in rule order.
    pub evidence: Vec<RuleEvidence>,
}

/// The `mine` analyzer's output (`heuristics.json`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MiningReport {
    /// Schema version this document was written under.
    pub schema_version: u32,
    /// Game name.
    pub game: String,
    /// Corpus run the dataset came from, if any.
    pub run_id: Option<String>,
    /// Parameters the miner ran with.
    pub params: MineParams,
    /// Manifest of the dataset mined.
    pub dataset: DatasetManifest,
    /// One candidate per `params.depths` entry, in order.
    pub candidates: Vec<MinedHeuristic>,
}

/// Engine and tree limits of one candidate, as recorded in provenance.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CandidateParams {
    /// Induction engine.
    pub engine: String,
    /// Depth limit (`0` = unlimited).
    pub max_depth: usize,
    /// Minimum rows per leaf.
    pub min_leaf: usize,
}

/// Concept-induction provenance of a `discover` run that had `[induction]` enabled.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConceptsProvenance {
    /// `config_hash` of the effective `InductionParams`.
    pub params_hash: String,
    /// Whether tier-2 supplied features were withheld from the dataset.
    pub withhold_tier2: bool,
    /// Promotion rounds actually run.
    pub rounds_run: usize,
    /// Concepts promoted across all rounds.
    pub promoted: usize,
    /// Promoted concept names, in promotion order.
    pub concepts: Vec<String>,
    /// Induction train/holdout shuffle seed.
    pub seed: u64,
}

/// Rerun-sufficient provenance of a heuristic the `discover` stage mined and archived.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DiscoveryProvenance {
    /// Experiment name.
    pub experiment: String,
    /// `config_hash` of the experiment config as loaded.
    pub config_hash: String,
    /// Corpus run the heuristic was mined from.
    pub corpus_run_id: String,
    /// Annotation mode of the mined annotations.
    pub annotations_mode: String,
    /// Miner identifier (`cart-v1` or `linfa-trees-0.8.1`).
    pub miner: String,
    /// Engine and tree limits of the candidate.
    pub params: CandidateParams,
    /// Seed of the train/holdout shuffle.
    pub mine_seed: u64,
    /// Holdout fraction used.
    pub holdout_fraction: f64,
    /// Serde names of the feature tiers in the dataset.
    pub tiers: Vec<String>,
    /// Dataset rows.
    pub dataset_rows: usize,
    /// Candidate name within the mining report.
    pub candidate: String,
    /// Concept-induction provenance, when induction was enabled; absent otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concepts: Option<ConceptsProvenance>,
}

/// Corpus run, effective-params hash, seed, and promotion round of a promoted concept.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConceptProvenance {
    /// Corpus run id the concept was induced from, if any.
    pub run_id: Option<String>,
    /// `config_hash` of the effective `InductionParams`.
    pub params_hash: String,
    /// Induction train/holdout shuffle seed.
    pub seed: u64,
    /// Promotion round the concept was promoted in.
    pub round: usize,
}

/// Information-gain and downstream-probe metrics of a promoted concept.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConceptScores {
    /// Information gain on the training split.
    pub train_gain: f64,
    /// Information gain on the holdout split, when a holdout exists.
    pub holdout_gain: Option<f64>,
    /// Downstream probe's rule count before adding this concept.
    pub probe_rules_before: usize,
    /// Downstream probe's rule count after adding this concept.
    pub probe_rules_after: usize,
    /// Downstream probe's soundness before adding this concept.
    pub probe_soundness_before: f64,
    /// Downstream probe's soundness after adding this concept.
    pub probe_soundness_after: f64,
}

/// Extensional agreement between a promoted concept and one supplied feature.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct SimilarityEntry {
    /// Name of the supplied feature compared against.
    pub feature: String,
    /// Fraction of rows on which the concept and the feature agree.
    pub agreement: f64,
}

/// One concept promoted by concept induction.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct PromotedConcept {
    /// Promoted concept name (`concept_{n}`).
    pub name: String,
    /// The `FeatureDef`'s `Display` string.
    pub definition: String,
    /// The self-contained feature definition.
    pub def: FeatureDef,
    /// Concept value kind; `"bool"` in v1.
    pub kind: String,
    /// Promotion round.
    pub round: usize,
    /// Gain and downstream-probe metrics.
    pub scores: ConceptScores,
    /// Rerun-sufficient provenance.
    pub provenance: ConceptProvenance,
    /// Extensional agreement with supplied features.
    pub similarity: Vec<SimilarityEntry>,
}

/// One candidate shortlisted for downstream probing during a promotion round.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ShortlistEntry {
    /// The candidate's feature-expression rendering.
    pub expr: String,
    /// Information gain on the training split.
    pub train_gain: f64,
    /// Information gain on the holdout split, when a holdout exists.
    pub holdout_gain: Option<f64>,
    /// `"promoted:concept_{n}"`, `"rejected-downstream"`, or `"duplicate"`.
    pub outcome: String,
}

/// Per-round enumeration, deduplication, scoring, and promotion counters.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RoundReport {
    /// Round number.
    pub round: usize,
    /// Columns available to this round.
    pub columns: usize,
    /// Candidate predicates enumerated.
    pub enumerated: usize,
    /// Candidates dropped by deduplication (constant or duplicate).
    pub deduplicated: usize,
    /// Candidates scored.
    pub scored: usize,
    /// Candidates passing the gain filters.
    pub passed: usize,
    /// Shortlisted candidates, in shortlist order.
    pub shortlisted: Vec<ShortlistEntry>,
    /// Promoted concept names, in promotion order.
    pub promoted: Vec<String>,
}

/// The `concepts` analyzer's output (`concepts.json`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ConceptReport {
    /// Schema version this document was written under.
    pub schema_version: u32,
    /// Game name.
    pub game: String,
    /// Corpus run the dataset came from, if any.
    pub run_id: Option<String>,
    /// Parameters induction ran with.
    pub params: InductionParams,
    /// Names of features withheld from the dataset.
    pub withheld: Vec<String>,
    /// Dataset rows.
    pub rows: usize,
    /// Training rows.
    pub train_rows: usize,
    /// Holdout rows.
    pub holdout_rows: usize,
    /// Columns available before any concept was promoted.
    pub base_columns: usize,
    /// Action classes present in the base dataset.
    pub base_classes: usize,
    /// Per-round reports, in round order.
    pub rounds: Vec<RoundReport>,
    /// Promoted concepts, in promotion order.
    pub promoted: Vec<PromotedConcept>,
    /// The label map verbatim; `{}` when none was given.
    pub labels: BTreeMap<String, String>,
}

/// Given-versus-discovered usage breakdown of a feature or the whole vocabulary.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UsageBreakdown {
    /// Rows or rules referencing the feature(s).
    pub referenced: usize,
    /// Counts by tier; keys are always exactly `"primitive"`, `"supplied"`, `"invented"`.
    pub counts: BTreeMap<String, usize>,
    /// Fractions by tier; keys are always exactly `"primitive"`, `"supplied"`, `"invented"`.
    pub fractions: BTreeMap<String, f64>,
}

/// Usage breakdown for one vocabulary candidate.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CandidateUsage {
    /// Candidate feature name.
    pub name: String,
    /// Usage breakdown for this candidate.
    pub usage: UsageBreakdown,
}

/// The `vocabulary` analyzer's output (`vocabulary.json`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct VocabularyReport {
    /// Schema version this document was written under.
    pub schema_version: u32,
    /// Game name.
    pub game: String,
    /// Corpus run the dataset came from, if any.
    pub run_id: Option<String>,
    /// Feature counts by tier; keys are always exactly `"primitive"`, `"supplied"`, `"invented"`.
    pub tiers: BTreeMap<String, usize>,
    /// Per-candidate usage breakdowns.
    pub candidates: Vec<CandidateUsage>,
    /// Overall usage breakdown across all candidates.
    pub overall: UsageBreakdown,
}

/// One archived candidate as listed in the `discover` manifest.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DiscoverCandidate {
    /// Candidate name.
    pub name: String,
    /// Archive entry id.
    pub entry_id: String,
    /// Rules emitted.
    pub rules: usize,
    /// Headline loss rate against the reference.
    pub loss_rate_vs_reference: f64,
    /// Agreement rate with annotated optimal actions, when measured.
    pub agreement_rate: Option<f64>,
    /// Novelty distance of the archive entry.
    pub novelty: f64,
}

/// The `discover` stage's manifest (`discover.json`).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DiscoverManifest {
    /// Schema version this document was written under.
    pub schema_version: u32,
    /// Experiment name.
    pub name: String,
    /// Game name.
    pub game: String,
    /// Corpus run id of the generated corpus.
    pub run_id: String,
    /// `config_hash` of the experiment config as loaded.
    pub config_hash: String,
    /// `EvaluationReport::evaluation_id` of the harness run.
    pub evaluation_id: String,
    /// `Roster::id()` of the opposition.
    pub roster_id: String,
    /// Archive directory relative to the run directory, or the absolute path given.
    pub archive: String,
    /// Archived candidates, in mining-report order.
    pub candidates: Vec<DiscoverCandidate>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::tictactoe::{Board, Move, Outcome, Player, TicTacToe};
    use crate::strategy::minimax::{MinimaxConfig, TieBreak};
    use std::path::Path;

    fn assert_corpus_game<G: CorpusGame>() {}

    #[test]
    fn tictactoe_is_a_corpus_game() {
        assert_corpus_game::<TicTacToe>();
    }

    #[test]
    fn check_schema_version_accepts_current_and_rejects_others() {
        assert!(check_schema_version(Path::new("p"), 1).is_ok());
        let err = check_schema_version(Path::new("p"), 99);
        match &err {
            Err(IoError::SchemaVersion { expected, found, .. }) => {
                assert_eq!(*expected, 1);
                assert_eq!(*found, 99);
            }
            other => panic!("expected SchemaVersion error, got {other:?}"),
        }
        assert!(err.unwrap_err().to_string().contains("schema_version 99"));
    }

    #[test]
    fn records_round_trip_through_json_and_carry_schema_version() {
        let cell = CellKey {
            index: 0,
            strategies: vec!["random".to_string(), "perfect".to_string()],
            evaluator: "default".to_string(),
            opening: "none".to_string(),
            random_opening_plies: 0,
        };

        let game = GameRecord::<Board, Move, Player, Outcome> {
            schema_version: SCHEMA_VERSION,
            run_id: "r".to_string(),
            game_id: "0:0".to_string(),
            cell: cell.clone(),
            game_index: 0,
            seed: 42,
            players: vec![Player::X, Player::O],
            specs: vec![
                StrategySpec::Random,
                StrategySpec::Minimax(MinimaxConfig {
                    depth: 9,
                    epsilon: 0.0,
                    tie_break: TieBreak::SeededUniform,
                }),
            ],
            opening_plies: 0,
            actions: vec![Move(4), Move(0)],
            outcome: None,
            length: 2,
            final_state: Board::parse("O...X....").unwrap(),
            final_canonical_state: Board::parse("O...X....").unwrap(),
        };
        let json = serde_json::to_string(&game).unwrap();
        let round_tripped: GameRecord<Board, Move, Player, Outcome> = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, game);
        assert_eq!(serde_json::to_value(&game).unwrap()["schema_version"], 1);

        let position = PositionRecord::<Board, Move, Player, Outcome> {
            schema_version: SCHEMA_VERSION,
            game_id: "0:0".to_string(),
            ply: 0,
            side_to_move: Player::X,
            state: Board::empty(),
            canonical_state: Board::empty(),
            canonical_transform: vec![],
            legal_actions: vec![Move(0), Move(1)],
            chosen_action: Move(4),
            chosen_by: "opening".to_string(),
            outcome: None,
            game_length: 2,
        };
        let json = serde_json::to_string(&position).unwrap();
        let round_tripped: PositionRecord<Board, Move, Player, Outcome> = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, position);
        assert_eq!(serde_json::to_value(&position).unwrap()["schema_version"], 1);
        assert!(json.starts_with("{\"schema_version\":1,\"game_id\":"));

        let annotation = AnnotationRecord::<Board, Move, Player> {
            schema_version: SCHEMA_VERSION,
            state: Board::empty(),
            canonical_state: Board::empty(),
            side_to_move: Player::X,
            terminal: false,
            value: 0,
            optimal_actions: vec![Move(4)],
            engine_action: Some(Move(4)),
            engine_agrees: Some(true),
        };
        let json = serde_json::to_string(&annotation).unwrap();
        let round_tripped: AnnotationRecord<Board, Move, Player> = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, annotation);
        assert_eq!(serde_json::to_value(&annotation).unwrap()["schema_version"], 1);
    }

    #[test]
    fn file_name_constants() {
        assert_eq!(RUN_FILE, "run.json");
        assert_eq!(GAMES_FILE, "games.jsonl");
        assert_eq!(POSITIONS_FILE, "positions.jsonl");
        assert_eq!(ANNOTATIONS_FILE, "annotations.jsonl");
        assert_eq!(ANNOTATE_FILE, "annotate.json");
        assert_eq!(SUMMARY_FILE, "summary.json");
        assert_eq!(ANALYZE_FILE, "analyze.json");
        assert_eq!(SCHEMA_VERSION, 1);
        assert_eq!(EVALUATION_FILE, "evaluation.json");
        assert_eq!(ARCHIVE_FILE, "archive.json");
        assert_eq!(ARCHIVE_ENTRIES_FILE, "entries.jsonl");
    }

    fn keys(value: &serde_json::Value) -> Vec<String> {
        value.as_object().unwrap().keys().cloned().collect()
    }

    #[test]
    fn tally_from_counts_rates() {
        let zero = Tally::from_counts(0, 0, 0, 0);
        assert_eq!(zero.games, 0);
        assert_eq!(zero.win_rate, 0.0);
        assert_eq!(zero.draw_rate, 0.0);
        assert_eq!(zero.loss_rate, 0.0);

        let mixed = Tally::from_counts(1, 2, 1, 0);
        assert_eq!(mixed.games, 4);
        assert_eq!(mixed.win_rate, 0.25);
        assert_eq!(mixed.draw_rate, 0.5);
        assert_eq!(mixed.loss_rate, 0.25);

        let unfinished = Tally::from_counts(0, 1, 0, 1);
        assert_eq!(unfinished.games, 2);
        assert_eq!(unfinished.unfinished, 1);
        assert_eq!(unfinished.loss_rate, 0.0);
    }

    #[test]
    fn tournament_records_round_trip_and_field_names() {
        let pairing = PairingResult {
            opponent: "random".to_string(),
            seat: 0,
            player: "X".to_string(),
            tally: Tally::from_counts(1, 1, 0, 0),
        };
        let json = serde_json::to_string(&pairing).unwrap();
        assert_eq!(
            json,
            r#"{"opponent":"random","seat":0,"player":"X","games":2,"wins":1,"draws":1,"losses":0,"unfinished":0,"win_rate":0.5,"draw_rate":0.5,"loss_rate":0.0}"#
        );
        let round_tripped: PairingResult = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, pairing);
        assert_eq!(
            keys(&serde_json::to_value(&pairing).unwrap()),
            vec![
                "draw_rate",
                "draws",
                "games",
                "loss_rate",
                "losses",
                "opponent",
                "player",
                "seat",
                "unfinished",
                "win_rate",
                "wins",
            ]
        );

        let pairing2 = PairingResult {
            opponent: "random".to_string(),
            seat: 1,
            player: "O".to_string(),
            tally: Tally::from_counts(1, 1, 0, 0),
        };
        let opponent = OpponentResult {
            opponent: "random".to_string(),
            tally: Tally::from_counts(2, 2, 0, 0),
            by_seat: vec![pairing.clone(), pairing2],
        };
        let json = serde_json::to_string(&opponent).unwrap();
        assert!(json.starts_with(r#"{"opponent":"random","games":4,"#));
        assert!(json.ends_with("}]}"));
        let round_tripped: OpponentResult = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, opponent);
        assert_eq!(
            keys(&serde_json::to_value(&opponent).unwrap()),
            vec![
                "by_seat",
                "draw_rate",
                "draws",
                "games",
                "loss_rate",
                "losses",
                "opponent",
                "unfinished",
                "win_rate",
                "wins",
            ]
        );

        let tournament = TournamentResult {
            roster_id: "ttt-v1".to_string(),
            reference: "perfect".to_string(),
            games_per_pairing: 2,
            seed: 0,
            opponents: vec![opponent],
            totals: Tally::from_counts(2, 2, 0, 0),
        };
        let json = serde_json::to_string(&tournament).unwrap();
        let round_tripped: TournamentResult = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, tournament);
        let value = serde_json::to_value(&tournament).unwrap();
        assert_eq!(
            keys(&value),
            vec!["games_per_pairing", "opponents", "reference", "roster_id", "seed", "totals"]
        );
        assert_eq!(
            keys(&value["totals"]),
            vec![
                "draw_rate",
                "draws",
                "games",
                "loss_rate",
                "losses",
                "unfinished",
                "win_rate",
                "wins"
            ]
        );
    }

    #[test]
    fn evaluation_records_round_trip_and_field_names() {
        let agreement = AgreementSummary {
            positions: 3,
            agreeing: 2,
            rate: 2.0 / 3.0,
            by_value: BTreeMap::from([
                (
                    -1,
                    AgreementCounts {
                        positions: 1,
                        agreeing: 1,
                        rate: 1.0,
                    },
                ),
                (
                    0,
                    AgreementCounts {
                        positions: 1,
                        agreeing: 1,
                        rate: 1.0,
                    },
                ),
                (
                    1,
                    AgreementCounts {
                        positions: 1,
                        agreeing: 0,
                        rate: 0.0,
                    },
                ),
            ]),
        };
        let json = serde_json::to_string(&agreement).unwrap();
        let round_tripped: AgreementSummary = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, agreement);
        assert!(json.contains(r#""by_value":{"-1":{"#));
        assert_eq!(
            keys(&serde_json::to_value(&agreement).unwrap()),
            vec!["agreeing", "by_value", "positions", "rate"]
        );

        let signature = BehaviorSignature {
            sample_id: "abc".to_string(),
            actions: vec![serde_json::json!(4), serde_json::json!({"cell": 0})],
        };
        let json = serde_json::to_string(&signature).unwrap();
        let round_tripped: BehaviorSignature = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, signature);
        assert_eq!(keys(&serde_json::to_value(&signature).unwrap()), vec!["actions", "sample_id"]);

        let headline = Headline {
            reference: "perfect".to_string(),
            loss_rate_vs_reference: 0.0,
            win_rate: 0.5,
            draw_rate: 0.5,
            loss_rate: 0.0,
            games: 4,
            unfinished: 0,
            agreement_rate: Some(1.0),
        };
        let json = serde_json::to_string(&headline).unwrap();
        let round_tripped: Headline = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, headline);
        assert_eq!(
            keys(&serde_json::to_value(&headline).unwrap()),
            vec![
                "agreement_rate",
                "draw_rate",
                "games",
                "loss_rate",
                "loss_rate_vs_reference",
                "reference",
                "unfinished",
                "win_rate",
            ]
        );

        let pairing = PairingResult {
            opponent: "random".to_string(),
            seat: 0,
            player: "X".to_string(),
            tally: Tally::from_counts(1, 1, 0, 0),
        };
        let opponent = OpponentResult {
            opponent: "random".to_string(),
            tally: Tally::from_counts(1, 1, 0, 0),
            by_seat: vec![pairing],
        };
        let tournament = TournamentResult {
            roster_id: "ttt-v1".to_string(),
            reference: "perfect".to_string(),
            games_per_pairing: 2,
            seed: 0,
            opponents: vec![opponent],
            totals: Tally::from_counts(1, 1, 0, 0),
        };

        let evaluation = StrategyEvaluation {
            name: "perfect".to_string(),
            kind: "minimax".to_string(),
            spec: StrategySpec::Minimax(MinimaxConfig {
                depth: 9,
                epsilon: 0.0,
                tie_break: TieBreak::SeededUniform,
            }),
            spec_hash: "0123456789abcdef".to_string(),
            tournament,
            headline,
            agreement: Some(agreement),
            signature: Some(signature),
        };
        let json = serde_json::to_string(&evaluation).unwrap();
        let round_tripped: StrategyEvaluation = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, evaluation);
        let value = serde_json::to_value(&evaluation).unwrap();
        assert_eq!(
            keys(&value),
            vec![
                "agreement",
                "headline",
                "kind",
                "name",
                "signature",
                "spec",
                "spec_hash",
                "tournament"
            ]
        );
        assert_eq!(value["spec"]["kind"], "minimax");

        let report = EvaluationReport {
            schema_version: SCHEMA_VERSION,
            evaluation_id: "feedfacefeedface".to_string(),
            game: "ttt".to_string(),
            roster: Roster::graded("ttt", 1, &[2], 9).unwrap(),
            config: EvaluationConfig {
                evaluator: "default".to_string(),
                games_per_pairing: 2,
                seed: 0,
                max_plies: None,
                reference: "perfect".to_string(),
                annotations: Some(AnnotationSource {
                    mode: "corpus".to_string(),
                    run_id: Some("5eafa65f76637df3".to_string()),
                    records: 390,
                }),
            },
            strategies: vec![evaluation],
        };
        let json = serde_json::to_string(&report).unwrap();
        let round_tripped: EvaluationReport = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, report);
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(value["schema_version"], 1);
        assert_eq!(
            keys(&value),
            vec!["config", "evaluation_id", "game", "roster", "schema_version", "strategies"]
        );
        assert_eq!(
            keys(&value["config"]),
            vec![
                "annotations",
                "evaluator",
                "games_per_pairing",
                "max_plies",
                "reference",
                "seed"
            ]
        );
        assert_eq!(keys(&value["config"]["annotations"]), vec!["mode", "records", "run_id"]);
        assert!(json.starts_with(r#"{"schema_version":1,"evaluation_id":"feedfacefeedface","game":"ttt","roster":{"#));
    }

    #[test]
    fn archive_records_round_trip_and_field_names() {
        let index = ArchiveIndex {
            schema_version: SCHEMA_VERSION,
            game: "ttt".to_string(),
            entries: vec![ArchiveIndexEntry {
                entry_id: "e0".to_string(),
                name: "perfect".to_string(),
                kind: "minimax".to_string(),
                sequence: 0,
            }],
        };
        let json = serde_json::to_string(&index).unwrap();
        let round_tripped: ArchiveIndex = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, index);
        let value = serde_json::to_value(&index).unwrap();
        assert_eq!(keys(&value), vec!["entries", "game", "schema_version"]);
        assert_eq!(keys(&value["entries"][0]), vec!["entry_id", "kind", "name", "sequence"]);

        let spec = StrategySpec::Minimax(MinimaxConfig {
            depth: 9,
            epsilon: 0.0,
            tie_break: TieBreak::SeededUniform,
        });

        let agreement = AgreementSummary {
            positions: 3,
            agreeing: 2,
            rate: 2.0 / 3.0,
            by_value: BTreeMap::from([(
                0,
                AgreementCounts {
                    positions: 3,
                    agreeing: 2,
                    rate: 2.0 / 3.0,
                },
            )]),
        };
        let signature = BehaviorSignature {
            sample_id: "abc".to_string(),
            actions: vec![serde_json::json!(4)],
        };
        let headline = Headline {
            reference: "perfect".to_string(),
            loss_rate_vs_reference: 0.0,
            win_rate: 0.5,
            draw_rate: 0.5,
            loss_rate: 0.0,
            games: 4,
            unfinished: 0,
            agreement_rate: Some(1.0),
        };
        let pairing = PairingResult {
            opponent: "random".to_string(),
            seat: 0,
            player: "X".to_string(),
            tally: Tally::from_counts(1, 1, 0, 0),
        };
        let opponent = OpponentResult {
            opponent: "random".to_string(),
            tally: Tally::from_counts(1, 1, 0, 0),
            by_seat: vec![pairing],
        };
        let tournament = TournamentResult {
            roster_id: "ttt-v1".to_string(),
            reference: "perfect".to_string(),
            games_per_pairing: 2,
            seed: 0,
            opponents: vec![opponent],
            totals: Tally::from_counts(1, 1, 0, 0),
        };
        let evaluation = StrategyEvaluation {
            name: "perfect".to_string(),
            kind: "minimax".to_string(),
            spec: spec.clone(),
            spec_hash: "0123456789abcdef".to_string(),
            tournament,
            headline,
            agreement: Some(agreement),
            signature: Some(signature),
        };

        let entry = ArchiveEntry {
            schema_version: SCHEMA_VERSION,
            entry_id: "e0".to_string(),
            sequence: 0,
            name: "perfect".to_string(),
            kind: "minimax".to_string(),
            spec,
            spec_hash: "0123456789abcdef".to_string(),
            provenance: Provenance {
                game: "ttt".to_string(),
                source: "evaluate".to_string(),
                evaluation_id: "feedfacefeedface".to_string(),
                roster_id: "ttt-v1".to_string(),
                seed: 0,
                games_per_pairing: 2,
                evaluator: "default".to_string(),
                corpus_run_id: None,
                annotations_run_id: None,
                annotations_mode: Some("exhaustive".to_string()),
                discovery: None,
            },
            evaluation,
            novelty: Novelty {
                method: "m1-v1".to_string(),
                nearest_entry_id: None,
                nearest_name: None,
                spec_distance: 1.0,
                behavior_distance: None,
                distance: 1.0,
                is_novel: true,
            },
        };
        let json = serde_json::to_string(&entry).unwrap();
        let round_tripped: ArchiveEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, entry);
        assert!(json.starts_with(
            r#"{"schema_version":1,"entry_id":"e0","sequence":0,"name":"perfect","kind":"minimax","spec":{"kind":"minimax","#
        ));
        let value = serde_json::to_value(&entry).unwrap();
        assert_eq!(
            keys(&value),
            vec![
                "entry_id",
                "evaluation",
                "kind",
                "name",
                "novelty",
                "provenance",
                "schema_version",
                "sequence",
                "spec",
                "spec_hash",
            ]
        );
        assert_eq!(
            keys(&value["provenance"]),
            vec![
                "annotations_mode",
                "annotations_run_id",
                "corpus_run_id",
                "evaluation_id",
                "evaluator",
                "game",
                "games_per_pairing",
                "roster_id",
                "seed",
                "source",
            ]
        );
        assert_eq!(
            keys(&value["novelty"]),
            vec![
                "behavior_distance",
                "distance",
                "is_novel",
                "method",
                "nearest_entry_id",
                "nearest_name",
                "spec_distance",
            ]
        );
        assert!(value["provenance"]["corpus_run_id"].is_null());
    }

    #[test]
    fn new_file_consts() {
        assert_eq!(DATASET_FILE, "dataset.jsonl");
        assert_eq!(DATASET_MANIFEST_FILE, "dataset.json");
        assert_eq!(HEURISTICS_FILE, "heuristics.json");
        assert_eq!(DISCOVER_FILE, "discover.json");
        assert_eq!(MINE_ENGINE_CART, "cart");
        assert_eq!(MINE_ENGINE_LINFA_TREES, "linfa-trees");
    }

    #[test]
    fn mine_params_defaults() {
        let expected = MineParams {
            engine: "cart".to_string(),
            depths: vec![8],
            min_leaf: 1,
            seed: 0,
            holdout_fraction: 0.0,
        };
        assert_eq!(MineParams::default(), expected);
        assert_eq!(serde_json::from_str::<MineParams>("{}").unwrap(), expected);
        assert_eq!(toml::from_str::<MineParams>("").unwrap(), expected);
        assert!(toml::from_str::<MineParams>("nope = 1").is_err());
        assert!(MineParams::default().validate().is_ok());
    }

    #[test]
    fn mine_params_validate_rejects_bad_values() {
        fn err_msg(params: MineParams) -> String {
            match params.validate() {
                Err(CorpusError::Config(msg)) => msg,
                other => panic!("expected Config error, got {other:?}"),
            }
        }

        let base = MineParams::default();

        let mut p = base.clone();
        p.engine = "x".to_string();
        assert!(err_msg(p).contains("[mine] engine"));

        let mut p = base.clone();
        p.depths = vec![];
        assert!(err_msg(p).contains("at least one depth"));

        let mut p = base.clone();
        p.depths = vec![4, 4];
        assert!(err_msg(p).contains("more than once"));

        let mut p = base.clone();
        p.min_leaf = 0;
        assert!(err_msg(p).contains("min_leaf"));

        let mut p = base.clone();
        p.holdout_fraction = 1.0;
        assert!(err_msg(p).contains("holdout_fraction"));

        let mut p = base.clone();
        p.holdout_fraction = -0.1;
        assert!(err_msg(p).contains("holdout_fraction"));

        let mut p = base.clone();
        p.engine = "linfa-trees".to_string();
        p.holdout_fraction = 0.2;
        p.depths = vec![0, 8];
        assert!(p.validate().is_ok());
    }

    #[test]
    fn m3_file_consts() {
        assert_eq!(CONCEPTS_FILE, "concepts.json");
        assert_eq!(VOCABULARY_FILE, "vocabulary.json");
    }

    #[test]
    fn induction_params_defaults() {
        let expected = InductionParams {
            enabled: false,
            withhold_tier2: false,
            extended_tier1: true,
            rounds: 2,
            max_thresholds: 4,
            beam: 64,
            top_k: 8,
            max_promoted: 4,
            min_train_gain: 0.01,
            min_holdout_gain: 0.005,
            holdout_fraction: 0.25,
            probe_depth: 6,
            soundness_tolerance: 0.005,
            min_soundness_gain: 0.001,
            compare_atoms: false,
            seed: 0,
            label_map: None,
        };
        assert_eq!(InductionParams::default(), expected);
        assert_eq!(serde_json::from_str::<InductionParams>("{}").unwrap(), expected);
        assert_eq!(toml::from_str::<InductionParams>("").unwrap(), expected);
        assert!(toml::from_str::<InductionParams>("nope = 1").is_err());

        let mut expected_partial = expected.clone();
        expected_partial.enabled = true;
        expected_partial.withhold_tier2 = true;
        expected_partial.rounds = 3;
        let parsed: InductionParams = toml::from_str("enabled = true\nwithhold_tier2 = true\nrounds = 3").unwrap();
        assert_eq!(parsed, expected_partial);
        assert!(parsed.validate().is_ok());

        assert!(InductionParams::default().validate().is_ok());
    }

    #[test]
    fn induction_params_validate_rejects_bad_values() {
        fn err_msg(params: InductionParams) -> String {
            match params.validate() {
                Err(CorpusError::Config(msg)) => msg,
                other => panic!("expected Config error, got {other:?}"),
            }
        }

        let base = InductionParams::default();

        let mut p = base.clone();
        p.rounds = 0;
        assert!(err_msg(p).contains("[induction] rounds"));

        let mut p = base.clone();
        p.max_thresholds = 0;
        assert!(err_msg(p).contains("[induction] max_thresholds"));

        let mut p = base.clone();
        p.beam = 0;
        assert!(err_msg(p).contains("[induction] beam"));

        let mut p = base.clone();
        p.top_k = 0;
        assert!(err_msg(p).contains("[induction] top_k"));

        let mut p = base.clone();
        p.max_promoted = 0;
        assert!(err_msg(p).contains("[induction] max_promoted"));

        let mut p = base.clone();
        p.min_train_gain = -0.1;
        assert!(err_msg(p).contains("[induction] min_train_gain"));

        let mut p = base.clone();
        p.min_holdout_gain = -0.1;
        assert!(err_msg(p).contains("[induction] min_holdout_gain"));

        let mut p = base.clone();
        p.holdout_fraction = 1.0;
        assert!(err_msg(p).contains("[induction] holdout_fraction"));

        let mut p = base.clone();
        p.holdout_fraction = -0.1;
        assert!(err_msg(p).contains("[induction] holdout_fraction"));

        let mut p = base.clone();
        p.soundness_tolerance = -0.1;
        assert!(err_msg(p).contains("[induction] soundness_tolerance"));

        let mut p = base.clone();
        p.min_soundness_gain = -0.1;
        assert!(err_msg(p).contains("[induction] min_soundness_gain"));

        let mut p = base.clone();
        p.withhold_tier2 = true;
        p.enabled = false;
        assert!(err_msg(p).contains("withhold_tier2 requires enabled = true"));

        let mut p = base.clone();
        p.probe_depth = 0;
        assert!(p.validate().is_ok());

        let mut p = base;
        p.enabled = true;
        p.withhold_tier2 = true;
        assert!(p.validate().is_ok());
    }

    #[test]
    fn concepts_provenance_is_optional_and_round_trips() {
        let base_discovery = DiscoveryProvenance {
            experiment: "e".to_string(),
            config_hash: "abc".to_string(),
            corpus_run_id: "r".to_string(),
            annotations_mode: "corpus".to_string(),
            miner: "cart-v1".to_string(),
            params: CandidateParams {
                engine: "cart".to_string(),
                max_depth: 8,
                min_leaf: 1,
            },
            mine_seed: 0,
            holdout_fraction: 0.0,
            tiers: vec!["primitive".to_string()],
            dataset_rows: 10,
            candidate: "mined-d8-l1".to_string(),
            concepts: None,
        };
        let json = serde_json::to_string(&base_discovery).unwrap();
        assert!(!json.contains("concepts"));

        let parsed: DiscoveryProvenance = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.concepts, None);

        let with_concepts = DiscoveryProvenance {
            concepts: Some(ConceptsProvenance {
                params_hash: "def".to_string(),
                withhold_tier2: false,
                rounds_run: 2,
                promoted: 1,
                concepts: vec!["concept_1".to_string()],
                seed: 0,
            }),
            ..base_discovery
        };
        let json = serde_json::to_string(&with_concepts).unwrap();
        let round_tripped: DiscoveryProvenance = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, with_concepts);
    }

    #[test]
    fn concept_and_vocabulary_reports_round_trip() {
        use crate::core::features::FeatureExpr;

        let concept_report = ConceptReport {
            schema_version: SCHEMA_VERSION,
            game: "ttt".to_string(),
            run_id: Some("run1".to_string()),
            params: InductionParams::default(),
            withheld: vec![],
            rows: 10,
            train_rows: 8,
            holdout_rows: 2,
            base_columns: 5,
            base_classes: 3,
            rounds: vec![RoundReport {
                round: 1,
                columns: 5,
                enumerated: 20,
                deduplicated: 18,
                scored: 18,
                passed: 3,
                shortlisted: vec![ShortlistEntry {
                    expr: "center".to_string(),
                    train_gain: 0.05,
                    holdout_gain: Some(0.04),
                    outcome: "promoted:concept_1".to_string(),
                }],
                promoted: vec!["concept_1".to_string()],
            }],
            promoted: vec![PromotedConcept {
                name: "concept_1".to_string(),
                definition: "concept_1 := true".to_string(),
                def: FeatureDef::derived(
                    "concept_1",
                    Tier::Invented,
                    "invented concept (round 1)",
                    FeatureExpr::Const {
                        value: crate::core::features::FeatureValue::Bool(true),
                    },
                ),
                kind: "bool".to_string(),
                round: 1,
                scores: ConceptScores {
                    train_gain: 0.05,
                    holdout_gain: Some(0.04),
                    probe_rules_before: 4,
                    probe_rules_after: 3,
                    probe_soundness_before: 0.9,
                    probe_soundness_after: 0.95,
                },
                provenance: ConceptProvenance {
                    run_id: Some("run1".to_string()),
                    params_hash: "abc".to_string(),
                    seed: 0,
                    round: 1,
                },
                similarity: vec![SimilarityEntry {
                    feature: "center".to_string(),
                    agreement: 0.99,
                }],
            }],
            labels: BTreeMap::new(),
        };
        let json = serde_json::to_string(&concept_report).unwrap();
        let round_tripped: ConceptReport = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, concept_report);

        let vocabulary_report = VocabularyReport {
            schema_version: SCHEMA_VERSION,
            game: "ttt".to_string(),
            run_id: Some("run1".to_string()),
            tiers: BTreeMap::from([
                ("primitive".to_string(), 5),
                ("supplied".to_string(), 2),
                ("invented".to_string(), 1),
            ]),
            candidates: vec![CandidateUsage {
                name: "center".to_string(),
                usage: UsageBreakdown {
                    referenced: 4,
                    counts: BTreeMap::from([
                        ("primitive".to_string(), 4),
                        ("supplied".to_string(), 0),
                        ("invented".to_string(), 0),
                    ]),
                    fractions: BTreeMap::from([
                        ("primitive".to_string(), 1.0),
                        ("supplied".to_string(), 0.0),
                        ("invented".to_string(), 0.0),
                    ]),
                },
            }],
            overall: UsageBreakdown {
                referenced: 8,
                counts: BTreeMap::from([
                    ("primitive".to_string(), 5),
                    ("supplied".to_string(), 2),
                    ("invented".to_string(), 1),
                ]),
                fractions: BTreeMap::from([
                    ("primitive".to_string(), 0.625),
                    ("supplied".to_string(), 0.25),
                    ("invented".to_string(), 0.125),
                ]),
            },
        };
        let json = serde_json::to_string(&vocabulary_report).unwrap();
        let round_tripped: VocabularyReport = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, vocabulary_report);
    }

    #[test]
    fn provenance_without_discovery_serializes_unchanged() {
        let base = Provenance {
            game: "ttt".to_string(),
            source: "evaluate".to_string(),
            evaluation_id: "feedfacefeedface".to_string(),
            roster_id: "ttt-v1".to_string(),
            seed: 0,
            games_per_pairing: 2,
            evaluator: "default".to_string(),
            corpus_run_id: None,
            annotations_run_id: None,
            annotations_mode: None,
            discovery: None,
        };
        let json = serde_json::to_string(&base).unwrap();
        assert!(!json.contains("discovery"));

        let value = serde_json::json!({
            "game": "ttt",
            "source": "evaluate",
            "evaluation_id": "feedfacefeedface",
            "roster_id": "ttt-v1",
            "seed": 0,
            "games_per_pairing": 2,
            "evaluator": "default",
            "corpus_run_id": null,
            "annotations_run_id": null,
            "annotations_mode": null,
        });
        let parsed: Provenance = serde_json::from_value(value).unwrap();
        assert_eq!(parsed.discovery, None);

        let with_discovery = Provenance {
            discovery: Some(DiscoveryProvenance {
                experiment: "e".to_string(),
                config_hash: "abc".to_string(),
                corpus_run_id: "r".to_string(),
                annotations_mode: "corpus".to_string(),
                miner: "cart-v1".to_string(),
                params: CandidateParams {
                    engine: "cart".to_string(),
                    max_depth: 8,
                    min_leaf: 1,
                },
                mine_seed: 0,
                holdout_fraction: 0.0,
                tiers: vec!["primitive".to_string()],
                dataset_rows: 10,
                candidate: "mined-d8-l1".to_string(),
                concepts: None,
            }),
            ..base
        };
        let json = serde_json::to_string(&with_discovery).unwrap();
        assert!(json.contains(r#""miner":"cart-v1""#));
        let round_tripped: Provenance = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, with_discovery);
    }

    #[test]
    fn dataset_and_mining_documents_round_trip() {
        let column = DatasetColumn {
            name: "center".to_string(),
            tier: Tier::Supplied,
            kind: "bool".to_string(),
            description: "center cell occupied".to_string(),
        };
        assert_eq!(serde_json::to_value(&column).unwrap()["tier"], "supplied");

        let manifest = DatasetManifest {
            schema_version: SCHEMA_VERSION,
            game: "ttt".to_string(),
            run_id: Some("r".to_string()),
            annotations_mode: "corpus".to_string(),
            tiers: vec!["primitive".to_string(), "supplied".to_string()],
            columns: vec![column],
            classes: vec!["center".to_string()],
            rows: 1,
            annotated: 1,
            nonterminal: 1,
            collapsed: 0,
            label_counts: BTreeMap::from([("none".to_string(), 1)]),
            value_counts: BTreeMap::from([("0".to_string(), 1)]),
            side_to_move_counts: BTreeMap::from([("0".to_string(), 1)]),
            unlabeled: 1,
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let round_tripped: DatasetManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, manifest);

        let row = DatasetRow::<Board> {
            schema_version: SCHEMA_VERSION,
            canonical_state: Board::empty(),
            side_to_move: 0,
            value: 0,
            occurrences: 1,
            legal: vec![0, 1],
            optimal: vec![4],
            qualifying: vec!["center".to_string()],
            label: "center".to_string(),
            values: vec![1.0, 0.0],
        };
        assert!(serde_json::to_value(&row).unwrap()["values"].is_array());
        let json = serde_json::to_string(&row).unwrap();
        let round_tripped: DatasetRow<Board> = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, row);

        let heuristic = HeuristicStrategy::new("mined-d8-l1");
        let evidence = RuleEvidence {
            rule: "rule0".to_string(),
            class: "center".to_string(),
            support: 1,
            sound: 1,
            soundness: 1.0,
            label_matches: 1,
            occurrences: 1,
            outcomes: OutcomeTally {
                win: 1,
                draw: 0,
                loss: 0,
            },
        };
        let candidate = MinedHeuristic {
            name: "mined-d8-l1".to_string(),
            engine: "cart".to_string(),
            max_depth: 8,
            min_leaf: 1,
            heuristic,
            rules: 1,
            leaves: 2,
            depth: 1,
            train_rows: 1,
            holdout_rows: 0,
            train_accuracy: 1.0,
            train_soundness: 1.0,
            holdout_accuracy: None,
            holdout_soundness: None,
            fallback_rows: 0,
            evidence: vec![evidence],
        };
        let report = MiningReport {
            schema_version: SCHEMA_VERSION,
            game: "ttt".to_string(),
            run_id: Some("r".to_string()),
            params: MineParams::default(),
            dataset: manifest,
            candidates: vec![candidate],
        };
        let json = serde_json::to_string(&report).unwrap();
        let round_tripped: MiningReport = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, report);

        let discover = DiscoverManifest {
            schema_version: SCHEMA_VERSION,
            name: "exp".to_string(),
            game: "ttt".to_string(),
            run_id: "r".to_string(),
            config_hash: "abc".to_string(),
            evaluation_id: "eval".to_string(),
            roster_id: "ttt-v1".to_string(),
            archive: "archive".to_string(),
            candidates: vec![DiscoverCandidate {
                name: "mined-d8-l1".to_string(),
                entry_id: "e0".to_string(),
                rules: 1,
                loss_rate_vs_reference: 0.0,
                agreement_rate: Some(1.0),
                novelty: 1.0,
            }],
        };
        let json = serde_json::to_string(&discover).unwrap();
        let round_tripped: DiscoverManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(round_tripped, discover);
    }
}
