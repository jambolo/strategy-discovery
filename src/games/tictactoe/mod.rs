//! Tic-tac-toe domain: board, rules, primitives, tier-2 features, canonicalization, the static
//! evaluator, and the `game-player` engine adapter. Only this module may know anything about
//! tic-tac-toe.

use crate::strategy::registry::EngineBundle;
use std::sync::Arc;

pub mod board;
pub mod canonical;
pub mod corpus;
pub mod engine;
pub mod eval;
pub mod features;
pub mod primitives;
pub mod roster;
pub mod rules;

pub use board::{Board, LINES, Move, Outcome, Player, TicTacToe};
pub use corpus::game_bundle;
pub use eval::TicTacToeEvaluator;
pub use roster::benchmark_roster;
pub use rules::TicTacToeRules;

/// Rules + evaluator bundle for building tic-tac-toe strategies through
/// [`crate::strategy::registry::StrategyRegistry`].
pub fn engine_bundle() -> EngineBundle<TicTacToe> {
    EngineBundle {
        rules: Arc::new(TicTacToeRules),
        evaluator: Arc::new(TicTacToeEvaluator),
    }
}
