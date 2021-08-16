#!/usr/bin/env bash

# Analyze rustc-rayon-core if present
[ -d "rustc-rayon" ] && cd rustc-rayon && cargo rudra

export RUDRA_ALSO_ANALYZE="std,core,alloc"
export XARGO_HOME="xargo-home"
export RUDRA_USE_XARGO_INSTEAD_OF_CARGO="true"

# Delete the existing xargo home folder to recompile everything.
rm -rf "$XARGO_HOME"

# Pass any other arguments as-is to rudra.
cargo rudra -- "$@"
