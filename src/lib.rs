//! Automated strategy discovery framework for board games.
//!
//! Module map: [`core`] holds the game-agnostic framework contracts, feature algebra and
//! heuristic-rule DSL; [`games`] holds concrete game domains; [`strategy`], [`discovery`],
//! [`io`] and [`cli`] are pipeline layers filled in by later phases.

pub mod cli;
pub mod core;
pub mod discovery;
pub mod games;
pub mod io;
pub mod strategy;

/// Crate name from Cargo metadata.
pub const NAME: &str = env!("CARGO_PKG_NAME");
/// Crate version from Cargo metadata.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
