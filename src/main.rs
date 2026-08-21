use std::process::ExitCode;

fn main() -> ExitCode {
    match strategy_discovery::cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
