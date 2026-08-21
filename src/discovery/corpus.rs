//! Corpus generation runner: plays every sweep cell and streams `games.jsonl`,
//! `positions.jsonl` and `run.json` into a run directory.
//!
//! `generate` resolves a [`GenerateConfig`] into cells (strategy pairing x evaluator x opening x
//! random-opening-ply count), plays every cell's games through a [`RayonMatchEngine`], and
//! expands each played game into one [`GameRecord`] plus one [`PositionRecord`] per ply via
//! [`expand_game`]. Same resolved config and seed produce byte-identical output regardless of
//! thread count, so nothing environment-dependent (timestamps, paths, thread counts) may reach
//! an output file, and every map/list is built in a defined order.

use crate::core::traits::{MatchConfig, MatchEngine, MatchRecord};
use crate::discovery::bundle::GameBundle;
use crate::discovery::config::{Cell, CorpusError, GenerateConfig, resolve};
use crate::discovery::match_engine::{RayonMatchEngine, opening_plies_played};
use crate::io::hash::config_hash;
use crate::io::jsonl::{JsonlWriter, write_json_pretty};
use crate::io::replay::{final_state, replay};
use crate::io::schema::{CellKey, CorpusGame, GAMES_FILE, GameRecord, POSITIONS_FILE, PositionRecord, RUN_FILE, SCHEMA_VERSION};
use crate::strategy::engine::EngineGame;
use crate::strategy::registry::StrategyRegistry;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// How a sweep is executed. Defaults to parallel play on rayon's global pool.
#[derive(Debug, Clone, Default)]
pub struct GenerateOptions {
    /// Run each cell's games on a private pool of this many threads.
    pub threads: Option<usize>,
    /// Run each cell's games serially; takes precedence over `threads`.
    pub serial: bool,
}

/// Manifest of one generation run, written to `run.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RunMetadata<A, P> {
    /// Schema version this manifest was written under.
    pub schema_version: u32,
    /// Identifier of this run, equal to `config_hash`.
    pub run_id: String,
    /// Content hash of the resolved config.
    pub config_hash: String,
    /// Name of the crate that produced this run.
    pub crate_name: String,
    /// Version of the crate that produced this run.
    pub crate_version: String,
    /// Game name this run was generated for.
    pub game: String,
    /// Players in turn order.
    pub players: Vec<P>,
    /// The resolved config this run was generated from.
    pub config: GenerateConfig<A>,
    /// Every cell enumerated from `config`, in sweep order.
    pub cells: Vec<CellKey>,
    /// Total number of games written across all cells.
    pub games: usize,
    /// Total number of positions written across all cells.
    pub positions: usize,
}

/// Resolves `config`, plays every enumerated cell, and streams `games.jsonl`, `positions.jsonl`
/// and `run.json` into `out_dir`. Cells are processed sequentially and in order; parallelism (if
/// any) lives entirely inside each cell's match engine.
pub fn generate<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    config: GenerateConfig<G::Action>,
    options: &GenerateOptions,
    out_dir: &Path,
) -> Result<RunMetadata<G::Action, G::Player>, CorpusError> {
    let resolved = resolve(config, bundle)?;
    let run_id = config_hash(&resolved.config)?;

    std::fs::create_dir_all(out_dir).map_err(|source| crate::io::IoError::Io {
        path: out_dir.to_path_buf(),
        source,
    })?;

    let mut games_writer = JsonlWriter::create(&out_dir.join(GAMES_FILE))?;
    let mut positions_writer = JsonlWriter::create(&out_dir.join(POSITIONS_FILE))?;

    let mut registries: BTreeMap<String, StrategyRegistry<G>> = BTreeMap::new();

    for cell in &resolved.cells {
        if !registries.contains_key(&cell.key.evaluator) {
            let engine_bundle = bundle.engine_bundle(&cell.key.evaluator)?;
            registries.insert(cell.key.evaluator.clone(), StrategyRegistry::new(engine_bundle));
        }
        let registry = registries.get(&cell.key.evaluator).expect("just inserted above");

        let providers: Vec<Box<dyn crate::core::traits::StrategyProvider<G>>> = cell
            .entries
            .iter()
            .map(|e| registry.build(&e.spec))
            .collect::<Result<_, _>>()?;
        let players: Vec<(G::Player, &dyn crate::core::traits::StrategyProvider<G>)> = bundle
            .players
            .iter()
            .copied()
            .zip(providers.iter().map(|p| p.as_ref()))
            .collect();

        let engine = if options.serial {
            RayonMatchEngine::serial(bundle.rules.clone())
        } else if let Some(n) = options.threads {
            RayonMatchEngine::with_threads(bundle.rules.clone(), n)
        } else {
            RayonMatchEngine::new(bundle.rules.clone())
        }
        .with_opening(cell.opening.clone());

        let records = engine.run(
            &MatchConfig {
                games: resolved.config.games_per_cell,
                seed: cell.seed,
                max_plies: resolved.config.max_plies,
            },
            &players,
        )?;

        for record in &records {
            let (game, positions) = expand_game(bundle, &run_id, cell, record)?;
            games_writer.write(&game)?;
            for position in &positions {
                positions_writer.write(position)?;
            }
        }
    }

    let games = games_writer.finish()?;
    let positions = positions_writer.finish()?;

    let metadata = RunMetadata {
        schema_version: SCHEMA_VERSION,
        run_id: run_id.clone(),
        config_hash: run_id,
        crate_name: crate::NAME.to_string(),
        crate_version: crate::VERSION.to_string(),
        game: bundle.name.clone(),
        players: bundle.players.clone(),
        config: resolved.config,
        cells: resolved.cells.iter().map(|c| c.key.clone()).collect(),
        games,
        positions,
    };

    write_json_pretty(&out_dir.join(RUN_FILE), &metadata)?;

    Ok(metadata)
}

