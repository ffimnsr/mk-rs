mod command;
mod precondition;
mod task;
mod task_context;
mod task_dependency;
mod task_root;
mod use_npm;

use std::collections::HashSet;
use std::sync::{
  Arc,
  Mutex,
};

pub type ExecutionStack = Arc<Mutex<HashSet<String>>>;

pub use command::*;
pub use precondition::*;
pub use task::*;
pub use task_context::*;
pub use task_dependency::*;
pub use task_root::*;
pub use use_npm::*;
