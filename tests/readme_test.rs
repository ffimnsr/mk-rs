#[test]
fn test_readme_1() -> anyhow::Result<()> {
  trycmd::TestCases::new().case("README.md");

  Ok(())
}
