//! Minimax strategy over the `game-player` engine: configurable depth, epsilon-random play
//! and seeded uniform tie-breaking via principal-variation replay (`root_values`).

use crate::core::kinds;
use crate::core::traits::{GameRules, StateEvaluator, Strategy, StrategyError, StrategyProvider};
use crate::strategy::engine::{AliceEvaluator, EngineGame, RulesResponseGenerator};
use game_player::PlayerId;
use game_player::StaticEvaluator;
use game_player::minimax::search;
use rand::seq::IndexedRandom;
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::sync::Arc;

/// How `MinimaxStrategy` resolves ties among root actions with equal searched value.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum TieBreak {
    /// Defer to `game-player`'s own internal tie-break (best static value, then generation
    /// order): deterministic but unseedable.
    Engine,
    /// Collect every root action whose depth-searched value equals the best, and pick among
    /// them uniformly using the strategy's seeded RNG.
    #[default]
    SeededUniform,
}

/// Configuration for [`MinimaxStrategy`] / [`MinimaxProvider`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MinimaxConfig {
    /// Search depth in plies. Must be `>= 1`.
    pub depth: u32,
    /// Probability in `[0, 1]` of playing a uniformly random legal action instead of
    /// searching, checked once per ply before any tie-break RNG use.
    #[serde(default)]
    pub epsilon: f64,
    /// Tie-break policy among root actions with equal searched value.
    #[serde(default)]
    pub tie_break: TieBreak,
}

impl MinimaxConfig {
    /// Validates `depth >= 1` and `epsilon` finite and in `[0, 1]`.
    pub fn validate(&self) -> Result<(), StrategyError> {
        if self.depth == 0 {
            return Err(StrategyError::Other("minimax depth must be >= 1".to_string()));
        }
        if !(0.0..=1.0).contains(&self.epsilon) {
            return Err(StrategyError::Other(format!(
                "minimax epsilon must be in [0, 1], got {}",
                self.epsilon
            )));
        }
        Ok(())
    }
}

/// Alice-perspective depth-`depth` minimax value of each action in `legal`, in `legal` order.
///
/// `game-player`'s `search` exposes no root values, only a chosen move. Each value is
/// recovered by principal-variation replay: the depth-`k` value of a state equals the
/// depth-`(k - 1)` value of the state reached by playing `search`'s chosen move, so replaying
/// that move to a leaf (terminal state or `k == 0`) and statically evaluating the leaf
/// recovers the depth-`k` value of the root action's child. Public because Phase 5 annotation
/// and Phase 7 metrics reuse it.
pub fn root_values<G: EngineGame>(
    rules: &dyn GameRules<G>,
    evaluator: &dyn StateEvaluator<G>,
    state: &G::State,
    legal: &[G::Action],
    depth: u32,
) -> Vec<(G::Action, f32)> {
    let sef = AliceEvaluator::new(rules, evaluator);
    let rg = RulesResponseGenerator::new(rules);
    legal
        .iter()
        .map(|action| {
            let child = rules.apply(state, action).expect("legal action applies");
            let value = pv_value(rules, &sef, &rg, child, depth.saturating_sub(1));
            (action.clone(), value)
        })
        .collect()
}

/// Depth-`k` value of `state` by principal-variation replay: at a terminal state or `k == 0`
/// the value is the static (terminal-aware) evaluation; otherwise it is the depth-`(k - 1)`
/// value of the state reached by playing the engine's chosen move.
fn pv_value<G: EngineGame>(
    rules: &dyn GameRules<G>,
    sef: &AliceEvaluator<'_, G>,
    rg: &RulesResponseGenerator<'_, G>,
    mut state: G::State,
    mut k: u32,
) -> f32 {
    loop {
        if rules.is_terminal(&state) || k == 0 {
            return sef.evaluate(&state);
        }
        let action = search(sef, rg, &state, k).expect("non-terminal state has a move");
        state = rules.apply(&state, &action).expect("legal action applies");
        k -= 1;
    }
}

