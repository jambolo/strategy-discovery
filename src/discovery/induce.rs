//! v1 concept induction: mints candidate boolean concepts over an existing feature dataset,
//! scores them by information gain against the value and label targets, deduplicates them
//! extensionally, and promotes a shortlisted candidate only when adding it to a downstream
//! CART probe genuinely improves the induced heuristic (fewer rules at no soundness cost, or
//! the same rule count with higher soundness).
//!
//! The enumeration grammar (levels 1 and 2) builds candidates over the dataset's own columns:
//! `Cmp` (equality/threshold comparisons on int/set atoms, `Count` on set columns), `And`,
//! `Or`, and `Not` combinations of level-1 survivors. Every candidate is deduplicated against
//! its full-row bit vector (train + holdout) before scoring, so a bare reference to an
//! existing column and any later expression with identical extension are dropped for free.
//! Promoted concepts are named `concept_{n}` by a global 1-based counter that advances only on
//! promotion. See [`induce_concepts`] for the entry point and the crate's phase brief for the
//! full algorithm this module implements exactly.

use crate::core::features::{CmpOp, FeatureDef, FeatureExpr, FeatureValue, Tier};
use crate::core::traits::{GameDomain, GeneratorError};
use crate::discovery::analyze::{AnalyzeContext, AnalyzeOptions, Analyzer, AnalyzerOutput};
use crate::discovery::bundle::GameBundle;
use crate::discovery::config::CorpusError;
use crate::discovery::dataset::{Dataset, dataset_from_context, induction_spec};
use crate::discovery::mine::{mine_one, split_train_holdout};
use crate::io::{
    CONCEPTS_FILE, ConceptProvenance, ConceptReport, ConceptScores, CorpusGame, DatasetColumn, InductionParams, IoError,
    MINE_ENGINE_CART, PromotedConcept, RoundReport, SCHEMA_VERSION, ShortlistEntry, SimilarityEntry, config_hash,
};
use crate::strategy::engine::EngineGame;

/// One scored, deduplicated candidate concept produced during enumeration.
#[derive(Clone)]
struct Candidate {
    /// The candidate's feature expression.
    expr: FeatureExpr,
    /// Full-row (train + holdout, dataset row order) boolean extension.
    bits: Vec<bool>,
    /// Information gain on the training split.
    train_gain: f64,
    /// Information gain on the holdout split, when a holdout exists.
    holdout_gain: Option<f64>,
    /// Position in the round's enumeration sequence, used as a stable tie-break.
    seq: usize,
}

/// One atom extracted from the current round's dataset columns: a predicate or numeric source
/// expression together with its per-row integer/boolean values, ready for threshold or equality
/// candidate construction.
enum Atom {
    /// A `bool`-kind column: `Ref(name)`, bit is `value != 0.0`.
    Predicate { expr: FeatureExpr, bits: Vec<bool> },
    /// An `int`-kind column (`Ref(name)`) or a `set`-kind column (`Count(Ref(name))`), values as
    /// integers.
    Numeric { expr: FeatureExpr, values: Vec<i64> },
}

/// Result of one round's enumerate/dedupe/score/filter pass.
struct RoundPass {
    /// Passed candidates, ranked by shortlist order, truncated to `top_k`.
    shortlist: Vec<Candidate>,
    /// Total candidates processed (levels 1 + D + 2).
    enumerated: usize,
    /// Candidates dropped by deduplication (constant or duplicate).
    deduplicated: usize,
    /// Candidates admitted to scoring.
    scored: usize,
    /// Candidates passing both gain filters.
    passed: usize,
}

/// Shannon entropy contribution of one class fraction `p` in bits; `0 * log2(0) == 0`.
fn h_term(p: f64) -> f64 {
    if p <= 0.0 { 0.0 } else { -p * p.log2() }
}

/// Entropy in bits of `target` restricted to `indices`, base 2, with `0 * log2(0) = 0`.
fn entropy(target: &[usize], indices: &[usize]) -> f64 {
    if indices.is_empty() {
        return 0.0;
    }
    let mut counts: Vec<usize> = Vec::new();
    for &i in indices {
        let c = target[i];
        if c >= counts.len() {
            counts.resize(c + 1, 0);
        }
        counts[c] += 1;
    }
    let n = indices.len() as f64;
    counts.iter().map(|&c| h_term(c as f64 / n)).sum()
}

/// Information gain of splitting `indices` on `bits` against `target`: `H(t|S) - sum_v (|S_v|/|S|)
/// H(t|S_v)`, with an empty `S_v` contributing `0`.
fn info_gain(bits: &[bool], target: &[usize], indices: &[usize]) -> f64 {
    if indices.is_empty() {
        return 0.0;
    }
    let base = entropy(target, indices);
    let (false_idx, true_idx): (Vec<usize>, Vec<usize>) = indices.iter().partition(|&&i| !bits[i]);
    let n = indices.len() as f64;
    let weighted =
        (false_idx.len() as f64 / n) * entropy(target, &false_idx) + (true_idx.len() as f64 / n) * entropy(target, &true_idx);
    base - weighted
}

/// `max(gain against value_target, gain against label_target)` over `indices`.
fn gain_pair(bits: &[bool], value_target: &[usize], label_target: &[usize], indices: &[usize]) -> f64 {
    let value_gain = info_gain(bits, value_target, indices);
    let label_gain = info_gain(bits, label_target, indices);
    value_gain.max(label_gain)
}

