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
# this executes: cargo install --debug --path ./ --force --locked
./install-debug

crux ./test.rs  # for single file testing
cargo crux  # for crate compilation
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
