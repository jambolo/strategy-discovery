//! Game-neutral featurizer: turns a state into its feature environment, combining a
//! mechanically-built tier-1 extractor with an optional game-supplied extractor behind
//! one vocabulary, with canonical-frame helpers.

use crate::core::derived::PrimitiveFeatures;
use crate::core::features::{FeatureDef, FeatureError, FeatureValue, FeatureVector, FeatureVocabulary, Tier};
use crate::core::symmetry::Permutation;
use crate::core::traits::{Canonicalize, FeatureExtractor, GameDomain, GamePrimitives, GameRules};
use std::sync::Arc;

/// Name of the featurizer's own feature: index of the player to move in turn order.
pub const SIDE_TO_MOVE: &str = "side_to_move";

/// Construction options for a [`Featurizer`]: whether the game-supplied tier-2
/// extractor is included and whether the extended mechanical tier-1 families are
/// minted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FeaturizerSpec {
    /// Include the game-supplied tier-2 extractor, when the game has one.
    pub include_supplied: bool,
    /// Mint the extended mechanical tier-1 families (line-signature counts and
    /// cell-membership sets).
    pub extended: bool,
}

impl FeaturizerSpec {
    /// The pre-extension behavior: supplied features included, no extended families.
    pub fn base() -> Self {
        FeaturizerSpec {
            include_supplied: true,
            extended: false,
        }
    }
}

/// Turns a state into its feature environment: tier-1 primitives (built mechanically) plus an
/// optional game-supplied extractor, behind one vocabulary, with canonical-frame helpers.
pub struct Featurizer<G: GameDomain> {
    rules: Arc<dyn GameRules<G>>,
    primitives: Arc<dyn GamePrimitives<G>>,
    canonicalizer: Option<Arc<dyn Canonicalize<G>>>,
    players: Vec<G::Player>,
    extractors: Vec<Arc<dyn FeatureExtractor<G>>>,
    vocabulary: FeatureVocabulary,
}

impl<G: GameDomain> Featurizer<G> {
    /// Builds a featurizer. Always includes a mechanically-built tier-1 extractor over
    /// `rules`/`primitives`; `supplied`, if given, is appended after it. Fails if any two
    /// feature names collide (including `side_to_move` itself).
    pub fn new(
        rules: Arc<dyn GameRules<G>>,
        primitives: Arc<dyn GamePrimitives<G>>,
        canonicalizer: Option<Arc<dyn Canonicalize<G>>>,
        players: Vec<G::Player>,
        supplied: Option<Arc<dyn FeatureExtractor<G>>>,
    ) -> Result<Self, FeatureError> {
        Self::with_spec(rules, primitives, canonicalizer, players, supplied, FeaturizerSpec::base())
    }

    /// Builds a featurizer per `spec`: the tier-1 extractor is the extended mechanical
    /// set when `spec.extended`, else the base set; `supplied`, if given, is appended
    /// after it only when `spec.include_supplied`. Fails if any two feature names
    /// collide (including `side_to_move` itself).
    pub fn with_spec(
        rules: Arc<dyn GameRules<G>>,
        primitives: Arc<dyn GamePrimitives<G>>,
        canonicalizer: Option<Arc<dyn Canonicalize<G>>>,
        players: Vec<G::Player>,
        supplied: Option<Arc<dyn FeatureExtractor<G>>>,
        spec: FeaturizerSpec,
    ) -> Result<Self, FeatureError> {
        let mut extractors: Vec<Arc<dyn FeatureExtractor<G>>> = vec![if spec.extended {
            Arc::new(PrimitiveFeatures::extended(rules.clone(), primitives.clone()))
        } else {
            Arc::new(PrimitiveFeatures::new(rules.clone(), primitives.clone()))
        }];
        if spec.include_supplied
            && let Some(supplied) = supplied
        {
            extractors.push(supplied);
        }

        let mut vocabulary = FeatureVocabulary::new();
        vocabulary.push(FeatureDef::native(
            SIDE_TO_MOVE,
            Tier::Primitive,
            "index of the player to move in turn order (0 = first player)",
        ))?;
        for extractor in &extractors {
            for def in extractor.definitions() {
                vocabulary.push(def)?;
            }
        }

        Ok(Featurizer {
            rules,
            primitives,
            canonicalizer,
            players,
            extractors,
            vocabulary,
        })
    }

    /// The full feature vocabulary: `side_to_move` first, then every tier-1 definition,
    /// then every supplied definition, in that order.
    pub fn vocabulary(&self) -> &FeatureVocabulary {
        &self.vocabulary
    }

    /// The wrapped rules.
    pub fn rules(&self) -> &Arc<dyn GameRules<G>> {
        &self.rules
    }

