//! Naive x86-64 code generator.
//!
//! Each virtual register is mapped to a stack slot. This produces correct
//! but unoptimized code — register allocation comes later.

mod x86_64;

pub use x86_64::X86_64Generator;
