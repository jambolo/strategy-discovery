//! Validated permutations, finite permutation groups, and mechanical orbit derivation.
//!
//! A game declares its symmetry group as permutations of position indices `0..degree`
//! (e.g. cell indices on a board). This module derives further tier-1 primitives
//! *mechanically* from that group: orbits of positions (position classes under symmetry)
//! and orbits of position-sets (classes of, e.g., lines or regions under symmetry).
//! Nothing here references any concrete game.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Errors constructing permutations and symmetry groups.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SymmetryError {
    /// `map` is not a bijection on `0..degree`.
    #[error("permutation is not a bijection on 0..{degree}: {map:?}")]
    NotBijection { degree: usize, map: Vec<usize> },
    /// A permutation's degree did not match the expected degree.
    #[error("degree mismatch: expected {expected}, got {actual}")]
    DegreeMismatch { expected: usize, actual: usize },
    /// An explicit element set did not contain the identity permutation.
    #[error("element set does not contain the identity")]
    MissingIdentity,
    /// An explicit element set was not closed under composition.
    #[error("element set is not closed under composition")]
    NotClosed,
}

/// A permutation of positions `0..degree`. `map[i]` is the image of position `i`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Permutation {
    map: Vec<usize>,
}

impl Permutation {
    /// Validates that `map` is a bijection on `0..map.len()`.
    pub fn new(map: Vec<usize>) -> Result<Self, SymmetryError> {
        let degree = map.len();
        let mut seen = vec![false; degree];
        for &v in &map {
            if v >= degree || seen[v] {
                return Err(SymmetryError::NotBijection { degree, map });
            }
            seen[v] = true;
        }
        Ok(Permutation { map })
    }

    /// The identity permutation on `0..degree`.
    pub fn identity(degree: usize) -> Self {
        Permutation {
            map: (0..degree).collect(),
        }
    }

    /// The degree (number of positions) this permutation acts on.
    pub fn degree(&self) -> usize {
        self.map.len()
    }

    /// Whether this permutation fixes every position.
    pub fn is_identity(&self) -> bool {
        self.map.iter().enumerate().all(|(i, &v)| i == v)
    }

    /// Image of `position`. Panics if `position >= degree`.
    pub fn apply(&self, position: usize) -> usize {
        self.map[position]
    }

    /// The underlying image map as a slice.
    pub fn as_slice(&self) -> &[usize] {
        &self.map
    }

    /// `self ∘ other`: apply `other` first, then `self`. Panics on degree mismatch.
    pub fn compose(&self, other: &Permutation) -> Permutation {
        assert_eq!(
            self.degree(),
            other.degree(),
            "compose: degree mismatch ({} vs {})",
            self.degree(),
            other.degree()
        );
        let map = (0..self.degree()).map(|i| self.apply(other.apply(i))).collect();
        Permutation { map }
    }

    /// The inverse permutation.
    pub fn inverse(&self) -> Permutation {
        let mut inv = vec![0usize; self.degree()];
        for (i, &v) in self.map.iter().enumerate() {
            inv[v] = i;
        }
        Permutation { map: inv }
    }

    /// Moves data with positions: `out[self.apply(i)] = items[i]`. Panics if `items.len() != degree`.
    pub fn permute<T: Clone>(&self, items: &[T]) -> Vec<T> {
        assert_eq!(
            items.len(),
            self.degree(),
            "permute: item count {} must match degree {}",
            items.len(),
            self.degree()
        );
        let mut out: Vec<Option<T>> = vec![None; self.degree()];
        for (i, item) in items.iter().enumerate() {
            out[self.apply(i)] = Some(item.clone());
        }
        out.into_iter()
            .map(|slot| slot.expect("bijection guarantees every slot is filled exactly once"))
            .collect()
    }
}

/// A finite permutation group on positions `0..degree`. `elements()[0]` is always the identity;
/// remaining elements are sorted (derived `Ord` on `Permutation`) so construction is deterministic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymmetryGroup {
    degree: usize,
    elements: Vec<Permutation>,
}

impl SymmetryGroup {
    /// The group containing only the identity.
    pub fn trivial(degree: usize) -> Self {
        SymmetryGroup {
            degree,
            elements: vec![Permutation::identity(degree)],
        }
    }

