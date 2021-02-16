#! /bin/bash

if ! [ -e $HOME/.cargo/bin/rustup ]
then
    echo "rustup not found. Please either run 'rustup_install.sh' or 'curl https://sh.rustup.rs -sSf | sh]'"
    return
fi
# Fetch rustup if not installed.
# curl https://sh.rustup.rs -sSf | sh
#rustup install nightly
#rustup update nightly
# if you want clippy, the rust linter:
#cargo +nightly install clippy
# if you want to use rust-wasm, uncomment
#cargo install -f cargo-web
