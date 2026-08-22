//! Game bundle: everything the corpus, annotation, evaluation and summary stages need from a game.

use crate::core::traits::{Canonicalize, GamePrimitives, GameRules, StateEvaluator};
use crate::discovery::config::CorpusError;
use crate::strategy::engine::EngineGame;
use crate::strategy::registry::EngineBundle;
use crate::strategy::roster::{Roster, RosterEntry};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Everything the corpus, annotation, evaluation and summary stages need from a game.
pub struct GameBundle<G: EngineGame> {
    /// The game's name; must equal `GenerateConfig.game`.
    pub name: String,
    /// Rules of play.
    pub rules: Arc<dyn GameRules<G>>,
    /// Turn order; `players[0]` moves first.
    pub players: Vec<G::Player>,
    /// Named evaluators available for this game.
    pub evaluators: BTreeMap<String, Arc<dyn StateEvaluator<G>>>,
    /// Key into `evaluators` used when no evaluator is named explicitly.
    pub default_evaluator: String,
    /// Optional symmetry canonicalizer.
    pub canonicalizer: Option<Arc<dyn Canonicalize<G>>>,
    /// Optional structural primitives.
    pub primitives: Option<Arc<dyn GamePrimitives<G>>>,
    /// Default benchmark strategies for this game.
    pub default_strategies: Vec<RosterEntry>,
    /// Versioned benchmark population the evaluation harness plays against; `roster.id()` is the
    /// provenance `roster_id` of archived results.
    pub roster: Roster,
    /// Engine depth that is exhaustive for this game.
    pub full_search_depth: u32,
    /// Coverage denominator: number of known canonical positions, if known.
    pub known_canonical_positions: Option<usize>,
}

impl<G: EngineGame> GameBundle<G> {
    /// Rules + the named evaluator; unknown name -> `CorpusError::Config`.
    pub fn engine_bundle(&self, evaluator: &str) -> Result<EngineBundle<G>, CorpusError> {
        let evaluator = self.evaluators.get(evaluator).ok_or_else(|| {
            CorpusError::Config(format!(
                "unknown evaluator `{evaluator}`; known evaluators: {}",
                self.evaluators.keys().cloned().collect::<Vec<_>>().join(", ")
            ))
        })?;
        Ok(EngineBundle {
            rules: self.rules.clone(),
            evaluator: evaluator.clone(),
        })
    }

    /// `(canonical_state, transform.as_slice().to_vec())`, or `(state.clone(), vec![])` without
    /// a canonicalizer.
    pub fn canonicalize(&self, state: &G::State) -> (G::State, Vec<usize>) {
        match &self.canonicalizer {
            Some(canonicalizer) => {
                let (canonical, transform) = canonicalizer.canonicalize_with_transform(state);
                (canonical, transform.as_slice().to_vec())
            }
            None => (state.clone(), Vec::new()),
        }
    }

    /// Index of `player` in `players`.
    pub fn player_slot(&self, player: G::Player) -> Option<usize> {
        self.players.iter().position(|p| *p == player)
    }
}

#[cfg(test)]
mod tests {
    use crate::games::tictactoe::{Board, game_bundle};

    #[test]
    fn bundle_without_canonicalizer_is_identity() {
        let mut bundle = game_bundle();
        bundle.canonicalizer = None;
        bundle.primitives = None;

        let board = Board::parse("..X......").unwrap();
        assert_eq!(bundle.canonicalize(&board), (board, vec![]));
    }
}
