//! Exhaustive negamax solver with memoization, per the step spec: `value` is from the
//! side-to-move perspective (+1 side to move wins, 0 draw, -1 side to move loses).

use std::collections::HashMap;
use strategy_discovery::core::traits::GameRules;
use strategy_discovery::games::tictactoe::{Board, Move, Outcome, TicTacToeRules};

/// Solved value and optimal replies for one position.
#[derive(Debug, Clone)]
pub struct Solved {
    pub value: i8,
    #[allow(dead_code)]
    pub optimal: Vec<Move>,
}

/// Solves `state` (and, transitively, every position reachable from it), memoizing in `memo`.
pub fn solve(rules: &TicTacToeRules, memo: &mut HashMap<Board, Solved>, state: Board) -> Solved {
    if let Some(solved) = memo.get(&state) {
        return solved.clone();
    }

    if let Some(outcome) = rules.outcome(&state) {
        let value = match outcome {
            Outcome::Win(_) => -1,
            Outcome::Draw => 0,
        };
        let solved = Solved {
            value,
            optimal: Vec::new(),
        };
        memo.insert(state, solved.clone());
        return solved;
    }

    let mut child_values: Vec<(Move, i8)> = Vec::new();
    for action in rules.legal_actions(&state) {
        let next = rules.apply(&state, &action).expect("legal action must apply");
        let child = solve(rules, memo, next);
        child_values.push((action, -child.value));
    }

    let best_value = child_values
        .iter()
        .map(|(_, v)| *v)
        .max()
        .expect("non-terminal state has legal actions");
    let optimal: Vec<Move> = child_values
        .into_iter()
        .filter(|(_, v)| *v == best_value)
        .map(|(a, _)| a)
        .collect();

    let solved = Solved {
        value: best_value,
        optimal,
    };
    memo.insert(state, solved.clone());
    solved
}
