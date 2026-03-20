//! Naive x86-64 code generator (AT&T syntax).
//!
//! Strategy: every virtual register is assigned a stack slot.
//! All operations load operands from stack, compute in registers,
//! and store the result back to the stack slot.

use crate::ir::{
    BinOp, CmpOp, Function, GlobalData, Instruction, Module, Terminator, UnaryOp, VReg,
};
use std::collections::HashMap;
use std::fmt::Write;

/// The x86-64 code generator.
pub struct X86_64Generator {
    output: String,
    /// Map from VReg to stack offset (negative from %rbp).
    stack_slots: HashMap<VReg, i32>,
    /// Next available stack offset.
    next_stack_offset: i32,
    /// Tracks alloca sizes for proper stack frame calculation.
    alloca_slots: HashMap<VReg, i32>,
}

/// System V AMD64 ABI argument registers (in order).
const ARG_REGS: [&str; 6] = ["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];

impl X86_64Generator {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            stack_slots: HashMap::new(),
            next_stack_offset: 0,
            alloca_slots: HashMap::new(),
        }
    }

    /// Generate x86-64 assembly for an entire module.
    pub fn generate(&mut self, module: &Module) -> String {
        self.output.clear();

        // Emit global data (string literals, etc.)
        if !module.globals.is_empty() {
            writeln!(self.output, "    .section .rodata").unwrap();
            for global in &module.globals {
                self.emit_global_data(global);
            }
            writeln!(self.output).unwrap();
        }

        // Emit functions
        writeln!(self.output, "    .text").unwrap();
        for func in &module.functions {
            self.generate_function(func);
        }

        self.output.clone()
    }

    fn emit_global_data(&mut self, global: &GlobalData) {
        writeln!(self.output, "{}:", global.name).unwrap();
        // Emit as .asciz if it ends with null byte, otherwise .byte
        if global.data.last() == Some(&0) {
            let s = String::from_utf8_lossy(&global.data[..global.data.len() - 1]);
            writeln!(self.output, "    .asciz \"{}\"", escape_string(&s)).unwrap();
        } else {
            let bytes: Vec<String> = global.data.iter().map(|b| format!("{}", b)).collect();
            writeln!(self.output, "    .byte {}", bytes.join(", ")).unwrap();
        }
    }

    fn generate_function(&mut self, func: &Function) {
        self.stack_slots.clear();
        self.next_stack_offset = 0;
        self.alloca_slots.clear();

        // Assign stack slots to all VRegs used in the function
        self.assign_stack_slots(func);

        // Function header
        writeln!(self.output, "    .globl {}", func.name).unwrap();
        writeln!(self.output, "{}:", func.name).unwrap();

        // Prologue
        writeln!(self.output, "    pushq %rbp").unwrap();
        writeln!(self.output, "    movq %rsp, %rbp").unwrap();

        // Allocate stack frame (align to 16 bytes)
        let frame_size = align_up((-self.next_stack_offset) as u32, 16);
        if frame_size > 0 {
            writeln!(self.output, "    subq ${}, %rsp", frame_size).unwrap();
        }

        // Move parameters from registers to stack slots
        for (i, param) in func.params.iter().enumerate() {
            if i < ARG_REGS.len() {
                let offset = self.stack_slots[param];
                writeln!(self.output, "    movq {}, {}(%rbp)", ARG_REGS[i], offset).unwrap();
            }
            // TODO: stack-passed arguments for > 6 params
        }

        // Generate code for each basic block
        for block in &func.blocks {
            writeln!(self.output, ".L{}_{}: ", func.name, block.id.0).unwrap();

            for inst in &block.instructions {
                self.generate_instruction(inst, &func.name);
            }

            if let Some(ref term) = block.terminator {
                self.generate_terminator(term, &func.name);
            }
        }

        writeln!(self.output).unwrap();
    }

    /// Pre-assign stack slots for all virtual registers in the function.
    fn assign_stack_slots(&mut self, func: &Function) {
        // Assign slots for parameters
        for param in &func.params {
            self.get_or_create_slot(*param);
        }

        // Assign slots for all instruction destinations
        for block in &func.blocks {
            for inst in &block.instructions {
                if let Some(dst) = inst.dst() {
                    self.get_or_create_slot(dst);
                }
                // For alloca, track the size
                if let Instruction::Alloca { dst, size } = inst {
                    self.alloca_slots.insert(*dst, *size as i32);
                }
            }
        }
    }

    fn get_or_create_slot(&mut self, vreg: VReg) -> i32 {
        if let Some(&offset) = self.stack_slots.get(&vreg) {
            return offset;
        }
        self.next_stack_offset -= 8;
        let offset = self.next_stack_offset;
        self.stack_slots.insert(vreg, offset);
        offset
    }

    fn vreg_offset(&self, vreg: VReg) -> i32 {
        *self
            .stack_slots
            .get(&vreg)
            .unwrap_or_else(|| panic!("VReg {} not assigned a stack slot", vreg))
    }

    /// Load a VReg value into a physical register.
    fn load_vreg(&mut self, vreg: VReg, reg: &str) {
        let offset = self.vreg_offset(vreg);
        writeln!(self.output, "    movq {}(%rbp), {}", offset, reg).unwrap();
    }

    /// Store a physical register value into a VReg's stack slot.
    fn store_vreg(&mut self, reg: &str, vreg: VReg) {
        let offset = self.vreg_offset(vreg);
        writeln!(self.output, "    movq {}, {}(%rbp)", reg, offset).unwrap();
    }

    fn generate_instruction(&mut self, inst: &Instruction, func_name: &str) {
        match inst {
            Instruction::Constant { dst, value } => {
                writeln!(self.output, "    movq ${}, %rax", value).unwrap();
                self.store_vreg("%rax", *dst);
            }

            Instruction::BinOp { dst, op, lhs, rhs } => {
                self.load_vreg(*lhs, "%rax");
                self.load_vreg(*rhs, "%rcx");
                match op {
                    BinOp::Add => {
                        writeln!(self.output, "    addq %rcx, %rax").unwrap();
                    }
                    BinOp::Sub => {
                        writeln!(self.output, "    subq %rcx, %rax").unwrap();
                    }
                    BinOp::Mul => {
                        writeln!(self.output, "    imulq %rcx, %rax").unwrap();
                    }
                    BinOp::Div => {
                        writeln!(self.output, "    cqto").unwrap(); // sign-extend %rax into %rdx:%rax
                        writeln!(self.output, "    idivq %rcx").unwrap();
                    }
                    BinOp::Mod => {
                        writeln!(self.output, "    cqto").unwrap();
                        writeln!(self.output, "    idivq %rcx").unwrap();
                        writeln!(self.output, "    movq %rdx, %rax").unwrap(); // remainder in %rdx
                    }
                    BinOp::And => {
                        writeln!(self.output, "    andq %rcx, %rax").unwrap();
                    }
                    BinOp::Or => {
                        writeln!(self.output, "    orq %rcx, %rax").unwrap();
                    }
                    BinOp::Xor => {
                        writeln!(self.output, "    xorq %rcx, %rax").unwrap();
                    }
                    BinOp::Shl => {
                        // Shift amount must be in %cl
                        writeln!(self.output, "    shlq %cl, %rax").unwrap();
                    }
                    BinOp::Shr => {
                        writeln!(self.output, "    sarq %cl, %rax").unwrap();
                    }
                }
                self.store_vreg("%rax", *dst);
            }

            Instruction::UnaryOp { dst, op, operand } => {
                self.load_vreg(*operand, "%rax");
                match op {
                    UnaryOp::Neg => {
                        writeln!(self.output, "    negq %rax").unwrap();
                    }
                    UnaryOp::Not => {
                        // Logical not: !x == (x == 0)
                        writeln!(self.output, "    cmpq $0, %rax").unwrap();
                        writeln!(self.output, "    sete %al").unwrap();
                        writeln!(self.output, "    movzbq %al, %rax").unwrap();
                    }
                    UnaryOp::BitNot => {
                        writeln!(self.output, "    notq %rax").unwrap();
                    }
                }
                self.store_vreg("%rax", *dst);
            }

            Instruction::Cmp { dst, op, lhs, rhs } => {
                self.load_vreg(*lhs, "%rax");
                self.load_vreg(*rhs, "%rcx");
                writeln!(self.output, "    cmpq %rcx, %rax").unwrap();
                let set_inst = match op {
                    CmpOp::Eq => "sete",
                    CmpOp::Ne => "setne",
                    CmpOp::Lt => "setl",
                    CmpOp::Le => "setle",
                    CmpOp::Gt => "setg",
                    CmpOp::Ge => "setge",
                };
                writeln!(self.output, "    {} %al", set_inst).unwrap();
                writeln!(self.output, "    movzbq %al, %rax").unwrap();
                self.store_vreg("%rax", *dst);
            }

            Instruction::Alloca { dst, size: _ } => {
                // The alloca address is the stack slot itself.
                // We store the address of the stack slot as the value.
                let offset = self.vreg_offset(*dst);
                // We use a separate region for alloca'd memory.
                // For simplicity in naive codegen, the alloca slot address
                // is just the slot's own address on the stack.
                writeln!(self.output, "    leaq {}(%rbp), %rax", offset).unwrap();
                self.store_vreg("%rax", *dst);
            }

            Instruction::Load { dst, addr } => {
                self.load_vreg(*addr, "%rax"); // load the address
                writeln!(self.output, "    movq (%rax), %rax").unwrap(); // dereference
                self.store_vreg("%rax", *dst);
            }

            Instruction::Store { addr, value } => {
                self.load_vreg(*addr, "%rax"); // load the address
                self.load_vreg(*value, "%rcx"); // load the value
                writeln!(self.output, "    movq %rcx, (%rax)").unwrap(); // store
            }

            Instruction::Call { dst, func, args } => {
                // Push args into argument registers (System V AMD64 ABI)
                // We need to be careful about register clobbering.
                // First, push all args to stack, then pop into arg regs.
                let num_reg_args = args.len().min(ARG_REGS.len());

                // Load args into argument registers
                // We load in reverse order to avoid clobbering
                for i in (0..num_reg_args).rev() {
                    self.load_vreg(args[i], ARG_REGS[i]);
                }

                // Stack-passed arguments (if any) — push right to left
                if args.len() > ARG_REGS.len() {
                    for i in (ARG_REGS.len()..args.len()).rev() {
                        self.load_vreg(args[i], "%rax");
                        writeln!(self.output, "    pushq %rax").unwrap();
                    }
                }

                // Align stack to 16 bytes before call if needed
                let stack_args = if args.len() > ARG_REGS.len() {
                    args.len() - ARG_REGS.len()
                } else {
                    0
                };
                let stack_adjustment = if stack_args % 2 != 0 { 8 } else { 0 };
                if stack_adjustment > 0 {
                    writeln!(self.output, "    subq ${}, %rsp", stack_adjustment).unwrap();
                }

                // Zero %rax for varargs functions (AL = number of vector args)
                writeln!(self.output, "    xorl %eax, %eax").unwrap();

                // If the function name starts with a dot or is a known libc function,
                // call it directly. Otherwise use PLT for external functions.
                if is_likely_external(func) {
                    writeln!(self.output, "    call {}@PLT", func).unwrap();
                } else {
                    writeln!(self.output, "    call {}", func).unwrap();
                }

                // Clean up stack args
                let total_stack = (stack_args * 8) as i32 + stack_adjustment;
                if total_stack > 0 {
                    writeln!(self.output, "    addq ${}, %rsp", total_stack).unwrap();
                }

                // Store return value
                if let Some(d) = dst {
                    self.store_vreg("%rax", *d);
                }
            }

            Instruction::Phi { dst, incoming } => {
                // In naive codegen, phi nodes are handled by having predecessor
                // blocks write to the phi's stack slot before branching.
                // For now, we emit a comment — the actual moves happen at branch sites.
                // Actually, for a simple approach, we'll handle phis by emitting
                // moves in predecessor blocks. But since we generate linearly,
                // we handle this with a simpler strategy: each phi source should
                // have stored into the phi dst slot.
                //
                // For the naive approach: we just ensure the slot exists.
                // The predecessor blocks must have been modified to store into
                // this slot. This is handled by the lowering pass.
                let _ = (dst, incoming, func_name);
                writeln!(self.output, "    # phi {} (resolved by predecessors)", dst).unwrap();
            }

            Instruction::GlobalRef { dst, name } => {
                writeln!(self.output, "    leaq {}(%rip), %rax", name).unwrap();
                self.store_vreg("%rax", *dst);
            }
        }
    }

    fn generate_terminator(&mut self, term: &Terminator, func_name: &str) {
        match term {
            Terminator::Ret(value) => {
                if let Some(v) = value {
                    self.load_vreg(*v, "%rax");
                }
                // Epilogue
                writeln!(self.output, "    movq %rbp, %rsp").unwrap();
                writeln!(self.output, "    popq %rbp").unwrap();
                writeln!(self.output, "    ret").unwrap();
            }

            Terminator::Branch(target) => {
                writeln!(self.output, "    jmp .L{}_{}", func_name, target.0).unwrap();
            }

            Terminator::CondBranch {
                cond,
                true_bb,
                false_bb,
            } => {
                self.load_vreg(*cond, "%rax");
                writeln!(self.output, "    cmpq $0, %rax").unwrap();
                writeln!(self.output, "    jne .L{}_{}", func_name, true_bb.0).unwrap();
                writeln!(self.output, "    jmp .L{}_{}", func_name, false_bb.0).unwrap();
            }
        }
    }
}

