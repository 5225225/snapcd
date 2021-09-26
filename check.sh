set -e 

cargo fmt --all -- --check

cargo +stable clippy --all-targets --all-features -- -D warnings

cargo +nightly clippy --all-targets --all-features -- -D warnings || true

(cd libsnapcd; cargo +nightly rustdoc -- -Z unstable-options --check; )

cargo test --all