    /// Closure of `generators` under composition (always includes the identity).
    ///
    /// # Errors
    /// [`SymmetryError::DegreeMismatch`] if any generator's degree differs from `degree`.
    pub fn generate(degree: usize, generators: &[Permutation]) -> Result<Self, SymmetryError> {
        for g in generators {
            if g.degree() != degree {
                return Err(SymmetryError::DegreeMismatch {
                    expected: degree,
                    actual: g.degree(),
                });
            }
        }

        let mut elements: BTreeSet<Permutation> = BTreeSet::new();
        elements.insert(Permutation::identity(degree));

        let mut changed = true;
        while changed {
            changed = false;
            let current: Vec<Permutation> = elements.iter().cloned().collect();
            for a in &current {
                for g in generators {
                    if elements.insert(g.compose(a)) {
                        changed = true;
                    }
                    if elements.insert(a.compose(g)) {
                        changed = true;
                    }
                }
            }
        }

        Ok(SymmetryGroup {
            degree,
            elements: elements.into_iter().collect(),
        })
    }

    /// Validates an explicit element list: every degree matches (else `DegreeMismatch`), the identity is
    /// present (else `MissingIdentity`), and the set is closed under composition (else `NotClosed`).
    /// Duplicates are removed. Checks run in that order.
    pub fn from_elements(degree: usize, elements: Vec<Permutation>) -> Result<Self, SymmetryError> {
        for e in &elements {
            if e.degree() != degree {
                return Err(SymmetryError::DegreeMismatch {
                    expected: degree,
                    actual: e.degree(),
                });
            }
        }

        let unique: BTreeSet<Permutation> = elements.into_iter().collect();

        if !unique.contains(&Permutation::identity(degree)) {
            return Err(SymmetryError::MissingIdentity);
        }

        for a in &unique {
            for b in &unique {
                if !unique.contains(&a.compose(b)) {
                    return Err(SymmetryError::NotClosed);
                }
            }
        }

        Ok(SymmetryGroup {
            degree,
            elements: unique.into_iter().collect(),
        })
    }

    /// The degree (number of positions) this group acts on.
    pub fn degree(&self) -> usize {
        self.degree
    }

    /// Number of elements in the group.
    pub fn order(&self) -> usize {
        self.elements.len()
    }

    /// The group's elements, identity first, remaining sorted.
    pub fn elements(&self) -> &[Permutation] {
        &self.elements
    }

    /// Orbits of positions under the group. Each orbit is sorted ascending; orbits are ordered by
    /// their smallest element. Every position appears in exactly one orbit.
    pub fn orbits(&self) -> Vec<Vec<usize>> {
        let mut visited = vec![false; self.degree];
        let mut result = Vec::new();
        for start in 0..self.degree {
            if visited[start] {
                continue;
            }
            let orbit: BTreeSet<usize> = self.elements.iter().map(|p| p.apply(start)).collect();
            for &p in &orbit {
                visited[p] = true;
            }
            result.push(orbit.into_iter().collect());
        }
        result
    }

    /// `orbit_index()[p]` = index into [`SymmetryGroup::orbits`] of the orbit containing `p`.
    pub fn orbit_index(&self) -> Vec<usize> {
        let orbits = self.orbits();
        let mut index = vec![0usize; self.degree];
        for (orbit_idx, orbit) in orbits.iter().enumerate() {
            for &p in orbit {
                index[p] = orbit_idx;
            }
        }
        index
    }

