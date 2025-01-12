local greet = function()
  return 'Hello, World!'
end

return {
  tasks = {
    greet = string.format("echo '%s'", greet()),
    install_hooks = {
        commands = {
            { command = "git config --local core.hooksPath .githooks" }
        },
        description = "Install git hooks"
    },
    run = {
        commands = {
            { command = "cargo r" }
        },
        description = "Run the project"
    },
    fmt = {
        commands = {
            { command = "cargo fmt --all" }
        },
        description = "Format the project"
    },
    lint = {
        commands = {
            { command = "cargo clippy --all-features --all-targets --tests --benches -- -Dclippy::all" }
        },
        description = "Lint check the project"
    }
  }
}
