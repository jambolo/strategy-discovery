//! Game-agnostic glue between the framework's `GameRules`/`StateEvaluator` and the
//! `game-player` minimax engine. The framework owns terminal values (`WIN_VALUE`); a
//! game's `StateEvaluator` supplies only the non-terminal heuristic.

use crate::core::traits::{GameDomain, GameRules, StateEvaluator};
use game_player::minimax::ResponseGenerator;
use game_player::{PlayerId, State as EngineState, StaticEvaluator};

/// Engine value of a won game from the winner's perspective. Alice (maximizing side) wins
/// at `+WIN_VALUE`, Bob at `-WIN_VALUE`, a draw is `0.0`. Heuristic values are clamped to
/// `(-WIN_VALUE, WIN_VALUE)` exclusive so they can never be mistaken for a terminal result.
pub const WIN_VALUE: f32 = 100.0;

/// The only game-side glue `game-player` needs: mapping between the game's player type and
/// `PlayerId`, and extracting the winner from an outcome. The associated-type bound makes
/// `G::State` usable as a `game_player::State` wherever `G: EngineGame`.
pub trait EngineGame: GameDomain<State: EngineState<Action = <Self as GameDomain>::Action>> {
    /// Engine identity of `player`. The player mapped to `PlayerId::Alice` is maximized.
    fn player_id(player: Self::Player) -> PlayerId;
    /// Inverse of [`EngineGame::player_id`].
    fn player_from_id(id: PlayerId) -> Self::Player;
    /// Winner recorded in a terminal outcome, `None` for a draw.
    fn winner(outcome: &Self::Outcome) -> Option<Self::Player>;
}

/// `ResponseGenerator` over a `GameRules`: `generate` is exactly `legal_actions`, which is
/// empty iff the state is terminal (the policy `game-player` requires).
pub struct RulesResponseGenerator<'a, G: GameDomain> {
    rules: &'a dyn GameRules<G>,
}

impl<'a, G: GameDomain> RulesResponseGenerator<'a, G> {
    /// Wrap `rules`.
    pub fn new(rules: &'a dyn GameRules<G>) -> Self {
        Self { rules }
    }
}

impl<G: EngineGame> ResponseGenerator for RulesResponseGenerator<'_, G> {
    type State = G::State;

    fn generate(&self, state: &G::State, _depth: u32) -> Vec<G::Action> {
        self.rules.legal_actions(state)
    }
}

/// `StaticEvaluator` from Alice's perspective: terminal states map to `±WIN_VALUE` / `0.0`
/// via the rules' outcome; non-terminal states use the game's `StateEvaluator` evaluated
/// for the Alice player and clamped to `[-(WIN_VALUE - 1), WIN_VALUE - 1]`.
pub struct AliceEvaluator<'a, G: GameDomain> {
    rules: &'a dyn GameRules<G>,
    evaluator: &'a dyn StateEvaluator<G>,
}

impl<'a, G: GameDomain> AliceEvaluator<'a, G> {
    /// Wrap `rules` and `evaluator`.
    pub fn new(rules: &'a dyn GameRules<G>, evaluator: &'a dyn StateEvaluator<G>) -> Self {
        Self { rules, evaluator }
    }
}

