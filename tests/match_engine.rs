//! Phase 4 match-engine acceptance tests: perfect-vs-perfect draws across many seeds, decisive
//! non-perfect pairings, serial/parallel byte identity, error paths, roster build-and-play, and
//! `StrategySpec` round-trips.

use std::collections::HashSet;
use std::sync::Arc;
use strategy_discovery::core::dsl::HeuristicStrategy;
use strategy_discovery::core::kinds;
use strategy_discovery::core::traits::{
    GameRules, MatchConfig, MatchEngine, MatchError, MatchRecord, StrategyError, StrategyProvider,
};
use strategy_discovery::discovery::RayonMatchEngine;
use strategy_discovery::games::tictactoe::{
    Board, Move, Outcome, Player, TicTacToe, TicTacToeRules, benchmark_roster, engine_bundle,
};
use strategy_discovery::strategy::{MinimaxConfig, RandomProvider, ScriptedProvider, StrategyRegistry, StrategySpec, TieBreak};

/// A minimax provider built through the registry, matching how real callers construct one.
fn minimax(depth: u32, epsilon: f64, tie_break: TieBreak) -> Box<dyn StrategyProvider<TicTacToe>> {
    StrategyRegistry::new(engine_bundle())
        .build(&StrategySpec::Minimax(MinimaxConfig {
            depth,
            epsilon,
            tie_break,
        }))
        .unwrap()
}

/// A parallel `RayonMatchEngine` (global rayon pool) over the tic-tac-toe rules.
fn engine() -> RayonMatchEngine<TicTacToe> {
    RayonMatchEngine::new(engine_bundle().rules)
}

/// Number of records decided by a win (draws and `max_plies`-aborted `None` outcomes excluded).
fn decisive(records: &[MatchRecord<Move, Outcome>]) -> usize {
    records.iter().filter(|r| matches!(r.outcome, Some(Outcome::Win(_)))).count()
}

#[test]
fn perfect_vs_perfect_always_draws() {
    let provider = minimax(9, 0.0, TieBreak::SeededUniform);
    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, provider.as_ref()), (Player::O, provider.as_ref())];
    let config = MatchConfig {
        games: 32,
        seed: 7,
        max_plies: None,
    };
    let records = engine().run(&config, players).unwrap();

    assert_eq!(records.len(), 32);
    let mut distinct: HashSet<Vec<Move>> = HashSet::new();
    for (i, record) in records.iter().enumerate() {
        assert_eq!(record.game_index, i);
        assert_eq!(record.outcome, Some(Outcome::Draw));
        assert_eq!(record.actions.len(), 9);
        distinct.insert(record.actions.clone());
    }
    assert!(distinct.len() >= 2, "only {} distinct action sequences", distinct.len());
}

#[test]
fn non_perfect_settings_are_decisive() {
    let random = RandomProvider::<TicTacToe>::new();
    let perfect = minimax(9, 0.0, TieBreak::SeededUniform);

    // (a) random vs perfect, both colour assignments: decisive, and the perfect side never loses.
    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, &random), (Player::O, perfect.as_ref())];
    let config = MatchConfig {
        games: 100,
        seed: 42,
        max_plies: None,
    };
    let records = engine().run(&config, players).unwrap();
    assert!(decisive(&records) >= 1);
    assert!(!records.iter().any(|r| r.outcome == Some(Outcome::Win(Player::X))));

    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, perfect.as_ref()), (Player::O, &random)];
    let config = MatchConfig {
        games: 100,
        seed: 43,
        max_plies: None,
    };
    let records = engine().run(&config, players).unwrap();
    assert!(decisive(&records) >= 1);
    assert!(!records.iter().any(|r| r.outcome == Some(Outcome::Win(Player::O))));

    // (b) depth-1 vs depth-9, both colour assignments: decisive.
    let weak = minimax(1, 0.0, TieBreak::SeededUniform);
    let strong = minimax(9, 0.0, TieBreak::SeededUniform);

    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, weak.as_ref()), (Player::O, strong.as_ref())];
    let config = MatchConfig {
        games: 100,
        seed: 42,
        max_plies: None,
    };
    let records = engine().run(&config, players).unwrap();
    assert!(decisive(&records) >= 1);

    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, strong.as_ref()), (Player::O, weak.as_ref())];
    let config = MatchConfig {
        games: 100,
        seed: 42,
        max_plies: None,
    };
    let records = engine().run(&config, players).unwrap();
    assert!(decisive(&records) >= 1);

    // (c) depth-9 self-play with epsilon 0.25: decisive.
    let noisy = minimax(9, 0.25, TieBreak::SeededUniform);
    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, noisy.as_ref()), (Player::O, noisy.as_ref())];
    let config = MatchConfig {
        games: 200,
        seed: 1,
        max_plies: None,
    };
    let records = engine().run(&config, players).unwrap();
    assert!(decisive(&records) >= 1);
}

