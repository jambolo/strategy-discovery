//! Game-neutral `discover` orchestration.
//!
//! `run_discover` picks up where the `generate -> annotate -> analyze` chain (driven by the
//! CLI layer, not this module) leaves off: it reads a run directory's `run.json` and
//! `heuristics.json`, evaluates every mined candidate against the benchmark roster, archives
//! each evaluated candidate with rerun-sufficient discovery provenance, and writes a
//! `discover.json` manifest into the run directory. It never reads `dataset.jsonl` or
//! re-derives the dataset: candidates come only from the mining report. When
//! `[induction] enabled` was set for the run, each candidate's discovery provenance also
//! records the induction outcome (read back from `concepts.json`) via [`ConceptsProvenance`].
//!
//! This module never prints to stdout; the CLI layer reports on [`DiscoverOutcome`].

use std::path::Path;

use crate::discovery::analyze::RunHeader;
use crate::discovery::archive::{Appended, Archive, NewEntry};
use crate::discovery::bundle::GameBundle;
use crate::discovery::config::CorpusError;
use crate::discovery::evaluate::{evaluate_strategies, load_annotations, mode_name};
use crate::discovery::experiment::ResolvedExperiment;
use crate::io::{
    CONCEPTS_FILE, CandidateParams, ConceptReport, ConceptsProvenance, CorpusGame, DISCOVER_FILE, DiscoverCandidate,
    DiscoverManifest, DiscoveryProvenance, EVALUATION_FILE, EvaluationReport, HEURISTICS_FILE, MINE_ENGINE_CART,
    MINE_ENGINE_LINFA_TREES, MiningReport, Provenance, RUN_FILE, SCHEMA_VERSION, config_hash, read_json, write_json_pretty,
};
use crate::strategy::engine::EngineGame;
use crate::strategy::registry::StrategySpec;
use crate::strategy::roster::RosterEntry;

/// Everything the `discover` stage produced, for the CLI layer to print from.
pub struct DiscoverOutcome {
    /// The `evaluate` stage's report, also persisted to `evaluation.json`.
    pub report: EvaluationReport,
    /// The `discover` stage's manifest, also persisted to `discover.json`.
    pub manifest: DiscoverManifest,
    /// One novelty distance per candidate, in candidate order.
    pub novelties: Vec<f64>,
    /// The archive's total entry count after all appends.
    pub archive_entries: usize,
}

/// Runs the `discover` stage's library orchestration: evaluate mined candidates, archive them
/// with discovery provenance, and write `discover.json`.
///
/// `config_hash` is the hash of the experiment config as loaded, computed by the CLI layer.
pub fn run_discover<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    resolved: &ResolvedExperiment<G::Action>,
    config_hash: &str,
) -> Result<DiscoverOutcome, CorpusError> {
    let header: RunHeader = read_json(&resolved.out.join(RUN_FILE))?;
    let mining: MiningReport = read_json(&resolved.out.join(HEURISTICS_FILE))?;

    let concepts_path = resolved.out.join(CONCEPTS_FILE);
    let concepts = if resolved.analyze.induction.enabled && concepts_path.exists() {
        let report: ConceptReport = read_json(&concepts_path)?;
        Some(concepts_provenance(&report)?)
    } else {
        None
    };

    let strategies: Vec<RosterEntry> = mining
        .candidates
        .iter()
        .map(|candidate| RosterEntry {
            name: candidate.name.clone(),
            spec: StrategySpec::HeuristicRules {
                heuristic: candidate.heuristic.clone(),
            },
        })
        .collect();

    let annotations = load_annotations(bundle, &resolved.out)?;

    let discover = &resolved.discover;
    let report = evaluate_strategies(
        bundle,
        &strategies,
        discover.roster.as_ref(),
        Some(&annotations),
        &discover.config,
    )?;
    write_json_pretty(&resolved.out.join(EVALUATION_FILE), &report)?;

    let mut archive = Archive::open(&discover.archive_dir, &bundle.name)?;
    let mut novelties: Vec<f64> = Vec::with_capacity(mining.candidates.len());
    let mut candidates: Vec<DiscoverCandidate> = Vec::with_capacity(mining.candidates.len());

    for (candidate, evaluation) in mining.candidates.iter().zip(&report.strategies) {
        let provenance = Provenance {
            game: report.game.clone(),
            source: "discover".to_string(),
            evaluation_id: report.evaluation_id.clone(),
            roster_id: report.roster.id(),
            seed: discover.config.seed,
            games_per_pairing: discover.config.games_per_pairing,
            evaluator: discover.config.evaluator.clone(),
            corpus_run_id: Some(header.run_id.clone()),
            annotations_run_id: report.config.annotations.as_ref().and_then(|a| a.run_id.clone()),
            annotations_mode: report.config.annotations.as_ref().map(|a| a.mode.clone()),
            discovery: Some(DiscoveryProvenance {
                experiment: resolved.name.clone(),
                config_hash: config_hash.to_string(),
                corpus_run_id: header.run_id.clone(),
                annotations_mode: mode_name(annotations.metadata.mode).to_string(),
                miner: miner_id(&candidate.engine),
                params: CandidateParams {
                    engine: candidate.engine.clone(),
                    max_depth: candidate.max_depth,
                    min_leaf: candidate.min_leaf,
                },
                mine_seed: mining.params.seed,
                holdout_fraction: mining.params.holdout_fraction,
                tiers: mining.dataset.tiers.clone(),
                dataset_rows: mining.dataset.rows,
                candidate: candidate.name.clone(),
                concepts: concepts.clone(),
            }),
        };

        let appended = archive.append(NewEntry {
            name: evaluation.name.clone(),
            spec: evaluation.spec.clone(),
            provenance,
            evaluation: evaluation.clone(),
        })?;
        let id = match appended {
            Appended::New(id) | Appended::Duplicate(id) => id,
        };
        let entry = archive
            .get(&id)
            .ok_or_else(|| CorpusError::Config(format!("archive entry `{id}` vanished after append")))?;
        let distance = entry.novelty.distance;
        novelties.push(distance);
        candidates.push(DiscoverCandidate {
            name: candidate.name.clone(),
            entry_id: id,
            rules: candidate.rules,
            loss_rate_vs_reference: evaluation.headline.loss_rate_vs_reference,
            agreement_rate: evaluation.headline.agreement_rate,
            novelty: distance,
        });
    }

    let manifest = DiscoverManifest {
        schema_version: SCHEMA_VERSION,
        name: resolved.name.clone(),
        game: bundle.name.clone(),
        run_id: header.run_id.clone(),
        config_hash: config_hash.to_string(),
        evaluation_id: report.evaluation_id.clone(),
        roster_id: report.roster.id(),
        archive: archive_display(&discover.archive_dir, &resolved.out),
        candidates,
    };
    write_json_pretty(&resolved.out.join(DISCOVER_FILE), &manifest)?;

    Ok(DiscoverOutcome {
        archive_entries: archive.len(),
        report,
        manifest,
        novelties,
    })
}

