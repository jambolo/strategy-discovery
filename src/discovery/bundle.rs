//! Game bundle: everything the corpus, annotation, evaluation and summary stages need from a game.

use crate::core::featurizer::Featurizer;
use crate::core::traits::{Canonicalize, FeatureExtractor, GamePrimitives, GameRules, StateEvaluator};
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
    /// Optional game-supplied tier-2 feature extractor (strategic vocabulary such as threats).
    pub supplied_features: Option<Arc<dyn FeatureExtractor<G>>>,
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
            featurizer: self.featurizer(true).ok().map(Arc::new),
        })
    }

    /// Builds the game's [`Featurizer`]: tier-1 primitives plus, when `include_supplied`,
    /// the game-supplied tier-2 extractor. `include_supplied = false` withholds tier-2
    /// (a feature-ablation switch).
    ///
    /// A game without primitives cannot featurize ([`CorpusError::Precondition`]); a
    /// vocabulary name clash is a game-definition bug ([`CorpusError::Config`]).
    pub fn featurizer(&self, include_supplied: bool) -> Result<Featurizer<G>, CorpusError> {
        let primitives = self.primitives.clone().ok_or_else(|| {
            CorpusError::Precondition(format!(
                "game `{}` declares no primitives; feature extraction, heuristic-rules play and mining are unavailable",
                self.name
            ))
        })?;
        let supplied = if include_supplied {
            self.supplied_features.clone()
        } else {
            None
        };
        Featurizer::new(
            self.rules.clone(),
            primitives,
            self.canonicalizer.clone(),
            self.players.clone(),
            supplied,
        )
        .map_err(|e| CorpusError::Config(format!("game `{}` feature vocabulary: {e}", self.name)))
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
