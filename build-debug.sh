#!/bin/sh
cargo build 
sudo cp -r target/debug/libstarlight.a /usr/lib
sudo cp -r target/debug/libstarlight.so /usr/lib
sudo cp -r target/debug/starlight /usr/local/bin
sudo cp -r target/debug/starlight-bundle /usr/local/bin
