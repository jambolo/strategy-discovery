//! Uniform random strategy: one seeded `ChaCha8Rng` draw per ply.

use crate::core::kinds;
use crate::core::traits::{GameDomain, Strategy, StrategyError, StrategyProvider};
use rand::SeedableRng;
use rand::seq::IndexedRandom;
use rand_chacha::ChaCha8Rng;
use std::marker::PhantomData;

/// Provider for [`RandomStrategy`]; kind `"random"`.
#[derive(Debug, Clone, Copy)]
pub struct RandomProvider<G: GameDomain> {
    _game: PhantomData<G>,
}

impl<G: GameDomain> RandomProvider<G> {
    /// Create the provider.
    pub fn new() -> Self {
        Self { _game: PhantomData }
    }
}

impl<G: GameDomain> Default for RandomProvider<G> {
    fn default() -> Self {
        Self::new()
    }
}

impl<G: GameDomain> StrategyProvider<G> for RandomProvider<G> {
    fn kind(&self) -> &str {
        kinds::RANDOM
    }

    fn create(&self, seed: u64) -> Box<dyn Strategy<G>> {
        Box::new(RandomStrategy::<G>::new(seed))
    }
}

/// Chooses uniformly among the legal actions using a `ChaCha8Rng` seeded from a `u64`;
/// exactly one RNG draw per call (none when `legal` is empty).
#[derive(Debug, Clone)]
pub struct RandomStrategy<G: GameDomain> {
    rng: ChaCha8Rng,
    _game: PhantomData<G>,
}

impl<G: GameDomain> RandomStrategy<G> {
    /// Create a strategy seeded with `seed`.
    pub fn new(seed: u64) -> Self {
        Self {
            rng: ChaCha8Rng::seed_from_u64(seed),
            _game: PhantomData,
        }
    }
}

impl<G: GameDomain> Strategy<G> for RandomStrategy<G> {
    fn choose(&mut self, _state: &G::State, legal: &[G::Action]) -> Result<G::Action, StrategyError> {
        legal.choose(&mut self.rng).cloned().ok_or(StrategyError::NoLegalActions)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::GameRules;
    use crate::games::tictactoe::{Board, TicTacToe, TicTacToeRules};

    #[test]
    fn provider_kind_is_random() {
        assert_eq!(RandomProvider::<TicTacToe>::new().kind(), "random");
        assert_eq!(RandomProvider::<TicTacToe>::default().kind(), kinds::RANDOM);
    }

    #[test]
    fn choice_is_always_legal() {
        let rules = TicTacToeRules;
        let board = Board::empty();
        let legal = rules.legal_actions(&board);
        let provider = RandomProvider::<TicTacToe>::new();
        for seed in 0..8 {
            let mut strategy = provider.create(seed);
            for _ in 0..30 {
                let chosen = strategy.choose(&board, &legal).expect("legal is non-empty");
                assert!(legal.contains(&chosen));
            }
        }
    }

    #[test]
    fn same_seed_reproduces_sequence() {
        let rules = TicTacToeRules;
        let board = Board::empty();
        let legal = rules.legal_actions(&board);
        let provider = RandomProvider::<TicTacToe>::new();

        let mut a = provider.create(11);
        let mut b = provider.create(11);
        let seq_a: Vec<_> = (0..30).map(|_| a.choose(&board, &legal).unwrap()).collect();
        let seq_b: Vec<_> = (0..30).map(|_| b.choose(&board, &legal).unwrap()).collect();
        assert_eq!(seq_a, seq_b);
    }

    #[test]
    fn different_seeds_differ() {
        let rules = TicTacToeRules;
        let board = Board::empty();
        let legal = rules.legal_actions(&board);
        let provider = RandomProvider::<TicTacToe>::new();

        let mut a = provider.create(1);
        let mut b = provider.create(2);
        let seq_a: Vec<_> = (0..30).map(|_| a.choose(&board, &legal).unwrap()).collect();
        let seq_b: Vec<_> = (0..30).map(|_| b.choose(&board, &legal).unwrap()).collect();
        assert_ne!(seq_a, seq_b);
    }

    #[test]
    fn empty_legal_is_no_legal_actions() {
        let board = Board::empty();
        let provider = RandomProvider::<TicTacToe>::new();
        let mut strategy = provider.create(0);
        let legal: Vec<crate::games::tictactoe::Move> = Vec::new();
        assert_eq!(strategy.choose(&board, &legal), Err(StrategyError::NoLegalActions));
    }

    #[test]
    fn self_play_game_terminates_with_outcome() {
        let rules = TicTacToeRules;
        let provider = RandomProvider::<TicTacToe>::new();
        let mut strategy_a = provider.create(3);
        let mut strategy_b = provider.create(4);

        let mut state = rules.initial_state();
        let mut plies = 0;
        loop {
            let legal = rules.legal_actions(&state);
            if legal.is_empty() {
                break;
            }
            assert!(plies < 9, "tic-tac-toe cannot exceed 9 plies");
            let action = if plies % 2 == 0 {
                strategy_a.choose(&state, &legal).unwrap()
            } else {
                strategy_b.choose(&state, &legal).unwrap()
            };
            state = rules.apply(&state, &action).unwrap();
            plies += 1;
        }

        assert!(rules.outcome(&state).is_some());
        assert!(plies <= 9);
    }
}
