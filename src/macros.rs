#[macro_export]
macro_rules! handle_output {
  ($output:expr, $context:expr) => {
    let output = $output
      .take()
      .with_context(|| format!("Failed to open {}", stringify!($output)))?;
    let multi_clone = $context.multi.clone();
    thread::spawn(move || {
      let reader = BufReader::new(output);
      for line in reader.lines().map_while(Result::ok) {
        let _ = multi_clone.println(line);
      }
    });
  };
}

#[macro_export]
macro_rules! run_shell_command {
  ($value:expr, $cmd:expr, $verbose:expr) => {{
    let arg = $value.trim_start_matches("$(").trim_end_matches(")");
    let stdout = if $verbose {
      std::process::Stdio::piped()
    } else {
      std::process::Stdio::null()
    };

    let mut child = $cmd.arg(arg).stdout(stdout).spawn()?;
    let stdout = child
      .stdout
      .take()
      .ok_or_else(|| anyhow::anyhow!("Failed to open stdout"))?;
    let buf = std::io::BufReader::new(stdout);
    let output = buf
      .lines()
      .next()
      .ok_or_else(|| anyhow::anyhow!("Failed to read stdout"))??;
    output
  }};
}

#[macro_export]
macro_rules! get_template_command_value {
  ($value:expr, $context:expr) => {{
    let value = $value.trim_start_matches("${{").trim_end_matches("}}").trim();
    let value = if value.starts_with("env.") {
      let value = value.trim_start_matches("env.");
      let value = $context
        .env_vars
        .get(value)
        .ok_or_else(|| anyhow::anyhow!("Failed to find environment variable"))?;
      value
    } else {
      value
    };
    value.to_string()
  }};
}
