//! `game_player::State` adapter for the tic-tac-toe board, plus `Player` <-> `PlayerId`
//! conversions. Bridges `Board`/`TicTacToeRules` to the `game-player` search engine without
//! either side knowing about the other's abstractions.

use super::board::{Board, Move, Player};
use game_player::{PlayerId, State};

impl From<Player> for PlayerId {
    /// `X` is the maximizing side in minimax, so it maps to `Alice`; `O` maps to `Bob`.
    fn from(player: Player) -> PlayerId {
        match player {
            Player::X => PlayerId::Alice,
            Player::O => PlayerId::Bob,
        }
    }
}

impl From<PlayerId> for Player {
    /// Inverse of `From<Player> for PlayerId`.
    fn from(id: PlayerId) -> Player {
        match id {
            PlayerId::Alice => Player::X,
            PlayerId::Bob => Player::O,
        }
    }
}

/// Thin wrapper over `PlayerId::from(player)`.
pub fn player_id(player: Player) -> PlayerId {
    player.into()
}

/// Thin wrapper over `Player::from(id)`.
pub fn player_from_id(id: PlayerId) -> Player {
    id.into()
}

impl State for Board {
    type Action = Move;

    /// 2 bits per cell (`None` = 0, `X` = 1, `O` = 2), then 1 bit for side to move (`O` = 1).
    /// Unique for every position.
    fn fingerprint(&self) -> u64 {
        let mut fp = 0u64;
        for i in 0..9 {
            let code = match self.cell(i) {
                None => 0,
                Some(Player::X) => 1,
                Some(Player::O) => 2,
            };
            fp = (fp << 2) | code;
        }
        fp = (fp << 1) | (self.to_move() == Player::O) as u64;
        fp
    }

    fn whose_turn(&self) -> PlayerId {
        self.to_move().into()
    }

    /// Agrees with `TicTacToeRules::is_terminal`: a board is terminal iff it has a winner or is full.
    fn is_terminal(&self) -> bool {
        self.winner().is_some() || self.is_full()
    }

    fn apply(&self, action: &Move) -> Board {
        self.place(action.0, self.to_move())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::GameRules;
    use crate::games::tictactoe::rules::TicTacToeRules;
    use std::collections::HashSet;

    #[test]
    fn player_to_player_id_conversions() {
        assert_eq!(PlayerId::from(Player::X), PlayerId::Alice);
        assert_eq!(PlayerId::from(Player::O), PlayerId::Bob);
    }

    #[test]
    fn player_id_round_trips_both_ways() {
        for &player in &[Player::X, Player::O] {
            let id: PlayerId = player.into();
            let back: Player = id.into();
            assert_eq!(back, player);
        }
        for &id in &[PlayerId::Alice, PlayerId::Bob] {
            let player: Player = id.into();
            let back: PlayerId = player.into();
            assert_eq!(back, id);
        }
    }

    #[test]
    fn helper_functions_match_from_impls() {
        assert_eq!(player_id(Player::X), PlayerId::from(Player::X));
        assert_eq!(player_id(Player::O), PlayerId::from(Player::O));
        assert_eq!(player_from_id(PlayerId::Alice), Player::from(PlayerId::Alice));
        assert_eq!(player_from_id(PlayerId::Bob), Player::from(PlayerId::Bob));
    }

    #[test]
    fn empty_board_fingerprint_is_zero() {
        assert_eq!(State::fingerprint(&Board::empty()), 0);
    }

    #[test]
    fn fingerprint_after_x_at_cell_zero() {
        let board = Board::empty().place(0, Player::X);
        assert_eq!(State::fingerprint(&board), (1u64 << 17) | 1);
    }

    #[test]
    fn fingerprints_are_unique_across_all_reachable_positions() {
        let rules = TicTacToeRules;
        let positions = rules.reachable_positions();
        assert_eq!(positions.len(), 5478);
        let fingerprints: HashSet<u64> = positions.iter().map(State::fingerprint).collect();
        assert_eq!(fingerprints.len(), 5478);
    }

    #[test]
    fn is_terminal_agrees_with_rules_over_all_reachable_positions() {
        let rules = TicTacToeRules;
        for state in rules.reachable_positions() {
            assert_eq!(
                State::is_terminal(&state),
                GameRules::is_terminal(&rules, &state),
                "mismatch for {state}"
            );
        }
    }

    #[test]
    fn whose_turn_agrees_with_rules_over_all_reachable_positions() {
        let rules = TicTacToeRules;
        for state in rules.reachable_positions() {
            assert_eq!(
                State::whose_turn(&state),
                PlayerId::from(rules.player_to_move(&state)),
                "mismatch for {state}"
            );
        }
    }

    #[test]
    fn apply_agrees_with_rules_over_all_legal_moves_from_every_nonterminal_position() {
        let rules = TicTacToeRules;
        for state in rules.reachable_positions() {
            if GameRules::is_terminal(&rules, &state) {
                continue;
            }
            for action in rules.legal_actions(&state) {
                let via_state_trait = State::apply(&state, &action);
                let via_rules = rules.apply(&state, &action).unwrap();
                assert_eq!(via_state_trait, via_rules, "mismatch for {state} + {action:?}");
            }
        }
    }
}
