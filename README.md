# Rudra

Rudra is a static analyzer to detect common undefined behaviors in Rust programs.

## Configurations

### Unsafe Counter

- UNSAFE_COUNTER_LOG
  - Adjust logging level for `unsafe-counter`. Use `.env` file at your discretion.
  - Default: `info,tokei::language::language_type=error`

### Rudra Runner

- RUDRA_RUNNER_LOG
  - Adjust logging level for `rudra-runner`. Use `.env` file at your discretion.
  - Default: `info`
- RUDRA_SCRATCH_DIR
  - Directory to store crawled crates (default: ../rudra_scratch)
- RUDRA_REPORT_DIR
  - Directory to store reports (default: ../rudra_report)
  - Rudra-Runner will automatically set `RUDRA_REPORT_PATH`

### Rudra

- Use `-v` or `-vv` to make logging more verbose.
  More than two v's will be ignored, and only the last option will be considered (it does not accumulate).
- RUDRA_REPORT_PATH
  - Report file location. If set, Rudra analysis result will be serialized and
    saved to that file. Otherwise, the result will be printed to stderr.
  - If there already exists a file at the path, the existing content will be erased.
- RUDRA_LOG_PATH
  - Log file location. If set, log will be saved to this file as well as printed to stderr.

## Development Setup

You need a specific version of nightly Rust for Rudra development.

(TODO: Check again about MIRI testing)

```
# Toolchain setup
rustup install nightly-2020-08-26
rustup default nightly-2020-08-26
rustup component add rustc-dev
rustup component add miri

# Environment variable setup, put these in your `.bashrc`
export RUDRA_RUST_CHANNEL=nightly-2020-08-26
export RUDRA_PATH="<your project path>"

export RUDRA_SCRATCH_DIR="<your scratch path>"
export RUDRA_REPORT_DIR="<your report path>"

export RUSTFLAGS="-L $HOME/.rustup/toolchains/${RUDRA_RUST_CHANNEL}-x86_64-unknown-linux-gnu/lib"
export LD_LIBRARY_PATH="${LD_LIBRARY_PATH}:$HOME/.rustup/toolchains/${RUDRA_RUST_CHANNEL}-x86_64-unknown-linux-gnu/lib"

# Test your installation
python test.py
```

Don't forget to add `.env` file for your local development. See "Configurations" for an example.

## Code Formatting

1. Follow whatever `rustfmt` does
2. Use an empty comment line if you want to bypass rustfmt's default formatting
3. Group `use` statements in order of `std` - `rustc` internals - 3rd party - local order

## Setup rust-analyzer

Run:
```
cd ..
git clone https://github.com/rust-lang/rust.git rust-nightly-2020-08-26
cd rust-nightly-08-26
git checkout bf4342114
git submodule init
git submodule update
```

Then, add this to the workspace setting (`.vscode/settings.json`):
```
{
    "rust-analyzer.rustcSource": "<your path to rust-nightly-2020-08-26>/Cargo.toml"
}
```

## Install Rudra to Cargo

```
# this executes: cargo install --debug --path "$(dirname "$0")" --force --locked
./install-debug

rudra --crate-type lib tests/unsafe_destructor/normal1.rs  # for single file testing (you need to set library include path, or use `cargo run` instead)
cargo rudra  # for crate compilation
```
