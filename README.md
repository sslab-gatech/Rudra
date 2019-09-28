# Crux

Crux is a cross-language static checker for Rust

## Development Setup

```
# Install Rust nightly toolchain
rustup install toolchain nightly

# Test your installation
cargo run -- samples/example.rs
```

## Install to Cargo

```
cargo install [--debug] --path ./ --force --locked
# Now these commands work
crux ./test.rs  # for single file testing
cargo crux  # for crate compilation
```
