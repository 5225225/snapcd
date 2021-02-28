#![warn(
    anonymous_parameters,
    bare_trait_objects,
    elided_lifetimes_in_paths,
    missing_debug_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unused_extern_crates,
    unused_import_braces,
    unused_qualifications
)]
#![macro_use]

macro_rules! ldbg {
    () => {
        log::trace!("");
    };
    ($val:expr) => {
        // Use of `match` here is intentional because it affects the lifetimes
        // of temporaries - https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                log::trace!("{} = {:?}", stringify!($val), &tmp);
                tmp
            }
        }
    };
    // Trailing comma with single argument is ignored
    ($val:expr,) => { ldbg!($val) };
    ($($val:expr),+ $(,)?) => {
        ($(ldbg!($val)),+,)
    };
}

#[allow(dead_code)]
fn test_ldbg() {
    ldbg!();
    ldbg!(0x4242);
    ldbg!(0x4242,);
    ldbg!(1, 2, 3, 4);
}

pub mod base32;
pub mod cache;
pub mod commit;
pub mod dir;
pub mod ds;
pub mod file;
pub mod filter;
pub mod key;
pub mod keyish;
pub mod object;
pub mod crypto;

pub use ds::DataStore;
pub use ds::{GetReflogError, Reflog, WalkReflogError};
pub use keyish::Keyish;
pub use object::Object;
