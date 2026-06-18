//! `blobfig` — inspect a blobfig file from the command line.
//!
//! Values marked secret are printed as `<redacted>` (the tool relies on the
//! library's redacting `Display`), so pointing this at a config file will not
//! leak secrets to your terminal or logs.

use clap::Parser;
use std::path::PathBuf;
use std::process::ExitCode;

/// Inspect a blobfig file. Secret values are shown as `<redacted>`.
#[derive(Parser)]
#[command(name = "blobfig", version, about)]
struct Args {
    /// Path to the blobfig file to display.
    path: PathBuf,
}

fn main() -> ExitCode {
    let args = Args::parse();

    let bytes = match std::fs::read(&args.path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("blobfig: cannot read {}: {e}", args.path.display());
            return ExitCode::FAILURE;
        }
    };

    match blobfig::parse(&bytes) {
        Ok(value) => {
            println!("{value}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("blobfig: cannot parse {}: {e}", args.path.display());
            ExitCode::FAILURE
        }
    }
}