    /// The wrapped primitives.
    pub fn primitives(&self) -> &Arc<dyn GamePrimitives<G>> {
        &self.primitives
    }

    /// Turn order used to compute `side_to_move`.
    pub fn players(&self) -> &[G::Player] {
        &self.players
    }

    /// Number of addressable positions, from `primitives.position_count()`.
    pub fn position_count(&self) -> usize {
        self.primitives.position_count()
    }

    /// Index of the mover in `players`, or `None` if the mover is not listed.
    pub fn side_to_move(&self, state: &G::State) -> Option<usize> {
        let mover = self.rules.player_to_move(state);
        self.players.iter().position(|p| *p == mover)
    }

    /// Canonical state and the transform mapping `state` to it. Identity when no
    /// canonicalizer was configured.
    pub fn canonical_frame(&self, state: &G::State) -> (G::State, Permutation) {
        match &self.canonicalizer {
            Some(canonicalizer) => canonicalizer.canonicalize_with_transform(state),
            None => (state.clone(), Permutation::identity(self.position_count())),
        }
    }

    /// Feature values for `state` as given, with no canonicalization.
    pub fn extract(&self, state: &G::State) -> FeatureVector {
        let mut out = FeatureVector::new();
        let side_to_move = self.side_to_move(state).map_or(-1, |s| s as i64);
        out.insert(SIDE_TO_MOVE, FeatureValue::Int(side_to_move));
        for extractor in &self.extractors {
            out = out.merged(&extractor.extract(state));
        }
        out
    }

    /// Feature values computed in the canonical frame, plus the transform used to reach it.
    pub fn extract_canonical(&self, state: &G::State) -> (FeatureVector, Permutation) {
        let (canonical, perm) = self.canonical_frame(state);
        (self.extract(&canonical), perm)
    }

    /// Positions targeted by `legal`, mapped through `perm`. `None` if any action has no
    /// addressable position.
    pub fn action_positions(&self, legal: &[G::Action], perm: &Permutation) -> Option<Vec<usize>> {
        legal
            .iter()
            .map(|a| self.primitives.action_position(a).map(|p| perm.apply(p)))
            .collect()
    }

    /// Appends a derived (expression-backed) definition to the vocabulary. Callers
    /// must pass a derived definition ([`FeatureDef::is_native`] is `false`); native
    /// definitions are the extractors' to declare. [`Featurizer::extract`] stays
    /// native-only — consumers evaluate derived definitions through
    /// [`FeatureVocabulary::evaluate`]. Fails as the vocabulary push fails: a
    /// duplicate name, or a reference to a name not yet defined.
    pub fn push_derived(&mut self, def: FeatureDef) -> Result<(), FeatureError> {
        debug_assert!(!def.is_native(), "push_derived requires a derived definition");
        self.vocabulary.push(def)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::features::{CmpOp, FeatureExpr};
    use crate::core::symmetry::SymmetryGroup;
    use crate::core::traits::RulesError;

    /// A 4-position ring: players alternate placing on empty cells; the game ends
    /// (drawn) once every cell is filled. Used only to exercise `Featurizer` against
    /// an abstract topology with no game-specific vocabulary.
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

    /// A tier-2 extractor: the number of occupied cells.
    struct RingSupplied;

    impl FeatureExtractor<Ring> for RingSupplied {
        fn definitions(&self) -> Vec<FeatureDef> {
            vec![FeatureDef::native("ring.filled", Tier::Supplied, "number of occupied cells")]
        }

        fn extract(&self, state: &RingState) -> FeatureVector {
            let filled = state.cells.iter().filter(|c| c.is_some()).count() as i64;
            let mut out = FeatureVector::new();
            out.insert("ring.filled", FeatureValue::Int(filled));
            out
        }
    }

    /// A tier-2 extractor that collides with the tier-1 `free` name.
    struct RingDuplicateSupplied;

    impl FeatureExtractor<Ring> for RingDuplicateSupplied {
        fn definitions(&self) -> Vec<FeatureDef> {
            vec![FeatureDef::native("free", Tier::Supplied, "duplicate of tier-1 free")]
        }

        fn extract(&self, _state: &RingState) -> FeatureVector {
            FeatureVector::new()
        }
    }

    /// Stand-in canonicalizer that always rotates by one position. Idempotence is not
    /// exercised.
    struct RingRotator;

    impl Canonicalize<Ring> for RingRotator {
        fn canonicalize_with_transform(&self, state: &RingState) -> (RingState, Permutation) {
            let rot = Permutation::new(vec![1, 2, 3, 0]).unwrap();
            (RingPrimitives.transform(state, &rot), rot)
        }
    }

    /// Primitives that never report an action's position.
    struct NoPositions;

    impl GamePrimitives<Ring> for NoPositions {
        fn position_count(&self) -> usize {
            RingPrimitives.position_count()
        }

        fn adjacent(&self, position: usize) -> Vec<usize> {
            RingPrimitives.adjacent(position)
        }

        fn lines(&self) -> Vec<Vec<usize>> {
            RingPrimitives.lines()
        }

        fn symmetry_group(&self) -> SymmetryGroup {
            RingPrimitives.symmetry_group()
        }

        fn occupant(&self, state: &RingState, position: usize) -> Option<u8> {
            RingPrimitives.occupant(state, position)
        }

        fn action_position(&self, _action: &usize) -> Option<usize> {
            None
        }

        fn transform(&self, state: &RingState, perm: &Permutation) -> RingState {
            RingPrimitives.transform(state, perm)
        }
    }

    fn rot() -> Permutation {
        Permutation::new(vec![1, 2, 3, 0]).unwrap()
    }

    fn featurizer(supplied: bool, canonicalizer: bool) -> Featurizer<Ring> {
        let supplied: Option<Arc<dyn FeatureExtractor<Ring>>> = if supplied { Some(Arc::new(RingSupplied)) } else { None };
        let canonicalizer: Option<Arc<dyn Canonicalize<Ring>>> = if canonicalizer { Some(Arc::new(RingRotator)) } else { None };
        Featurizer::new(
            Arc::new(RingRules),
            Arc::new(RingPrimitives),
            canonicalizer,
            vec![0, 1],
            supplied,
        )
        .unwrap()
    }

    #[test]
    fn vocabulary_order_side_to_move_then_tier1_then_supplied() {
        let f = featurizer(true, false);
        let tier1_names: Vec<String> = PrimitiveFeatures::new(RingRules, RingPrimitives)
            .definitions()
            .iter()
            .map(|d| d.name.clone())
            .collect();
        let mut expected = vec![SIDE_TO_MOVE.to_string()];
        expected.extend(tier1_names);
        expected.push("ring.filled".to_string());

        let names: Vec<&str> = f.vocabulary().defs().iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, expected);
        assert_eq!(f.vocabulary().len(), 25);

        let side = f.vocabulary().get(SIDE_TO_MOVE).unwrap();
        assert!(side.is_native());
        assert_eq!(side.tier, Tier::Primitive);
    }

