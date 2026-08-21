//! Tic-tac-toe domain: board, rules, primitives, tier-2 features, canonicalization and the
//! `game-player` engine adapter. Only this module may know anything about tic-tac-toe.

pub mod board;
pub mod canonical;
pub mod engine;
pub mod features;
pub mod primitives;
pub mod rules;

pub use board::{Board, LINES, Move, Outcome, Player, TicTacToe};
pub use rules::TicTacToeRules;
