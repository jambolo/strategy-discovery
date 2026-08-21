//! Rules of play for tic-tac-toe: legality, transitions, termination.

use super::board::{Board, Move, Outcome, Player, TicTacToe};
use crate::core::traits::{GameRules, RulesError};
use std::collections::HashSet;

/// Rules of tic-tac-toe.
#[derive(Debug, Clone, Copy, Default)]
pub struct TicTacToeRules;

impl GameRules<TicTacToe> for TicTacToeRules {
    fn initial_state(&self) -> Board {
        Board::empty()
    }

    fn player_to_move(&self, state: &Board) -> Player {
        state.to_move()
    }

    fn legal_actions(&self, state: &Board) -> Vec<Move> {
        if self.outcome(state).is_some() {
            return Vec::new();
        }
        (0..9).filter(|&i| state.cell(i).is_none()).map(Move).collect()
    }

    fn apply(&self, state: &Board, action: &Move) -> Result<Board, RulesError> {
        if self.outcome(state).is_some() {
            return Err(RulesError::GameOver);
        }
        if action.0 >= 9 {
            return Err(RulesError::IllegalAction(format!(
                "cell index {} out of range 0..9",
                action.0
            )));
        }
        if state.cell(action.0).is_some() {
            return Err(RulesError::IllegalAction(format!("cell {} is occupied", action.0)));
        }
        Ok(state.place(action.0, state.to_move()))
    }

    fn outcome(&self, state: &Board) -> Option<Outcome> {
        if let Some(p) = state.winner() {
            Some(Outcome::Win(p))
        } else if state.is_full() {
            Some(Outcome::Draw)
        } else {
            None
        }
    }
}

impl TicTacToeRules {
    /// All positions reachable from the initial state by breadth-first traversal over
    /// `legal_actions` (empty on terminal states, so terminals are included but never expanded),
    /// deduplicated, in first-discovery order. Always has exactly 5478 entries.
    pub fn reachable_positions(&self) -> Vec<Board> {
        let start = self.initial_state();
        let mut seen: HashSet<Board> = HashSet::new();
        seen.insert(start);
        let mut order = vec![start];
        let mut frontier = vec![start];
        while !frontier.is_empty() {
            let mut next_frontier = Vec::new();
            for state in frontier {
                for action in self.legal_actions(&state) {
                    let next = self.apply(&state, &action).expect("legal action must apply");
                    if seen.insert(next) {
                        order.push(next);
                        next_frontier.push(next);
                    }
                }
            }
            frontier = next_frontier;
        }
        order
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::GameRules;
    use crate::games::tictactoe::board::LINES;

    #[test]
    fn every_line_is_a_win_for_either_player() {
        let rules = TicTacToeRules;
        for &line in LINES.iter() {
            for &winner in &[Player::X, Player::O] {
                let loser = winner.other();
                let mut cells = [None; 9];
                for &i in &line {
                    cells[i] = Some(winner);
                }
                let non_line: Vec<usize> = (0..9).filter(|i| !line.contains(i)).take(2).collect();
                for &i in &non_line {
                    cells[i] = Some(loser);
                }
                let board = Board::from_cells(cells, winner);
                assert_eq!(rules.outcome(&board), Some(Outcome::Win(winner)));
                assert!(rules.is_terminal(&board));
                assert!(rules.legal_actions(&board).is_empty());
            }
        }
    }

    #[test]
    fn full_board_with_no_winner_is_a_draw() {
        let rules = TicTacToeRules;
        let board = Board::parse("XOXXOOOXX").unwrap();
        assert_eq!(rules.outcome(&board), Some(Outcome::Draw));
        assert!(rules.legal_actions(&board).is_empty());
    }

    #[test]
    fn initial_state_is_nonterminal_with_nine_actions() {
        let rules = TicTacToeRules;
        let state = rules.initial_state();
        assert_eq!(rules.outcome(&state), None);
        assert_eq!(rules.legal_actions(&state).len(), 9);
        assert_eq!(rules.player_to_move(&state), Player::X);
    }

    #[test]
    fn turns_alternate_and_cells_update() {
        let rules = TicTacToeRules;
        let state = rules.initial_state();
        let state = rules.apply(&state, &Move(0)).unwrap();
        assert_eq!(rules.player_to_move(&state), Player::O);
        let state = rules.apply(&state, &Move(1)).unwrap();
        assert_eq!(rules.player_to_move(&state), Player::X);
        assert_eq!(state.cell(0), Some(Player::X));
        assert_eq!(state.cell(1), Some(Player::O));
    }

    #[test]
    fn apply_rejects_out_of_range_action() {
        let rules = TicTacToeRules;
        let state = rules.initial_state();
        assert_eq!(
            rules.apply(&state, &Move(9)),
            Err(RulesError::IllegalAction("cell index 9 out of range 0..9".to_string()))
        );
    }

    #[test]
    fn apply_rejects_occupied_cell() {
        let rules = TicTacToeRules;
        let state = rules.initial_state();
        let state = rules.apply(&state, &Move(4)).unwrap();
        assert_eq!(
            rules.apply(&state, &Move(4)),
            Err(RulesError::IllegalAction("cell 4 is occupied".to_string()))
        );
    }

    #[test]
    fn apply_rejects_moves_on_a_draw_board() {
        let rules = TicTacToeRules;
        let board = Board::parse("XOXXOOOXX").unwrap();
        assert_eq!(rules.apply(&board, &Move(0)), Err(RulesError::GameOver));
    }

    #[test]
    fn apply_rejects_moves_on_a_won_board() {
        let rules = TicTacToeRules;
        let board = Board::from_cells(
            [
                Some(Player::X),
                Some(Player::X),
                Some(Player::X),
                Some(Player::O),
                Some(Player::O),
                None,
                None,
                None,
                None,
            ],
            Player::O,
        );
        assert_eq!(rules.apply(&board, &Move(5)), Err(RulesError::GameOver));
    }

    #[test]
    fn parse_rejects_wrong_length() {
        assert!(Board::parse("XOXO").is_err());
    }

    #[test]
    fn parse_rejects_bad_char() {
        assert!(Board::parse("XOXOXOXO?").is_err());
    }

    #[test]
    fn parse_rejects_more_o_than_x() {
        assert!(Board::parse("OOX......").is_err());
    }

    #[test]
    fn parse_accepts_empty_board_with_x_to_move() {
        let board = Board::parse(".........").unwrap();
        assert_eq!(board.to_move(), Player::X);
    }

    #[test]
    fn parse_derives_o_to_move_after_one_x() {
        let board = Board::parse("X........").unwrap();
        assert_eq!(board.to_move(), Player::O);
        assert_eq!(board.to_string(), "X..\n...\n...");
    }

    #[test]
    fn reachable_positions_count() {
        let rules = TicTacToeRules;
        assert_eq!(rules.reachable_positions().len(), 5478);
    }

    #[test]
    fn board_round_trips_through_json() {
        let board = Board::parse("XOXXOOOXX").unwrap();
        let json = serde_json::to_string(&board).unwrap();
        let round_tripped: Board = serde_json::from_str(&json).unwrap();
        assert_eq!(board, round_tripped);
    }

    #[test]
    fn outcome_round_trips_through_json() {
        let outcome = Outcome::Win(Player::O);
        let json = serde_json::to_string(&outcome).unwrap();
        let round_tripped: Outcome = serde_json::from_str(&json).unwrap();
        assert_eq!(outcome, round_tripped);
    }
}
