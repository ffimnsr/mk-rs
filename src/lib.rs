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

/// The schema module contains the data structures used to represent the tasks
pub mod schema;

pub use schema::ExecutionStack;
