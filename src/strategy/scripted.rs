//! Scripted strategy: replays a fixed action list. Debug/test aid; not spec-constructible.

use crate::core::kinds;
use crate::core::traits::{GameDomain, Strategy, StrategyError, StrategyProvider};

/// Provider for [`ScriptedStrategy`]; kind `"scripted"`. Every created strategy replays the
/// same action list from the start; the seed is ignored.
#[derive(Debug, Clone)]
pub struct ScriptedProvider<G: GameDomain> {
    actions: Vec<G::Action>,
}

impl<G: GameDomain> ScriptedProvider<G> {
    /// Provider replaying `actions` in order (this side's moves only).
    pub fn new(actions: Vec<G::Action>) -> Self {
        Self { actions }
    }

    /// The scripted actions.
    pub fn actions(&self) -> &[G::Action] {
        &self.actions
    }
}

impl<G: GameDomain> StrategyProvider<G> for ScriptedProvider<G> {
    fn kind(&self) -> &str {
        kinds::SCRIPTED
    }

    fn create(&self, _seed: u64) -> Box<dyn Strategy<G>> {
        Box::new(ScriptedStrategy::<G>::new(self.actions.clone()))
    }
}

/// Replays a script: returns the next action if it is legal, otherwise an error. The cursor
/// advances only on success.
#[derive(Debug, Clone)]
pub struct ScriptedStrategy<G: GameDomain> {
    actions: Vec<G::Action>,
    cursor: usize,
}

impl<G: GameDomain> ScriptedStrategy<G> {
    /// Strategy replaying `actions` from the first entry.
    pub fn new(actions: Vec<G::Action>) -> Self {
        Self { actions, cursor: 0 }
    }

    /// Number of actions already replayed.
    pub fn position(&self) -> usize {
        self.cursor
    }
}

impl<G: GameDomain> Strategy<G> for ScriptedStrategy<G> {
    fn choose(&mut self, _state: &G::State, legal: &[G::Action]) -> Result<G::Action, StrategyError> {
        if legal.is_empty() {
            return Err(StrategyError::NoLegalActions);
        }
        let Some(action) = self.actions.get(self.cursor) else {
            return Err(StrategyError::Other(format!(
                "script exhausted after {} actions",
                self.actions.len()
            )));
        };
        if !legal.contains(action) {
            return Err(StrategyError::Other(format!(
                "scripted action {action:?} is not legal at ply {}",
                self.cursor
            )));
        }
        self.cursor += 1;
        Ok(action.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::GameRules;
    use crate::games::tictactoe::{Board, Move, Outcome, Player, TicTacToe, TicTacToeRules};

    #[test]
    fn provider_kind_is_scripted() {
        assert_eq!(ScriptedProvider::<TicTacToe>::new(vec![]).kind(), "scripted");
    }

    #[test]
    fn scripted_game_replays_exactly() {
        let rules = TicTacToeRules;
        let x_script = vec![Move(4), Move(2), Move(3), Move(1), Move(8)];
        let o_script = vec![Move(0), Move(6), Move(5), Move(7)];
        let mut x = ScriptedStrategy::<TicTacToe>::new(x_script);
        let mut o = ScriptedStrategy::<TicTacToe>::new(o_script);

        let mut state = rules.initial_state();
        let mut played = Vec::new();
        loop {
            let legal = rules.legal_actions(&state);
            if legal.is_empty() {
                break;
            }
            let action = match rules.player_to_move(&state) {
                Player::X => x.choose(&state, &legal).unwrap(),
                Player::O => o.choose(&state, &legal).unwrap(),
            };
            played.push(action);
            state = rules.apply(&state, &action).unwrap();
        }

        assert_eq!(
            played,
            vec![
                Move(4),
                Move(0),
                Move(2),
                Move(6),
                Move(3),
                Move(5),
                Move(1),
                Move(7),
                Move(8),
            ]
        );
        assert_eq!(rules.outcome(&state), Some(Outcome::Draw));
        assert_eq!(state, Board::parse("OXXXXOOOX").unwrap());

        let err = x.choose(&state, &[Move(0)]).unwrap_err();
        match err {
            StrategyError::Other(m) => assert_eq!(m, "script exhausted after 5 actions"),
            other => panic!("expected Other, got {other:?}"),
        }
    }

    #[test]
    fn illegal_scripted_action_is_an_error() {
        let rules = TicTacToeRules;
        let board = Board::parse("X........").unwrap();
        let mut strategy = ScriptedStrategy::<TicTacToe>::new(vec![Move(0)]);
        let legal = rules.legal_actions(&board);

        let err = strategy.choose(&board, &legal).unwrap_err();
        match &err {
            StrategyError::Other(m) => {
                assert_eq!(m, "scripted action Move(0) is not legal at ply 0")
            }
            other => panic!("expected Other, got {other:?}"),
        }
        assert_eq!(strategy.position(), 0);

        let err_again = strategy.choose(&board, &legal).unwrap_err();
        assert_eq!(err_again, err);
        assert_eq!(strategy.position(), 0);
    }

    #[test]
    fn empty_legal_is_no_legal_actions() {
        let mut strategy = ScriptedStrategy::<TicTacToe>::new(vec![]);
        let err = strategy.choose(&Board::empty(), &[]).unwrap_err();
        assert_eq!(err, StrategyError::NoLegalActions);
    }

    #[test]
    fn seed_is_ignored() {
        let provider = ScriptedProvider::<TicTacToe>::new(vec![Move(4), Move(0)]);
        let rules = TicTacToeRules;
        let board = Board::empty();
        let legal = rules.legal_actions(&board);

        let mut s1 = provider.create(1);
        let mut s2 = provider.create(2);
        assert_eq!(s1.choose(&board, &legal).unwrap(), s2.choose(&board, &legal).unwrap());
    }

    #[test]
    fn cursor_advances_only_on_success() {
        let mut strategy = ScriptedStrategy::<TicTacToe>::new(vec![Move(4), Move(0)]);
        let board = Board::empty();
        let rules = TicTacToeRules;
        let legal = rules.legal_actions(&board);

        assert_eq!(strategy.choose(&board, &legal).unwrap(), Move(4));
        assert_eq!(strategy.position(), 1);

        let err = strategy.choose(&board, &[Move(1)]);
        assert!(err.is_err());
        assert_eq!(strategy.position(), 1);
    }
}
