//! Named, versioned benchmark strategy populations ("rosters") for opposition play. Phase 4
//! ships the [`Roster`] / [`RosterEntry`] types only; the Phase 7 harness evaluates a strategy
//! under test by playing every entry in both colours.

use crate::core::traits::StrategyError;
use crate::strategy::minimax::{MinimaxConfig, TieBreak};
use crate::strategy::registry::StrategySpec;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// One named opponent in a [`Roster`]: a spec-constructible strategy paired with the name used
/// to look it up and report results against it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct RosterEntry {
    /// Unique (within its roster) name identifying this opponent.
    pub name: String,
    /// The strategy spec this entry builds.
    pub spec: StrategySpec,
}

/// A named, versioned benchmark population of opponents. The Phase 7 harness evaluates a
/// strategy under test by playing every entry in both colours; Phase 4 only ships the type.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Roster {
    /// Roster name, unique within whatever catalog holds it.
    pub name: String,
    /// Roster version; bump when entries change so archived results stay attributable to the
    /// roster version that produced them.
    pub version: u32,
    /// Opponents making up this roster.
    pub entries: Vec<RosterEntry>,
}

impl Roster {
    /// Builds a roster from `name`, `version` and `entries`, then [`Self::validate`]s it.
    pub fn new(name: impl Into<String>, version: u32, entries: Vec<RosterEntry>) -> Result<Self, StrategyError> {
        let roster = Self {
            name: name.into(),
            version,
            entries,
        };
        roster.validate()?;
        Ok(roster)
    }

    /// Stable identifier combining name and version: `"{name}-v{version}"`.
    pub fn id(&self) -> String {
        format!("{}-v{}", self.name, self.version)
    }

    /// Validates that the roster name is non-empty, that entry names are unique, and that every
    /// entry name is non-empty. Serde does not validate on deserialize; callers deserializing a
    /// roster must call this explicitly.
    pub fn validate(&self) -> Result<(), StrategyError> {
        if self.name.is_empty() {
            return Err(StrategyError::Other("roster name must not be empty".to_string()));
        }
        let mut seen: HashSet<&str> = HashSet::with_capacity(self.entries.len());
        for entry in &self.entries {
            if !seen.insert(entry.name.as_str()) {
                return Err(StrategyError::Other(format!("duplicate roster entry name `{}`", entry.name)));
            }
        }
        for entry in &self.entries {
            if entry.name.is_empty() {
                return Err(StrategyError::Other("roster entry name must not be empty".to_string()));
            }
        }
        Ok(())
    }

    /// Looks up an entry by name.
    pub fn get(&self, name: &str) -> Option<&RosterEntry> {
        self.entries.iter().find(|entry| entry.name == name)
    }

    /// Builds a standard graded roster: a `random` baseline, one `depth-{d}` minimax entry per
    /// `d` in `depths` (in slice order, `epsilon: 0.0`, [`TieBreak::SeededUniform`]), then a
    /// `perfect` minimax entry at `perfect_depth`. Duplicate depths produce duplicate entry
    /// names, so [`Self::new`] rejects them via [`Self::validate`].
    pub fn graded(name: impl Into<String>, version: u32, depths: &[u32], perfect_depth: u32) -> Result<Self, StrategyError> {
        let mut entries = Vec::with_capacity(depths.len() + 2);
        entries.push(RosterEntry {
            name: "random".to_string(),
            spec: StrategySpec::Random,
        });
        for &depth in depths {
            entries.push(RosterEntry {
                name: format!("depth-{depth}"),
                spec: StrategySpec::Minimax(MinimaxConfig {
                    depth,
                    epsilon: 0.0,
                    tie_break: TieBreak::SeededUniform,
                }),
            });
        }
        entries.push(RosterEntry {
            name: "perfect".to_string(),
            spec: StrategySpec::Minimax(MinimaxConfig {
                depth: perfect_depth,
                epsilon: 0.0,
                tie_break: TieBreak::SeededUniform,
            }),
        });
        Self::new(name, version, entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graded_builds_entries_in_order() {
        let roster = Roster::graded("g", 3, &[1, 2, 4], 9).unwrap();

        let names: Vec<&str> = roster.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["random", "depth-1", "depth-2", "depth-4", "perfect"]);

        assert_eq!(roster.entries[0].spec, StrategySpec::Random);
        assert_eq!(
            roster.get("depth-2").unwrap().spec,
            StrategySpec::Minimax(MinimaxConfig {
                depth: 2,
                epsilon: 0.0,
                tie_break: TieBreak::SeededUniform,
            })
        );
        match &roster.get("perfect").unwrap().spec {
            StrategySpec::Minimax(cfg) => assert_eq!(cfg.depth, 9),
            other => panic!("expected Minimax, got {other:?}"),
        }
        assert_eq!(roster.id(), "g-v3");
    }

    #[test]
    fn graded_rejects_duplicate_depths() {
        match Roster::graded("g", 1, &[2, 2], 9) {
            Err(StrategyError::Other(msg)) => assert!(msg.contains("duplicate roster entry name `depth-2`")),
            other => panic!("expected Err(Other(_)), got {other:?}"),
        }
    }

    #[test]
    fn validate_rejects_empty_names() {
        assert!(Roster::new("", 1, vec![]).is_err());
        assert!(
            Roster::new(
                "r",
                1,
                vec![RosterEntry {
                    name: String::new(),
                    spec: StrategySpec::Random,
                }],
            )
            .is_err()
        );
    }

    #[test]
    fn get_finds_entry_by_name() {
        let roster = Roster::graded("g", 1, &[1], 9).unwrap();
        assert!(roster.get("perfect").is_some());
        assert!(roster.get("depth-1").is_some());
        assert!(roster.get("depth-7").is_none());
    }

    #[test]
    fn roster_round_trips_json_and_toml() {
        let roster = Roster::graded("ttt", 1, &[1, 4], 9).unwrap();

        let json = serde_json::to_string(&roster).unwrap();
        assert_eq!(serde_json::from_str::<Roster>(&json).unwrap(), roster);

        let as_toml = toml::to_string(&roster).unwrap();
        assert_eq!(toml::from_str::<Roster>(&as_toml).unwrap(), roster);
        assert!(as_toml.contains("[[entries]]"));
        assert!(as_toml.contains("kind = \"minimax\""));
    }
}
