#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::{env, fs};

use clap::Command;
use clap_mangen::Man;

#[path = "../cli/bin/cli_entry.rs"]
mod cli_entry;
#[path = "../cli/bin/secrets/mod.rs"]
mod secrets;
#[path = "../cli/bin/task_selector.rs"]
mod task_selector;

fn main() -> anyhow::Result<()> {
  let output_dir = env::args_os()
    .nth(1)
    .map(PathBuf::from)
    .unwrap_or_else(|| PathBuf::from("target/man/man1"));
  fs::create_dir_all(&output_dir)?;
  generate_pages(cli_entry::command(), &["mk"], &output_dir)?;
  Ok(())
}

fn generate_pages(command: Command, command_path: &[&str], output_dir: &Path) -> anyhow::Result<()> {
  render_page(&command, command_path, output_dir)?;

  for subcommand in command.get_subcommands() {
    if subcommand.get_name() == "help" || subcommand.is_hide_set() {
      continue;
    }

    let mut next_path = command_path.to_vec();
    next_path.push(subcommand.get_name());
    generate_pages(subcommand.clone(), &next_path, output_dir)?;
  }

  Ok(())
}

fn render_page(command: &Command, command_path: &[&str], output_dir: &Path) -> anyhow::Result<()> {
  let page_name = command_path.join("-");
  let bin_name = command_path.join(" ");
  let mut render_command = command.clone();
  render_command = render_command.disable_help_subcommand(true);
  render_command = render_command.name(leak(page_name.clone()));
  render_command = render_command.display_name(leak(page_name.clone()));
  render_command = render_command.bin_name(leak(bin_name));

  let mut buffer = Vec::new();
  Man::new(render_command).render(&mut buffer)?;
  fs::write(output_dir.join(format!("{page_name}.1")), buffer)?;

  Ok(())
}

fn leak(value: String) -> &'static str {
  Box::leak(value.into_boxed_str())
}
