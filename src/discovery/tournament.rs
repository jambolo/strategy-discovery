//! Tournament harness: plays a strategy under test against every roster entry from every seat
//! and tallies wins, draws, losses and unfinished games. Game-agnostic.
//!
//! For each roster entry (in roster order) and each seat the strategy under test can occupy, one
//! batch of games is played with the under-test strategy in that seat and the opponent in every
//! other seat. Batch seeds are derived from a single master seed via [`cell_seed`] so a
//! tournament is fully reproducible, and results are classified against `bundle.players[seat]`
//! (never a raw engine `PlayerId`) so an aborted game (`outcome: None`) is always tallied as
//! unfinished, never as a loss.

use crate::core::traits::{MatchConfig, MatchEngine, StrategyProvider};
use crate::discovery::bundle::GameBundle;
use crate::discovery::config::{CorpusError, cell_seed};
use crate::discovery::match_engine::RayonMatchEngine;
use crate::io::schema::{OpponentResult, PairingResult, Tally, TournamentResult};
use crate::strategy::engine::EngineGame;
use crate::strategy::registry::StrategyRegistry;
use crate::strategy::roster::{Roster, RosterEntry};

/// Knobs for one tournament of a strategy under test against a roster.
#[derive(Debug, Clone, PartialEq)]
pub struct TournamentConfig {
    /// Games played per (opponent, seat) pairing.
    pub games_per_pairing: usize,
    /// Master seed; pairing `i` plays with batch seed `cell_seed(seed, i)`.
    pub seed: u64,
    /// Ply cap per game (`outcome: None` when hit); `None` = unlimited.
    pub max_plies: Option<usize>,
    /// Roster entry name the headline loss rate is measured against; must name a roster entry.
    pub reference: String,
    /// Private rayon pool size; `None` = the global pool.
    pub threads: Option<usize>,
    /// Play every game serially (takes precedence over `threads`).
    pub serial: bool,
}

/// A tournament result plus the runtime-only ply count the benches need.
#[derive(Debug, Clone, PartialEq)]
pub struct TournamentRun {
    /// The serializable result.
    pub result: TournamentResult,
    /// Total plies played over every game of every pairing.
    pub plies: usize,
}

/// Name of the last entry in `roster`, used as the default [`TournamentConfig::reference`].
/// An empty roster has no entry to reference.
pub fn default_reference(roster: &Roster) -> Result<String, CorpusError> {
    roster
        .entries
        .last()
        .map(|entry| entry.name.clone())
        .ok_or_else(|| CorpusError::Config("roster has no entries".to_string()))
}

