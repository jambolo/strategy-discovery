//! Integration suite for corpus generation (`strategy_discovery::discovery::corpus`) and
//! summary/verification (`strategy_discovery::discovery::summary`): proves byte-identical
//! seeded generation, serial/parallel equivalence, that persisted positions replay exactly to
//! the actions that produced them, correct opening handling, the documented cell/game seed
//! formulas, diversity thresholds passing on real sweeps and failing on a degenerate one, and
//! rejection of a corrupted schema version and of invalid sweep configurations.

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use strategy_discovery::core::symmetry::Permutation;
use strategy_discovery::discovery::config::{CorpusError, GenerateConfig, cell_seed, resolve};
use strategy_discovery::discovery::corpus::{GenerateOptions, RunMetadata, generate};
use strategy_discovery::discovery::match_engine::game_seed;
use strategy_discovery::discovery::summary::{DiversityThresholds, analyze_corpus, verify_corpus};
use strategy_discovery::games::tictactoe::{Board, Move, Outcome, Player, game_bundle};
use strategy_discovery::io::replay::replay;
use strategy_discovery::io::schema::{GAMES_FILE, GameRecord, POSITIONS_FILE, PositionRecord, RUN_FILE};
use strategy_discovery::io::{IoError, read_jsonl};

/// Fresh, empty output directory for one test, under the integration-test temp dir.
fn out_dir(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Reads a checked-in fixture into a `String`, normalized to LF line endings so string mutations
/// below (which insert or match `\n`-terminated lines) behave the same regardless of
/// `core.autocrlf`. The working directory of an integration test is the package root.
fn fixture(name: &str) -> String {
    std::fs::read_to_string(Path::new("tests/fixtures").join(name))
        .unwrap()
        .replace("\r\n", "\n")
}

/// Generates one corpus from `text` into a fresh directory named `name`, using `options`.
fn generate_into(name: &str, text: &str, options: &GenerateOptions) -> (RunMetadata<Move, Player>, PathBuf) {
    let dir = out_dir(name);
    let bundle = game_bundle();
    let config = GenerateConfig::<Move>::from_toml_str(text).unwrap();
    let metadata = generate(&bundle, config, options, &dir).unwrap();
    (metadata, dir)
}

#[test]
fn same_seed_is_byte_identical() {
    let text = fixture("generate-small.toml");
    let (meta_a, dir_a) = generate_into("same-seed-a", &text, &GenerateOptions::default());
    let (meta_b, dir_b) = generate_into("same-seed-b", &text, &GenerateOptions::default());

    assert_eq!(meta_a.run_id, meta_b.run_id);
    for file in [RUN_FILE, GAMES_FILE, POSITIONS_FILE] {
        assert_eq!(
            std::fs::read(dir_a.join(file)).unwrap(),
            std::fs::read(dir_b.join(file)).unwrap(),
            "{file} differs"
        );
    }
}

#[test]
fn serial_equals_parallel() {
    let text = fixture("generate-small.toml");
    let (_, dir_serial) = generate_into(
        "serial-eq-parallel-serial",
        &text,
        &GenerateOptions {
            serial: true,
            threads: None,
        },
    );
    let (_, dir_threads) = generate_into(
        "serial-eq-parallel-threads",
        &text,
        &GenerateOptions {
            serial: false,
            threads: Some(4),
        },
    );
    let (_, dir_default) = generate_into("serial-eq-parallel-default", &text, &GenerateOptions::default());

    for file in [RUN_FILE, GAMES_FILE, POSITIONS_FILE] {
        let serial_bytes = std::fs::read(dir_serial.join(file)).unwrap();
        assert_eq!(
            serial_bytes,
            std::fs::read(dir_threads.join(file)).unwrap(),
            "{file} (threads) differs"
        );
        assert_eq!(
            serial_bytes,
            std::fs::read(dir_default.join(file)).unwrap(),
            "{file} (default) differs"
        );
    }
}

#[test]
fn defaults_spelled_out_produce_same_run_id() {
    let small = fixture("generate-small.toml");
    let explicit = fixture("generate-small-explicit.toml");
    let (meta_small, dir_small) = generate_into("defaults-small", &small, &GenerateOptions::default());
    let (meta_explicit, dir_explicit) = generate_into("defaults-explicit", &explicit, &GenerateOptions::default());

    assert_eq!(meta_small.run_id, meta_explicit.run_id);
    for file in [RUN_FILE, GAMES_FILE, POSITIONS_FILE] {
        assert_eq!(
            std::fs::read(dir_small.join(file)).unwrap(),
            std::fs::read(dir_explicit.join(file)).unwrap(),
            "{file} differs"
        );
    }
}

#[test]
fn positions_equal_replay_of_games() {
    let text = fixture("generate-small.toml");
    let bundle = game_bundle();
    let (_, dir) = generate_into("positions-equal-replay", &text, &GenerateOptions::default());

    assert_eq!(verify_corpus(&bundle, &dir).unwrap(), 144);

    let games: Vec<GameRecord<Board, Move, Player, Outcome>> = read_jsonl(&dir.join(GAMES_FILE)).unwrap();
    let positions: Vec<PositionRecord<Board, Move, Player, Outcome>> = read_jsonl(&dir.join(POSITIONS_FILE)).unwrap();

    let mut by_game: BTreeMap<&str, Vec<&PositionRecord<Board, Move, Player, Outcome>>> = BTreeMap::new();
    for position in &positions {
        by_game.entry(position.game_id.as_str()).or_default().push(position);
    }

    let primitives = bundle.primitives.as_ref().unwrap();
    let canonicalizer = bundle.canonicalizer.as_ref().unwrap();

    for game in &games {
        let group = by_game.get(game.game_id.as_str()).unwrap();
        assert_eq!(group.len(), game.actions.len(), "game {}: position count", game.game_id);

        let pairs = replay(bundle.rules.as_ref(), &game.actions).unwrap();

        for (ply, position) in group.iter().enumerate() {
            let (state_before, _) = &pairs[ply];
            assert_eq!(position.state, *state_before, "game {}: state at ply {ply}", game.game_id);
            assert_eq!(
                position.legal_actions,
                bundle.rules.legal_actions(&position.state),
                "game {}: legal_actions at ply {ply}",
                game.game_id
            );
            assert_eq!(
                position.canonical_state,
                canonicalizer.canonicalize(&position.state),
                "game {}: canonical_state at ply {ply}",
                game.game_id
            );
            let perm = Permutation::new(position.canonical_transform.clone()).unwrap();
            assert_eq!(
                primitives.transform(&position.state, &perm),
                position.canonical_state,
                "game {}: canonical_transform at ply {ply}",
                game.game_id
            );
        }
    }
}

#[test]
fn openings_are_honoured() {
    let text = fixture("generate-small.toml");
    let (_, dir) = generate_into("openings-are-honoured", &text, &GenerateOptions::default());

    let games: Vec<GameRecord<Board, Move, Player, Outcome>> = read_jsonl(&dir.join(GAMES_FILE)).unwrap();
    let positions: Vec<PositionRecord<Board, Move, Player, Outcome>> = read_jsonl(&dir.join(POSITIONS_FILE)).unwrap();

    let mut by_game: BTreeMap<&str, Vec<&PositionRecord<Board, Move, Player, Outcome>>> = BTreeMap::new();
    for position in &positions {
        by_game.entry(position.game_id.as_str()).or_default().push(position);
    }

    for game in &games {
        let group = by_game.get(game.game_id.as_str()).unwrap();
        let fixed_len = if game.cell.opening == "center" { 1usize } else { 0usize };

        if game.cell.opening == "center" {
            assert_eq!(game.actions[0], Move(4), "game {}", game.game_id);
            assert_eq!(group[0].chosen_by, "opening", "game {}", game.game_id);
        }

        if game.cell.opening == "none" && game.cell.random_opening_plies == 2 && group.len() >= 2 {
            assert_eq!(group[0].chosen_by, "random-opening", "game {}: ply 0", game.game_id);
            assert_eq!(group[1].chosen_by, "random-opening", "game {}: ply 1", game.game_id);
        }

        let expected_opening_plies = game.length.min(fixed_len + game.cell.random_opening_plies as usize);
        assert_eq!(game.opening_plies, expected_opening_plies, "game {}", game.game_id);
    }

    let target_cell_index = games
        .iter()
        .find(|g| g.cell.opening == "none" && g.cell.random_opening_plies == 2)
        .map(|g| g.cell.index)
        .expect("fixture has a none/2 cell");
    let first_actions: HashSet<Move> = games
        .iter()
        .filter(|g| g.cell.index == target_cell_index)
        .map(|g| g.actions[0])
        .collect();
    assert!(first_actions.len() >= 2, "first_actions={first_actions:?}");
}

#[test]
fn seeds_follow_the_documented_formulas() {
    let text = fixture("generate-small.toml");
    let bundle = game_bundle();
    let resolved = resolve(GenerateConfig::<Move>::from_toml_str(&text).unwrap(), &bundle).unwrap();

    let (_, dir) = generate_into("seeds-follow-formulas", &text, &GenerateOptions::default());
    let games: Vec<GameRecord<Board, Move, Player, Outcome>> = read_jsonl(&dir.join(GAMES_FILE)).unwrap();

    for game in &games {
        let expected = game_seed(cell_seed(20260821, game.cell.index), game.game_index);
        assert_eq!(game.seed, expected, "game {}", game.game_id);
    }

    let distinct_cell_seeds: BTreeSet<u64> = resolved.cells.iter().map(|c| c.seed).collect();
    assert_eq!(distinct_cell_seeds.len(), 36);
}

#[test]
fn diversity_thresholds_pass_across_seeds() {
    let text = fixture("generate-small.toml");
    let bundle = game_bundle();
    let thresholds = DiversityThresholds {
        min_canonical_coverage: 0.25,
        min_decisive_fraction: 0.2,
        min_distinct_game_fraction: 0.5,
    };

    for seed in [1u64, 2, 3] {
        let seeded_text = text.replace("seed = 20260821", &format!("seed = {seed}"));
        let (_, dir) = generate_into(
            &format!("diversity-pass-seed-{seed}"),
            &seeded_text,
            &GenerateOptions::default(),
        );
        let summary = analyze_corpus(&bundle, &dir, &thresholds).unwrap();

        println!(
            "[diversity] seed={seed} canonical_coverage={:?} decisive_fraction={:.4} distinct_game_fraction={:.4}",
            summary.canonical_coverage, summary.decisive_fraction, summary.distinct_game_fraction
        );

        assert!(
            summary.diversity_pass,
            "seed {seed}: failures={:?}",
            summary.diversity_failures
        );
        assert!(summary.diversity_failures.is_empty(), "seed {seed}");
    }
}

#[test]
fn diversity_catches_identical_perfect_play() {
    let text = fixture("generate-draws.toml");
    let bundle = game_bundle();
    let (_, dir) = generate_into("diversity-catches-draws", &text, &GenerateOptions::default());

    let summary = analyze_corpus(&bundle, &dir, &DiversityThresholds::default()).unwrap();

    assert_eq!(summary.games, 16);
    assert_eq!(summary.distinct_games, 1);
    assert_eq!(summary.decisive_games, 0);
    assert!(!summary.diversity_pass);
    assert!(summary.diversity_failures.len() >= 2, "{:?}", summary.diversity_failures);
}

#[test]
fn schema_version_mismatch_is_an_error() {
    let text = fixture("generate-draws.toml");
    let bundle = game_bundle();
    let (_, dir) = generate_into("schema-version-mismatch", &text, &GenerateOptions::default());

    let positions_path = dir.join(POSITIONS_FILE);
    let contents = std::fs::read_to_string(&positions_path).unwrap();
    assert!(contents.contains("\"schema_version\":1"), "unexpected positions.jsonl shape");
    let patched = contents.replacen("\"schema_version\":1", "\"schema_version\":99", 1);
    std::fs::write(&positions_path, patched).unwrap();

    match analyze_corpus(&bundle, &dir, &DiversityThresholds::default()) {
        Err(CorpusError::Io(IoError::SchemaVersion { expected, found, .. })) => {
            assert_eq!(expected, 1);
            assert_eq!(found, 99);
        }
        other => panic!("expected Err(Io(SchemaVersion {{ .. }})), got {other:?}"),
    }
}

#[test]
fn invalid_configs_are_rejected() {
    let text = fixture("generate-small.toml");
    let bundle = game_bundle();

    let config_cases: Vec<(String, &str)> = vec![
        (
            text.replace(
                "games_per_cell = 4\n",
                "games_per_cell = 4\npairings = [[\"random\", \"nope\"]]\n",
            ),
            "nope",
        ),
        (
            text.replace("evaluators = [\"default\"]", "evaluators = [\"nope\"]"),
            "unknown evaluator",
        ),
        (text.replace("games_per_cell = 4", "games_per_cell = 0"), "games_per_cell"),
        (text.replace("actions = [4]", "actions = [4, 4]"), "center"),
        (text.replace("schema_version = 1", "schema_version = 2"), "schema_version"),
    ];

    for (mutated, expected) in config_cases {
        let config = GenerateConfig::<Move>::from_toml_str(&mutated).unwrap_or_else(|e| panic!("case {expected:?}: {e}"));
        match resolve(config, &bundle) {
            Err(CorpusError::Config(msg)) => assert!(msg.contains(expected), "case {expected:?}: message was {msg:?}"),
            other => panic!("case {expected:?}: expected Err(Config(_)), got {other:?}"),
        }
    }

    let bogus = text.replace("games_per_cell = 4", "games_per_cell = 4\nbogus_key = 1");
    match GenerateConfig::<Move>::from_toml_str(&bogus) {
        Err(CorpusError::Io(IoError::Toml { message, .. })) => assert!(message.contains("bogus_key")),
        other => panic!("expected Err(Io(Toml {{ .. }})), got {other:?}"),
    }
}