/// Factory for [`MinimaxStrategy`] instances sharing rules, evaluator and configuration but
/// independently seeded.
pub struct MinimaxProvider<G: EngineGame> {
    rules: Arc<dyn GameRules<G>>,
    evaluator: Arc<dyn StateEvaluator<G>>,
    config: MinimaxConfig,
}

impl<G: EngineGame> MinimaxProvider<G> {
    /// Builds a provider, validating `config`.
    pub fn new(
        rules: Arc<dyn GameRules<G>>,
        evaluator: Arc<dyn StateEvaluator<G>>,
        config: MinimaxConfig,
    ) -> Result<Self, StrategyError> {
        config.validate()?;
        Ok(Self {
            rules,
            evaluator,
            config,
        })
    }

    /// The provider's configuration.
    pub fn config(&self) -> &MinimaxConfig {
        &self.config
    }
}

impl<G: EngineGame> StrategyProvider<G> for MinimaxProvider<G> {
    fn kind(&self) -> &str {
        kinds::MINIMAX
    }

    fn create(&self, seed: u64) -> Box<dyn Strategy<G>> {
        Box::new(MinimaxStrategy {
            rules: Arc::clone(&self.rules),
            evaluator: Arc::clone(&self.evaluator),
            config: self.config.clone(),
            rng: ChaCha8Rng::seed_from_u64(seed),
        })
    }
}

/// Minimax over the `game-player` engine, with epsilon-random play and seeded tie-breaking.
pub struct MinimaxStrategy<G: EngineGame> {
    rules: Arc<dyn GameRules<G>>,
    evaluator: Arc<dyn StateEvaluator<G>>,
    config: MinimaxConfig,
    rng: ChaCha8Rng,
}

impl<G: EngineGame> MinimaxStrategy<G> {
    /// Builds a strategy, validating `config`, seeded with `seed`.
    pub fn new(
        rules: Arc<dyn GameRules<G>>,
        evaluator: Arc<dyn StateEvaluator<G>>,
        config: MinimaxConfig,
        seed: u64,
    ) -> Result<Self, StrategyError> {
        config.validate()?;
        Ok(Self {
            rules,
            evaluator,
            config,
            rng: ChaCha8Rng::seed_from_u64(seed),
        })
    }

    /// The strategy's configuration.
    pub fn config(&self) -> &MinimaxConfig {
        &self.config
    }

    /// Depth-`config.depth` minimax value of each action in `legal`; see [`root_values`].
    pub fn root_values(&self, state: &G::State, legal: &[G::Action]) -> Vec<(G::Action, f32)> {
        root_values(self.rules.as_ref(), self.evaluator.as_ref(), state, legal, self.config.depth)
    }
}

