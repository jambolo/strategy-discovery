//! Static evaluator for tic-tac-toe: open-line count from the given perspective.

use super::board::{Board, LINES, Player, TicTacToe};
use crate::core::traits::StateEvaluator;
use crate::strategy::engine::WIN_VALUE;

/// Open-line heuristic: lines still winnable by `perspective` minus lines still winnable by
/// the opponent, in `[-8, 8]` on non-terminal boards. Terminal boards return `±WIN_VALUE`
/// for a win/loss and `0.0` for a draw, consistent with the engine adapter.
#[derive(Debug, Clone, Copy, Default)]
pub struct TicTacToeEvaluator;

impl TicTacToeEvaluator {
    /// Number of lines containing no mark of `player`.
    fn lines_without(board: &Board, player: Player) -> usize {
        LINES
            .iter()
            .filter(|line| line.iter().all(|&i| board.cell(i) != Some(player)))
            .count()
    }
}

impl StateEvaluator<TicTacToe> for TicTacToeEvaluator {
    fn evaluate(&self, board: &Board, perspective: Player) -> f32 {
        match board.winner() {
            Some(winner) if winner == perspective => WIN_VALUE,
            Some(_) => -WIN_VALUE,
            None if board.is_full() => 0.0,
            None => {
                let mine = Self::lines_without(board, perspective.other());
                let theirs = Self::lines_without(board, perspective);
                mine as f32 - theirs as f32
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::GameRules;
    use crate::games::tictactoe::Move;
    use crate::games::tictactoe::TicTacToeRules;
    use crate::strategy::engine::{AliceEvaluator, RulesResponseGenerator};
    use game_player::minimax;

    #[test]
    fn empty_board_evaluates_to_zero() {
        let eval = TicTacToeEvaluator;
        let board = Board::empty();
        assert_eq!(eval.evaluate(&board, Player::X), 0.0);
        assert_eq!(eval.evaluate(&board, Player::O), 0.0);
    }

    #[test]
    fn fixture_values() {
        let eval = TicTacToeEvaluator;

        let board = Board::parse("X........").unwrap();
        assert_eq!(eval.evaluate(&board, Player::X), 3.0);
        assert_eq!(eval.evaluate(&board, Player::O), -3.0);

        let board = Board::parse("X...O....").unwrap();
        assert_eq!(eval.evaluate(&board, Player::X), -1.0);
        assert_eq!(eval.evaluate(&board, Player::O), 1.0);
    }

    #[test]
    fn terminal_values() {
        let eval = TicTacToeEvaluator;

        let board = Board::parse("XXXOO....").unwrap();
        assert_eq!(eval.evaluate(&board, Player::X), 100.0);
        assert_eq!(eval.evaluate(&board, Player::O), -100.0);

        let board = Board::parse("XX.OOO.X.").unwrap();
        assert_eq!(eval.evaluate(&board, Player::X), -100.0);
        assert_eq!(eval.evaluate(&board, Player::O), 100.0);

        let board = Board::parse("XOXXOOOXX").unwrap();
        assert_eq!(eval.evaluate(&board, Player::X), 0.0);
        assert_eq!(eval.evaluate(&board, Player::O), 0.0);
    }

    #[test]
    fn range_is_within_eight_on_all_nonterminal_positions() {
        let eval = TicTacToeEvaluator;
        let rules = TicTacToeRules;
        let positions = rules.reachable_positions();
        assert_eq!(positions.len(), 5478);

        for b in positions.iter().filter(|b| !rules.is_terminal(b)) {
            for p in [Player::X, Player::O] {
                let v = eval.evaluate(b, p);
                assert!((-8.0..=8.0).contains(&v), "value {v} out of range for {b:?}");
                assert_eq!(v, v.trunc(), "value {v} not integral for {b:?}");
            }
        }
    }

    #[test]
    fn perspectives_are_antisymmetric_on_all_nonterminal_positions() {
        let eval = TicTacToeEvaluator;
        let rules = TicTacToeRules;
        let positions = rules.reachable_positions();

        for b in positions.iter().filter(|b| !rules.is_terminal(b)) {
            assert_eq!(eval.evaluate(b, Player::X), -eval.evaluate(b, Player::O));
        }
    }

    #[test]
    fn depth_one_search_from_empty_board_takes_center() {
        let rules = TicTacToeRules;
        let eval = TicTacToeEvaluator;
        let sef = AliceEvaluator::<TicTacToe>::new(&rules, &eval);
        let rg = RulesResponseGenerator::<TicTacToe>::new(&rules);

        assert_eq!(minimax::search(&sef, &rg, &Board::empty(), 1), Some(Move(4)));
    }
}
