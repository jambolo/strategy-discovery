//! Command-line entry points — one subcommand per pipeline stage (Phase 6).
//! Placeholder: prints the crate name and version.

/// Runs the CLI. Placeholder until subcommands land.
pub fn run() -> anyhow::Result<()> {
    println!("{} {}", crate::NAME, crate::VERSION);
    Ok(())
}