    /// Orbits of position-SETS: `sets[j]` is in the same class as `sets[i]` iff some element maps
    /// `sets[i]` onto `sets[j]` (compared as sets, order-insensitive). Returns classes of indices into
    /// `sets`, each class ascending, classes ordered by smallest index.
    pub fn set_orbits(&self, sets: &[Vec<usize>]) -> Vec<Vec<usize>> {
        let normalized: Vec<Vec<usize>> = sets
            .iter()
            .map(|s| {
                let mut v = s.clone();
                v.sort_unstable();
                v
            })
            .collect();

        let mut visited = vec![false; sets.len()];
        let mut classes = Vec::new();
        for i in 0..sets.len() {
            if visited[i] {
                continue;
            }
            let images: BTreeSet<Vec<usize>> = self
                .elements
                .iter()
                .map(|perm| {
                    let mut image: Vec<usize> = sets[i].iter().map(|&p| perm.apply(p)).collect();
                    image.sort_unstable();
                    image
                })
                .collect();

            let mut class = Vec::new();
            for (j, norm) in normalized.iter().enumerate() {
                if !visited[j] && images.contains(norm) {
                    class.push(j);
                    visited[j] = true;
                }
            }
            classes.push(class);
        }
        classes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perm(map: &[usize]) -> Permutation {
        Permutation::new(map.to_vec()).unwrap()
    }

    #[test]
    fn new_rejects_non_bijection() {
        assert_eq!(
            Permutation::new(vec![0, 0, 1]),
            Err(SymmetryError::NotBijection {
                degree: 3,
                map: vec![0, 0, 1],
            })
        );
    }

    #[test]
    fn apply_inverse_and_compose_with_inverse_is_identity() {
        let p = perm(&[1, 2, 0]);
        assert_eq!(p.apply(2), 0);
        assert_eq!(p.inverse().as_slice(), &[2, 0, 1]);
        assert!(p.compose(&p.inverse()).is_identity());
    }

    #[test]
    fn compose_applies_other_first() {
        let p = perm(&[1, 2, 0]);
        let q = perm(&[0, 2, 1]);
        // q maps 1 -> 2, then p maps 2 -> 0.
        assert_eq!(p.compose(&q).apply(1), 0);
    }

    #[test]
    fn permute_moves_data_by_image_position() {
        let p = perm(&[1, 2, 0]);
        assert_eq!(p.permute(&[10, 20, 30]), vec![30, 10, 20]);
    }

    #[test]
    fn trivial_group_orbits_are_singletons() {
        let g = SymmetryGroup::trivial(3);
        assert_eq!(g.order(), 1);
        assert_eq!(g.orbits(), vec![vec![0], vec![1], vec![2]]);
    }

    #[test]
    fn generate_reflection_on_three_positions() {
        let g = SymmetryGroup::generate(3, &[perm(&[2, 1, 0])]).unwrap();
        assert_eq!(g.order(), 2);
        assert_eq!(g.orbits(), vec![vec![0, 2], vec![1]]);
        assert_eq!(g.orbit_index(), vec![0, 1, 0]);
    }

    #[test]
    fn generate_four_cycle() {
        let g = SymmetryGroup::generate(4, &[perm(&[1, 2, 3, 0])]).unwrap();
        assert_eq!(g.order(), 4);
        assert_eq!(g.orbits(), vec![vec![0, 1, 2, 3]]);
    }

    #[test]
    fn generate_dihedral_group_of_order_eight() {
        let g = SymmetryGroup::generate(4, &[perm(&[1, 2, 3, 0]), perm(&[0, 3, 2, 1])]).unwrap();
        assert_eq!(g.order(), 8);
        assert!(g.elements()[0].is_identity());
        for e in g.elements() {
            assert_eq!(e.degree(), 4);
        }
    }

    #[test]
    fn generate_rejects_degree_mismatch() {
        assert_eq!(
            SymmetryGroup::generate(3, &[Permutation::identity(4)]),
            Err(SymmetryError::DegreeMismatch { expected: 3, actual: 4 })
        );
    }

    #[test]
    fn from_elements_validates_identity_and_closure() {
        assert_eq!(
            SymmetryGroup::from_elements(3, vec![perm(&[1, 2, 0])]),
            Err(SymmetryError::MissingIdentity)
        );
        assert_eq!(
            SymmetryGroup::from_elements(3, vec![Permutation::identity(3), perm(&[1, 2, 0])]),
            Err(SymmetryError::NotClosed)
        );
        let g = SymmetryGroup::from_elements(3, vec![Permutation::identity(3), perm(&[1, 2, 0]), perm(&[2, 0, 1])]).unwrap();
        assert_eq!(g.order(), 3);
    }

    #[test]
    fn set_orbits_groups_lines_under_four_cycle() {
        let g = SymmetryGroup::generate(4, &[perm(&[1, 2, 3, 0])]).unwrap();
        let sets = vec![vec![0, 1], vec![1, 2], vec![2, 3], vec![3, 0], vec![0, 2], vec![1, 3]];
        assert_eq!(g.set_orbits(&sets), vec![vec![0, 1, 2, 3], vec![4, 5]]);
    }

    #[test]
    fn set_orbits_is_order_insensitive() {
        let g = SymmetryGroup::trivial(2);
        let sets = vec![vec![1, 0], vec![0, 1]];
        assert_eq!(g.set_orbits(&sets), vec![vec![0, 1]]);
    }

    #[test]
    fn permutation_serde_round_trip_is_compact() {
        assert_eq!(serde_json::to_string(&Permutation::identity(2)).unwrap(), "[0,1]");
    }

    #[test]
    fn symmetry_group_serde_round_trips() {
        let g = SymmetryGroup::generate(4, &[perm(&[1, 2, 3, 0]), perm(&[0, 3, 2, 1])]).unwrap();
        let json = serde_json::to_string(&g).unwrap();
        let back: SymmetryGroup = serde_json::from_str(&json).unwrap();
        assert_eq!(back, g);
    }
}
