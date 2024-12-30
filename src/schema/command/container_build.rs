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
use serde::Deserialize;
use which::which;

use crate::defaults::default_true;
use crate::schema::TaskContext;

#[allow(dead_code)]
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

#[allow(dead_code)]
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

    if self.container_build.sbom {
      cmd.arg("--sbom");
    }

    if self.container_build.no_cache {
      cmd.arg("--no-cache");
    }

    if self.container_build.force_rm {
      cmd.arg("--force-rm");
    }

    if let Some(build_args) = &self.container_build.build_args {
      for arg in build_args {
        cmd.arg("--build-arg").arg(arg);
      }
    }

    if let Some(labels) = &self.container_build.labels {
      for label in labels {
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

    cmd.arg(&self.container_build.context);

    // Inject environment variables in both container and command
    for (key, value) in context.env_vars.iter() {
      cmd.env(key, value);
    }

    log::trace!("Running command: {:?}", cmd);

    let mut cmd = cmd.spawn()?;
    if self.verbose {
      let stdout = cmd.stdout.take().with_context(|| "Failed to open stdout")?;
      let stderr = cmd.stderr.take().with_context(|| "Failed to open stderr")?;

      let multi_clone = context.multi.clone();
      thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
          let _ = multi_clone.println(line);
        }
      });

      let multi_clone = context.multi.clone();
      thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
          let _ = multi_clone.println(line);
        }
      });
    }

    let status = cmd.wait()?;
    if !status.success() {
      anyhow::bail!("Container build failed");
    }

    Ok(())
  }
}
