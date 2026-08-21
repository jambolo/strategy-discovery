//! Replays an action list through a game's rules to reconstruct the sequence of positions it
//! passed through. The single source of truth for turning a persisted action list back into
//! `(state_before, action)` pairs.

use crate::core::traits::{GameDomain, GameRules, RulesError};

/// Rebuilds `(state_before, action)` pairs by applying `actions` from `rules.initial_state()`;
/// the single source of truth for turning an action list into positions.
#[allow(clippy::type_complexity)]
pub fn replay<G: GameDomain>(rules: &dyn GameRules<G>, actions: &[G::Action]) -> Result<Vec<(G::State, G::Action)>, RulesError> {
    let mut state = rules.initial_state();
    let mut pairs = Vec::with_capacity(actions.len());
    for action in actions {
        let next = rules.apply(&state, action)?;
        pairs.push((state, action.clone()));
        state = next;
    }
    Ok(pairs)
}

/// State after applying every action in `actions` from the initial state.
pub fn final_state<G: GameDomain>(rules: &dyn GameRules<G>, actions: &[G::Action]) -> Result<G::State, RulesError> {
    let mut state = rules.initial_state();
    for action in actions {
        state = rules.apply(&state, action)?;
    }
    Ok(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::tictactoe::{Board, Move, Outcome, Player, TicTacToeRules};

    #[test]
    fn replay_reproduces_a_known_game() {
        let rules = TicTacToeRules;
        let actions = vec![Move(4), Move(0), Move(1), Move(2), Move(7)];
        let pairs = replay(&rules, &actions).unwrap();
        assert_eq!(pairs.len(), 5);
        assert_eq!(pairs[0].0, Board::empty());
        for (i, action) in actions.iter().enumerate() {
            assert_eq!(&pairs[i].1, action);
        }
        for i in 1..pairs.len() {
            let expected = rules.apply(&pairs[i - 1].0, &pairs[i - 1].1).unwrap();
            assert_eq!(pairs[i].0, expected);
        }
        let final_board = final_state(&rules, &actions).unwrap();
        assert_eq!(final_board.winner(), Some(Player::X));
        assert_eq!(rules.outcome(&final_board), Some(Outcome::Win(Player::X)));
    }

    #[test]
    fn replay_of_empty_list_is_initial_state() {
        let rules = TicTacToeRules;
        assert!(replay(&rules, &[]).unwrap().is_empty());
        assert_eq!(final_state(&rules, &[]).unwrap(), Board::empty());
    }

    #[test]
    fn replay_rejects_illegal_sequences() {
        let rules = TicTacToeRules;

        let occupied = [Move(4), Move(4)];
        match replay(&rules, &occupied) {
            Err(RulesError::IllegalAction(_)) => {}
            other => panic!("expected IllegalAction, got {other:?}"),
        }
        match final_state(&rules, &occupied) {
            Err(RulesError::IllegalAction(_)) => {}
            other => panic!("expected IllegalAction, got {other:?}"),
        }

        let finished = [Move(0), Move(3), Move(1), Move(4), Move(2), Move(5)];
        match replay(&rules, &finished) {
            Err(RulesError::GameOver) => {}
            other => panic!("expected GameOver, got {other:?}"),
        }
        match final_state(&rules, &finished) {
            Err(RulesError::GameOver) => {}
            other => panic!("expected GameOver, got {other:?}"),
        }
    }
}
