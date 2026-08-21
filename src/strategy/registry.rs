//! Strategy registry: maps [`StrategySpec`] — the serializable, spec-constructible
//! description of a strategy — to a built [`StrategyProvider`] via a pluggable factory table
//! shared across one game's rules and evaluator.

use crate::core::dsl::HeuristicStrategy;
use crate::core::interpreter::RuleInterpreterProvider;
use crate::core::kinds;
use crate::core::traits::{GameDomain, GameRules, StateEvaluator, StrategyError, StrategyProvider};
use crate::strategy::engine::EngineGame;
use crate::strategy::minimax::{MinimaxConfig, MinimaxProvider};
use crate::strategy::random::RandomProvider;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;

/// Serializable, spec-constructible description of a strategy. Tag strings mirror
/// [`crate::core::kinds`]; see [`StrategyRegistry`] for turning a spec into a
/// [`StrategyProvider`].
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum StrategySpec {
    /// Engine-backed minimax strategy.
    Minimax(MinimaxConfig),
    /// Uniform random strategy.
    Random,
    /// Generic rule-interpreter over a heuristic.
    HeuristicRules {
        /// The heuristic to interpret.
        heuristic: HeuristicStrategy,
    },
    /// Reserved: evolutionary strategy generator (plan.md Phase 8). Not yet buildable.
    Evolutionary,
    /// Reserved: LLM-generated strategy (plan.md Phase 8). Not yet buildable.
    Llm,
}

impl StrategySpec {
    /// The registered kind name (see [`crate::core::kinds`]) this spec builds.
    pub fn kind(&self) -> &'static str {
        match self {
            StrategySpec::Minimax(_) => kinds::MINIMAX,
            StrategySpec::Random => kinds::RANDOM,
            StrategySpec::HeuristicRules { .. } => kinds::HEURISTIC_RULES,
            StrategySpec::Evolutionary => kinds::EVOLUTIONARY,
            StrategySpec::Llm => kinds::LLM,
        }
    }
}

/// Shared game handles that a [`StrategyRegistry`] passes to its factories: rules of play and
/// a static evaluator, held by `Arc` so every built strategy can share one instance of each.
pub struct EngineBundle<G: GameDomain> {
    /// Rules of play.
    pub rules: Arc<dyn GameRules<G>>,
    /// Static state evaluator.
    pub evaluator: Arc<dyn StateEvaluator<G>>,
}

impl<G: GameDomain> Clone for EngineBundle<G> {
    fn clone(&self) -> Self {
        Self {
            rules: Arc::clone(&self.rules),
            evaluator: Arc::clone(&self.evaluator),
        }
    }
}

/// Builds a [`StrategyProvider`] from a spec and the registry's [`EngineBundle`]. Registered
/// per strategy kind in a [`StrategyRegistry`].
pub type Factory<G> =
    Box<dyn Fn(&StrategySpec, &EngineBundle<G>) -> Result<Box<dyn StrategyProvider<G>>, StrategyError> + Send + Sync>;

/// Maps [`StrategySpec`] kinds to provider factories over one shared [`EngineBundle`].
pub struct StrategyRegistry<G: EngineGame> {
    bundle: EngineBundle<G>,
    factories: BTreeMap<String, Factory<G>>,
}

impl<G: EngineGame> StrategyRegistry<G> {
    /// A registry over `bundle`, pre-populated with the five built-in kinds: `minimax`,
    /// `random`, `heuristic-rules`, `evolutionary`, `llm`.
    pub fn new(bundle: EngineBundle<G>) -> Self {
        let mut registry = Self {
            bundle,
            factories: BTreeMap::new(),
        };
        registry.register(kinds::MINIMAX, Box::new(build_minimax::<G>));
        registry.register(kinds::RANDOM, Box::new(build_random::<G>));
        registry.register(kinds::HEURISTIC_RULES, Box::new(build_heuristic_rules::<G>));
        registry.register(kinds::EVOLUTIONARY, Box::new(build_evolutionary::<G>));
        registry.register(kinds::LLM, Box::new(build_llm::<G>));
        registry
    }

