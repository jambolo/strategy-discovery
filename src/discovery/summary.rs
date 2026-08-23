//! Summary aggregator: outcome distributions and corpus-diversity metrics over a run
//! directory, plus a corpus consistency verifier.
//!
//! [`analyze_corpus`] reads a generated corpus run directory (`run.json`, `games.jsonl`,
//! `positions.jsonl`) and writes `summary.json`: outcome distributions by pairing, game
//! length, ply and first-action symmetry orbit, plus diversity metrics that guard against a
//! corpus of low-variety games. [`SummaryAnalyzer`] is the [`crate::discovery::analyze::Analyzer`]
//! wrapper around [`summarize`] that the analyzer registry runs; [`analyze_corpus`] is now a thin
//! wrapper over the registry ([`crate::discovery::analyze::analyze_outputs`]) kept for direct
//! callers that only want `summary.json`. [`verify_corpus`] independently reconstructs every game
//! from its action list via [`crate::io::replay::replay`] and checks it against the persisted
//! positions.
//!
//! Repeated analysis of the same corpus must produce a byte-identical `summary.json`: every
//! map in [`CorpusSummary`] is a `BTreeMap` and no field carries timing, threading, or
//! machine-specific data.

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use crate::discovery::analyze::{
    AnalyzeContext, AnalyzeOptions, Analyzer, AnalyzerOutput, GameRec, PosRec, analyze_outputs, builtin_registry, load_corpus,
};
use crate::discovery::bundle::GameBundle;
use crate::discovery::config::CorpusError;
use crate::io::{CorpusGame, IoError, MineParams, SCHEMA_VERSION, SUMMARY_FILE, final_state, replay};
use crate::strategy::engine::EngineGame;

/// Outcome tally over a set of games.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Default)]
pub struct OutcomeCounts {
    /// Wins keyed by `format!("{winner:?}")`.
    pub wins: BTreeMap<String, usize>,
    /// Games that ended drawn.
    pub draws: usize,
    /// Games with no outcome (aborted by `max_plies`).
    pub unfinished: usize,
    /// Total games tallied.
    pub total: usize,
}

/// Minimum diversity a corpus must reach to be usable for mining.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct DiversityThresholds {
    /// Minimum fraction of known canonical positions the corpus must cover.
    pub min_canonical_coverage: f64,
    /// Minimum fraction of games that must end decisively (a win, not a draw).
    pub min_decisive_fraction: f64,
    /// Minimum fraction of games that must have a distinct action sequence.
    pub min_distinct_game_fraction: f64,
}

impl Default for DiversityThresholds {
    fn default() -> Self {
        DiversityThresholds {
            min_canonical_coverage: 0.5,
            min_decisive_fraction: 0.2,
            min_distinct_game_fraction: 0.5,
        }
    }
}

/// Aggregated view of one corpus run, written to `summary.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct CorpusSummary {
    /// Schema version this summary was written under.
    pub schema_version: u32,
    /// Identifier of the run this summary was computed from.
    pub run_id: Option<String>,
    /// Game name.
    pub game: String,
    /// Number of games tallied.
    pub games: usize,
    /// Number of positions tallied.
    pub positions: usize,
    /// Number of distinct game action sequences.
    pub distinct_games: usize,
    /// `distinct_games / games`.
    pub distinct_game_fraction: f64,
    /// Number of distinct raw states seen (positions and game final states).
    pub distinct_positions: usize,
    /// Number of distinct canonical states seen (positions and game final states).
    pub distinct_canonical_positions: usize,
    /// Known canonical position count for this game, if known.
    pub known_canonical_positions: Option<usize>,
    /// `distinct_canonical_positions / known_canonical_positions`, if known.
    pub canonical_coverage: Option<f64>,
    /// Number of games that ended in a win (not a draw or unfinished).
    pub decisive_games: usize,
    /// `decisive_games / games`.
    pub decisive_fraction: f64,
    /// Outcome tally over every game.
    pub outcomes: OutcomeCounts,
    /// Outcome tally keyed by `cell.strategies.join(" vs ")`.
    pub by_pairing: BTreeMap<String, OutcomeCounts>,
    /// Number of games keyed by game length in plies.
    pub by_length: BTreeMap<usize, usize>,
    /// Outcome tally keyed by ply index, over every position.
    pub by_ply: BTreeMap<usize, OutcomeCounts>,
    /// Outcome tally keyed by the symmetry orbit of each game's first action; empty when the
    /// game has no primitives.
    pub by_first_action_orbit: BTreeMap<usize, OutcomeCounts>,
    /// Thresholds this summary was checked against.
    pub thresholds: DiversityThresholds,
    /// Whether every diversity threshold was met.
    pub diversity_pass: bool,
    /// Human-readable description of every threshold that was missed, in a fixed order.
    pub diversity_failures: Vec<String>,
}

