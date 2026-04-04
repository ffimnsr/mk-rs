# Config Schema

## CLI Commands

| Command | Description |
| --- | --- |
| `mk validate` | Validate task configuration without executing tasks. |
| `mk validate --json` | Emit validation results in JSON format. |
| `mk plan <task>` | Show the resolved dependency and command plan for a task. |
| `mk plan <task> --json` | Emit the task plan in JSON format. |
| `mk run <task> --dry-run` | Print the resolved task plan without executing commands. |
| `mk run <task> --force` | Bypass task cache and force execution. |
| `mk run <task> --json-events` | Emit newline-delimited JSON task and command events. |
| `mk clean-cache` | Remove persisted task cache metadata. |

Planning commands are side-effect free and do not evaluate shell or template expressions.
Relative `extends`, `env_file`, command `work_dir`, container build `context`, and `containerfile` paths resolve from the config file directory. Task cache `inputs` and `outputs` prefer a single effective local command `work_dir` when the task defines one consistently.

## Root

| Name | Type | Default Value | Required | Description |
| --- | --- | --- | --- | --- |
| tasks | HashMap<String, Task> | - | true | Contains list of tasks keyed by task name. |
| environment | HashMap<String, String> | {} | false | Environment variables applied to all tasks. |
| env_file | String[] | [] | false | Environment files applied to all tasks. |
| secrets_path | String[] | [] | false | Secret paths whose decrypted values are parsed as dotenv content and merged into the environment. |
| vault_location | String | ./.mk/vault | false | Secret vault location used for `secrets_path` and `${{ secrets.NAME }}` resolution. |
| keys_location | String | ~/.config/mk/priv | false | Private key directory used for secret decryption. |
| key_name | String | default | false | Private key name used for secret decryption. |
| gpg_key_id | String | - | false | GPG key ID or fingerprint. When set, mk delegates all vault crypto to the system `gpg` binary, enabling YubiKey and passphrase-protected keys. |
| use_npm | Bool or UseNpm | false | false | This allows mk to use npm scripts as tasks. |
| use_cargo | Bool or UseCargo | false | false | This allows mk to use cargo commands as tasks. |
| container_runtime | auto / docker / podman | auto | false | Default container runtime for container commands. |
| extends | String | - | false | Load and merge another task file before the current file. |

### UseNpm

| Name | Type | Default Value | Required | Description |
| --- | --- | --- | --- | --- |
| package_manager | String | - | false | The package manager to use (.e.g pnpm, npm, yarn). |
| work_dir | String | - | false | The working directory to run the command in. |

### UseCargo

| Name | Type | Default Value | Required | Description |
| --- | --- | --- | --- | --- |
| work_dir | String | - | false | The working directory to run the command in. |

`include` is deprecated and unsupported. Use `extends` instead. Loading a config that still declares `include` fails fast.

### Task

| Name | Type | Default Value | Required | Description |
| --- | --- | --- | --- | --- |
| commands | CommandRunner[] | - | true | The commands to run. |
| preconditions | Precondition[] | [] | false | The preconditions that must be met before the task can be executed. |
| depends_on | [String / TaskDependency][] | [] | false | The tasks that must be executed before this task can be executed. |
| labels | HashMap<String, String> | {} | false | The labels for the task. |
| description | String | \<empty-string\> | false | The description of the task. |
| environment | HashMap<String, String> | {} | false | The environment variables to set before running the task. |
| env_file | String[] | [] | false | The environment files to load before running the task. |
| secrets_path | String[] | [] | false | Secret paths whose decrypted values are parsed as dotenv content and merged into the task environment. |
| vault_location | String | inherited | false | Override the secret vault location for this task. |
| keys_location | String | inherited | false | Override the private key directory for this task. |
| key_name | String | inherited | false | Override the private key name for this task. |
| gpg_key_id | String | inherited | false | Override the GPG key ID for this task. When set, delegates vault crypto to the system `gpg` binary. |
| shell | String | sh | false | The shell to call for command execution. |
| parallel | bool | false | false | Run local_run commands in parallel. |
| execution | TaskExecution | - | false | Richer execution settings for parallel mode. |
| cache | TaskCache | - | false | Enable incremental cache lookups for the task. |
| inputs | String[] | [] | false | Files or glob patterns that affect task output. |
| outputs | String[] | [] | false | Files produced by the task. |
| ignore_errors | bool | false | false | Ignore errors if the task fails? |
| verbose | bool | true | false | Show verbose output. |

Task environment values also support `${{ secrets.path/to/secret }}` in addition to `${{ env.NAME }}`. Secret templates decrypt the referenced secret and inject the raw value.

