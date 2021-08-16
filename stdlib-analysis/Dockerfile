# This docker image contains all the steps to analyze the nightly-2020-08-26
# rustc and standard library.
FROM rudra:latest

# Install xargo to easily compile rustc and the standard library.
RUN cargo install xargo
# Grab the sources for rust iself.
RUN rustup component add rust-src

# Grab the sources for rustc's rayon fork for the version used in
# nightly-2020-08-26.
RUN git clone https://github.com/rust-lang/rustc-rayon.git \
    && cd rustc-rayon \
    && git checkout ae7bbbd2756

# Add the rudra_analyze_std.sh script and Cargo lock files from this folder.
# This is just a simple crate that will force xargo to build rustc and the
# standard library from source.
ADD . /

ENTRYPOINT ["/rudra_analyze_std.sh"]
