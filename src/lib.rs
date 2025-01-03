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

/// The file module contains the file path handling functions
pub mod file;

/// The schema module contains the data structures used to represent the tasks
pub mod schema;

/// The version module contains the version information for the library
pub mod version;

/// The macros module contains the custom macros used in the library
#[macro_use]
pub mod macros;

/// The execution stack module contains the stack used to track the execution of tasks
pub use schema::ExecutionStack;