```yaml
vault_location: ./.mk/vault
keys_location: ./.mk/keys
key_name: team
secrets_path:
  - app/common

tasks:
  deploy:
    environment:
      PSQL_PASSWORD: ${{ secrets.app/database/password }}
    commands:
      - command: ./deploy.sh
```

#### TaskExecution

| Name | Type | Default Value | Required | Description |
| --- | --- | --- | --- | --- |
| mode | sequential / parallel | sequential | false | Execute commands sequentially or in parallel. |
| max_parallel | usize | number-of-commands | false | Limit concurrent command execution when mode is parallel. |
| fail_fast | bool | true | false | Stop scheduling new parallel work after the first failure. |

#### TaskCache

| Name | Type | Default Value | Required | Description |
| --- | --- | --- | --- | --- |
| enabled | bool | true | false | Enable cache reads and writes for the task. |

#### CommandRunner

The command runner can either be a `CommandRun`, `LocalRun`, `ContainerRun`, `ContainerBuild`, and `TaskRun`.

##### CommandRun

Run the command string without any customatization:

```yaml
tasks:
  commands:
    - echo "foobar"
```

##### LocalRun

Run the command in local available shell.

| Name | Type | Default Value | Required | Description |
| --- | --- | --- | --- | --- |
| command | String | - | true | The command to run. |
| shell | String | sh | false | The shell to call. |
| test | String | - | false | A test command to run before executing the main command. |
| work_dir | String | \<current-working-directory\> | false | The working directory to run the command into. |
| interactive | bool | false | false | Run the command interactively (stdin/stdout attached). |
| retrigger | bool | false | false | Allow pressing `R` to stop and restart a non-interactive command manually. Currently only supported on Unix systems. Not supported with `interactive: true`, parallel task execution, or `--json-events`. |
| save_output_as | String | - | false | Save the command stdout as a task-scoped output for later commands in the same task. |
| ignore_errors | bool | false | false | Ignore errors if the task fails? |
| verbose | bool | true | false | Show verbose output. |

```yaml
tasks:
  commands:
    - command: echo foobar
      shell: /bin/zsh
      work_dir: /srv
      retrigger: true
      save_output_as: release_tag
      ignore_errors: true
```

Saved outputs can be reused by later local commands in the same task with `${{ outputs.NAME }}`. Captured stdout keeps internal newlines and trims trailing newline characters.

```yaml
tasks:
  release:
    environment:
      IMAGE_TAG: app-${{ outputs.version }}
    commands:
      - command: printf '1.2.3\n'
        save_output_as: version
      - command: echo "$IMAGE_TAG"
      - command: printf '%s\n' "${{ outputs.version }}"
```

##### ContainerRun

Run the command in container environment. This automatically searches for available `docker` or `podman` command to use.

| Name | Type | Default Value | Required | Description |
| --- | --- | --- | --- | --- |
| container_command | String[] | - | true | The command to run in the container. |
| image | String | - | true | The container image to use. |
| mounted_paths | String[] | [] | false | The mounted paths to bind mount into the container. Relative host-side paths resolve from the config file directory. |
| runtime | auto / docker / podman | auto | false | Explicitly choose the container runtime. |
| ignore_errors | bool | false | false | Ignore errors if the task fails? |
| verbose | bool | true | false | Show verbose output. |

**Example**

```yaml
tasks:
  commands:
    - container_command: echo foobar
      image: docker.io/library/bash:latest
      mounted_paths:
        - /srv:/srv:ro,z
      ignore_errors: true
```

##### ContainerBuild

Build a container image. This automatically searches for available `docker` or `podman` command to use.

| Name | Type | Default Value | Required | Description |
| --- | --- | --- | --- | --- |
| container_build | ContainerBuildArgs | - | true | The command build arguments. |
| verbose | bool | false | false | Show verbose output. |

###### ContainerBuildArgs

| Name | Type | Default Value | Required | Description |
| --- | --- | --- | --- | --- |
| image_name | String | - | true | The image name to put in image tag. |
| context | String | - | true | Defines the path to a directory to build the container. |
| containerfile | String | - | false | The containerfile or dockerfile to use (automatically searches context for either a `Containerfile` or `Dockerfile`). |
| tags | String[] | [] | false | The tags to apply to the container image. |
| build_args | String[] | [] | false | Build arguments to pass to the container. |
| labels | String[] | [] | false | Labels to apply to the container image. |
| sbom | bool | false | false | Add sbom to image. |
| no_cache | bool | false | false | Don't cache builds. |
| force_rm | bool | false | false | Delete intermediary containers use to build images. |
| runtime | auto / docker / podman | auto | false | Explicitly choose the container runtime. |

