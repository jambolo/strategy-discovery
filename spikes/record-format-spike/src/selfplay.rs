//! Random self-play generator: uniform legal-move choice, one `PositionRecord` per ply.

use crate::record::{PositionRecord, fnv1a_64};
use rand::{RngExt, SeedableRng};
use rand_chacha::ChaCha8Rng;
use strategy_discovery::core::traits::{Canonicalize, GamePrimitives, GameRules};
use strategy_discovery::games::tictactoe::board::{Board, Outcome, Player};
use strategy_discovery::games::tictactoe::canonical::TicTacToeCanonicalizer;
use strategy_discovery::games::tictactoe::primitives::TicTacToePrimitives;
use strategy_discovery::games::tictactoe::rules::TicTacToeRules;

const STRATEGY_KIND: &str = "random";
const CONFIG_LABEL: &str = "random-selfplay-v1";

/// SplitMix64: fast, well-distributed seed derivation for per-game RNG streams.
fn splitmix64(seed: u64) -> u64 {
    let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// 9-character board encoding over `{X, O, .}`, cell `i` at char `i`, row-major.
fn encode_board(board: &Board) -> String {
    board
        .cells()
        .iter()
        .map(|c| match c {
            Some(Player::X) => 'X',
            Some(Player::O) => 'O',
            None => '.',
        })
        .collect()
}

/// Orbit index of `position` under `primitives`'s symmetry group: the index of the orbit
/// (from `symmetry_group().orbits()`) that contains `position`.
fn orbit_index(primitives: &TicTacToePrimitives, position: usize) -> u8 {
    let orbits = primitives.symmetry_group().orbits();
    orbits
        .iter()
        .position(|orbit| orbit.contains(&position))
        .expect("every position belongs to exactly one orbit") as u8
}

/// Plays `games` games of random self-play from `master_seed`, returning one `PositionRecord`
/// per ply across all games, in game order then ply order.
pub fn generate(games: u64, master_seed: u64) -> Vec<PositionRecord> {
    let rules = TicTacToeRules;
    let canon = TicTacToeCanonicalizer::new();
    let primitives = TicTacToePrimitives;
    let config_hash = format!("{:016x}", fnv1a_64(CONFIG_LABEL.as_bytes()));

    let mut all_records = Vec::new();
    for game_id in 0..games {
        let derived_seed = splitmix64(master_seed ^ game_id);
        let mut rng = ChaCha8Rng::seed_from_u64(derived_seed);
        let mut state = rules.initial_state();
        let mut move_number: u8 = 0;
        let mut first_move_orbit: Option<u8> = None;
        let mut game_records = Vec::new();

        loop {
            let legal = rules.legal_actions(&state);
            if legal.is_empty() {
                break;
            }
            let idx = rng.random_range(0..legal.len());
            let chosen = legal[idx];

            if move_number == 0 {
                first_move_orbit = Some(orbit_index(&primitives, chosen.0));
            }
            let mask = legal.iter().fold(0u16, |acc, m| acc | (1u16 << m.0));
            let side_to_move = match state.to_move() {
                Player::X => "X",
                Player::O => "O",
            };

            game_records.push(PositionRecord {
                schema_version: 1,
                game_id,
                seed: derived_seed,
                strategy_kind: STRATEGY_KIND.to_string(),
                config_hash: config_hash.clone(),
                move_number,
                side_to_move: side_to_move.to_string(),
                state: encode_board(&state),
                canonical_state: encode_board(&canon.canonicalize(&state)),
                legal_moves_mask: mask,
                chosen_move: chosen.0 as u8,
                first_move_orbit: first_move_orbit.expect("set on the first ply of every game"),
                outcome: String::new(),
                game_length: 0,
            });

            state = rules.apply(&state, &chosen).expect("chosen move is legal");
            move_number += 1;
        }

        let outcome_str = match rules.outcome(&state).expect("loop exits only on a terminal state") {
            Outcome::Win(Player::X) => "X",
            Outcome::Win(Player::O) => "O",
            Outcome::Draw => "draw",
        };
        for record in &mut game_records {
            record.outcome = outcome_str.to_string();
            record.game_length = move_number;
        }
        all_records.extend(game_records);
    }
    all_records
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_produces_one_record_per_ply_and_terminal_outcomes() {
        let records = generate(50, 1);
        assert!(!records.is_empty());
        for r in &records {
            assert!(["X", "O", "draw"].contains(&r.outcome.as_str()));
            assert_eq!(r.state.len(), 9);
            assert_eq!(r.canonical_state.len(), 9);
        }
    }

    #[test]
    fn generate_is_deterministic_for_a_fixed_seed() {
        let a = generate(20, 42);
        let b = generate(20, 42);
        assert_eq!(a, b);
    }
}
