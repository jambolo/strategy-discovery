//! The tic-tac-toe benchmark opponent population used to evaluate strategies under test.

use crate::strategy::roster::Roster;

/// The versioned tic-tac-toe benchmark opponent population: a `random` baseline, minimax
/// entries `depth-1` through `depth-4` and `depth-6`, then `perfect` (depth 9). Evaluating a
/// strategy under test means playing every entry in both colours (the Phase 7 harness). Bump
/// the roster version whenever entries change, so archived results stay comparable only within
/// the version that produced them.
pub fn benchmark_roster() -> Roster {
    Roster::graded("ttt-benchmark", 1, &[1, 2, 3, 4, 6], 9).expect("constant benchmark roster is valid")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::{MatchConfig, MatchEngine};
    use crate::discovery::RayonMatchEngine;
    use crate::games::tictactoe::{Outcome, Player, engine_bundle};
    use crate::strategy::minimax::TieBreak;
    use crate::strategy::registry::{StrategyRegistry, StrategySpec};

    #[test]
    fn benchmark_roster_has_seven_entries_in_order() {
        let roster = benchmark_roster();

        let names: Vec<&str> = roster.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["random", "depth-1", "depth-2", "depth-3", "depth-4", "depth-6", "perfect"]
        );
        assert_eq!(roster.id(), "ttt-benchmark-v1");
        assert_eq!(roster.version, 1);
    }

    #[test]
    fn benchmark_roster_specs_are_seeded_uniform_minimax() {
        let roster = benchmark_roster();

        assert_eq!(roster.entries[0].spec, StrategySpec::Random);

        let expected_depths: [u32; 6] = [1, 2, 3, 4, 6, 9];
        for (entry, depth) in roster.entries[1..].iter().zip(expected_depths) {
            match &entry.spec {
                StrategySpec::Minimax(cfg) => {
                    assert_eq!(cfg.epsilon, 0.0);
                    assert_eq!(cfg.tie_break, TieBreak::SeededUniform);
                    assert_eq!(cfg.depth, depth);
                }
                other => panic!("expected Minimax for `{}`, got {other:?}", entry.name),
            }
        }
    }

    #[test]
    fn every_benchmark_entry_builds_through_registry() {
        let registry = StrategyRegistry::new(engine_bundle());
        let roster = benchmark_roster();

        for entry in &roster.entries {
            let provider = registry.build(&entry.spec).unwrap();
            let expected_kind = if entry.name == "random" { "random" } else { "minimax" };
            assert_eq!(provider.kind(), expected_kind);
        }
    }

    #[test]
    fn engine_bundle_plays_a_game_through_the_registry() {
        let registry = StrategyRegistry::new(engine_bundle());
        let roster = benchmark_roster();
        let entry = roster.get("perfect").unwrap();
        let provider = registry.build(&entry.spec).unwrap();

        let engine = RayonMatchEngine::serial(engine_bundle().rules);
        let config = MatchConfig {
            games: 1,
            seed: 1,
            max_plies: None,
        };
        let records = engine
            .run(&config, &[(Player::X, provider.as_ref()), (Player::O, provider.as_ref())])
            .unwrap();

        assert_eq!(records.len(), 1);
        assert_eq!(records[0].outcome, Some(Outcome::Draw));
    }
}
