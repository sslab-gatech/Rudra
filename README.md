# Rudra

Rudra is a static analyzer to detect common undefined behaviors in Rust programs.

## Basic usage

```
# this executes: cargo install --path "$(dirname "$0")" --force
./install-release

rudra --crate-type lib tests/unsafe_destructor/normal1.rs  # for single file testing (you need to set library include path, or use `cargo run` instead)
cargo rudra  # for crate compilation
```

## Using Docker

`rudra:latest` image must exist to use rudra-runner.

```
# Build a docker image
docker build . -t rudra:latest

# Optional: cleanup old images
docker system prune

# Run Rudra on a single target
docker run -t --rm --user "$(id -u)":"$(id -g)" -v "$PWD":/tmp/rudra -w /tmp/rudra rudra:latest cargo rudra -Zno-index-update

# Run Rudra runner
docker run -t --rm --user "$(id -u)":"$(id -g)" -v "$RUDRA_RUNNER_HOME":/tmp/rudra-runner-home \
  --env RUDRA_RUNNER_HOME=/tmp/rudra-runner-home -v "$PWD":/tmp/rudra -w /tmp/rudra rudra:latest rudra-runner
```

## Configurations

### Unsafe Counter

- UNSAFE_COUNTER_LOG
  - Adjust logging level for `unsafe-counter`. Use `.env` file at your discretion.
  - Default: `info,tokei::language::language_type=error`

### Rudra Runner

- RUDRA_RUNNER_LOG
  - Adjust logging level for `rudra-runner`. Use `.env` file at your discretion.
  - Default: `info`
- RUDRA_RUNNER_HOME
  - Home directory for Rudra Runner
    - There is a setup script: `./setup_rudra_runner_home.py <path>`
  - This is only used for Rudra runner. The default `cargo rudra` will use the default cargo directory.
  - Directory structure:
    - cargo_home
    - sccache_home
    - rudra_cache
      - db-dump.tar.gz
      - db-dump
        - 2020-07-04-140112
          - data
            - crates.csv
            - versions.csv
            - (other files)
          - (other files)
      - For each crate, `crate-x.y.z` directory and `crate-x.y.z.crate` tarball
    - campaign
      - YYYYMMDD_HHmmss
        - report
        - log
  - `CARGO_HOME` and `SCCACHE_DIR` will be automatically set when the runner is used.
    - `SCCACHE_CACHE_SIZE` will be set to "10T"
  - `RUDRA_REPORT_PATH` and `RUDRA_LOG_PATH` will be automatically set when runner is used.

### Rudra

- Use `-v` or `-vv` to make logging more verbose.
  More than two v's will be ignored, and only the last option will be considered (it does not accumulate).
- If `sccache` is found in the path, it will be used to build dependencies
- `RUDRA_REPORT_PATH`
  - Report file location. If set, Rudra analysis result will be serialized and
    saved to that file. Otherwise, the result will be printed to stderr.
  - If there already exists a file at the path, the existing content will be erased.
- `RUDRA_LOG_PATH`
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
export RUDRA_RUNNER_HOME="<your runner home path - use setup_rudra_runner_home.py>"

export RUSTFLAGS="-L $HOME/.rustup/toolchains/${RUDRA_RUST_CHANNEL}-x86_64-unknown-linux-gnu/lib"
export LD_LIBRARY_PATH="${LD_LIBRARY_PATH}:$HOME/.rustup/toolchains/${RUDRA_RUST_CHANNEL}-x86_64-unknown-linux-gnu/lib"

# Test your installation
python test.py
```

You can add `.env` file for local customization. See "Configurations" for an example.

### Code Formatting

1. Follow whatever `rustfmt` does
2. Use an empty comment line if you want to bypass rustfmt's default formatting
3. Group `use` statements in order of `std` - `rustc` internals - 3rd party - local order

### Setup rust-analyzer

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
