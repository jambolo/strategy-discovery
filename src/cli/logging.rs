//! Logging setup: a `tracing` subscriber on stderr controlled by `-v`/`-q` and `RUST_LOG`.
//!
//! stdout carries only a stage's result; every log line goes to stderr, so logging can never
//! change a persisted file or a stdout line. `RUST_LOG` (when set and non-empty) overrides the
//! flag-derived level with an `EnvFilter` directive string, e.g. `RUST_LOG=off`.

use tracing_subscriber::EnvFilter;

/// Level name selected by the verbosity flags: `quiet` -> `error`; otherwise `warn` (0),
/// `info` (1), `debug` (2) or `trace` (3 or more `-v`).
pub fn level_name(verbosity: u8, quiet: bool) -> &'static str {
    if quiet {
        return "error";
    }
    match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    }
}

/// The `EnvFilter` directive to install: `rust_log` when it is set and non-blank, else
/// [`level_name`] of the flags.
pub fn filter_directive(verbosity: u8, quiet: bool, rust_log: Option<&str>) -> String {
    match rust_log {
        Some(value) if !value.trim().is_empty() => value.to_string(),
        _ => level_name(verbosity, quiet).to_string(),
    }
}

/// Installs the global stderr subscriber once; later calls are no-ops, so `run_from` may be
/// called repeatedly in one process (tests).
pub fn init(verbosity: u8, quiet: bool) {
    let directive = filter_directive(verbosity, quiet, std::env::var("RUST_LOG").ok().as_deref());
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(directive))
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .with_target(false)
        .without_time()
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_name_table() {
        assert_eq!(level_name(0, true), "error");
        assert_eq!(level_name(5, true), "error");
        assert_eq!(level_name(0, false), "warn");
        assert_eq!(level_name(1, false), "info");
        assert_eq!(level_name(2, false), "debug");
        assert_eq!(level_name(3, false), "trace");
        assert_eq!(level_name(9, false), "trace");
    }

    #[test]
    fn filter_directive_prefers_rust_log() {
        assert_eq!(filter_directive(1, false, None), "info");
        assert_eq!(filter_directive(1, false, Some("")), "info");
        assert_eq!(filter_directive(1, false, Some("   ")), "info");
        assert_eq!(filter_directive(0, false, Some("off")), "off");
        assert_eq!(
            filter_directive(2, false, Some("strategy_discovery=trace")),
            "strategy_discovery=trace"
        );
        assert_eq!(filter_directive(0, true, None), "error");
    }

    #[test]
    fn init_twice_is_harmless() {
        // The first `init` wins process-wide, so install `error` first to keep lib test
        // stderr free of INFO lines; the second call is then a no-op.
        init(0, true);
        init(1, false);
        tracing::info!("logging smoke");
    }
}
