//! Game-name resolution — the ONLY non-test file in the crate that names a concrete game.
//!
//! Every command resolves a game name (from `--game`, a config's `game` field, or a run
//! directory's `run.json`) through `dispatch_game!`, which binds a `&GameBundle<G>` for
//! the matching concrete game and evaluates a generic body over it. Adding a game = one
//! arm in `dispatch_game!` plus one entry in [`KNOWN_GAMES`]; nothing else in `src/cli/`
//! changes.

use crate::discovery::CorpusError;
use crate::io::{IoError, RUN_FILE, read_json};
use std::path::Path;

/// Every game name the CLI can resolve, in display order.
pub const KNOWN_GAMES: &[&str] = &["tictactoe"];

/// The one field of a config or manifest needed to pick a game bundle.
#[derive(serde::Deserialize)]
struct GameName {
    game: String,
}

/// Error for a game name that is not in [`KNOWN_GAMES`].
pub fn unknown_game(name: &str) -> CorpusError {
    CorpusError::Config(format!("unknown game `{name}`; known games: {}", KNOWN_GAMES.join(", ")))
}

/// The `game` field of the TOML config at `path`; a read failure is `IoError::Io`, a parse
/// failure `IoError::Toml`, both naming `path`.
pub fn game_of_toml_file(path: &Path) -> Result<String, CorpusError> {
    let text = std::fs::read_to_string(path).map_err(|source| IoError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let header: GameName = toml::from_str(&text).map_err(|e| IoError::Toml {
        path: path.to_path_buf(),
        message: e.to_string(),
    })?;
    Ok(header.game)
}

/// The `game` field of `<dir>/run.json`; an absent manifest is `IoError::Missing`.
pub fn game_of_run_dir(dir: &Path) -> Result<String, CorpusError> {
    let run_path = dir.join(RUN_FILE);
    if !run_path.is_file() {
        return Err(IoError::Missing { path: run_path }.into());
    }
    let header: GameName = read_json(&run_path)?;
    Ok(header.game)
}

/// Resolves `$name` (a `&str`) to its game bundle and evaluates `$body` with `$bundle`
/// bound to `&GameBundle<G>` for the concrete `G`. `$body` must evaluate to a
/// `Result<T, E>` with `E: From<CorpusError>`; an unknown name evaluates to
/// `Err(unknown_game(name).into())`.
macro_rules! dispatch_game {
    ($name:expr, |$bundle:ident| $body:expr) => {
        match $name {
            "tictactoe" => {
                let $bundle = &$crate::games::tictactoe::game_bundle();
                $body
            }
            other => Err($crate::cli::games::unknown_game(other).into()),
        }
    };
}
pub(crate) use dispatch_game;
