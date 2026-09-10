//! Annotation pass: labels corpus or exhaustively enumerated positions with exhaustive-solver
//! ground truth cross-checked against the `game-player` engine. This is a SEPARATE pass that
//! reads an already-generated corpus (or enumerates a small game exhaustively) — it never
//! regenerates games. Exhaustive modes are explicitly a small-game accelerant: they walk and
//! memoize the entire reachable state space, so they only suit games small enough to fit in
//! memory; larger games must annotate a sampled corpus instead.
//!
//! Byte-identity: repeated runs over the same input produce byte-identical `annotations.jsonl`
//! and `annotate.json` (no timestamps, durations, thread counts, hostnames or absolute paths).
//! Records are emitted in a defined order — first-appearance order for corpus mode, BFS order
//! for exhaustive mode — never by iterating a `HashMap`/`HashSet`.

use crate::discovery::bundle::GameBundle;
use crate::discovery::config::CorpusError;
use crate::discovery::solver::{ExhaustiveSolver, reachable_states};
use crate::io::jsonl::{JsonlReader, write_json_pretty, write_jsonl};
use crate::io::schema::{ANNOTATE_FILE, ANNOTATIONS_FILE, AnnotationRecord, CorpusGame, POSITIONS_FILE, RUN_FILE, SCHEMA_VERSION};
use crate::io::{IoError, read_json};
use crate::strategy::engine::{AliceEvaluator, EngineGame, RulesResponseGenerator};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::Path;

/// Which set of positions an annotation pass covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AnnotateMode {
    /// Every distinct position of an already-generated corpus.
    Corpus,
    /// Every reachable position of the game (small games only).
    Exhaustive,
}

/// Mirror of `ExhaustiveSolver::DEFAULT_LIMIT`, usable without naming a game type.
pub const DEFAULT_SOLVER_LIMIT: usize = 1_000_000;

/// Knobs for an annotation pass.
#[derive(Debug, Clone)]
pub struct AnnotateOptions {
    /// Engine search depth for the cross-check; `None` means the bundle's `full_search_depth`.
    pub engine_depth: Option<u32>,
    /// Maximum states the exhaustive solver may memoize.
    pub solver_limit: usize,
}

impl Default for AnnotateOptions {
    fn default() -> Self {
        AnnotateOptions {
            engine_depth: None,
            solver_limit: DEFAULT_SOLVER_LIMIT,
        }
    }
}

/// Manifest of one annotation pass, written to `annotate.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AnnotateMetadata {
    /// Schema version this manifest was written under.
    pub schema_version: u32,
    /// Which set of positions this pass covered.
    pub mode: AnnotateMode,
    /// Game name this pass annotated.
    pub game: String,
    /// `run_id` of the corpus this pass read; `None` in exhaustive mode.
    pub run_id: Option<String>,
    /// Engine search depth used for the cross-check.
    pub engine_depth: u32,
    /// Evaluator name used to build the engine cross-check.
    pub evaluator: String,
    /// Total positions annotated.
    pub annotated: usize,
    /// Of `annotated`, how many were terminal states.
    pub terminal: usize,
    /// Of `annotated`, how many had `engine_agrees == Some(false)`.
    pub disagreements: usize,
    /// Number of states the exhaustive solver memoized.
    pub solver_states: usize,
}

/// The fields of `run.json` this pass needs; other fields are ignored.
#[derive(Deserialize)]
struct RunHeader {
    schema_version: u32,
    game: String,
    run_id: String,
}

