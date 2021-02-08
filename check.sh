set -e 

cargo clean -p snapcd

cargo fmt --all -- --check

cargo clippy --all-targets --all-features -- -D warnings

cargo test --all
