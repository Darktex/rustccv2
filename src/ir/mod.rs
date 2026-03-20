//! SSA-based Intermediate Representation for rustcc.
//!
//! The IR uses a standard SSA form with basic blocks, phi functions, and
//! virtual registers. Each value is defined exactly once (SSA property).

mod builder;
mod types;

pub use builder::IrBuilder;
pub use types::*;

use std::fmt;

/// A unique identifier for a virtual register (SSA value).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VReg(pub u32);

impl fmt::Display for VReg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "%{}", self.0)
    }
}

/// A unique identifier for a basic block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bb{}", self.0)
    }
}

/// Binary arithmetic/logic operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    And,
    Or,
    Xor,
    Shl,
    Shr,
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BinOp::Add => write!(f, "add"),
            BinOp::Sub => write!(f, "sub"),
            BinOp::Mul => write!(f, "mul"),
            BinOp::Div => write!(f, "div"),
            BinOp::Mod => write!(f, "mod"),
            BinOp::And => write!(f, "and"),
            BinOp::Or => write!(f, "or"),
            BinOp::Xor => write!(f, "xor"),
            BinOp::Shl => write!(f, "shl"),
            BinOp::Shr => write!(f, "shr"),
        }
    }
}

/// Comparison operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

impl fmt::Display for CmpOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CmpOp::Eq => write!(f, "eq"),
            CmpOp::Ne => write!(f, "ne"),
            CmpOp::Lt => write!(f, "lt"),
            CmpOp::Le => write!(f, "le"),
            CmpOp::Gt => write!(f, "gt"),
            CmpOp::Ge => write!(f, "ge"),
        }
    }
}

/// Unary operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOp::Neg => write!(f, "neg"),
            UnaryOp::Not => write!(f, "not"),
            UnaryOp::BitNot => write!(f, "bitnot"),
        }
    }
}

/// An SSA instruction.
#[derive(Debug, Clone)]
pub enum Instruction {
    /// `dst = constant(value)`
    Constant { dst: VReg, value: i64 },

    /// `dst = binop(lhs, rhs)`
    BinOp {
        dst: VReg,
        op: BinOp,
        lhs: VReg,
        rhs: VReg,
    },

    /// `dst = unaryop(operand)`
    UnaryOp {
        dst: VReg,
        op: UnaryOp,
        operand: VReg,
    },

    /// `dst = cmp(op, lhs, rhs)` — produces 0 or 1
    Cmp {
        dst: VReg,
        op: CmpOp,
        lhs: VReg,
        rhs: VReg,
    },

    /// `dst = alloca(size)` — allocate stack space, returns pointer
    Alloca { dst: VReg, size: u32 },

    /// `dst = load(addr)` — load from memory
    Load { dst: VReg, addr: VReg },

    /// `store(addr, value)` — store to memory
    Store { addr: VReg, value: VReg },

    /// `dst = call(func, args)` — function call
    Call {
        dst: Option<VReg>,
        func: String,
        args: Vec<VReg>,
    },

    /// `dst = phi([(value, block), ...])` — SSA phi function
    Phi {
        dst: VReg,
        incoming: Vec<(VReg, BlockId)>,
    },

    /// `dst = global_ref(name)` — reference to a global (string literal, etc.)
    GlobalRef { dst: VReg, name: String },
}

impl Instruction {
    /// Returns the destination register, if any.
    pub fn dst(&self) -> Option<VReg> {
        match self {
            Instruction::Constant { dst, .. }
            | Instruction::BinOp { dst, .. }
            | Instruction::UnaryOp { dst, .. }
            | Instruction::Cmp { dst, .. }
            | Instruction::Alloca { dst, .. }
            | Instruction::Load { dst, .. }
            | Instruction::Phi { dst, .. }
            | Instruction::GlobalRef { dst, .. } => Some(*dst),
            Instruction::Call { dst, .. } => *dst,
            Instruction::Store { .. } => None,
        }
    }

    /// Returns all VRegs used (read) by this instruction.
    pub fn uses(&self) -> Vec<VReg> {
        match self {
            Instruction::Constant { .. } => vec![],
            Instruction::BinOp { lhs, rhs, .. } => vec![*lhs, *rhs],
            Instruction::UnaryOp { operand, .. } => vec![*operand],
            Instruction::Cmp { lhs, rhs, .. } => vec![*lhs, *rhs],
            Instruction::Alloca { .. } => vec![],
            Instruction::Load { addr, .. } => vec![*addr],
            Instruction::Store { addr, value, .. } => vec![*addr, *value],
            Instruction::Call { args, .. } => args.clone(),
            Instruction::Phi { incoming, .. } => incoming.iter().map(|(v, _)| *v).collect(),
            Instruction::GlobalRef { .. } => vec![],
        }
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instruction::Constant { dst, value } => {
                write!(f, "  {} = const {}", dst, value)
            }
            Instruction::BinOp { dst, op, lhs, rhs } => {
                write!(f, "  {} = {} {}, {}", dst, op, lhs, rhs)
            }
            Instruction::UnaryOp { dst, op, operand } => {
                write!(f, "  {} = {} {}", dst, op, operand)
            }
            Instruction::Cmp { dst, op, lhs, rhs } => {
                write!(f, "  {} = cmp {} {}, {}", dst, op, lhs, rhs)
            }
            Instruction::Alloca { dst, size } => {
                write!(f, "  {} = alloca {}", dst, size)
            }
            Instruction::Load { dst, addr } => {
                write!(f, "  {} = load {}", dst, addr)
            }
            Instruction::Store { addr, value } => {
                write!(f, "  store {}, {}", addr, value)
            }
            Instruction::Call { dst, func, args } => {
                let args_str: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                if let Some(d) = dst {
                    write!(f, "  {} = call {}({})", d, func, args_str.join(", "))
                } else {
                    write!(f, "  call {}({})", func, args_str.join(", "))
                }
            }
            Instruction::Phi { dst, incoming } => {
                let entries: Vec<String> = incoming
                    .iter()
                    .map(|(v, b)| format!("[{}, {}]", v, b))
                    .collect();
                write!(f, "  {} = phi {}", dst, entries.join(", "))
            }
            Instruction::GlobalRef { dst, name } => {
                write!(f, "  {} = global_ref @{}", dst, name)
            }
        }
    }
}

