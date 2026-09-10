//! `play` subcommand: play games between named strategies and print a transcript, optionally
//! also writing a corpus run directory byte-identical to `generate` on the equivalent sweep
//! TOML (same `resolve` -> `config_hash` -> `RunWriter` -> `play_sweep` -> `finish` composition).

use crate::cli::games::dispatch_game;
use crate::discovery::corpus::{RunWriter, play_sweep};
use crate::discovery::{CorpusError, GameBundle, GenerateConfig, GenerateOptions, NamedOpening, Pairing, StrategyFile, resolve};
use crate::io::CorpusGame;
use crate::io::hash::config_hash;
use crate::io::replay::replay;
use crate::strategy::engine::EngineGame;
use crate::strategy::roster::RosterEntry;
use std::path::PathBuf;

/// Flags for `play`.
#[derive(clap::Args, Debug)]
pub struct PlayArgs {
    /// Game to play.
    #[arg(long)]
    pub game: String,
    /// Comma-separated strategy entry names, one per player, in turn order.
    #[arg(long)]
    pub players: String,
    /// Number of games to play.
    #[arg(long, default_value_t = 1)]
    pub games: usize,
    /// Master seed for the run.
    #[arg(long, default_value_t = 0)]
    pub seed: u64,
    /// Forced opening actions as a JSON array, e.g. `[4,0]`.
    #[arg(long)]
    pub opening: Option<String>,
    /// Random plies played after the forced opening.
    #[arg(long, default_value_t = 0)]
    pub random_opening_plies: u32,
    /// Abort a game after this many plies.
    #[arg(long)]
    pub max_plies: Option<usize>,
    /// Evaluator name; defaults to the game bundle's default evaluator.
    #[arg(long)]
    pub evaluator: Option<String>,
    /// TOML strategy roster file; defaults to the game's built-in roster.
    #[arg(long)]
    pub strategies: Option<PathBuf>,
    /// Also write a corpus run directory (`run.json`, `games.jsonl`, `positions.jsonl`).
    #[arg(long)]
    pub out: Option<PathBuf>,
    /// Run each game serially; takes precedence over `--threads`.
    #[arg(long)]
    pub serial: bool,
    /// Run games on a private pool of this many threads.
    #[arg(long)]
    pub threads: Option<usize>,
}

/// Runs `play`.
pub(super) fn run(args: PlayArgs) -> anyhow::Result<()> {
    let game = args.game.clone();
    dispatch_game!(game.as_str(), |bundle| play_for(bundle, &args))
}

/// Plays `args.games` games of one strategy pairing for `bundle`, printing a transcript and
/// optionally streaming a corpus run directory through the same path `generate` uses.
fn play_for<G>(bundle: &GameBundle<G>, args: &PlayArgs) -> anyhow::Result<()>
where
    G: EngineGame + CorpusGame,
    G::State: std::fmt::Display,
{
    let roster: Vec<RosterEntry> = match &args.strategies {
        Some(path) => StrategyFile::from_toml_file(path)?.strategies,
        None => bundle.default_strategies.clone(),
    };

    let names: Vec<String> = args.players.split(',').map(|s| s.trim().to_string()).collect();
    if names.is_empty() || names.len() != bundle.players.len() {
        return Err(CorpusError::Config(format!(
            "--players must name {} strategies (one per player), got {}: {}",
            bundle.players.len(),
            names.len(),
            args.players
        ))
        .into());
    }

    let available: Vec<String> = roster.iter().map(|entry| entry.name.clone()).collect();
    let mut named_entries: Vec<RosterEntry> = Vec::with_capacity(names.len());
    for name in &names {
        let entry = roster.iter().find(|entry| &entry.name == name).ok_or_else(|| {
            CorpusError::Config(format!(
                "unknown strategy `{name}`; available strategies: {}",
                available.join(", ")
            ))
        })?;
        named_entries.push(entry.clone());
    }

    let mut strategies: Vec<RosterEntry> = Vec::with_capacity(named_entries.len());
    for entry in &named_entries {
        if !strategies.iter().any(|seen: &RosterEntry| seen.name == entry.name) {
            strategies.push(entry.clone());
        }
    }

    let evaluators = vec![args.evaluator.clone().unwrap_or_else(|| bundle.default_evaluator.clone())];

    let opening_actions: Vec<G::Action> = match &args.opening {
        Some(text) => {
            let actions: Vec<G::Action> = serde_json::from_str(text)
                .map_err(|e| CorpusError::Config(format!("--opening `{text}` is not a JSON array of actions: {e}")))?;
            replay(bundle.rules.as_ref(), &actions)
                .map_err(|e| CorpusError::Config(format!("--opening `{text}` is not a legal action sequence: {e}")))?;
            actions
        }
        None => Vec::new(),
    };
    let openings = vec![NamedOpening {
        name: "play".to_string(),
        actions: opening_actions,
    }];

    let config = GenerateConfig::<G::Action> {
        schema_version: crate::io::SCHEMA_VERSION,
        game: bundle.name.clone(),
        seed: args.seed,
        games_per_cell: args.games,
        max_plies: args.max_plies,
        strategies,
        pairings: vec![Pairing(names.clone())],
        evaluators,
        openings,
        random_opening_plies: vec![args.random_opening_plies],
    };

    let resolved = resolve(config, bundle)?;
    let run_id = config_hash(&resolved.config)?;
    let options = GenerateOptions {
        threads: args.threads,
        serial: args.serial,
    };

    let mut writer = match &args.out {
        Some(dir) => Some(RunWriter::create(dir)?),
        None => None,
    };

    let mut outcomes: Vec<Option<G::Outcome>> = Vec::new();

    play_sweep(bundle, &resolved, &run_id, &options, |_cell, game, positions| {
        if let Some(w) = writer.as_mut() {
            w.write_game(&game, &positions)?;
        }

        println!(
            "game {} seed={:#018x} players={} opening_plies={}",
            game.game_id,
            game.seed,
            game.cell.strategies.join(","),
            game.opening_plies
        );
        for position in &positions {
            let action = serde_json::to_string(&position.chosen_action)
                .map_err(|e| CorpusError::Config(format!("action at ply {} is not serializable: {e}", position.ply)))?;
            let opening = if position.chosen_by == "opening" || position.chosen_by == "random-opening" {
                " (opening)"
            } else {
                ""
            };
            println!("{:>2}. {:?} {}{}", position.ply, position.side_to_move, action, opening);
        }
        for line in game.final_state.to_string().lines() {
            println!("  {line}");
        }
        let outcome = game.outcome.as_ref().map_or_else(|| "none".to_string(), |o| format!("{o:?}"));
        println!("outcome={} length={}", outcome, game.length);
        println!();

        outcomes.push(game.outcome);
        Ok(())
    })?;

    if let Some(w) = writer {
        w.finish(bundle, resolved, run_id)?;
    }

    let draws = outcomes
        .iter()
        .filter(|outcome| matches!(outcome, Some(o) if G::winner(o).is_none()))
        .count();
    let unfinished = outcomes.iter().filter(|outcome| outcome.is_none()).count();
    let wins = bundle
        .players
        .iter()
        .map(|player| {
            let count = outcomes
                .iter()
                .filter(|outcome| matches!(outcome, Some(o) if G::winner(o) == Some(*player)))
                .count();
            format!("{player:?}:{count}")
        })
        .collect::<Vec<_>>()
        .join(",");

    println!(
        "games={} wins={} draws={} unfinished={}",
        outcomes.len(),
        wins,
        draws,
        unfinished
    );
    tracing::info!(games = outcomes.len(), "played");

    Ok(())
}
