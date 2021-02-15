#! /usr/bin/sh
cargo test --workspace --verbose
rustup default nightly
rustup component add clippy
cargo +nightly clippy --workspace --all-targets