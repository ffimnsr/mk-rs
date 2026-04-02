use std::path::PathBuf;

use serde::{
  Deserialize,
  Serialize,
};
use which::which;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ContainerRuntime {
  Auto,
  Docker,
  Podman,
}

impl ContainerRuntime {
  pub fn resolve(runtime: Option<&ContainerRuntime>) -> anyhow::Result<PathBuf> {
    match runtime.unwrap_or(&ContainerRuntime::Auto) {
      ContainerRuntime::Auto => which("docker")
        .or_else(|_| which("podman"))
        .map_err(|_| anyhow::anyhow!("Failed to find docker or podman")),
      ContainerRuntime::Docker => which("docker").map_err(|_| anyhow::anyhow!("Failed to find docker")),
      ContainerRuntime::Podman => which("podman").map_err(|_| anyhow::anyhow!("Failed to find podman")),
    }
  }

  pub fn name(&self) -> &'static str {
    match self {
      ContainerRuntime::Auto => "auto",
      ContainerRuntime::Docker => "docker",
      ContainerRuntime::Podman => "podman",
    }
  }
}
