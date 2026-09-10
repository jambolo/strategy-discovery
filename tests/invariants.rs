//! Property-based invariant tests: tic-tac-toe rules/canonicalization over
//! random play-outs, and serde round trips for the feature/DSL/strategy types.

use proptest::prelude::*;
use strategy_discovery::core::dsl::{ActionSelector, HeuristicStrategy, Rule};
use strategy_discovery::core::features::{ArithOp, CmpOp, FeatureExpr, FeatureValue, SetOp};
use strategy_discovery::core::traits::{Canonicalize, GameRules};
use strategy_discovery::games::tictactoe::canonical::TicTacToeCanonicalizer;
use strategy_discovery::games::tictactoe::{Board, LINES, Move, Player, TicTacToeRules};
use strategy_discovery::strategy::minimax::{MinimaxConfig, TieBreak};
use strategy_discovery::strategy::registry::StrategySpec;

/// Random play-outs from the initial state, driven by choice indices; stops at
/// a terminal state or when the choice vector is exhausted.
fn reachable_board() -> impl Strategy<Value = Board> {
    prop::collection::vec(any::<u8>(), 0..=9).prop_map(|choices| {
        let rules = TicTacToeRules;
        let mut state = rules.initial_state();
        for c in choices {
            let legal = rules.legal_actions(&state);
            if legal.is_empty() {
                break;
            }
            state = rules.apply(&state, &legal[c as usize % legal.len()]).unwrap();
        }
        state
    })
}

fn feature_value() -> impl Strategy<Value = FeatureValue> {
    prop_oneof![
        any::<bool>().prop_map(FeatureValue::Bool),
        any::<i64>().prop_map(FeatureValue::Int),
        (-1.0e6f64..1.0e6f64).prop_map(FeatureValue::Float),
        prop::collection::btree_set(0usize..9, 0..4).prop_map(FeatureValue::Set),
    ]
}

fn feature_expr() -> impl Strategy<Value = FeatureExpr> {
    let leaf = prop_oneof![
        feature_value().prop_map(|value| FeatureExpr::Const { value }),
        "[a-z][a-z0-9_]{0,7}".prop_map(|name| FeatureExpr::Ref { name }),
    ];
    leaf.prop_recursive(4, 64, 8, |inner| {
        prop_oneof![
            inner.clone().prop_map(|expr| FeatureExpr::Not { expr: Box::new(expr) }),
            prop::collection::vec(inner.clone(), 0..3).prop_map(|exprs| FeatureExpr::And { exprs }),
            prop::collection::vec(inner.clone(), 0..3).prop_map(|exprs| FeatureExpr::Or { exprs }),
            (
                prop_oneof![
                    Just(CmpOp::Eq),
                    Just(CmpOp::Ne),
                    Just(CmpOp::Lt),
                    Just(CmpOp::Le),
                    Just(CmpOp::Gt),
                    Just(CmpOp::Ge),
                ],
                inner.clone(),
                inner.clone(),
            )
                .prop_map(|(op, lhs, rhs)| FeatureExpr::Cmp {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }),
            (
                prop_oneof![Just(ArithOp::Add), Just(ArithOp::Sub), Just(ArithOp::Mul)],
                inner.clone(),
                inner.clone(),
            )
                .prop_map(|(op, lhs, rhs)| FeatureExpr::Arith {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }),
            inner.clone().prop_map(|expr| FeatureExpr::Count { expr: Box::new(expr) }),
            (
                prop_oneof![Just(SetOp::Union), Just(SetOp::Intersection), Just(SetOp::Difference)],
                inner.clone(),
                inner.clone(),
            )
                .prop_map(|(op, lhs, rhs)| FeatureExpr::SetOp {
                    op,
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }),
            (inner.clone(), inner).prop_map(|(set, element)| FeatureExpr::Contains {
                set: Box::new(set),
                element: Box::new(element),
            }),
        ]
    })
}

fn action_selector() -> impl Strategy<Value = ActionSelector> {
    prop_oneof![
        Just(ActionSelector::AnyLegal),
        feature_expr().prop_map(|expr| ActionSelector::TargetIn { expr }),
        feature_expr().prop_map(|expr| ActionSelector::Maximize { expr }),
    ]
}

fn heuristic_strategy() -> impl Strategy<Value = HeuristicStrategy> {
    (
        "[a-z][a-z0-9_]{0,7}",
        prop::collection::vec(("[a-z][a-z0-9_]{0,7}", any::<i32>(), feature_expr(), action_selector()), 0..4),
        action_selector(),
    )
        .prop_map(|(name, rules, fallback)| {
            let mut strategy = HeuristicStrategy::new(name);
            for (rule_name, priority, condition, action) in rules {
                strategy.push_rule(Rule::new(rule_name, priority, condition, action));
            }
            strategy.fallback = fallback;
            strategy
        })
}

