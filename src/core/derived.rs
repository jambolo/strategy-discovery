//! Tier-1 feature extractor derived mechanically from a game's primitives and rules.
//!
//! [`PrimitiveFeatures`] mints occupancy features from symmetry orbits and win lines
//! declared by [`GamePrimitives`], relative to whichever player is to move in a given
//! state. It carries no strategic insight and names no game: everything it produces
//! falls out of `position_count`, `lines`, and `symmetry_group`.

use crate::core::features::{FeatureDef, FeatureValue, FeatureVector, Tier};
use crate::core::traits::{FeatureExtractor, GameDomain, GamePrimitives, GameRules};
use std::collections::BTreeSet;
use std::marker::PhantomData;

/// Tier-1 extractor derived mechanically from a game's primitives and rules.
///
/// Produces `free`/`mine`/`theirs` occupancy sets over all positions, plus per-orbit
/// and per-line occupancy counts, all relative to `rules.player_to_move(state)`. In
/// `extended` mode, also mints line-signature counts (`lines.mine{a}.theirs{b}`) and
/// cell-membership sets (`cells.mine{a}.theirs{b}.ge{k}`), derived only from `lines`
/// and occupancy.
pub struct PrimitiveFeatures<G: GameDomain, R: GameRules<G>, P: GamePrimitives<G>> {
    rules: R,
    primitives: P,
    orbits: Vec<Vec<usize>>,
    lines: Vec<Vec<usize>>,
    extended: bool,
    _game: PhantomData<fn() -> G>,
}

impl<G: GameDomain, R: GameRules<G>, P: GamePrimitives<G>> PrimitiveFeatures<G, R, P> {
    /// Builds an extractor, computing `orbits = primitives.symmetry_group().orbits()`
    /// and `lines = primitives.lines()` once.
    pub fn new(rules: R, primitives: P) -> Self {
        let orbits = primitives.symmetry_group().orbits();
        let lines = primitives.lines();
        PrimitiveFeatures {
            rules,
            primitives,
            orbits,
            lines,
            extended: false,
            _game: PhantomData,
        }
    }

    /// Builds an extractor that additionally mints the extended mechanical families —
    /// line-signature counts (`lines.mine{a}.theirs{b}`) and cell-membership sets
    /// (`cells.mine{a}.theirs{b}.ge{k}`) — after the base definitions. Derived only
    /// from `primitives.lines()` and occupancy.
    pub fn extended(rules: R, primitives: P) -> Self {
        let orbits = primitives.symmetry_group().orbits();
        let lines = primitives.lines();
        PrimitiveFeatures {
            rules,
            primitives,
            orbits,
            lines,
            extended: true,
            _game: PhantomData,
        }
    }

    /// Position orbits under the game's symmetry group, in [`SymmetryGroup::orbits`]
    /// order.
    ///
    /// [`SymmetryGroup::orbits`]: crate::core::symmetry::SymmetryGroup::orbits
    pub fn orbits(&self) -> &[Vec<usize>] {
        &self.orbits
    }

    /// Win-condition lines, in `primitives.lines()` order.
    pub fn lines(&self) -> &[Vec<usize>] {
        &self.lines
    }

    /// The wrapped rules.
    pub fn rules(&self) -> &R {
        &self.rules
    }

    /// The wrapped primitives.
    pub fn primitives(&self) -> &P {
        &self.primitives
    }
}

/// Formats `positions` the way [`FeatureValue::Set`] displays, e.g. `{0, 2, 6}`.
fn format_positions(positions: &[usize]) -> String {
    FeatureValue::Set(positions.iter().copied().collect()).to_string()
}

/// The `(a, b)` line-signature pairs realizable by some line, in `(a asc, b asc)`
/// order, for Family A (`lines.mine{a}.theirs{b}`) when `for_family_b` is `false`, or
/// Family B (`cells.mine{a}.theirs{b}.ge{k}`) when `true`. Shared by `definitions()`
/// and `extract()` so both mint exactly the same name sets.
fn realizable_signatures(lines: &[Vec<usize>], for_family_b: bool) -> Vec<(usize, usize)> {
    let max_len = lines.iter().map(Vec::len).max().unwrap_or(0);
    let mut sigs = Vec::new();
    for a in 0..=max_len {
        for b in 0..=(max_len - a) {
            let realizable = if for_family_b {
                lines.iter().any(|l| l.len() > a + b)
            } else {
                lines.iter().any(|l| l.len() >= a + b)
            };
            if realizable {
                sigs.push((a, b));
            }
        }
    }
    sigs
}