/// `numerator as f64 / denominator as f64`, exactly `0.0` when `denominator` is `0`.
fn frac(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

/// Tallies one game's or position's outcome into `counts`.
fn tally<G: EngineGame>(counts: &mut OutcomeCounts, outcome: Option<&G::Outcome>) {
    counts.total += 1;
    match outcome {
        None => counts.unfinished += 1,
        Some(o) => match G::winner(o) {
            Some(p) => *counts.wins.entry(format!("{p:?}")).or_insert(0) += 1,
            None => counts.draws += 1,
        },
    }
}

/// Aggregates `games` and `positions` into a [`CorpusSummary`], checking `thresholds`.
pub fn summarize<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    games: &[GameRec<G>],
    positions: &[PosRec<G>],
    thresholds: &DiversityThresholds,
) -> CorpusSummary {
    let run_id = games.first().map(|g| g.run_id.clone());

    let mut outcomes = OutcomeCounts::default();
    let mut by_pairing: BTreeMap<String, OutcomeCounts> = BTreeMap::new();
    let mut by_length: BTreeMap<usize, usize> = BTreeMap::new();
    let mut decisive_games = 0usize;

    let mut distinct_action_seqs: HashSet<&Vec<G::Action>> = HashSet::new();
    let mut distinct_states: HashSet<&G::State> = HashSet::new();
    let mut distinct_canonical: HashSet<&G::State> = HashSet::new();

    for game in games {
        tally::<G>(&mut outcomes, game.outcome.as_ref());
        if let Some(o) = &game.outcome
            && G::winner(o).is_some()
        {
            decisive_games += 1;
        }

        let pairing_key = game.cell.strategies.join(" vs ");
        tally::<G>(by_pairing.entry(pairing_key).or_default(), game.outcome.as_ref());

        *by_length.entry(game.length).or_insert(0) += 1;

        distinct_action_seqs.insert(&game.actions);
        distinct_states.insert(&game.final_state);
        distinct_canonical.insert(&game.final_canonical_state);
    }

    let mut by_ply: BTreeMap<usize, OutcomeCounts> = BTreeMap::new();
    for position in positions {
        distinct_states.insert(&position.state);
        distinct_canonical.insert(&position.canonical_state);
        tally::<G>(by_ply.entry(position.ply).or_default(), position.outcome.as_ref());
    }

    let mut by_first_action_orbit: BTreeMap<usize, OutcomeCounts> = BTreeMap::new();
    if let Some(primitives) = &bundle.primitives {
        let orbit_index = primitives.symmetry_group().orbit_index();
        for game in games {
            if let Some(first_action) = game.actions.first()
                && let Some(p) = primitives.action_position(first_action)
            {
                tally::<G>(
                    by_first_action_orbit.entry(orbit_index[p]).or_default(),
                    game.outcome.as_ref(),
                );
            }
        }
    }

    let distinct_games = distinct_action_seqs.len();
    let distinct_game_fraction = frac(distinct_games, games.len());
    let decisive_fraction = frac(decisive_games, games.len());
    let known_canonical_positions = bundle.known_canonical_positions;
    let canonical_coverage = known_canonical_positions.map(|k| frac(distinct_canonical.len(), k));

    let mut diversity_failures = Vec::new();
    if let Some(c) = canonical_coverage
        && c < thresholds.min_canonical_coverage
    {
        diversity_failures.push(format!(
            "canonical_coverage {c:.2} < {:.2}",
            thresholds.min_canonical_coverage
        ));
    }
    if decisive_fraction < thresholds.min_decisive_fraction {
        diversity_failures.push(format!(
            "decisive_fraction {decisive_fraction:.2} < {:.2}",
            thresholds.min_decisive_fraction
        ));
    }
    if distinct_game_fraction < thresholds.min_distinct_game_fraction {
        diversity_failures.push(format!(
            "distinct_game_fraction {distinct_game_fraction:.2} < {:.2}",
            thresholds.min_distinct_game_fraction
        ));
    }
    let diversity_pass = diversity_failures.is_empty();

    CorpusSummary {
        schema_version: SCHEMA_VERSION,
        run_id,
        game: bundle.name.clone(),
        games: games.len(),
        positions: positions.len(),
        distinct_games,
        distinct_game_fraction,
        distinct_positions: distinct_states.len(),
        distinct_canonical_positions: distinct_canonical.len(),
        known_canonical_positions,
        canonical_coverage,
        decisive_games,
        decisive_fraction,
        outcomes,
        by_pairing,
        by_length,
        by_ply,
        by_first_action_orbit,
        thresholds: thresholds.clone(),
        diversity_pass,
        diversity_failures,
    }
}

