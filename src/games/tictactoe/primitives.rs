//! `GamePrimitives<TicTacToe>`: structural facts about the 3x3 board with no strategic
//! insight. Position orbits (centre/corners/edges) fall out of `symmetry_group()` mechanically —
//! they are never hand-listed here.

use super::board::{Board, LINES, Move, TicTacToe};
use crate::core::symmetry::{Permutation, SymmetryGroup};
use crate::core::traits::GamePrimitives;

/// 90-degree clockwise rotation of the 3x3 grid, as a cell permutation.
pub fn rot90() -> Permutation {
    Permutation::new(vec![2, 5, 8, 1, 4, 7, 0, 3, 6]).expect("rot90 is a permutation")
}

/// Horizontal reflection of the 3x3 grid, as a cell permutation.
pub fn reflect_horizontal() -> Permutation {
    Permutation::new(vec![2, 1, 0, 5, 4, 3, 8, 7, 6]).expect("reflect is a permutation")
}

/// Structural primitives for tic-tac-toe: 9 positions, orthogonal adjacency, the 8 win
/// lines, and the dihedral group of order 8 generated from a rotation and a reflection.
#[derive(Debug, Clone, Copy, Default)]
pub struct TicTacToePrimitives;

impl GamePrimitives<TicTacToe> for TicTacToePrimitives {
    fn position_count(&self) -> usize {
        9
    }

    fn adjacent(&self, position: usize) -> Vec<usize> {
        assert!(position < 9);
        let row = position / 3;
        let col = position % 3;
        let mut neighbours = Vec::with_capacity(4);
        if row > 0 {
            neighbours.push(position - 3);
        }
        if col > 0 {
            neighbours.push(position - 1);
        }
        if col < 2 {
            neighbours.push(position + 1);
        }
        if row < 2 {
            neighbours.push(position + 3);
        }
        neighbours.sort_unstable();
        neighbours
    }

    fn lines(&self) -> Vec<Vec<usize>> {
        LINES.iter().map(|l| l.to_vec()).collect()
    }

    fn symmetry_group(&self) -> SymmetryGroup {
        SymmetryGroup::generate(9, &[rot90(), reflect_horizontal()]).expect("D4 generators have degree 9")
    }

    fn occupant(&self, state: &Board, position: usize) -> Option<crate::games::tictactoe::board::Player> {
        state.cell(position)
    }

    fn action_position(&self, action: &Move) -> Option<usize> {
        Some(action.0)
    }

