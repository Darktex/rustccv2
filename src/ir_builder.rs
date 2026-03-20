//! Lowers AST to SSA IR.

use std::collections::HashMap;

use crate::ast::*;
use crate::ir::*;

pub struct IrBuilder {
    functions: Vec<IrFunction>,
    string_literals: Vec<String>,
}

struct FunctionBuilder {
    name: String,
    params: Vec<VReg>,
    blocks: Vec<BasicBlock>,
    current_block: Label,
    next_vreg: VReg,
    next_label: Label,
    /// Map variable name -> stack slot vreg
    variables: Vec<HashMap<String, VReg>>,
    /// For break/continue: (break_label, continue_label)
    loop_stack: Vec<(Label, Label)>,
}

impl FunctionBuilder {
    fn new(name: String) -> Self {
        Self {
            name,
            params: Vec::new(),
            blocks: vec![BasicBlock {
                label: 0,
                instructions: Vec::new(),
                terminator: Terminator::None,
            }],
            current_block: 0,
            next_vreg: 0,
            next_label: 1,
            variables: vec![HashMap::new()],
            loop_stack: Vec::new(),
        }
    }

    fn new_vreg(&mut self) -> VReg {
        let v = self.next_vreg;
        self.next_vreg += 1;
        v
    }

    fn new_label(&mut self) -> Label {
        let l = self.next_label;
        self.next_label += 1;
        l
    }

    fn emit(&mut self, inst: Instruction) {
        let block = &mut self.blocks[self.current_block as usize];
        block.instructions.push(inst);
    }

    fn terminate(&mut self, term: Terminator) {
        let block = &mut self.blocks[self.current_block as usize];
        if matches!(block.terminator, Terminator::None) {
            block.terminator = term;
        }
    }

    fn is_terminated(&self) -> bool {
        !matches!(
            self.blocks[self.current_block as usize].terminator,
            Terminator::None
        )
    }

    fn start_block(&mut self, label: Label) {
        self.blocks.push(BasicBlock {
            label,
            instructions: Vec::new(),
            terminator: Terminator::None,
        });
        self.current_block = label;
    }

    fn push_scope(&mut self) {
        self.variables.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        self.variables.pop();
    }

    fn declare_var(&mut self, name: &str) -> VReg {
        let slot = self.new_vreg();
        self.emit(Instruction::Alloca(slot));
        if let Some(scope) = self.variables.last_mut() {
            scope.insert(name.to_string(), slot);
        }
        slot
    }

    fn lookup_var(&self, name: &str) -> Result<VReg, String> {
        for scope in self.variables.iter().rev() {
            if let Some(&slot) = scope.get(name) {
                return Ok(slot);
            }
        }
        Err(format!("Undefined variable: {name}"))
    }

    fn build(self) -> IrFunction {
        IrFunction {
            name: self.name,
            params: self.params,
            blocks: self.blocks,
            next_vreg: self.next_vreg,
            next_label: self.next_label,
            stack_size: 0, // computed later
        }
    }
}

impl IrBuilder {
    pub fn new() -> Self {
        Self {
            functions: Vec::new(),
            string_literals: Vec::new(),
        }
    }

    pub fn lower(mut self, program: &Program) -> Result<IrProgram, String> {
        for decl in &program.declarations {
            match decl {
                Declaration::Function(func) => {
                    if let Some(ref body) = func.body {
                        let ir_func = self.lower_function(func, body)?;
                        self.functions.push(ir_func);
                    }
                }
                Declaration::GlobalVar(_) => {
                    // Global variables: TODO for later milestones
                }
            }
        }
        Ok(IrProgram {
            functions: self.functions,
            string_literals: self.string_literals,
        })
    }

    fn add_string(&mut self, s: &str) -> StringId {
        // Check if string already exists
        for (i, existing) in self.string_literals.iter().enumerate() {
            if existing == s {
                return i;
            }
        }
        let id = self.string_literals.len();
        self.string_literals.push(s.to_string());
        id
    }