/// Extracts one [`Atom`] per dataset column in column order, skipping `SIDE_TO_MOVE` and
/// `float`-kind columns (no atom in v1).
fn extract_atoms(columns: &[DatasetColumn], row_values: &[&Vec<f64>]) -> Vec<Atom> {
    let mut atoms = Vec::new();
    for (j, col) in columns.iter().enumerate() {
        if col.name == crate::core::featurizer::SIDE_TO_MOVE {
            continue;
        }
        match col.kind.as_str() {
            "bool" => {
                let bits: Vec<bool> = row_values.iter().map(|v| v[j] != 0.0).collect();
                atoms.push(Atom::Predicate {
                    expr: FeatureExpr::named(&col.name),
                    bits,
                });
            }
            "int" => {
                let values: Vec<i64> = row_values.iter().map(|v| v[j] as i64).collect();
                atoms.push(Atom::Numeric {
                    expr: FeatureExpr::named(&col.name),
                    values,
                });
            }
            "set" => {
                let values: Vec<i64> = row_values.iter().map(|v| v[j] as i64).collect();
                atoms.push(Atom::Numeric {
                    expr: FeatureExpr::count(FeatureExpr::named(&col.name)),
                    values,
                });
            }
            _ => {}
        }
    }
    atoms
}

/// Tries to admit one candidate expression with full-row `bits`: drops constants and extensional
/// duplicates against `seen`, else scores it and records pass/fail. Updates the round counters
/// and (when admitted) `seen`/`scored_out`.
#[allow(clippy::too_many_arguments)]
fn try_admit(
    expr: FeatureExpr,
    bits: Vec<bool>,
    seq: usize,
    train: &[usize],
    holdout: &[usize],
    value_target: &[usize],
    label_target: &[usize],
    params: &InductionParams,
    seen: &mut Vec<Vec<bool>>,
    enumerated: &mut usize,
    deduplicated: &mut usize,
    scored: &mut usize,
    passed: &mut usize,
    scored_out: &mut Vec<Candidate>,
) {
    *enumerated += 1;
    let constant = bits.iter().all(|&b| b) || bits.iter().all(|&b| !b);
    if constant || seen.contains(&bits) {
        *deduplicated += 1;
        return;
    }
    seen.push(bits.clone());
    *scored += 1;

    let train_gain = gain_pair(&bits, value_target, label_target, train);
    let holdout_gain = if holdout.is_empty() {
        None
    } else {
        Some(gain_pair(&bits, value_target, label_target, holdout))
    };

    let passes = train_gain >= params.min_train_gain && holdout_gain.is_none_or(|g| g >= params.min_holdout_gain);
    if passes {
        *passed += 1;
        scored_out.push(Candidate {
            expr,
            bits,
            train_gain,
            holdout_gain,
            seq,
        });
    }
}

/// Enumerates and scores one round's candidates against `dataset`'s current columns, returning
/// the shortlist (ranked, truncated to `top_k`) and the round's enumeration counters.
fn enumerate_and_score<G: GameDomain>(
    dataset: &Dataset<G>,
    train: &[usize],
    holdout: &[usize],
    value_target: &[usize],
    label_target: &[usize],
    params: &InductionParams,
) -> RoundPass {
    let row_values: Vec<&Vec<f64>> = dataset.rows.iter().map(|r| &r.values).collect();
    let atoms = extract_atoms(&dataset.manifest.columns, &row_values);

    let mut seen: Vec<Vec<bool>> = Vec::new();
    let mut enumerated = 0usize;
    let mut deduplicated = 0usize;
    let mut scored = 0usize;
    let mut passed = 0usize;
    let mut seq = 0usize;
    let mut level1: Vec<Candidate> = Vec::new();

    macro_rules! admit {
        ($expr:expr, $bits:expr) => {{
            let s = seq;
            seq += 1;
            try_admit(
                $expr,
                $bits,
                s,
                train,
                holdout,
                value_target,
                label_target,
                params,
                &mut seen,
                &mut enumerated,
                &mut deduplicated,
                &mut scored,
                &mut passed,
                &mut level1,
            );
        }};
    }

    // Level 1: predicate atoms, in atom order.
    for atom in &atoms {
        if let Atom::Predicate { expr, bits } = atom {
            admit!(expr.clone(), bits.clone());
        }
    }
    // Level 1: numeric atoms, in atom order, op-major (Eq then Ge).
    for atom in &atoms {
        if let Atom::Numeric { expr, values } = atom {
            let mut thresholds: Vec<i64> = train.iter().map(|&i| values[i]).collect();
            thresholds.sort_unstable();
            thresholds.dedup();
            thresholds.truncate(params.max_thresholds);

            for &t in &thresholds {
                let bits: Vec<bool> = values.iter().map(|&v| v == t).collect();
                admit!(FeatureExpr::compare(CmpOp::Eq, expr.clone(), FeatureExpr::int(t)), bits);
            }
            for (k, &t) in thresholds.iter().enumerate() {
                if k == 0 {
                    continue;
                }
                let bits: Vec<bool> = values.iter().map(|&v| v >= t).collect();
                admit!(FeatureExpr::compare(CmpOp::Ge, expr.clone(), FeatureExpr::int(t)), bits);
            }
        }
    }
    // Family D: ordered pairs of numeric atoms, only when enabled.
    if params.compare_atoms {
        let numeric: Vec<(&FeatureExpr, &Vec<i64>)> = atoms
            .iter()
            .filter_map(|a| match a {
                Atom::Numeric { expr, values } => Some((expr, values)),
                Atom::Predicate { .. } => None,
            })
            .collect();
        for (a, (expr_a, values_a)) in numeric.iter().enumerate() {
            for (b, (expr_b, values_b)) in numeric.iter().enumerate() {
                if a == b {
                    continue;
                }
                let bits: Vec<bool> = values_a.iter().zip(values_b.iter()).map(|(&x, &y)| x > y).collect();
                admit!(FeatureExpr::compare(CmpOp::Gt, (*expr_a).clone(), (*expr_b).clone()), bits);
            }
        }
    }

    // Beam: level-1 (+ Family D) passed candidates ranked by (train_gain desc, seq asc), used
    // only to generate level-2 combinations; the shortlist draws from all passed level-1
    // candidates, not just the beam.
    let level1_all = level1.clone();
    let mut beam = level1;
    beam.sort_by(|a, b| b.train_gain.total_cmp(&a.train_gain).then(a.seq.cmp(&b.seq)));
    beam.truncate(params.beam);

    let mut level2: Vec<Candidate> = Vec::new();
    for p in &beam {
        let bits: Vec<bool> = p.bits.iter().map(|&b| !b).collect();
        let s = seq;
        seq += 1;
        try_admit(
            FeatureExpr::negate(p.expr.clone()),
            bits,
            s,
            train,
            holdout,
            value_target,
            label_target,
            params,
            &mut seen,
            &mut enumerated,
            &mut deduplicated,
            &mut scored,
            &mut passed,
            &mut level2,
        );
    }
    for i in 0..beam.len() {
        for j in (i + 1)..beam.len() {
            let and_bits: Vec<bool> = beam[i].bits.iter().zip(beam[j].bits.iter()).map(|(&x, &y)| x && y).collect();
            let s = seq;
            seq += 1;
            try_admit(
                FeatureExpr::all(vec![beam[i].expr.clone(), beam[j].expr.clone()]),
                and_bits,
                s,
                train,
                holdout,
                value_target,
                label_target,
                params,
                &mut seen,
                &mut enumerated,
                &mut deduplicated,
                &mut scored,
                &mut passed,
                &mut level2,
            );

            let or_bits: Vec<bool> = beam[i].bits.iter().zip(beam[j].bits.iter()).map(|(&x, &y)| x || y).collect();
            let s = seq;
            seq += 1;
            try_admit(
                FeatureExpr::any(vec![beam[i].expr.clone(), beam[j].expr.clone()]),
                or_bits,
                s,
                train,
                holdout,
                value_target,
                label_target,
                params,
                &mut seen,
                &mut enumerated,
                &mut deduplicated,
                &mut scored,
                &mut passed,
                &mut level2,
            );
        }
    }

    let mut all_passed: Vec<Candidate> = level1_all;
    all_passed.extend(level2);

    all_passed.sort_by(|a, b| {
        let key = |c: &Candidate| c.holdout_gain.unwrap_or(c.train_gain);
        key(b)
            .total_cmp(&key(a))
            .then(b.train_gain.total_cmp(&a.train_gain))
            .then(a.seq.cmp(&b.seq))
    });
    all_passed.truncate(params.top_k);

    RoundPass {
        shortlist: all_passed,
        enumerated,
        deduplicated,
        scored,
        passed,
    }
}