    /// Registers `factory` for `kind`, replacing any existing factory for that kind.
    /// `StrategySpec` is a closed enum, so this exists to plug real generators into the
    /// reserved `evolutionary`/`llm` kinds once Phase 8 lands, or to override a built-in
    /// (e.g. in tests).
    pub fn register(&mut self, kind: &str, factory: Factory<G>) {
        self.factories.insert(kind.to_string(), factory);
    }

    /// Builds a provider for `spec` via the factory registered for `spec.kind()`.
    pub fn build(&self, spec: &StrategySpec) -> Result<Box<dyn StrategyProvider<G>>, StrategyError> {
        let kind = spec.kind();
        let factory = self
            .factories
            .get(kind)
            .ok_or_else(|| StrategyError::Other(format!("no factory registered for strategy kind `{kind}`")))?;
        factory(spec, &self.bundle)
    }

    /// Registered strategy kinds, sorted.
    pub fn kinds(&self) -> Vec<&str> {
        self.factories.keys().map(String::as_str).collect()
    }

    /// The engine bundle this registry builds strategies against.
    pub fn bundle(&self) -> &EngineBundle<G> {
        &self.bundle
    }
}

/// Builds `spec.kind() == "minimax"` into a [`MinimaxProvider`]; any other spec is reported
/// via [`wrong_kind`].
fn build_minimax<G: EngineGame>(
    spec: &StrategySpec,
    bundle: &EngineBundle<G>,
) -> Result<Box<dyn StrategyProvider<G>>, StrategyError> {
    match spec {
        StrategySpec::Minimax(cfg) => {
            let provider = MinimaxProvider::new(Arc::clone(&bundle.rules), Arc::clone(&bundle.evaluator), cfg.clone())?;
            Ok(Box::new(provider))
        }
        other => Err(wrong_kind(kinds::MINIMAX, other)),
    }
}

/// Builds `spec.kind() == "random"` into a [`RandomProvider`]; any other spec is reported via
/// [`wrong_kind`].
fn build_random<G: EngineGame>(
    spec: &StrategySpec,
    _bundle: &EngineBundle<G>,
) -> Result<Box<dyn StrategyProvider<G>>, StrategyError> {
    match spec {
        StrategySpec::Random => Ok(Box::new(RandomProvider::<G>::new())),
        other => Err(wrong_kind(kinds::RANDOM, other)),
    }
}

/// Builds `spec.kind() == "heuristic-rules"` into a [`RuleInterpreterProvider`]; any other
/// spec is reported via [`wrong_kind`].
fn build_heuristic_rules<G: EngineGame>(
    spec: &StrategySpec,
    _bundle: &EngineBundle<G>,
) -> Result<Box<dyn StrategyProvider<G>>, StrategyError> {
    match spec {
        StrategySpec::HeuristicRules { heuristic } => {
            let provider = RuleInterpreterProvider::<G>::new(heuristic.clone()).map_err(|e| StrategyError::Other(e.to_string()))?;
            Ok(Box::new(provider))
        }
        other => Err(wrong_kind(kinds::HEURISTIC_RULES, other)),
    }
}

/// `spec.kind() == "evolutionary"` is reserved: no generator is implemented yet (plan.md
/// Phase 8). Any other spec is reported via [`wrong_kind`].
fn build_evolutionary<G: EngineGame>(
    spec: &StrategySpec,
    _bundle: &EngineBundle<G>,
) -> Result<Box<dyn StrategyProvider<G>>, StrategyError> {
    match spec {
        StrategySpec::Evolutionary => Err(reserved(kinds::EVOLUTIONARY)),
        other => Err(wrong_kind(kinds::EVOLUTIONARY, other)),
    }
}