impl<G: EngineGame> StaticEvaluator for AliceEvaluator<'_, G> {
    type State = G::State;

    fn evaluate(&self, state: &G::State) -> f32 {
        match self.rules.outcome(state) {
            Some(outcome) => match G::winner(&outcome) {
                Some(p) if G::player_id(p) == PlayerId::Alice => WIN_VALUE,
                Some(_) => -WIN_VALUE,
                None => 0.0,
            },
            None => self
                .evaluator
                .evaluate(state, G::player_from_id(PlayerId::Alice))
                .clamp(-(WIN_VALUE - 1.0), WIN_VALUE - 1.0),
        }
    }

    fn alice_wins_value(&self) -> f32 {
        WIN_VALUE
    }

    fn bob_wins_value(&self) -> f32 {
        -WIN_VALUE
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::{GameRules, StateEvaluator};
    use crate::games::tictactoe::{Board, Move, Player, TicTacToe, TicTacToeRules};
    use game_player::minimax::search;

    struct ConstEval(f32);
    impl StateEvaluator<TicTacToe> for ConstEval {
        fn evaluate(&self, _: &Board, _: Player) -> f32 {
            self.0
        }
    }

    struct SideEval;
    impl StateEvaluator<TicTacToe> for SideEval {
        fn evaluate(&self, _: &Board, p: Player) -> f32 {
            if p == Player::X { 1.0 } else { -1.0 }
        }
    }

    #[test]
    fn win_value_constants() {
        let eval = ConstEval(0.0);
        let alice_eval = AliceEvaluator::<TicTacToe>::new(&TicTacToeRules, &eval);
        assert_eq!(alice_eval.alice_wins_value(), 100.0);
        assert_eq!(alice_eval.bob_wins_value(), -100.0);
        assert_eq!(WIN_VALUE, 100.0);
    }

    #[test]
    fn alice_evaluator_terminal_values() {
        let eval = ConstEval(0.0);
        let alice_eval = AliceEvaluator::<TicTacToe>::new(&TicTacToeRules, &eval);
        assert_eq!(alice_eval.evaluate(&Board::parse("XXXOO....").unwrap()), 100.0);
        assert_eq!(alice_eval.evaluate(&Board::parse("XX.OOO.X.").unwrap()), -100.0);
        assert_eq!(alice_eval.evaluate(&Board::parse("XOXXOOOXX").unwrap()), 0.0);
    }

    #[test]
    fn alice_evaluator_clamps_heuristic() {
        let board = Board::empty();

        let eval = ConstEval(1000.0);
        let alice_eval = AliceEvaluator::<TicTacToe>::new(&TicTacToeRules, &eval);
        assert_eq!(alice_eval.evaluate(&board), 99.0);

        let eval = ConstEval(-1000.0);
        let alice_eval = AliceEvaluator::<TicTacToe>::new(&TicTacToeRules, &eval);
        assert_eq!(alice_eval.evaluate(&board), -99.0);

        let eval = ConstEval(3.5);
        let alice_eval = AliceEvaluator::<TicTacToe>::new(&TicTacToeRules, &eval);
        assert_eq!(alice_eval.evaluate(&board), 3.5);

        let eval = ConstEval(-99.0);
        let alice_eval = AliceEvaluator::<TicTacToe>::new(&TicTacToeRules, &eval);
        assert_eq!(alice_eval.evaluate(&board), -99.0);
    }

    #[test]
    fn alice_evaluator_uses_alice_perspective() {
        let eval = SideEval;
        let alice_eval = AliceEvaluator::<TicTacToe>::new(&TicTacToeRules, &eval);
        assert_eq!(alice_eval.evaluate(&Board::empty()), 1.0);
    }

    #[test]
    fn response_generator_empty_iff_terminal_over_all_positions() {
        let rules = TicTacToeRules;
        let rg = RulesResponseGenerator::<TicTacToe>::new(&rules);
        let positions = rules.reachable_positions();
        assert_eq!(positions.len(), 5478);
        for b in positions {
            assert_eq!(rg.generate(&b, 1).is_empty(), rules.is_terminal(&b));
            assert_eq!(rg.generate(&b, 1), rules.legal_actions(&b));
        }
    }

    #[test]
    fn search_with_adapters_takes_immediate_win() {
        let rules = TicTacToeRules;
        let eval = ConstEval(0.0);
        let sef = AliceEvaluator::<TicTacToe>::new(&rules, &eval);
        let rg = RulesResponseGenerator::<TicTacToe>::new(&rules);

        assert_eq!(search(&sef, &rg, &Board::parse("XX.OO....").unwrap(), 1), Some(Move(2)));
        assert_eq!(search(&sef, &rg, &Board::parse("XXXOO....").unwrap(), 1), None);
    }
}
