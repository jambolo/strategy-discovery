//! The tic-tac-toe [`GameBundle`]: rules, `[X, O]`, evaluators `default` (open-lines heuristic) and
//! `zero` (`ConstantEvaluator(0.0)`), the D4 canonicalizer and primitives, the tier-2
//! [`TicTacToeFeatures`] extractor, the benchmark roster as both `default_strategies` and `roster`,
//! exhaustive depth 9 and 765 known canonical positions.

use super::board::{Player, TicTacToe};
use super::canonical::TicTacToeCanonicalizer;
use super::eval::TicTacToeEvaluator;
use super::features::TicTacToeFeatures;
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

    let roster = benchmark_roster();

    GameBundle {
        name: "tictactoe".to_string(),
        rules: Arc::new(TicTacToeRules),
        players: vec![Player::X, Player::O],
        evaluators,
        default_evaluator: "default".to_string(),
        canonicalizer: Some(Arc::new(TicTacToeCanonicalizer::new())),
        primitives: Some(Arc::new(TicTacToePrimitives)),
        supplied_features: Some(Arc::new(TicTacToeFeatures)),
        default_strategies: roster.entries.clone(),
        roster,
        full_search_depth: 9,
        known_canonical_positions: Some(765),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::derived::PrimitiveFeatures;
    use crate::core::featurizer::FeaturizerSpec;
    use crate::core::symmetry::Permutation;
    use crate::core::traits::{FeatureExtractor, GameRules};
    use crate::discovery::config::CorpusError;
    use crate::games::tictactoe::Board;
    use std::collections::HashSet;

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

    #[test]
    fn bundle_roster_is_the_versioned_benchmark_population() {
        let bundle = game_bundle();

        assert_eq!(bundle.roster.id(), "ttt-benchmark-v1");
        assert_eq!(bundle.roster.entries, bundle.default_strategies);
        assert_eq!(bundle.roster.entries.len(), 7);
        assert_eq!(bundle.roster.entries.last().unwrap().name, "perfect");
    }

    #[test]
    fn featurizer_vocabulary_order_and_count() {
        let featurizer = game_bundle().featurizer(true).unwrap();
        let names: Vec<String> = featurizer.vocabulary().defs().iter().map(|d| d.name.clone()).collect();

        let tier1_names: Vec<String> = PrimitiveFeatures::new(TicTacToeRules, TicTacToePrimitives)
            .definitions()
            .iter()
            .map(|d| d.name.clone())
            .collect();
        assert_eq!(tier1_names.len(), 47);

        let mut expected = vec!["side_to_move".to_string()];
        expected.extend(tier1_names);
        expected.extend(TicTacToeFeatures.definitions().iter().map(|d| d.name.clone()));

        assert_eq!(names, expected);
        assert_eq!(expected.len(), 70);
        assert_eq!(&names[..48], &expected[..48]);
        for name in &names[48..] {
            assert!(name.starts_with("ttt."), "unexpected tier-2 name: {name}");
        }
    }

    #[test]
    fn featurizer_without_supplied_is_tier1_only() {
        let vocabulary_owner = game_bundle().featurizer(false).unwrap();
        let vocabulary = vocabulary_owner.vocabulary();
        assert_eq!(vocabulary.len(), 48);
        assert!(vocabulary.defs().iter().all(|d| !d.name.starts_with("ttt.")));
    }

    #[test]
    fn featurizer_with_extended_counts() {
        let f = game_bundle()
            .featurizer_with(FeaturizerSpec {
                include_supplied: true,
                extended: true,
            })
            .unwrap();
        let names: Vec<String> = f.vocabulary().defs().iter().map(|d| d.name.clone()).collect();

        assert_eq!(names.len(), 104);
        assert_eq!(names[0], "side_to_move");

        let tier1_names: Vec<String> = PrimitiveFeatures::new(TicTacToeRules, TicTacToePrimitives)
            .definitions()
            .iter()
            .map(|d| d.name.clone())
            .collect();
        assert_eq!(names[1..48], tier1_names[..]);

        assert_eq!(names[48], "lines.mine0.theirs0");
        assert_eq!(names[57], "lines.mine3.theirs0");
        assert_eq!(names[58], "cells.mine0.theirs0.ge1");
        assert_eq!(names[81], "cells.mine2.theirs0.ge4");

        assert_eq!(names[82..].len(), 22);
        for name in &names[82..] {
            assert!(name.starts_with("ttt."), "unexpected supplied name: {name}");
        }

        let without_supplied = game_bundle()
            .featurizer_with(FeaturizerSpec {
                include_supplied: false,
                extended: true,
            })
            .unwrap();
        let names_no_supplied: Vec<String> = without_supplied.vocabulary().defs().iter().map(|d| d.name.clone()).collect();
        assert_eq!(names_no_supplied.len(), 82);
        assert!(names_no_supplied.iter().all(|n| !n.starts_with("ttt.")));
    }

    #[test]
    fn extended_tier1_matches_supplied_extensionally() {
        let f = game_bundle()
            .featurizer_with(FeaturizerSpec {
                include_supplied: true,
                extended: true,
            })
            .unwrap();
        let rules = TicTacToeRules;

        let mut seen: HashSet<Board> = HashSet::new();
        let mut stack: Vec<Board> = vec![Board::empty()];
        seen.insert(Board::empty());

        while let Some(state) = stack.pop() {
            if rules.outcome(&state).is_some() {
                continue;
            }
            let v = f.extract(&state);
            assert_eq!(v.get("lines.mine2.theirs0"), v.get("ttt.threats.mine"));
            assert_eq!(v.get("lines.mine0.theirs2"), v.get("ttt.threats.theirs"));
            assert_eq!(v.get("cells.mine2.theirs0.ge1"), v.get("ttt.winning_cells"));
            assert_eq!(v.get("cells.mine0.theirs2.ge1"), v.get("ttt.blocking_cells"));
            assert_eq!(v.get("cells.mine1.theirs0.ge2"), v.get("ttt.fork_cells"));

            for action in rules.legal_actions(&state) {
                let next = rules.apply(&state, &action).unwrap();
                if seen.insert(next) {
                    stack.push(next);
                }
            }
        }

        assert_eq!(seen.len(), 5478);
    }

    #[test]
    fn featurizer_requires_primitives() {
        let mut bundle = game_bundle();
        bundle.primitives = None;

        match bundle.featurizer(true) {
            Err(CorpusError::Precondition(msg)) => assert!(msg.contains("declares no primitives")),
            Err(other) => panic!("expected Err(CorpusError::Precondition(_)), got Err({other:?})"),
            Ok(_) => panic!("expected Err(CorpusError::Precondition(_)), got Ok(_)"),
        }
    }
}