fn strategy_spec() -> impl Strategy<Value = StrategySpec> {
    prop_oneof![
        Just(StrategySpec::Random),
        (
            1u32..=9,
            0.0f64..=1.0,
            prop_oneof![Just(TieBreak::Engine), Just(TieBreak::SeededUniform)]
        )
            .prop_map(|(depth, epsilon, tie_break)| StrategySpec::Minimax(MinimaxConfig {
                depth,
                epsilon,
                tie_break,
            })),
        heuristic_strategy().prop_map(|heuristic| StrategySpec::HeuristicRules { heuristic }),
    ]
}

/// Tic-tac-toe board validity: cell-count parity matches `to_move`, no more
/// than one player has completed a line, and `legal_actions` matches the
/// empty cells (or is empty when terminal).
fn is_valid(board: &Board) -> bool {
    let x = board.cells().iter().filter(|c| **c == Some(Player::X)).count();
    let o = board.cells().iter().filter(|c| **c == Some(Player::O)).count();
    if !(x == o || x == o + 1) {
        return false;
    }
    let expected_to_move = if x == o { Player::X } else { Player::O };
    if board.to_move() != expected_to_move {
        return false;
    }
    let has_line = |player: Player| LINES.iter().any(|line| line.iter().all(|&i| board.cell(i) == Some(player)));
    if has_line(Player::X) && has_line(Player::O) {
        return false;
    }
    let rules = TicTacToeRules;
    let expected_legal: Vec<Move> = (0..9).filter(|&i| board.cell(i).is_none()).map(Move).collect();
    let actual_legal = rules.legal_actions(board);
    if rules.is_terminal(board) {
        actual_legal.is_empty()
    } else {
        actual_legal == expected_legal
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn terminal_states_have_no_legal_moves(board in reachable_board()) {
        let rules = TicTacToeRules;
        prop_assert_eq!(rules.is_terminal(&board), rules.outcome(&board).is_some());
        let legal = rules.legal_actions(&board);
        if rules.is_terminal(&board) {
            prop_assert!(legal.is_empty());
            prop_assert!(rules.apply(&board, &Move(0)).is_err());
        } else {
            prop_assert!(!legal.is_empty());
        }
    }

    #[test]
    fn legal_moves_preserve_board_validity(board in reachable_board()) {
        prop_assert!(is_valid(&board));
        let rules = TicTacToeRules;
        for a in rules.legal_actions(&board) {
            let next = rules.apply(&board, &a).unwrap();
            prop_assert!(is_valid(&next));
            prop_assert_eq!(next.cell(a.0), Some(board.to_move()));
        }
    }

    #[test]
    fn canonicalization_is_idempotent(board in reachable_board()) {
        let c = TicTacToeCanonicalizer::new();
        let (canonical, perm) = c.canonicalize_with_transform(&board);
        prop_assert_eq!(perm.as_slice().len(), 9);
        let (again, perm2) = c.canonicalize_with_transform(&canonical);
        prop_assert_eq!(again, canonical);
        prop_assert!(perm2.is_identity());
        prop_assert_eq!(c.canonicalize(&canonical), canonical);
    }

    #[test]
    fn canonicalization_preserves_outcome(board in reachable_board()) {
        let rules = TicTacToeRules;
        let c = TicTacToeCanonicalizer::new();
        let canonical = c.canonicalize(&board);
        prop_assert_eq!(rules.outcome(&canonical), rules.outcome(&board));
        prop_assert_eq!(canonical.to_move(), board.to_move());
        prop_assert_eq!(rules.legal_actions(&canonical).len(), rules.legal_actions(&board).len());
        prop_assert!(is_valid(&canonical));
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn feature_expr_serde_round_trips(expr in feature_expr()) {
        let json = serde_json::to_string(&expr).unwrap();
        let back: FeatureExpr = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(back, expr);
    }

    #[test]
    fn heuristic_strategy_serde_round_trips(h in heuristic_strategy()) {
        let json = serde_json::to_string(&h).unwrap();
        let back: HeuristicStrategy = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(back.kind.as_str(), "heuristic-rules");
        prop_assert_eq!(back, h);
    }

    #[test]
    fn strategy_spec_serde_round_trips(spec in strategy_spec()) {
        let json = serde_json::to_string(&spec).unwrap();
        let back: StrategySpec = serde_json::from_str(&json).unwrap();
        prop_assert_eq!(&back, &spec);

        let text = toml::to_string(&spec).unwrap();
        let back: StrategySpec = toml::from_str(&text).unwrap();
        prop_assert_eq!(back, spec);
    }
}
