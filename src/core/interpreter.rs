//! Executes a [`HeuristicStrategy`] as a [`Strategy`]: canonical-frame rule evaluation with
//! seeded tie-breaks.
//!
//! Semantics: `choose` computes the canonical frame of the given state, evaluates the
//! heuristic's feature vocabulary there, and walks [`HeuristicStrategy::ordered_rules`] in
//! priority order. The first rule whose condition is true AND whose selector yields at least
//! one candidate wins; a selector yielding no candidates does not fire its rule, and evaluation
//! continues to the next rule. If no rule fires, the strategy's `fallback` selector is tried the
//! same way, falling back further to every legal action if it too yields nothing. Exactly one
//! candidate is returned directly; more than one is broken uniformly by a seeded RNG.
//! `ActionSelector::TargetIn` positions are interpreted in the canonical frame and mapped back
//! to the raw frame through the canonicalizing transform. `ActionSelector::Maximize` evaluates
//! its expression on the state that *results* from each candidate move — i.e. from the
//! perspective of whichever player is to move next, typically the opponent; the interpreter
//! does not "correct" for this.

use crate::core::dsl::{ActionSelector, DslError, HeuristicStrategy, Rule};
use crate::core::features::{FeatureExpr, FeatureValue, FeatureVector};
use crate::core::featurizer::Featurizer;
use crate::core::kinds::HEURISTIC_RULES;
use crate::core::symmetry::Permutation;
use crate::core::traits::{GameDomain, Strategy, StrategyError, StrategyProvider};
use rand::SeedableRng;
use rand::seq::IndexedRandom;
use rand_chacha::ChaCha8Rng;
use std::sync::Arc;

/// Validates `heuristic` against `featurizer`'s vocabulary: [`HeuristicStrategy::validate`]
/// first, then rejects any native definition the featurizer does not itself produce.
fn validate_against_featurizer<G: GameDomain>(heuristic: &HeuristicStrategy, featurizer: &Featurizer<G>) -> Result<(), DslError> {
    heuristic.validate()?;
    let unavailable: Vec<String> = heuristic
        .definitions
        .defs()
        .iter()
        .filter(|def| def.is_native() && !featurizer.vocabulary().contains(&def.name))
        .map(|def| def.name.clone())
        .collect();
    if !unavailable.is_empty() {
        return Err(DslError::UnavailableFeatures(unavailable));
    }
    Ok(())
}

/// Evaluates a feature expression, mapping any [`crate::core::features::FeatureError`] to
/// [`StrategyError::Other`].
fn eval(expr: &FeatureExpr, env: &FeatureVector) -> Result<FeatureValue, StrategyError> {
    expr.evaluate(env)
        .map_err(|e| StrategyError::Other(format!("evaluating features: {e}")))
}

/// Executes a [`HeuristicStrategy`] as a [`Strategy`]: canonical-frame rule evaluation with a
/// seeded tie-break RNG.
pub struct RuleInterpreter<G: GameDomain> {
    heuristic: HeuristicStrategy,
    ordered: Vec<Rule>,
    featurizer: Arc<Featurizer<G>>,
    seed: u64,
    rng: ChaCha8Rng,
}

