//! Integration tests: a hand-written `heuristic-rules` strategy takes wins, blocks threats, is
//! seed-deterministic, respects the canonical frame, plays through the match engine, and rejects
//! unavailable native features at construction time.

use std::sync::Arc;
use strategy_discovery::core::dsl::{ActionSelector, HeuristicStrategy, Rule};
use strategy_discovery::core::features::{CmpOp, FeatureDef, FeatureExpr, Tier};
use strategy_discovery::core::featurizer::Featurizer;
use strategy_discovery::core::interpreter::{RuleInterpreter, RuleInterpreterProvider};
use strategy_discovery::core::traits::{GameRules, MatchConfig, MatchEngine, Strategy, StrategyProvider};
use strategy_discovery::discovery::RayonMatchEngine;
use strategy_discovery::games::tictactoe::{Board, TicTacToe, TicTacToeRules, engine_bundle, game_bundle};
use strategy_discovery::strategy::{MinimaxConfig, RandomProvider, StrategyRegistry, StrategySpec, TieBreak};

/// The tic-tac-toe featurizer, natives + supplied vocabulary included.
fn featurizer() -> Arc<Featurizer<TicTacToe>> {
    Arc::new(game_bundle().featurizer(true).unwrap())
}

/// Win-then-block heuristic: take an immediate winning cell, else block an opponent threat.
fn win_block() -> HeuristicStrategy {
    let mut h = HeuristicStrategy::new("win-block");
    h.define(FeatureDef::native(
        "ttt.winning_cells",
        Tier::Supplied,
        "cells completing a line for the mover",
    ))
    .unwrap();
    h.define(FeatureDef::native(
        "ttt.blocking_cells",
        Tier::Supplied,
        "cells blocking an opponent win",
    ))
    .unwrap();
    h.define(FeatureDef::derived(
        "has_win",
        Tier::Invented,
        "a winning move exists",
        FeatureExpr::compare(
            CmpOp::Gt,
            FeatureExpr::count(FeatureExpr::named("ttt.winning_cells")),
            FeatureExpr::int(0),
        ),
    ))
    .unwrap();
    h.push_rule(Rule::new(
        "win",
        2,
        FeatureExpr::named("has_win"),
        ActionSelector::TargetIn {
            expr: FeatureExpr::named("ttt.winning_cells"),
        },
    ));
    h.push_rule(Rule::new(
        "block",
        1,
        FeatureExpr::compare(
            CmpOp::Gt,
            FeatureExpr::count(FeatureExpr::named("ttt.blocking_cells")),
            FeatureExpr::int(0),
        ),
        ActionSelector::TargetIn {
            expr: FeatureExpr::named("ttt.blocking_cells"),
        },
    ));
    h.validate().unwrap();
    h
}

/// A heuristic that always maximizes the count of free cells: every successor ties, so the
/// seeded tie-break decides among all legal actions.
fn expand() -> HeuristicStrategy {
    let mut h = HeuristicStrategy::new("expand");
    h.define(FeatureDef::native("free", Tier::Primitive, "empty cells")).unwrap();
    h.push_rule(Rule::new(
        "expand",
        1,
        FeatureExpr::boolean(true),
        ActionSelector::Maximize {
            expr: FeatureExpr::count(FeatureExpr::named("free")),
        },
    ));
    h.validate().unwrap();
    h
}

#[test]
fn takes_immediate_win() {
    let board = Board::parse("XX.O.O...").unwrap();
    let legal = TicTacToeRules.legal_actions(&board);

    for seed in [0u64, 12345u64] {
        let mut interpreter = RuleInterpreter::<TicTacToe>::new(win_block(), featurizer(), seed).unwrap();
        let chosen = interpreter.choose(&board, &legal).unwrap();
        assert_eq!(chosen.0, 2, "seed {seed}");
    }
}

#[test]
fn blocks_immediate_threat() {
    let board = Board::parse("OO.X...X.").unwrap();
    let legal = TicTacToeRules.legal_actions(&board);
    let mut interpreter = RuleInterpreter::<TicTacToe>::new(win_block(), featurizer(), 0).unwrap();
    let chosen = interpreter.choose(&board, &legal).unwrap();
    assert_eq!(chosen.0, 2);
}

