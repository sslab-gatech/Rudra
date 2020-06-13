# Crux

Crux is a static analyzer to detect common undefined behaviors in Rust programs.

## Development Setup

You need nightly Rust for Crux and custom Miri for PoC testing.

```
# Toolchain setup
rustup component add rustc-dev
export RUSTFLAGS="-L $HOME/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib"

# Clone custom build MIRI
git clone https://github.com/JOE1994/miri miri-custom

# Setup custom MIRI and related toolchain
cd miri-custom
cargo install rustup-toolchain-install-master
git checkout custom_use
./rustup-toolchain
./miri install
cd ..

# Verify that you have the correct custom MIRI version by checking the commit ID
cargo miri --version

# Test your installation
cargo run -- --crate-type lib samples/trivial_escape.rs
```

You may want to add `.env` file for your local development:

```
CRUX_LOG=warn,unsafe_counter=info,crawl=info,tokei::language::language_type=error
```

## Updating Custom MIRI

```
# (inside `miri-custom` directory)
git rebase master
./rustup-toolchain
./miri install
```

## Code Formatting

1. Follow whatever `rustfmt` does
2. Group `use` statements in order of `std` - `rustc` internals - 3rd party - local order

## Install Crux to Cargo

```
# this executes: cargo install --debug --path ./ --force --locked
./install-debug

crux ./test.rs  # for single file testing (you need to set library include path, or use `cargo run` instead)
cargo crux  # for crate compilation
cargo crux-update  # wrapper for ./install-debug
```

## Baseline Algorithm

```
Input: P
output: UAF

cg = CallGraph(P)
pta = PTA(p, cg)
path = collectHeapOpDFPath(P)
foreach P in Path
    UAF(p, pta)
```

- [ ] build call graph -> start from the root node
- [ ] flow-insensitive points-to analysis
- [ ] build data-flow graph
  - [ ] detect alloc / load / store / dealloc
  - just analyze the same function multiple times
- [ ] see if use of a pointer overlaps with a dropped pointer
