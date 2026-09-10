//! Tactical regression fixtures for the minimax engine, re-verified against a test-local
//! exhaustive oracle: the seven tactical fixtures (with depth-1/depth-2 sub-fixtures) plus
//! exhaustive depth-9 checks over all 5478 reachable positions (engine move in tie set; tie
//! set subset of the oracle's optimal-move set).

use game_player::minimax::search;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::sync::Arc;
use strategy_discovery::core::traits::{GameRules, StrategyProvider};
use strategy_discovery::games::tictactoe::{Board, Move, Outcome, Player, TicTacToe, TicTacToeEvaluator, TicTacToeRules};
use strategy_discovery::strategy::{AliceEvaluator, MinimaxConfig, MinimaxProvider, RulesResponseGenerator, TieBreak, root_values};

/// Exhaustive game-tree solution for one state: `value` is from the side-to-move's perspective
/// (`+1` win / `0` draw / `-1` loss; at a terminal `Win` the side to move is the loser),
/// `optimal` is every legal action achieving `value`, in ascending (legal-action) order.
#[derive(Clone)]
struct Solved {
    value: i8,
    optimal: Vec<Move>,
}

fn solve(rules: &TicTacToeRules, state: &Board, memo: &mut HashMap<Board, Solved>) -> Solved {
    if let Some(cached) = memo.get(state) {
        return cached.clone();
    }
    let solved = if let Some(outcome) = rules.outcome(state) {
        match outcome {
            Outcome::Win(_) => Solved {
                value: -1,
                optimal: Vec::new(),
            },
            Outcome::Draw => Solved {
                value: 0,
                optimal: Vec::new(),
            },
        }
    } else {
        let mut best_value = i8::MIN;
        let mut child_values: Vec<(Move, i8)> = Vec::new();
        for action in rules.legal_actions(state) {
            let child_state = rules.apply(state, &action).expect("legal action must apply");
            let child = solve(rules, &child_state, memo);
            let value = -child.value;
            child_values.push((action, value));
            best_value = best_value.max(value);
        }
        let optimal = child_values
            .into_iter()
            .filter(|(_, v)| *v == best_value)
            .map(|(a, _)| a)
            .collect();
        Solved {
            value: best_value,
            optimal,
        }
    };
    memo.insert(*state, solved.clone());
    solved
}

/// Fixture boards: `(id, board, expected depth-9 optimal-move cells)`, ascending within each row.
const FIXTURES: [(&str, &str, &[usize]); 7] = [
    ("win-x", "XX.OO....", &[2]),
    ("win-o", "XX.OO..X.", &[2, 5]),
    ("block-o", "XX.O.....", &[2, 4, 5, 6, 7, 8]),
    ("block-x", "X...OO.X.", &[3]),
    ("win-over-block", "XX.OO.X..", &[5]),
    ("fork-defence", "X...O...X", &[1, 3, 5, 7]),
    ("empty", ".........", &[0, 1, 2, 3, 4, 5, 6, 7, 8]),
];

fn board(s: &str) -> Board {
    Board::parse(s).unwrap()
}

fn moves(cells: &[usize]) -> Vec<Move> {
    cells.iter().map(|&c| Move(c)).collect()
}

/// Every legal move at `board` whose depth-`depth` Alice-perspective value ties the best value
/// for the side to move (the set `TieBreak::SeededUniform` picks from), in `legal` order.
fn tie_set(board: &Board, depth: u32) -> Vec<Move> {
    let rules = TicTacToeRules;
    let evaluator = TicTacToeEvaluator;
    let legal = rules.legal_actions(board);
    let values = root_values(&rules, &evaluator, board, &legal, depth);
    let best = if board.to_move() == Player::X {
        values.iter().map(|(_, v)| *v).max_by(f32::total_cmp)
    } else {
        values.iter().map(|(_, v)| *v).min_by(f32::total_cmp)
    }
    .expect("legal is non-empty");
    values
        .into_iter()
        .filter(|(_, v)| v.total_cmp(&best) == Ordering::Equal)
        .map(|(a, _)| a)
        .collect()
}

fn engine_move(board: &Board, depth: u32) -> Move {
    let rules = TicTacToeRules;
    let evaluator = TicTacToeEvaluator;
    let sef = AliceEvaluator::<TicTacToe>::new(&rules, &evaluator);
    let rg = RulesResponseGenerator::<TicTacToe>::new(&rules);
    search(&sef, &rg, board, depth).expect("non-terminal")
}

fn seeded_provider(depth: u32) -> MinimaxProvider<TicTacToe> {
    MinimaxProvider::new(
        Arc::new(TicTacToeRules),
        Arc::new(TicTacToeEvaluator),
        MinimaxConfig {
            depth,
            epsilon: 0.0,
            tie_break: TieBreak::SeededUniform,
        },
    )
    .expect("valid minimax config")
}

