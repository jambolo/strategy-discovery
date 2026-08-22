//! Strategy archive: append-only store of evaluated strategies with provenance and novelty
//! metadata (`archive.json` index plus `entries.jsonl`). Game-agnostic.

use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Write as _;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::core::dsl::HeuristicStrategy;
use crate::discovery::config::CorpusError;
use crate::io::{
    ARCHIVE_ENTRIES_FILE, ARCHIVE_FILE, ArchiveEntry, ArchiveIndex, ArchiveIndexEntry, BehaviorSignature, IoError, Novelty,
    Provenance, SCHEMA_VERSION, StrategyEvaluation, check_schema_version, config_hash, read_json, read_jsonl, write_json_pretty,
};
use crate::strategy::registry::StrategySpec;

/// Novelty method identifier written into every `Novelty`.
pub const NOVELTY_METHOD: &str = "m1-v1";

/// What a caller supplies to `Archive::append`; identity, sequence and novelty are computed by the archive.
#[derive(Debug, Clone, PartialEq)]
pub struct NewEntry {
    /// Strategy entry name.
    pub name: String,
    /// The strategy spec.
    pub spec: StrategySpec,
    /// Where the entry came from.
    pub provenance: Provenance,
    /// The evaluation that produced it.
    pub evaluation: StrategyEvaluation,
}

/// Outcome of an append: a new entry, or an identical `entry_id` already present (nothing written).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Appended {
    /// A new entry with this id was written.
    New(String),
    /// An entry with this id already existed; the archive is unchanged.
    Duplicate(String),
}

/// An open strategy archive directory.
#[derive(Debug)]
pub struct Archive {
    dir: PathBuf,
    index: ArchiveIndex,
    entries: Vec<ArchiveEntry>,
}

impl Archive {
    /// Opens the archive at `dir` for `game`, creating an empty one if `archive.json` is absent.
    ///
    /// Fails if the archive at `dir` belongs to a different game, or if the index and
    /// `entries.jsonl` disagree about which entries exist.
    pub fn open(dir: &Path, game: &str) -> Result<Archive, CorpusError> {
        let index_path = dir.join(ARCHIVE_FILE);
        let entries_path = dir.join(ARCHIVE_ENTRIES_FILE);

        if !index_path.exists() {
            std::fs::create_dir_all(dir).map_err(|source| IoError::Io {
                path: dir.to_path_buf(),
                source,
            })?;
            let index = ArchiveIndex {
                schema_version: SCHEMA_VERSION,
                game: game.to_string(),
                entries: Vec::new(),
            };
            write_json_pretty(&index_path, &index)?;
            std::fs::write(&entries_path, b"").map_err(|source| IoError::Io {
                path: entries_path.clone(),
                source,
            })?;
            return Ok(Archive {
                dir: dir.to_path_buf(),
                index,
                entries: Vec::new(),
            });
        }

        let index: ArchiveIndex = read_json(&index_path)?;
        check_schema_version(&index_path, index.schema_version)?;
        if index.game != game {
            return Err(CorpusError::Config(format!(
                "archive at {} belongs to game `{}`, not `{game}`",
                dir.display(),
                index.game
            )));
        }

        let entries: Vec<ArchiveEntry> = if entries_path.exists() {
            read_jsonl(&entries_path)?
        } else {
            Vec::new()
        };
        for entry in &entries {
            check_schema_version(&entries_path, entry.schema_version)?;
        }

        if index.entries.len() != entries.len()
            || index
                .entries
                .iter()
                .zip(entries.iter())
                .any(|(idx, entry)| idx.entry_id != entry.entry_id || idx.sequence != entry.sequence)
        {
            return Err(CorpusError::Io(IoError::Invalid(format!(
                "archive index at {} does not agree with entries at {}",
                index_path.display(),
                entries_path.display()
            ))));
        }

        Ok(Archive {
            dir: dir.to_path_buf(),
            index,
            entries,
        })
    }

