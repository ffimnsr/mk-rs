# Contributing

Thanks for helping improve `mk`.

## Before You Start

- Read the [Code of Conduct](CODE_OF_CONDUCT.md)
- Check open issues before starting large changes
- For bugs and feature ideas, open or discuss an issue first when the change is
  not obviously small

## Development Setup

1. Install Rust.
2. Clone the repository.
3. Run the main checks from the repository root.

```bash
cargo fmt --all
cargo clippy --all-features --all-targets --tests --benches -- -Dclippy::all
cargo test
```

You can also use the project tasks:

```bash
mk fmt
mk lint
mk check
```

## Making Changes

- Keep changes focused and easy to review
- Add or update tests when behavior changes
- Update documentation when commands, config, or output changes
- Preserve existing style and naming unless the change is intentionally a refactor

## Pull Requests

When opening a pull request, please include:

- A clear summary of the change
- Why the change is needed
- Notes about tests you ran
- Any follow-up work or known limitations

Small, self-contained pull requests are easiest to review and merge.

## Reporting Bugs

Please use GitHub Issues and include:

- Steps to reproduce
- Expected behavior
- Actual behavior
- Version, platform, and relevant config

## Security Reports

For vulnerabilities, do not open a public issue.

Follow [SECURITY.md](SECURITY.md).

## Licensing

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.