impl<G: GameDomain, R: GameRules<G>, P: GamePrimitives<G>> FeatureExtractor<G> for PrimitiveFeatures<G, R, P> {
    fn definitions(&self) -> Vec<FeatureDef> {
        let mut defs = Vec::with_capacity(3 + 4 * (self.orbits.len() + self.lines.len()));
        defs.push(FeatureDef::native("free", Tier::Primitive, "empty positions"));
        defs.push(FeatureDef::native(
            "mine",
            Tier::Primitive,
            "positions held by the player to move",
        ));
        defs.push(FeatureDef::native("theirs", Tier::Primitive, "positions held by opponents"));

        for (k, orbit) in self.orbits.iter().enumerate() {
            let set = format_positions(orbit);
            defs.push(FeatureDef::native(
                format!("orbit{k}.free"),
                Tier::Primitive,
                format!("empty positions in orbit {k} {set}"),
            ));
            defs.push(FeatureDef::native(
                format!("orbit{k}.mine"),
                Tier::Primitive,
                format!("positions in orbit {k} {set} held by the player to move"),
            ));
            defs.push(FeatureDef::native(
                format!("orbit{k}.theirs"),
                Tier::Primitive,
                format!("positions in orbit {k} {set} held by opponents"),
            ));
            defs.push(FeatureDef::native(
                format!("orbit{k}.empty"),
                Tier::Primitive,
                format!("empty positions in orbit {k} {set} (count)"),
            ));
        }

        for (l, line) in self.lines.iter().enumerate() {
            let set = format_positions(line);
            defs.push(FeatureDef::native(
                format!("line{l}.free"),
                Tier::Primitive,
                format!("empty positions on line {l} {set}"),
            ));
            defs.push(FeatureDef::native(
                format!("line{l}.mine"),
                Tier::Primitive,
                format!("positions on line {l} {set} held by the player to move"),
            ));
            defs.push(FeatureDef::native(
                format!("line{l}.theirs"),
                Tier::Primitive,
                format!("positions on line {l} {set} held by opponents"),
            ));
            defs.push(FeatureDef::native(
                format!("line{l}.empty"),
                Tier::Primitive,
                format!("empty positions on line {l} {set} (count)"),
            ));
        }

        if self.extended {
            let kmax = (0..self.primitives.position_count())
                .map(|p| self.lines.iter().filter(|l| l.contains(&p)).count())
                .max()
                .unwrap_or(0);

            for (a, b) in realizable_signatures(&self.lines, false) {
                defs.push(FeatureDef::native(
                    format!("lines.mine{a}.theirs{b}"),
                    Tier::Primitive,
                    format!("number of lines with exactly {a} positions held by the player to move and {b} held by opponents"),
                ));
            }

            for (a, b) in realizable_signatures(&self.lines, true) {
                for k in 1..=kmax {
                    defs.push(FeatureDef::native(
                        format!("cells.mine{a}.theirs{b}.ge{k}"),
                        Tier::Primitive,
                        format!(
                            "empty positions on at least {k} lines that each have exactly {a} positions held by the player to move and {b} held by opponents"
                        ),
                    ));
                }
            }
        }

        defs
    }

