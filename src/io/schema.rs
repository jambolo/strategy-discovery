//! Versioned record schema for corpus persistence: file-name constants, the [`CorpusGame`]
//! bound that makes a game's associated types serializable, and the three record shapes
//! (`GameRecord`, `PositionRecord`, `AnnotationRecord`) written by the pipeline stages.

use crate::core::traits::GameDomain;
use crate::io::IoError;
use crate::strategy::registry::StrategySpec;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
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
        assert_eq!(SCHEMA_VERSION, 1);
    }
}
