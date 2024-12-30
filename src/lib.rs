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

/// The defaults module contains the default values for the library
pub mod defaults;

/// The schema module contains the data structures used to represent the tasks
pub mod schema;

/// The version module contains the version information for the library
pub mod version;

pub use schema::ExecutionStack;
