//! Error taxonomy -> process exit codes: 0 success, 1 runtime failure, 2 usage/input error,
//! 3 a requested check failed. `exit_code` classifies an `anyhow::Error` by walking its cause
//! chain outermost-first; the first classifiable cause decides.

use crate::core::traits::{MatchError, StrategyError};
use crate::discovery::CorpusError;
use crate::io::IoError;
use std::io::ErrorKind;
use thiserror::Error;

/// Exit code for a runtime failure (I/O write failure, engine/strategy failure, solver limit, internal invariant).
pub const EXIT_FAILURE: u8 = 1;
/// Exit code for an invalid invocation or input.
pub const EXIT_USAGE: u8 = 2;
/// Exit code for a requested check that failed (`analyze --strict`).
pub const EXIT_CHECK: u8 = 3;

/// Errors raised by the CLI layer itself.
#[derive(Debug, Error)]
pub enum CliError {
    /// Invalid flag combination or missing required flag (exit 2).
    #[error("{0}")]
    Usage(String),
    /// A requested check failed (exit 3); the message lists every failure.
    #[error("check failed: {0}")]
    CheckFailed(String),
}

/// Exit code for `err`: the first cause in `err.chain()` that is a [`CliError`], [`CorpusError`],
/// [`IoError`], [`StrategyError`], `std::io::Error` or `toml::de::Error` decides; otherwise 1.
pub fn exit_code(err: &anyhow::Error) -> u8 {
    for cause in err.chain() {
        if let Some(e) = cause.downcast_ref::<CliError>() {
            return match e {
                CliError::Usage(_) => EXIT_USAGE,
                CliError::CheckFailed(_) => EXIT_CHECK,
            };
        }
        if let Some(e) = cause.downcast_ref::<CorpusError>() {
            return corpus_exit_code(e);
        }
        if let Some(e) = cause.downcast_ref::<IoError>() {
            return io_exit_code(e);
        }
        if let Some(e) = cause.downcast_ref::<StrategyError>() {
            return strategy_exit_code(e);
        }
        if let Some(e) = cause.downcast_ref::<std::io::Error>() {
            return if e.kind() == ErrorKind::NotFound {
                EXIT_USAGE
            } else {
                EXIT_FAILURE
            };
        }
        if cause.downcast_ref::<toml::de::Error>().is_some() {
            return EXIT_USAGE;
        }
    }
    EXIT_FAILURE
}

fn corpus_exit_code(err: &CorpusError) -> u8 {
    match err {
        CorpusError::Config(_) | CorpusError::Precondition(_) => EXIT_USAGE,
        CorpusError::Io(e) => io_exit_code(e),
        CorpusError::Strategy(e) | CorpusError::Match(MatchError::Strategy(e)) => strategy_exit_code(e),
        CorpusError::Match(_) | CorpusError::Rules(_) | CorpusError::Solver(_) => EXIT_FAILURE,
    }
}

fn io_exit_code(err: &IoError) -> u8 {
    match err {
        IoError::Io { source, .. } => {
            if source.kind() == ErrorKind::NotFound {
                EXIT_USAGE
            } else {
                EXIT_FAILURE
            }
        }
        IoError::Json { .. } => EXIT_FAILURE,
        IoError::Toml { .. } | IoError::SchemaVersion { .. } | IoError::Missing { .. } | IoError::Invalid(_) => EXIT_USAGE,
    }
}

