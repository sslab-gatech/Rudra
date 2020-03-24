# Crux

Crux is a cross-language static checker for Rust

## Development Setup

```
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

## Install to Cargo

```
# this executes: cargo install --debug --path ./ --force --locked
./install-debug

crux ./test.rs  # for single file testing
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

* [ ] build call graph -> start from the root node
* [ ] flow-insensitive points-to analysis
* [ ] build data-flow graph
    * [ ] detect alloc / load / store / dealloc
    * just analyze the same function multiple times
* [ ] see if use of a pointer overlaps with a dropped pointer
