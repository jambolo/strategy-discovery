//! Framework contracts. Games implement these; pipeline layers consume only these.

use crate::core::features::{FeatureDef, FeatureVector};
use crate::core::symmetry::{Permutation, SymmetryGroup};
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;
use thiserror::Error;

/// Type-level description of one game: a marker type carrying the state/action/player/outcome types.
pub trait GameDomain: Send + Sync + 'static {
    /// Game state. Must be cheap to clone and share no mutable state across parallel games.
    type State: Clone + Eq + Hash + Debug + Send + Sync + 'static;
    /// A single move.
    type Action: Clone + Eq + Hash + Debug + Send + Sync + 'static;
    /// A player identity.
    type Player: Copy + Eq + Hash + Debug + Send + Sync + 'static;
    /// Terminal result of a completed game.
    type Outcome: Clone + Eq + Debug + Send + Sync + 'static;
}

/// Errors produced while applying game rules.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum RulesError {
    /// The game has already reached a terminal state.
    #[error("game is over; no further actions accepted")]
    GameOver,
    /// The requested action is not legal in the given state.
    #[error("illegal action: {0}")]
    IllegalAction(String),
}

/// Rules of play: initial state, legality, transitions, termination.
pub trait GameRules<G: GameDomain>: Send + Sync {
    /// The state a game begins in.
    fn initial_state(&self) -> G::State;
    /// Player whose turn it is (defined for terminal states too; value is then irrelevant).
    fn player_to_move(&self, state: &G::State) -> G::Player;
    /// Empty iff `state` is terminal.
    fn legal_actions(&self, state: &G::State) -> Vec<G::Action>;
    /// `Err(GameOver)` on a terminal state; `Err(IllegalAction)` if `action` is not legal.
    fn apply(&self, state: &G::State, action: &G::Action) -> Result<G::State, RulesError>;
    /// `Some` iff `state` is terminal.
    fn outcome(&self, state: &G::State) -> Option<G::Outcome>;
    /// Whether `state` is terminal.
    fn is_terminal(&self, state: &G::State) -> bool {
        self.outcome(state).is_some()
    }
}

/// Structural facts a game declares with no strategic insight. Positions are `0..position_count()`.
pub trait GamePrimitives<G: GameDomain>: Send + Sync {
    /// Number of addressable positions.
    fn position_count(&self) -> usize;
    /// Neighbours of `position` in the board topology (ascending, no duplicates).
    fn adjacent(&self, position: usize) -> Vec<usize>;
    /// Win-condition lines as position lists. Order is stable and is the line index used by derived features.
    fn lines(&self) -> Vec<Vec<usize>>;
    /// Symmetry group acting on positions; its degree must equal `position_count()`.
    fn symmetry_group(&self) -> SymmetryGroup;
    /// Occupant of `position` in `state`, `None` if empty.
    fn occupant(&self, state: &G::State, position: usize) -> Option<G::Player>;
    /// Position an action targets, if the game's actions address positions.
    fn action_position(&self, action: &G::Action) -> Option<usize>;
    /// Image of `state` under `perm`: the occupant of position `p` moves to `perm.apply(p)`;
    /// player-to-move and all other state are unchanged.
    fn transform(&self, state: &G::State, perm: &Permutation) -> G::State;
}

/// Delegation so shared `Arc<dyn GameRules<G>>` handles satisfy generic bounds.
impl<G: GameDomain, T: GameRules<G> + ?Sized> GameRules<G> for Arc<T> {
    fn initial_state(&self) -> G::State {
        (**self).initial_state()
    }
    fn player_to_move(&self, state: &G::State) -> G::Player {
        (**self).player_to_move(state)
    }
    fn legal_actions(&self, state: &G::State) -> Vec<G::Action> {
        (**self).legal_actions(state)
    }
    fn apply(&self, state: &G::State, action: &G::Action) -> Result<G::State, RulesError> {
        (**self).apply(state, action)
    }
    fn outcome(&self, state: &G::State) -> Option<G::Outcome> {
        (**self).outcome(state)
    }
    fn is_terminal(&self, state: &G::State) -> bool {
        (**self).is_terminal(state)
    }
}

/// Delegation so shared `Arc<dyn GamePrimitives<G>>` handles satisfy generic bounds.
impl<G: GameDomain, T: GamePrimitives<G> + ?Sized> GamePrimitives<G> for Arc<T> {
    fn position_count(&self) -> usize {
        (**self).position_count()
    }
    fn adjacent(&self, position: usize) -> Vec<usize> {
        (**self).adjacent(position)
    }
    fn lines(&self) -> Vec<Vec<usize>> {
        (**self).lines()
    }
    fn symmetry_group(&self) -> SymmetryGroup {
        (**self).symmetry_group()
    }
    fn occupant(&self, state: &G::State, position: usize) -> Option<G::Player> {
        (**self).occupant(state, position)
    }
    fn action_position(&self, action: &G::Action) -> Option<usize> {
        (**self).action_position(action)
    }
    fn transform(&self, state: &G::State, perm: &Permutation) -> G::State {
        (**self).transform(state, perm)
    }
}

/// State → named feature values over the tiered vocabulary.
pub trait FeatureExtractor<G: GameDomain>: Send + Sync {
    /// Definitions of every feature this extractor produces, in output order.
    fn definitions(&self) -> Vec<FeatureDef>;
    /// Values for every name in `definitions()`.
    fn extract(&self, state: &G::State) -> FeatureVector;
}

