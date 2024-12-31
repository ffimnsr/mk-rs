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
