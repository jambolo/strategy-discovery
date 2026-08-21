//! Generic rule-interpreter [`Strategy`] over the [`crate::core::dsl`] heuristic DSL.
//!
//! Execution (walking the rule list against a live position) lands with heuristic mining
//! in Phase 8 (see `docs/plan.md`). Until then, [`RuleInterpreter::choose`] validates its
//! inputs and reports [`StrategyError::Unimplemented`] rather than pretending to play.

use crate::core::dsl::{DslError, HeuristicStrategy};
use crate::core::kinds::HEURISTIC_RULES;
use crate::core::traits::{GameDomain, Strategy, StrategyError, StrategyProvider};
use std::marker::PhantomData;

/// Executes a [`HeuristicStrategy`] as a [`Strategy`]. Execution lands in Phase 8; until then
/// `choose` returns [`StrategyError::Unimplemented`].
pub struct RuleInterpreter<G: GameDomain> {
    heuristic: HeuristicStrategy,
    seed: u64,
    _game: PhantomData<fn() -> G>,
}

impl<G: GameDomain> RuleInterpreter<G> {
    /// Validates `heuristic` (via [`HeuristicStrategy::validate`]) before accepting it.
    pub fn new(heuristic: HeuristicStrategy, seed: u64) -> Result<Self, DslError> {
        heuristic.validate()?;
        Ok(Self {
            heuristic,
            seed,
            _game: PhantomData,
        })
    }

    /// The validated heuristic this interpreter executes.
    pub fn heuristic(&self) -> &HeuristicStrategy {
        &self.heuristic
    }

    /// The seed this interpreter was constructed with.
    pub fn seed(&self) -> u64 {
        self.seed
    }
}

impl<G: GameDomain> Strategy<G> for RuleInterpreter<G> {
    /// Empty `legal` yields `NoLegalActions`; otherwise `Unimplemented { kind: "heuristic-rules",
    /// detail: "rule interpreter execution lands with heuristic mining (plan.md Phase 8)" }`.
    fn choose(&mut self, state: &G::State, legal: &[G::Action]) -> Result<G::Action, StrategyError> {
        let _ = state;
        if legal.is_empty() {
            return Err(StrategyError::NoLegalActions);
        }
        Err(StrategyError::Unimplemented {
            kind: HEURISTIC_RULES.to_string(),
            detail: "rule interpreter execution lands with heuristic mining (plan.md Phase 8)".to_string(),
        })
    }
}

/// [`StrategyProvider`] for the reserved `heuristic-rules` kind.
pub struct RuleInterpreterProvider<G: GameDomain> {
    heuristic: HeuristicStrategy,
    _game: PhantomData<fn() -> G>,
}

impl<G: GameDomain> RuleInterpreterProvider<G> {
    /// Validates once here so `create` cannot fail.
    pub fn new(heuristic: HeuristicStrategy) -> Result<Self, DslError> {
        heuristic.validate()?;
        Ok(Self {
            heuristic,
            _game: PhantomData,
        })
    }

    /// The validated heuristic this provider creates interpreters for.
    pub fn heuristic(&self) -> &HeuristicStrategy {
        &self.heuristic
    }
}

impl<G: GameDomain> StrategyProvider<G> for RuleInterpreterProvider<G> {
    fn kind(&self) -> &str {
        HEURISTIC_RULES
    }

    fn create(&self, seed: u64) -> Box<dyn Strategy<G>> {
        Box::new(RuleInterpreter {
            heuristic: self.heuristic.clone(),
            seed,
            _game: PhantomData,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::dsl::{ActionSelector, Rule};
    use crate::core::features::FeatureExpr;

    struct Toy;

    impl GameDomain for Toy {
        type State = u8;
        type Action = u8;
        type Player = u8;
        type Outcome = u8;
    }

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn new_accepts_valid_heuristic() {
        let interpreter = RuleInterpreter::<Toy>::new(HeuristicStrategy::new("t"), 7).unwrap();
        assert_eq!(interpreter.seed(), 7);
        assert_eq!(interpreter.heuristic().name, "t");
    }

    #[test]
    fn new_rejects_undefined_features() {
        let mut heuristic = HeuristicStrategy::new("t");
        heuristic.push_rule(Rule::new("r", 1, FeatureExpr::named("ghost"), ActionSelector::AnyLegal));

        match RuleInterpreter::<Toy>::new(heuristic.clone(), 0) {
            Err(err) => assert_eq!(err, DslError::UndefinedFeatures(vec!["ghost".into()])),
            Ok(_) => panic!("expected UndefinedFeatures"),
        }

        match RuleInterpreterProvider::<Toy>::new(heuristic) {
            Err(err) => assert_eq!(err, DslError::UndefinedFeatures(vec!["ghost".into()])),
            Ok(_) => panic!("expected UndefinedFeatures"),
        }
    }

    #[test]
    fn choose_reports_no_legal_actions() {
        let mut interpreter = RuleInterpreter::<Toy>::new(HeuristicStrategy::new("t"), 0).unwrap();
        assert_eq!(interpreter.choose(&0, &[]).unwrap_err(), StrategyError::NoLegalActions);
    }

    #[test]
    fn choose_reports_unimplemented() {
        let mut interpreter = RuleInterpreter::<Toy>::new(HeuristicStrategy::new("t"), 0).unwrap();
        match interpreter.choose(&0, &[1]).unwrap_err() {
            StrategyError::Unimplemented { kind, .. } => assert_eq!(kind, "heuristic-rules"),
            other => panic!("expected Unimplemented, got {other:?}"),
        }
    }

    #[test]
    fn provider_kind_and_create() {
        let provider = RuleInterpreterProvider::<Toy>::new(HeuristicStrategy::new("p")).unwrap();
        assert_eq!(provider.kind(), "heuristic-rules");

        let mut strategy = provider.create(3);
        match strategy.choose(&0, &[2]).unwrap_err() {
            StrategyError::Unimplemented { .. } => {}
            other => panic!("expected Unimplemented, got {other:?}"),
        }
    }

    #[test]
    fn provider_is_send_sync() {
        assert_send_sync::<RuleInterpreterProvider<Toy>>();
    }
}