/// Runs a tournament of `under_test` against every entry of `roster`, returning both the
/// serializable [`TournamentResult`] and the runtime-only ply count.
///
/// Validates before playing any game: `roster.validate()`, that `config.reference` names a
/// `roster` entry, and that every strategy (under test, then each roster entry in roster order)
/// builds through `registry`. Then, for every roster entry (`opponent_index`, roster order) and
/// every seat in `0..bundle.players.len()`, plays one batch with the under-test strategy in that
/// seat and the opponent in every other seat, seeded by `cell_seed(config.seed, opponent_index *
/// seats + seat)`. A game whose `outcome` is `None` (aborted by `max_plies`) is tallied as
/// unfinished; otherwise a draw (`G::winner` returns `None`), a win (the winner equals
/// `bundle.players[seat]`), or a loss (any other winner). Every tally is built via
/// [`Tally::from_counts`].
pub fn run_tournament_detailed<G: EngineGame>(
    bundle: &GameBundle<G>,
    registry: &StrategyRegistry<G>,
    under_test: &RosterEntry,
    roster: &Roster,
    config: &TournamentConfig,
) -> Result<TournamentRun, CorpusError> {
    roster.validate()?;

    if !roster.entries.iter().any(|entry| entry.name == config.reference) {
        let names: Vec<&str> = roster.entries.iter().map(|entry| entry.name.as_str()).collect();
        return Err(CorpusError::Config(format!(
            "unknown reference `{}`; roster `{}` entries: {}",
            config.reference,
            roster.id(),
            names.join(", ")
        )));
    }

    let under_test_provider = registry.build(&under_test.spec)?;
    let mut opponent_providers: Vec<Box<dyn StrategyProvider<G>>> = Vec::with_capacity(roster.entries.len());
    for entry in &roster.entries {
        opponent_providers.push(registry.build(&entry.spec)?);
    }

    let engine: Box<dyn MatchEngine<G>> = if config.serial {
        Box::new(RayonMatchEngine::serial(bundle.rules.clone()))
    } else if let Some(n) = config.threads {
        Box::new(RayonMatchEngine::with_threads(bundle.rules.clone(), n))
    } else {
        Box::new(RayonMatchEngine::new(bundle.rules.clone()))
    };

    let seats = bundle.players.len();
    let mut opponents = Vec::with_capacity(roster.entries.len());
    let mut plies = 0usize;
    let mut totals_wins = 0usize;
    let mut totals_draws = 0usize;
    let mut totals_losses = 0usize;
    let mut totals_unfinished = 0usize;

    for (opponent_index, entry) in roster.entries.iter().enumerate() {
        let opponent_provider = opponent_providers[opponent_index].as_ref();

        let mut opponent_wins = 0usize;
        let mut opponent_draws = 0usize;
        let mut opponent_losses = 0usize;
        let mut opponent_unfinished = 0usize;
        let mut by_seat = Vec::with_capacity(seats);

        for seat in 0..seats {
            let pairing_index = opponent_index * seats + seat;
            let batch_seed = cell_seed(config.seed, pairing_index);

            let mut players: Vec<(G::Player, &dyn StrategyProvider<G>)> = Vec::with_capacity(seats);
            for slot in 0..seats {
                let provider: &dyn StrategyProvider<G> = if slot == seat {
                    under_test_provider.as_ref()
                } else {
                    opponent_provider
                };
                players.push((bundle.players[slot], provider));
            }

            tracing::debug!(opponent = %entry.name, seat, "playing tournament pairing");

            let match_config = MatchConfig {
                games: config.games_per_pairing,
                seed: batch_seed,
                max_plies: config.max_plies,
            };
            let records = engine.run(&match_config, &players)?;

            let mut wins = 0usize;
            let mut draws = 0usize;
            let mut losses = 0usize;
            let mut unfinished = 0usize;
            for record in &records {
                plies += record.actions.len();
                match &record.outcome {
                    None => unfinished += 1,
                    Some(outcome) => match G::winner(outcome) {
                        None => draws += 1,
                        Some(winner) if winner == bundle.players[seat] => wins += 1,
                        Some(_) => losses += 1,
                    },
                }
            }

            opponent_wins += wins;
            opponent_draws += draws;
            opponent_losses += losses;
            opponent_unfinished += unfinished;

            by_seat.push(PairingResult {
                opponent: entry.name.clone(),
                seat,
                player: format!("{:?}", bundle.players[seat]),
                tally: Tally::from_counts(wins, draws, losses, unfinished),
            });
        }

        totals_wins += opponent_wins;
        totals_draws += opponent_draws;
        totals_losses += opponent_losses;
        totals_unfinished += opponent_unfinished;

        opponents.push(OpponentResult {
            opponent: entry.name.clone(),
            tally: Tally::from_counts(opponent_wins, opponent_draws, opponent_losses, opponent_unfinished),
            by_seat,
        });
    }

    let result = TournamentResult {
        roster_id: roster.id(),
        reference: config.reference.clone(),
        games_per_pairing: config.games_per_pairing,
        seed: config.seed,
        opponents,
        totals: Tally::from_counts(totals_wins, totals_draws, totals_losses, totals_unfinished),
    };

    Ok(TournamentRun { result, plies })
}

