//! Rule-induction spike: fits decision trees over a labelled tic-tac-toe position dataset with
//! `linfa-trees` and `smartcore`, records fit time / accuracy / determinism / structure
//! traversability, prints one tree as an ordered decision list, and surveys association-mining
//! crates. Throwaway evaluation crate; not part of the pipeline.

mod dataset;
mod decision_list;
mod header;
mod models;
mod solver;
mod survey;

use clap::Parser;
use std::fs;
use std::path::PathBuf;

/// CLI arguments.
#[derive(Parser, Debug)]
struct Args {
    /// Output directory; artifacts are written to `<out>/benchmarks/`.
    #[arg(long, default_value = concat!(env!("CARGO_MANIFEST_DIR"), "/../../artifacts"))]
    out: String,
    /// Seed for the train/held-out split.
    #[arg(long, default_value_t = 20_260_820u64)]
    seed: u64,
}

fn main() {
    let args = Args::parse();

    let built = dataset::build(args.seed);
    let linfa_result = models::fit_linfa(&built);
    let smartcore_result = models::fit_smartcore(&built);

    let decision = decision_list::render(&linfa_result.model, &built.column_names, &built.train_labels);

    let header = header::collect();
    let survey_entries = survey::load();

    let md = render_markdown(&header, &built, &linfa_result, &smartcore_result, &decision, &survey_entries);
    let json = render_json(&header, &built, &linfa_result, &smartcore_result, &decision, &survey_entries);

    let out_dir = PathBuf::from(&args.out).join("benchmarks");
    fs::create_dir_all(&out_dir).expect("create output directory");
    fs::write(out_dir.join("rule-induction.md"), md).expect("write rule-induction.md");
    fs::write(
        out_dir.join("rule-induction.json"),
        serde_json::to_string_pretty(&json).expect("serialize json"),
    )
    .expect("write rule-induction.json");

    println!("wrote {}", out_dir.join("rule-induction.md").display());
}

fn render_markdown(
    header: &header::Header,
    built: &dataset::Built,
    linfa_result: &models::LinfaResult,
    smartcore_result: &models::SmartcoreResult,
    decision: &decision_list::Decision,
    survey_entries: &[survey::SurveyEntry],
) -> String {
    let mut md = String::new();

    md.push_str("## Header\n\n");
    md.push_str("| field | value |\n");
    md.push_str("| --- | --- |\n");
    for (k, v) in header.pairs() {
        md.push_str(&format!("| {k} | {v} |\n"));
    }
    md.push('\n');

    md.push_str("## Dataset\n\n");
    md.push_str(&format!("- rows: {}\n", built.n_rows));
    md.push_str(&format!("- features: {}\n", built.n_features));
    md.push_str(&format!("- train rows: {}\n", built.train_labels.len()));
    md.push_str(&format!("- held-out rows: {}\n", built.holdout_labels.len()));
    md.push_str("- classes: 0=loss 1=draw 2=win (side to move)\n");
    md.push_str(&format!(
        "- class counts: loss={} draw={} win={}\n",
        built.class_counts[0], built.class_counts[1], built.class_counts[2]
    ));
    md.push_str(&format!("- seed: {}\n", built.seed));
    md.push('\n');

    md.push_str("## Results\n\n");
    md.push_str("| library | version | fit_ms | train_accuracy | holdout_accuracy | deterministic | traversable | api_used |\n");
    md.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    md.push_str(&format!(
        "| linfa-trees | {} | {:.3} | {:.4} | {:.4} | {} | {} | {} |\n",
        built.versions.linfa_trees,
        linfa_result.fit_ms,
        linfa_result.train_accuracy,
        linfa_result.holdout_accuracy,
        if linfa_result.deterministic { "YES" } else { "NO" },
        linfa_result.traversable,
        linfa_result.api_used,
    ));
    md.push_str(&format!(
        "| smartcore | {} | {:.3} | {:.4} | {:.4} | {} | {} | {} |\n",
        built.versions.smartcore,
        smartcore_result.fit_ms,
        smartcore_result.train_accuracy,
        smartcore_result.holdout_accuracy,
        if smartcore_result.deterministic { "YES" } else { "NO" },
        smartcore_result.traversable,
        smartcore_result.api_used,
    ));
    md.push('\n');

    md.push_str("## Decision list\n\n");
    md.push_str(&format!(
        "- source: {} max_depth=8 leaves={}\n\n",
        decision.source,
        decision.rules.len()
    ));
    md.push_str("```text\n");
    for rule in &decision.rules {
        md.push_str(rule);
        md.push('\n');
    }
    md.push_str(&format!("ELSE class={}\n", decision.majority_class));
    md.push_str("```\n\n");

    md.push_str("## Association-mining crate survey\n\n");
    md.push_str("| crate | version | description | updated_at | found_by |\n");
    md.push_str("| --- | --- | --- | --- | --- |\n");
    for e in survey_entries {
        md.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            e.crate_name, e.version, e.description, e.updated_at, e.found_by
        ));
    }

    md
}

fn render_json(
    header: &header::Header,
    built: &dataset::Built,
    linfa_result: &models::LinfaResult,
    smartcore_result: &models::SmartcoreResult,
    decision: &decision_list::Decision,
    survey_entries: &[survey::SurveyEntry],
) -> serde_json::Value {
    serde_json::json!({
        "header": header.to_json(),
        "component_versions": {
            "linfa": built.versions.linfa,
            "linfa-trees": built.versions.linfa_trees,
            "smartcore": built.versions.smartcore,
            "ndarray": built.versions.ndarray,
        },
        "dataset": {
            "rows": built.n_rows,
            "features": built.n_features,
            "train_rows": built.train_labels.len(),
            "holdout_rows": built.holdout_labels.len(),
            "classes": { "0": "loss", "1": "draw", "2": "win" },
            "class_counts": {
                "loss": built.class_counts[0],
                "draw": built.class_counts[1],
                "win": built.class_counts[2],
            },
            "seed": built.seed,
        },
        "results": [
            {
                "library": "linfa-trees",
                "version": built.versions.linfa_trees,
                "fit_ms": linfa_result.fit_ms,
                "train_accuracy": linfa_result.train_accuracy,
                "holdout_accuracy": linfa_result.holdout_accuracy,
                "deterministic": linfa_result.deterministic,
                "traversable": linfa_result.traversable,
                "api_used": linfa_result.api_used,
                "node_count": linfa_result.node_count,
                "leaf_count": linfa_result.leaf_count,
            },
            {
                "library": "smartcore",
                "version": built.versions.smartcore,
                "fit_ms": smartcore_result.fit_ms,
                "train_accuracy": smartcore_result.train_accuracy,
                "holdout_accuracy": smartcore_result.holdout_accuracy,
                "deterministic": smartcore_result.deterministic,
                "traversable": smartcore_result.traversable,
                "api_used": smartcore_result.api_used,
                "node_count": smartcore_result.node_count,
                "leaf_count": smartcore_result.leaf_count,
            },
        ],
        "decision_list": {
            "source": decision.source,
            "rules": decision.rules,
            "else_class": decision.majority_class,
        },
        "survey": survey_entries,
    })
}