    fn extract(&self, state: &G::State) -> FeatureVector {
        let mover = self.rules.player_to_move(state);
        let n = self.primitives.position_count();

        let mut free = BTreeSet::new();
        let mut mine = BTreeSet::new();
        let mut theirs = BTreeSet::new();
        for p in 0..n {
            match self.primitives.occupant(state, p) {
                None => {
                    free.insert(p);
                }
                Some(player) if player == mover => {
                    mine.insert(p);
                }
                Some(_) => {
                    theirs.insert(p);
                }
            }
        }

        let mut out = FeatureVector::new();
        out.insert("free", FeatureValue::Set(free.clone()));
        out.insert("mine", FeatureValue::Set(mine.clone()));
        out.insert("theirs", FeatureValue::Set(theirs.clone()));

        for (k, orbit) in self.orbits.iter().enumerate() {
            let orbit_free: BTreeSet<usize> = orbit.iter().copied().filter(|p| free.contains(p)).collect();
            let orbit_mine = orbit.iter().copied().filter(|p| mine.contains(p)).count() as i64;
            let orbit_theirs = orbit.iter().copied().filter(|p| theirs.contains(p)).count() as i64;
            let orbit_empty = orbit_free.len() as i64;
            out.insert(format!("orbit{k}.free"), FeatureValue::Set(orbit_free));
            out.insert(format!("orbit{k}.mine"), FeatureValue::Int(orbit_mine));
            out.insert(format!("orbit{k}.theirs"), FeatureValue::Int(orbit_theirs));
            out.insert(format!("orbit{k}.empty"), FeatureValue::Int(orbit_empty));
        }

        for (l, line) in self.lines.iter().enumerate() {
            let line_free: BTreeSet<usize> = line.iter().copied().filter(|p| free.contains(p)).collect();
            let line_mine = line.iter().copied().filter(|p| mine.contains(p)).count() as i64;
            let line_theirs = line.iter().copied().filter(|p| theirs.contains(p)).count() as i64;
            let line_empty = line_free.len() as i64;
            out.insert(format!("line{l}.free"), FeatureValue::Set(line_free));
            out.insert(format!("line{l}.mine"), FeatureValue::Int(line_mine));
            out.insert(format!("line{l}.theirs"), FeatureValue::Int(line_theirs));
            out.insert(format!("line{l}.empty"), FeatureValue::Int(line_empty));
        }

        if self.extended {
            let sigs: Vec<(usize, usize)> = self
                .lines
                .iter()
                .map(|line| {
                    (
                        line.iter().filter(|p| mine.contains(p)).count(),
                        line.iter().filter(|p| theirs.contains(p)).count(),
                    )
                })
                .collect();
            let kmax = (0..n)
                .map(|p| self.lines.iter().filter(|l| l.contains(&p)).count())
                .max()
                .unwrap_or(0);

            for (a, b) in realizable_signatures(&self.lines, false) {
                let count = sigs.iter().filter(|&&s| s == (a, b)).count() as i64;
                out.insert(format!("lines.mine{a}.theirs{b}"), FeatureValue::Int(count));
            }

            for (a, b) in realizable_signatures(&self.lines, true) {
                for k in 1..=kmax {
                    let set: BTreeSet<usize> = free
                        .iter()
                        .copied()
                        .filter(|&p| {
                            self.lines
                                .iter()
                                .enumerate()
                                .filter(|(l, line)| line.contains(&p) && sigs[*l] == (a, b))
                                .count()
                                >= k
                        })
                        .collect();
                    out.insert(format!("cells.mine{a}.theirs{b}.ge{k}"), FeatureValue::Set(set));
                }
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::symmetry::{Permutation, SymmetryGroup};
    use crate::core::traits::RulesError;
    use std::collections::BTreeSet as Set;

    /// A 4-position ring: players alternate placing on empty cells; the game ends
    /// (drawn) once every cell is filled. Used only to exercise `PrimitiveFeatures`
    /// against an abstract topology with no game-specific vocabulary.
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

    fn extractor() -> PrimitiveFeatures<Ring, RingRules, RingPrimitives> {
        PrimitiveFeatures::new(RingRules, RingPrimitives)
    }

    #[test]
    fn orbits_and_definitions_shape() {
        let ext = extractor();
        assert_eq!(ext.orbits(), &[vec![0, 1, 2, 3]]);
        let defs = ext.definitions();
        assert_eq!(defs.len(), 23);

        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        let unique: Set<&str> = names.iter().copied().collect();
        assert_eq!(unique.len(), names.len(), "feature names must be unique");

        let def_names: Set<String> = defs.iter().map(|d| d.name.clone()).collect();
        let initial = RingRules.initial_state();
        let values = ext.extract(&initial);
        let value_names: Set<String> = values.iter().map(|(name, _)| name.clone()).collect();
        assert_eq!(def_names, value_names);
    }

    #[test]
    fn definitions_content_and_tier() {
        let ext = extractor();
        let defs = ext.definitions();

        assert_eq!(defs[3].name, "orbit0.free");
        assert_eq!(defs[3].description, "empty positions in orbit 0 {0, 1, 2, 3}");

        assert_eq!(defs[7].name, "line0.free");

        for def in &defs {
            assert_eq!(def.tier, Tier::Primitive);
            assert!(def.is_native());
        }
    }

    #[test]
    fn initial_state_features() {
        let ext = extractor();
        let initial = RingRules.initial_state();
        let values = ext.extract(&initial);

        assert_eq!(values.get("free"), Some(&FeatureValue::Set(Set::from([0, 1, 2, 3]))));
        assert_eq!(values.get("mine"), Some(&FeatureValue::Set(Set::new())));
        assert_eq!(values.get("orbit0.empty"), Some(&FeatureValue::Int(4)));
        assert_eq!(values.get("line0.mine"), Some(&FeatureValue::Int(0)));
    }

    #[test]
    fn after_one_move_features() {
        let ext = extractor();
        let rules = RingRules;
        let s1 = rules.apply(&rules.initial_state(), &1).unwrap();
        let values = ext.extract(&s1);

        assert_eq!(values.get("mine"), Some(&FeatureValue::Set(Set::new())));
        assert_eq!(values.get("theirs"), Some(&FeatureValue::Set(Set::from([1]))));
        assert_eq!(values.get("orbit0.theirs"), Some(&FeatureValue::Int(1)));
        assert_eq!(values.get("orbit0.empty"), Some(&FeatureValue::Int(3)));
        assert_eq!(values.get("line0.theirs"), Some(&FeatureValue::Int(1)));
        assert_eq!(values.get("line0.free"), Some(&FeatureValue::Set(Set::from([0]))));
        assert_eq!(values.get("line0.empty"), Some(&FeatureValue::Int(1)));
        assert_eq!(values.get("line2.theirs"), Some(&FeatureValue::Int(0)));
    }

    #[test]
    fn after_two_moves_features() {
        let ext = extractor();
        let rules = RingRules;
        let s1 = rules.apply(&rules.initial_state(), &1).unwrap();
        let s2 = rules.apply(&s1, &3).unwrap();
        let values = ext.extract(&s2);

        assert_eq!(values.get("mine"), Some(&FeatureValue::Set(Set::from([1]))));
        assert_eq!(values.get("theirs"), Some(&FeatureValue::Set(Set::from([3]))));
        assert_eq!(values.get("line1.mine"), Some(&FeatureValue::Int(1)));
        assert_eq!(values.get("line1.theirs"), Some(&FeatureValue::Int(0)));
    }

    #[test]
    fn extended_ring_definitions_shape_and_order() {
        let base = extractor();
        assert_eq!(base.definitions().len(), 23);

        let ext = PrimitiveFeatures::extended(RingRules, RingPrimitives);
        let defs = ext.definitions();
        assert_eq!(defs.len(), 35);

        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        let unique: Set<&str> = names.iter().copied().collect();
        assert_eq!(unique.len(), names.len(), "feature names must be unique");

        for def in &defs {
            assert_eq!(def.tier, Tier::Primitive);
            assert!(def.is_native());
        }

        let base_names: Vec<String> = base.definitions().iter().map(|d| d.name.clone()).collect();
        assert_eq!(
            &names[0..23],
            base_names.iter().map(String::as_str).collect::<Vec<_>>().as_slice()
        );

        assert_eq!(
            &names[23..29],
            &[
                "lines.mine0.theirs0",
                "lines.mine0.theirs1",
                "lines.mine0.theirs2",
                "lines.mine1.theirs0",
                "lines.mine1.theirs1",
                "lines.mine2.theirs0",
            ]
        );
        assert_eq!(
            &names[29..35],
            &[
                "cells.mine0.theirs0.ge1",
                "cells.mine0.theirs0.ge2",
                "cells.mine0.theirs1.ge1",
                "cells.mine0.theirs1.ge2",
                "cells.mine1.theirs0.ge1",
                "cells.mine1.theirs0.ge2",
            ]
        );

        assert_eq!(
            defs[23].description,
            "number of lines with exactly 0 positions held by the player to move and 0 held by opponents"
        );
        assert_eq!(
            defs[29].description,
            "empty positions on at least 1 lines that each have exactly 0 positions held by the player to move and 0 held by opponents"
        );

        let def_names: Set<String> = defs.iter().map(|d| d.name.clone()).collect();
        let initial = RingRules.initial_state();
        let values = ext.extract(&initial);
        let value_names: Set<String> = values.iter().map(|(name, _)| name.clone()).collect();
        assert_eq!(def_names, value_names);
    }

    #[test]
    fn extended_ring_values() {
        let ext = PrimitiveFeatures::extended(RingRules, RingPrimitives);
        let rules = RingRules;

        let initial = rules.initial_state();
        let values = ext.extract(&initial);
        assert_eq!(values.get("lines.mine0.theirs0"), Some(&FeatureValue::Int(4)));
        assert_eq!(values.get("lines.mine0.theirs1"), Some(&FeatureValue::Int(0)));
        assert_eq!(values.get("lines.mine1.theirs0"), Some(&FeatureValue::Int(0)));
        assert_eq!(values.get("lines.mine2.theirs0"), Some(&FeatureValue::Int(0)));
        assert_eq!(
            values.get("cells.mine0.theirs0.ge1"),
            Some(&FeatureValue::Set(Set::from([0, 1, 2, 3])))
        );
        assert_eq!(
            values.get("cells.mine0.theirs0.ge2"),
            Some(&FeatureValue::Set(Set::from([0, 1, 2, 3])))
        );
        assert_eq!(values.get("cells.mine0.theirs1.ge1"), Some(&FeatureValue::Set(Set::new())));
        assert_eq!(values.get("cells.mine1.theirs0.ge1"), Some(&FeatureValue::Set(Set::new())));

        let s1 = rules.apply(&initial, &1).unwrap();
        let values = ext.extract(&s1);
        assert_eq!(values.get("lines.mine0.theirs0"), Some(&FeatureValue::Int(2)));
        assert_eq!(values.get("lines.mine0.theirs1"), Some(&FeatureValue::Int(2)));
        assert_eq!(values.get("lines.mine1.theirs0"), Some(&FeatureValue::Int(0)));
        assert_eq!(
            values.get("cells.mine0.theirs0.ge1"),
            Some(&FeatureValue::Set(Set::from([0, 2, 3])))
        );
        assert_eq!(
            values.get("cells.mine0.theirs0.ge2"),
            Some(&FeatureValue::Set(Set::from([3])))
        );
        assert_eq!(
            values.get("cells.mine0.theirs1.ge1"),
            Some(&FeatureValue::Set(Set::from([0, 2])))
        );
        assert_eq!(values.get("cells.mine0.theirs1.ge2"), Some(&FeatureValue::Set(Set::new())));
        assert_eq!(values.get("cells.mine1.theirs0.ge1"), Some(&FeatureValue::Set(Set::new())));

        let s2 = rules.apply(&s1, &3).unwrap();
        let values = ext.extract(&s2);
        assert_eq!(values.get("lines.mine0.theirs0"), Some(&FeatureValue::Int(0)));
        assert_eq!(values.get("lines.mine0.theirs1"), Some(&FeatureValue::Int(2)));
        assert_eq!(values.get("lines.mine1.theirs0"), Some(&FeatureValue::Int(2)));
        assert_eq!(values.get("lines.mine1.theirs1"), Some(&FeatureValue::Int(0)));
        assert_eq!(values.get("lines.mine2.theirs0"), Some(&FeatureValue::Int(0)));
        assert_eq!(values.get("cells.mine0.theirs0.ge1"), Some(&FeatureValue::Set(Set::new())));
        assert_eq!(
            values.get("cells.mine0.theirs1.ge1"),
            Some(&FeatureValue::Set(Set::from([0, 2])))
        );
        assert_eq!(
            values.get("cells.mine1.theirs0.ge1"),
            Some(&FeatureValue::Set(Set::from([0, 2])))
        );
        assert_eq!(values.get("cells.mine1.theirs0.ge2"), Some(&FeatureValue::Set(Set::new())));
    }
}