/// The `summary` analyzer: wraps [`summarize`] and writes `summary.json`.
#[derive(Debug, Clone, Copy, Default)]
pub struct SummaryAnalyzer;

impl<G: EngineGame + CorpusGame> Analyzer<G> for SummaryAnalyzer {
    fn name(&self) -> &str {
        "summary"
    }
    fn description(&self) -> &str {
        "outcome distributions and corpus-diversity metrics (summary.json)"
    }
    fn requires_annotations(&self) -> bool {
        false
    }
    fn run(&self, ctx: &AnalyzeContext<'_, G>, options: &AnalyzeOptions) -> Result<AnalyzerOutput, CorpusError> {
        let mut summary = summarize(ctx.bundle, ctx.games, ctx.positions, &options.thresholds);
        summary.run_id = Some(ctx.run_id.clone());
        AnalyzerOutput::new(SUMMARY_FILE, &summary, summary.diversity_pass)
    }
    fn render(&self, output: &serde_json::Value) -> Result<String, CorpusError> {
        let summary: CorpusSummary = serde_json::from_value(output.clone())
            .map_err(|e| CorpusError::Io(IoError::Invalid(format!("{SUMMARY_FILE}: {e}"))))?;
        Ok(render_summary(&summary))
    }
}

/// `value` as a decimal string, or `-` when absent.
fn opt_usize(value: Option<usize>) -> String {
    value.map_or_else(|| "-".to_string(), |v| v.to_string())
}

/// `value` formatted `{:.3}`, or `-` when absent.
fn opt_frac(value: Option<f64>) -> String {
    value.map_or_else(|| "-".to_string(), |v| format!("{v:.3}"))
}

/// `"yes"` or `"no"`.
fn yes_no(pass: bool) -> &'static str {
    if pass { "yes" } else { "no" }
}

