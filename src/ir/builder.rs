//! IR Builder — constructs SSA IR from higher-level representations.
//!
//! The builder provides a convenient API for creating IR functions,
//! basic blocks, and instructions while maintaining SSA form.

use super::{
    BasicBlock, BinOp, BlockId, CmpOp, Function, Instruction, Module, Terminator, UnaryOp, VReg,
};

/// Builder for constructing IR functions incrementally.
pub struct IrBuilder {
    pub module: Module,
    current_func: Option<usize>,
    current_block: Option<BlockId>,
}

impl IrBuilder {
    /// Create a new IR builder.
    pub fn new() -> Self {
        Self {
            module: Module::new(),
            current_func: None,
            current_block: None,
        }
    }

    /// Start building a new function.
    pub fn begin_function(&mut self, name: &str) -> usize {
        let func = Function::new(name.to_string());
        self.module.functions.push(func);
        let idx = self.module.functions.len() - 1;
        self.current_func = Some(idx);
        self.current_block = None;
        idx
    }

    /// Add a parameter to the current function, returns the VReg for it.
    pub fn add_param(&mut self) -> VReg {
        let func = self.current_function_mut();
        let vreg = VReg(func.next_vreg);
        func.next_vreg += 1;
        func.params.push(vreg);
        vreg
    }

    /// Create a new basic block in the current function.
    pub fn create_block(&mut self) -> BlockId {
        let func = self.current_function_mut();
        let id = BlockId(func.next_block);
        func.next_block += 1;
        func.blocks.push(BasicBlock::new(id));
        id
    }

    /// Set the current insertion point to the given block.
    pub fn switch_to_block(&mut self, block: BlockId) {
        self.current_block = Some(block);
    }

    /// Get the current block ID.
    pub fn current_block(&self) -> BlockId {
        self.current_block.expect("No current block set")
    }

    /// Allocate a fresh virtual register.
    fn fresh_vreg(&mut self) -> VReg {
        let func = self.current_function_mut();
        let vreg = VReg(func.next_vreg);
        func.next_vreg += 1;
        vreg
    }

    /// Emit an integer constant.
    pub fn build_constant(&mut self, value: i64) -> VReg {
        let dst = self.fresh_vreg();
        self.push_instruction(Instruction::Constant { dst, value });
        dst
    }

    /// Emit a binary operation.
    pub fn build_binop(&mut self, op: BinOp, lhs: VReg, rhs: VReg) -> VReg {
        let dst = self.fresh_vreg();
        self.push_instruction(Instruction::BinOp { dst, op, lhs, rhs });
        dst
    }

    /// Emit a unary operation.
    pub fn build_unaryop(&mut self, op: UnaryOp, operand: VReg) -> VReg {
        let dst = self.fresh_vreg();
        self.push_instruction(Instruction::UnaryOp { dst, op, operand });
        dst
    }

    /// Emit a comparison.
    pub fn build_cmp(&mut self, op: CmpOp, lhs: VReg, rhs: VReg) -> VReg {
        let dst = self.fresh_vreg();
        self.push_instruction(Instruction::Cmp { dst, op, lhs, rhs });
        dst
    }

    /// Emit a stack allocation.
    pub fn build_alloca(&mut self, size: u32) -> VReg {
        let dst = self.fresh_vreg();
        self.push_instruction(Instruction::Alloca { dst, size });
        dst
    }

    /// Emit a memory load.
    pub fn build_load(&mut self, addr: VReg) -> VReg {
        let dst = self.fresh_vreg();
        self.push_instruction(Instruction::Load { dst, addr });
        dst
    }

    /// Emit a memory store.
    pub fn build_store(&mut self, addr: VReg, value: VReg) {
        self.push_instruction(Instruction::Store { addr, value });
    }

    /// Emit a function call with a return value.
    pub fn build_call(&mut self, func_name: &str, args: Vec<VReg>) -> VReg {
        let dst = self.fresh_vreg();
        self.push_instruction(Instruction::Call {
            dst: Some(dst),
            func: func_name.to_string(),
            args,
        });
        dst
    }

    /// Emit a void function call (no return value).
    pub fn build_call_void(&mut self, func_name: &str, args: Vec<VReg>) {
        self.push_instruction(Instruction::Call {
            dst: None,
            func: func_name.to_string(),
            args,
        });
    }

    /// Emit a phi node.
    pub fn build_phi(&mut self, incoming: Vec<(VReg, BlockId)>) -> VReg {
        let dst = self.fresh_vreg();
        self.push_instruction(Instruction::Phi { dst, incoming });
        dst
    }

    /// Emit a global reference (e.g., for string literals).
    pub fn build_global_ref(&mut self, name: &str) -> VReg {
        let dst = self.fresh_vreg();
        self.push_instruction(Instruction::GlobalRef {
            dst,
            name: name.to_string(),
        });
        dst
    }

    /// Emit a return terminator.
    pub fn build_ret(&mut self, value: Option<VReg>) {
        self.set_terminator(Terminator::Ret(value));
    }

    /// Emit an unconditional branch.
    pub fn build_branch(&mut self, target: BlockId) {
        self.set_terminator(Terminator::Branch(target));
    }

    /// Emit a conditional branch.
    pub fn build_cond_branch(&mut self, cond: VReg, true_bb: BlockId, false_bb: BlockId) {
        self.set_terminator(Terminator::CondBranch {
            cond,
            true_bb,
            false_bb,
        });
    }

    /// Add a string literal to the module's globals.
    pub fn add_string_literal(&mut self, data: &str) -> String {
        self.module.add_string_literal(data)
    }

    /// Finish building and return the module.
    pub fn finish(self) -> Module {
        self.module
    }

    // --- Internal helpers ---

    fn current_function_mut(&mut self) -> &mut Function {
        let idx = self.current_func.expect("No current function");
        &mut self.module.functions[idx]
    }

    fn push_instruction(&mut self, inst: Instruction) {
        let block_id = self.current_block.expect("No current block");
        let func = self.current_function_mut();
        let block = func
            .block_mut(block_id)
            .expect("Current block not found in function");
        block.instructions.push(inst);
    }

    fn set_terminator(&mut self, term: Terminator) {
        let block_id = self.current_block.expect("No current block");
        let func = self.current_function_mut();
        let block = func
            .block_mut(block_id)
            .expect("Current block not found in function");
        block.terminator = Some(term);
    }
}

impl Default for IrBuilder {
    fn default() -> Self {
        Self::new()
    }
}
