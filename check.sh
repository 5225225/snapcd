set -e 

cargo clean -p snapcd

cargo fmt --all -- --check

cargo +stable clippy --all-targets --all-features -- -D warnings

cargo test --all