/// Maps a `concepts` analyzer's [`ConceptReport`] to the rerun-sufficient
/// [`ConceptsProvenance`] recorded on every archived candidate when induction ran.
fn concepts_provenance(report: &ConceptReport) -> Result<ConceptsProvenance, CorpusError> {
    Ok(ConceptsProvenance {
        params_hash: config_hash(&report.params)?,
        withhold_tier2: report.params.withhold_tier2,
        rounds_run: report.rounds.len(),
        promoted: report.promoted.len(),
        concepts: report.promoted.iter().map(|p| p.name.clone()).collect(),
        seed: report.params.seed,
    })
}

/// Maps a `MineParams::engine` value to the stable miner identifier recorded in provenance.
fn miner_id(engine: &str) -> String {
    if engine == MINE_ENGINE_CART {
        "cart-v1".to_string()
    } else if engine == MINE_ENGINE_LINFA_TREES {
        "linfa-trees-0.8.1".to_string()
    } else {
        engine.to_string()
    }
}

/// Renders an archive directory for persistence in `discover.json`: forward-slash-joined and
/// relative to the run directory when possible, else the directory's own display form.
fn archive_display(archive_dir: &Path, out: &Path) -> String {
    match archive_dir.strip_prefix(out) {
        Ok(relative) => {
            let joined = relative
                .components()
                .map(|component| component.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            if joined.is_empty() { ".".to_string() } else { joined }
        }
        Err(_) => archive_dir.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn miner_id_maps_both_engines() {
        assert_eq!(miner_id(MINE_ENGINE_CART), "cart-v1");
        assert_eq!(miner_id(MINE_ENGINE_LINFA_TREES), "linfa-trees-0.8.1");
    }

    #[test]
    fn archive_display_is_relative_to_out() {
        let out = Path::new("runs/x");
        assert_eq!(archive_display(&out.join("archive"), out), "archive");
        assert_eq!(archive_display(&out.join("a").join("b"), out), "a/b");
    }

    #[test]
    fn archive_display_keeps_absolute_paths() {
        let out = Path::new("runs/x");
        let archive_dir = Path::new("C:/elsewhere/archive");
        assert_eq!(archive_display(archive_dir, out), archive_dir.display().to_string());
    }

    #[test]
    fn concepts_provenance_maps_report_fields() {
        use crate::io::InductionParams;
        use std::collections::BTreeMap;

        let report = ConceptReport {
            schema_version: 1,
            game: "g".to_string(),
            run_id: None,
            params: InductionParams {
                withhold_tier2: true,
                seed: 7,
                ..InductionParams::default()
            },
            withheld: vec![],
            rows: 0,
            train_rows: 0,
            holdout_rows: 0,
            base_columns: 0,
            base_classes: 0,
            rounds: vec![crate::io::RoundReport {
                round: 1,
                columns: 0,
                enumerated: 0,
                deduplicated: 0,
                scored: 5,
                passed: 0,
                shortlisted: vec![],
                promoted: vec![],
            }],
            promoted: vec![],
            labels: BTreeMap::new(),
        };

        let provenance = concepts_provenance(&report).expect("maps cleanly");
        assert_eq!(provenance.params_hash.len(), 16);
        assert!(provenance.withhold_tier2);
        assert_eq!(provenance.rounds_run, 1);
        assert_eq!(provenance.promoted, 0);
        assert!(provenance.concepts.is_empty());
        assert_eq!(provenance.seed, 7);
    }
}
