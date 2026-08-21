//! Discovery pipeline runtime: the rayon match engine (Phase 4); corpus generation,
//! annotation and the strategy archive arrive in Phase 5.

pub mod match_engine;
pub use match_engine::{RayonMatchEngine, game_seed, play_game, player_seed, splitmix64};
