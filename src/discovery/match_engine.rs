//! Rayon-backed `MatchEngine`: plays batches of games in game-index order, serially or in
//! parallel, with per-game and per-player seeds deterministically derived from a master seed
//! (ADR 0002). Game-agnostic: nothing here knows about any specific game.

use crate::core::traits::{
    GameDomain, GameRules, MatchConfig, MatchEngine, MatchError, MatchRecord, StrategyError, StrategyProvider,
};
use rayon::prelude::*;
use std::sync::Arc;

/// SplitMix64 mixing function: deterministic, well-distributed 64-bit hash used to derive
/// per-game and per-player seeds from a master seed. Reference values: `splitmix64(0) ==
/// 0xE220A8397B1DCDAF`, `splitmix64(1) == 0x910A2DEC89025CC1`.
pub fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E3779B97F4A7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Per-game seed derived from a batch's master seed and the game's index within the batch.
/// Reference values: `game_seed(42, 0) == 0xBDD732262FEB6E95`, `game_seed(42, 1) ==
/// 0x28EFE333B266F103`, `game_seed(42, 2) == 0x5FD30D2FCBEF75E3`.
pub fn game_seed(master: u64, game_index: usize) -> u64 {
    splitmix64(master ^ (game_index as u64).wrapping_mul(0x9E3779B97F4A7C15))
}

/// Per-player seed derived from a game's seed and the player's slot (index into the `players`
/// list passed to [`MatchEngine::run`]). Reference values: `player_seed(0, 0) ==
/// 0x2D0F28C7E7E786B2`, `player_seed(0x28EFE333B266F103, 0) == 0xED6D6CCF127CB19E`,
/// `player_seed(0x28EFE333B266F103, 1) == 0xEB03688B255D6EEE`.
pub fn player_seed(game_seed: u64, slot: usize) -> u64 {
    splitmix64(game_seed ^ (slot as u64 + 1).wrapping_mul(0xD1B54A32D192ED03))
}

/// Plays one game to completion (or to `max_plies`) and returns its record.
///
/// One strategy instance is created per entry in `players` (even entries never reached, e.g. a
/// player who never gets a turn on this game's path), seeded with `player_seed(game_seed, slot)`
/// where `slot` is the entry's index in `players`. If a player identity appears more than once in
/// `players`, the *first* matching entry is used every time it is that player's turn.
#[allow(clippy::type_complexity)]
pub fn play_game<G: GameDomain>(
    rules: &dyn GameRules<G>,
    players: &[(G::Player, &dyn StrategyProvider<G>)],
    game_index: usize,
    game_seed: u64,
    max_plies: Option<usize>,
) -> Result<MatchRecord<G::Action, G::Outcome>, MatchError> {
    let mut strategies: Vec<_> = players
        .iter()
        .enumerate()
        .map(|(slot, (_, provider))| provider.create(player_seed(game_seed, slot)))
        .collect();

    let mut state = rules.initial_state();
    let mut actions = Vec::new();
    let outcome = loop {
        let legal = rules.legal_actions(&state);
        if legal.is_empty() {
            break rules.outcome(&state);
        }
        if max_plies.is_some_and(|m| actions.len() >= m) {
            break None;
        }
        let to_move = rules.player_to_move(&state);
        let Some(slot) = players.iter().position(|(p, _)| *p == to_move) else {
            return Err(MatchError::MissingProvider(format!("{to_move:?}")));
        };
        let action = strategies[slot].choose(&state, &legal)?;
        if !legal.contains(&action) {
            return Err(MatchError::Strategy(StrategyError::Other(format!(
                "strategy `{}` returned illegal action {action:?}",
                players[slot].1.kind()
            ))));
        }
        state = rules.apply(&state, &action)?;
        actions.push(action);
    };

    Ok(MatchRecord {
        game_index,
        seed: game_seed,
        actions,
        outcome,
    })
}

/// `MatchEngine` running games over rayon: serially, on rayon's global pool, or on a private pool
/// of a fixed size. Every mode produces byte-identical `MatchRecord`s for the same config and
/// providers (ADR 0002); thread count is a performance knob only.
pub struct RayonMatchEngine<G: GameDomain> {
    rules: Arc<dyn GameRules<G>>,
    threads: Option<usize>,
    parallel: bool,
}

impl<G: GameDomain> RayonMatchEngine<G> {
    /// Parallel engine running on rayon's global thread pool.
    pub fn new(rules: Arc<dyn GameRules<G>>) -> Self {
        RayonMatchEngine {
            rules,
            threads: None,
            parallel: true,
        }
    }

