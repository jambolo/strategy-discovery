//! Board state, cells, moves and outcomes for tic-tac-toe.

use crate::core::traits::GameDomain;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A player. `X` moves first.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Player {
    /// The first player to move.
    X,
    /// The second player to move.
    O,
}

impl Player {
    /// The opponent.
    pub fn other(self) -> Player {
        match self {
            Player::X => Player::O,
            Player::O => Player::X,
        }
    }
}

/// Result of a finished game.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub enum Outcome {
    /// The named player won.
    Win(Player),
    /// The board filled with no winner.
    Draw,
}

/// A move: claim cell `.0` (row-major index 0..9).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Move(pub usize);

/// The eight win lines; the index in this array is the line index.
pub const LINES: [[usize; 3]; 8] = [
    [0, 1, 2],
    [3, 4, 5],
    [6, 7, 8],
    [0, 3, 6],
    [1, 4, 7],
    [2, 5, 8],
    [0, 4, 8],
    [2, 4, 6],
];

/// 3x3 board plus side to move. `Copy`; 10 bytes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, Serialize, Deserialize)]
pub struct Board {
    cells: [Option<Player>; 9],
    to_move: Player,
}

impl Board {
    /// An empty board with `X` to move.
    pub fn empty() -> Board {
        Board {
            cells: [None; 9],
            to_move: Player::X,
        }
    }

    /// Construct a board directly from cell contents and side to move.
    pub fn from_cells(cells: [Option<Player>; 9], to_move: Player) -> Board {
        Board { cells, to_move }
    }

    /// The board's cells, row-major.
    pub fn cells(&self) -> &[Option<Player>; 9] {
        &self.cells
    }

    /// Occupant of `index`. Panics if `index >= 9`.
    pub fn cell(&self, index: usize) -> Option<Player> {
        self.cells[index]
    }

    /// Player whose turn it is.
    pub fn to_move(&self) -> Player {
        self.to_move
    }

    /// The winner, if any: the first line in `LINES` order held entirely by one player.
    pub fn winner(&self) -> Option<Player> {
        for line in LINES {
            let [a, b, c] = line;
            if let Some(p) = self.cells[a]
                && self.cells[b] == Some(p)
                && self.cells[c] == Some(p)
            {
                return Some(p);
            }
        }
        None
    }

    /// Whether every cell is occupied.
    pub fn is_full(&self) -> bool {
        self.cells.iter().all(Option::is_some)
    }

    /// A copy with `index` claimed by `player` and the side to move flipped. No legality checks.
    pub fn place(&self, index: usize, player: Player) -> Board {
        let mut cells = self.cells;
        cells[index] = Some(player);
        Board {
            cells,
            to_move: player.other(),
        }
    }

    /// Parse a board from a 9-character string over `{X, O, .}`, cell `i` at char `i`, row-major.
    /// Side to move is derived from counts: `X` to move when counts are equal, `O` to move when
    /// `X` has exactly one more than `O`. Any other length, character, or count is an error.
    pub fn parse(s: &str) -> Result<Board, String> {
        let chars: Vec<char> = s.chars().collect();
        if chars.len() != 9 {
            return Err(format!("expected 9 characters, got {}", chars.len()));
        }
        let mut cells = [None; 9];
        for (i, ch) in chars.iter().enumerate() {
            cells[i] = match ch {
                'X' => Some(Player::X),
                'O' => Some(Player::O),
                '.' => None,
                other => return Err(format!("invalid character '{other}' at position {i}")),
            };
        }
        let x_count = cells.iter().filter(|c| **c == Some(Player::X)).count();
        let o_count = cells.iter().filter(|c| **c == Some(Player::O)).count();
        let to_move = if x_count == o_count {
            Player::X
        } else if x_count == o_count + 1 {
            Player::O
        } else {
            return Err(format!("invalid cell counts: X={x_count}, O={o_count}"));
        };
        Ok(Board { cells, to_move })
    }
}

impl Default for Board {
    fn default() -> Board {
        Board::empty()
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for row in 0..3 {
            if row > 0 {
                writeln!(f)?;
            }
            for col in 0..3 {
                let ch = match self.cells[row * 3 + col] {
                    Some(Player::X) => 'X',
                    Some(Player::O) => 'O',
                    None => '.',
                };
                write!(f, "{ch}")?;
            }
        }
        Ok(())
    }
}

/// Marker type for the tic-tac-toe domain.
#[derive(Debug, Clone, Copy, Default)]
pub struct TicTacToe;

impl GameDomain for TicTacToe {
    type State = Board;
    type Action = Move;
    type Player = Player;
    type Outcome = Outcome;
}
