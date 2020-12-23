# Rudra on the Rust Standard Library

This folder documents the tools and instructions on how to analyze the rust
standard library using Rudra.

The process is roughly based on [RalfJung's miri-test-libstd](https://github.com/RalfJung/miri-test-libstd)
and uses `xargo` to build the standard library.

## Pre-requisites

1. Set up `rudra` as per instructions in the main README.

2. `cargo install xargo`

   *Ensure that `xargo` is installed.*

3. `rustup component add rust-src`

   *Make sure that you have the source code for the rust library installed*

## Analyzing

1. Install the modified release or debug version of rudra as per usual with
   `install-debug.sh` or `install-release.sh`

2. Run the `rudra_analyze_std.sh` script from this folder. You can pass any
   rudra arguments to this script such as
   `./rudra_analyze_std.sh -Zrudra-disable-panic-safety`.

## I want to analyze a different Rust version

Sure thing, just use the `XARGO_RUST_SRC` variable. For example you can compile
the latest nightly Rust standard library with:

```bash
rustup toolchain install nightly
rustup +nightly component add rust-src

export XARGO_RUST_SRC=/home/ammar/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib/rustlib/src/rust/library
./rudra_analyze_std.sh
```

## Why xargo?

`xargo` offers the easiest way to actually compile the standard library. While
it is possible to compile the standard library fully from source
(*rust-lang/rust*), their build system is fairly complicated and sets
environment variables and flags internally.

In contrast, `xargo` wraps all this complexity and builds the standard library
for us with just one line. It is far easier to hook for rudra than trying to
compile fully from source.
