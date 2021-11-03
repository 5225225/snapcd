//! `libsnapcd` is a set of libraries useful to manipulating `snapcd` repositories.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(noop_method_call)]
#![warn(trivial_casts)]
#![warn(trivial_numeric_casts)]
#![warn(unreachable_pub)]
#![warn(unused_crate_dependencies)]
#![warn(unused_import_braces)]
#![warn(unused_lifetimes)]
#![warn(unused_qualifications)]

pub(crate) mod base32;
pub mod cache;
pub mod chunker;
pub mod commit;
pub mod crypto;
pub mod diff;
pub mod dir;
pub mod ds;
pub mod entry;
pub mod file;
pub mod filter;
pub mod key;
pub mod keyish;
pub mod network;
pub mod object;