#[test]
fn parallel_equals_serial() {
    let provider = minimax(4, 0.1, TieBreak::SeededUniform);
    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, provider.as_ref()), (Player::O, provider.as_ref())];
    let config = MatchConfig {
        games: 64,
        seed: 2024,
        max_plies: None,
    };

    let rules = engine_bundle().rules;
    let serial = RayonMatchEngine::serial(Arc::clone(&rules)).run(&config, players).unwrap();
    let four = RayonMatchEngine::with_threads(Arc::clone(&rules), 4)
        .run(&config, players)
        .unwrap();
    let global = RayonMatchEngine::new(Arc::clone(&rules)).run(&config, players).unwrap();

    assert_eq!(serial, four);
    assert_eq!(serial, global);
    assert_eq!(serde_json::to_vec(&serial).unwrap(), serde_json::to_vec(&four).unwrap());
    assert_eq!(serde_json::to_vec(&serial).unwrap(), serde_json::to_vec(&global).unwrap());
    assert_eq!(serial.len(), 64);
    assert!(decisive(&serial) >= 1);
}

#[test]
fn max_plies_aborts_with_none_outcome() {
    let provider = minimax(9, 0.0, TieBreak::SeededUniform);
    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, provider.as_ref()), (Player::O, provider.as_ref())];
    let config = MatchConfig {
        games: 4,
        seed: 0,
        max_plies: Some(3),
    };
    let records = engine().run(&config, players).unwrap();

    assert_eq!(records.len(), 4);
    for record in &records {
        assert_eq!(record.actions.len(), 3);
        assert_eq!(record.outcome, None);
    }
}

#[test]
fn missing_provider_is_an_error() {
    let provider = minimax(9, 0.0, TieBreak::SeededUniform);
    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, provider.as_ref())];
    let config = MatchConfig {
        games: 1,
        seed: 0,
        max_plies: None,
    };
    let err = engine().run(&config, players).unwrap_err();
    assert_eq!(err, MatchError::MissingProvider("O".to_string()));
}

#[test]
fn illegal_scripted_action_is_an_error() {
    let x = ScriptedProvider::<TicTacToe>::new(vec![Move(4), Move(4)]);
    let o = ScriptedProvider::<TicTacToe>::new(vec![Move(0)]);
    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, &x), (Player::O, &o)];
    let config = MatchConfig {
        games: 1,
        seed: 0,
        max_plies: None,
    };
    let err = engine().run(&config, players).unwrap_err();
    match err {
        MatchError::Strategy(StrategyError::Other(msg)) => assert!(msg.contains("is not legal")),
        other => panic!("expected MatchError::Strategy(StrategyError::Other(_)), got {other:?}"),
    }
}

#[test]
fn scripted_game_replays_exactly() {
    let x = ScriptedProvider::<TicTacToe>::new(vec![Move(4), Move(2), Move(3), Move(1), Move(8)]);
    let o = ScriptedProvider::<TicTacToe>::new(vec![Move(0), Move(6), Move(5), Move(7)]);
    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, &x), (Player::O, &o)];
    let config = MatchConfig {
        games: 1,
        seed: 0,
        max_plies: None,
    };

    let records1 = engine().run(&config, players).unwrap();
    let records2 = engine().run(&config, players).unwrap();
    assert_eq!(records1, records2);

    let expected_actions: Vec<Move> = [4, 0, 2, 6, 3, 5, 1, 7, 8].into_iter().map(Move).collect();
    assert_eq!(records1[0].actions, expected_actions);
    assert_eq!(records1[0].outcome, Some(Outcome::Draw));
}

