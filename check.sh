set -e 

cargo clean -p snapcd

cargo fmt --all -- --check

cargo clippy

cargo test
