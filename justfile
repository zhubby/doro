set positional-arguments

# Display help
help:
    just -l

# Run the Doro CLI.
cli *args:
    cargo run -p doro-cli -- "$@"

# Run the control-plane API.
control-plane *args:
    cargo run -p doro-cli -- control-plane "$@"

# Run the local agent skeleton.
agent *args:
    cargo run -p doro-cli -- agent "$@"

# format code
fmt:
    cargo fmt -- --config imports_granularity=Item

# Check the Rust workspace.
check:
    cargo check --workspace