    fn transform(&self, state: &Board, perm: &Permutation) -> Board {
        let mut cells = [None; 9];
        for p in 0..9 {
            cells[perm.apply(p)] = state.cell(p);
        }
        Board::from_cells(cells, state.to_move())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::derived::PrimitiveFeatures;
    use crate::core::traits::FeatureExtractor;
    use crate::games::tictactoe::board::Player;
    use crate::games::tictactoe::rules::TicTacToeRules;

    #[test]
    fn rot90_has_order_four() {
        let r = rot90();
        let r2 = r.compose(&r);
        let r3 = r2.compose(&r);
        let r4 = r3.compose(&r);
        assert!(!r.is_identity());
        assert!(!r2.is_identity());
        assert!(!r3.is_identity());
        assert!(r4.is_identity());
    }

    #[test]
    fn reflect_horizontal_has_order_two() {
        let f = reflect_horizontal();
        assert!(!f.is_identity());
        assert!(f.compose(&f).is_identity());
    }

    #[test]
    fn symmetry_group_is_dihedral_of_order_eight() {
        let g = TicTacToePrimitives.symmetry_group();
        assert_eq!(g.order(), 8);
        assert_eq!(g.degree(), 9);
    }

    #[test]
    fn symmetry_group_orbits_are_centre_corners_edges() {
        let g = TicTacToePrimitives.symmetry_group();
        assert_eq!(g.orbits(), vec![vec![0, 2, 6, 8], vec![1, 3, 5, 7], vec![4]]);
    }

    #[test]
    fn adjacent_matches_expected_neighbours() {
        let p = TicTacToePrimitives;
        assert_eq!(p.adjacent(0), vec![1, 3]);
        assert_eq!(p.adjacent(1), vec![0, 2, 4]);
        assert_eq!(p.adjacent(4), vec![1, 3, 5, 7]);
        assert_eq!(p.adjacent(8), vec![5, 7]);
    }

    #[test]
    fn adjacent_is_ascending_and_symmetric_for_every_position() {
        let p = TicTacToePrimitives;
        for pos in 0..9 {
            let neighbours = p.adjacent(pos);
            let mut sorted = neighbours.clone();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(neighbours, sorted, "adjacent({pos}) must be strictly ascending, no dups");
            for &q in &neighbours {
                assert!(p.adjacent(q).contains(&pos), "adjacency must be symmetric: {pos} <-> {q}");
            }
        }
    }

    #[test]
    fn lines_matches_board_lines_exactly() {
        let p = TicTacToePrimitives;
        let lines = p.lines();
        assert_eq!(lines.len(), 8);
        for (l, line) in lines.iter().enumerate() {
            assert_eq!(line, &LINES[l].to_vec());
            let mut distinct = line.clone();
            distinct.sort_unstable();
            distinct.dedup();
            assert_eq!(distinct.len(), 3);
            assert!(line.iter().all(|&pos| pos < 9));
        }
    }

    #[test]
    fn transform_rotates_a_single_marked_cell() {
        let p = TicTacToePrimitives;
        let board = Board::parse("X........").unwrap();
        let rotated = p.transform(&board, &rot90());
        assert_eq!(rotated.cell(2), Some(Player::X));
        for i in (0..9).filter(|&i| i != 2) {
            assert_eq!(rotated.cell(i), None);
        }
        assert_eq!(rotated.to_move(), Player::O);
    }

    #[test]
    fn transform_reflects_a_single_marked_cell() {
        let p = TicTacToePrimitives;
        let board = Board::parse("X........").unwrap();
        let reflected = p.transform(&board, &reflect_horizontal());
        assert_eq!(reflected.cell(2), Some(Player::X));
        for i in (0..9).filter(|&i| i != 2) {
            assert_eq!(reflected.cell(i), None);
        }
    }

    #[test]
    fn transform_under_identity_is_unchanged() {
        let p = TicTacToePrimitives;
        let board = Board::parse("X........").unwrap();
        let same = p.transform(&board, &Permutation::identity(9));
        assert_eq!(same, board);
    }

    #[test]
    fn transform_then_inverse_transform_is_identity_for_every_group_element() {
        let p = TicTacToePrimitives;
        let board = Board::parse("XO..X.O..").unwrap();
        let g = p.symmetry_group();
        for elem in g.elements() {
            let forward = p.transform(&board, elem);
            let back = p.transform(&forward, &elem.inverse());
            assert_eq!(back, board, "transform/inverse round trip failed for {elem:?}");
        }
    }

    #[test]
    fn occupant_reports_cell_contents() {
        let p = TicTacToePrimitives;
        let board = Board::parse("X........").unwrap();
        assert_eq!(p.occupant(&board, 0), Some(Player::X));
        assert_eq!(p.occupant(&board, 1), None);
    }

    #[test]
    fn action_position_is_the_move_index() {
        let p = TicTacToePrimitives;
        assert_eq!(p.action_position(&Move(7)), Some(7));
    }

    #[test]
    fn tier1_feature_extractor_definitions_count_and_orbits() {
        let ext = PrimitiveFeatures::new(TicTacToeRules, TicTacToePrimitives);
        assert_eq!(ext.definitions().len(), 47);
        assert_eq!(ext.orbits(), &[vec![0, 2, 6, 8], vec![1, 3, 5, 7], vec![4]]);
    }
}
