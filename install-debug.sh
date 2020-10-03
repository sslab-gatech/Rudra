#!/bin/bash
cargo install --debug --path "$(dirname "$0")" --force --features backtraces