    /// Appends `entry`, computing its identity, sequence and novelty. Returns
    /// [`Appended::Duplicate`] without writing anything if an entry with the same identity
    /// (game, spec, evaluation id) is already archived.
    pub fn append(&mut self, entry: NewEntry) -> Result<Appended, CorpusError> {
        let id = entry_id(&self.index.game, &entry.spec, &entry.provenance.evaluation_id)?;
        if self.get(&id).is_some() {
            return Ok(Appended::Duplicate(id));
        }

        let sequence = self.entries.len();
        let novelty = novelty(&entry.spec, entry.evaluation.signature.as_ref(), &self.entries);
        let spec_hash = config_hash(&entry.spec)?;
        let archive_entry = ArchiveEntry {
            schema_version: SCHEMA_VERSION,
            entry_id: id.clone(),
            sequence,
            name: entry.name,
            kind: entry.spec.kind().to_string(),
            spec: entry.spec,
            spec_hash,
            provenance: entry.provenance,
            evaluation: entry.evaluation,
            novelty,
        };

        self.index.entries.push(ArchiveIndexEntry {
            entry_id: id.clone(),
            name: archive_entry.name.clone(),
            kind: archive_entry.kind.clone(),
            sequence,
        });
        write_json_pretty(&self.dir.join(ARCHIVE_FILE), &self.index)?;

        let entries_path = self.dir.join(ARCHIVE_ENTRIES_FILE);
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(&entries_path)
            .map_err(|source| IoError::Io {
                path: entries_path.clone(),
                source,
            })?;
        let line = serde_json::to_string(&archive_entry).map_err(|source| IoError::Json {
            path: entries_path.clone(),
            line: sequence + 1,
            source,
        })?;
        writeln!(file, "{line}").map_err(|source| IoError::Io {
            path: entries_path.clone(),
            source,
        })?;

        self.entries.push(archive_entry);
        Ok(Appended::New(id))
    }

    /// Looks up an archived entry by its `entry_id`.
    pub fn get(&self, entry_id: &str) -> Option<&ArchiveEntry> {
        self.entries.iter().find(|entry| entry.entry_id == entry_id)
    }

    /// Every archived entry, in append (`sequence`) order.
    pub fn entries(&self) -> &[ArchiveEntry] {
        &self.entries
    }

    /// The archive's index document.
    pub fn index(&self) -> &ArchiveIndex {
        &self.index
    }

    /// Number of archived entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the archive has no entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The game this archive holds entries for.
    pub fn game(&self) -> &str {
        &self.index.game
    }

    /// The directory this archive was opened from.
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

/// Identity key an archive entry is hashed from: a change to any field is a different entry.
#[derive(Serialize)]
struct EntryKey<'a> {
    game: &'a str,
    spec: &'a StrategySpec,
    evaluation_id: &'a str,
}

/// Computes an entry's identity: `config_hash` of `(game, spec, evaluation_id)`.
pub fn entry_id(game: &str, spec: &StrategySpec, evaluation_id: &str) -> Result<String, CorpusError> {
    Ok(config_hash(&EntryKey {
        game,
        spec,
        evaluation_id,
    })?)
}

/// Distance in `[0, 1]` between two strategy specs. `1.0` when the specs are different kinds.
pub fn spec_distance(a: &StrategySpec, b: &StrategySpec) -> f64 {
    if a.kind() != b.kind() {
        return 1.0;
    }
    match (a, b) {
        (StrategySpec::Random, StrategySpec::Random) => 0.0,
        (StrategySpec::Minimax(x), StrategySpec::Minimax(y)) => {
            let depth_term = if x.depth == 0 && y.depth == 0 {
                0.0
            } else {
                (x.depth as f64 - y.depth as f64).abs() / x.depth.max(y.depth) as f64
            };
            let epsilon_term = (x.epsilon - y.epsilon).abs();
            let tie_break_term = if x.tie_break != y.tie_break { 0.1 } else { 0.0 };
            depth_term.max(epsilon_term).max(tie_break_term)
        }
        (StrategySpec::HeuristicRules { heuristic: x }, StrategySpec::HeuristicRules { heuristic: y }) => {
            let feature_set = |heuristic: &HeuristicStrategy| -> BTreeSet<String> {
                let mut set: BTreeSet<String> = heuristic
                    .rules
                    .iter()
                    .map(|rule| serde_json::to_string(rule).expect("strategy spec serializes"))
                    .collect();
                set.insert(format!(
                    "fallback:{}",
                    serde_json::to_string(&heuristic.fallback).expect("strategy spec serializes")
                ));
                set
            };
            let sa = feature_set(x);
            let sb = feature_set(y);
            if sa.is_empty() && sb.is_empty() {
                0.0
            } else {
                let intersection = sa.intersection(&sb).count();
                let union = sa.union(&sb).count();
                1.0 - (intersection as f64 / union as f64)
            }
        }
        (StrategySpec::Evolutionary, StrategySpec::Evolutionary) => 0.0,
        (StrategySpec::Llm, StrategySpec::Llm) => 0.0,
        _ => unreachable!("kind() matched but spec variants differ"),
    }
}

