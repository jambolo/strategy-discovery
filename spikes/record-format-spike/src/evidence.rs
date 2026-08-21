//! Evidence header shared by the `.md` and `.json` outputs, and small markdown-table helpers.

use serde_json::{Value, json};
use std::process::Command;

/// Fixed environment facts plus the run's date, git commit, rustc version and invocation.
pub struct Header {
    pub date: String,
    pub git_commit: String,
    pub rustc: String,
    pub os: String,
    pub cpu: String,
    pub ram: String,
    pub build_profile: String,
    pub command: String,
}

impl Header {
    pub fn gather(command: String) -> Header {
        let date = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let git_commit = run_trimmed("git", &["rev-parse", "HEAD"]);
        let rustc = run_trimmed("rustc", &["--version"]);
        let build_profile = if cfg!(debug_assertions) { "debug" } else { "release" }.to_string();
        Header {
            date,
            git_commit,
            rustc,
            os: "Windows 11 Pro 10.0.26200".to_string(),
            cpu: "AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs)".to_string(),
            ram: "31 GiB".to_string(),
            build_profile,
            command,
        }
    }

    pub fn md_rows(&self) -> Vec<(&'static str, &str)> {
        vec![
            ("date", &self.date),
            ("git_commit", &self.git_commit),
            ("rustc", &self.rustc),
            ("os", &self.os),
            ("cpu", &self.cpu),
            ("ram", &self.ram),
            ("build_profile", &self.build_profile),
            ("command", &self.command),
        ]
    }

    pub fn to_json(&self) -> Value {
        json!({
            "date": self.date,
            "git_commit": self.git_commit,
            "rustc": self.rustc,
            "os": self.os,
            "cpu": self.cpu,
            "ram": self.ram,
            "build_profile": self.build_profile,
            "command": self.command,
        })
    }
}

fn run_trimmed(cmd: &str, args: &[&str]) -> String {
    let output = Command::new(cmd)
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap_or_else(|e| panic!("run `{cmd} {args:?}`: {e}"));
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Renders a `| a | b |` markdown table with a `| --- | --- |` delimiter row.
pub fn md_table_2(header: [&str; 2], rows: &[(impl AsRef<str>, impl AsRef<str>)]) -> String {
    let mut out = String::new();
    out.push_str(&format!("| {} | {} |\n", header[0], header[1]));
    out.push_str("| --- | --- |\n");
    for (a, b) in rows {
        out.push_str(&format!("| {} | {} |\n", a.as_ref(), b.as_ref()));
    }
    out
}

/// Median of three timings, in milliseconds.
pub fn median3(mut xs: [f64; 3]) -> f64 {
    xs.sort_by(|a, b| a.partial_cmp(b).expect("timings are finite"));
    xs[1]
}
