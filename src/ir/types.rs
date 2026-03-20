//! Type representations used in the IR.

use std::fmt;

/// IR types — a simplified type system for code generation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IrType {
    /// void
    Void,
    /// Integer type with bit width (e.g., 8, 16, 32, 64)
    Int(u32),
    /// Pointer to another type
    Ptr(Box<IrType>),
}

impl IrType {
    /// 8-bit integer (char)
    pub fn i8() -> Self {
        IrType::Int(8)
    }

    /// 32-bit integer (int)
    pub fn i32() -> Self {
        IrType::Int(32)
    }

    /// 64-bit integer (long)
    pub fn i64() -> Self {
        IrType::Int(64)
    }

    /// Pointer to the given type
    pub fn ptr(inner: IrType) -> Self {
        IrType::Ptr(Box::new(inner))
    }

    /// Size in bytes
    pub fn size_bytes(&self) -> u32 {
        match self {
            IrType::Void => 0,
            IrType::Int(bits) => (bits + 7) / 8,
            IrType::Ptr(_) => 8, // x86-64
        }
    }
}

impl fmt::Display for IrType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrType::Void => write!(f, "void"),
            IrType::Int(bits) => write!(f, "i{}", bits),
            IrType::Ptr(inner) => write!(f, "{}*", inner),
        }
    }
}