    #[test]
    fn without_supplied_has_tier1_only() {
        let f = featurizer(false, false);
        assert_eq!(f.vocabulary().len(), 24);
        assert!(f.vocabulary().defs().iter().all(|d| !d.name.starts_with("ring.")));
    }

    #[test]
    fn duplicate_supplied_name_is_rejected() {
        let result = Featurizer::<Ring>::new(
            Arc::new(RingRules),
            Arc::new(RingPrimitives),
            None,
            vec![0, 1],
            Some(Arc::new(RingDuplicateSupplied)),
        );
        match result {
            Err(e) => assert_eq!(e, FeatureError::Duplicate("free".to_string())),
            Ok(_) => panic!("expected duplicate-name rejection"),
        }
    }

    #[test]
    fn extract_reports_side_to_move() {
        let f = featurizer(true, false);
        let rules = RingRules;
        let initial = rules.initial_state();
        let values = f.extract(&initial);
        assert_eq!(values.get(SIDE_TO_MOVE), Some(&FeatureValue::Int(0)));

        let s1 = rules.apply(&initial, &1).unwrap();
        let values = f.extract(&s1);
        assert_eq!(values.get(SIDE_TO_MOVE), Some(&FeatureValue::Int(1)));

        let stray = RingState {
            cells: [None; 4],
            to_move: 7,
        };
        let values = f.extract(&stray);
        assert_eq!(values.get(SIDE_TO_MOVE), Some(&FeatureValue::Int(-1)));
        assert!(values.get("free").is_some());
        assert!(values.get("ring.filled").is_some());
        assert_eq!(values.len(), f.vocabulary().len());
    }

    #[test]
    fn canonical_frame_without_canonicalizer_is_identity() {
        let f = featurizer(false, false);
        let s = RingRules.initial_state();
        assert_eq!(f.canonical_frame(&s), (s.clone(), Permutation::identity(4)));
    }

    #[test]
    fn canonical_frame_with_canonicalizer_applies_transform() {
        let f = featurizer(true, true);
        let rules = RingRules;
        let s = rules.apply(&rules.initial_state(), &1).unwrap();
        let (values, perm) = f.extract_canonical(&s);
        let expected = f.extract(&RingPrimitives.transform(&s, &rot()));
        assert_eq!((values, perm), (expected, rot()));
    }

    #[test]
    fn action_positions_maps_through_perm() {
        let f = featurizer(false, false);
        assert_eq!(f.action_positions(&[0, 2], &rot()), Some(vec![1, 3]));
        assert_eq!(f.action_positions(&[0, 2], &Permutation::identity(4)), Some(vec![0, 2]));
    }

