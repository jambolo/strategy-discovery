//! Association-mining crate survey, hand-collected via `cargo search` and embedded at compile
//! time from `survey.json`.

use serde::{Deserialize, Serialize};

/// One surveyed crate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SurveyEntry {
    #[serde(rename = "crate")]
    pub crate_name: String,
    pub version: String,
    pub description: String,
    pub updated_at: String,
    pub found_by: String,
}

/// Loads the embedded survey.
pub fn load() -> Vec<SurveyEntry> {
    let raw = include_str!("../survey.json");
    serde_json::from_str(raw).expect("survey.json is valid JSON")
}
