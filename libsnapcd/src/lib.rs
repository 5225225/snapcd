//! `libsnapcd` is a set of libraries useful to manipulating `snapcd` repositories.

#![forbid(unsafe_code)]
#![warn(clippy::pedantic)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![warn(noop_method_call)]
#![warn(trivial_casts)]
#![warn(trivial_numeric_casts)]
#![warn(unreachable_pub)]
#![warn(unused_import_braces)]
#![warn(unused_lifetimes)]
#![warn(unused_qualifications)]
// Way too many false positives
// use cargo-udeps instead
// #![warn(unused_crate_dependencies)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_panics_doc)]
#![allow(missing_docs)]

pub mod base32;
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
