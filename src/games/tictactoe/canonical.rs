//! Canonicalization of tic-tac-toe boards under the D4 symmetry group: the minimum-encoded
//! image over all 8 group elements is the canonical representative.

use super::board::Board;
use super::primitives::TicTacToePrimitives;
use crate::core::symmetry::{Permutation, SymmetryGroup};
use crate::core::traits::{Canonicalize, GamePrimitives};
use crate::games::tictactoe::board::TicTacToe;

/// Encodes a board as a base-3 digit string over cells `0..9`, `digit(None)=0`,
/// `digit(Some(X))=1`, `digit(Some(O))=2`, most significant digit first (cell 0). Side to move
/// is invariant under `transform` and does not enter the encoding.
pub fn encode(board: &Board) -> u32 {
    let mut code: u32 = 0;
    for p in 0..9 {
        let digit = match board.cell(p) {
            None => 0,
            Some(super::board::Player::X) => 1,
            Some(super::board::Player::O) => 2,
        };
        code = code * 3 + digit;
    }
    code
}

/// Canonicalizes tic-tac-toe boards by taking the minimum-encoded image over the 8 elements of
/// the D4 symmetry group.
#[derive(Debug, Clone)]
pub struct TicTacToeCanonicalizer {
    group: SymmetryGroup,
    primitives: TicTacToePrimitives,
}

impl TicTacToeCanonicalizer {
    /// Builds a canonicalizer, computing the D4 symmetry group once.
    pub fn new() -> Self {
        let primitives = TicTacToePrimitives;
        TicTacToeCanonicalizer {
            group: primitives.symmetry_group(),
            primitives,
        }
    }
}

impl Default for TicTacToeCanonicalizer {
    fn default() -> Self {
        TicTacToeCanonicalizer::new()
    }
}

impl Canonicalize<TicTacToe> for TicTacToeCanonicalizer {
    fn canonicalize_with_transform(&self, state: &Board) -> (Board, Permutation) {
        let mut best_code = encode(state);
        let mut best_state = *state;
        let mut best_perm = Permutation::identity(9);
        for perm in self.group.elements() {
            let image = self.primitives.transform(state, perm);
            let code = encode(&image);
            if code < best_code {
                best_code = code;
                best_state = image;
                best_perm = perm.clone();
            }
        }
        (best_state, best_perm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::GameRules;
    use crate::games::tictactoe::rules::TicTacToeRules;
    use std::collections::HashSet;

    #[test]
    fn encode_empty_board_is_zero() {
        assert_eq!(encode(&Board::empty()), 0);
    }

    #[test]
    fn encode_last_cell_x_is_one() {
        assert_eq!(encode(&Board::parse("........X").unwrap()), 1);
    }

    #[test]
    fn encode_first_cell_x_is_3_pow_8() {
        assert_eq!(encode(&Board::parse("X........").unwrap()), 6561);
    }

    #[test]
    fn encode_first_cell_o_is_2_times_3_pow_8() {
        let mut cells = [None; 9];
        cells[0] = Some(super::super::board::Player::O);
        let board = Board::from_cells(cells, super::super::board::Player::X);
        assert_eq!(encode(&board), 13122);
    }

    #[test]
    fn canonical_form_count() {
        let rules = TicTacToeRules;
        let canon = TicTacToeCanonicalizer::new();
        let reachable = rules.reachable_positions();
        assert_eq!(reachable.len(), 5478);
        let canonical_forms: HashSet<Board> = reachable.iter().map(|s| canon.canonicalize(s)).collect();
        assert_eq!(canonical_forms.len(), 765);
    }

    #[test]
    fn canonicalize_is_idempotent_over_all_reachable_positions() {
        let rules = TicTacToeRules;
        let canon = TicTacToeCanonicalizer::new();
        for s in rules.reachable_positions() {
            let canonical = canon.canonicalize(&s);
            let (twice, perm) = canon.canonicalize_with_transform(&canonical);
            assert_eq!(twice, canonical);
            assert!(perm.is_identity());
        }
    }

    #[test]
    fn transform_by_returned_permutation_reproduces_canonical_form() {
        let rules = TicTacToeRules;
        let canon = TicTacToeCanonicalizer::new();
        let primitives = TicTacToePrimitives;
        for s in rules.reachable_positions() {
            let (canonical, perm) = canon.canonicalize_with_transform(&s);
            assert_eq!(primitives.transform(&s, &perm), canonical);
        }
    }

    #[test]
    fn canonicalize_preserves_outcome_and_player_to_move_over_all_reachable_positions() {
        let rules = TicTacToeRules;
        let canon = TicTacToeCanonicalizer::new();
        for s in rules.reachable_positions() {
            let canonical = canon.canonicalize(&s);
            assert_eq!(rules.outcome(&canonical), rules.outcome(&s));
            assert_eq!(rules.player_to_move(&canonical), rules.player_to_move(&s));
        }
    }

    #[test]
    fn orbit_of_a_single_corner_mark_is_a_single_canonical_form() {
        let canon = TicTacToeCanonicalizer::new();
        let corners = [
            Board::parse("X........").unwrap(),
            Board::parse("..X......").unwrap(),
            Board::parse("......X..").unwrap(),
            Board::parse("........X").unwrap(),
        ];
        let canonical_forms: Vec<Board> = corners.iter().map(|b| canon.canonicalize(b)).collect();
        for w in canonical_forms.windows(2) {
            assert_eq!(w[0], w[1]);
        }

        let edge = Board::parse(".X.......").unwrap();
        assert_ne!(canonical_forms[0], canon.canonicalize(&edge));
    }

    #[test]
    fn canonicalize_of_empty_board_is_itself_with_identity() {
        let canon = TicTacToeCanonicalizer::new();
        let (canonical, perm) = canon.canonicalize_with_transform(&Board::empty());
        assert_eq!(canonical, Board::empty());
        assert!(perm.is_identity());
    }
}