/// `spec.kind() == "llm"` is reserved: no generator is implemented yet (plan.md Phase 8). Any
/// other spec is reported via [`wrong_kind`].
fn build_llm<G: EngineGame>(spec: &StrategySpec, _bundle: &EngineBundle<G>) -> Result<Box<dyn StrategyProvider<G>>, StrategyError> {
    match spec {
        StrategySpec::Llm => Err(reserved(kinds::LLM)),
        other => Err(wrong_kind(kinds::LLM, other)),
    }
}

/// A factory received a spec whose variant doesn't match the kind it builds.
fn wrong_kind(expected: &str, spec: &StrategySpec) -> StrategyError {
    StrategyError::Other(format!("factory for `{expected}` received spec of kind `{}`", spec.kind()))
}

/// A reserved kind with no generator implemented yet.
fn reserved(kind: &str) -> StrategyError {
    StrategyError::Unimplemented {
        kind: kind.to_string(),
        detail: "reserved extension kind; no generator implemented (plan.md Phase 8)".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::tictactoe::{Board, Move, TicTacToe, TicTacToeEvaluator, TicTacToeRules};
    use crate::strategy::minimax::TieBreak;
    use crate::strategy::scripted::ScriptedProvider;

    fn bundle() -> EngineBundle<TicTacToe> {
        EngineBundle {
            rules: Arc::new(TicTacToeRules),
            evaluator: Arc::new(TicTacToeEvaluator),
        }
    }

    #[test]
    fn spec_tags_equal_kinds_constants() {
        let cases: Vec<(StrategySpec, &str)> = vec![
            (
                StrategySpec::Minimax(MinimaxConfig {
                    depth: 1,
                    epsilon: 0.0,
                    tie_break: TieBreak::SeededUniform,
                }),
                kinds::MINIMAX,
            ),
            (StrategySpec::Random, kinds::RANDOM),
            (
                StrategySpec::HeuristicRules {
                    heuristic: HeuristicStrategy::new("h"),
                },
                kinds::HEURISTIC_RULES,
            ),
            (StrategySpec::Evolutionary, kinds::EVOLUTIONARY),
            (StrategySpec::Llm, kinds::LLM),
        ];
        for (spec, expected) in cases {
            let value = serde_json::to_value(&spec).unwrap();
            assert_eq!(value["kind"].as_str(), Some(spec.kind()));
            assert_eq!(spec.kind(), expected);
        }
    }

    #[test]
    fn minimax_spec_json_shape() {
        let spec = StrategySpec::Minimax(MinimaxConfig {
            depth: 9,
            epsilon: 0.0,
            tie_break: TieBreak::SeededUniform,
        });
        let json = serde_json::to_string(&spec).unwrap();
        assert_eq!(
            json,
            r#"{"kind":"minimax","depth":9,"epsilon":0.0,"tie_break":"seeded-uniform"}"#
        );
    }

    #[test]
    fn specs_round_trip_json_and_toml() {
        let specs = vec![
            StrategySpec::Minimax(MinimaxConfig {
                depth: 9,
                epsilon: 0.0,
                tie_break: TieBreak::SeededUniform,
            }),
            StrategySpec::Minimax(MinimaxConfig {
                depth: 3,
                epsilon: 0.5,
                tie_break: TieBreak::Engine,
            }),
            StrategySpec::Random,
            StrategySpec::Evolutionary,
            StrategySpec::Llm,
        ];
        for spec in specs {
            let json = serde_json::to_string(&spec).unwrap();
            assert_eq!(serde_json::from_str::<StrategySpec>(&json).unwrap(), spec);

            let as_toml = toml::to_string(&spec).unwrap();
            assert_eq!(toml::from_str::<StrategySpec>(&as_toml).unwrap(), spec);
        }

        let heuristic_spec = StrategySpec::HeuristicRules {
            heuristic: HeuristicStrategy::new("h"),
        };
        let json = serde_json::to_string(&heuristic_spec).unwrap();
        assert_eq!(serde_json::from_str::<StrategySpec>(&json).unwrap(), heuristic_spec);
    }

    #[test]
    fn unknown_kind_is_a_parse_error() {
        assert!(serde_json::from_str::<StrategySpec>(r#"{"kind":"mcts"}"#).is_err());
    }

    #[test]
    fn kinds_are_sorted_builtins() {
        let registry = StrategyRegistry::new(bundle());
        assert_eq!(
            registry.kinds(),
            vec!["evolutionary", "heuristic-rules", "llm", "minimax", "random"]
        );
    }

    #[test]
    fn build_minimax_and_random_play_a_legal_move() {
        let registry = StrategyRegistry::new(bundle());
        let rules = TicTacToeRules;
        let board = Board::empty();
        let legal = rules.legal_actions(&board);

        let minimax_provider = registry
            .build(&StrategySpec::Minimax(MinimaxConfig {
                depth: 1,
                epsilon: 0.0,
                tie_break: TieBreak::SeededUniform,
            }))
            .unwrap();
        assert_eq!(minimax_provider.kind(), "minimax");
        let chosen = minimax_provider.create(0).choose(&board, &legal).unwrap();
        assert!(legal.contains(&chosen));

        let random_provider = registry.build(&StrategySpec::Random).unwrap();
        assert_eq!(random_provider.kind(), "random");
        let chosen = random_provider.create(0).choose(&board, &legal).unwrap();
        assert!(legal.contains(&chosen));
    }

    #[test]
    fn build_rejects_invalid_minimax_config() {
        let registry = StrategyRegistry::new(bundle());
        let spec = StrategySpec::Minimax(MinimaxConfig {
            depth: 0,
            epsilon: 0.0,
            tie_break: TieBreak::SeededUniform,
        });
        assert!(matches!(registry.build(&spec), Err(StrategyError::Other(_))));
    }

    #[test]
    fn build_heuristic_rules_is_unimplemented_on_choose() {
        let registry = StrategyRegistry::new(bundle());
        let spec = StrategySpec::HeuristicRules {
            heuristic: HeuristicStrategy::new("h"),
        };
        let provider = registry.build(&spec).unwrap();
        assert_eq!(provider.kind(), "heuristic-rules");

        let board = Board::empty();
        let legal = TicTacToeRules.legal_actions(&board);
        match provider.create(0).choose(&board, &legal).unwrap_err() {
            StrategyError::Unimplemented { kind, .. } => assert_eq!(kind, "heuristic-rules"),
            other => panic!("expected Unimplemented, got {other:?}"),
        }
    }

    #[test]
    fn build_reserved_kinds_is_unimplemented() {
        let registry = StrategyRegistry::new(bundle());

        match registry.build(&StrategySpec::Evolutionary) {
            Err(StrategyError::Unimplemented { kind, .. }) => assert_eq!(kind, "evolutionary"),
            Err(other) => panic!("expected Unimplemented, got Err({other:?})"),
            Ok(_) => panic!("expected Unimplemented, got Ok"),
        }

        match registry.build(&StrategySpec::Llm) {
            Err(StrategyError::Unimplemented { kind, .. }) => assert_eq!(kind, "llm"),
            Err(other) => panic!("expected Unimplemented, got Err({other:?})"),
            Ok(_) => panic!("expected Unimplemented, got Ok"),
        }
    }

    #[test]
    fn register_replaces_builtin_factory() {
        let mut registry = StrategyRegistry::new(bundle());
        registry.register(
            "random",
            Box::new(|_, _| Ok(Box::new(ScriptedProvider::<TicTacToe>::new(vec![Move(4)])))),
        );
        let provider = registry.build(&StrategySpec::Random).unwrap();
        assert_eq!(provider.kind(), "scripted");
    }
}
