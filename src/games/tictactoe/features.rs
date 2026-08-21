//! Tic-tac-toe tier-2 (hand-supplied) features: per-line X/O counts, threats,
//! winning/blocking/fork cell sets, and a win-available flag.
//!
//! "Mover" is `state.to_move()`; "opponent" is the mover's other player. All
//! mover-relative features are computed relative to the side to move, even on
//! terminal states (extraction never panics). Names are prefixed `ttt.` to
//! avoid colliding with the tier-1 primitive feature names (`free`, `mine`,
//! `theirs`, `orbit{k}.*`, `line{l}.*`).

use super::board::{Board, LINES, Player, TicTacToe};
use crate::core::features::{FeatureDef, FeatureValue, FeatureVector, Tier};
use crate::core::traits::FeatureExtractor;
use std::collections::BTreeSet;

/// Hand-supplied (tier-2) feature extractor for tic-tac-toe.
#[derive(Debug, Clone, Copy, Default)]
pub struct TicTacToeFeatures;

impl TicTacToeFeatures {
    /// Description text for a per-line mark-count feature, naming the line's cells.
    fn line_description(l: usize, symbol: char) -> String {
        let cells = LINES[l];
        format!(
            "number of {symbol} marks on line {l} {{{}, {}, {}}}",
            cells[0], cells[1], cells[2]
        )
    }
}

impl FeatureExtractor<TicTacToe> for TicTacToeFeatures {
    fn definitions(&self) -> Vec<FeatureDef> {
        let mut defs = Vec::with_capacity(22);
        for l in 0..8 {
            defs.push(FeatureDef::native(
                format!("ttt.line{l}.x"),
                Tier::Supplied,
                Self::line_description(l, 'X'),
            ));
            defs.push(FeatureDef::native(
                format!("ttt.line{l}.o"),
                Tier::Supplied,
                Self::line_description(l, 'O'),
            ));
        }
        defs.push(FeatureDef::native(
            "ttt.threats.mine",
            Tier::Supplied,
            "lines with exactly 2 mover marks and 1 empty cell",
        ));
        defs.push(FeatureDef::native(
            "ttt.threats.theirs",
            Tier::Supplied,
            "lines with exactly 2 opponent marks and 1 empty cell",
        ));
        defs.push(FeatureDef::native(
            "ttt.winning_cells",
            Tier::Supplied,
            "empty cells that complete a mover threat (mover wins immediately by playing there)",
        ));
        defs.push(FeatureDef::native(
            "ttt.blocking_cells",
            Tier::Supplied,
            "empty cells that complete an opponent threat (mover must play there to block)",
        ));
        defs.push(FeatureDef::native(
            "ttt.fork_cells",
            Tier::Supplied,
            "empty cells c such that at least 2 lines through c each contain exactly 1 mover mark \
             and 0 opponent marks (playing c creates >= 2 new threats)",
        ));
        defs.push(FeatureDef::native(
            "ttt.win_available",
            Tier::Supplied,
            "ttt.winning_cells is non-empty",
        ));
        defs
    }

