#!/bin/sh -e
rustup install nightly-2021-08-20
rustup default nightly-2021-08-20
rustup component add rustc-dev
rustup component add miri