/// A terminator instruction — the last instruction in a basic block.
#[derive(Debug, Clone)]
pub enum Terminator {
    /// `ret value` — return from function
    Ret(Option<VReg>),

    /// `br target` — unconditional branch
    Branch(BlockId),

    /// `condbr cond, true_target, false_target` — conditional branch
    CondBranch {
        cond: VReg,
        true_bb: BlockId,
        false_bb: BlockId,
    },
}

impl Terminator {
    /// Returns all VRegs used by this terminator.
    pub fn uses(&self) -> Vec<VReg> {
        match self {
            Terminator::Ret(Some(v)) => vec![*v],
            Terminator::Ret(None) => vec![],
            Terminator::Branch(_) => vec![],
            Terminator::CondBranch { cond, .. } => vec![*cond],
        }
    }

    /// Returns all successor block IDs.
    pub fn successors(&self) -> Vec<BlockId> {
        match self {
            Terminator::Ret(_) => vec![],
            Terminator::Branch(target) => vec![*target],
            Terminator::CondBranch {
                true_bb, false_bb, ..
            } => vec![*true_bb, *false_bb],
        }
    }
}

impl fmt::Display for Terminator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Terminator::Ret(Some(v)) => write!(f, "  ret {}", v),
            Terminator::Ret(None) => write!(f, "  ret void"),
            Terminator::Branch(target) => write!(f, "  br {}", target),
            Terminator::CondBranch {
                cond,
                true_bb,
                false_bb,
            } => {
                write!(f, "  condbr {}, {}, {}", cond, true_bb, false_bb)
            }
        }
    }
}

/// A basic block: a sequence of instructions followed by a terminator.
#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BlockId,
    pub instructions: Vec<Instruction>,
    pub terminator: Option<Terminator>,
}

impl BasicBlock {
    pub fn new(id: BlockId) -> Self {
        Self {
            id,
            instructions: Vec::new(),
            terminator: None,
        }
    }
}

impl fmt::Display for BasicBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{}:", self.id)?;
        for inst in &self.instructions {
            writeln!(f, "{}", inst)?;
        }
        if let Some(ref term) = self.terminator {
            writeln!(f, "{}", term)?;
        }
        Ok(())
    }
}

/// A global data item (e.g., string literal).
#[derive(Debug, Clone)]
pub struct GlobalData {
    pub name: String,
    pub data: Vec<u8>,
}

/// A function in the IR.
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<VReg>,
    pub blocks: Vec<BasicBlock>,
    pub next_vreg: u32,
    pub next_block: u32,
}

impl Function {
    pub fn new(name: String) -> Self {
        Self {
            name,
            params: Vec::new(),
            blocks: Vec::new(),
            next_vreg: 0,
            next_block: 0,
        }
    }

    /// Get the entry block ID (always bb0).
    pub fn entry_block(&self) -> BlockId {
        BlockId(0)
    }

    /// Find a basic block by ID.
    pub fn block(&self, id: BlockId) -> Option<&BasicBlock> {
        self.blocks.iter().find(|b| b.id == id)
    }

    /// Find a basic block by ID (mutable).
    pub fn block_mut(&mut self, id: BlockId) -> Option<&mut BasicBlock> {
        self.blocks.iter_mut().find(|b| b.id == id)
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let params: Vec<String> = self.params.iter().map(|p| p.to_string()).collect();
        writeln!(f, "fn {}({}):", self.name, params.join(", "))?;
        for block in &self.blocks {
            write!(f, "{}", block)?;
        }
        Ok(())
    }
}

/// A complete IR module (compilation unit).
#[derive(Debug, Clone)]
pub struct Module {
    pub functions: Vec<Function>,
    pub globals: Vec<GlobalData>,
}

impl Module {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            globals: Vec::new(),
        }
    }

    /// Add a string literal global, returns the name.
    pub fn add_string_literal(&mut self, data: &str) -> String {
        let name = format!(".str.{}", self.globals.len());
        let mut bytes = data.as_bytes().to_vec();
        bytes.push(0); // null terminator
        self.globals.push(GlobalData {
            name: name.clone(),
            data: bytes,
        });
        name
    }
}

impl Default for Module {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for global in &self.globals {
            writeln!(
                f,
                "@{} = {:?}",
                global.name,
                String::from_utf8_lossy(&global.data)
            )?;
        }
        if !self.globals.is_empty() {
            writeln!(f)?;
        }
        for func in &self.functions {
            write!(f, "{}", func)?;
            writeln!(f)?;
        }
        Ok(())
    }
}