    fn extract(&self, state: &Board) -> FeatureVector {
        let mover = state.to_move();
        let opponent = mover.other();

        let mut vector = FeatureVector::new();

        // Per-line X/O counts and (if exactly one) the index of the line's empty cell.
        let mut x_count = [0i64; 8];
        let mut o_count = [0i64; 8];
        let mut sole_empty = [0usize; 8];

        for (l, line) in LINES.iter().enumerate() {
            for &idx in line {
                match state.cell(idx) {
                    Some(Player::X) => x_count[l] += 1,
                    Some(Player::O) => o_count[l] += 1,
                    None => sole_empty[l] = idx,
                }
            }
            vector.insert(format!("ttt.line{l}.x"), FeatureValue::Int(x_count[l]));
            vector.insert(format!("ttt.line{l}.o"), FeatureValue::Int(o_count[l]));
        }

        let mover_count = |l: usize| -> i64 {
            match mover {
                Player::X => x_count[l],
                Player::O => o_count[l],
            }
        };
        let opp_count = |l: usize| -> i64 {
            match opponent {
                Player::X => x_count[l],
                Player::O => o_count[l],
            }
        };

        let mut threats_mine = 0i64;
        let mut threats_theirs = 0i64;
        let mut winning_cells = BTreeSet::new();
        let mut blocking_cells = BTreeSet::new();

        for (l, &empty_idx) in sole_empty.iter().enumerate() {
            if mover_count(l) == 2 && opp_count(l) == 0 {
                threats_mine += 1;
                winning_cells.insert(empty_idx);
            }
            if opp_count(l) == 2 && mover_count(l) == 0 {
                threats_theirs += 1;
                blocking_cells.insert(empty_idx);
            }
        }

        // A fork cell is an empty cell through which at least 2 lines each carry
        // exactly 1 mover mark and 0 opponent marks: playing it creates >= 2 new threats.
        let mut fork_cells = BTreeSet::new();
        for c in 0..9 {
            if state.cell(c).is_some() {
                continue;
            }
            let qualifying = LINES
                .iter()
                .enumerate()
                .filter(|(l, line)| line.contains(&c) && mover_count(*l) == 1 && opp_count(*l) == 0)
                .count();
            if qualifying >= 2 {
                fork_cells.insert(c);
            }
        }

        let win_available = !winning_cells.is_empty();

        vector.insert("ttt.threats.mine", FeatureValue::Int(threats_mine));
        vector.insert("ttt.threats.theirs", FeatureValue::Int(threats_theirs));
        vector.insert("ttt.winning_cells", FeatureValue::Set(winning_cells));
        vector.insert("ttt.blocking_cells", FeatureValue::Set(blocking_cells));
        vector.insert("ttt.fork_cells", FeatureValue::Set(fork_cells));
        vector.insert("ttt.win_available", FeatureValue::Bool(win_available));

        vector
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn definitions_are_well_formed() {
        let defs = TicTacToeFeatures.definitions();
        assert_eq!(defs.len(), 22);
        let names: BTreeSet<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names.len(), 22, "names must be unique");
        assert_eq!(defs.first().unwrap().name, "ttt.line0.x");
        assert_eq!(defs.last().unwrap().name, "ttt.win_available");
        for def in &defs {
            assert_eq!(def.tier, Tier::Supplied);
            assert!(def.is_native());
        }
    }

    #[test]
    fn fixture_one_single_open_threat() {
        // NOTE: the step's context table labels this board "X to move", but
        // `Board::parse` (already merged, out of this step's scope) derives O
        // to move for X=2/O=1 counts (X has exactly one more than O). The
        // table's feature values match a mover with 2 marks on line 0, i.e.
        // they describe X's perspective; asserted here relative to the
        // actual mover (O) with mine/theirs and winning/blocking swapped
        // accordingly. See report `deviations`.
        let board = Board::parse("XX..O....").unwrap();
        assert_eq!(board.to_move(), Player::O);
        let fv = TicTacToeFeatures.extract(&board);
        assert_eq!(fv.get("ttt.line0.x"), Some(&FeatureValue::Int(2)));
        assert_eq!(fv.get("ttt.line0.o"), Some(&FeatureValue::Int(0)));
        assert_eq!(fv.get("ttt.line1.o"), Some(&FeatureValue::Int(1)));
        assert_eq!(fv.get("ttt.threats.mine"), Some(&FeatureValue::Int(0)));
        assert_eq!(fv.get("ttt.threats.theirs"), Some(&FeatureValue::Int(1)));
        assert_eq!(fv.get("ttt.winning_cells"), Some(&FeatureValue::Set(BTreeSet::new())));
        assert_eq!(fv.get("ttt.blocking_cells"), Some(&FeatureValue::Set(BTreeSet::from([2]))));
        assert_eq!(fv.get("ttt.fork_cells"), Some(&FeatureValue::Set(BTreeSet::new())));
        assert_eq!(fv.get("ttt.win_available"), Some(&FeatureValue::Bool(false)));
    }