fn strategy_exit_code(err: &StrategyError) -> u8 {
    match err {
        StrategyError::Unimplemented { .. } => EXIT_USAGE,
        StrategyError::NoLegalActions | StrategyError::Other(_) => EXIT_FAILURE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::RulesError;
    use crate::discovery::SolverError;
    use std::path::PathBuf;

    /// Wraps `e` the same way the CLI's `?` operator does before `main` calls [`exit_code`].
    fn code<E: std::error::Error + Send + Sync + 'static>(e: E) -> u8 {
        exit_code(&anyhow::Error::from(e))
    }

    #[test]
    fn plain_anyhow_error_is_failure() {
        assert_eq!(exit_code(&anyhow::anyhow!("x")), EXIT_FAILURE);
    }

    #[test]
    fn cli_usage_is_usage() {
        assert_eq!(code(CliError::Usage("x".to_string())), EXIT_USAGE);
    }

    #[test]
    fn cli_check_failed_is_check() {
        assert_eq!(code(CliError::CheckFailed("x".to_string())), EXIT_CHECK);
    }

    #[test]
    fn config_is_usage() {
        assert_eq!(code(CorpusError::Config("x".to_string())), EXIT_USAGE);
    }

    #[test]
    fn precondition_is_usage() {
        assert_eq!(code(CorpusError::Precondition("x".to_string())), EXIT_USAGE);
    }

    #[test]
    fn corpus_io_not_found_is_usage() {
        let err = CorpusError::Io(IoError::Io {
            path: PathBuf::from("x"),
            source: std::io::Error::from(ErrorKind::NotFound),
        });
        assert_eq!(code(err), EXIT_USAGE);
    }

    #[test]
    fn io_permission_denied_is_failure() {
        let err = IoError::Io {
            path: PathBuf::from("x"),
            source: std::io::Error::from(ErrorKind::PermissionDenied),
        };
        assert_eq!(code(err), EXIT_FAILURE);
    }

    #[test]
    fn json_is_failure() {
        let err = IoError::Json {
            path: PathBuf::from("x"),
            line: 1,
            source: serde_json::from_str::<u32>("x").unwrap_err(),
        };
        assert_eq!(code(err), EXIT_FAILURE);
    }

    #[test]
    fn toml_is_usage() {
        let err = IoError::Toml {
            path: PathBuf::from("x"),
            message: "bad".to_string(),
        };
        assert_eq!(code(err), EXIT_USAGE);
    }

    #[test]
    fn schema_version_is_usage() {
        let err = IoError::SchemaVersion {
            path: PathBuf::from("x"),
            expected: 1,
            found: 2,
        };
        assert_eq!(code(err), EXIT_USAGE);
    }

    #[test]
    fn missing_is_usage() {
        let err = IoError::Missing {
            path: PathBuf::from("x"),
        };
        assert_eq!(code(err), EXIT_USAGE);
    }

    #[test]
    fn invalid_is_usage() {
        assert_eq!(code(IoError::Invalid("bad".to_string())), EXIT_USAGE);
    }

    #[test]
    fn match_missing_provider_is_failure() {
        let err = CorpusError::Match(MatchError::MissingProvider("p1".to_string()));
        assert_eq!(code(err), EXIT_FAILURE);
    }

    #[test]
    fn match_strategy_unimplemented_is_usage() {
        let err = CorpusError::Match(MatchError::Strategy(StrategyError::Unimplemented {
            kind: "k".to_string(),
            detail: "d".to_string(),
        }));
        assert_eq!(code(err), EXIT_USAGE);
    }

    #[test]
    fn rules_is_failure() {
        assert_eq!(code(CorpusError::Rules(RulesError::GameOver)), EXIT_FAILURE);
    }

    #[test]
    fn solver_is_failure() {
        assert_eq!(code(CorpusError::Solver(SolverError::TooLarge { limit: 1 })), EXIT_FAILURE);
    }

    #[test]
    fn strategy_unimplemented_is_usage() {
        let err = CorpusError::Strategy(StrategyError::Unimplemented {
            kind: "k".to_string(),
            detail: "d".to_string(),
        });
        assert_eq!(code(err), EXIT_USAGE);
    }

    #[test]
    fn no_legal_actions_is_failure() {
        assert_eq!(code(StrategyError::NoLegalActions), EXIT_FAILURE);
    }

    #[test]
    fn strategy_other_is_failure() {
        assert_eq!(code(StrategyError::Other("x".to_string())), EXIT_FAILURE);
    }

    #[test]
    fn raw_io_not_found_is_usage() {
        assert_eq!(code(std::io::Error::from(ErrorKind::NotFound)), EXIT_USAGE);
    }

    #[test]
    fn raw_toml_error_is_usage() {
        let err = toml::from_str::<toml::Table>("a = [").unwrap_err();
        assert_eq!(code(err), EXIT_USAGE);
    }

    #[test]
    fn context_wrapped_corpus_error_is_classified() {
        let err = anyhow::Error::from(CorpusError::Config("c".to_string())).context("reading x");
        assert_eq!(exit_code(&err), EXIT_USAGE);
    }
}
