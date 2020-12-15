export RUDRA_RUST_CHANNEL=nightly-2020-08-26
export RUDRA_PATH="$PWD"
export RUDRA_SCRATCH_DIR="/tmp/"
export RUDRA_REPORT_DIR="/tmp/"
export RUSTFLAGS="-L $HOME/.rustup/toolchains/${RUDRA_RUST_CHANNEL}-x86_64-unknown-linux-gnu/lib"
export LD_LIBRARY_PATH="${LD_LIBRARY_PATH}:$HOME/.rustup/toolchains/${RUDRA_RUST_CHANNEL}-x86_64-unknown-linux-gnu/lib"
