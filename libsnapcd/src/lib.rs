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
#![warn(clippy::use_self)]
#![warn(clippy::clone_on_ref_ptr)]
#![warn(clippy::create_dir)]
#![warn(clippy::decimal_literal_representation)]
#![warn(clippy::if_then_some_else_none)]
#![warn(clippy::mod_module_files)]
#![warn(clippy::multiple_inherent_impl)]
#![warn(clippy::rest_pat_in_fully_bound_structs)]
#![warn(clippy::same_name_method)]
#![warn(clippy::str_to_string)]
#![warn(clippy::unseparated_literal_suffix)]

// Way too many false positives
// use cargo-udeps instead
// #![warn(unused_crate_dependencies)]

#[allow(clippy::missing_errors_doc)]
#[allow(missing_docs)]
pub mod base32;

#[allow(clippy::missing_errors_doc)]
#[allow(missing_docs)]
pub mod cache;

pub mod chunker;

#[allow(clippy::missing_errors_doc)]
#[allow(missing_docs)]
pub mod commit;

pub mod crypto;

#[allow(clippy::missing_errors_doc)]
#[allow(clippy::missing_panics_doc)]
#[allow(missing_docs)]
pub mod diff;

#[allow(clippy::missing_errors_doc)]
#[allow(clippy::missing_panics_doc)]
#[allow(missing_docs)]
pub mod dir;

#[allow(clippy::missing_errors_doc)]
#[allow(missing_docs)]
pub mod ds;

#[allow(clippy::missing_panics_doc)]
#[allow(missing_docs)]
pub mod entry;

#[allow(clippy::missing_errors_doc)]
#[allow(clippy::missing_panics_doc)]
#[allow(missing_docs)]
pub mod file;

#[allow(clippy::missing_panics_doc)]
#[allow(missing_docs)]
pub mod filter;

#[allow(clippy::missing_errors_doc)]
#[allow(missing_docs)]
pub mod key;

#[allow(missing_docs)]
pub mod keyish;

#[allow(clippy::missing_panics_doc)]
#[allow(missing_docs)]
pub mod network;

#[allow(clippy::missing_errors_doc)]
#[allow(missing_docs)]
pub mod object;
