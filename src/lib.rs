//! # mk-lib
//!
//! `mk-lib` is a library for parsing and running tasks defined in a YAML file.
//!
//! ## Data formats
//!
//! The following data formats are supported:
//!
//! - [YAML], a self-proclaimed human-friendly configuration language that ain't
//!   markup language.
//!
//! [YAML]: https://github.com/dtolnay/serde-yaml

/// Task execution cache helpers
pub mod cache;

/// The defaults module contains the default values for the library
pub mod defaults;

/// The file module contains the file path handling functions
pub mod file;

/// The schema module contains the data structures used to represent the tasks
pub mod schema;

/// Shared secret vault helpers used by the CLI and task execution
pub mod secrets;

/// The version module contains the version information for the library
pub mod version;

/// The macros module contains the custom macros used in the library
#[macro_use]
pub mod macros;

/// The utils module contains the utility functions used in the library
pub mod utils;

/// Shared task execution state types
pub use schema::{
  ActiveTasks,
  CompletedTasks,
};

/// Generate the JSON Schema for the task configuration file as a pretty-printed JSON string.
pub fn generate_schema() -> anyhow::Result<String> {
  let schema = schemars::schema_for!(schema::TaskRoot);
  Ok(serde_json::to_string_pretty(&schema)?)
}