    /// Parallel engine running on a private pool of `n` threads, built fresh for each call to
    /// [`MatchEngine::run`]. Panics if the pool fails to build (an environment error, not a data
    /// error).
    pub fn with_threads(rules: Arc<dyn GameRules<G>>, n: usize) -> Self {
        RayonMatchEngine {
            rules,
            threads: Some(n),
            parallel: true,
        }
    }

    /// Engine running a plain sequential loop, no rayon involved.
    pub fn serial(rules: Arc<dyn GameRules<G>>) -> Self {
        RayonMatchEngine {
            rules,
            threads: None,
            parallel: false,
        }
    }

    /// The fixed thread count this engine was configured with, if any.
    pub fn threads(&self) -> Option<usize> {
        self.threads
    }

    /// Whether this engine runs games in parallel.
    pub fn is_parallel(&self) -> bool {
        self.parallel
    }
}

impl<G: GameDomain> MatchEngine<G> for RayonMatchEngine<G> {
    fn run(
        &self,
        config: &MatchConfig,
        players: &[(G::Player, &dyn StrategyProvider<G>)],
    ) -> Result<Vec<MatchRecord<G::Action, G::Outcome>>, MatchError> {
        let rules = self.rules.as_ref();

        if !self.parallel {
            return (0..config.games)
                .map(|i| play_game(rules, players, i, game_seed(config.seed, i), config.max_plies))
                .collect();
        }

        let run_parallel = || {
            (0..config.games)
                .into_par_iter()
                .map(|i| play_game(rules, players, i, game_seed(config.seed, i), config.max_plies))
                .collect()
        };

        match self.threads {
            Some(n) => rayon::ThreadPoolBuilder::new()
                .num_threads(n)
                .build()
                .expect("rayon thread pool builds")
                .install(run_parallel),
            None => run_parallel(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::Strategy;
    use crate::games::tictactoe::{Board, Move, Outcome, Player, TicTacToe, TicTacToeRules};
    use rand::SeedableRng;
    use rand::seq::IndexedRandom;
    use rand_chacha::ChaCha8Rng;

    /// Always plays `legal[0]`.
    struct FirstLegal;
    impl Strategy<TicTacToe> for FirstLegal {
        fn choose(&mut self, _state: &Board, legal: &[Move]) -> Result<Move, StrategyError> {
            Ok(legal[0])
        }
    }
    impl StrategyProvider<TicTacToe> for FirstLegal {
        fn kind(&self) -> &str {
            "first"
        }
        fn create(&self, _seed: u64) -> Box<dyn Strategy<TicTacToe>> {
            Box::new(FirstLegal)
        }
    }

    /// Always plays the last element of `legal`.
    struct LastLegal;
    impl Strategy<TicTacToe> for LastLegal {
        fn choose(&mut self, _state: &Board, legal: &[Move]) -> Result<Move, StrategyError> {
            Ok(legal[legal.len() - 1])
        }
    }
    impl StrategyProvider<TicTacToe> for LastLegal {
        fn kind(&self) -> &str {
            "last"
        }
        fn create(&self, _seed: u64) -> Box<dyn Strategy<TicTacToe>> {
            Box::new(LastLegal)
        }
    }

    /// Plays a uniformly random legal action from a seeded RNG.
    struct SeededRandomStrategy {
        rng: ChaCha8Rng,
    }
    impl Strategy<TicTacToe> for SeededRandomStrategy {
        fn choose(&mut self, _state: &Board, legal: &[Move]) -> Result<Move, StrategyError> {
            Ok(*legal.choose(&mut self.rng).expect("legal is nonempty"))
        }
    }
    struct SeededRandom;
    impl StrategyProvider<TicTacToe> for SeededRandom {
        fn kind(&self) -> &str {
            "seeded-random"
        }
        fn create(&self, seed: u64) -> Box<dyn Strategy<TicTacToe>> {
            Box::new(SeededRandomStrategy {
                rng: ChaCha8Rng::seed_from_u64(seed),
            })
        }
    }

    /// Always returns an out-of-range action, to exercise the illegal-action error path.
    struct Illegal;
    impl Strategy<TicTacToe> for Illegal {
        fn choose(&mut self, _state: &Board, _legal: &[Move]) -> Result<Move, StrategyError> {
            Ok(Move(9))
        }
    }
    impl StrategyProvider<TicTacToe> for Illegal {
        fn kind(&self) -> &str {
            "illegal"
        }
        fn create(&self, _seed: u64) -> Box<dyn Strategy<TicTacToe>> {
            Box::new(Illegal)
        }
    }

    /// Always fails, to exercise strategy error propagation.
    struct Failing;
    impl Strategy<TicTacToe> for Failing {
        fn choose(&mut self, _state: &Board, _legal: &[Move]) -> Result<Move, StrategyError> {
            Err(StrategyError::Other("boom".into()))
        }
    }
    impl StrategyProvider<TicTacToe> for Failing {
        fn kind(&self) -> &str {
            "failing"
        }
        fn create(&self, _seed: u64) -> Box<dyn Strategy<TicTacToe>> {
            Box::new(Failing)
        }
    }

    #[test]
    fn splitmix64_reference_values() {
        assert_eq!(splitmix64(0), 0xE220A8397B1DCDAF);
        assert_eq!(splitmix64(1), 0x910A2DEC89025CC1);
    }

    #[test]
    fn game_seed_reference_values() {
        assert_eq!(game_seed(42, 0), 0xBDD732262FEB6E95);
        assert_eq!(game_seed(42, 1), 0x28EFE333B266F103);
        assert_eq!(game_seed(42, 2), 0x5FD30D2FCBEF75E3);
    }

    #[test]
    fn player_seed_reference_values() {
        assert_eq!(player_seed(0, 0), 0x2D0F28C7E7E786B2);
        assert_eq!(player_seed(0x28EFE333B266F103, 0), 0xED6D6CCF127CB19E);
        assert_eq!(player_seed(0x28EFE333B266F103, 1), 0xEB03688B255D6EEE);
    }

    #[test]
    fn first_legal_self_play_is_deterministic() {
        let rules: Arc<dyn GameRules<TicTacToe>> = Arc::new(TicTacToeRules);
        let engine = RayonMatchEngine::serial(rules);
        let config = MatchConfig {
            games: 1,
            seed: 0,
            max_plies: None,
        };
        let records = engine
            .run(&config, &[(Player::X, &FirstLegal), (Player::O, &FirstLegal)])
            .unwrap();
        assert_eq!(records.len(), 1);
        let record = &records[0];
        assert_eq!(record.game_index, 0);
        assert_eq!(record.seed, game_seed(0, 0));
        assert_eq!(
            record.actions,
            vec![Move(0), Move(1), Move(2), Move(3), Move(4), Move(5), Move(6)]
        );
        assert_eq!(record.outcome, Some(Outcome::Win(Player::X)));
    }

    #[test]
    fn records_are_in_game_index_order_with_derived_seeds() {
        let config = MatchConfig {
            games: 16,
            seed: 42,
            max_plies: None,
        };
        let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, &SeededRandom), (Player::O, &SeededRandom)];

        for engine in [
            RayonMatchEngine::serial(Arc::new(TicTacToeRules) as Arc<dyn GameRules<TicTacToe>>),
            RayonMatchEngine::new(Arc::new(TicTacToeRules) as Arc<dyn GameRules<TicTacToe>>),
            RayonMatchEngine::with_threads(Arc::new(TicTacToeRules) as Arc<dyn GameRules<TicTacToe>>, 3),
        ] {
            let records = engine.run(&config, players).unwrap();
            assert_eq!(records.len(), 16);
            for (i, record) in records.iter().enumerate() {
                assert_eq!(record.game_index, i);
                assert_eq!(record.seed, game_seed(42, i));
            }
        }
    }

    #[test]
    fn serial_equals_parallel() {
        let config = MatchConfig {
            games: 64,
            seed: 2026,
            max_plies: None,
        };
        let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, &SeededRandom), (Player::O, &SeededRandom)];

