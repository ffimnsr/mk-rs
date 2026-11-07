use anyhow::Result;
use clap::{Args, Subcommand};
use serde_json::Value;

use crate::json_output::print_json_value;

/// Subcommands for inspecting the task configuration schema offline.
#[derive(Debug, Args)]
#[command(
  arg_required_else_help = true,
  about = "View the task configuration JSON Schema"
)]
pub(crate) struct Schema {
  #[command(subcommand)]
  command: SchemaCommand,
}

/// The available schema subcommands.
#[derive(Debug, Subcommand)]
enum SchemaCommand {
  #[command(about = "Print the JSON Schema for the task configuration file")]
  View,
}

impl Schema {
  /// Execute the requested schema subcommand.
  pub(crate) fn execute(&self) -> Result<()> {
    match self.command {
      SchemaCommand::View => Self::view(),
    }
  }

  fn view() -> Result<()> {
    let schema = mk_lib::generate_schema()?;
    let schema: Value = serde_json::from_str(&schema)?;
    print_json_value(schema)
  }
}
