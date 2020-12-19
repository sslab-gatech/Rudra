#!/usr/bin/env bash
export RUDRA_ALSO_ANALYZE="std,core,alloc"
export XARGO_HOME="xargo-home"
export CARGO_CHECK="xargo-check"

# Delete the existing xargo home folder to recompile everything.
rm -rf "$XARGO_HOME"

# Pass any other arguments as-is to rudra.
cargo rudra -- -Zrudra-act-as-compiler "$@"