/// One [`OutcomeCounts`] rendered as `wins | draws | unfinished | total` table-cell text, `wins`
/// being its `k=v` pairs joined by `, ` (or `-` when empty).
fn outcome_row(counts: &OutcomeCounts) -> String {
    let wins = if counts.wins.is_empty() {
        "-".to_string()
    } else {
        counts
            .wins
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    format!("{wins} | {} | {} | {}", counts.draws, counts.unfinished, counts.total)
}

/// Renders `summary` into the deterministic Markdown produced by [`SummaryAnalyzer::render`]:
/// a `## Summary` overview table followed by `### Diversity`, `### Outcomes`, `### By pairing`,
/// `### By length`, `### By ply` and `### By first-action orbit` sections, each heading and each
/// table set off by blank lines, the whole string ending in a single `\n`.
fn render_summary(summary: &CorpusSummary) -> String {
    let mut blocks: Vec<String> = Vec::new();

    blocks.push("## Summary".to_string());
    blocks.push(
        [
            "| metric | value |".to_string(),
            "| --- | --- |".to_string(),
            format!("| run_id | {} |", summary.run_id.as_deref().unwrap_or("-")),
            format!("| game | {} |", summary.game),
            format!("| games | {} |", summary.games),
            format!("| positions | {} |", summary.positions),
            format!("| distinct_games | {} |", summary.distinct_games),
            format!("| distinct_game_fraction | {:.3} |", summary.distinct_game_fraction),
            format!("| distinct_positions | {} |", summary.distinct_positions),
            format!("| distinct_canonical_positions | {} |", summary.distinct_canonical_positions),
            format!(
                "| known_canonical_positions | {} |",
                opt_usize(summary.known_canonical_positions)
            ),
            format!("| canonical_coverage | {} |", opt_frac(summary.canonical_coverage)),
            format!("| decisive_games | {} |", summary.decisive_games),
            format!("| decisive_fraction | {:.3} |", summary.decisive_fraction),
        ]
        .join("\n"),
    );

    blocks.push("### Diversity".to_string());
    let coverage_row = match summary.canonical_coverage {
        Some(v) => format!(
            "| canonical_coverage | {v:.3} | {:.3} | {} |",
            summary.thresholds.min_canonical_coverage,
            yes_no(v >= summary.thresholds.min_canonical_coverage)
        ),
        None => format!(
            "| canonical_coverage | - | {:.3} | - |",
            summary.thresholds.min_canonical_coverage
        ),
    };
    blocks.push(
        [
            "| metric | value | threshold | pass |".to_string(),
            "| --- | --- | --- | --- |".to_string(),
            coverage_row,
            format!(
                "| decisive_fraction | {:.3} | {:.3} | {} |",
                summary.decisive_fraction,
                summary.thresholds.min_decisive_fraction,
                yes_no(summary.decisive_fraction >= summary.thresholds.min_decisive_fraction)
            ),
            format!(
                "| distinct_game_fraction | {:.3} | {:.3} | {} |",
                summary.distinct_game_fraction,
                summary.thresholds.min_distinct_game_fraction,
                yes_no(summary.distinct_game_fraction >= summary.thresholds.min_distinct_game_fraction)
            ),
        ]
        .join("\n"),
    );
    blocks.push(format!("Diversity pass: {}", yes_no(summary.diversity_pass)));
    if !summary.diversity_failures.is_empty() {
        blocks.push(
            summary
                .diversity_failures
                .iter()
                .map(|f| format!("- {f}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }

    blocks.push("### Outcomes".to_string());
    let mut outcome_rows = vec!["| outcome | games |".to_string(), "| --- | --- |".to_string()];
    for (winner, n) in &summary.outcomes.wins {
        outcome_rows.push(format!("| win {winner} | {n} |"));
    }
    outcome_rows.push(format!("| draw | {} |", summary.outcomes.draws));
    outcome_rows.push(format!("| unfinished | {} |", summary.outcomes.unfinished));
    outcome_rows.push(format!("| total | {} |", summary.outcomes.total));
    blocks.push(outcome_rows.join("\n"));

    blocks.push("### By pairing".to_string());
    let mut pairing_rows = vec![
        "| pairing | wins | draws | unfinished | total |".to_string(),
        "| --- | --- | --- | --- | --- |".to_string(),
    ];
    for (pairing, counts) in &summary.by_pairing {
        pairing_rows.push(format!("| {pairing} | {} |", outcome_row(counts)));
    }
    blocks.push(pairing_rows.join("\n"));

    blocks.push("### By length".to_string());
    let mut length_rows = vec!["| length | games |".to_string(), "| --- | --- |".to_string()];
    for (length, n) in &summary.by_length {
        length_rows.push(format!("| {length} | {n} |"));
    }
    blocks.push(length_rows.join("\n"));

    blocks.push("### By ply".to_string());
    let mut ply_rows = vec![
        "| ply | wins | draws | unfinished | total |".to_string(),
        "| --- | --- | --- | --- | --- |".to_string(),
    ];
    for (ply, counts) in &summary.by_ply {
        ply_rows.push(format!("| {ply} | {} |", outcome_row(counts)));
    }
    blocks.push(ply_rows.join("\n"));

    blocks.push("### By first-action orbit".to_string());
    let mut orbit_rows = vec![
        "| orbit | wins | draws | unfinished | total |".to_string(),
        "| --- | --- | --- | --- | --- |".to_string(),
    ];
    for (orbit, counts) in &summary.by_first_action_orbit {
        orbit_rows.push(format!("| {orbit} | {} |", outcome_row(counts)));
    }
    blocks.push(orbit_rows.join("\n"));

    format!("{}\n", blocks.join("\n\n"))
}

/// Reads a corpus run from `corpus_dir`, summarizes it against `thresholds`, and writes
/// `summary.json` (via the `summary` analyzer) and `analyze.json` alongside the run.
pub fn analyze_corpus<G: EngineGame + CorpusGame>(
    bundle: &GameBundle<G>,
    corpus_dir: &Path,
    thresholds: &DiversityThresholds,
) -> Result<CorpusSummary, CorpusError> {
    let registry = builtin_registry::<G>();
    let options = AnalyzeOptions {
        analyzers: vec!["summary".to_string()],
        thresholds: thresholds.clone(),
        strict: false,
        mine: MineParams::default(),
    };
    let (_, outputs) = analyze_outputs(bundle, corpus_dir, &registry, &options)?;
    let output = outputs
        .into_iter()
        .next()
        .ok_or_else(|| CorpusError::Io(IoError::Invalid("summary analyzer produced no output".to_string())))?;
    serde_json::from_str(&output.json).map_err(|source| {
        CorpusError::Io(IoError::Json {
            path: corpus_dir.join(SUMMARY_FILE),
            line: 0,
            source,
        })
    })
}

/// Builds a mismatch error for one ply of one game.
fn ply_mismatch(game_id: &str, what: &str, ply: usize) -> CorpusError {
    CorpusError::Io(IoError::Invalid(format!("game {game_id}: {what} mismatch at ply {ply}")))
}

/// Builds a mismatch error for one game as a whole.
fn game_mismatch(game_id: &str, what: &str) -> CorpusError {
    CorpusError::Io(IoError::Invalid(format!("game {game_id}: {what} mismatch")))
}

/// Reads a corpus run from `corpus_dir` and checks it for internal consistency: every game's
/// positions replay to exactly the persisted states, actions, legal-action sets, canonical
/// forms and outcome. Returns the number of games verified.
pub fn verify_corpus<G: EngineGame + CorpusGame>(bundle: &GameBundle<G>, corpus_dir: &Path) -> Result<usize, CorpusError> {
    let loaded = load_corpus(bundle, corpus_dir)?;

    let mut grouped: BTreeMap<&str, Vec<&PosRec<G>>> = BTreeMap::new();
    for position in &loaded.positions {
        grouped.entry(position.game_id.as_str()).or_default().push(position);
    }

    for game in &loaded.games {
        let group = grouped
            .get(game.game_id.as_str())
            .ok_or_else(|| game_mismatch(&game.game_id, "missing positions"))?;
        if group.len() != game.actions.len() {
            return Err(game_mismatch(&game.game_id, "position count"));
        }

        let pairs = replay(bundle.rules.as_ref(), &game.actions)?;

        for (i, position) in group.iter().enumerate() {
            let (state_before, action) = &pairs[i];

            if position.ply != i {
                return Err(ply_mismatch(&game.game_id, "ply", i));
            }
            if position.state != *state_before {
                return Err(ply_mismatch(&game.game_id, "state", i));
            }
            if position.chosen_action != *action {
                return Err(ply_mismatch(&game.game_id, "chosen_action", i));
            }
            if position.side_to_move != bundle.rules.player_to_move(state_before) {
                return Err(ply_mismatch(&game.game_id, "side_to_move", i));
            }
            if position.legal_actions != bundle.rules.legal_actions(state_before) {
                return Err(ply_mismatch(&game.game_id, "legal_actions", i));
            }
            let (canonical_state, transform) = bundle.canonicalize(state_before);
            if position.canonical_state != canonical_state || position.canonical_transform != transform {
                return Err(ply_mismatch(&game.game_id, "canonical_state", i));
            }
            if position.game_length != game.actions.len() {
                return Err(ply_mismatch(&game.game_id, "game_length", i));
            }
            if position.outcome != game.outcome {
                return Err(ply_mismatch(&game.game_id, "outcome", i));
            }
        }

        if game.final_state != final_state(bundle.rules.as_ref(), &game.actions)? {
            return Err(game_mismatch(&game.game_id, "final_state"));
        }
        if game.final_canonical_state != bundle.canonicalize(&game.final_state).0 {
            return Err(game_mismatch(&game.game_id, "final_canonical_state"));
        }
        if game.length != game.actions.len() {
            return Err(game_mismatch(&game.game_id, "length"));
        }
    }

    Ok(loaded.games.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::games::tictactoe::{Board, Move, Outcome, Player, TicTacToe, game_bundle};
    use crate::io::schema::ANALYZE_FILE;
    use crate::io::{CellKey, GAMES_FILE, GameRecord, POSITIONS_FILE, PositionRecord, RUN_FILE, write_json_pretty, write_jsonl};

    /// Builds a hand-crafted [`GameRecord`]/[`PositionRecord`] pair for `actions`, using
    /// `replay` and `bundle.canonicalize` the same way [`verify_corpus`] checks them.
    #[allow(clippy::type_complexity)]
    fn make_game(
        bundle: &GameBundle<TicTacToe>,
        game_id: &str,
        strategies: [&str; 2],
        actions: &[Move],
        outcome: Option<Outcome>,
    ) -> (
        GameRecord<Board, Move, Player, Outcome>,
        Vec<PositionRecord<Board, Move, Player, Outcome>>,
    ) {
        let pairs = replay(bundle.rules.as_ref(), actions).unwrap();
        let final_state_value = final_state(bundle.rules.as_ref(), actions).unwrap();
        let final_canonical_state = bundle.canonicalize(&final_state_value).0;

        let positions: Vec<PositionRecord<Board, Move, Player, Outcome>> = pairs
            .iter()
            .enumerate()
            .map(|(ply, (state, action))| {
                let (canonical_state, canonical_transform) = bundle.canonicalize(state);
                PositionRecord {
                    schema_version: SCHEMA_VERSION,
                    game_id: game_id.to_string(),
                    ply,
                    side_to_move: bundle.rules.player_to_move(state),
                    state: *state,
                    canonical_state,
                    canonical_transform,
                    legal_actions: bundle.rules.legal_actions(state),
                    chosen_action: *action,
                    chosen_by: "test".to_string(),
                    outcome,
                    game_length: actions.len(),
                }
            })
            .collect();

        let game = GameRecord {
            schema_version: SCHEMA_VERSION,
            run_id: "r".to_string(),
            game_id: game_id.to_string(),
            cell: CellKey {
                index: 0,
                strategies: strategies.iter().map(|s| s.to_string()).collect(),
                evaluator: "default".to_string(),
                opening: "none".to_string(),
                random_opening_plies: 0,
            },
            game_index: 0,
            seed: 0,
            players: bundle.players.clone(),
            specs: vec![],
            opening_plies: 0,
            actions: actions.to_vec(),
            outcome,
            length: actions.len(),
            final_state: final_state_value,
            final_canonical_state,
        };

        (game, positions)
    }

    #[test]
    fn default_thresholds_are_the_documented_values() {
        assert_eq!(
            DiversityThresholds::default(),
            DiversityThresholds {
                min_canonical_coverage: 0.5,
                min_decisive_fraction: 0.2,
                min_distinct_game_fraction: 0.5,
            }
        );
    }

    #[test]
    fn summarize_tallies_outcomes_and_pairings() {
        let bundle = game_bundle();

        let (win_game, win_positions) = make_game(
            &bundle,
            "0:0",
            ["a", "b"],
            &[Move(0), Move(3), Move(1), Move(4), Move(2)],
            Some(Outcome::Win(Player::X)),
        );
        let (draw_game, draw_positions) = make_game(
            &bundle,
            "0:1",
            ["a", "b"],
            &[
                Move(0),
                Move(1),
                Move(2),
                Move(4),
                Move(3),
                Move(5),
                Move(7),
                Move(6),
                Move(8),
            ],
            Some(Outcome::Draw),
        );

        let games = vec![win_game, draw_game];
        let mut positions = win_positions;
        positions.extend(draw_positions);

        let summary = summarize(&bundle, &games, &positions, &DiversityThresholds::default());

        assert_eq!(summary.games, 2);
        assert_eq!(summary.distinct_games, 2);
        assert_eq!(summary.decisive_games, 1);
        assert!((summary.distinct_game_fraction - 1.0).abs() < 1e-9);
        assert!((summary.decisive_fraction - 0.5).abs() < 1e-9);
        assert_eq!(summary.outcomes.wins["X"], 1);
        assert_eq!(summary.outcomes.draws, 1);
        assert_eq!(summary.outcomes.total, 2);

        let pairing_keys: Vec<&String> = summary.by_pairing.keys().collect();
        assert_eq!(pairing_keys, vec!["a vs b"]);

        assert!(summary.by_length.contains_key(&5));
        assert!(summary.by_length.contains_key(&9));

        assert_eq!(summary.by_first_action_orbit.len(), 1);
        assert_eq!(summary.by_first_action_orbit[&0].total, 2);

        assert_eq!(summary.positions, 14);
    }

    #[test]
    fn diversity_failures_name_every_missed_threshold() {
        let bundle = game_bundle();
        let actions = [
            Move(0),
            Move(1),
            Move(2),
            Move(4),
            Move(3),
            Move(5),
            Move(7),
            Move(6),
            Move(8),
        ];

        let mut games = Vec::new();
        let mut positions = Vec::new();
        for i in 0..3 {
            let (game, game_positions) = make_game(&bundle, &format!("0:{i}"), ["a", "b"], &actions, Some(Outcome::Draw));
            games.push(game);
            positions.extend(game_positions);
        }

        let summary = summarize(&bundle, &games, &positions, &DiversityThresholds::default());

        assert!(!summary.diversity_pass);
        assert_eq!(summary.diversity_failures.len(), 3);
        assert!(summary.diversity_failures[0].starts_with("canonical_coverage"));
        assert!(summary.diversity_failures[1].starts_with("decisive_fraction"));
        assert!(summary.diversity_failures[2].starts_with("distinct_game_fraction"));
    }

    #[test]
    fn verify_corpus_accepts_a_hand_built_corpus() {
        let bundle = game_bundle();

        let (game1, positions1) = make_game(
            &bundle,
            "0:0",
            ["a", "b"],
            &[Move(0), Move(3), Move(1), Move(4), Move(2)],
            Some(Outcome::Win(Player::X)),
        );
        let (game2, positions2) = make_game(
            &bundle,
            "0:1",
            ["a", "b"],
            &[
                Move(0),
                Move(1),
                Move(2),
                Move(4),
                Move(3),
                Move(5),
                Move(7),
                Move(6),
                Move(8),
            ],
            Some(Outcome::Draw),
        );

        let dir = std::env::temp_dir().join(format!("sd-summary-verify-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        write_json_pretty(
            &dir.join(RUN_FILE),
            &serde_json::json!({"schema_version": 1, "game": "tictactoe", "run_id": "r"}),
        )
        .unwrap();
        write_jsonl(&dir.join(GAMES_FILE), vec![game1, game2]).unwrap();
        let mut all_positions = positions1;
        all_positions.extend(positions2);
        write_jsonl(&dir.join(POSITIONS_FILE), all_positions).unwrap();

        assert_eq!(verify_corpus(&bundle, &dir).unwrap(), 2);

        let summary = analyze_corpus(&bundle, &dir, &DiversityThresholds::default()).unwrap();
        assert_eq!(summary.run_id, Some("r".to_string()));
        assert!(dir.join(SUMMARY_FILE).is_file());
        assert!(dir.join(ANALYZE_FILE).is_file());
    }

    #[test]
    fn summary_render_is_markdown_and_deterministic() {
        let bundle = game_bundle();

        let (win_game, win_positions) = make_game(
            &bundle,
            "0:0",
            ["a", "b"],
            &[Move(0), Move(3), Move(1), Move(4), Move(2)],
            Some(Outcome::Win(Player::X)),
        );
        let (draw_game, draw_positions) = make_game(
            &bundle,
            "0:1",
            ["a", "b"],
            &[
                Move(0),
                Move(1),
                Move(2),
                Move(4),
                Move(3),
                Move(5),
                Move(7),
                Move(6),
                Move(8),
            ],
            Some(Outcome::Draw),
        );

        let games = vec![win_game, draw_game];
        let mut positions = win_positions;
        positions.extend(draw_positions);

        let summary = summarize(&bundle, &games, &positions, &DiversityThresholds::default());
        let value = serde_json::to_value(&summary).unwrap();

        let analyzer: &dyn Analyzer<TicTacToe> = &SummaryAnalyzer;
        let rendered = analyzer.render(&value).unwrap();

        assert!(rendered.starts_with("## Summary\n\n"));
        assert!(rendered.contains("| --- |"));
        assert!(rendered.contains("canonical_coverage"));
        assert!(!rendered.contains("|---|"));

        let lines: Vec<&str> = rendered.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.starts_with('#') {
                assert_eq!(lines.get(i + 1), Some(&""), "heading {line:?} not followed by a blank line");
            }
        }

        let rendered_again = analyzer.render(&value).unwrap();
        assert_eq!(rendered, rendered_again);
    }
}