    #[test]
    fn action_positions_none_without_positions() {
        let f = Featurizer::<Ring>::new(Arc::new(RingRules), Arc::new(NoPositions), None, vec![0, 1], None).unwrap();
        assert_eq!(f.action_positions(&[0], &Permutation::identity(4)), None);
    }

    fn with_spec(supplied: bool, spec: FeaturizerSpec) -> Featurizer<Ring> {
        let supplied: Option<Arc<dyn FeatureExtractor<Ring>>> = if supplied { Some(Arc::new(RingSupplied)) } else { None };
        Featurizer::<Ring>::with_spec(
            Arc::new(RingRules),
            Arc::new(RingPrimitives),
            None,
            vec![0, 1],
            supplied,
            spec,
        )
        .unwrap()
    }

    #[test]
    fn with_spec_extended_and_supplied_matrix() {
        let base = with_spec(true, FeaturizerSpec::base());
        assert_eq!(base.vocabulary().len(), 25);
        assert!(
            base.vocabulary()
                .defs()
                .iter()
                .all(|d| !d.name.starts_with("lines.") && !d.name.starts_with("cells."))
        );
        let base_names: Vec<&str> = base.vocabulary().defs().iter().map(|d| d.name.as_str()).collect();
        let new = featurizer(true, false);
        let new_names: Vec<&str> = new.vocabulary().defs().iter().map(|d| d.name.as_str()).collect();
        assert_eq!(base_names, new_names);

        let no_supplied = with_spec(
            true,
            FeaturizerSpec {
                include_supplied: false,
                extended: false,
            },
        );
        assert_eq!(no_supplied.vocabulary().len(), 24);

        let extended_and_supplied = with_spec(
            true,
            FeaturizerSpec {
                include_supplied: true,
                extended: true,
            },
        );
        assert_eq!(extended_and_supplied.vocabulary().len(), 37);
        assert!(extended_and_supplied.vocabulary().get("lines.mine2.theirs0").is_some());
        assert!(extended_and_supplied.vocabulary().get("cells.mine1.theirs0.ge2").is_some());
        assert_eq!(extended_and_supplied.vocabulary().defs().last().unwrap().name, "ring.filled");

        let extended_only = with_spec(
            true,
            FeaturizerSpec {
                include_supplied: false,
                extended: true,
            },
        );
        assert_eq!(extended_only.vocabulary().len(), 36);
    }

    #[test]
    fn with_spec_extended_extract_covers_vocabulary() {
        let f = with_spec(
            true,
            FeaturizerSpec {
                include_supplied: true,
                extended: true,
            },
        );
        let values = f.extract(&RingRules.initial_state());
        assert_eq!(values.len(), 37);
        let value_names: std::collections::BTreeSet<&str> = values.iter().map(|(n, _)| n.as_str()).collect();
        let vocab_names: std::collections::BTreeSet<&str> = f.vocabulary().defs().iter().map(|d| d.name.as_str()).collect();
        assert_eq!(value_names, vocab_names);
    }

    #[test]
    fn push_derived_appends_and_errors() {
        let mut f = featurizer(true, false);
        f.push_derived(FeatureDef::derived(
            "ring.pair",
            Tier::Invented,
            "at least two cells are free",
            FeatureExpr::compare(CmpOp::Gt, FeatureExpr::count(FeatureExpr::named("free")), FeatureExpr::int(1)),
        ))
        .unwrap();
        assert_eq!(f.vocabulary().len(), 26);
        let last = f.vocabulary().defs().last().unwrap();
        assert_eq!(last.name, "ring.pair");
        assert!(!last.is_native());

        let initial = RingRules.initial_state();
        let native = f.extract(&initial);
        assert!(native.get("ring.pair").is_none());
        let evaluated = f.vocabulary().evaluate(&native).unwrap();
        assert_eq!(evaluated.get("ring.pair"), Some(&FeatureValue::Bool(true)));

        let dup = f.push_derived(FeatureDef::derived(
            "ring.pair",
            Tier::Invented,
            "duplicate",
            FeatureExpr::compare(CmpOp::Gt, FeatureExpr::count(FeatureExpr::named("free")), FeatureExpr::int(1)),
        ));
        assert_eq!(dup, Err(FeatureError::Duplicate("ring.pair".to_string())));

        let unknown = f.push_derived(FeatureDef::derived(
            "ring.nope",
            Tier::Invented,
            "references an undefined name",
            FeatureExpr::compare(CmpOp::Gt, FeatureExpr::count(FeatureExpr::named("nope")), FeatureExpr::int(1)),
        ));
        assert!(matches!(unknown, Err(FeatureError::Unknown(_))));
    }
}
