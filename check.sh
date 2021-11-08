set -e 

cargo fmt --all -- --check

cargo clippy --all-targets --all-features -- -D warnings

(cd libsnapcd; cargo rustdoc -- -Z unstable-options --check; )

cargo test --all