impl Default for X86_64Generator {
    fn default() -> Self {
        Self::new()
    }
}

/// Check if a function is likely external (libc, etc.)
fn is_likely_external(name: &str) -> bool {
    matches!(
        name,
        "printf"
            | "fprintf"
            | "sprintf"
            | "snprintf"
            | "puts"
            | "putchar"
            | "getchar"
            | "malloc"
            | "calloc"
            | "realloc"
            | "free"
            | "memcpy"
            | "memset"
            | "memmove"
            | "memcmp"
            | "strlen"
            | "strcpy"
            | "strncpy"
            | "strcmp"
            | "strncmp"
            | "strcat"
            | "exit"
            | "abort"
            | "fopen"
            | "fclose"
            | "fread"
            | "fwrite"
            | "scanf"
            | "sscanf"
            | "atoi"
            | "atol"
            | "strtol"
            | "strtoul"
    )
}

/// Escape a string for use in assembly .asciz directive.
fn escape_string(s: &str) -> String {
    let mut result = String::new();
    for ch in s.chars() {
        match ch {
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\n' => result.push_str("\\n"),
            '\t' => result.push_str("\\t"),
            '\r' => result.push_str("\\r"),
            '\0' => result.push_str("\\0"),
            c => result.push(c),
        }
    }
    result
}

fn align_up(value: u32, align: u32) -> u32 {
    (value + align - 1) & !(align - 1)
}