/// Pure promotion predicate: a candidate is promoted only if the probe's fitted heuristic
/// actually references it, and doing so either shrinks the rule count without a meaningful
/// soundness drop, or holds the rule count and raises soundness by at least `min_soundness_gain`.
fn promotion_accepts(
    references_concept: bool,
    before_rules: usize,
    after_rules: usize,
    before_soundness: f64,
    after_soundness: f64,
    soundness_tolerance: f64,
    min_soundness_gain: f64,
) -> bool {
    references_concept
        && ((after_rules < before_rules && after_soundness >= before_soundness - soundness_tolerance)
            || (after_rules <= before_rules && after_soundness >= before_soundness + min_soundness_gain))
}

/// Boolifies a [`FeatureValue`] for similarity comparison: `Bool` as-is, `Int > 0`, `Float > 0.0`,
/// `Set` non-empty.
fn boolify(value: &FeatureValue) -> bool {
    match value {
        FeatureValue::Bool(b) => *b,
        FeatureValue::Int(i) => *i > 0,
        FeatureValue::Float(f) => *f > 0.0,
        FeatureValue::Set(s) => !s.is_empty(),
    }
}

/// Runs v1 concept induction over `dataset`, returning the full report (`labels` left empty for
/// the caller to fill). See the module docs for the algorithm.
pub fn induce_concepts<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    dataset: &Dataset<G>,
    params: &InductionParams,
) -> Result<ConceptReport, CorpusError>
where
    G::State: Clone + Eq + std::hash::Hash,
{
    params.validate()?;

    let classes = &dataset.manifest.classes;
    let value_target: Vec<usize> = dataset.rows.iter().map(|r| (r.value + 1) as usize).collect();
    let label_target: Vec<usize> = dataset
        .rows
        .iter()
        .map(|r| dataset.class_index(&r.label).unwrap_or(classes.len()))
        .collect();

    let (train, holdout) = split_train_holdout(dataset.rows.len(), params.seed, params.holdout_fraction);

    let params_hash = config_hash(params).map_err(CorpusError::Io)?;

    let spec = induction_spec(params);
    let withheld: Vec<String> = if !spec.include_supplied {
        bundle
            .supplied_features
            .as_ref()
            .map(|extractor| extractor.definitions().iter().map(|d| d.name.clone()).collect())
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    let similarity_targets: Vec<FeatureDef> = bundle
        .supplied_features
        .as_ref()
        .map(|extractor| extractor.definitions())
        .unwrap_or_default();
    let extracted: Vec<crate::core::features::FeatureVector> = bundle
        .supplied_features
        .as_ref()
        .map(|extractor| dataset.rows.iter().map(|r| extractor.extract(&r.canonical_state)).collect())
        .unwrap_or_default();

    let base_columns = dataset.manifest.columns.len();
    let base_classes = classes.len();

    let mut current = Dataset {
        manifest: dataset.manifest.clone(),
        rows: dataset.rows.clone(),
    };
    let mut promoted_defs: Vec<FeatureDef> = Vec::new();
    let mut promoted: Vec<PromotedConcept> = Vec::new();
    let mut rounds: Vec<RoundReport> = Vec::new();

    for round in 1..=params.rounds {
        let columns = current.manifest.columns.len();
        let pass = enumerate_and_score(&current, &train, &holdout, &value_target, &label_target, params);

        if pass.shortlist.is_empty() {
            rounds.push(RoundReport {
                round,
                columns,
                enumerated: pass.enumerated,
                deduplicated: pass.deduplicated,
                scored: pass.scored,
                passed: pass.passed,
                shortlisted: Vec::new(),
                promoted: Vec::new(),
            });
            break;
        }

        let mut probe_featurizer = bundle.featurizer_with(spec)?;
        for def in &promoted_defs {
            probe_featurizer
                .push_derived(def.clone())
                .map_err(|e| CorpusError::Config(format!("pushing concept `{}`: {e}", def.name)))?;
        }
        let all_idx: Vec<usize> = (0..current.rows.len()).collect();
        let baseline = mine_one(
            &current,
            &all_idx,
            &[],
            MINE_ENGINE_CART,
            params.probe_depth,
            1,
            &probe_featurizer,
        )
        .map_err(|GeneratorError::Failed(m)| CorpusError::Config(format!("induction probe: {m}")))?;
        let mut before_rules = baseline.rules;
        let mut before_soundness = baseline.train_soundness;

        let mut shortlisted: Vec<ShortlistEntry> = Vec::new();
        let mut promoted_this_round: Vec<String> = Vec::new();
        let mut round_bits: Vec<Vec<bool>> = Vec::new();

        for cand in &pass.shortlist {
            if promoted_this_round.len() >= params.max_promoted {
                break;
            }
            if round_bits.iter().any(|b| b == &cand.bits) {
                shortlisted.push(ShortlistEntry {
                    expr: cand.expr.to_string(),
                    train_gain: cand.train_gain,
                    holdout_gain: cand.holdout_gain,
                    outcome: "duplicate".to_string(),
                });
                continue;
            }

            let name = format!("concept_{}", promoted.len() + 1);
            let description = format!("invented concept (round {round})");
            let def = FeatureDef::derived(&name, Tier::Invented, &description, cand.expr.clone());

            let mut trial = Dataset {
                manifest: current.manifest.clone(),
                rows: current.rows.clone(),
            };
            trial.manifest.columns.push(DatasetColumn {
                name: name.clone(),
                tier: Tier::Invented,
                kind: "bool".to_string(),
                description: description.clone(),
            });
            if !trial.manifest.tiers.iter().any(|t| t == "invented") {
                trial.manifest.tiers.push("invented".to_string());
            }
            for (row, &b) in trial.rows.iter_mut().zip(cand.bits.iter()) {
                row.values.push(if b { 1.0 } else { 0.0 });
            }

            let mut trial_featurizer = bundle.featurizer_with(spec)?;
            for pdef in &promoted_defs {
                trial_featurizer
                    .push_derived(pdef.clone())
                    .map_err(|e| CorpusError::Config(format!("pushing concept `{}`: {e}", pdef.name)))?;
            }
            trial_featurizer
                .push_derived(def.clone())
                .map_err(|e| CorpusError::Config(format!("pushing concept `{}`: {e}", def.name)))?;

            let trial_all_idx: Vec<usize> = (0..trial.rows.len()).collect();
            let after = mine_one(
                &trial,
                &trial_all_idx,
                &[],
                MINE_ENGINE_CART,
                params.probe_depth,
                1,
                &trial_featurizer,
            )
            .map_err(|GeneratorError::Failed(m)| CorpusError::Config(format!("induction probe: {m}")))?;

            let references_concept = after.heuristic.references().contains(&name);
            let accepted = promotion_accepts(
                references_concept,
                before_rules,
                after.rules,
                before_soundness,
                after.train_soundness,
                params.soundness_tolerance,
                params.min_soundness_gain,
            );

            if accepted {
                let mut similarity: Vec<SimilarityEntry> = similarity_targets
                    .iter()
                    .map(|target_def| {
                        let agree = extracted
                            .iter()
                            .zip(cand.bits.iter())
                            .filter(|(fv, b)| fv.get(&target_def.name).map(boolify).unwrap_or(false) == **b)
                            .count();
                        SimilarityEntry {
                            feature: target_def.name.clone(),
                            agreement: agree as f64 / extracted.len().max(1) as f64,
                        }
                    })
                    .collect();
                similarity.sort_by(|a, b| b.agreement.total_cmp(&a.agreement).then(a.feature.cmp(&b.feature)));

                promoted.push(PromotedConcept {
                    name: name.clone(),
                    definition: def.to_string(),
                    def: def.clone(),
                    kind: "bool".to_string(),
                    round,
                    scores: ConceptScores {
                        train_gain: cand.train_gain,
                        holdout_gain: cand.holdout_gain,
                        probe_rules_before: before_rules,
                        probe_rules_after: after.rules,
                        probe_soundness_before: before_soundness,
                        probe_soundness_after: after.train_soundness,
                    },
                    provenance: ConceptProvenance {
                        run_id: dataset.manifest.run_id.clone(),
                        params_hash: params_hash.clone(),
                        seed: params.seed,
                        round,
                    },
                    similarity,
                });

                current = trial;
                promoted_defs.push(def);
                before_rules = after.rules;
                before_soundness = after.train_soundness;
                round_bits.push(cand.bits.clone());
                promoted_this_round.push(name.clone());

                shortlisted.push(ShortlistEntry {
                    expr: cand.expr.to_string(),
                    train_gain: cand.train_gain,
                    holdout_gain: cand.holdout_gain,
                    outcome: format!("promoted:{name}"),
                });
            } else {
                shortlisted.push(ShortlistEntry {
                    expr: cand.expr.to_string(),
                    train_gain: cand.train_gain,
                    holdout_gain: cand.holdout_gain,
                    outcome: "rejected-downstream".to_string(),
                });
            }
        }

        rounds.push(RoundReport {
            round,
            columns,
            enumerated: pass.enumerated,
            deduplicated: pass.deduplicated,
            scored: pass.scored,
            passed: pass.passed,
            shortlisted,
            promoted: promoted_this_round.clone(),
        });

        if promoted_this_round.is_empty() {
            break;
        }
    }

    Ok(ConceptReport {
        schema_version: SCHEMA_VERSION,
        game: dataset.manifest.game.clone(),
        run_id: dataset.manifest.run_id.clone(),
        params: params.clone(),
        withheld,
        rows: dataset.rows.len(),
        train_rows: train.len(),
        holdout_rows: holdout.len(),
        base_columns,
        base_classes,
        rounds,
        promoted,
        labels: std::collections::BTreeMap::new(),
    })
}

/// The `concepts` analyzer: runs v1 concept induction over the feature dataset, applies an
/// optional label map, and writes `concepts.json`.
pub struct ConceptsAnalyzer;

impl<G: EngineGame + CorpusGame> Analyzer<G> for ConceptsAnalyzer
where
    G::State: Clone + Eq + std::hash::Hash,
{
    fn name(&self) -> &str {
        "concepts"
    }
    fn description(&self) -> &str {
        "induces and promotes tier-3 concepts from the feature dataset"
    }
    fn requires_annotations(&self) -> bool {
        true
    }
    fn run(&self, ctx: &AnalyzeContext<'_, G>, options: &AnalyzeOptions) -> Result<AnalyzerOutput, CorpusError> {
        options.induction.validate()?;
        if !options.induction.enabled {
            return Err(CorpusError::Config(
                "analyzer `concepts` requires [induction] enabled = true (or --induce)".to_string(),
            ));
        }
        let spec = induction_spec(&options.induction);
        let dataset = dataset_from_context(ctx, spec, &[])?;
        let mut report = induce_concepts(ctx.bundle, &dataset, &options.induction)?;

        if let Some(path) = &options.induction.label_map {
            let text = std::fs::read_to_string(path)
                .map_err(|e| CorpusError::Config(format!("[induction] label_map {}: {e}", path.display())))?;
            let labels: std::collections::BTreeMap<String, String> =
                toml::from_str(&text).map_err(|e| CorpusError::Config(format!("[induction] label_map {}: {e}", path.display())))?;
            report.labels = labels;
        }

        AnalyzerOutput::new(CONCEPTS_FILE, &report, true)
    }
    fn render(&self, output: &serde_json::Value) -> Result<String, CorpusError> {
        let report: ConceptReport = serde_json::from_value(output.clone())
            .map_err(|e| CorpusError::Io(IoError::Invalid(format!("{CONCEPTS_FILE}: {e}"))))?;
        Ok(render_concepts(&report))
    }
}

/// Renders `report` into the deterministic Markdown produced by [`ConceptsAnalyzer::render`]: a
/// `## Concepts` overview table, `### Promoted`, `### Similarity`, and `### Rounds` blocks, the
/// whole string ending in a single `\n`.
fn render_concepts(report: &ConceptReport) -> String {
    let mut blocks: Vec<String> = Vec::new();

    blocks.push("## Concepts".to_string());
    let enumerated: usize = report.rounds.iter().map(|r| r.enumerated).sum();
    let scored: usize = report.rounds.iter().map(|r| r.scored).sum();
    blocks.push(
        [
            "| metric | value |".to_string(),
            "| --- | --- |".to_string(),
            format!("| rounds | {} |", report.rounds.len()),
            format!("| enumerated | {enumerated} |"),
            format!("| scored | {scored} |"),
            format!("| promoted | {} |", report.promoted.len()),
            format!("| withheld | {} |", report.withheld.len()),
        ]
        .join("\n"),
    );

    let mut promoted_items: Vec<String> = Vec::new();
    for p in &report.promoted {
        let label_suffix = report
            .labels
            .get(&p.name)
            .map(|label| format!(" (label: {label})"))
            .unwrap_or_default();
        promoted_items.push(format!("- `{}`{label_suffix}", p.definition));
    }
    blocks.push(render_heading_block("### Promoted", &promoted_items));

    let mut similarity_lines = vec![
        "| concept | feature | agreement |".to_string(),
        "| --- | --- | --- |".to_string(),
    ];
    for p in &report.promoted {
        for s in p.similarity.iter().take(3) {
            similarity_lines.push(format!("| {} | {} | {:.3} |", p.name, s.feature, s.agreement));
        }
    }
    blocks.push(render_heading_block("### Similarity", &similarity_lines));

    let mut rounds_lines = vec![
        "| round | columns | enumerated | deduplicated | scored | passed | shortlisted | promoted |".to_string(),
        "| --- | --- | --- | --- | --- | --- | --- | --- |".to_string(),
    ];
    for r in &report.rounds {
        rounds_lines.push(format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            r.round,
            r.columns,
            r.enumerated,
            r.deduplicated,
            r.scored,
            r.passed,
            r.shortlisted.len(),
            r.promoted.len()
        ));
    }
    blocks.push(render_heading_block("### Rounds", &rounds_lines));

    format!("{}\n", blocks.join("\n\n"))
}

