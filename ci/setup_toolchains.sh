#!/bin/sh -e
rustup install nightly-2021-10-21
rustup default nightly-2021-10-21
rustup component add rustc-dev
rustup component add miri
