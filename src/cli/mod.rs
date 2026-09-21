/// Placeholder entry point: replaced by the clap-derive command tree.
pub fn run() -> std::process::ExitCode {
    match crate::tui::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("pmux: {err:#}");
            std::process::ExitCode::FAILURE
        }
    }
}
