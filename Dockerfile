# Copied and modified from the rust official image
# https://github.com/rust-lang/docker-rust/blob/bbc7feb12033da3909dced4e88ddbb6964fbc328/1.50.0/buster/Dockerfile
FROM buildpack-deps:buster

ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH \
    RUST_VERSION=nightly-2020-08-26 \
    SCCACHE_VERSION=v0.2.15

ENV RUSTFLAGS="-L ${RUSTUP_HOME}/toolchains/${RUST_VERSION}-x86_64-unknown-linux-gnu/lib" \
    LD_LIBRARY_PATH="${RUSTUP_HOME}/toolchains/${RUST_VERSION}-x86_64-unknown-linux-gnu/lib"

# Install Rust
RUN set -eux; \
    dpkgArch="$(dpkg --print-architecture)"; \
    case "${dpkgArch##*-}" in \
        amd64) rustArch='x86_64-unknown-linux-gnu'; rustupSha256='ed7773edaf1d289656bdec2aacad12413b38ad0193fff54b2231f5140a4b07c5' ;; \
        # arm64) rustArch='aarch64-unknown-linux-gnu'; rustupSha256='f80a0a792b3ab905ab4919474daf4d3f60e574fc6987e69bfba2fd877241a8de' ;; \
        *) echo >&2 "unsupported architecture: ${dpkgArch}"; exit 1 ;; \
    esac; \
    url="https://static.rust-lang.org/rustup/archive/1.23.1/${rustArch}/rustup-init"; \
    wget "$url"; \
    echo "${rustupSha256} *rustup-init" | sha256sum -c -; \
    chmod +x rustup-init; \
    ./rustup-init -y --no-modify-path --profile minimal --default-toolchain $RUST_VERSION --default-host ${rustArch}; \
    rm rustup-init; \
    chmod -R a+w $RUSTUP_HOME $CARGO_HOME; \
    rustup component add rustc-dev; \
    rustup --version; \
    cargo --version; \
    rustc --version;

# Install sccache
RUN set -eux; \
    dpkgArch="$(dpkg --print-architecture)"; \
    case "${dpkgArch##*-}" in \
        amd64) sccacheArch='x86_64'; sccacheSha256='e5d03a9aa3b9fac7e490391bbe22d4f42c840d31ef9eaf127a03101930cbb7ca' ;; \
        # arm64) sccacheArch='aarch64'; sccacheSha256='90d91d21a767e3f558196dbd52395f6475c08de5c4951a4c8049575fa6894489' ;; \
        *) echo >&2 "unsupported architecture: ${dpkgArch}"; exit 1 ;; \
    esac; \
    dirname="sccache-${SCCACHE_VERSION}-${sccacheArch}-unknown-linux-musl"; \
    filename="${dirname}.tar.gz"; \
    url="https://github.com/mozilla/sccache/releases/download/${SCCACHE_VERSION}/${filename}"; \
    wget "$url"; \
    echo "${sccacheSha256} *${filename}" | sha256sum -c -; \
    tar -xvzf ${filename}; \
    mv ${dirname}/sccache /usr/local/bin/sccache; \
    chmod +x /usr/local/bin/sccache; \
    rm -rf ${filename} ${dirname};

# Install Rudra
COPY rust-toolchain /tmp/rust-toolchain
COPY crawl /tmp/crawl
RUN set -eux; \
    cargo install --path /tmp/crawl --bin rudra-runner --bin unsafe-counter; \
    rm -rf /tmp/rust-toolchain /tmp/crawl;

COPY . /tmp/rudra/
RUN set -eux; \
    cd /tmp/rudra; \
    ./install-release.sh; \
    rm -rf /tmp/rudra/;