/// [`run_tournament_detailed`], discarding the runtime-only ply count.
pub fn run_tournament<G: EngineGame>(
    bundle: &GameBundle<G>,
    registry: &StrategyRegistry<G>,
    under_test: &RosterEntry,
    roster: &Roster,
    config: &TournamentConfig,
) -> Result<TournamentResult, CorpusError> {
    run_tournament_detailed(bundle, registry, under_test, roster, config).map(|run| run.result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::dsl::HeuristicStrategy;
    use crate::games::tictactoe::game_bundle;
    use crate::strategy::registry::StrategySpec;

    fn tiny_roster() -> Roster {
        toml::from_str(include_str!("../../tests/fixtures/roster-tiny.toml")).unwrap()
    }

    fn registry() -> StrategyRegistry<crate::games::tictactoe::TicTacToe> {
        StrategyRegistry::new(game_bundle().engine_bundle("default").unwrap())
    }

    fn config(games_per_pairing: usize, seed: u64) -> TournamentConfig {
        TournamentConfig {
            games_per_pairing,
            seed,
            max_plies: None,
            reference: "perfect".to_string(),
            threads: None,
            serial: true,
        }
    }

    #[test]
    fn perfect_never_loses_to_tiny_roster() {
        let bundle = game_bundle();
        let registry = registry();
        let roster = tiny_roster();
        let under_test = roster.get("perfect").unwrap().clone();
        let cfg = config(2, 0);

        let result = run_tournament(&bundle, &registry, &under_test, &roster, &cfg).unwrap();

        assert_eq!(result.roster_id, "tiny-v1");
        assert_eq!(result.reference, "perfect");
        assert_eq!(result.games_per_pairing, 2);
        assert_eq!(result.seed, 0);

        let names: Vec<&str> = result.opponents.iter().map(|o| o.opponent.as_str()).collect();
        assert_eq!(names, vec!["random", "depth-2", "perfect"]);

        for opponent in &result.opponents {
            assert_eq!(opponent.by_seat.len(), 2);
            let seats: Vec<usize> = opponent.by_seat.iter().map(|p| p.seat).collect();
            assert_eq!(seats, vec![0, 1]);
            let players: Vec<&str> = opponent.by_seat.iter().map(|p| p.player.as_str()).collect();
            assert_eq!(players, vec!["X", "O"]);

            for pairing in &opponent.by_seat {
                assert_eq!(pairing.tally.games, 2);
                assert_eq!(pairing.tally.losses, 0);
                assert_eq!(pairing.tally.loss_rate, 0.0);
            }

            assert_eq!(opponent.tally.games, 4);
            assert_eq!(opponent.tally.losses, 0);
        }

        assert_eq!(result.totals.games, 12);
        assert_eq!(result.totals.losses, 0);
        assert_eq!(result.totals.loss_rate, 0.0);
    }

    #[test]
    fn random_loses_to_perfect_with_pinned_seed() {
        let bundle = game_bundle();
        let registry = registry();
        let roster = tiny_roster();
        let under_test = roster.get("random").unwrap().clone();
        let cfg = config(2, 20260822);

        let result = run_tournament(&bundle, &registry, &under_test, &roster, &cfg).unwrap();

        let perfect = result.opponents.iter().find(|o| o.opponent == "perfect").unwrap();
        assert_eq!(perfect.tally.games, 4);
        assert_eq!(perfect.tally.wins, 0);
        assert_eq!(perfect.tally.draws, 1);
        assert_eq!(perfect.tally.losses, 3);
        assert_eq!(perfect.tally.loss_rate, 0.75);

        assert!(result.totals.losses >= 3);
    }

    #[test]
    fn serial_threads_and_pool_agree() {
        let bundle = game_bundle();
        let registry = registry();
        let roster = tiny_roster();
        let under_test = roster.get("depth-2").unwrap().clone();

        let serial_cfg = config(2, 1);
        let threads_cfg = TournamentConfig {
            serial: false,
            threads: Some(2),
            ..config(2, 1)
        };
        let pool_cfg = TournamentConfig {
            serial: false,
            threads: None,
            ..config(2, 1)
        };

        let serial = run_tournament(&bundle, &registry, &under_test, &roster, &serial_cfg).unwrap();
        let threads = run_tournament(&bundle, &registry, &under_test, &roster, &threads_cfg).unwrap();
        let pool = run_tournament(&bundle, &registry, &under_test, &roster, &pool_cfg).unwrap();

        assert_eq!(serial, threads);
        assert_eq!(serial, pool);
    }

    #[test]
    fn heuristic_rules_under_test_plays_and_is_scored() {
        let bundle = game_bundle();
        let registry = registry();
        let roster = tiny_roster();
        let under_test = RosterEntry {
            name: "empty".to_string(),
            spec: StrategySpec::HeuristicRules {
                heuristic: HeuristicStrategy::new("empty"),
            },
        };
        let cfg = config(1, 0);

        let result = run_tournament(&bundle, &registry, &under_test, &roster, &cfg).unwrap();

        assert_eq!(result.reference, "perfect");
        assert_eq!(result.opponents.len(), 3);
        assert_eq!(result.totals.games, 6);
        assert_eq!(result.totals.unfinished, 0);
    }

    #[test]
    fn unknown_reference_is_a_config_error() {
        let bundle = game_bundle();
        let registry = registry();
        let roster = tiny_roster();
        let under_test = roster.get("perfect").unwrap().clone();
        let cfg = TournamentConfig {
            reference: "nope".to_string(),
            ..config(2, 0)
        };

        let err = run_tournament(&bundle, &registry, &under_test, &roster, &cfg).unwrap_err();
        assert!(matches!(err, CorpusError::Config(msg) if msg.contains("unknown reference `nope`")));
    }

    #[test]
    fn default_reference_is_the_last_entry() {
        let roster = tiny_roster();
        assert_eq!(default_reference(&roster).unwrap(), "perfect");

        let empty = Roster {
            name: "empty".to_string(),
            version: 1,
            entries: vec![],
        };
        assert!(matches!(default_reference(&empty), Err(CorpusError::Config(_))));
    }

    #[test]
    fn unfinished_games_are_never_losses() {
        let bundle = game_bundle();
        let registry = registry();
        let roster = tiny_roster();
        let under_test = roster.get("perfect").unwrap().clone();
        let cfg = TournamentConfig {
            max_plies: Some(1),
            ..config(1, 0)
        };

        let run = run_tournament_detailed(&bundle, &registry, &under_test, &roster, &cfg).unwrap();

        for opponent in &run.result.opponents {
            for pairing in &opponent.by_seat {
                assert_eq!(pairing.tally.unfinished, 1);
                assert_eq!(pairing.tally.losses, 0);
            }
        }
        assert_eq!(run.result.totals.unfinished, 6);
        assert_eq!(run.result.totals.games, 6);
        assert_eq!(run.plies, 6);
    }
}
