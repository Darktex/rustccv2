//! rustcc: A C compiler written in Rust.
//!
//! This crate provides the compiler infrastructure:
//! - `ir`: SSA-based intermediate representation
//! - `codegen`: x86-64 code generation

pub mod codegen;
pub mod ir;