#[test]
fn roster_entries_all_build_and_play() {
    let roster = benchmark_roster();
    assert_eq!(roster.id(), "ttt-benchmark-v1");
    assert_eq!(roster.entries.len(), 7);

    let registry = StrategyRegistry::new(engine_bundle());
    let random_o = RandomProvider::<TicTacToe>::new();
    for entry in &roster.entries {
        let provider = registry.build(&entry.spec).unwrap();
        let expected_kind = if entry.name == "random" {
            kinds::RANDOM
        } else {
            kinds::MINIMAX
        };
        assert_eq!(provider.kind(), expected_kind, "entry {}", entry.name);

        let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, provider.as_ref()), (Player::O, &random_o)];
        let config = MatchConfig {
            games: 1,
            seed: 5,
            max_plies: None,
        };
        let records = engine().run(&config, players).unwrap();
        assert_eq!(records.len(), 1);
        assert!(records[0].outcome.is_some(), "entry {}", entry.name);
    }

    let heuristic_spec = StrategySpec::HeuristicRules {
        heuristic: HeuristicStrategy::new("h"),
    };
    let heuristic_provider = registry.build(&heuristic_spec).unwrap();
    assert_eq!(heuristic_provider.kind(), kinds::HEURISTIC_RULES);
    let legal = TicTacToeRules.legal_actions(&Board::empty());
    match heuristic_provider.create(0).choose(&Board::empty(), &legal) {
        Err(StrategyError::Unimplemented { kind, .. }) => assert_eq!(kind, kinds::HEURISTIC_RULES),
        other => panic!("expected Unimplemented, got {other:?}"),
    }

    match registry.build(&StrategySpec::Evolutionary) {
        Err(StrategyError::Unimplemented { kind, .. }) => assert_eq!(kind, kinds::EVOLUTIONARY),
        Err(other) => panic!("expected Unimplemented, got Err({other:?})"),
        Ok(_) => panic!("expected Unimplemented, got Ok"),
    }
    match registry.build(&StrategySpec::Llm) {
        Err(StrategyError::Unimplemented { kind, .. }) => assert_eq!(kind, kinds::LLM),
        Err(other) => panic!("expected Unimplemented, got Err({other:?})"),
        Ok(_) => panic!("expected Unimplemented, got Ok"),
    }
}

#[test]
fn spec_round_trips_json_and_toml() {
    let round_trip_specs = vec![
        StrategySpec::Minimax(MinimaxConfig {
            depth: 9,
            epsilon: 0.0,
            tie_break: TieBreak::SeededUniform,
        }),
        StrategySpec::Minimax(MinimaxConfig {
            depth: 2,
            epsilon: 0.5,
            tie_break: TieBreak::Engine,
        }),
        StrategySpec::Random,
        StrategySpec::Evolutionary,
        StrategySpec::Llm,
    ];
    for spec in &round_trip_specs {
        let json = serde_json::to_string(spec).unwrap();
        assert_eq!(&serde_json::from_str::<StrategySpec>(&json).unwrap(), spec);

        let as_toml = toml::to_string(spec).unwrap();
        assert_eq!(&toml::from_str::<StrategySpec>(&as_toml).unwrap(), spec);
    }

    let heuristic_spec = StrategySpec::HeuristicRules {
        heuristic: HeuristicStrategy::new("h"),
    };
    let heuristic_json = serde_json::to_string(&heuristic_spec).unwrap();
    assert_eq!(serde_json::from_str::<StrategySpec>(&heuristic_json).unwrap(), heuristic_spec);

    assert_eq!(
        serde_json::to_string(&round_trip_specs[0]).unwrap(),
        r#"{"kind":"minimax","depth":9,"epsilon":0.0,"tie_break":"seeded-uniform"}"#
    );

    for spec in round_trip_specs.iter().chain(std::iter::once(&heuristic_spec)) {
        let value = serde_json::to_value(spec).unwrap();
        assert_eq!(value["kind"].as_str(), Some(spec.kind()));
    }

    assert_eq!(round_trip_specs[0].kind(), kinds::MINIMAX);
    assert_eq!(StrategySpec::Random.kind(), kinds::RANDOM);
    assert_eq!(heuristic_spec.kind(), kinds::HEURISTIC_RULES);
    assert_eq!(StrategySpec::Evolutionary.kind(), kinds::EVOLUTIONARY);
    assert_eq!(StrategySpec::Llm.kind(), kinds::LLM);
}