#[test]
fn canonical_frame_choice_is_symmetric() {
    let win = Board::parse("XX.O.O...").unwrap();
    let rotated = Board::parse(".OX..X.O.").unwrap();

    let legal_win = TicTacToeRules.legal_actions(&win);
    let legal_rot = TicTacToeRules.legal_actions(&rotated);

    let mut a = RuleInterpreter::<TicTacToe>::new(win_block(), featurizer(), 7).unwrap();
    let mut b = RuleInterpreter::<TicTacToe>::new(win_block(), featurizer(), 7).unwrap();

    let chosen_win = a.choose(&win, &legal_win).unwrap();
    let chosen_rot = b.choose(&rotated, &legal_rot).unwrap();

    assert_eq!(chosen_win.0, 2);
    assert_eq!(chosen_rot.0, 8);
}

#[test]
fn same_seed_same_moves_and_seeds_differ() {
    fn self_play(seed: u64) -> Vec<strategy_discovery::games::tictactoe::Move> {
        let mut interpreter = RuleInterpreter::<TicTacToe>::new(expand(), featurizer(), seed).unwrap();
        let mut board = Board::empty();
        let mut actions = Vec::new();
        loop {
            let legal = TicTacToeRules.legal_actions(&board);
            if legal.is_empty() {
                break;
            }
            let chosen = interpreter.choose(&board, &legal).unwrap();
            board = TicTacToeRules.apply(&board, &chosen).unwrap();
            actions.push(chosen);
        }
        actions
    }

    let seq_a = self_play(3);
    let seq_b = self_play(3);
    assert_eq!(seq_a, seq_b);

    let mut firsts = std::collections::BTreeSet::new();
    for seed in 0..50u64 {
        let mut interpreter = RuleInterpreter::<TicTacToe>::new(expand(), featurizer(), seed).unwrap();
        let legal = TicTacToeRules.legal_actions(&Board::empty());
        let chosen = interpreter.choose(&Board::empty(), &legal).unwrap();
        firsts.insert(chosen.0);
    }
    assert!(firsts.len() >= 2, "expected diverse first choices, got {firsts:?}");
}

#[test]
fn plays_through_match_engine_vs_random_and_perfect() {
    use strategy_discovery::games::tictactoe::Player;

    let registry = StrategyRegistry::new(engine_bundle());
    let provider = registry
        .build(&StrategySpec::HeuristicRules { heuristic: win_block() })
        .unwrap();

    let random = RandomProvider::<TicTacToe>::new();
    let engine = RayonMatchEngine::new(engine_bundle().rules);

    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, provider.as_ref()), (Player::O, &random)];
    let config = MatchConfig {
        games: 2,
        seed: 9,
        max_plies: None,
    };
    let records = engine.run(&config, players).unwrap();
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|r| r.outcome.is_some()));

    let perfect = registry
        .build(&StrategySpec::Minimax(MinimaxConfig {
            depth: 9,
            epsilon: 0.0,
            tie_break: TieBreak::SeededUniform,
        }))
        .unwrap();
    let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, perfect.as_ref()), (Player::O, provider.as_ref())];
    let config = MatchConfig {
        games: 2,
        seed: 9,
        max_plies: None,
    };
    let records = engine.run(&config, players).unwrap();
    assert_eq!(records.len(), 2);
    assert!(records.iter().all(|r| r.outcome.is_some()));
}

#[test]
fn unavailable_native_feature_rejected_at_build() {
    let mut h = HeuristicStrategy::new("bad");
    h.define(FeatureDef::native("nope.feature", Tier::Supplied, "does not exist"))
        .unwrap();
    h.push_rule(Rule::new(
        "r",
        1,
        FeatureExpr::named("nope.feature"),
        ActionSelector::AnyLegal,
    ));

    match RuleInterpreterProvider::<TicTacToe>::new(h, featurizer()) {
        Err(strategy_discovery::core::dsl::DslError::UnavailableFeatures(v)) => {
            assert_eq!(v, vec!["nope.feature".to_string()]);
        }
        Err(other) => panic!("expected UnavailableFeatures, got {other:?}"),
        Ok(_) => panic!("expected UnavailableFeatures, got Ok"),
    }
}
