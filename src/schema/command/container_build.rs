use std::io::{
  BufRead as _,
  BufReader,
};
use std::path::Path;
use std::process::Command as ProcessCommand;
use std::thread;

use anyhow::Context as _;
use git2::Repository;
use serde::Deserialize;
use which::which;

use crate::defaults::default_verbose;
use crate::schema::{
  get_output_handler,
  is_shell_command,
  is_template_command,
  TaskContext,
};
use crate::{
  get_template_command_value,
  handle_output,
  run_shell_command,
};

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

  /// Generate a Software Bill of Materials (SBOM) for the container image
  #[serde(default)]
  pub sbom: bool,

  /// Do not use cache when building the container
  #[serde(default)]
  pub no_cache: bool,

  /// Always remove intermediate containers
  #[serde(default)]
  pub force_rm: bool,
}

#[derive(Debug, Deserialize)]
pub struct ContainerBuild {
  /// The command to run in the container
  pub container_build: ContainerBuildArgs,

  /// Show verbose output
  #[serde(default)]
  pub verbose: Option<bool>,
}

#[allow(dead_code)]
impl ContainerBuild {
  pub fn execute(&self, context: &TaskContext) -> anyhow::Result<()> {
    assert!(!self.container_build.context.is_empty());

    let verbose = self.verbose.or(context.verbose).unwrap_or(default_verbose());

    let stdout = get_output_handler(verbose);
    let stderr = get_output_handler(verbose);

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
        let label = self.get_label(context, label.trim())?;
        cmd.arg("--label").arg(label);
      }
    }

    if let Some(tags) = &self.container_build.tags {
      for tag in tags {
        let tag = self.get_tag(context, tag.trim())?;
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
    if verbose {
      handle_output!(cmd.stdout, context);
      handle_output!(cmd.stderr, context);
    }

    let status = cmd.wait()?;
    if !status.success() {
      anyhow::bail!("Container build failed");
    }

    Ok(())
  }

  fn get_tag(&self, context: &TaskContext, tag_in: &str) -> anyhow::Result<String> {
    let verbose = self.verbose.or(context.verbose).unwrap_or(default_verbose());

    if is_shell_command(tag_in)? {
      let mut cmd = context.shell().proc();
      let output = run_shell_command!(tag_in, cmd, verbose);
      Ok(output)
    } else if is_template_command(tag_in)? {
      let output = get_template_command_value!(tag_in, context);
      Ok(output)
    } else {
      Ok(tag_in.to_string())
    }
  }

  fn get_label(&self, context: &TaskContext, label_in: &str) -> anyhow::Result<String> {
    use chrono::prelude::*;

    let verbose = self.verbose.or(context.verbose).unwrap_or(default_verbose());

    if let Some((key, value)) = label_in.split_once('=') {
      match value {
        "MK_NOW" => {
          // Create formatted time in +%Y-%m-%dT%H:%M:%S%z format
          let now: DateTime<Local> = Local::now();
          let now = now.format("%Y-%m-%dT%H:%M:%S%z").to_string();
          Ok(format!("{}={}", key, now))
        },
        "MK_GIT_REVISION" => {
          let revision = self.get_git_revision().unwrap_or_else(|_| "unknown".to_string());
          Ok(format!("{}={}", key, revision))
        },
        "MK_GIT_REMOTE_ORIGIN" => {
          let remote_url = self
            .get_git_remote_origin()
            .unwrap_or_else(|_| "unknown".to_string());
          Ok(format!("{}={}", key, remote_url))
        },
        _ => {
          let value = if is_shell_command(value)? {
            let mut cmd = context.shell().proc();
            run_shell_command!(value, cmd, verbose)
          } else if is_template_command(value)? {
            get_template_command_value!(value, context)
          } else {
            value.to_string()
          };

          Ok(format!("{}={}", key, value))
        },
      }
    } else {
      Ok(label_in.to_string())
    }
  }

  fn get_git_revision(&self) -> anyhow::Result<String> {
    let repo = Repository::open(".").context("Failed to open git repository")?;
    let head = repo.head().context("Failed to get git HEAD reference")?;
    let commit = head
      .peel_to_commit()
      .context("Failed to resolve git HEAD commit")?;
    Ok(commit.id().to_string())
  }

  fn get_git_remote_origin(&self) -> anyhow::Result<String> {
    let repo = Repository::open(".").context("Failed to open git repository")?;
    let remote = repo
      .find_remote("origin")
      .context("Failed to find git remote origin")?;
    let url = remote.url().context("Failed to get git remote URL")?;
    Ok(url.to_string())
  }
}

#[cfg(test)]
mod test {
  use anyhow::Ok;

  use super::*;

  #[test]
  fn test_container_build_1() -> anyhow::Result<()> {
    let yaml = r#"
      container_build:
        image_name: my-image
        context: .
        tags:
          - latest
        labels:
          - "org.opencontainers.image.created=MK_NOW"
          - "org.opencontainers.image.revision=MK_GIT_REVISION"
          - "org.opencontainers.image.source=MK_GIT_REMOTE_ORIGIN"
        sbom: true
        no_cache: true
        force_rm: true
      verbose: false
    "#;
    let container_build = serde_yaml::from_str::<ContainerBuild>(yaml)?;

    assert_eq!(container_build.verbose, Some(false));
    assert_eq!(container_build.container_build.image_name, "my-image");
    assert_eq!(container_build.container_build.context, ".");
    assert_eq!(
      container_build.container_build.tags,
      Some(vec!["latest".to_string()])
    );
    assert_eq!(
      container_build.container_build.labels,
      Some(vec![
        "org.opencontainers.image.created=MK_NOW".to_string(),
        "org.opencontainers.image.revision=MK_GIT_REVISION".to_string(),
        "org.opencontainers.image.source=MK_GIT_REMOTE_ORIGIN".to_string(),
      ])
    );
    assert!(container_build.container_build.sbom);
    assert!(container_build.container_build.no_cache);
    assert!(container_build.container_build.force_rm);
    Ok(())
  }

  #[test]
  fn test_container_build_2() -> anyhow::Result<()> {
    let yaml = r#"
      container_build:
        image_name: my-image
        context: .
    "#;
    let container_build = serde_yaml::from_str::<ContainerBuild>(yaml)?;

    assert_eq!(container_build.verbose, None);
    assert_eq!(container_build.container_build.image_name, "my-image");
    assert_eq!(container_build.container_build.context, ".");
    assert_eq!(container_build.container_build.tags, None,);
    assert_eq!(container_build.container_build.labels, None,);
    assert!(!container_build.container_build.sbom);
    assert!(!container_build.container_build.no_cache);
    assert!(!container_build.container_build.force_rm);

    Ok(())
  }

  #[test]
  fn test_container_build_3() -> anyhow::Result<()> {
    let yaml = r#"
      container_build:
        image_name: docker.io/my-image/my-image
        context: /hello/world
    "#;
    let container_build = serde_yaml::from_str::<ContainerBuild>(yaml)?;

    assert_eq!(container_build.verbose, None);
    assert_eq!(
      container_build.container_build.image_name,
      "docker.io/my-image/my-image"
    );
    assert_eq!(container_build.container_build.context, "/hello/world");
    assert_eq!(container_build.container_build.tags, None,);
    assert_eq!(container_build.container_build.labels, None,);
    assert!(!container_build.container_build.sbom);
    assert!(!container_build.container_build.no_cache);
    assert!(!container_build.container_build.force_rm);

    Ok(())
  }
}
