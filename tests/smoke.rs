//! Compile-and-wire smoke tests for the module tree.

#[test]
fn cli_bootstrap_runs() {
    assert!(strategy_discovery::cli::run().is_ok());
}

#[test]
fn crate_metadata_is_exposed() {
    assert_eq!(strategy_discovery::NAME, "strategy-discovery");
    assert!(!strategy_discovery::VERSION.is_empty());
}
