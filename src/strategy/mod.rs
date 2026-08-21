//! Strategy implementations: `game-player` engine adapters, minimax, random and scripted
//! strategies. Game-agnostic; games plug in through [`engine::EngineGame`].

pub mod engine;
pub mod minimax;
pub mod random;
pub mod registry;
pub mod roster;
pub mod scripted;

pub use engine::{AliceEvaluator, EngineGame, RulesResponseGenerator, WIN_VALUE};
pub use minimax::{MinimaxConfig, MinimaxProvider, MinimaxStrategy, TieBreak, root_values};
pub use random::{RandomProvider, RandomStrategy};
pub use registry::{EngineBundle, Factory, StrategyRegistry, StrategySpec};
pub use roster::{Roster, RosterEntry};
pub use scripted::{ScriptedProvider, ScriptedStrategy};
