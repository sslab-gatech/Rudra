#!/bin/sh -e
rustup install nightly-2020-08-26
rustup default nightly-2020-08-26
rustup component add rustc-dev
rustup component add miri
