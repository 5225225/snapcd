cargo clean --package snapcd &&
cargo clippy -- \
    -W clippy::all \
    -W clippy::pedantic \
    -D clippy::option_unwrap_used \
    -D clippy::result_unwrap_used \