/// Annotates `states`, in order, against `bundle`'s default evaluator and exhaustive `solver`,
/// cross-checked with the `game-player` engine at `engine_depth`.
#[allow(clippy::type_complexity)]
pub fn annotate_states<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    states: &[G::State],
    engine_depth: u32,
    solver: &mut ExhaustiveSolver<G>,
) -> Result<Vec<AnnotationRecord<G::State, G::Action, G::Player>>, CorpusError> {
    let rules = bundle.rules.as_ref();
    let evaluator = bundle
        .evaluators
        .get(&bundle.default_evaluator)
        .ok_or_else(|| CorpusError::Config(format!("unknown evaluator `{}`", bundle.default_evaluator)))?;
    let sef = AliceEvaluator::new(rules, evaluator.as_ref());
    let rg = RulesResponseGenerator::new(rules);

    let mut records = Vec::with_capacity(states.len());
    for state in states {
        let solved = solver.solve(state)?;
        let terminal = rules.is_terminal(state);
        let engine_action = game_player::minimax::search(&sef, &rg, state, engine_depth);
        let engine_agrees = engine_action.as_ref().map(|a| solved.optimal.contains(a));
        let (canonical_state, _) = bundle.canonicalize(state);

        records.push(AnnotationRecord {
            schema_version: SCHEMA_VERSION,
            state: state.clone(),
            canonical_state,
            side_to_move: rules.player_to_move(state),
            terminal,
            value: solved.value,
            optimal_actions: solved.optimal,
            engine_action,
            engine_agrees,
        });
    }
    Ok(records)
}

/// Counts records with `terminal == true` and records with `engine_agrees == Some(false)`.
fn counts<S, A, P>(records: &[AnnotationRecord<S, A, P>]) -> (usize, usize) {
    let terminal = records.iter().filter(|r| r.terminal).count();
    let disagreements = records.iter().filter(|r| r.engine_agrees == Some(false)).count();
    (terminal, disagreements)
}

/// Annotates every distinct position of an already-generated corpus at `corpus_dir`, writing
/// `annotations.jsonl` and `annotate.json` alongside it. Never regenerates the corpus.
pub fn annotate_corpus<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    corpus_dir: &Path,
    options: &AnnotateOptions,
) -> Result<AnnotateMetadata, CorpusError> {
    let started = std::time::Instant::now();
    let run_path = corpus_dir.join(RUN_FILE);
    if !run_path.exists() {
        return Err(CorpusError::Io(IoError::Missing { path: run_path }));
    }
    let header: RunHeader = read_json(&run_path)?;
    crate::io::schema::check_schema_version(&run_path, header.schema_version)?;
    if header.game != bundle.name {
        return Err(CorpusError::Config(format!(
            "corpus game `{}` does not match bundle `{}`",
            header.game, bundle.name
        )));
    }

    let positions_path = corpus_dir.join(POSITIONS_FILE);
    if !positions_path.exists() {
        return Err(CorpusError::Io(IoError::Missing { path: positions_path }));
    }

    let mut seen: HashSet<G::State> = HashSet::new();
    let mut states: Vec<G::State> = Vec::new();
    for record in
        JsonlReader::<crate::io::schema::PositionRecord<G::State, G::Action, G::Player, G::Outcome>>::open(&positions_path)?
    {
        let record = record?;
        crate::io::schema::check_schema_version(&positions_path, record.schema_version)?;
        if seen.insert(record.state.clone()) {
            states.push(record.state);
        }
    }

    let engine_depth = options.engine_depth.unwrap_or(bundle.full_search_depth);
    let mut solver = ExhaustiveSolver::new(bundle.rules.clone(), options.solver_limit);
    let records = annotate_states(bundle, &states, engine_depth, &mut solver)?;
    write_jsonl(&corpus_dir.join(ANNOTATIONS_FILE), &records)?;

    let (terminal, disagreements) = counts(&records);
    let metadata = AnnotateMetadata {
        schema_version: SCHEMA_VERSION,
        mode: AnnotateMode::Corpus,
        game: bundle.name.clone(),
        run_id: Some(header.run_id),
        engine_depth,
        evaluator: bundle.default_evaluator.clone(),
        annotated: records.len(),
        terminal,
        disagreements,
        solver_states: solver.solved_states(),
    };
    write_json_pretty(&corpus_dir.join(ANNOTATE_FILE), &metadata)?;
    tracing::info!(
        mode = "corpus",
        annotated = metadata.annotated,
        terminal = metadata.terminal,
        disagreements = metadata.disagreements,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "annotated"
    );
    Ok(metadata)
}

