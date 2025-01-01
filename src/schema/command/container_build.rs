use std::io::{
  BufRead as _,
  BufReader,
};
use std::path::Path;
use std::process::{
  Command as ProcessCommand,
  Stdio,
};
use std::thread;

use anyhow::Context as _;
use git2::Repository;
use serde::Deserialize;
use which::which;

use crate::defaults::default_true;
use crate::handle_output;
use crate::schema::TaskContext;

#[derive(Debug, Deserialize)]
pub struct ContainerBuildArgs {
  /// The image name to build
  pub image_name: String,

  /// Defines the path to a directory to build the container
  pub context: String,

  /// The containerfile or dockerfile to use
  #[serde(default)]
  pub containerfile: Option<String>,

  /// The tags to apply to the container image
  #[serde(default)]
  pub tags: Option<Vec<String>>,

  /// Build arguments to pass to the container
  #[serde(default)]
  pub build_args: Option<Vec<String>>,

  /// Labels to apply to the container image
  #[serde(default)]
  pub labels: Option<Vec<String>>,

  #[serde(default)]
  pub sbom: bool,

  #[serde(default)]
  pub no_cache: bool,

  #[serde(default)]
  pub force_rm: bool,
}

#[derive(Debug, Deserialize)]
pub struct ContainerBuild {
  /// The command to run in the container
  pub container_build: ContainerBuildArgs,

  /// Show verbose output
  #[serde(default = "default_true")]
  pub verbose: bool,
}

#[allow(dead_code)]
impl ContainerBuild {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    assert!(!self.container_build.context.is_empty());

    let stdout = if self.verbose {
      Stdio::piped()
    } else {
      Stdio::null()
    };
    let stderr = if self.verbose {
      Stdio::piped()
    } else {
      Stdio::null()
    };

    let container_runtime = which("docker")
      .or_else(|_| which("podman"))
      .with_context(|| "Failed to find docker or podman")?;

    let mut cmd = ProcessCommand::new(container_runtime);
    cmd.arg("build").stdout(stdout).stderr(stderr);

    if self.container_build.sbom {
      cmd.arg("--sbom=true");
    }

    if self.container_build.no_cache {
      cmd.arg("--no-cache=true");
    }

    if self.container_build.force_rm {
      cmd.arg("--force-rm=true");
    }

    if let Some(build_args) = &self.container_build.build_args {
      for arg in build_args {
        cmd.arg("--build-arg").arg(arg);
      }
    }

    if let Some(labels) = &self.container_build.labels {
      for label in labels {
        let label = self.get_label(label);
        cmd.arg("--label").arg(label);
      }
    }

    if let Some(tags) = &self.container_build.tags {
      for tag in tags {
        let tag = format!("{}:{}", &self.container_build.image_name, tag);
        cmd.arg("-t").arg(tag);
      }
    } else {
      let tag = format!("{}:latest", &self.container_build.image_name);
      cmd.arg("-t").arg(tag);
    }

    if let Some(containerfile) = &self.container_build.containerfile {
      cmd.arg("-f").arg(containerfile);
    } else {
      let dockerfile = format!("{}/Dockerfile", &self.container_build.context);
      let containerfile = format!("{}/Containerfile", &self.container_build.context);

      // Check for Dockerfile and Containerfile
      if Path::new(&dockerfile).exists() {
        cmd.arg("-f").arg(dockerfile);
      } else if Path::new(&containerfile).exists() {
        cmd.arg("-f").arg(containerfile);
      } else {
        anyhow::bail!("Failed to find Dockerfile or Containerfile in context");
      }
    }

    let build_path: &str = &self.container_build.context;
    cmd.arg(build_path);

    let cmd_str = format!("{:?}", cmd);
    context.multi.println(cmd_str)?;

    // Inject environment variables in both container and command
    for (key, value) in context.env_vars.iter() {
      cmd.env(key, value);
    }

    log::trace!("Running command: {:?}", cmd);

    let mut cmd = cmd.spawn()?;
    if self.verbose {
      handle_output!(cmd.stdout, context);
      handle_output!(cmd.stderr, context);
    }

    let status = cmd.wait()?;
    if !status.success() {
      anyhow::bail!("Container build failed");
    }

    Ok(())
  }

  fn get_label(&self, label_in: &str) -> String {
    use chrono::prelude::*;

    if let Some((key, value)) = label_in.split_once('=') {
      match value {
        "MK_NOW" => {
          // Create formatted time in +%Y-%m-%dT%H:%M:%S%z format
          let now: DateTime<Local> = Local::now();
          let now = now.format("%Y-%m-%dT%H:%M:%S%z").to_string();
          format!("{}={}", key, now)
        },
        "MK_GIT_REVISION" => {
          let revision = self.get_git_revision().unwrap_or_else(|_| "unknown".to_string());
          format!("{}={}", key, revision)
        },
        "MK_GIT_REMOTE_ORIGIN" => {
          let remote_url = self
            .get_git_remote_origin()
            .unwrap_or_else(|_| "unknown".to_string());
          format!("{}={}", key, remote_url)
        },
        _ => format!("{}={}", key, value),
      }
    } else {
      label_in.to_string()
    }
  }

  fn get_git_revision(&self) -> anyhow::Result<String> {
    let repo = Repository::open(".").context("Failed to open Git repository")?;
    let head = repo.head().context("Failed to get HEAD reference")?;
    let commit = head
      .peel_to_commit()
      .context("Failed to resolve HEAD to commit")?;
    Ok(commit.id().to_string())
  }

  fn get_git_remote_origin(&self) -> anyhow::Result<String> {
    let repo = Repository::open(".").context("Failed to open Git repository")?;
    let remote = repo
      .find_remote("origin")
      .context("Failed to find 'origin' remote")?;
    let url = remote.url().context("Failed to get remote URL")?;
    Ok(url.to_string())
  }
}
