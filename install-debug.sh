#!/bin/bash
cargo install --locked --debug --path "$(dirname "$0")" --force --features backtraces
