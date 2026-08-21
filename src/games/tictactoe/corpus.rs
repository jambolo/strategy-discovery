//! The tic-tac-toe [`GameBundle`]: rules, `[X, O]`, evaluators `default` (open-lines heuristic) and
//! `zero` (`ConstantEvaluator(0.0)`), the D4 canonicalizer and primitives, the benchmark roster as
//! default strategies, exhaustive depth 9 and 765 known canonical positions.

use super::board::{Player, TicTacToe};
use super::canonical::TicTacToeCanonicalizer;
use super::eval::TicTacToeEvaluator;
use super::primitives::TicTacToePrimitives;
use super::roster::benchmark_roster;
use super::rules::TicTacToeRules;
use crate::discovery::bundle::GameBundle;
use crate::strategy::engine::ConstantEvaluator;
use std::collections::BTreeMap;
use std::sync::Arc;

/// Builds the tic-tac-toe [`GameBundle`].
pub fn game_bundle() -> GameBundle<TicTacToe> {
    let mut evaluators: BTreeMap<String, _> = BTreeMap::new();
    evaluators.insert("default".to_string(), Arc::new(TicTacToeEvaluator) as Arc<_>);
    evaluators.insert("zero".to_string(), Arc::new(ConstantEvaluator(0.0)) as Arc<_>);

    GameBundle {
        name: "tictactoe".to_string(),
        rules: Arc::new(TicTacToeRules),
        players: vec![Player::X, Player::O],
        evaluators,
        default_evaluator: "default".to_string(),
        canonicalizer: Some(Arc::new(TicTacToeCanonicalizer::new())),
        primitives: Some(Arc::new(TicTacToePrimitives)),
        default_strategies: benchmark_roster().entries,
        full_search_depth: 9,
        known_canonical_positions: Some(765),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::symmetry::Permutation;
    use crate::discovery::config::CorpusError;
    use crate::games::tictactoe::Board;

    #[test]
    fn game_bundle_facts() {
        let bundle = game_bundle();

        assert_eq!(bundle.name, "tictactoe");
        assert_eq!(bundle.players, vec![Player::X, Player::O]);

        let evaluator_names: Vec<&str> = bundle.evaluators.keys().map(String::as_str).collect();
        assert_eq!(evaluator_names, vec!["default", "zero"]);
        assert_eq!(bundle.default_evaluator, "default");

        assert!(bundle.canonicalizer.is_some());
        assert!(bundle.primitives.is_some());

        let strategy_names: Vec<&str> = bundle.default_strategies.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            strategy_names,
            vec!["random", "depth-1", "depth-2", "depth-3", "depth-4", "depth-6", "perfect"]
        );

        assert_eq!(bundle.full_search_depth, 9);
        assert_eq!(bundle.known_canonical_positions, Some(765));
    }

    #[test]
    fn engine_bundle_selects_evaluator_by_name() {
        let bundle = game_bundle();

        let zero = bundle.engine_bundle("zero").unwrap();
        assert_eq!(zero.evaluator.evaluate(&Board::empty(), Player::X), 0.0);

        assert!(bundle.engine_bundle("default").is_ok());

        match bundle.engine_bundle("nope") {
            Err(CorpusError::Config(msg)) => {
                assert!(msg.contains("unknown evaluator `nope`"));
                assert!(msg.contains("default, zero"));
            }
            Err(other) => panic!("expected CorpusError::Config(_), got {other:?}"),
            Ok(_) => panic!("expected Err(CorpusError::Config(_)), got Ok(_)"),
        }
    }

    #[test]
    fn canonicalize_returns_state_and_transform() {
        let bundle = game_bundle();
        let state = Board::parse("..X......").unwrap();

        let (canonical, transform) = bundle.canonicalize(&state);
        assert_eq!(transform.len(), 9);

        let perm = Permutation::new(transform.clone()).unwrap();
        assert_eq!(bundle.primitives.as_ref().unwrap().transform(&state, &perm), canonical);
        assert_eq!(canonical, bundle.canonicalizer.as_ref().unwrap().canonicalize(&state));

        assert_eq!(bundle.canonicalize(&Board::empty()).0, Board::empty());
    }

    #[test]
    fn player_slot_follows_turn_order() {
        let bundle = game_bundle();
        assert_eq!(bundle.player_slot(Player::X), Some(0));
        assert_eq!(bundle.player_slot(Player::O), Some(1));
    }
}