/// Annotates every reachable position of `bundle`'s game (a small-game accelerant: the entire
/// state space is enumerated and memoized), writing `annotations.jsonl` and `annotate.json`
/// into `out_dir`.
pub fn annotate_exhaustive<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    out_dir: &Path,
    options: &AnnotateOptions,
) -> Result<AnnotateMetadata, CorpusError> {
    let started = std::time::Instant::now();
    std::fs::create_dir_all(out_dir).map_err(|source| {
        CorpusError::Io(IoError::Io {
            path: out_dir.to_path_buf(),
            source,
        })
    })?;

    let states = reachable_states(bundle.rules.as_ref(), options.solver_limit)?;

    let engine_depth = options.engine_depth.unwrap_or(bundle.full_search_depth);
    let mut solver = ExhaustiveSolver::new(bundle.rules.clone(), options.solver_limit);
    let records = annotate_states(bundle, &states, engine_depth, &mut solver)?;
    write_jsonl(&out_dir.join(ANNOTATIONS_FILE), &records)?;

    let (terminal, disagreements) = counts(&records);
    let metadata = AnnotateMetadata {
        schema_version: SCHEMA_VERSION,
        mode: AnnotateMode::Exhaustive,
        game: bundle.name.clone(),
        run_id: None,
        engine_depth,
        evaluator: bundle.default_evaluator.clone(),
        annotated: records.len(),
        terminal,
        disagreements,
        solver_states: solver.solved_states(),
    };
    write_json_pretty(&out_dir.join(ANNOTATE_FILE), &metadata)?;
    tracing::info!(
        mode = "exhaustive",
        annotated = metadata.annotated,
        terminal = metadata.terminal,
        disagreements = metadata.disagreements,
        elapsed_ms = started.elapsed().as_millis() as u64,
        "annotated"
    );
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::tictactoe::{Board, Move, TicTacToe, game_bundle};

    #[test]
    fn annotate_mode_serializes_kebab_case() {
        assert_eq!(serde_json::to_string(&AnnotateMode::Exhaustive).unwrap(), "\"exhaustive\"");
        let round_tripped: AnnotateMode = serde_json::from_str("\"corpus\"").unwrap();
        assert_eq!(round_tripped, AnnotateMode::Corpus);
    }

    #[test]
    fn default_solver_limit_matches_the_solver() {
        assert_eq!(DEFAULT_SOLVER_LIMIT, ExhaustiveSolver::<TicTacToe>::DEFAULT_LIMIT);
    }

    #[test]
    fn annotate_states_labels_known_boards() {
        let bundle = game_bundle();
        let mut solver = ExhaustiveSolver::new(bundle.rules.clone(), DEFAULT_SOLVER_LIMIT);
        let states = vec![
            Board::empty(),
            Board::parse("XX.OO....").unwrap(),
            Board::parse("XXXOO....").unwrap(),
        ];

        let records = annotate_states(&bundle, &states, 9, &mut solver).unwrap();
        assert_eq!(records.len(), 3);

        assert_eq!(records[0].value, 0);
        assert_eq!(records[0].optimal_actions.len(), 9);
        assert!(!records[0].terminal);
        assert_eq!(records[0].engine_agrees, Some(true));

        assert_eq!(records[1].value, 1);
        assert_eq!(records[1].optimal_actions, vec![Move(2)]);
        assert_eq!(records[1].engine_agrees, Some(true));

        assert!(records[2].terminal);
        assert!(records[2].optimal_actions.is_empty());
        assert_eq!(records[2].engine_action, None);
        assert_eq!(records[2].engine_agrees, None);

        for record in &records {
            assert_eq!(record.schema_version, SCHEMA_VERSION);
        }
    }

    #[test]
    fn annotate_exhaustive_writes_both_files() {
        let bundle = game_bundle();
        let out_dir = std::env::temp_dir().join(format!("sd-annotate-exhaustive-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&out_dir);

        let metadata = annotate_exhaustive(&bundle, &out_dir, &AnnotateOptions::default()).unwrap();

        assert_eq!(metadata.annotated, 5478);
        assert_eq!(metadata.terminal, 958);
        assert_eq!(metadata.disagreements, 0);
        assert_eq!(metadata.engine_depth, 9);
        assert_eq!(metadata.mode, AnnotateMode::Exhaustive);
        assert!(metadata.run_id.is_none());

        assert!(out_dir.join(ANNOTATIONS_FILE).exists());
        assert!(out_dir.join(ANNOTATE_FILE).exists());
    }
}
