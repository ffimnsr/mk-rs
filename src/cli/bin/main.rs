//! # mk-cli
//!
//! `mk-cli` is a command line interface for the `mk` library.
use cli_entry::CliEntry;
use mk_lib::schema::ExecutionInterrupted;

/// The entry point for the CLI
mod cli_entry;

/// The struct that represents the stored secrets
mod secrets;

/// Fuzzy task selector helpers
mod task_selector;

/// The main function
fn main() -> anyhow::Result<()> {
  match run() {
    Ok(()) => Ok(()),
    Err(error) if error.downcast_ref::<ExecutionInterrupted>().is_some() => {
      std::process::exit(130);
    },
    Err(error) => Err(error),
  }
}

fn run() -> anyhow::Result<()> {
  if CliEntry::maybe_print_task_completion_from_env()? {
    return Ok(());
  }

  let cli = CliEntry::new()?;
  cli.run()
}