        let serial = RayonMatchEngine::serial(Arc::new(TicTacToeRules) as Arc<dyn GameRules<TicTacToe>>)
            .run(&config, players)
            .unwrap();
        let threaded = RayonMatchEngine::with_threads(Arc::new(TicTacToeRules) as Arc<dyn GameRules<TicTacToe>>, 4)
            .run(&config, players)
            .unwrap();
        let global = RayonMatchEngine::new(Arc::new(TicTacToeRules) as Arc<dyn GameRules<TicTacToe>>)
            .run(&config, players)
            .unwrap();

        assert_eq!(serial, threaded);
        assert_eq!(serial, global);
        assert_eq!(serde_json::to_vec(&serial).unwrap(), serde_json::to_vec(&threaded).unwrap());
        assert_eq!(serde_json::to_vec(&serial).unwrap(), serde_json::to_vec(&global).unwrap());
        assert!(serial.iter().any(|record| matches!(record.outcome, Some(Outcome::Win(_)))));
    }

    #[test]
    fn max_plies_yields_none_outcome() {
        let rules: Arc<dyn GameRules<TicTacToe>> = Arc::new(TicTacToeRules);
        let engine = RayonMatchEngine::serial(rules);
        let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, &FirstLegal), (Player::O, &FirstLegal)];

        let config = MatchConfig {
            games: 1,
            seed: 0,
            max_plies: Some(3),
        };
        let record = &engine.run(&config, players).unwrap()[0];
        assert_eq!(record.actions.len(), 3);
        assert_eq!(record.outcome, None);

        let config = MatchConfig {
            games: 1,
            seed: 0,
            max_plies: Some(9),
        };
        let record = &engine.run(&config, players).unwrap()[0];
        assert!(record.outcome.is_some());
    }

    #[test]
    fn max_plies_zero_records_no_actions() {
        let rules: Arc<dyn GameRules<TicTacToe>> = Arc::new(TicTacToeRules);
        let engine = RayonMatchEngine::serial(rules);
        let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, &FirstLegal), (Player::O, &FirstLegal)];
        let config = MatchConfig {
            games: 1,
            seed: 0,
            max_plies: Some(0),
        };
        let record = &engine.run(&config, players).unwrap()[0];
        assert!(record.actions.is_empty());
        assert_eq!(record.outcome, None);
    }

    #[test]
    fn missing_provider_is_error() {
        let rules: Arc<dyn GameRules<TicTacToe>> = Arc::new(TicTacToeRules);
        let engine = RayonMatchEngine::serial(rules);
        let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, &FirstLegal)];
        let config = MatchConfig {
            games: 1,
            seed: 0,
            max_plies: None,
        };
        let err = engine.run(&config, players).unwrap_err();
        assert_eq!(err, MatchError::MissingProvider("O".to_string()));
    }

    #[test]
    fn duplicate_player_first_slot_wins() {
        let rules: Arc<dyn GameRules<TicTacToe>> = Arc::new(TicTacToeRules);
        let engine = RayonMatchEngine::serial(rules);
        let config = MatchConfig {
            games: 1,
            seed: 0,
            max_plies: None,
        };

        let duplicate_players: &[(Player, &dyn StrategyProvider<TicTacToe>)] =
            &[(Player::X, &FirstLegal), (Player::X, &LastLegal), (Player::O, &LastLegal)];
        let with_duplicate = engine.run(&config, duplicate_players).unwrap();

        let plain_players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, &FirstLegal), (Player::O, &LastLegal)];
        let without_duplicate = engine.run(&config, plain_players).unwrap();

        assert_eq!(with_duplicate[0].actions, without_duplicate[0].actions);
    }

    #[test]
    fn illegal_action_is_strategy_error() {
        let rules: Arc<dyn GameRules<TicTacToe>> = Arc::new(TicTacToeRules);
        let engine = RayonMatchEngine::serial(rules);
        let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, &Illegal), (Player::O, &FirstLegal)];
        let config = MatchConfig {
            games: 1,
            seed: 0,
            max_plies: None,
        };
        let err = engine.run(&config, players).unwrap_err();
        match err {
            MatchError::Strategy(StrategyError::Other(m)) => {
                assert!(m.contains("illegal action"));
                assert!(m.contains("`illegal`"));
            }
            other => panic!("expected MatchError::Strategy(StrategyError::Other(_)), got {other:?}"),
        }
    }

    #[test]
    fn strategy_error_propagates() {
        let players: &[(Player, &dyn StrategyProvider<TicTacToe>)] = &[(Player::X, &Failing), (Player::O, &FirstLegal)];
        let config = MatchConfig {
            games: 1,
            seed: 0,
            max_plies: None,
        };

        let serial_rules: Arc<dyn GameRules<TicTacToe>> = Arc::new(TicTacToeRules);
        let serial_err = RayonMatchEngine::serial(serial_rules).run(&config, players).unwrap_err();
        assert_eq!(serial_err, MatchError::Strategy(StrategyError::Other("boom".to_string())));

        let parallel_rules: Arc<dyn GameRules<TicTacToe>> = Arc::new(TicTacToeRules);
        let parallel_err = RayonMatchEngine::new(parallel_rules).run(&config, players).unwrap_err();
        assert_eq!(parallel_err, MatchError::Strategy(StrategyError::Other("boom".to_string())));
    }

    #[test]
    fn player_seeds_differ_per_slot() {
        assert_ne!(player_seed(game_seed(0, 0), 0), player_seed(game_seed(0, 0), 1));

        let rules: Arc<dyn GameRules<TicTacToe>> = Arc::new(TicTacToeRules);
        let engine = RayonMatchEngine::serial(rules);
        let config = MatchConfig {
            games: 1,
            seed: 0,
            max_plies: None,
        };

        let random_players: &[(Player, &dyn StrategyProvider<TicTacToe>)] =
            &[(Player::X, &SeededRandom), (Player::O, &SeededRandom)];
        let random_record = &engine.run(&config, random_players).unwrap()[0];

        let deterministic_players: &[(Player, &dyn StrategyProvider<TicTacToe>)] =
            &[(Player::X, &FirstLegal), (Player::O, &FirstLegal)];
        let deterministic_record = &engine.run(&config, deterministic_players).unwrap()[0];

        assert_ne!(random_record.actions, deterministic_record.actions);
    }
}