**Example**

```yaml
tasks:
  commands:
    - container_build:
        image_name: ghcr.io/test/test
        context: .
        containerfile: ./docker/Dockerfile
        tags:
          - latest
          - '1.0.0'
          - '1.0'
        labels:
          - org.opencontainers.image.created=MK_NOW
        sbom: true
        force_rm: true
```

Some commands templates for labels: `MK_NOW` to get current date formatted, `MK_GIT_REVISION` to get the current git revision, and lastly `MK_GIT_REMOTE_ORIGIN` to get remote origin of git repo folder.

##### TaskRun

Run another task.

| Name | Type | Default Value | Required | Description |
| --- | --- | --- | --- | --- |
| task | String | - | true | The name of the task to run. |
| ignore_errors | bool | false | false | Ignore errors if the task fails? |
| verbose | bool | true | false | Show verbose output. |

**Example**

```yaml
tasks:
  commands:
    - task: task_a
```

#### Precondition

The preconditions that must be met before the task can be executed.

| Name | Type | Default Value | Required | Description |
| --- | --- | --- | --- | --- |
| command | String | - | true | The commands to run. |
| message | String | - | false | The message to display if you get error. |
| shell | String | sh | false | The shell to call. |
| work_dir | String | \<current-working-directory\> | false | The working directory to run the command into. |
| verbose | bool | true | false | Show verbose output. |

**Example**

```yaml
tasks:
  preconditions:
    - command: test -d $PWD/.test
      message: Directory does exist
      work_dir: /srv
      shell: zsh
    - command: test $(uname) = Linux
      message: OS is not linux
```

#### TaskDependency

The tasks that must be executed before this task can be executed.

| Name | Type | Default Value | Required | Description |
| --- | --- | --- | --- | --- |
| name | String | - | true | The name of the task to run. |

TaskDependency can be either a string task name or an object with `name`.

**Example**

```yaml
tasks:
  depends_on:
    - name: task_a
```

## Real-world Example

```yaml
tasks:
  install-hooks:
    commands:
      - command: git config --local core.hooksPath .githooks
    description: Install git hooks
  fmt:
    commands:
      - command: cargo fmt --all
    description: Format the project
  lint:
    commands:
      - command: cargo clippy --all-features --all-targets --tests --benches -- -Dclippy::all
    description: Lint check the project
  check:
    commands:
      - command: cargo c
    description: Check the project
  build:
    commands:
      - command: cargo b
    description: Build the project
    depends_on:
      - name: check
  build-in-container:
    commands:
      - container_command:
          - cargo
          - c
        image: docker.io/library/rust:latest
    description: Build the project in a container
    depends_on:
      - name: check
  pack:
    preconditions:
      - command: git diff-index --quiet --exit-code HEAD --
      - command: cargo c
    commands:
      - command: |
          latest_version=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].version')
          name=$(cargo metadata --no-deps --format-version 1 | jq -r '.packages[0].name')
          podman build \
            --sbom=true \
            --label org.opencontainers.image.created=$(date +%Y-%m-%dT%H:%M:%S%z) \
            --label org.opencontainers.image.authors=gh:@ffimnsr \
            --label org.opencontainers.image.description="$name $latest_version" \
            --label org.opencontainers.image.revision=$(git rev-parse HEAD) \
            --label org.opencontainers.image.source=$(git remote get-url origin) \
            --label org.opencontainers.image.title=$name \
            --label org.opencontainers.image.url=https://github.com/ffimnsr/mk-rs \
            --label org.opencontainers.image.version=$latest_version \
            -f Containerfile \
            -t ghcr.io/ffimnsr/$name-rs:$latest_version \
            -t ghcr.io/ffimnsr/$name-rs:latest .
    description: Build the container image
  pack_2:
    preconditions:
      - command: cargo c
    commands:
      - container_build:
          image_name: ghcr.io/ffimnsr/mk-rs
          context: .
          tags:
            - latest
          sbom: true
          labels:
            - org.opencontainers.image.created=MK_NOW
            - org.opencontainers.image.authors=gh:@ffimnsr
            - org.opencontainers.image.description=mk-rs
            - org.opencontainers.image.revision=MK_GIT_REVISION
            - org.opencontainers.image.source=MK_GIT_REMOTE_ORIGIN
            - org.opencontainers.image.title=mk-rs
            - org.opencontainers.image.url=https://github.com/ffimnsr/mk-rs
            - org.opencontainers.image.version=latest
    description: Build the container image
  docs:
    commands:
      - command: docsify serve docs
    description: Serve the documentation
```
