//! Game-agnostic exhaustive solver: memoized negamax over [`GameRules`], value from the
//! side-to-move perspective (ADR 0001). Exhaustive enumeration is a small-game accelerant —
//! it walks the entire reachable state space and holds it in memory, so it only suits games
//! whose state space is small; larger games must sample instead.

use crate::core::traits::{GameDomain, GameRules};
use crate::strategy::engine::EngineGame;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use thiserror::Error;

/// Result of solving one state: its value and every action that achieves it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Solved<A> {
    /// Value from the perspective of the player to move in the solved state: `1` win, `0`
    /// draw, `-1` loss, assuming optimal play by both sides.
    pub value: i8,
    /// Every legal action achieving `value`, in `legal_actions` order. Empty for terminal
    /// states.
    pub optimal: Vec<A>,
}

/// Errors produced by the exhaustive solver.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SolverError {
    /// The reachable state space exceeds the solver's configured limit. Exhaustive modes are
    /// a small-game accelerant: larger games must fall back to sampling instead.
    #[error(
        "state space exceeds the exhaustive-solver limit of {limit} states; exhaustive modes are a small-game accelerant — larger games must sample"
    )]
    TooLarge {
        /// The configured limit that was exceeded.
        limit: usize,
    },
}

/// Memoized negamax solver over a game's [`GameRules`]. Exhaustive: it stores every state it
/// visits, so it is intended for small state spaces (see [`SolverError::TooLarge`]).
pub struct ExhaustiveSolver<G: EngineGame> {
    /// Rules of the game being solved.
    rules: Arc<dyn GameRules<G>>,
    /// States solved so far, keyed by state. Lookup-only: never iterated to produce output.
    memo: HashMap<G::State, Solved<G::Action>>,
    /// Maximum number of states this solver will memoize before failing with
    /// [`SolverError::TooLarge`].
    limit: usize,
}

impl<G: EngineGame> ExhaustiveSolver<G> {
    /// Default memoization limit used when callers don't need a tighter bound.
    pub const DEFAULT_LIMIT: usize = 1_000_000;

    /// Builds a solver over `rules` that will memoize at most `limit` states.
    pub fn new(rules: Arc<dyn GameRules<G>>, limit: usize) -> Self {
        Self {
            rules,
            memo: HashMap::new(),
            limit,
        }
    }

    /// Solves `state`: its value from the side-to-move perspective and every optimal action.
    /// Memoized; recurses into every reachable successor. Fails with
    /// [`SolverError::TooLarge`] if solving would memoize more than `limit` states.
    pub fn solve(&mut self, state: &G::State) -> Result<Solved<G::Action>, SolverError> {
        if let Some(solved) = self.memo.get(state) {
            return Ok(solved.clone());
        }

        let solved = if let Some(outcome) = self.rules.outcome(state) {
            let value = match G::winner(&outcome) {
                Some(winner) if winner == self.rules.player_to_move(state) => 1,
                Some(_) => -1,
                None => 0,
            };
            Solved {
                value,
                optimal: Vec::new(),
            }
        } else {
            let mut valued_actions = Vec::new();
            for action in self.rules.legal_actions(state) {
                let next = self.rules.apply(state, &action).expect("legal action must apply");
                let value = -self.solve(&next)?.value;
                valued_actions.push((action, value));
            }
            let best = valued_actions
                .iter()
                .map(|(_, v)| *v)
                .max()
                .expect("non-terminal state has legal actions");
            let optimal = valued_actions
                .into_iter()
                .filter(|(_, v)| *v == best)
                .map(|(a, _)| a)
                .collect();
            Solved { value: best, optimal }
        };

        if self.memo.len() >= self.limit {
            return Err(SolverError::TooLarge { limit: self.limit });
        }
        self.memo.insert(state.clone(), solved.clone());
        Ok(solved)
    }

    /// Number of states memoized so far.
    pub fn solved_states(&self) -> usize {
        self.memo.len()
    }

    /// The configured memoization limit.
    pub fn limit(&self) -> usize {
        self.limit
    }
}