    #[test]
    fn fixture_two_block_and_fork() {
        let board = Board::parse("XO..O...X").unwrap();
        assert_eq!(board.to_move(), Player::X);
        let fv = TicTacToeFeatures.extract(&board);
        assert_eq!(fv.get("ttt.threats.mine"), Some(&FeatureValue::Int(0)));
        assert_eq!(fv.get("ttt.threats.theirs"), Some(&FeatureValue::Int(1)));
        assert_eq!(fv.get("ttt.winning_cells"), Some(&FeatureValue::Set(BTreeSet::new())));
        assert_eq!(fv.get("ttt.blocking_cells"), Some(&FeatureValue::Set(BTreeSet::from([7]))));
        assert_eq!(fv.get("ttt.fork_cells"), Some(&FeatureValue::Set(BTreeSet::from([6]))));
        assert_eq!(fv.get("ttt.win_available"), Some(&FeatureValue::Bool(false)));
    }

    #[test]
    fn fixture_three_mover_is_o() {
        let board = Board::parse("XXO.O...X").unwrap();
        assert_eq!(board.to_move(), Player::O);
        let fv = TicTacToeFeatures.extract(&board);
        assert_eq!(fv.get("ttt.threats.mine"), Some(&FeatureValue::Int(1)));
        assert_eq!(fv.get("ttt.threats.theirs"), Some(&FeatureValue::Int(0)));
        assert_eq!(fv.get("ttt.winning_cells"), Some(&FeatureValue::Set(BTreeSet::from([6]))));
        assert_eq!(fv.get("ttt.blocking_cells"), Some(&FeatureValue::Set(BTreeSet::new())));
        assert_eq!(fv.get("ttt.win_available"), Some(&FeatureValue::Bool(true)));
    }

    #[test]
    fn fixture_four_empty_board() {
        let board = Board::empty();
        let fv = TicTacToeFeatures.extract(&board);
        for l in 0..8 {
            assert_eq!(fv.get(&format!("ttt.line{l}.x")), Some(&FeatureValue::Int(0)));
            assert_eq!(fv.get(&format!("ttt.line{l}.o")), Some(&FeatureValue::Int(0)));
        }
        assert_eq!(fv.get("ttt.threats.mine"), Some(&FeatureValue::Int(0)));
        assert_eq!(fv.get("ttt.threats.theirs"), Some(&FeatureValue::Int(0)));
        assert_eq!(fv.get("ttt.winning_cells"), Some(&FeatureValue::Set(BTreeSet::new())));
        assert_eq!(fv.get("ttt.blocking_cells"), Some(&FeatureValue::Set(BTreeSet::new())));
        assert_eq!(fv.get("ttt.fork_cells"), Some(&FeatureValue::Set(BTreeSet::new())));
        assert_eq!(fv.get("ttt.win_available"), Some(&FeatureValue::Bool(false)));
    }

    #[test]
    fn extract_names_match_definitions_on_empty_board() {
        let extractor = TicTacToeFeatures;
        let def_names: BTreeSet<String> = extractor.definitions().into_iter().map(|d| d.name).collect();
        let fv = extractor.extract(&Board::empty());
        let value_names: BTreeSet<String> = fv.iter().map(|(name, _)| name.clone()).collect();
        assert_eq!(def_names, value_names);
    }

    #[test]
    fn extract_does_not_panic_on_terminal_board() {
        let board = Board::parse("XXXOO....").unwrap();
        assert_eq!(board.to_move(), Player::O);
        let _ = TicTacToeFeatures.extract(&board);
    }
}