impl<G: GameDomain> RuleInterpreter<G> {
    /// Validates `heuristic` against `featurizer`'s vocabulary before accepting it.
    pub fn new(heuristic: HeuristicStrategy, featurizer: Arc<Featurizer<G>>, seed: u64) -> Result<Self, DslError> {
        validate_against_featurizer(&heuristic, &featurizer)?;
        let ordered = heuristic.ordered_rules().into_iter().cloned().collect();
        Ok(Self {
            heuristic,
            ordered,
            featurizer,
            seed,
            rng: ChaCha8Rng::seed_from_u64(seed),
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

    /// Candidate actions for `selector`, given the canonical-frame `env`, the canonicalizing
    /// `perm`, and the raw `state`/`legal`. An empty result means the selector does not fire.
    fn candidates(
        &self,
        selector: &ActionSelector,
        state: &G::State,
        legal: &[G::Action],
        perm: &Permutation,
        env: &FeatureVector,
    ) -> Result<Vec<G::Action>, StrategyError> {
        match selector {
            ActionSelector::AnyLegal => Ok(legal.to_vec()),
            ActionSelector::TargetIn { expr } => {
                let targets = eval(expr, env)?
                    .as_set()
                    .map_err(|e| StrategyError::Other(format!("evaluating features: {e}")))?
                    .clone();
                let primitives = self.featurizer.primitives();
                Ok(legal
                    .iter()
                    .filter(|a| {
                        primitives
                            .action_position(a)
                            .map(|p| perm.apply(p))
                            .is_some_and(|p| targets.contains(&p))
                    })
                    .cloned()
                    .collect())
            }
            ActionSelector::Maximize { expr } => {
                let mut best: Option<f64> = None;
                let mut scored: Vec<(G::Action, f64)> = Vec::with_capacity(legal.len());
                for a in legal {
                    let next = self
                        .featurizer
                        .rules()
                        .apply(state, a)
                        .map_err(|e| StrategyError::Other(e.to_string()))?;
                    let (next_canonical, _) = self.featurizer.canonical_frame(&next);
                    let native = self.featurizer.extract(&next_canonical);
                    let next_env = self
                        .heuristic
                        .definitions
                        .evaluate(&native)
                        .map_err(|e| StrategyError::Other(format!("evaluating features: {e}")))?;
                    let value = eval(expr, &next_env)?;
                    let v = match value {
                        FeatureValue::Int(i) => i as f64,
                        FeatureValue::Float(f) => f,
                        _ => return Err(StrategyError::Other("maximize expression must be numeric".to_string())),
                    };
                    best = Some(best.map_or(v, |b| if v > b { v } else { b }));
                    scored.push((a.clone(), v));
                }
                let Some(best) = best else {
                    return Ok(Vec::new());
                };
                Ok(scored
                    .into_iter()
                    .filter(|(_, v)| f64::total_cmp(v, &best).is_eq())
                    .map(|(a, _)| a)
                    .collect())
            }
        }
    }
}

impl<G: GameDomain> Strategy<G> for RuleInterpreter<G> {
    /// Empty `legal` yields `NoLegalActions`. Otherwise walks the ordered rule list, then the
    /// fallback, then all of `legal`, tie-breaking with the seeded RNG (see module docs).
    fn choose(&mut self, state: &G::State, legal: &[G::Action]) -> Result<G::Action, StrategyError> {
        if legal.is_empty() {
            return Err(StrategyError::NoLegalActions);
        }

        let (canonical, perm) = self.featurizer.canonical_frame(state);
        let native = self.featurizer.extract(&canonical);
        let env = self
            .heuristic
            .definitions
            .evaluate(&native)
            .map_err(|e| StrategyError::Other(format!("evaluating features: {e}")))?;

        let mut candidates: Vec<G::Action> = Vec::new();
        for rule in self.ordered.clone() {
            let fired = eval(&rule.condition, &env)?
                .as_bool()
                .map_err(|e| StrategyError::Other(format!("evaluating features: {e}")))?;
            if !fired {
                continue;
            }
            let picked = self.candidates(&rule.action, state, legal, &perm, &env)?;
            if !picked.is_empty() {
                candidates = picked;
                break;
            }
        }

        if candidates.is_empty() {
            let fallback = self.heuristic.fallback.clone();
            candidates = self.candidates(&fallback, state, legal, &perm, &env)?;
        }
        if candidates.is_empty() {
            candidates = legal.to_vec();
        }

        if candidates.len() == 1 {
            return Ok(candidates.into_iter().next().unwrap());
        }
        Ok(candidates.choose(&mut self.rng).cloned().unwrap())
    }
}

/// [`StrategyProvider`] for the reserved `heuristic-rules` kind.
pub struct RuleInterpreterProvider<G: GameDomain> {
    heuristic: HeuristicStrategy,
    featurizer: Arc<Featurizer<G>>,
}

impl<G: GameDomain> RuleInterpreterProvider<G> {
    /// Validates once here so `create` cannot fail.
    pub fn new(heuristic: HeuristicStrategy, featurizer: Arc<Featurizer<G>>) -> Result<Self, DslError> {
        validate_against_featurizer(&heuristic, &featurizer)?;
        Ok(Self { heuristic, featurizer })
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
        Box::new(
            RuleInterpreter::new(self.heuristic.clone(), Arc::clone(&self.featurizer), seed)
                .expect("validated at provider construction"),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::dsl::Rule;
    use crate::core::features::{FeatureDef, FeatureExpr, Tier};
    use crate::core::symmetry::SymmetryGroup;
    use crate::core::traits::{Canonicalize, GamePrimitives, GameRules, RulesError};

    /// A 4-position ring: players alternate placing on empty cells; the game ends (drawn)
    /// once every cell is filled. Copied verbatim from `src/core/derived.rs` tests.
    #[derive(Clone, PartialEq, Eq, Hash, Debug)]
    struct RingState {
        cells: [Option<u8>; 4],
        to_move: u8,
    }

    struct Ring;

    impl GameDomain for Ring {
        type State = RingState;
        type Action = usize;
        type Player = u8;
        type Outcome = u8;
    }

    struct RingRules;

    impl GameRules<Ring> for RingRules {
        fn initial_state(&self) -> RingState {
            RingState {
                cells: [None; 4],
                to_move: 0,
            }
        }

        fn player_to_move(&self, state: &RingState) -> u8 {
            state.to_move
        }

        fn legal_actions(&self, state: &RingState) -> Vec<usize> {
            (0..4).filter(|&i| state.cells[i].is_none()).collect()
        }

        fn apply(&self, state: &RingState, action: &usize) -> Result<RingState, RulesError> {
            if self.outcome(state).is_some() {
                return Err(RulesError::GameOver);
            }
            let a = *action;
            if a >= 4 || state.cells[a].is_some() {
                return Err(RulesError::IllegalAction(format!("cell {a} unavailable")));
            }
            let mut next = state.clone();
            next.cells[a] = Some(state.to_move);
            next.to_move = 1 - state.to_move;
            Ok(next)
        }

        fn outcome(&self, state: &RingState) -> Option<u8> {
            if state.cells.iter().all(Option::is_some) {
                Some(0)
            } else {
                None
            }
        }
    }

    struct RingPrimitives;

    impl GamePrimitives<Ring> for RingPrimitives {
        fn position_count(&self) -> usize {
            4
        }

        fn adjacent(&self, position: usize) -> Vec<usize> {
            let mut v = vec![(position + 3) % 4, (position + 1) % 4];
            v.sort_unstable();
            v
        }

        fn lines(&self) -> Vec<Vec<usize>> {
            vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 0]]
        }

        fn symmetry_group(&self) -> SymmetryGroup {
            SymmetryGroup::generate(4, &[Permutation::new(vec![1, 2, 3, 0]).unwrap()]).unwrap()
        }

        fn occupant(&self, state: &RingState, position: usize) -> Option<u8> {
            state.cells[position]
        }

        fn action_position(&self, action: &usize) -> Option<usize> {
            Some(*action)
        }

        fn transform(&self, state: &RingState, perm: &Permutation) -> RingState {
            let mut cells = [None; 4];
            for p in 0..4 {
                cells[perm.apply(p)] = state.cells[p];
            }
            RingState {
                cells,
                to_move: state.to_move,
            }
        }
    }

    /// Rotates every state by one position, mapping raw position `p` to canonical `p + 1 mod 4`.
    struct RingRotator;

    impl Canonicalize<Ring> for RingRotator {
        fn canonicalize_with_transform(&self, state: &RingState) -> (RingState, Permutation) {
            let rot = Permutation::new(vec![1, 2, 3, 0]).unwrap();
            (RingPrimitives.transform(state, &rot), rot)
        }
    }

    fn featurizer(canonicalizer: bool) -> Arc<Featurizer<Ring>> {
        let canonicalizer: Option<Arc<dyn Canonicalize<Ring>>> = if canonicalizer { Some(Arc::new(RingRotator)) } else { None };
        Arc::new(Featurizer::new(Arc::new(RingRules), Arc::new(RingPrimitives), canonicalizer, vec![0, 1], None).unwrap())
    }

    #[test]
    fn provider_rejects_unavailable_native() {
        let mut heuristic = HeuristicStrategy::new("h");
        heuristic
            .define(FeatureDef::native("missing.feature", Tier::Supplied, "not produced"))
            .unwrap();
        heuristic.push_rule(Rule::new(
            "r",
            1,
            FeatureExpr::named("missing.feature"),
            ActionSelector::AnyLegal,
        ));

        match RuleInterpreterProvider::<Ring>::new(heuristic, featurizer(false)) {
            Err(DslError::UnavailableFeatures(v)) => assert_eq!(v, vec!["missing.feature".to_string()]),
            Err(other) => panic!("expected UnavailableFeatures, got {other:?}"),
            Ok(_) => panic!("expected UnavailableFeatures, got Ok"),
        }
    }

    #[test]
    fn empty_heuristic_choose_is_seeded_and_legal() {
        let heuristic = HeuristicStrategy::new("empty");
        let state = RingRules.initial_state();
        let legal = RingRules.legal_actions(&state);

        let mut interpreter = RuleInterpreter::<Ring>::new(heuristic.clone(), featurizer(false), 0).unwrap();
        let chosen = interpreter.choose(&state, &legal).unwrap();
        assert!(legal.contains(&chosen));

        let mut a = RuleInterpreter::<Ring>::new(heuristic.clone(), featurizer(false), 11).unwrap();
        let mut b = RuleInterpreter::<Ring>::new(heuristic.clone(), featurizer(false), 11).unwrap();
        let seq_a: Vec<usize> = (0..5).map(|_| a.choose(&state, &legal).unwrap()).collect();
        let seq_b: Vec<usize> = (0..5).map(|_| b.choose(&state, &legal).unwrap()).collect();
        assert_eq!(seq_a, seq_b);

        let mut firsts = std::collections::BTreeSet::new();
        for seed in 0..8 {
            let mut interpreter = RuleInterpreter::<Ring>::new(heuristic.clone(), featurizer(false), seed).unwrap();
            firsts.insert(interpreter.choose(&state, &legal).unwrap());
        }
        assert!(firsts.len() >= 2, "expected diverse first choices, got {firsts:?}");
    }

    #[test]
    fn target_in_skips_empty_and_selects() {
        let mut heuristic = HeuristicStrategy::new("h");
        heuristic.push_rule(Rule::new(
            "empty-target",
            3,
            FeatureExpr::boolean(true),
            ActionSelector::TargetIn {
                expr: FeatureExpr::set([]),
            },
        ));
        heuristic.push_rule(Rule::new(
            "real-target",
            2,
            FeatureExpr::boolean(true),
            ActionSelector::TargetIn {
                expr: FeatureExpr::set([2]),
            },
        ));

        let mut interpreter = RuleInterpreter::<Ring>::new(heuristic, featurizer(false), 0).unwrap();
        let state = RingRules.initial_state();
        let legal = RingRules.legal_actions(&state);
        assert_eq!(interpreter.choose(&state, &legal).unwrap(), 2);
    }

    #[test]
    fn maximize_requires_numeric() {
        let mut heuristic = HeuristicStrategy::new("h");
        heuristic
            .define(FeatureDef::native("free", Tier::Primitive, "free positions"))
            .unwrap();
        heuristic.push_rule(Rule::new(
            "r",
            1,
            FeatureExpr::boolean(true),
            ActionSelector::Maximize {
                expr: FeatureExpr::named("free"),
            },
        ));

        let mut interpreter = RuleInterpreter::<Ring>::new(heuristic, featurizer(false), 0).unwrap();
        let state = RingRules.initial_state();
        let legal = RingRules.legal_actions(&state);
        match interpreter.choose(&state, &legal).unwrap_err() {
            StrategyError::Other(msg) => assert!(msg.contains("numeric"), "unexpected message: {msg}"),
            other => panic!("expected Other, got {other:?}"),
        }
    }

    #[test]
    fn canonical_targets_map_back_to_raw_frame() {
        let mut heuristic = HeuristicStrategy::new("h");
        heuristic.push_rule(Rule::new(
            "r",
            1,
            FeatureExpr::boolean(true),
            ActionSelector::TargetIn {
                expr: FeatureExpr::set([1]),
            },
        ));

        let mut interpreter = RuleInterpreter::<Ring>::new(heuristic, featurizer(true), 0).unwrap();
        let state = RingRules.initial_state();
        let legal = RingRules.legal_actions(&state);
        assert_eq!(interpreter.choose(&state, &legal).unwrap(), 0);
    }
}