/// Expands one played [`MatchRecord`] into its [`GameRecord`] and per-ply [`PositionRecord`]s.
#[allow(clippy::type_complexity)]
pub fn expand_game<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    run_id: &str,
    cell: &Cell<G::Action>,
    record: &MatchRecord<G::Action, G::Outcome>,
) -> Result<
    (
        GameRecord<G::State, G::Action, G::Player, G::Outcome>,
        Vec<PositionRecord<G::State, G::Action, G::Player, G::Outcome>>,
    ),
    CorpusError,
> {
    let rules = bundle.rules.as_ref();
    let pairs = replay(rules, &record.actions)?;
    let last_state = final_state(rules, &record.actions)?;
    let (final_canonical_state, _) = bundle.canonicalize(&last_state);
    let opening_plies = opening_plies_played(record, &cell.opening);
    let game_id = format!("{}:{}", cell.key.index, record.game_index);

    let game = GameRecord {
        schema_version: SCHEMA_VERSION,
        run_id: run_id.to_string(),
        game_id: game_id.clone(),
        cell: cell.key.clone(),
        game_index: record.game_index,
        seed: record.seed,
        players: bundle.players.clone(),
        specs: cell.entries.iter().map(|e| e.spec.clone()).collect(),
        opening_plies,
        actions: record.actions.clone(),
        outcome: record.outcome.clone(),
        length: record.actions.len(),
        final_state: last_state,
        final_canonical_state,
    };

    let mut positions = Vec::with_capacity(pairs.len());
    for (i, (state, action)) in pairs.iter().enumerate() {
        let side_to_move = rules.player_to_move(state);
        let (canonical_state, canonical_transform) = bundle.canonicalize(state);
        let legal_actions = rules.legal_actions(state);

        let chosen_by = if i < cell.opening.actions.len() {
            "opening".to_string()
        } else if i < opening_plies {
            "random-opening".to_string()
        } else {
            let slot = bundle.player_slot(side_to_move).ok_or_else(|| {
                CorpusError::Config(format!(
                    "side to move is not one of the game's players at ply {i} of game {game_id}"
                ))
            })?;
            cell.entries[slot].name.clone()
        };

        positions.push(PositionRecord {
            schema_version: SCHEMA_VERSION,
            game_id: game_id.clone(),
            ply: i,
            side_to_move,
            state: state.clone(),
            canonical_state,
            canonical_transform,
            legal_actions,
            chosen_action: action.clone(),
            chosen_by,
            outcome: record.outcome.clone(),
            game_length: record.actions.len(),
        });
    }

    Ok((game, positions))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::match_engine::Opening;
    use crate::games::tictactoe::{Board, Move, Outcome, Player, game_bundle};
    use crate::io::jsonl::read_jsonl;

    const CONFIG: &str = r#"
schema_version = 1
game = "tictactoe"
seed = 7
games_per_cell = 2
pairings = [["random", "random"], ["random", "depth-1"], ["depth-1", "random"]]

[[strategies]]
name = "random"
[strategies.spec]
kind = "random"

[[strategies]]
name = "depth-1"
[strategies.spec]
kind = "minimax"
depth = 1
"#;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("sd-corpus-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn generate_writes_the_three_files() {
        let dir = temp_dir("writes-three-files");
        let bundle = game_bundle();
        let config = GenerateConfig::<Move>::from_toml_str(CONFIG).unwrap();

        let metadata = generate(&bundle, config, &GenerateOptions::default(), &dir).unwrap();

        assert!(dir.join(GAMES_FILE).is_file());
        assert!(dir.join(POSITIONS_FILE).is_file());
        assert!(dir.join(RUN_FILE).is_file());

        assert_eq!(metadata.games, 6);
        assert_eq!(metadata.cells.len(), 3);
        assert_eq!(metadata.run_id, metadata.config_hash);
        assert_eq!(metadata.crate_name, crate::NAME);

        let games: Vec<GameRecord<Board, Move, Player, Outcome>> = read_jsonl(&dir.join(GAMES_FILE)).unwrap();
        assert_eq!(games.len(), 6);
        let total_length: usize = games.iter().map(|g| g.length).sum();
        assert_eq!(metadata.positions, total_length);
    }

    #[test]
    fn same_seed_produces_identical_bytes() {
        let dir_a = temp_dir("identical-bytes-a");
        let dir_b = temp_dir("identical-bytes-b");
        let bundle = game_bundle();

        let config_a = GenerateConfig::<Move>::from_toml_str(CONFIG).unwrap();
        let config_b = GenerateConfig::<Move>::from_toml_str(CONFIG).unwrap();

        generate(&bundle, config_a, &GenerateOptions::default(), &dir_a).unwrap();
        generate(&bundle, config_b, &GenerateOptions::default(), &dir_b).unwrap();

        for file in [GAMES_FILE, POSITIONS_FILE, RUN_FILE] {
            assert_eq!(
                std::fs::read(dir_a.join(file)).unwrap(),
                std::fs::read(dir_b.join(file)).unwrap(),
                "{file} differs"
            );
        }
    }

    #[test]
    fn expand_game_marks_opening_plies() {
        let bundle = game_bundle();
        let config_text = r#"
schema_version = 1
game = "tictactoe"
seed = 3
games_per_cell = 1
pairings = [["random", "random"]]
random_opening_plies = [1]

[[strategies]]
name = "random"
[strategies.spec]
kind = "random"

[[openings]]
name = "center"
actions = [4]
"#;
        let config = GenerateConfig::<Move>::from_toml_str(config_text).unwrap();
        let resolved = resolve(config, &bundle).unwrap();
        let cell: &Cell<Move> = &resolved.cells[0];
        assert_eq!(
            cell.opening,
            Opening {
                actions: vec![Move(4)],
                random_plies: 1
            }
        );

        let engine = RayonMatchEngine::serial(bundle.rules.clone()).with_opening(cell.opening.clone());

        let registry = StrategyRegistry::new(bundle.engine_bundle(&cell.key.evaluator).unwrap());
        let providers: Vec<Box<dyn crate::core::traits::StrategyProvider<_>>> =
            cell.entries.iter().map(|e| registry.build(&e.spec).unwrap()).collect();
        let players: Vec<(Player, &dyn crate::core::traits::StrategyProvider<_>)> = bundle
            .players
            .iter()
            .copied()
            .zip(providers.iter().map(|p| p.as_ref()))
            .collect();

        let record = engine
            .run(
                &MatchConfig {
                    games: 1,
                    seed: cell.seed,
                    max_plies: None,
                },
                &players,
            )
            .unwrap()
            .remove(0);

        let (game, positions) = expand_game(&bundle, "run", cell, &record).unwrap();

        assert_eq!(positions[0].chosen_by, "opening");
        assert_eq!(positions[0].chosen_action, Move(4));
        assert_eq!(positions[1].chosen_by, "random-opening");
        let slot = bundle.player_slot(positions[2].side_to_move).unwrap();
        assert_eq!(positions[2].chosen_by, cell.entries[slot].name);
        assert_eq!(game.opening_plies, 2);
    }
}