#[test]
fn fixture_table_matches_oracle() {
    let rules = TicTacToeRules;
    let mut memo = HashMap::new();
    for (id, board_str, expected) in FIXTURES {
        let b = board(board_str);
        let solved = solve(&rules, &b, &mut memo);
        assert_eq!(solved.optimal, moves(expected), "fixture {id}: oracle mismatch");
        let expected_side = if matches!(id, "win-x" | "block-x" | "empty") {
            Player::X
        } else {
            Player::O
        };
        assert_eq!(b.to_move(), expected_side, "fixture {id}: to_move mismatch");
    }
}

#[test]
fn fixture_tie_sets_at_depth_nine_match_expected() {
    for (id, board_str, expected) in FIXTURES {
        let b = board(board_str);
        assert_eq!(tie_set(&b, 9), moves(expected), "fixture {id}: tie set mismatch");
    }
}

#[test]
fn depth_one_solves_immediate_wins() {
    let cases = [
        ("XX.OO....", Move(2)), // win-x
        ("XX.OO..X.", Move(5)), // win-o
        ("XX.OO.X..", Move(5)), // win-over-block
    ];
    for (board_str, expected) in cases {
        let b = board(board_str);
        assert_eq!(tie_set(&b, 1), vec![expected], "board {board_str}");
        assert_eq!(engine_move(&b, 1), expected, "board {board_str}");
    }
}

#[test]
fn depth_two_solves_immediate_blocks() {
    let cases = [
        ("XX.O.....", Move(2)), // block-o
        ("X...OO.X.", Move(3)), // block-x
    ];
    for (board_str, expected) in cases {
        let b = board(board_str);
        assert_eq!(tie_set(&b, 2), vec![expected], "board {board_str}");
        assert_eq!(engine_move(&b, 2), expected, "board {board_str}");
    }
}

#[test]
fn seeded_uniform_strategy_chooses_within_expected_set() {
    let rules = TicTacToeRules;
    let provider = seeded_provider(9);
    for (id, board_str, expected) in FIXTURES {
        let b = board(board_str);
        let legal = rules.legal_actions(&b);
        let expected_moves = moves(expected);
        for seed in 0..8u64 {
            let mut strat = provider.create(seed);
            let m = strat
                .choose(&b, &legal)
                .unwrap_or_else(|e| panic!("fixture {id} seed {seed}: {e}"));
            assert!(
                expected_moves.contains(&m),
                "fixture {id} seed {seed}: {m:?} not in {expected_moves:?}"
            );
        }
    }
}

#[test]
fn engine_move_in_tie_set_for_all_positions_at_depth_nine() {
    let rules = TicTacToeRules;
    let positions = rules.reachable_positions();
    assert_eq!(positions.len(), 5478);

    let mut non_terminal_count = 0usize;
    let mut offenders: Vec<String> = Vec::new();
    for b in &positions {
        if rules.is_terminal(b) {
            continue;
        }
        non_terminal_count += 1;
        let ts = tie_set(b, 9);
        let em = engine_move(b, 9);
        if !ts.contains(&em) {
            offenders.push(format!("{b:?}: engine_move={em:?} tie_set={ts:?}"));
        }
    }
    assert_eq!(non_terminal_count, 4520);
    assert!(
        offenders.is_empty(),
        "engine move outside tie set for {} board(s):\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}

#[test]
fn tie_set_subset_of_oracle_for_all_positions_at_depth_nine() {
    let rules = TicTacToeRules;
    let positions = rules.reachable_positions();
    assert_eq!(positions.len(), 5478);

    let mut memo = HashMap::new();
    let mut non_terminal_count = 0usize;
    let mut offenders: Vec<String> = Vec::new();
    for b in &positions {
        if rules.is_terminal(b) {
            continue;
        }
        non_terminal_count += 1;
        let ts = tie_set(b, 9);
        let solved = solve(&rules, b, &mut memo);
        let oracle_optimal = &solved.optimal;
        let bad: Vec<Move> = ts.iter().copied().filter(|m| !oracle_optimal.contains(m)).collect();
        if !bad.is_empty() {
            offenders.push(format!(
                "{b:?}: tie_set={ts:?} oracle_optimal={oracle_optimal:?} not_in_oracle={bad:?}"
            ));
        }
    }
    assert_eq!(non_terminal_count, 4520);
    assert!(
        offenders.is_empty(),
        "tie set not subset of oracle optimal for {} board(s):\n{}",
        offenders.len(),
        offenders.join("\n")
    );
}