impl<G: EngineGame> Strategy<G> for MinimaxStrategy<G> {
    fn choose(&mut self, state: &G::State, legal: &[G::Action]) -> Result<G::Action, StrategyError> {
        if legal.is_empty() {
            return Err(StrategyError::NoLegalActions);
        }
        if legal.len() == 1 {
            // Forced move: return without touching the RNG so streams stay stable across
            // forced positions (matters for `epsilon_one_matches_reference_uniform_stream`).
            return Ok(legal[0].clone());
        }

        if self.config.epsilon > 0.0 {
            let r: f64 = self.rng.random();
            if r < self.config.epsilon {
                let action = legal.choose(&mut self.rng).cloned().expect("legal is non-empty");
                return Ok(action);
            }
        }

        let rules: &dyn GameRules<G> = self.rules.as_ref();
        let evaluator: &dyn StateEvaluator<G> = self.evaluator.as_ref();
        let sef = AliceEvaluator::new(rules, evaluator);
        let rg = RulesResponseGenerator::new(rules);

        match self.config.tie_break {
            TieBreak::Engine => Ok(search(&sef, &rg, state, self.config.depth).expect("non-terminal state has a move")),
            TieBreak::SeededUniform => {
                let values = root_values(rules, evaluator, state, legal, self.config.depth);
                let maximize = G::player_id(rules.player_to_move(state)) == PlayerId::Alice;
                let best = if maximize {
                    values.iter().map(|(_, v)| *v).max_by(f32::total_cmp)
                } else {
                    values.iter().map(|(_, v)| *v).min_by(f32::total_cmp)
                }
                .expect("legal is non-empty");
                let ties: Vec<G::Action> = values
                    .into_iter()
                    .filter(|(_, v)| v.total_cmp(&best) == Ordering::Equal)
                    .map(|(a, _)| a)
                    .collect();
                Ok(ties.choose(&mut self.rng).cloned().expect("ties non-empty"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::tictactoe::{Board, Move, Outcome, Player, TicTacToe, TicTacToeEvaluator, TicTacToeRules};
    use std::collections::HashSet;

    /// Fixture boards from step 02-ttt-eval, all non-terminal.
    const FIXTURES: [&str; 7] = [
        "XX.OO....",
        "XX.OO..X.",
        "XX.O.....",
        "X...OO.X.",
        "XX.OO.X..",
        "X...O...X",
        ".........",
    ];

    fn provider(depth: u32, epsilon: f64, tie_break: TieBreak) -> MinimaxProvider<TicTacToe> {
        MinimaxProvider::new(
            Arc::new(TicTacToeRules),
            Arc::new(TicTacToeEvaluator),
            MinimaxConfig {
                depth,
                epsilon,
                tie_break,
            },
        )
        .unwrap()
    }

    fn engine_move(board: &Board, depth: u32) -> Move {
        let rules = TicTacToeRules;
        let eval = TicTacToeEvaluator;
        let sef = AliceEvaluator::<TicTacToe>::new(&rules, &eval);
        let rg = RulesResponseGenerator::<TicTacToe>::new(&rules);
        search(&sef, &rg, board, depth).expect("non-terminal state has a move")
    }

    #[test]
    fn config_rejects_zero_depth() {
        let result = MinimaxProvider::<TicTacToe>::new(
            Arc::new(TicTacToeRules),
            Arc::new(TicTacToeEvaluator),
            MinimaxConfig {
                depth: 0,
                epsilon: 0.0,
                tie_break: TieBreak::SeededUniform,
            },
        );
        assert!(matches!(result, Err(StrategyError::Other(_))));
    }

    #[test]
    fn config_rejects_epsilon_out_of_range() {
        for epsilon in [1.5, -0.1] {
            let config = MinimaxConfig {
                depth: 1,
                epsilon,
                tie_break: TieBreak::SeededUniform,
            };
            assert!(matches!(config.validate(), Err(StrategyError::Other(_))));
        }
        for epsilon in [0.0, 1.0] {
            let config = MinimaxConfig {
                depth: 1,
                epsilon,
                tie_break: TieBreak::SeededUniform,
            };
            assert!(config.validate().is_ok());
        }
    }

    #[test]
    fn config_serde_defaults_and_kebab_case() {
        let config: MinimaxConfig = serde_json::from_str(r#"{"depth":9}"#).unwrap();
        assert_eq!(config.epsilon, 0.0);
        assert_eq!(config.tie_break, TieBreak::SeededUniform);

        let json = serde_json::to_string(&MinimaxConfig {
            depth: 9,
            epsilon: 0.0,
            tie_break: TieBreak::SeededUniform,
        })
        .unwrap();
        assert_eq!(json, r#"{"depth":9,"epsilon":0.0,"tie_break":"seeded-uniform"}"#);

        assert_eq!(serde_json::to_string(&TieBreak::Engine).unwrap(), "\"engine\"");
    }

    #[test]
    fn provider_kind_is_minimax() {
        assert_eq!(provider(9, 0.0, TieBreak::SeededUniform).kind(), "minimax");
    }

    #[test]
    fn engine_tie_break_matches_search_on_fixtures() {
        let rules = TicTacToeRules;
        for fixture in FIXTURES {
            let board = Board::parse(fixture).unwrap();
            let legal = rules.legal_actions(&board);
            let mut strat = provider(9, 0.0, TieBreak::Engine).create(0);
            assert_eq!(strat.choose(&board, &legal), Ok(engine_move(&board, 9)));
        }
    }

    #[test]
    fn depth_one_takes_immediate_win() {
        let rules = TicTacToeRules;
        let cases = [("XX.OO....", Move(2)), ("XX.OO..X.", Move(5)), ("XX.OO.X..", Move(5))];
        for tie_break in [TieBreak::Engine, TieBreak::SeededUniform] {
            for seed in 0..4 {
                for (fixture, expected) in cases {
                    let board = Board::parse(fixture).unwrap();
                    let legal = rules.legal_actions(&board);
                    let mut strat = provider(1, 0.0, tie_break).create(seed);
                    assert_eq!(strat.choose(&board, &legal), Ok(expected));
                }
            }
        }
    }

    #[test]
    fn depth_two_blocks_immediate_threat() {
        let rules = TicTacToeRules;
        let cases = [("XX.O.....", Move(2)), ("X...OO.X.", Move(3))];
        for seed in 0..8 {
            for (fixture, expected) in cases {
                let board = Board::parse(fixture).unwrap();
                let legal = rules.legal_actions(&board);
                let mut strat = provider(2, 0.0, TieBreak::SeededUniform).create(seed);
                assert_eq!(strat.choose(&board, &legal), Ok(expected));
            }
        }
    }

    #[test]
    fn root_values_empty_board_depth_nine_all_zero() {
        let rules = TicTacToeRules;
        let eval = TicTacToeEvaluator;
        let board = Board::empty();
        let legal = rules.legal_actions(&board);
        let values = root_values(&rules, &eval, &board, &legal, 9);
        assert_eq!(values.len(), 9);
        for ((action, value), expected_action) in values.iter().zip(legal.iter()) {
            assert_eq!(action, expected_action);
            assert_eq!(*value, 0.0);
        }
    }

    #[test]
    fn root_values_immediate_win_is_unique_maximum() {
        let rules = TicTacToeRules;
        let eval = TicTacToeEvaluator;
        let board = Board::parse("XX.OO....").unwrap();
        let legal = rules.legal_actions(&board);
        for depth in [1, 9] {
            let values = root_values(&rules, &eval, &board, &legal, depth);
            for (action, value) in &values {
                if *action == Move(2) {
                    assert_eq!(*value, 100.0);
                } else {
                    assert!(*value < 100.0);
                }
            }
        }
    }

    #[test]
    fn root_values_method_matches_free_function() {
        let rules = TicTacToeRules;
        let eval = TicTacToeEvaluator;
        let board = Board::parse("XX.OO....").unwrap();
        let legal = rules.legal_actions(&board);

        let strat = MinimaxStrategy::<TicTacToe>::new(
            Arc::new(TicTacToeRules),
            Arc::new(TicTacToeEvaluator),
            MinimaxConfig {
                depth: 9,
                epsilon: 0.0,
                tie_break: TieBreak::SeededUniform,
            },
            0,
        )
        .unwrap();

        let via_method = strat.root_values(&board, &legal);
        let via_fn = root_values(&rules, &eval, &board, &legal, 9);
        assert_eq!(via_method, via_fn);
    }

    #[test]
    fn seeded_uniform_picks_only_best_valued_actions() {
        let rules = TicTacToeRules;
        let eval = TicTacToeEvaluator;
        for fixture in FIXTURES {
            let board = Board::parse(fixture).unwrap();
            let legal = rules.legal_actions(&board);
            let values = root_values(&rules, &eval, &board, &legal, 9);
            let maximize = rules.player_to_move(&board) == Player::X;
            let best = if maximize {
                values.iter().map(|(_, v)| *v).max_by(f32::total_cmp)
            } else {
                values.iter().map(|(_, v)| *v).min_by(f32::total_cmp)
            }
            .unwrap();
            let ties: Vec<Move> = values
                .iter()
                .filter(|(_, v)| v.total_cmp(&best) == Ordering::Equal)
                .map(|(a, _)| *a)
                .collect();

            for seed in 0..16 {
                let mut strat = provider(9, 0.0, TieBreak::SeededUniform).create(seed);
                let choice = strat.choose(&board, &legal).unwrap();
                assert!(ties.contains(&choice), "seed {seed}: {choice:?} not in {ties:?}");
            }
        }
    }

    #[test]
    fn seeded_uniform_is_reproducible_and_varies_across_seeds() {
        let rules = TicTacToeRules;
        let board = Board::empty();
        let legal = rules.legal_actions(&board);
        let p = provider(9, 0.0, TieBreak::SeededUniform);

        let first = p.create(5).choose(&board, &legal).unwrap();
        let second = p.create(5).choose(&board, &legal).unwrap();
        assert_eq!(first, second);

        let distinct: HashSet<Move> = (0..32).map(|seed| p.create(seed).choose(&board, &legal).unwrap()).collect();
        assert!(distinct.len() >= 3, "only {} distinct first choices", distinct.len());
    }

    #[test]
    fn epsilon_one_matches_reference_uniform_stream() {
        let rules = TicTacToeRules;
        for seed in [7u64, 8u64] {
            let mut strat = provider(9, 1.0, TieBreak::SeededUniform).create(seed);
            let mut rng = ChaCha8Rng::seed_from_u64(seed);
            let mut board = Board::empty();
            loop {
                let legal = rules.legal_actions(&board);
                if legal.is_empty() {
                    break;
                }
                let expected = if legal.len() == 1 {
                    legal[0]
                } else {
                    let _r: f64 = rng.random();
                    *legal.choose(&mut rng).unwrap()
                };
                assert_eq!(strat.choose(&board, &legal), Ok(expected));
                board = rules.apply(&board, &expected).unwrap();
            }
        }
    }

    #[test]
    fn single_legal_action_is_returned_directly() {
        let board = Board::parse("XOXXOOOX.").unwrap();
        let legal = TicTacToeRules.legal_actions(&board);
        assert_eq!(legal, vec![Move(8)]);
        let mut strat = provider(9, 0.5, TieBreak::SeededUniform).create(1);
        assert_eq!(strat.choose(&board, &legal), Ok(Move(8)));
    }

    #[test]
    fn empty_legal_is_no_legal_actions() {
        let mut strat = provider(9, 0.0, TieBreak::SeededUniform).create(0);
        assert_eq!(strat.choose(&Board::empty(), &[]), Err(StrategyError::NoLegalActions));
    }

    #[test]
    fn perfect_self_play_draws_for_a_few_seeds() {
        let rules = TicTacToeRules;
        let p = provider(9, 0.0, TieBreak::SeededUniform);
        for seed in 0..4u64 {
            let mut x_strat = p.create(seed);
            let mut o_strat = p.create(seed + 100);
            let mut board = Board::empty();
            loop {
                let legal = rules.legal_actions(&board);
                if legal.is_empty() {
                    break;
                }
                let action = if rules.player_to_move(&board) == Player::X {
                    x_strat.choose(&board, &legal).unwrap()
                } else {
                    o_strat.choose(&board, &legal).unwrap()
                };
                board = rules.apply(&board, &action).unwrap();
            }
            assert_eq!(rules.outcome(&board), Some(Outcome::Draw));
        }
    }
}
