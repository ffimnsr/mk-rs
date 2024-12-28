//! # mk-cli
//!
//! `mk-cli` is a command line interface for the `mk` library.
use cli_entry::CliEntry;

/// The entry point for the CLI
mod cli_entry;

/// The main function
fn main() -> anyhow::Result<()> {
  let cli = CliEntry::new()?;
  cli.run()
}
