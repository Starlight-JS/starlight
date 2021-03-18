#!/bin/sh
cargo build --release
sudo cp -r target/release/libstarlight.so /usr/lib
sudo cp -r target/release/starlight /usr/local/bin
sudo cp -r target/release/starlight-bundle /usr/local/bin
sudo cp -r target/release/libstarlight.a /usr/lib