/// Renders a `### heading` followed by a blank line and, if `body` is non-empty, the joined
/// `body` lines. Ensures every heading is followed by a blank line, even with an empty body.
fn render_heading_block(heading: &str, body: &[String]) -> String {
    if body.is_empty() {
        heading.to_string()
    } else {
        format!("{heading}\n\n{}", body.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::tictactoe::{Board, TicTacToe, TicTacToeRules};
    use crate::io::{DatasetManifest, DatasetRow};
    use std::collections::BTreeMap;

    fn ttt_state() -> Board {
        <TicTacToeRules as crate::core::traits::GameRules<TicTacToe>>::initial_state(&TicTacToeRules)
    }

    #[test]
    fn enumeration_order_bounds_and_shortlist() {
        let state = ttt_state();
        let columns = vec![
            DatasetColumn {
                name: "lines.mine2.theirs0".to_string(),
                tier: Tier::Primitive,
                kind: "int".to_string(),
                description: "d".to_string(),
            },
            DatasetColumn {
                name: "cells.mine2.theirs0.ge1".to_string(),
                tier: Tier::Primitive,
                kind: "set".to_string(),
                description: "d".to_string(),
            },
        ];
        let classes = vec!["cells.mine2.theirs0.ge1".to_string()];
        let int_vals = [0.0, 1.0, 2.0, 3.0];
        let set_vals = [1.0, 1.0, 2.0, 2.0];
        let labels = ["none", &classes[0], &classes[0], &classes[0]];
        let rows: Vec<DatasetRow<Board>> = (0..4)
            .map(|i| DatasetRow {
                schema_version: SCHEMA_VERSION,
                canonical_state: state,
                side_to_move: 0,
                value: 0,
                occurrences: 1,
                legal: vec![0],
                optimal: vec![0],
                qualifying: if labels[i] == "none" {
                    vec![]
                } else {
                    vec![classes[0].clone()]
                },
                label: labels[i].to_string(),
                values: vec![int_vals[i], set_vals[i]],
            })
            .collect();
        let manifest = DatasetManifest {
            schema_version: SCHEMA_VERSION,
            game: "tictactoe".to_string(),
            run_id: None,
            annotations_mode: "exhaustive".to_string(),
            tiers: vec!["primitive".to_string()],
            columns,
            classes,
            rows: 4,
            annotated: 4,
            nonterminal: 4,
            collapsed: 0,
            label_counts: BTreeMap::new(),
            value_counts: BTreeMap::new(),
            side_to_move_counts: BTreeMap::new(),
            unlabeled: 0,
        };
        let dataset: Dataset<TicTacToe> = Dataset { manifest, rows };

        let params = InductionParams {
            enabled: true,
            rounds: 1,
            max_thresholds: 2,
            beam: 1,
            top_k: 3,
            min_train_gain: 0.0,
            min_holdout_gain: 0.0,
            holdout_fraction: 0.0,
            ..Default::default()
        };

        let train = vec![0, 1, 2, 3];
        let holdout: Vec<usize> = vec![];
        let value_target = vec![1, 1, 1, 1];
        let label_target = vec![1, 0, 0, 0];

        let pass = enumerate_and_score(&dataset, &train, &holdout, &value_target, &label_target, &params);

        assert_eq!(pass.enumerated, 7);
        assert_eq!(pass.deduplicated, 2);
        assert_eq!(pass.scored, 5);
        assert_eq!(pass.passed, 5);

        let exprs: Vec<String> = pass.shortlist.iter().map(|c| c.expr.to_string()).collect();
        assert_eq!(
            exprs,
            vec![
                "(lines.mine2.theirs0 == 0)".to_string(),
                "(lines.mine2.theirs0 >= 1)".to_string(),
                "(count(cells.mine2.theirs0.ge1) == 1)".to_string(),
            ]
        );
        assert!(pass.shortlist.iter().all(|c| c.holdout_gain.is_none()));
    }

    #[test]
    fn dedupe_drops_constants_and_duplicates() {
        let state = ttt_state();
        let columns = vec![
            DatasetColumn {
                name: "b.const".to_string(),
                tier: Tier::Primitive,
                kind: "bool".to_string(),
                description: "d".to_string(),
            },
            DatasetColumn {
                name: "b.var".to_string(),
                tier: Tier::Primitive,
                kind: "bool".to_string(),
                description: "d".to_string(),
            },
            DatasetColumn {
                name: "i.dup".to_string(),
                tier: Tier::Primitive,
                kind: "int".to_string(),
                description: "d".to_string(),
            },
        ];
        let const_vals = [1.0, 1.0, 1.0, 1.0];
        let var_vals = [1.0, 0.0, 1.0, 0.0];
        let dup_vals = [1.0, 0.0, 1.0, 0.0];
        let rows: Vec<DatasetRow<Board>> = (0..4)
            .map(|i| DatasetRow {
                schema_version: SCHEMA_VERSION,
                canonical_state: state,
                side_to_move: 0,
                value: 0,
                occurrences: 1,
                legal: vec![0],
                optimal: vec![0],
                qualifying: vec![],
                label: "none".to_string(),
                values: vec![const_vals[i], var_vals[i], dup_vals[i]],
            })
            .collect();
        let manifest = DatasetManifest {
            schema_version: SCHEMA_VERSION,
            game: "tictactoe".to_string(),
            run_id: None,
            annotations_mode: "exhaustive".to_string(),
            tiers: vec!["primitive".to_string()],
            columns,
            classes: vec![],
            rows: 4,
            annotated: 4,
            nonterminal: 4,
            collapsed: 0,
            label_counts: BTreeMap::new(),
            value_counts: BTreeMap::new(),
            side_to_move_counts: BTreeMap::new(),
            unlabeled: 0,
        };
        let dataset: Dataset<TicTacToe> = Dataset { manifest, rows };

        let params = InductionParams {
            max_thresholds: 4,
            beam: 1,
            top_k: 8,
            min_train_gain: 0.0,
            min_holdout_gain: 0.0,
            holdout_fraction: 0.0,
            ..Default::default()
        };

        let train = vec![0, 1, 2, 3];
        let holdout: Vec<usize> = vec![];
        let value_target = vec![1, 1, 1, 1];
        let label_target = vec![0, 1, 0, 1];

        let pass = enumerate_and_score(&dataset, &train, &holdout, &value_target, &label_target, &params);

        assert_eq!(pass.enumerated, 6);
        assert_eq!(pass.deduplicated, 4);
        assert_eq!(pass.scored, 2);
        assert_eq!(pass.passed, 2);

        let exprs: Vec<String> = pass.shortlist.iter().map(|c| c.expr.to_string()).collect();
        assert_eq!(exprs, vec!["b.var".to_string(), "(i.dup == 0)".to_string()]);
    }

    #[test]
    fn information_gain_matches_hand_computed() {
        let indices = vec![0, 1, 2, 3];

        let bits = vec![true, true, false, false];
        let target = vec![0, 0, 1, 1];
        assert_eq!(info_gain(&bits, &target, &indices), 1.0);

        let bits = vec![true, false, true, false];
        let target = vec![0, 0, 1, 1];
        assert_eq!(info_gain(&bits, &target, &indices), 0.0);

        let bits = vec![true, true, true, false];
        let target = vec![0, 0, 1, 1];
        let ig = info_gain(&bits, &target, &indices);
        fn h(p: f64) -> f64 {
            -p * p.log2() - (1.0 - p) * (1.0 - p).log2()
        }
        let expected = h(0.5) - 0.75 * h(1.0 / 3.0);
        assert!((ig - expected).abs() < 1e-12);
    }

    #[test]
    fn rejects_candidate_without_downstream_improvement() {
        let tol = 0.005;
        let min_gain = 0.001;

        assert!(!promotion_accepts(false, 5, 3, 0.9, 0.9, tol, min_gain));
        assert!(!promotion_accepts(true, 5, 5, 0.9, 0.9, tol, min_gain));
        assert!(promotion_accepts(true, 5, 4, 0.9, 0.897, tol, min_gain));
        assert!(!promotion_accepts(true, 5, 4, 0.9, 0.894, tol, min_gain));
        assert!(promotion_accepts(true, 5, 5, 0.9, 0.902, tol, min_gain));
        assert!(!promotion_accepts(true, 5, 6, 0.9, 0.99, tol, min_gain));
    }

    fn promotion_dataset() -> Dataset<TicTacToe> {
        let state = ttt_state();
        let columns = vec![
            DatasetColumn {
                name: "lines.mine2.theirs0".to_string(),
                tier: Tier::Primitive,
                kind: "int".to_string(),
                description: "d".to_string(),
            },
            DatasetColumn {
                name: "lines.mine0.theirs2".to_string(),
                tier: Tier::Primitive,
                kind: "int".to_string(),
                description: "d".to_string(),
            },
            DatasetColumn {
                name: "cells.mine2.theirs0.ge1".to_string(),
                tier: Tier::Primitive,
                kind: "set".to_string(),
                description: "d".to_string(),
            },
        ];
        let classes = vec!["cells.mine2.theirs0.ge1".to_string()];
        let pairs = [(0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)];
        let mut rows: Vec<DatasetRow<Board>> = Vec::new();
        for (a, b) in pairs {
            for _ in 0..3 {
                let none = a == 0.0 && b == 0.0;
                let (label, qualifying) = if none {
                    ("none".to_string(), Vec::new())
                } else {
                    (classes[0].clone(), vec![classes[0].clone()])
                };
                rows.push(DatasetRow {
                    schema_version: SCHEMA_VERSION,
                    canonical_state: state,
                    side_to_move: 0,
                    value: 0,
                    occurrences: 1,
                    legal: vec![0],
                    optimal: vec![0],
                    qualifying,
                    label,
                    values: vec![a, b, 1.0],
                });
            }
        }
        let manifest = DatasetManifest {
            schema_version: SCHEMA_VERSION,
            game: "tictactoe".to_string(),
            run_id: None,
            annotations_mode: "exhaustive".to_string(),
            tiers: vec!["primitive".to_string()],
            columns,
            classes,
            rows: rows.len(),
            annotated: rows.len(),
            nonterminal: rows.len(),
            collapsed: 0,
            label_counts: BTreeMap::new(),
            value_counts: BTreeMap::new(),
            side_to_move_counts: BTreeMap::new(),
            unlabeled: 0,
        };
        Dataset { manifest, rows }
    }

    fn promotion_params() -> InductionParams {
        InductionParams {
            enabled: true,
            withhold_tier2: true,
            rounds: 1,
            beam: 8,
            top_k: 4,
            max_promoted: 2,
            min_train_gain: 0.001,
            holdout_fraction: 0.0,
            ..Default::default()
        }
    }

    fn holdout_dataset() -> Dataset<TicTacToe> {
        let state = ttt_state();
        let columns = vec![
            DatasetColumn {
                name: "lines.mine1.theirs1".to_string(),
                tier: Tier::Primitive,
                kind: "int".to_string(),
                description: "d".to_string(),
            },
            DatasetColumn {
                name: "cells.mine2.theirs0.ge1".to_string(),
                tier: Tier::Primitive,
                kind: "set".to_string(),
                description: "d".to_string(),
            },
        ];
        let classes = vec!["cells.mine2.theirs0.ge1".to_string()];
        let (train, holdout) = split_train_holdout(16, 7, 0.25);

        let mut label = vec!["none".to_string(); 16];
        let mut qualifying: Vec<Vec<String>> = vec![Vec::new(); 16];
        let mut h = [0.0f64; 16];

        for (k, &idx) in train.iter().enumerate() {
            if k < 6 {
                label[idx] = classes[0].clone();
                qualifying[idx] = vec![classes[0].clone()];
                h[idx] = 1.0;
            }
        }

        let holdout_labels = [classes[0].clone(), classes[0].clone(), "none".to_string(), "none".to_string()];
        let holdout_h = [1.0, 0.0, 1.0, 0.0];
        for (k, &idx) in holdout.iter().enumerate() {
            label[idx] = holdout_labels[k].clone();
            qualifying[idx] = if holdout_labels[k] == "none" {
                Vec::new()
            } else {
                vec![classes[0].clone()]
            };
            h[idx] = holdout_h[k];
        }

        let rows: Vec<DatasetRow<Board>> = (0..16)
            .map(|i| DatasetRow {
                schema_version: SCHEMA_VERSION,
                canonical_state: state,
                side_to_move: 0,
                value: 0,
                occurrences: 1,
                legal: vec![0],
                optimal: vec![0],
                qualifying: qualifying[i].clone(),
                label: label[i].clone(),
                values: vec![h[i], 1.0],
            })
            .collect();

        let manifest = DatasetManifest {
            schema_version: SCHEMA_VERSION,
            game: "tictactoe".to_string(),
            run_id: None,
            annotations_mode: "exhaustive".to_string(),
            tiers: vec!["primitive".to_string()],
            columns,
            classes,
            rows: rows.len(),
            annotated: rows.len(),
            nonterminal: rows.len(),
            collapsed: 0,
            label_counts: BTreeMap::new(),
            value_counts: BTreeMap::new(),
            side_to_move_counts: BTreeMap::new(),
            unlabeled: 0,
        };
        Dataset { manifest, rows }
    }

    #[test]
    fn promotes_compressing_candidate() {
        let dataset = promotion_dataset();
        let params = promotion_params();
        let report = induce_concepts(&crate::games::tictactoe::game_bundle(), &dataset, &params).unwrap();

        assert_eq!(report.promoted.len(), 1);
        let p = &report.promoted[0];
        assert_eq!(p.name, "concept_1");
        assert!(p.definition.contains(" [tier-3] = "));
        assert!(p.definition.contains("(lines.mine2.theirs0 == 0)"));
        assert_eq!(p.kind, "bool");
        assert!(matches!(p.def.tier, Tier::Invented));
        assert_eq!(p.scores.probe_rules_before, 2);
        assert_eq!(p.scores.probe_rules_after, 1);
        assert_eq!(p.provenance.params_hash.len(), 16);
        assert_eq!(p.provenance.round, 1);
        assert!(!p.similarity.is_empty());
        assert!(p.similarity.iter().all(|s| (0.0..=1.0).contains(&s.agreement)));

        assert_eq!(report.rounds.len(), 1);
        assert_eq!(report.rounds[0].promoted, vec!["concept_1".to_string()]);
        assert_eq!(report.rounds[0].shortlisted.len(), 4);
        assert_eq!(report.rounds[0].shortlisted[0].outcome, "promoted:concept_1");
        for entry in &report.rounds[0].shortlisted[1..] {
            assert_eq!(entry.outcome, "rejected-downstream");
        }
        assert_eq!(report.base_columns, 3);
        assert_eq!(report.base_classes, 1);
        assert_eq!(report.train_rows, 12);
        assert_eq!(report.holdout_rows, 0);
        assert!(report.labels.is_empty());
    }

    #[test]
    fn rejects_candidate_without_holdout_gain() {
        let dataset = holdout_dataset();
        let params = InductionParams {
            enabled: true,
            withhold_tier2: true,
            rounds: 1,
            beam: 8,
            top_k: 4,
            max_promoted: 2,
            seed: 7,
            holdout_fraction: 0.25,
            ..Default::default()
        };
        let bundle = crate::games::tictactoe::game_bundle();
        let report = induce_concepts(&bundle, &dataset, &params).unwrap();
        assert_eq!(report.rounds[0].scored, 2);
        assert_eq!(report.rounds[0].passed, 0);
        assert!(report.rounds[0].shortlisted.is_empty());
        assert!(report.promoted.is_empty());
        assert_eq!(report.train_rows, 12);
        assert_eq!(report.holdout_rows, 4);

        let params2 = InductionParams {
            holdout_fraction: 0.0,
            ..params
        };
        let report2 = induce_concepts(&bundle, &dataset, &params2).unwrap();
        assert_eq!(report2.rounds[0].passed, 2);
    }

    #[test]
    fn concept_report_json_is_identical_across_runs() {
        let dataset = promotion_dataset();
        let params = promotion_params();
        let bundle = crate::games::tictactoe::game_bundle();
        let r1 = induce_concepts(&bundle, &dataset, &params).unwrap();
        let r2 = induce_concepts(&bundle, &dataset, &params).unwrap();
        assert_eq!(serde_json::to_string(&r1).unwrap(), serde_json::to_string(&r2).unwrap());
    }

    #[test]
    fn concepts_render_lists_promoted_with_labels() {
        let dataset = promotion_dataset();
        let params = promotion_params();
        let bundle = crate::games::tictactoe::game_bundle();
        let mut report = induce_concepts(&bundle, &dataset, &params).unwrap();
        report.labels = BTreeMap::from([("concept_1".to_string(), "first invented concept".to_string())]);
        let rendered =
            <ConceptsAnalyzer as Analyzer<TicTacToe>>::render(&ConceptsAnalyzer, &serde_json::to_value(&report).unwrap()).unwrap();

        assert!(rendered.starts_with("## Concepts\n\n"));
        assert!(rendered.contains("\n### Promoted\n\n- `"));
        assert!(rendered.contains("\n### Similarity\n\n| concept |"));
        assert!(rendered.contains("\n### Rounds\n\n| round |"));
        assert!(rendered.contains("` (label: first invented concept)"));
        assert!(rendered.contains("| rounds | 1 |"));
        assert!(rendered.contains("| promoted | 1 |"));
        assert!(rendered.lines().any(|l| l.starts_with("| concept_1 | ")));
        assert!(rendered.ends_with('\n') && !rendered.ends_with("\n\n"));
        assert!(!rendered.contains("|---"));

        let lines: Vec<&str> = rendered.split('\n').collect();
        assert!(
            !lines.windows(2).any(|w| w[0].starts_with('#') && !w[1].is_empty()),
            "every heading must be followed by a blank line"
        );
    }

    #[test]
    fn concepts_analyzer_requires_induction_enabled() {
        let bundle = crate::games::tictactoe::game_bundle();
        let dir = tempfile::tempdir().unwrap();
        let games: Vec<crate::discovery::analyze::GameRec<TicTacToe>> = Vec::new();
        let positions: Vec<crate::discovery::analyze::PosRec<TicTacToe>> = Vec::new();
        let ctx = crate::discovery::analyze::AnalyzeContext::new(&bundle, dir.path(), "run".to_string(), &games, &positions);
        let options = AnalyzeOptions::default();

        let result = <ConceptsAnalyzer as Analyzer<TicTacToe>>::run(&ConceptsAnalyzer, &ctx, &options);

        match result {
            Err(e) => assert!(e.to_string().contains("requires [induction] enabled = true (or --induce)")),
            Ok(_) => panic!("expected Err"),
        }
    }
}
