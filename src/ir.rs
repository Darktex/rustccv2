//! SSA-based Intermediate Representation.
//!
//! The IR uses virtual registers (unlimited), basic blocks,
//! and a simple instruction set suitable for lowering to x86-64.

use std::fmt;

/// A virtual register index.
pub type VReg = u32;

/// A basic block label.
pub type Label = u32;

/// A string label (for data section references).
pub type StringId = usize;

#[derive(Debug, Clone)]
pub struct IrProgram {
    pub functions: Vec<IrFunction>,
    pub string_literals: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<VReg>,
    pub blocks: Vec<BasicBlock>,
    pub next_vreg: VReg,
    #[allow(dead_code)]
    pub next_label: Label,
    #[allow(dead_code)]
    pub stack_size: u32,
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub label: Label,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone)]
pub enum Instruction {
    /// dst = constant
    LoadImm(VReg, i64),
    /// dst = string label address
    LoadStringAddr(VReg, StringId),
    /// dst = lhs op rhs
    BinOp(VReg, IrBinOp, VReg, VReg),
    /// dst = op src
    UnaryOp(VReg, IrUnaryOp, VReg),
    /// dst = call func(args...)
    Call(VReg, String, Vec<VReg>),
    /// Allocate stack slot, store address in dst
    Alloca(VReg),
    /// Store value into stack slot
    Store(VReg, VReg), // Store(addr, value)
    /// Load value from stack slot into dst
    Load(VReg, VReg), // Load(dst, addr)
    /// Copy src to dst
    #[allow(dead_code)]
    Copy(VReg, VReg),
}

#[derive(Debug, Clone)]
pub enum Terminator {
    /// Return a value
    Return(VReg),
    /// Return void
    ReturnVoid,
    /// Unconditional jump
    Jump(Label),
    /// Conditional branch: if cond != 0 goto then_label else goto else_label
    Branch(VReg, Label, Label),
    /// Placeholder — block not yet terminated
    None,
}

#[derive(Debug, Clone, Copy)]
pub enum IrBinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    BitwiseAnd,
    BitwiseOr,
    BitwiseXor,
    ShiftLeft,
    ShiftRight,
}

#[derive(Debug, Clone, Copy)]
pub enum IrUnaryOp {
    Negate,
    BitwiseNot,
    LogicalNot,
}

impl fmt::Display for IrProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, s) in self.string_literals.iter().enumerate() {
            writeln!(f, ".str{i} = \"{s}\"")?;
        }
        for func in &self.functions {
            writeln!(f, "\n{func}")?;
        }
        Ok(())
    }
}

impl fmt::Display for IrFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let params: Vec<String> = self.params.iter().map(|v| format!("%{v}")).collect();
        writeln!(f, "fn {}({}):", self.name, params.join(", "))?;
        for block in &self.blocks {
            writeln!(f, "  bb{}:", block.label)?;
            for inst in &block.instructions {
                writeln!(f, "    {inst}")?;
            }
            writeln!(f, "    {}", block.terminator)?;
        }
        Ok(())
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instruction::LoadImm(dst, val) => write!(f, "%{dst} = {val}"),
            Instruction::LoadStringAddr(dst, id) => write!(f, "%{dst} = &.str{id}"),
            Instruction::BinOp(dst, op, l, r) => write!(f, "%{dst} = %{l} {op:?} %{r}"),
            Instruction::UnaryOp(dst, op, src) => write!(f, "%{dst} = {op:?} %{src}"),
            Instruction::Call(dst, name, args) => {
                let args: Vec<String> = args.iter().map(|v| format!("%{v}")).collect();
                write!(f, "%{dst} = call {name}({})", args.join(", "))
            }
            Instruction::Alloca(dst) => write!(f, "%{dst} = alloca"),
            Instruction::Store(addr, val) => write!(f, "store %{val} -> [%{addr}]"),
            Instruction::Load(dst, addr) => write!(f, "%{dst} = load [%{addr}]"),
            Instruction::Copy(dst, src) => write!(f, "%{dst} = copy %{src}"),
        }
    }
}

impl fmt::Display for Terminator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Terminator::Return(v) => write!(f, "ret %{v}"),
            Terminator::ReturnVoid => write!(f, "ret void"),
            Terminator::Jump(l) => write!(f, "jmp bb{l}"),
            Terminator::Branch(c, t, e) => write!(f, "br %{c}, bb{t}, bb{e}"),
            Terminator::None => write!(f, "<unterminated>"),
        }
    }
}
