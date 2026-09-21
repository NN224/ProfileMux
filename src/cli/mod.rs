pub mod commands;
pub mod mutate;
pub mod output;
pub mod selector;

use clap::Parser;
use commands::Cli;

pub fn run() -> std::process::ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => {
            let _ = err.print();
            return if err.use_stderr() {
                std::process::ExitCode::FAILURE
            } else {
                std::process::ExitCode::SUCCESS
            };
        }
    };

    let result = match cli.command {
        None => match crate::tui::run() {
            Ok(()) => Ok(std::process::ExitCode::SUCCESS),
            Err(err) => Err(err),
        },
        Some(cmd) => commands::execute(cmd),
    };

    match result {
        Ok(code) => code,
        Err(err) => {
            eprintln!("pmux: {err:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
