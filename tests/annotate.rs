//! Annotation pipeline acceptance tests: exhaustive coverage of all 5478 tic-tac-toe positions,
//! corpus-mode annotation of every distinct corpus state, and exhaustive-solver reference facts.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use strategy_discovery::core::traits::GameRules;
use strategy_discovery::discovery::annotate::{AnnotateMode, AnnotateOptions, annotate_corpus, annotate_exhaustive};
use strategy_discovery::discovery::config::GenerateConfig;
use strategy_discovery::discovery::corpus::{GenerateOptions, generate};
use strategy_discovery::discovery::solver::{ExhaustiveSolver, SolverError, reachable_states};
use strategy_discovery::games::tictactoe::{Board, Move, Outcome, Player, TicTacToe, TicTacToeRules, game_bundle};
use strategy_discovery::io::schema::{ANNOTATE_FILE, ANNOTATIONS_FILE, AnnotationRecord, POSITIONS_FILE, PositionRecord, RUN_FILE};
use strategy_discovery::io::{read_json, read_jsonl};

/// Fresh, empty output directory for one test, under the integration-test temp dir.
fn out_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Value-label convention: `"X wins"` / `"draw"` / `"O wins"` from `(side_to_move, value)`.
fn label(side_to_move: Player, value: i8) -> &'static str {
    match (side_to_move, value) {
        (Player::X, 1) | (Player::O, -1) => "X wins",
        (Player::O, 1) | (Player::X, -1) => "O wins",
        _ => "draw",
    }
}

#[test]
fn exhaustive_annotation_covers_all_positions() {
    let bundle = game_bundle();
    let dir_a = out_dir("exhaustive-a");
    let metadata = annotate_exhaustive(&bundle, &dir_a, &AnnotateOptions::default()).unwrap();

    assert_eq!(metadata.annotated, 5478);
    assert_eq!(metadata.terminal, 958);
    assert_eq!(metadata.disagreements, 0);
    assert_eq!(metadata.engine_depth, 9);
    assert_eq!(metadata.mode, AnnotateMode::Exhaustive);
    assert!(metadata.run_id.is_none());
    assert_eq!(metadata.game, "tictactoe");

    let records: Vec<AnnotationRecord<Board, Move, Player>> = read_jsonl(&dir_a.join(ANNOTATIONS_FILE)).unwrap();
    assert_eq!(records.len(), 5478);

    let (mut x_wins, mut draws, mut o_wins) = (0, 0, 0);
    for record in &records {
        assert_eq!(record.schema_version, 1);
        match label(record.side_to_move, record.value) {
            "X wins" => x_wins += 1,
            "draw" => draws += 1,
            "O wins" => o_wins += 1,
            other => panic!("unexpected label {other}"),
        }
        if record.terminal {
            assert!(record.optimal_actions.is_empty());
            assert_eq!(record.engine_action, None);
            assert_eq!(record.engine_agrees, None);
        } else {
            assert!(!record.optimal_actions.is_empty());
            assert_eq!(record.engine_agrees, Some(true));
        }
    }
    assert_eq!((x_wins, draws, o_wins), (2936, 1068, 1474));

    let manifest: serde_json::Value = read_json(&dir_a.join(ANNOTATE_FILE)).unwrap();
    assert_eq!(manifest["mode"], "exhaustive");

    let dir_b = out_dir("exhaustive-b");
    annotate_exhaustive(&bundle, &dir_b, &AnnotateOptions::default()).unwrap();

    assert_eq!(
        std::fs::read(dir_a.join(ANNOTATIONS_FILE)).unwrap(),
        std::fs::read(dir_b.join(ANNOTATIONS_FILE)).unwrap(),
        "annotations.jsonl differs between two exhaustive runs"
    );
    assert_eq!(
        std::fs::read(dir_a.join(ANNOTATE_FILE)).unwrap(),
        std::fs::read(dir_b.join(ANNOTATE_FILE)).unwrap(),
        "annotate.json differs between two exhaustive runs"
    );
}

#[test]
fn corpus_annotation_labels_every_distinct_state() {
    let bundle = game_bundle();
    let dir = out_dir("corpus-annotate");

    let config_text = std::fs::read_to_string("tests/fixtures/generate-small.toml").unwrap();
    let config = GenerateConfig::<Move>::from_toml_str(&config_text).unwrap();
    generate(&bundle, config, &GenerateOptions::default(), &dir).unwrap();

    let metadata = annotate_corpus(&bundle, &dir, &AnnotateOptions::default()).unwrap();

    let positions: Vec<PositionRecord<Board, Move, Player, Outcome>> = read_jsonl(&dir.join(POSITIONS_FILE)).unwrap();
    let mut seen: HashSet<Board> = HashSet::new();
    let mut distinct_states: Vec<Board> = Vec::new();
    for position in &positions {
        if seen.insert(position.state) {
            distinct_states.push(position.state);
        }
    }

    assert_eq!(metadata.annotated, distinct_states.len());
    assert_eq!(metadata.disagreements, 0);
    assert_eq!(metadata.mode, AnnotateMode::Corpus);

    let run_header: serde_json::Value = read_json(&dir.join(RUN_FILE)).unwrap();
    assert_eq!(metadata.run_id.as_deref(), run_header["run_id"].as_str());

    let annotations: Vec<AnnotationRecord<Board, Move, Player>> = read_jsonl(&dir.join(ANNOTATIONS_FILE)).unwrap();
    let annotated_states: Vec<Board> = annotations.iter().map(|r| r.state).collect();
    assert_eq!(annotated_states, distinct_states);

    for record in &annotations {
        if !record.terminal {
            let position = positions
                .iter()
                .find(|p| p.state == record.state)
                .expect("every annotated state came from a position record");
            for action in &record.optimal_actions {
                assert!(
                    position.legal_actions.contains(action),
                    "optimal action {action:?} for state {:?} is not legal there",
                    record.state
                );
            }
        }
    }
}

#[test]
fn solver_facts() {
    let rules: Arc<dyn GameRules<TicTacToe>> = Arc::new(TicTacToeRules);

    let mut solver = ExhaustiveSolver::new(Arc::clone(&rules), ExhaustiveSolver::<TicTacToe>::DEFAULT_LIMIT);

    let empty = Board::empty();
    let solved = solver.solve(&empty).unwrap();
    assert_eq!(solved.value, 0);
    assert_eq!(solved.optimal, (0..9).map(Move).collect::<Vec<_>>());

    let solved = solver.solve(&Board::parse("XX.OO....").unwrap()).unwrap();
    assert_eq!(solved.value, 1);
    assert_eq!(solved.optimal, vec![Move(2)]);

    let solved = solver.solve(&Board::parse("XX.O.....").unwrap()).unwrap();
    assert_eq!(solved.value, -1);
    assert_eq!(solved.optimal, vec![Move(2), Move(4), Move(5), Move(6), Move(7), Move(8)]);

    let solved = solver.solve(&Board::parse("X...O...X").unwrap()).unwrap();
    assert_eq!(solved.value, 0);
    assert_eq!(solved.optimal, vec![Move(1), Move(3), Move(5), Move(7)]);

    assert_eq!(solver.solved_states(), 5478);

    let via_solver = reachable_states(rules.as_ref(), 1_000_000).unwrap();
    let via_rules = TicTacToeRules.reachable_positions();
    assert_eq!(via_solver, via_rules);

    assert!(matches!(
        reachable_states(rules.as_ref(), 10),
        Err(SolverError::TooLarge { limit: 10 })
    ));
}
