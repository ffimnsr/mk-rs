use std::path::PathBuf;

use schemars::JsonSchema;
use serde::{
  Deserialize,
  Serialize,
};
use which::which;

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
/// The container runtime to use.
pub enum ContainerRuntime {
  Auto,
  Docker,
  Podman,
}

impl ContainerRuntime {
  pub fn resolve(runtime: Option<&ContainerRuntime>) -> anyhow::Result<PathBuf> {
    match runtime.unwrap_or(&ContainerRuntime::Auto) {
      ContainerRuntime::Auto => which("docker").or_else(|_| which("podman")).map_err(|_| {
        anyhow::anyhow!(
          "No container runtime found. Install Docker or Podman and ensure it is available in PATH."
        )
      }),
      ContainerRuntime::Docker => which("docker")
        .map_err(|_| anyhow::anyhow!("Docker not found. Install Docker and ensure it is available in PATH.")),
      ContainerRuntime::Podman => which("podman")
        .map_err(|_| anyhow::anyhow!("Podman not found. Install Podman and ensure it is available in PATH.")),
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