/// Distance in `[0, 1]` between two behavior signatures, or `None` when they are not comparable
/// (different sample or action-count).
pub fn behavior_distance(a: &BehaviorSignature, b: &BehaviorSignature) -> Option<f64> {
    if a.sample_id != b.sample_id || a.actions.len() != b.actions.len() {
        return None;
    }
    if a.actions.is_empty() {
        return Some(0.0);
    }
    let mismatches = a.actions.iter().zip(b.actions.iter()).filter(|(x, y)| x != y).count();
    Some(mismatches as f64 / a.actions.len() as f64)
}

/// Novelty of `spec` (with optional `signature`) relative to `existing` archived entries.
///
/// An empty archive is maximally novel. Otherwise the nearest existing entry is the one
/// minimizing `behavior_distance` when comparable, else `spec_distance`; ties keep the earliest
/// entry in `existing`.
pub fn novelty(spec: &StrategySpec, signature: Option<&BehaviorSignature>, existing: &[ArchiveEntry]) -> Novelty {
    if existing.is_empty() {
        return Novelty {
            method: NOVELTY_METHOD.to_string(),
            nearest_entry_id: None,
            nearest_name: None,
            spec_distance: 1.0,
            behavior_distance: None,
            distance: 1.0,
            is_novel: true,
        };
    }

    let mut best_spec_distance = f64::INFINITY;
    let mut best_behavior_distance: Option<f64> = None;
    let mut nearest: Option<&ArchiveEntry> = None;
    let mut best_distance = f64::INFINITY;

    for entry in existing {
        let sd = spec_distance(spec, &entry.spec);
        let bd = match (signature, entry.evaluation.signature.as_ref()) {
            (Some(a), Some(b)) => behavior_distance(a, b),
            _ => None,
        };
        let d = bd.unwrap_or(sd);

        best_spec_distance = best_spec_distance.min(sd);
        best_behavior_distance = match (best_behavior_distance, bd) {
            (None, bd) => bd,
            (Some(cur), Some(v)) => Some(cur.min(v)),
            (cur, None) => cur,
        };
        if d < best_distance {
            best_distance = d;
            nearest = Some(entry);
        }
    }

    let distance = best_behavior_distance.unwrap_or(best_spec_distance);
    Novelty {
        method: NOVELTY_METHOD.to_string(),
        nearest_entry_id: nearest.map(|entry| entry.entry_id.clone()),
        nearest_name: nearest.map(|entry| entry.name.clone()),
        spec_distance: best_spec_distance,
        behavior_distance: best_behavior_distance,
        distance,
        is_novel: distance > 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::dsl::{ActionSelector, HeuristicStrategy, Rule};
    use crate::core::features::{FeatureExpr, FeatureValue};
    use crate::io::schema::{Headline, Tally, TournamentResult};
    use crate::strategy::minimax::{MinimaxConfig, TieBreak};

    fn temp_dir() -> tempfile::TempDir {
        tempfile::Builder::new()
            .prefix("m1-archive-")
            .tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/target"))
            .unwrap()
    }

    fn minimax(depth: u32, epsilon: f64, tie_break: TieBreak) -> StrategySpec {
        StrategySpec::Minimax(MinimaxConfig {
            depth,
            epsilon,
            tie_break,
        })
    }

    fn sig(id: &str, actions: &[u64]) -> BehaviorSignature {
        BehaviorSignature {
            sample_id: id.to_string(),
            actions: actions.iter().map(|a| serde_json::Value::from(*a)).collect(),
        }
    }

    fn sample_evaluation(name: &str, spec: &StrategySpec, signature: Option<BehaviorSignature>) -> StrategyEvaluation {
        StrategyEvaluation {
            name: name.to_string(),
            kind: spec.kind().to_string(),
            spec: spec.clone(),
            spec_hash: config_hash(spec).unwrap(),
            tournament: TournamentResult {
                roster_id: "tiny-v1".to_string(),
                reference: "perfect".to_string(),
                games_per_pairing: 0,
                seed: 0,
                opponents: Vec::new(),
                totals: Tally::default(),
            },
            headline: Headline {
                reference: "perfect".to_string(),
                loss_rate_vs_reference: 0.0,
                win_rate: 0.0,
                draw_rate: 0.0,
                loss_rate: 0.0,
                games: 0,
                unfinished: 0,
                agreement_rate: None,
            },
            agreement: None,
            signature,
        }
    }

    fn sample_provenance(evaluation_id: &str) -> Provenance {
        Provenance {
            game: "testgame".to_string(),
            source: "evaluate".to_string(),
            evaluation_id: evaluation_id.to_string(),
            roster_id: "tiny-v1".to_string(),
            seed: 0,
            games_per_pairing: 0,
            evaluator: "default".to_string(),
            corpus_run_id: None,
            annotations_run_id: None,
            annotations_mode: None,
        }
    }

    #[test]
    fn spec_distance_table() {
        let m9 = minimax(9, 0.0, TieBreak::SeededUniform);
        assert_eq!(spec_distance(&m9, &m9), 0.0);
        assert_eq!(spec_distance(&StrategySpec::Random, &StrategySpec::Random), 0.0);
        assert_eq!(spec_distance(&StrategySpec::Random, &m9), 1.0);

        let empty_h = StrategySpec::HeuristicRules {
            heuristic: HeuristicStrategy::new("h"),
        };
        assert_eq!(spec_distance(&m9, &empty_h), 1.0);

        let m3 = minimax(3, 0.0, TieBreak::SeededUniform);
        assert!((spec_distance(&m3, &m9) - 2.0 / 3.0).abs() < 1e-9);

        let eps0 = minimax(9, 0.0, TieBreak::SeededUniform);
        let eps25 = minimax(9, 0.25, TieBreak::SeededUniform);
        assert_eq!(spec_distance(&eps0, &eps25), 0.25);

        let tb_engine = minimax(9, 0.0, TieBreak::Engine);
        let tb_seeded = minimax(9, 0.0, TieBreak::SeededUniform);
        assert_eq!(spec_distance(&tb_engine, &tb_seeded), 0.1);

        let m3_engine = minimax(3, 0.0, TieBreak::Engine);
        assert!((spec_distance(&m3_engine, &m9) - 2.0 / 3.0).abs() < 1e-9);

        let empty_h1 = StrategySpec::HeuristicRules {
            heuristic: HeuristicStrategy::new("one"),
        };
        let empty_h2 = StrategySpec::HeuristicRules {
            heuristic: HeuristicStrategy::new("two"),
        };
        assert_eq!(spec_distance(&empty_h1, &empty_h2), 0.0);

        let mut one_rule = HeuristicStrategy::new("with-rule");
        one_rule.rules.push(Rule::new(
            "r",
            0,
            FeatureExpr::Const {
                value: FeatureValue::Bool(true),
            },
            ActionSelector::AnyLegal,
        ));
        let one_rule_spec = StrategySpec::HeuristicRules { heuristic: one_rule };
        assert_eq!(spec_distance(&one_rule_spec, &empty_h1), 0.5);

        let mut h_max = HeuristicStrategy::new("max-fallback");
        h_max.fallback = ActionSelector::Maximize {
            expr: FeatureExpr::Const {
                value: FeatureValue::Int(0),
            },
        };
        let h_max_spec = StrategySpec::HeuristicRules { heuristic: h_max };
        assert_eq!(spec_distance(&empty_h1, &h_max_spec), 1.0);
    }

    #[test]
    fn behavior_distance_cases() {
        assert_eq!(behavior_distance(&sig("a", &[1, 2]), &sig("b", &[1, 2])), None);
        assert_eq!(
            behavior_distance(&sig("s", &[1, 2, 3, 4]), &sig("s", &[1, 2, 3, 5])),
            Some(0.25)
        );
        assert_eq!(
            behavior_distance(&sig("s", &[1, 2, 3, 4]), &sig("s", &[1, 2, 3, 4])),
            Some(0.0)
        );
        assert_eq!(behavior_distance(&sig("s", &[1, 2, 3, 4]), &sig("s", &[1, 2, 3])), None);
    }

    #[test]
    fn novelty_of_empty_archive_is_maximal() {
        let m9 = minimax(9, 0.0, TieBreak::SeededUniform);
        let n = novelty(&m9, None, &[]);
        assert_eq!(
            n,
            Novelty {
                method: NOVELTY_METHOD.to_string(),
                nearest_entry_id: None,
                nearest_name: None,
                spec_distance: 1.0,
                behavior_distance: None,
                distance: 1.0,
                is_novel: true,
            }
        );
    }

    #[test]
    fn novelty_uses_behavior_distance_when_comparable() {
        let dir = temp_dir();
        let mut archive = Archive::open(dir.path(), "testgame").unwrap();

        let spec_a = minimax(9, 0.0, TieBreak::SeededUniform);
        let sig_a = sig("s1", &[0, 1, 2, 3]);
        let appended = archive
            .append(NewEntry {
                name: "a".to_string(),
                spec: spec_a.clone(),
                provenance: sample_provenance("aaaaaaaaaaaaaaaa"),
                evaluation: sample_evaluation("a", &spec_a, Some(sig_a.clone())),
            })
            .unwrap();
        let id_a = match appended {
            Appended::New(id) => id,
            other => panic!("expected New, got {other:?}"),
        };

        // (i) same spec, differing signature.
        let n1 = novelty(&spec_a, Some(&sig("s1", &[0, 1, 2, 9])), archive.entries());
        assert_eq!(n1.spec_distance, 0.0);
        assert_eq!(n1.behavior_distance, Some(0.25));
        assert_eq!(n1.distance, 0.25);
        assert!(n1.is_novel);
        assert_eq!(n1.nearest_entry_id, Some(id_a.clone()));
        assert_eq!(n1.nearest_name, Some("a".to_string()));

        // (ii) identical spec and identical signature.
        let n2 = novelty(&spec_a, Some(&sig_a), archive.entries());
        assert_eq!(n2.distance, 0.0);
        assert!(!n2.is_novel);

        // (iii) different depth, incomparable signature (different sample id).
        let spec_3 = minimax(3, 0.0, TieBreak::SeededUniform);
        let n3 = novelty(&spec_3, Some(&sig("other", &[0, 1])), archive.entries());
        assert_eq!(n3.behavior_distance, None);
        assert!((n3.distance - 2.0 / 3.0).abs() < 1e-9);
        assert!(n3.is_novel);

        let spec_b = minimax(9, 0.0, TieBreak::SeededUniform);
        archive
            .append(NewEntry {
                name: "b".to_string(),
                spec: spec_b.clone(),
                provenance: sample_provenance("bbbbbbbbbbbbbbbb"),
                evaluation: sample_evaluation("b", &spec_b, None),
            })
            .unwrap();
        let b_entry = archive.entries().last().unwrap();
        assert_eq!(b_entry.novelty.distance, 0.0);
        assert_eq!(b_entry.novelty.nearest_entry_id, Some(id_a));
    }

    #[test]
    fn open_append_duplicate_and_reopen_round_trip() {
        let dir = temp_dir();
        let mut archive = Archive::open(dir.path(), "testgame").unwrap();
        assert!(archive.is_empty());
        assert!(dir.path().join(ARCHIVE_FILE).exists());
        assert!(dir.path().join(ARCHIVE_ENTRIES_FILE).exists());

        let spec_a = minimax(9, 0.0, TieBreak::SeededUniform);
        let new_a = NewEntry {
            name: "a".to_string(),
            spec: spec_a.clone(),
            provenance: sample_provenance("aaaaaaaaaaaaaaaa"),
            evaluation: sample_evaluation("a", &spec_a, Some(sig("s1", &[0, 1, 2, 3]))),
        };
        let appended = archive.append(new_a.clone()).unwrap();
        let id_a = match appended {
            Appended::New(id) => id,
            other => panic!("expected New, got {other:?}"),
        };
        assert_eq!(id_a, entry_id("testgame", &spec_a, "aaaaaaaaaaaaaaaa").unwrap());
        assert_eq!(id_a.len(), 16);

        let index_bytes_before = std::fs::read(dir.path().join(ARCHIVE_FILE)).unwrap();
        let entries_bytes_before = std::fs::read(dir.path().join(ARCHIVE_ENTRIES_FILE)).unwrap();

        let appended_again = archive.append(new_a).unwrap();
        assert_eq!(appended_again, Appended::Duplicate(id_a.clone()));
        assert_eq!(archive.len(), 1);
        assert_eq!(std::fs::read(dir.path().join(ARCHIVE_FILE)).unwrap(), index_bytes_before);
        assert_eq!(
            std::fs::read(dir.path().join(ARCHIVE_ENTRIES_FILE)).unwrap(),
            entries_bytes_before
        );

        let spec_b = StrategySpec::Random;
        let appended_b = archive
            .append(NewEntry {
                name: "b".to_string(),
                spec: spec_b.clone(),
                provenance: sample_provenance("cccccccccccccccc"),
                evaluation: sample_evaluation("b", &spec_b, None),
            })
            .unwrap();
        let id_b = match appended_b {
            Appended::New(id) => id,
            other => panic!("expected New, got {other:?}"),
        };
        assert_eq!(archive.len(), 2);
        assert_eq!(archive.get(&id_b).unwrap().sequence, 1);
        assert_eq!(archive.get(&id_b).unwrap().novelty.nearest_entry_id, Some(id_a.clone()));
        assert_eq!(archive.index().entries.len(), 2);

        let reopened = Archive::open(dir.path(), "testgame").unwrap();
        assert_eq!(reopened.entries(), archive.entries());
        assert_eq!(reopened.index(), archive.index());

        let entries_path = dir.path().join(ARCHIVE_ENTRIES_FILE);
        let from_jsonl: Vec<ArchiveEntry> = read_jsonl(&entries_path).unwrap();
        assert_eq!(from_jsonl, archive.entries());

        let tmp_entries = dir.path().join("entries-check.jsonl");
        crate::io::write_jsonl(&tmp_entries, archive.entries().iter()).unwrap();
        assert_eq!(std::fs::read(&tmp_entries).unwrap(), std::fs::read(&entries_path).unwrap());

        let tmp_index = dir.path().join("archive-check.json");
        write_json_pretty(&tmp_index, archive.index()).unwrap();
        assert_eq!(
            std::fs::read(&tmp_index).unwrap(),
            std::fs::read(dir.path().join(ARCHIVE_FILE)).unwrap()
        );

        let entries_text = std::fs::read_to_string(&entries_path).unwrap();
        for line in entries_text.lines() {
            let _: serde_json::Value = serde_json::from_str(line).unwrap();
        }
    }

    #[test]
    fn open_rejects_another_game() {
        let dir = temp_dir();
        Archive::open(dir.path(), "testgame").unwrap();

        let err = Archive::open(dir.path(), "othergame").unwrap_err();
        assert!(matches!(&err, CorpusError::Config(msg) if msg.contains("belongs to game `testgame`")));
    }
}
