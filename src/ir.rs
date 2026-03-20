/// SSA-based Intermediate Representation for the C compiler.
use crate::parser::{
    BinOp, Block, Declaration, Expr, FunctionDef, Program, Stmt, TypeSpec, UnaryOp,
};
use std::collections::HashMap;

pub type VReg = u32;
pub type Label = u32;

#[derive(Debug, Clone)]
pub struct Module {
    pub functions: Vec<Function>,
    pub string_literals: Vec<(String, String)>, // (label, value)
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub blocks: Vec<BasicBlock>,
    pub num_vregs: u32,
    pub locals: HashMap<String, VReg>,
    pub is_defined: bool,
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub label: Label,
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Instruction {
    /// dest = constant integer
    LoadImm { dest: VReg, value: i64 },
    /// dest = left op right
    BinOp {
        dest: VReg,
        op: IrBinOp,
        left: Operand,
        right: Operand,
    },
    /// dest = op operand
    UnaryOp {
        dest: VReg,
        op: IrUnaryOp,
        operand: Operand,
    },
    /// dest = cmp(left, right)
    Cmp {
        dest: VReg,
        op: CmpOp,
        left: Operand,
        right: Operand,
    },
    /// Copy value from one vreg to another
    Copy { dest: VReg, src: Operand },
    /// dest = call func(args...)
    Call {
        dest: Option<VReg>,
        func: String,
        args: Vec<Operand>,
    },
    /// Load address of string literal
    LoadStringAddr { dest: VReg, label: String },
    /// Alloca - allocate stack space for a local variable
    Alloca { dest: VReg, size: u32 },
    /// Store value to memory at address
    Store { addr: VReg, value: Operand },
    /// Load value from memory at address
    Load { dest: VReg, addr: VReg },
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Terminator {
    /// Return from function
    Return(Option<Operand>),
    /// Unconditional jump
    Jump(Label),
    /// Conditional branch
    Branch {
        condition: Operand,
        true_label: Label,
        false_label: Label,
    },
    /// Placeholder - should not appear in final IR
    None,
}

#[derive(Debug, Clone)]
pub enum Operand {
    VReg(VReg),
    Immediate(i64),
}

#[derive(Debug, Clone, Copy)]
pub enum IrBinOp {
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

#[derive(Debug, Clone, Copy)]
pub enum IrUnaryOp {
    Neg,
    Not,
    BitwiseNot,
}

#[derive(Debug, Clone, Copy)]
pub enum CmpOp {
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

// --------------- IR Builder ---------------

struct IrBuilder {
    functions: Vec<Function>,
    string_literals: Vec<(String, String)>,
    string_counter: u32,
}

struct FunctionBuilder {
    name: String,
    params: Vec<String>,
    blocks: Vec<BasicBlock>,
    current_block: Vec<Instruction>,
    current_label: Label,
    next_vreg: u32,
    next_label: Label,
    locals: HashMap<String, VReg>,
    // For break/continue
    break_label: Option<Label>,
    continue_label: Option<Label>,
}

impl FunctionBuilder {
    fn new(name: String, params: Vec<String>) -> Self {
        FunctionBuilder {
            name,
            params,
            blocks: Vec::new(),
            current_block: Vec::new(),
            current_label: 0,
            next_vreg: 0,
            next_label: 1,
            locals: HashMap::new(),
            break_label: None,
            continue_label: None,
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
        self.current_block.push(inst);
    }

    fn terminate(&mut self, term: Terminator) {
        let block = BasicBlock {
            label: self.current_label,
            instructions: std::mem::take(&mut self.current_block),
            terminator: term,
        };
        self.blocks.push(block);
    }

    fn start_block(&mut self, label: Label) {
        // If the current block hasn't been terminated yet (e.g., an unreachable
        // merge block after if/else where both branches return), emit it now
        // with an implicit return so all labels are defined in the output.
        if !self.blocks.iter().any(|b| b.label == self.current_label) {
            self.terminate(Terminator::Return(Some(Operand::Immediate(0))));
        }
        self.current_label = label;
        self.current_block = Vec::new();
    }

    fn finish(mut self) -> Function {
        // Always finalize the current block — even if it's empty, it may be
        // a branch target that other blocks jump to.  Give it an implicit
        // return so the assembler always finds the label.
        self.terminate(Terminator::Return(Some(Operand::Immediate(0))));
        Function {
            name: self.name,
            params: self.params,
            num_vregs: self.next_vreg,
            locals: self.locals,
            blocks: self.blocks,
            is_defined: true,
        }
    }
}

impl IrBuilder {
    fn new() -> Self {
        IrBuilder {
            functions: Vec::new(),
            string_literals: Vec::new(),
            string_counter: 0,
        }
    }

    fn add_string_literal(&mut self, value: &str) -> String {
        // Check if we already have this string
        for (label, val) in &self.string_literals {
            if val == value {
                return label.clone();
            }
        }
        let label = format!(".LC{}", self.string_counter);
        self.string_counter += 1;
        self.string_literals
            .push((label.clone(), value.to_string()));
        label
    }

    fn lower_program(&mut self, program: &Program) {
        for decl in &program.declarations {
            match decl {
                Declaration::Function(func) => self.lower_function(func),
                Declaration::GlobalVar(_) => {
                    // TODO: global variables
                }
                Declaration::Typedef(_, _) | Declaration::StructDecl(_) => {
                    // Type declarations don't generate IR
                }
            }
        }
    }

    fn lower_function(&mut self, func: &FunctionDef) {
        if func.body.is_none() {
            // Forward declaration - skip
            return;
        }

        let params: Vec<String> = func.params.iter().map(|p| p.name.clone()).collect();
        let mut fb = FunctionBuilder::new(func.name.clone(), params.clone());

        // Create vregs for parameters
        for param_name in &params {
            if !param_name.is_empty() {
                let vreg = fb.new_vreg();
                fb.locals.insert(param_name.clone(), vreg);
            }
        }

        // Lower body
        let body = func.body.as_ref().unwrap();
        self.lower_block(&mut fb, body);

        self.functions.push(fb.finish());
    }

    fn lower_block(&mut self, fb: &mut FunctionBuilder, block: &Block) {
        for stmt in &block.stmts {
            self.lower_stmt(fb, stmt);
        }
    }

    fn lower_stmt(&mut self, fb: &mut FunctionBuilder, stmt: &Stmt) {
        match stmt {
            Stmt::Return(expr) => {
                let operand = expr
                    .as_ref()
                    .map(|e| self.lower_expr(fb, e))
                    .unwrap_or(Operand::Immediate(0));
                fb.terminate(Terminator::Return(Some(operand)));
                // Start a new unreachable block for any following code
                let new_label = fb.new_label();
                fb.start_block(new_label);
            }
            Stmt::Expr(expr) => {
                self.lower_expr(fb, expr);
            }
            Stmt::VarDecl(decl) => {
                let vreg = fb.new_vreg();
                fb.locals.insert(decl.name.clone(), vreg);
                if let Some(init) = &decl.init {
                    let value = self.lower_expr(fb, init);
                    fb.emit(Instruction::Copy {
                        dest: vreg,
                        src: value,
                    });
                } else {
                    fb.emit(Instruction::LoadImm {
                        dest: vreg,
                        value: 0,
                    });
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.lower_expr(fb, condition);
                let then_label = fb.new_label();
                let else_label = fb.new_label();
                let end_label = fb.new_label();

                if else_branch.is_some() {
                    fb.terminate(Terminator::Branch {
                        condition: cond,
                        true_label: then_label,
                        false_label: else_label,
                    });
                } else {
                    fb.terminate(Terminator::Branch {
                        condition: cond,
                        true_label: then_label,
                        false_label: end_label,
                    });
                }

                // Then block
                fb.start_block(then_label);
                self.lower_stmt(fb, then_branch);
                // Only emit jump if the then block didn't already terminate
                if !fb.current_block.is_empty() {
                    fb.terminate(Terminator::Jump(end_label));
                }

                // Else block
                if let Some(else_br) = else_branch {
                    fb.start_block(else_label);
                    self.lower_stmt(fb, else_br);
                    // Only emit jump if the else block didn't already terminate
                    // (e.g., with a return statement)
                    if !fb.current_block.is_empty() {
                        fb.terminate(Terminator::Jump(end_label));
                    }
                }

                fb.start_block(end_label);
            }
            Stmt::While { condition, body } => {
                let cond_label = fb.new_label();
                let body_label = fb.new_label();
                let end_label = fb.new_label();

                let saved_break = fb.break_label;
                let saved_continue = fb.continue_label;
                fb.break_label = Some(end_label);
                fb.continue_label = Some(cond_label);

                fb.terminate(Terminator::Jump(cond_label));

                // Condition block
                fb.start_block(cond_label);
                let cond = self.lower_expr(fb, condition);
                fb.terminate(Terminator::Branch {
                    condition: cond,
                    true_label: body_label,
                    false_label: end_label,
                });

                // Body
                fb.start_block(body_label);
                self.lower_stmt(fb, body);
                fb.terminate(Terminator::Jump(cond_label));

                fb.start_block(end_label);
                fb.break_label = saved_break;
                fb.continue_label = saved_continue;
            }
            Stmt::For {
                init,
                condition,
                update,
                body,
            } => {
                // Init
                if let Some(init) = init {
                    self.lower_stmt(fb, init);
                }

                let cond_label = fb.new_label();
                let body_label = fb.new_label();
                let update_label = fb.new_label();
                let end_label = fb.new_label();

                let saved_break = fb.break_label;
                let saved_continue = fb.continue_label;
                fb.break_label = Some(end_label);
                fb.continue_label = Some(update_label);

                fb.terminate(Terminator::Jump(cond_label));

                // Condition
                fb.start_block(cond_label);
                if let Some(cond) = condition {
                    let cond_val = self.lower_expr(fb, cond);
                    fb.terminate(Terminator::Branch {
                        condition: cond_val,
                        true_label: body_label,
                        false_label: end_label,
                    });
                } else {
                    fb.terminate(Terminator::Jump(body_label));
                }

                // Body
                fb.start_block(body_label);
                self.lower_stmt(fb, body);
                fb.terminate(Terminator::Jump(update_label));

                // Update
                fb.start_block(update_label);
                if let Some(update) = update {
                    self.lower_expr(fb, update);
                }
                fb.terminate(Terminator::Jump(cond_label));

                fb.start_block(end_label);
                fb.break_label = saved_break;
                fb.continue_label = saved_continue;
            }
            Stmt::DoWhile { body, condition } => {
                let body_label = fb.new_label();
                let cond_label = fb.new_label();
                let end_label = fb.new_label();

                let saved_break = fb.break_label;
                let saved_continue = fb.continue_label;
                fb.break_label = Some(end_label);
                fb.continue_label = Some(cond_label);

                fb.terminate(Terminator::Jump(body_label));

                fb.start_block(body_label);
                self.lower_stmt(fb, body);
                fb.terminate(Terminator::Jump(cond_label));

                fb.start_block(cond_label);
                let cond = self.lower_expr(fb, condition);
                fb.terminate(Terminator::Branch {
                    condition: cond,
                    true_label: body_label,
                    false_label: end_label,
                });

                fb.start_block(end_label);
                fb.break_label = saved_break;
                fb.continue_label = saved_continue;
            }
            Stmt::Block(block) => {
                self.lower_block(fb, block);
            }
            Stmt::Break => {
                if let Some(label) = fb.break_label {
                    fb.terminate(Terminator::Jump(label));
                    let new_label = fb.new_label();
                    fb.start_block(new_label);
                }
            }
            Stmt::Continue => {
                if let Some(label) = fb.continue_label {
                    fb.terminate(Terminator::Jump(label));
                    let new_label = fb.new_label();
                    fb.start_block(new_label);
                }
            }
            Stmt::Empty => {}
        }
    }

    fn lower_expr(&mut self, fb: &mut FunctionBuilder, expr: &Expr) -> Operand {
        match expr {
            Expr::IntLiteral(val) => Operand::Immediate(*val),
            Expr::CharLiteral(c) => Operand::Immediate(*c as i64),
            Expr::StringLiteral(s) => {
                let label = self.add_string_literal(s);
                let dest = fb.new_vreg();
                fb.emit(Instruction::LoadStringAddr { dest, label });
                Operand::VReg(dest)
            }
            Expr::Identifier(name) => {
                if let Some(&vreg) = fb.locals.get(name) {
                    Operand::VReg(vreg)
                } else {
                    // Unknown variable - might be a function name used as value
                    Operand::Immediate(0)
                }
            }
            Expr::Binary { op, left, right } => {
                let l = self.lower_expr(fb, left);
                let r = self.lower_expr(fb, right);
                let dest = fb.new_vreg();

                match op {
                    BinOp::Add => fb.emit(Instruction::BinOp {
                        dest,
                        op: IrBinOp::Add,
                        left: l,
                        right: r,
                    }),
                    BinOp::Sub => fb.emit(Instruction::BinOp {
                        dest,
                        op: IrBinOp::Sub,
                        left: l,
                        right: r,
                    }),
                    BinOp::Mul => fb.emit(Instruction::BinOp {
                        dest,
                        op: IrBinOp::Mul,
                        left: l,
                        right: r,
                    }),
                    BinOp::Div => fb.emit(Instruction::BinOp {
                        dest,
                        op: IrBinOp::Div,
                        left: l,
                        right: r,
                    }),
                    BinOp::Mod => fb.emit(Instruction::BinOp {
                        dest,
                        op: IrBinOp::Mod,
                        left: l,
                        right: r,
                    }),
                    BinOp::Equal => fb.emit(Instruction::Cmp {
                        dest,
                        op: CmpOp::Eq,
                        left: l,
                        right: r,
                    }),
                    BinOp::NotEqual => fb.emit(Instruction::Cmp {
                        dest,
                        op: CmpOp::Ne,
                        left: l,
                        right: r,
                    }),
                    BinOp::Less => fb.emit(Instruction::Cmp {
                        dest,
                        op: CmpOp::Lt,
                        left: l,
                        right: r,
                    }),
                    BinOp::LessEqual => fb.emit(Instruction::Cmp {
                        dest,
                        op: CmpOp::Le,
                        left: l,
                        right: r,
                    }),
                    BinOp::Greater => fb.emit(Instruction::Cmp {
                        dest,
                        op: CmpOp::Gt,
                        left: l,
                        right: r,
                    }),
                    BinOp::GreaterEqual => fb.emit(Instruction::Cmp {
                        dest,
                        op: CmpOp::Ge,
                        left: l,
                        right: r,
                    }),
                    BinOp::LogicalAnd => {
                        // Short-circuit: if left is false, result is 0
                        // For simplicity, just compute both and AND
                        fb.emit(Instruction::BinOp {
                            dest,
                            op: IrBinOp::And,
                            left: l,
                            right: r,
                        });
                    }
                    BinOp::LogicalOr => {
                        fb.emit(Instruction::BinOp {
                            dest,
                            op: IrBinOp::Or,
                            left: l,
                            right: r,
                        });
                    }
                    BinOp::BitwiseAnd => fb.emit(Instruction::BinOp {
                        dest,
                        op: IrBinOp::And,
                        left: l,
                        right: r,
                    }),
                    BinOp::BitwiseOr => fb.emit(Instruction::BinOp {
                        dest,
                        op: IrBinOp::Or,
                        left: l,
                        right: r,
                    }),
                    BinOp::BitwiseXor => fb.emit(Instruction::BinOp {
                        dest,
                        op: IrBinOp::Xor,
                        left: l,
                        right: r,
                    }),
                    BinOp::ShiftLeft => fb.emit(Instruction::BinOp {
                        dest,
                        op: IrBinOp::Shl,
                        left: l,
                        right: r,
                    }),
                    BinOp::ShiftRight => fb.emit(Instruction::BinOp {
                        dest,
                        op: IrBinOp::Shr,
                        left: l,
                        right: r,
                    }),
                }
                Operand::VReg(dest)
            }
            Expr::Unary { op, operand } => {
                let val = self.lower_expr(fb, operand);
                let dest = fb.new_vreg();
                let ir_op = match op {
                    UnaryOp::Negate => IrUnaryOp::Neg,
                    UnaryOp::LogicalNot => IrUnaryOp::Not,
                    UnaryOp::BitwiseNot => IrUnaryOp::BitwiseNot,
                };
                fb.emit(Instruction::UnaryOp {
                    dest,
                    op: ir_op,
                    operand: val,
                });
                Operand::VReg(dest)
            }
            Expr::Assign { target, value } => {
                let val = self.lower_expr(fb, value);
                match target.as_ref() {
                    Expr::Identifier(name) => {
                        if let Some(&vreg) = fb.locals.get(name) {
                            fb.emit(Instruction::Copy {
                                dest: vreg,
                                src: val.clone(),
                            });
                            val
                        } else {
                            // Undeclared variable
                            let vreg = fb.new_vreg();
                            fb.locals.insert(name.clone(), vreg);
                            fb.emit(Instruction::Copy {
                                dest: vreg,
                                src: val.clone(),
                            });
                            val
                        }
                    }
                    Expr::Deref(addr_expr) => {
                        let addr = self.lower_expr(fb, addr_expr);
                        if let Operand::VReg(addr_vreg) = addr {
                            fb.emit(Instruction::Store {
                                addr: addr_vreg,
                                value: val.clone(),
                            });
                        }
                        val
                    }
                    _ => val,
                }
            }
            Expr::CompoundAssign { op, target, value } => {
                let current = self.lower_expr(fb, target);
                let rhs = self.lower_expr(fb, value);
                let dest = fb.new_vreg();
                let ir_op = match op {
                    BinOp::Add => IrBinOp::Add,
                    BinOp::Sub => IrBinOp::Sub,
                    BinOp::Mul => IrBinOp::Mul,
                    BinOp::Div => IrBinOp::Div,
                    BinOp::Mod => IrBinOp::Mod,
                    _ => IrBinOp::Add, // fallback
                };
                fb.emit(Instruction::BinOp {
                    dest,
                    op: ir_op,
                    left: current,
                    right: rhs,
                });
                // Store back
                if let Expr::Identifier(name) = target.as_ref() {
                    if let Some(&vreg) = fb.locals.get(name) {
                        fb.emit(Instruction::Copy {
                            dest: vreg,
                            src: Operand::VReg(dest),
                        });
                    }
                }
                Operand::VReg(dest)
            }
            Expr::Call { func, args } => {
                let func_name = match func.as_ref() {
                    Expr::Identifier(name) => name.clone(),
                    _ => "unknown".to_string(),
                };
                let ir_args: Vec<Operand> = args.iter().map(|a| self.lower_expr(fb, a)).collect();
                let dest = fb.new_vreg();
                fb.emit(Instruction::Call {
                    dest: Some(dest),
                    func: func_name,
                    args: ir_args,
                });
                Operand::VReg(dest)
            }
            Expr::PostIncrement(operand) => {
                let val = self.lower_expr(fb, operand);
                let old = fb.new_vreg();
                fb.emit(Instruction::Copy {
                    dest: old,
                    src: val.clone(),
                });
                let new_val = fb.new_vreg();
                fb.emit(Instruction::BinOp {
                    dest: new_val,
                    op: IrBinOp::Add,
                    left: val,
                    right: Operand::Immediate(1),
                });
                if let Expr::Identifier(name) = operand.as_ref() {
                    if let Some(&vreg) = fb.locals.get(name) {
                        fb.emit(Instruction::Copy {
                            dest: vreg,
                            src: Operand::VReg(new_val),
                        });
                    }
                }
                Operand::VReg(old)
            }
            Expr::PostDecrement(operand) => {
                let val = self.lower_expr(fb, operand);
                let old = fb.new_vreg();
                fb.emit(Instruction::Copy {
                    dest: old,
                    src: val.clone(),
                });
                let new_val = fb.new_vreg();
                fb.emit(Instruction::BinOp {
                    dest: new_val,
                    op: IrBinOp::Sub,
                    left: val,
                    right: Operand::Immediate(1),
                });
                if let Expr::Identifier(name) = operand.as_ref() {
                    if let Some(&vreg) = fb.locals.get(name) {
                        fb.emit(Instruction::Copy {
                            dest: vreg,
                            src: Operand::VReg(new_val),
                        });
                    }
                }
                Operand::VReg(old)
            }
            Expr::PreIncrement(operand) => {
                let val = self.lower_expr(fb, operand);
                let new_val = fb.new_vreg();
                fb.emit(Instruction::BinOp {
                    dest: new_val,
                    op: IrBinOp::Add,
                    left: val,
                    right: Operand::Immediate(1),
                });
                if let Expr::Identifier(name) = operand.as_ref() {
                    if let Some(&vreg) = fb.locals.get(name) {
                        fb.emit(Instruction::Copy {
                            dest: vreg,
                            src: Operand::VReg(new_val),
                        });
                    }
                }
                Operand::VReg(new_val)
            }
            Expr::PreDecrement(operand) => {
                let val = self.lower_expr(fb, operand);
                let new_val = fb.new_vreg();
                fb.emit(Instruction::BinOp {
                    dest: new_val,
                    op: IrBinOp::Sub,
                    left: val,
                    right: Operand::Immediate(1),
                });
                if let Expr::Identifier(name) = operand.as_ref() {
                    if let Some(&vreg) = fb.locals.get(name) {
                        fb.emit(Instruction::Copy {
                            dest: vreg,
                            src: Operand::VReg(new_val),
                        });
                    }
                }
                Operand::VReg(new_val)
            }
            Expr::Ternary {
                condition,
                then_expr,
                else_expr,
            } => {
                let cond = self.lower_expr(fb, condition);
                let result = fb.new_vreg();
                let then_label = fb.new_label();
                let else_label = fb.new_label();
                let end_label = fb.new_label();

                fb.terminate(Terminator::Branch {
                    condition: cond,
                    true_label: then_label,
                    false_label: else_label,
                });

                fb.start_block(then_label);
                let then_val = self.lower_expr(fb, then_expr);
                fb.emit(Instruction::Copy {
                    dest: result,
                    src: then_val,
                });
                fb.terminate(Terminator::Jump(end_label));

                fb.start_block(else_label);
                let else_val = self.lower_expr(fb, else_expr);
                fb.emit(Instruction::Copy {
                    dest: result,
                    src: else_val,
                });
                fb.terminate(Terminator::Jump(end_label));

                fb.start_block(end_label);
                Operand::VReg(result)
            }
            Expr::Deref(_) | Expr::AddrOf(_) | Expr::ArraySubscript { .. } => {
                // Simplified pointer handling
                Operand::Immediate(0)
            }
            Expr::Sizeof(ty) => {
                let size = match ty.as_ref() {
                    TypeSpec::Char | TypeSpec::SignedChar | TypeSpec::UnsignedChar => 1,
                    TypeSpec::Short | TypeSpec::UnsignedShort => 2,
                    TypeSpec::Int | TypeSpec::UnsignedInt => 4,
                    TypeSpec::Long | TypeSpec::UnsignedLong => 8,
                    TypeSpec::Pointer(_) | TypeSpec::FunctionPointer { .. } => 8,
                    TypeSpec::Array(elem_ty, Some(count)) => {
                        let elem_size = match elem_ty.as_ref() {
                            TypeSpec::Char | TypeSpec::SignedChar | TypeSpec::UnsignedChar => 1,
                            TypeSpec::Short | TypeSpec::UnsignedShort => 2,
                            TypeSpec::Int | TypeSpec::UnsignedInt => 4,
                            TypeSpec::Long | TypeSpec::UnsignedLong => 8,
                            TypeSpec::Pointer(_) => 8,
                            _ => 4,
                        };
                        elem_size * *count as i64
                    }
                    _ => 4,
                };
                Operand::Immediate(size)
            }
            Expr::Cast { expr, .. } => {
                // For now, just pass through the expression
                self.lower_expr(fb, expr)
            }
        }
    }
}

pub fn lower(program: &Program) -> Module {
    let mut builder = IrBuilder::new();
    builder.lower_program(program);
    Module {
        functions: builder.functions,
        string_literals: builder.string_literals,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser;

    #[test]
    fn test_lower_return_42() {
        let tokens = lexer::lex("int main() { return 42; }").unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = lower(&program);
        assert_eq!(module.functions.len(), 1);
        assert_eq!(module.functions[0].name, "main");
        assert!(!module.functions[0].blocks.is_empty());
    }

    #[test]
    fn test_lower_with_variable() {
        let tokens = lexer::lex("int main() { int x = 10; return x; }").unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = lower(&program);
        assert_eq!(module.functions.len(), 1);
    }
}
