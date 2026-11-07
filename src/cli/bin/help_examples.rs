//! Long-help examples shared by CLI command definitions.

pub(crate) const DOCTOR_EXAMPLES: &str = r#"Examples:
  mk doctor
  mk -c tasks.toml doctor
"#;

pub(crate) const SECRETS_DOCTOR_EXAMPLES: &str = r#"Examples:
  mk secrets doctor
  mk -c tasks.toml secrets doctor
"#;

pub(crate) const PURGE_KEY_EXAMPLES: &str = r#"Examples:
  mk secrets key purge-key --name old-key
  mk secrets key purge-key --location ./.mk/keys --name ci
"#;

pub(crate) const RUN_EXAMPLES: &str = r#"Examples:
  # Run one task and forward arguments
  mk run deploy -- production

  # Run tasks matching labels
  mk run --label ci --label env=prod

  # Preview the resolved plan without executing commands
  mk run build --dry-run

  # Run one matrix variant
  mk run test --set os=linux

Offline Podman:
  # tasks.yaml
  container_runtime: podman

  tasks:
    build-image:
      commands:
        - container_build:
            image_name: localhost/mk-rust
            context: .
            containerfile: ./Containerfile
            tags:
              - latest
            runtime: podman

    test-in-podman:
      depends_on:
        - name: build-image
      commands:
        - container_command:
            - cargo
            - test
            - --locked
          image: localhost/mk-rust:latest
          runtime: podman

  # Build once while online, then run without network access:
  mk run build-image
  mk run test-in-podman

  # mk adds no explicit pull step. Pre-pull or pre-build every image you need,
  # including Containerfile base images, before going offline.
"#;

pub(crate) const PLAN_EXAMPLES: &str = r#"Examples:
  mk plan build
  mk plan build --json
  mk plan --label ci
  mk plan test --set os=linux
  mk -c tasks.toml plan release
"#;

pub(crate) const LIST_EXAMPLES: &str = r#"Examples:
  mk list
  mk list --plain
  mk list --json
  mk list --label ci
  mk -c tasks.toml list --no-color
"#;

pub(crate) const WATCH_EXAMPLES: &str = r#"Examples:
  mk watch build
  mk watch build --path src --path Cargo.toml
  mk watch build --debounce 1s --clear
  mk watch --label ci --path src
"#;

pub(crate) const VALIDATE_EXAMPLES: &str = r#"Examples:
  mk validate
  mk validate --json
  mk -c tasks.toml validate
"#;

pub(crate) const INIT_EXAMPLES: &str = r#"Examples:
  mk init
  mk init tasks.toml
  mk init tasks.json
  mk init tasks.lua
  mk init --force tasks.yaml
"#;

pub(crate) const COMPLETION_EXAMPLES: &str = r#"Examples:
  mk completion bash > ~/.local/share/bash-completion/completions/mk
  mk completion zsh > ~/.zfunc/_mk
  mk completion fish > ~/.config/fish/completions/mk.fish
  mk completion powershell > mk-completion.ps1
"#;

pub(crate) const SCHEMA_VIEW_EXAMPLES: &str = r#"Examples:
  # Print the schema. No network access required.
  mk schema view

  # Save it for editor schema configuration
  mk schema view > schema.json

  # Reference the local file from tasks.yaml
  # yaml-language-server: $schema=./schema.json
"#;

pub(crate) const SECRETS_EXAMPLES: &str = r#"Examples:
  # Generate a built-in PGP key and initialize a vault
  mk secrets key generate-key --name default
  mk secrets vault init

  # Store a secret from stdin and read it back
  printf 's3cret' | mk secrets vault store app/token
  mk secrets vault show app/token

  # Use a hardware-backed GPG key
  mk secrets key import -g <KEY_ID> --name yubikey
  mk secrets vault init -g <KEY_ID>

  # Export a secret to a dotenv file
  mk secrets vault export app/env > .env

  # Inspect resolved secret settings
  mk secrets doctor

Secret data:
  Secret paths are slash-separated names:
    app/token
    app/production/env

  Single value, referenced in config with ${{ secrets.<path> }}:
    mk secrets vault store app/token 's3cret'

  Dotenv content, loaded into task environment with secrets_path:
    printf 'DATABASE_URL=postgres://...\nAPI_KEY=example\n' | mk secrets vault store app/production/env

    secrets:
      secrets_path:
        - app/production/env

    tasks:
      deploy:
        environment:
          API_TOKEN: ${{ secrets.app/token }}
"#;

pub(crate) const SECRETS_KEY_EXAMPLES: &str = r#"Examples:
  mk secrets key generate-key --name default
  mk secrets key list-keys
  mk secrets key import -g <KEY_ID> --name yubikey
  mk secrets key export-key --name default --output default.key
  mk secrets key purge-key --name default
"#;

pub(crate) const GENERATE_KEY_EXAMPLES: &str = r#"Examples:
  mk secrets key generate-key --name default
  mk secrets key generate-key --location ./.mk/keys --name ci --force
"#;

pub(crate) const IMPORT_KEY_EXAMPLES: &str = r#"Examples:
  mk secrets key import -g <KEY_ID> --name yubikey
  mk secrets key import -g <FINGERPRINT> --location ./.mk/keys
"#;

pub(crate) const LIST_KEYS_EXAMPLES: &str = r#"Examples:
  mk secrets key list-keys
  mk secrets key list-keys --location ./.mk/keys
"#;

pub(crate) const EXPORT_KEY_EXAMPLES: &str = r#"Examples:
  mk secrets key export-key --name default --output default.key
  mk secrets key export-key --name yubikey --output yubikey.key
"#;

pub(crate) const SECRETS_VAULT_EXAMPLES: &str = r#"Examples:
  mk secrets vault init
  mk secrets vault init -g <KEY_ID> --write-config
  printf 's3cret' | mk secrets vault store app/token
  mk secrets vault show app/token
  mk secrets vault list app/
  mk secrets vault export app/production/env > .env
  mk secrets vault purge app/token --yes
"#;

pub(crate) const INIT_VAULT_EXAMPLES: &str = r#"Examples:
  mk secrets vault init
  mk secrets vault init -g <KEY_ID> --write-config
  mk secrets vault init --vault-location ./.mk/vault --key-name default
"#;

pub(crate) const STORE_SECRET_EXAMPLES: &str = r#"Examples:
  printf 's3cret' | mk secrets vault store app/token
  mk secrets vault store app/token 's3cret' --force
"#;

pub(crate) const SHOW_SECRET_EXAMPLES: &str = r#"Examples:
  mk secrets vault show app/token
  mk secrets vault show app/token --plain
  mk secrets vault show app/production/env
"#;

pub(crate) const EXPORT_SECRET_EXAMPLES: &str = r#"Examples:
  mk secrets vault export app/env --output .env
  mk secrets vault export app/env > .env
"#;

pub(crate) const LIST_SECRETS_EXAMPLES: &str = r#"Examples:
  mk secrets vault list
  mk secrets vault list app/
  mk secrets vault list app/ --plain
"#;

pub(crate) const PURGE_SECRET_EXAMPLES: &str = r#"Examples:
  mk secrets vault purge app/token --yes
  mk secrets vault purge app/production/env --yes
"#;
