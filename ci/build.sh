#!/bin/bash -e
source ci/env.sh
cargo install --debug --path "$(dirname "$0")/../" --force --features backtraces
