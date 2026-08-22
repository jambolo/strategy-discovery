//! `generate` subcommand: a sweep config TOML -> a corpus run directory.

use crate::cli::games::{dispatch_game, game_of_toml_file};
use crate::discovery::{GameBundle, GenerateConfig, GenerateOptions, generate};
use crate::io::CorpusGame;
use crate::strategy::engine::EngineGame;
use std::path::Path;

/// Runs `generate --config <config_path> --out <out> [--threads N] [--serial]`.
pub(super) fn run(config_path: &Path, out: &Path, threads: Option<usize>, serial: bool) -> anyhow::Result<()> {
    let game = game_of_toml_file(config_path)?;
    let options = GenerateOptions { threads, serial };
    dispatch_game!(game.as_str(), |bundle| generate_for(bundle, config_path, &options, out))
}

/// Generates a corpus for one concrete game and prints the result line.
fn generate_for<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    config_path: &Path,
    options: &GenerateOptions,
    out: &Path,
) -> anyhow::Result<()> {
    let config = GenerateConfig::<G::Action>::from_toml_file(config_path)?;
    let metadata = generate(bundle, config, options, out)?;
    println!(
        "run_id={} cells={} games={} positions={} out={}",
        metadata.run_id,
        metadata.cells.len(),
        metadata.games,
        metadata.positions,
        out.display()
    );
    Ok(())
}