    fn lower_function(&mut self, func: &FunctionDecl, body: &Block) -> Result<IrFunction, String> {
        let mut fb = FunctionBuilder::new(func.name.clone());

        // Handle parameters — store them in stack slots
        for param in &func.params {
            let param_vreg = fb.new_vreg();
            fb.params.push(param_vreg);
            let slot = fb.declare_var(&param.name);
            fb.emit(Instruction::Store(slot, param_vreg));
        }

        self.lower_block(&mut fb, body)?;

        // Ensure function has a terminator
        if !fb.is_terminated() {
            if func.return_type == Type::Void {
                fb.terminate(Terminator::ReturnVoid);
            } else {
                // Default return 0
                let zero = fb.new_vreg();
                fb.emit(Instruction::LoadImm(zero, 0));
                fb.terminate(Terminator::Return(zero));
            }
        }

        Ok(fb.build())
    }

    fn lower_block(&mut self, fb: &mut FunctionBuilder, block: &Block) -> Result<(), String> {
        fb.push_scope();
        for stmt in block {
            if fb.is_terminated() {
                break;
            }
            self.lower_stmt(fb, stmt)?;
        }
        fb.pop_scope();
        Ok(())
    }

    fn lower_stmt(&mut self, fb: &mut FunctionBuilder, stmt: &Stmt) -> Result<(), String> {
        match stmt {
            Stmt::Return(Some(expr)) => {
                let val = self.lower_expr(fb, expr)?;
                fb.terminate(Terminator::Return(val));
            }
            Stmt::Return(None) => {
                fb.terminate(Terminator::ReturnVoid);
            }
            Stmt::Expr(expr) => {
                self.lower_expr(fb, expr)?;
            }
            Stmt::VarDecl(decl) => {
                let slot = fb.declare_var(&decl.name);
                if let Some(ref init) = decl.init {
                    let val = self.lower_expr(fb, init)?;
                    fb.emit(Instruction::Store(slot, val));
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.lower_expr(fb, condition)?;
                let then_label = fb.new_label();
                let else_label = fb.new_label();
                let end_label = fb.new_label();

                if else_branch.is_some() {
                    fb.terminate(Terminator::Branch(cond, then_label, else_label));
                } else {
                    fb.terminate(Terminator::Branch(cond, then_label, end_label));
                }

                // Then block
                fb.start_block(then_label);
                self.lower_stmt(fb, then_branch)?;
                if !fb.is_terminated() {
                    fb.terminate(Terminator::Jump(end_label));
                }

                // Else block
                if let Some(else_stmt) = else_branch {
                    fb.start_block(else_label);
                    self.lower_stmt(fb, else_stmt)?;
                    if !fb.is_terminated() {
                        fb.terminate(Terminator::Jump(end_label));
                    }
                }

                // Continue
                fb.start_block(end_label);
            }
            Stmt::While { condition, body } => {
                let cond_label = fb.new_label();
                let body_label = fb.new_label();
                let end_label = fb.new_label();

                fb.terminate(Terminator::Jump(cond_label));

                fb.start_block(cond_label);
                let cond = self.lower_expr(fb, condition)?;
                fb.terminate(Terminator::Branch(cond, body_label, end_label));

                fb.start_block(body_label);
                fb.loop_stack.push((end_label, cond_label));
                self.lower_stmt(fb, body)?;
                fb.loop_stack.pop();
                if !fb.is_terminated() {
                    fb.terminate(Terminator::Jump(cond_label));
                }

                fb.start_block(end_label);
            }
            Stmt::DoWhile { body, condition } => {
                let body_label = fb.new_label();
                let cond_label = fb.new_label();
                let end_label = fb.new_label();

                fb.terminate(Terminator::Jump(body_label));

                fb.start_block(body_label);
                fb.loop_stack.push((end_label, cond_label));
                self.lower_stmt(fb, body)?;
                fb.loop_stack.pop();
                if !fb.is_terminated() {
                    fb.terminate(Terminator::Jump(cond_label));
                }

                fb.start_block(cond_label);
                let cond = self.lower_expr(fb, condition)?;
                fb.terminate(Terminator::Branch(cond, body_label, end_label));

                fb.start_block(end_label);
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
            } => {
                fb.push_scope();

                if let Some(init_stmt) = init {
                    self.lower_stmt(fb, init_stmt)?;
                }

                let cond_label = fb.new_label();
                let body_label = fb.new_label();
                let update_label = fb.new_label();
                let end_label = fb.new_label();

                fb.terminate(Terminator::Jump(cond_label));

                fb.start_block(cond_label);
                if let Some(cond_expr) = condition {
                    let cond = self.lower_expr(fb, cond_expr)?;
                    fb.terminate(Terminator::Branch(cond, body_label, end_label));
                } else {
                    fb.terminate(Terminator::Jump(body_label));
                }

                fb.start_block(body_label);
                fb.loop_stack.push((end_label, update_label));
                self.lower_stmt(fb, body)?;
                fb.loop_stack.pop();
                if !fb.is_terminated() {
                    fb.terminate(Terminator::Jump(update_label));
                }

                fb.start_block(update_label);
                if let Some(update_expr) = update {
                    self.lower_expr(fb, update_expr)?;
                }
                fb.terminate(Terminator::Jump(cond_label));

                fb.start_block(end_label);
                fb.pop_scope();
            }
            Stmt::Block(block) => {
                self.lower_block(fb, block)?;
            }
            Stmt::Break => {
                if let Some(&(break_label, _)) = fb.loop_stack.last() {
                    fb.terminate(Terminator::Jump(break_label));
                } else {
                    return Err("break outside of loop".to_string());
                }
            }
            Stmt::Continue => {
                if let Some(&(_, continue_label)) = fb.loop_stack.last() {
                    fb.terminate(Terminator::Jump(continue_label));
                } else {
                    return Err("continue outside of loop".to_string());
                }
            }
            Stmt::Empty => {}
        }
        Ok(())
    }

    fn lower_expr(&mut self, fb: &mut FunctionBuilder, expr: &Expr) -> Result<VReg, String> {
        match expr {
            Expr::IntLiteral(n) => {
                let dst = fb.new_vreg();
                fb.emit(Instruction::LoadImm(dst, *n));
                Ok(dst)
            }
            Expr::StringLiteral(s) => {
                let id = self.add_string(s);
                let dst = fb.new_vreg();
                fb.emit(Instruction::LoadStringAddr(dst, id));
                Ok(dst)
            }
            Expr::Var(name) => {
                let slot = fb.lookup_var(name)?;
                let dst = fb.new_vreg();
                fb.emit(Instruction::Load(dst, slot));
                Ok(dst)
            }
            Expr::Assign(name, rhs) => {
                let val = self.lower_expr(fb, rhs)?;
                let slot = fb.lookup_var(name)?;
                fb.emit(Instruction::Store(slot, val));
                Ok(val)
            }
            Expr::CompoundAssign(op, name, rhs) => {
                let slot = fb.lookup_var(name)?;
                let old_val = fb.new_vreg();
                fb.emit(Instruction::Load(old_val, slot));
                let rhs_val = self.lower_expr(fb, rhs)?;
                let ir_op = match op {
                    CompoundOp::AddAssign => IrBinOp::Add,
                    CompoundOp::SubAssign => IrBinOp::Sub,
                    CompoundOp::MulAssign => IrBinOp::Mul,
                    CompoundOp::DivAssign => IrBinOp::Div,
                    CompoundOp::ModAssign => IrBinOp::Mod,
                };
                let result = fb.new_vreg();
                fb.emit(Instruction::BinOp(result, ir_op, old_val, rhs_val));
                fb.emit(Instruction::Store(slot, result));
                Ok(result)
            }
            Expr::BinaryOp(op, lhs, rhs) => {
                // Short-circuit for && and ||
                match op {
                    BinOp::LogicalAnd => return self.lower_logical_and(fb, lhs, rhs),
                    BinOp::LogicalOr => return self.lower_logical_or(fb, lhs, rhs),
                    _ => {}
                }

                let l = self.lower_expr(fb, lhs)?;
                let r = self.lower_expr(fb, rhs)?;
                let ir_op = match op {
                    BinOp::Add => IrBinOp::Add,
                    BinOp::Sub => IrBinOp::Sub,
                    BinOp::Mul => IrBinOp::Mul,
                    BinOp::Div => IrBinOp::Div,
                    BinOp::Mod => IrBinOp::Mod,
                    BinOp::Equal => IrBinOp::Equal,
                    BinOp::NotEqual => IrBinOp::NotEqual,
                    BinOp::Less => IrBinOp::Less,
                    BinOp::Greater => IrBinOp::Greater,
                    BinOp::LessEqual => IrBinOp::LessEqual,
                    BinOp::GreaterEqual => IrBinOp::GreaterEqual,
                    BinOp::BitwiseAnd => IrBinOp::BitwiseAnd,
                    BinOp::BitwiseOr => IrBinOp::BitwiseOr,
                    BinOp::BitwiseXor => IrBinOp::BitwiseXor,
                    BinOp::ShiftLeft => IrBinOp::ShiftLeft,
                    BinOp::ShiftRight => IrBinOp::ShiftRight,
                    BinOp::LogicalAnd | BinOp::LogicalOr => unreachable!(),
                };
                let dst = fb.new_vreg();
                fb.emit(Instruction::BinOp(dst, ir_op, l, r));
                Ok(dst)
            }
            Expr::UnaryOp(op, inner) => {
                let src = self.lower_expr(fb, inner)?;
                let ir_op = match op {
                    UnaryOp::Negate => IrUnaryOp::Negate,
                    UnaryOp::BitwiseNot => IrUnaryOp::BitwiseNot,
                    UnaryOp::LogicalNot => IrUnaryOp::LogicalNot,
                };
                let dst = fb.new_vreg();
                fb.emit(Instruction::UnaryOp(dst, ir_op, src));
                Ok(dst)
            }
            Expr::Call(name, args) => {
                let mut arg_regs = Vec::new();
                for arg in args {
                    arg_regs.push(self.lower_expr(fb, arg)?);
                }
                let dst = fb.new_vreg();
                fb.emit(Instruction::Call(dst, name.clone(), arg_regs));
                Ok(dst)
            }
            Expr::Ternary(cond, then_expr, else_expr) => {
                let cond_val = self.lower_expr(fb, cond)?;
                let then_label = fb.new_label();
                let else_label = fb.new_label();
                let end_label = fb.new_label();

                // Allocate result slot
                let result_slot = fb.new_vreg();
                fb.emit(Instruction::Alloca(result_slot));

                fb.terminate(Terminator::Branch(cond_val, then_label, else_label));

                fb.start_block(then_label);
                let then_val = self.lower_expr(fb, then_expr)?;
                fb.emit(Instruction::Store(result_slot, then_val));
                fb.terminate(Terminator::Jump(end_label));

                fb.start_block(else_label);
                let else_val = self.lower_expr(fb, else_expr)?;
                fb.emit(Instruction::Store(result_slot, else_val));
                fb.terminate(Terminator::Jump(end_label));

                fb.start_block(end_label);
                let result = fb.new_vreg();
                fb.emit(Instruction::Load(result, result_slot));
                Ok(result)
            }
            Expr::PreIncrement(name) => {
                let slot = fb.lookup_var(name)?;
                let old = fb.new_vreg();
                fb.emit(Instruction::Load(old, slot));
                let one = fb.new_vreg();
                fb.emit(Instruction::LoadImm(one, 1));
                let new_val = fb.new_vreg();
                fb.emit(Instruction::BinOp(new_val, IrBinOp::Add, old, one));
                fb.emit(Instruction::Store(slot, new_val));
                Ok(new_val)
            }
            Expr::PreDecrement(name) => {
                let slot = fb.lookup_var(name)?;
                let old = fb.new_vreg();
                fb.emit(Instruction::Load(old, slot));
                let one = fb.new_vreg();
                fb.emit(Instruction::LoadImm(one, 1));
                let new_val = fb.new_vreg();
                fb.emit(Instruction::BinOp(new_val, IrBinOp::Sub, old, one));
                fb.emit(Instruction::Store(slot, new_val));
                Ok(new_val)
            }
            Expr::PostIncrement(name) => {
                let slot = fb.lookup_var(name)?;
                let old = fb.new_vreg();
                fb.emit(Instruction::Load(old, slot));
                let one = fb.new_vreg();
                fb.emit(Instruction::LoadImm(one, 1));
                let new_val = fb.new_vreg();
                fb.emit(Instruction::BinOp(new_val, IrBinOp::Add, old, one));
                fb.emit(Instruction::Store(slot, new_val));
                Ok(old) // post-increment returns old value
            }
            Expr::PostDecrement(name) => {
                let slot = fb.lookup_var(name)?;
                let old = fb.new_vreg();
                fb.emit(Instruction::Load(old, slot));
                let one = fb.new_vreg();
                fb.emit(Instruction::LoadImm(one, 1));
                let new_val = fb.new_vreg();
                fb.emit(Instruction::BinOp(new_val, IrBinOp::Sub, old, one));
                fb.emit(Instruction::Store(slot, new_val));
                Ok(old)
            }
        }
    }

    fn lower_logical_and(
        &mut self,
        fb: &mut FunctionBuilder,
        lhs: &Expr,
        rhs: &Expr,
    ) -> Result<VReg, String> {
        let result_slot = fb.new_vreg();
        fb.emit(Instruction::Alloca(result_slot));

        let lhs_val = self.lower_expr(fb, lhs)?;
        let rhs_label = fb.new_label();
        let false_label = fb.new_label();
        let end_label = fb.new_label();

        fb.terminate(Terminator::Branch(lhs_val, rhs_label, false_label));

        fb.start_block(rhs_label);
        let rhs_val = self.lower_expr(fb, rhs)?;
        // Normalize to 0 or 1
        let zero = fb.new_vreg();
        fb.emit(Instruction::LoadImm(zero, 0));
        let cmp = fb.new_vreg();
        fb.emit(Instruction::BinOp(cmp, IrBinOp::NotEqual, rhs_val, zero));
        fb.emit(Instruction::Store(result_slot, cmp));
        fb.terminate(Terminator::Jump(end_label));

        fb.start_block(false_label);
        let zero2 = fb.new_vreg();
        fb.emit(Instruction::LoadImm(zero2, 0));
        fb.emit(Instruction::Store(result_slot, zero2));
        fb.terminate(Terminator::Jump(end_label));

        fb.start_block(end_label);
        let result = fb.new_vreg();
        fb.emit(Instruction::Load(result, result_slot));
        Ok(result)
    }

    fn lower_logical_or(
        &mut self,
        fb: &mut FunctionBuilder,
        lhs: &Expr,
        rhs: &Expr,
    ) -> Result<VReg, String> {
        let result_slot = fb.new_vreg();
        fb.emit(Instruction::Alloca(result_slot));

        let lhs_val = self.lower_expr(fb, lhs)?;
        let true_label = fb.new_label();
        let rhs_label = fb.new_label();
        let end_label = fb.new_label();

        fb.terminate(Terminator::Branch(lhs_val, true_label, rhs_label));

        fb.start_block(true_label);
        let one = fb.new_vreg();
        fb.emit(Instruction::LoadImm(one, 1));
        fb.emit(Instruction::Store(result_slot, one));
        fb.terminate(Terminator::Jump(end_label));

        fb.start_block(rhs_label);
        let rhs_val = self.lower_expr(fb, rhs)?;
        let zero = fb.new_vreg();
        fb.emit(Instruction::LoadImm(zero, 0));
        let cmp = fb.new_vreg();
        fb.emit(Instruction::BinOp(cmp, IrBinOp::NotEqual, rhs_val, zero));
        fb.emit(Instruction::Store(result_slot, cmp));
        fb.terminate(Terminator::Jump(end_label));

        fb.start_block(end_label);
        let result = fb.new_vreg();
        fb.emit(Instruction::Load(result, result_slot));
        Ok(result)
    }
}

impl Default for IrBuilder {
    fn default() -> Self {
        Self::new()
    }
}
