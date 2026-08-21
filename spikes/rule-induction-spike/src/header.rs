//! Evidence header shared by the `.md` and `.json` artifacts.

use std::process::Command;

/// Fixed environment facts plus per-run provenance (date, commit, rustc, invocation).
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

fn run_git_commit() -> String {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("run git rev-parse HEAD");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn run_rustc_version() -> String {
    let output = Command::new("rustc").arg("--version").output().expect("run rustc --version");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// Collects the header for this run.
pub fn collect() -> Header {
    let args: Vec<String> = std::env::args().skip(1).collect();
    Header {
        date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        git_commit: run_git_commit(),
        rustc: run_rustc_version(),
        os: "Windows 11 Pro 10.0.26200".to_string(),
        cpu: "AMD Ryzen 7 7800X3D 8-Core Processor (16 logical CPUs)".to_string(),
        ram: "31 GiB".to_string(),
        build_profile: if cfg!(debug_assertions) { "debug" } else { "release" }.to_string(),
        command: format!(
            "cargo run --release --manifest-path spikes/rule-induction-spike/Cargo.toml -- {}",
            args.join(" ")
        ),
    }
}

impl Header {
    /// `(field, value)` pairs in the fixed order used by the markdown table.
    pub fn pairs(&self) -> Vec<(&'static str, &str)> {
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

    /// The same fields, keyed for JSON.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
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