/// Every state reachable from `rules.initial_state()` by breadth-first traversal over
/// `legal_actions`, deduplicated, in first-discovery order (terminals included, never
/// expanded). Fails with [`SolverError::TooLarge`] if more than `limit` states are reachable.
pub fn reachable_states<G: GameDomain>(rules: &dyn GameRules<G>, limit: usize) -> Result<Vec<G::State>, SolverError> {
    let start = rules.initial_state();
    let mut seen: HashSet<G::State> = HashSet::new();
    seen.insert(start.clone());
    let mut order: Vec<G::State> = Vec::new();
    if order.len() >= limit {
        return Err(SolverError::TooLarge { limit });
    }
    order.push(start.clone());
    let mut frontier = vec![start];
    while !frontier.is_empty() {
        let mut next_frontier = Vec::new();
        for state in frontier {
            for action in rules.legal_actions(&state) {
                let next = rules.apply(&state, &action).expect("legal action must apply");
                if seen.insert(next.clone()) {
                    if order.len() >= limit {
                        return Err(SolverError::TooLarge { limit });
                    }
                    order.push(next.clone());
                    next_frontier.push(next);
                }
            }
        }
        frontier = next_frontier;
    }
    Ok(order)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::tictactoe::{Board, Move, Player, TicTacToe, TicTacToeRules};

    fn solver(limit: usize) -> ExhaustiveSolver<TicTacToe> {
        ExhaustiveSolver::new(Arc::new(TicTacToeRules), limit)
    }

    fn label(to_move: Player, value: i8) -> &'static str {
        match (to_move, value) {
            (Player::X, 1) | (Player::O, -1) => "X wins",
            (Player::O, 1) | (Player::X, -1) => "O wins",
            _ => "draw",
        }
    }

    #[test]
    fn solver_facts_on_fixture_boards() {
        let mut es = solver(ExhaustiveSolver::<TicTacToe>::DEFAULT_LIMIT);

        let empty = Board::empty();
        let solved = es.solve(&empty).unwrap();
        assert_eq!(solved.value, 0);
        assert_eq!(solved.optimal, (0..9).map(Move).collect::<Vec<_>>());

        let solved = es.solve(&Board::parse("XX.OO....").unwrap()).unwrap();
        assert_eq!(solved.value, 1);
        assert_eq!(solved.optimal, vec![Move(2)]);

        let solved = es.solve(&Board::parse("XX.O.....").unwrap()).unwrap();
        assert_eq!(solved.value, -1);
        assert_eq!(solved.optimal, vec![Move(2), Move(4), Move(5), Move(6), Move(7), Move(8)]);

        let solved = es.solve(&Board::parse("X...O...X").unwrap()).unwrap();
        assert_eq!(solved.value, 0);
        assert_eq!(solved.optimal, vec![Move(1), Move(3), Move(5), Move(7)]);

        let solved = es.solve(&Board::parse("XXXOO....").unwrap()).unwrap();
        assert_eq!(solved.value, -1);
        assert_eq!(solved.optimal, Vec::<Move>::new());

        let solved = es.solve(&Board::parse("XOXXOOOXX").unwrap()).unwrap();
        assert_eq!(solved.value, 0);
        assert_eq!(solved.optimal, Vec::<Move>::new());
    }

    #[test]
    fn solving_from_empty_memoizes_all_reachable_states_with_known_label_distribution() {
        let mut es = solver(ExhaustiveSolver::<TicTacToe>::DEFAULT_LIMIT);
        es.solve(&Board::empty()).unwrap();
        assert_eq!(es.solved_states(), 5478);

        let rules = TicTacToeRules;
        let reachable = reachable_states(&rules, ExhaustiveSolver::<TicTacToe>::DEFAULT_LIMIT).unwrap();

        let (mut all_x, mut all_draw, mut all_o) = (0, 0, 0);
        let (mut nt_x, mut nt_draw, mut nt_o) = (0, 0, 0);
        for state in &reachable {
            let solved = es.solve(state).unwrap();
            let lbl = label(state.to_move(), solved.value);
            let is_terminal = rules.is_terminal(state);
            match lbl {
                "X wins" => {
                    all_x += 1;
                    if !is_terminal {
                        nt_x += 1;
                    }
                }
                "draw" => {
                    all_draw += 1;
                    if !is_terminal {
                        nt_draw += 1;
                    }
                }
                "O wins" => {
                    all_o += 1;
                    if !is_terminal {
                        nt_o += 1;
                    }
                }
                other => unreachable!("unexpected label {other}"),
            }
        }

        assert_eq!((all_x, all_draw, all_o), (2936, 1068, 1474));
        assert_eq!((nt_x, nt_draw, nt_o), (2310, 1052, 1158));
        assert_eq!(es.solved_states(), 5478);
    }

    #[test]
    fn reachable_states_matches_tictactoe_reachable_positions() {
        let rules = TicTacToeRules;
        let via_solver = reachable_states(&rules, 1_000_000).unwrap();
        let via_rules = rules.reachable_positions();
        assert_eq!(via_solver, via_rules);
        assert_eq!(via_solver.len(), 5478);
        let terminal_count = via_solver.iter().filter(|state| rules.is_terminal(state)).count();
        assert_eq!(terminal_count, 958);
    }

    #[test]
    fn too_large_is_an_error() {
        let mut es = solver(10);
        assert_eq!(es.solve(&Board::empty()), Err(SolverError::TooLarge { limit: 10 }));

        let rules = TicTacToeRules;
        assert_eq!(reachable_states(&rules, 10), Err(SolverError::TooLarge { limit: 10 }));

        let mut es_full = solver(5478);
        assert!(es_full.solve(&Board::empty()).is_ok());
        assert_eq!(es_full.solved_states(), 5478);

        let full = reachable_states(&rules, 5478).unwrap();
        assert_eq!(full.len(), 5478);

        assert_eq!(reachable_states(&rules, 5477), Err(SolverError::TooLarge { limit: 5477 }));
    }

    #[test]
    fn too_large_message_flags_the_accelerant() {
        let message = SolverError::TooLarge { limit: 7 }.to_string();
        assert!(message.contains("limit of 7 states"));
        assert!(message.contains("small-game accelerant"));
    }
}
