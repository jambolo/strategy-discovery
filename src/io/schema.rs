//! Versioned record schema for corpus persistence: file-name constants, the [`CorpusGame`]
//! bound that makes a game's associated types serializable, and the three record shapes
//! (`GameRecord`, `PositionRecord`, `AnnotationRecord`) written by the pipeline stages.

use crate::core::traits::GameDomain;
use crate::discovery::agreement::AgreementCounts;
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
    /// Producing stage (`evaluate`; `discover` later).
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
}