/// Optional: canonical representative of a state under the symmetry group.
pub trait Canonicalize<G: GameDomain>: Send + Sync {
    /// Returns `(canonical, p)` with `primitives.transform(state, &p) == canonical`.
    /// Must be idempotent: canonicalizing a canonical state returns it with the identity.
    fn canonicalize_with_transform(&self, state: &G::State) -> (G::State, Permutation);
    /// Canonical representative of `state`, discarding the transform.
    fn canonicalize(&self, state: &G::State) -> G::State {
        self.canonicalize_with_transform(state).0
    }
}

/// Static evaluation for search engines. Higher is better for `perspective`.
pub trait StateEvaluator<G: GameDomain>: Send + Sync {
    /// Evaluate `state` from `perspective`'s point of view.
    fn evaluate(&self, state: &G::State, perspective: G::Player) -> f32;
}

/// Errors produced while choosing an action.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StrategyError {
    /// No legal actions were available to choose from.
    #[error("no legal actions available")]
    NoLegalActions,
    /// The requested strategy kind exists but is not yet implemented.
    #[error("strategy `{kind}` is not implemented: {detail}")]
    Unimplemented {
        /// Registered strategy kind name.
        kind: String,
        /// Human-readable detail.
        detail: String,
    },
    /// Any other strategy failure.
    #[error("{0}")]
    Other(String),
}

/// Chooses an action. One instance per game; `&mut self` allows seeded internal RNG state.
pub trait Strategy<G: GameDomain>: Send {
    /// `legal` is exactly `rules.legal_actions(state)`; the result must be one of its elements.
    fn choose(&mut self, state: &G::State, legal: &[G::Action]) -> Result<G::Action, StrategyError>;
}

/// Registry-facing factory: creates fresh, independently seeded strategy instances.
pub trait StrategyProvider<G: GameDomain>: Send + Sync {
    /// Kind name as registered (see `crate::core::kinds`).
    fn kind(&self) -> &str;
    /// Create a fresh strategy instance seeded with `seed`.
    fn create(&self, seed: u64) -> Box<dyn Strategy<G>>;
}

/// Errors produced while generating candidate strategies.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum GeneratorError {
    /// Generation failed for the given reason.
    #[error("strategy generation failed: {0}")]
    Failed(String),
}

/// Produces candidate strategies (corpus mining, evolutionary search, LLM generation, ...).
pub trait StrategyGenerator<G: GameDomain>: Send + Sync {
    /// Input the generator consumes to produce candidates.
    type Input;
    /// A generated candidate.
    type Candidate;
    /// Generate candidates from `input`, seeded with `seed`.
    fn generate(&mut self, input: &Self::Input, seed: u64) -> Result<Vec<Self::Candidate>, GeneratorError>;
}

/// Configuration for a batch of matches.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchConfig {
    /// Number of games to play.
    pub games: usize,
    /// Master seed; game `i` is derived from `(seed, i)` deterministically.
    pub seed: u64,
    /// Abort a game after this many plies (`outcome: None`); `None` = unlimited.
    pub max_plies: Option<usize>,
}

/// Record of a single played game.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchRecord<A, O> {
    /// Index of this game within its batch.
    pub game_index: usize,
    /// Seed derived for this game.
    pub seed: u64,
    /// Actions played, in order.
    pub actions: Vec<A>,
    /// Outcome, if the game reached a terminal state.
    pub outcome: Option<O>,
}

/// Errors produced while running a batch of matches.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MatchError {
    /// No strategy provider was supplied for the named player.
    #[error("no strategy provider for player {0}")]
    MissingProvider(String),
    /// A strategy failed to choose an action.
    #[error(transparent)]
    Strategy(#[from] StrategyError),
    /// The rules rejected an action or state.
    #[error(transparent)]
    Rules(#[from] RulesError),
}

/// Parallel play loop over strategy providers. Same config + providers ⇒ identical records,
/// serial or parallel.
pub trait MatchEngine<G: GameDomain>: Send + Sync {
    /// Run `config.games` games, dispatching each player's moves through its provider.
    #[allow(clippy::type_complexity)]
    fn run(
        &self,
        config: &MatchConfig,
        players: &[(G::Player, &dyn StrategyProvider<G>)],
    ) -> Result<Vec<MatchRecord<G::Action, G::Outcome>>, MatchError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_config_round_trips_through_json() {
        let config = MatchConfig {
            games: 3,
            seed: 7,
            max_plies: Some(9),
        };
        let json = serde_json::to_string(&config).unwrap();
        let round_tripped: MatchConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, round_tripped);
    }

    #[test]
    fn match_record_round_trips_through_json() {
        let record = MatchRecord::<u8, bool> {
            game_index: 1,
            seed: 2,
            actions: vec![1, 2],
            outcome: Some(true),
        };
        let json = serde_json::to_string(&record).unwrap();
        let round_tripped: MatchRecord<u8, bool> = serde_json::from_str(&json).unwrap();
        assert_eq!(record, round_tripped);
    }

    #[test]
    fn strategy_error_unimplemented_message() {
        let err = StrategyError::Unimplemented {
            kind: "x".into(),
            detail: "y".into(),
        };
        assert_eq!(err.to_string(), "strategy `x` is not implemented: y");
    }

    #[test]
    fn match_error_from_rules_error_message() {
        let err = MatchError::from(RulesError::GameOver);
        assert_eq!(err.to_string(), "game is over; no further actions accepted");
    }

    #[allow(dead_code)]
    struct Toy;

    impl GameDomain for Toy {
        type State = u8;
        type Action = u8;
        type Player = u8;
        type Outcome = u8;
    }

    fn _assert_strategy_object_safe(_: &dyn Strategy<Toy>) {}

    fn _assert_strategy_provider_object_safe(_: &dyn StrategyProvider<Toy>) {}
}
