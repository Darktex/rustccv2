//! Naive x86-64 code generator.
//!
//! Each virtual register maps to a stack slot.
//! Emits AT&T syntax x86-64 assembly.
//! Uses System V AMD64 ABI for function calls.

use crate::ir::*;

/// Calling convention: first 6 integer args in registers
const ARG_REGS: [&str; 6] = ["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];

pub struct CodeGen {
    output: String,
    /// Current function's vreg -> stack offset mapping
    vreg_offsets: Vec<i32>,
    stack_size: i32,
}

impl CodeGen {
    pub fn new() -> Self {
        Self {
            output: String::new(),
            vreg_offsets: Vec::new(),
            stack_size: 0,
        }
    }

    pub fn generate(mut self, program: &IrProgram) -> String {
        // Emit string literals in data section
        if !program.string_literals.is_empty() {
            self.emit("    .section .rodata");
            for (i, s) in program.string_literals.iter().enumerate() {
                self.emit(&format!(".Lstr{i}:"));
                // Escape string for assembly
                let escaped = escape_string_for_asm(s);
                self.emit(&format!("    .string \"{escaped}\""));
            }
        }

        self.emit("    .text");

        for func in &program.functions {
            self.gen_function(func);
        }

        self.output
    }

    fn emit(&mut self, line: &str) {
        self.output.push_str(line);
        self.output.push('\n');
    }

    fn vreg_offset(&self, vreg: VReg) -> i32 {
        self.vreg_offsets[vreg as usize]
    }

    fn vreg_mem(&self, vreg: VReg) -> String {
        let offset = self.vreg_offset(vreg);
        format!("{offset}(%rbp)")
    }

    fn gen_function(&mut self, func: &IrFunction) {
        self.emit(&format!("    .globl {}", func.name));
        self.emit(&format!("{}:", func.name));

        // Allocate stack space: each vreg gets 8 bytes
        let num_vregs = func.next_vreg as i32;
        self.vreg_offsets = Vec::with_capacity(num_vregs as usize);
        for i in 0..num_vregs {
            self.vreg_offsets.push(-8 * (i + 1));
        }
        // Align stack to 16 bytes
        self.stack_size = ((num_vregs * 8 + 15) / 16) * 16;

        // Prologue
        self.emit("    pushq %rbp");
        self.emit("    movq %rsp, %rbp");
        if self.stack_size > 0 {
            self.emit(&format!("    subq ${}, %rsp", self.stack_size));
        }

        // Move parameters from registers to stack slots
        for (i, &param_vreg) in func.params.iter().enumerate() {
            if i < ARG_REGS.len() {
                let mem = self.vreg_mem(param_vreg);
                self.emit(&format!("    movq {}, {mem}", ARG_REGS[i]));
            }
            // TODO: stack-passed params for > 6 args
        }

        // Generate code for each basic block
        for block in &func.blocks {
            self.emit(&format!(".L{}_bb{}:", func.name, block.label));

            for inst in &block.instructions {
                self.gen_instruction(inst, &func.name);
            }

            self.gen_terminator(&block.terminator, &func.name);
        }
    }

    fn gen_instruction(&mut self, inst: &Instruction, func_name: &str) {
        match inst {
            Instruction::LoadImm(dst, val) => {
                let mem = self.vreg_mem(*dst);
                self.emit(&format!("    movq ${val}, {mem}"));
            }
            Instruction::LoadStringAddr(dst, id) => {
                let mem = self.vreg_mem(*dst);
                self.emit(&format!("    leaq .Lstr{id}(%rip), %rax"));
                self.emit(&format!("    movq %rax, {mem}"));
            }
            Instruction::BinOp(dst, op, lhs, rhs) => {
                self.gen_binop(*dst, *op, *lhs, *rhs);
            }
            Instruction::UnaryOp(dst, op, src) => {
                let src_mem = self.vreg_mem(*src);
                let dst_mem = self.vreg_mem(*dst);
                self.emit(&format!("    movq {src_mem}, %rax"));
                match op {
                    IrUnaryOp::Negate => {
                        self.emit("    negq %rax");
                    }
                    IrUnaryOp::BitwiseNot => {
                        self.emit("    notq %rax");
                    }
                    IrUnaryOp::LogicalNot => {
                        self.emit("    cmpq $0, %rax");
                        self.emit("    sete %al");
                        self.emit("    movzbq %al, %rax");
                    }
                }
                self.emit(&format!("    movq %rax, {dst_mem}"));
            }
            Instruction::Call(dst, name, args) => {
                // Push args in reverse to registers
                for (i, &arg) in args.iter().enumerate() {
                    if i < ARG_REGS.len() {
                        let mem = self.vreg_mem(arg);
                        self.emit(&format!("    movq {mem}, {}", ARG_REGS[i]));
                    }
                    // TODO: stack args for > 6
                }
                // Align stack to 16 bytes before call (required by ABI)
                // The stack is already aligned from function prologue
                self.emit("    movq $0, %rax"); // varargs: no xmm args
                self.emit(&format!("    call {name}"));
                let dst_mem = self.vreg_mem(*dst);
                self.emit(&format!("    movq %rax, {dst_mem}"));
            }
            Instruction::Alloca(dst) => {
                // In our naive codegen, alloca just stores the address of the slot itself
                // The slot IS the vreg's stack position
                let offset = self.vreg_offset(*dst);
                let mem = self.vreg_mem(*dst);
                self.emit(&format!("    leaq {offset}(%rbp), %rax"));
                self.emit(&format!("    movq %rax, {mem}"));
            }
            Instruction::Store(addr, val) => {
                let addr_mem = self.vreg_mem(*addr);
                let val_mem = self.vreg_mem(*val);
                self.emit(&format!("    movq {addr_mem}, %rax")); // load address
                self.emit(&format!("    movq {val_mem}, %rcx")); // load value
                self.emit("    movq %rcx, (%rax)"); // store value at address
            }
            Instruction::Load(dst, addr) => {
                let addr_mem = self.vreg_mem(*addr);
                let dst_mem = self.vreg_mem(*dst);
                self.emit(&format!("    movq {addr_mem}, %rax")); // load address
                self.emit("    movq (%rax), %rax"); // load value from address
                self.emit(&format!("    movq %rax, {dst_mem}")); // store to dst
            }
            Instruction::Copy(dst, src) => {
                let src_mem = self.vreg_mem(*src);
                let dst_mem = self.vreg_mem(*dst);
                self.emit(&format!("    movq {src_mem}, %rax"));
                self.emit(&format!("    movq %rax, {dst_mem}"));
            }
        }
        let _ = func_name;
    }

    fn gen_binop(&mut self, dst: VReg, op: IrBinOp, lhs: VReg, rhs: VReg) {
        let lhs_mem = self.vreg_mem(lhs);
        let rhs_mem = self.vreg_mem(rhs);
        let dst_mem = self.vreg_mem(dst);

        match op {
            IrBinOp::Add => {
                self.emit(&format!("    movq {lhs_mem}, %rax"));
                self.emit(&format!("    addq {rhs_mem}, %rax"));
                self.emit(&format!("    movq %rax, {dst_mem}"));
            }
            IrBinOp::Sub => {
                self.emit(&format!("    movq {lhs_mem}, %rax"));
                self.emit(&format!("    subq {rhs_mem}, %rax"));
                self.emit(&format!("    movq %rax, {dst_mem}"));
            }
            IrBinOp::Mul => {
                self.emit(&format!("    movq {lhs_mem}, %rax"));
                self.emit(&format!("    imulq {rhs_mem}, %rax"));
                self.emit(&format!("    movq %rax, {dst_mem}"));
            }
            IrBinOp::Div => {
                self.emit(&format!("    movq {lhs_mem}, %rax"));
                self.emit("    cqto"); // sign-extend rax into rdx:rax
                self.emit(&format!("    idivq {rhs_mem}"));
                self.emit(&format!("    movq %rax, {dst_mem}"));
            }
            IrBinOp::Mod => {
                self.emit(&format!("    movq {lhs_mem}, %rax"));
                self.emit("    cqto");
                self.emit(&format!("    idivq {rhs_mem}"));
                self.emit(&format!("    movq %rdx, {dst_mem}")); // remainder in rdx
            }
            IrBinOp::Equal
            | IrBinOp::NotEqual
            | IrBinOp::Less
            | IrBinOp::Greater
            | IrBinOp::LessEqual
            | IrBinOp::GreaterEqual => {
                self.emit(&format!("    movq {lhs_mem}, %rax"));
                self.emit(&format!("    cmpq {rhs_mem}, %rax"));
                let set_instr = match op {
                    IrBinOp::Equal => "sete",
                    IrBinOp::NotEqual => "setne",
                    IrBinOp::Less => "setl",
                    IrBinOp::Greater => "setg",
                    IrBinOp::LessEqual => "setle",
                    IrBinOp::GreaterEqual => "setge",
                    _ => unreachable!(),
                };
                self.emit(&format!("    {set_instr} %al"));
                self.emit("    movzbq %al, %rax");
                self.emit(&format!("    movq %rax, {dst_mem}"));
            }
            IrBinOp::BitwiseAnd => {
                self.emit(&format!("    movq {lhs_mem}, %rax"));
                self.emit(&format!("    andq {rhs_mem}, %rax"));
                self.emit(&format!("    movq %rax, {dst_mem}"));
            }
            IrBinOp::BitwiseOr => {
                self.emit(&format!("    movq {lhs_mem}, %rax"));
                self.emit(&format!("    orq {rhs_mem}, %rax"));
                self.emit(&format!("    movq %rax, {dst_mem}"));
            }
            IrBinOp::BitwiseXor => {
                self.emit(&format!("    movq {lhs_mem}, %rax"));
                self.emit(&format!("    xorq {rhs_mem}, %rax"));
                self.emit(&format!("    movq %rax, {dst_mem}"));
            }
            IrBinOp::ShiftLeft => {
                self.emit(&format!("    movq {lhs_mem}, %rax"));
                self.emit(&format!("    movq {rhs_mem}, %rcx"));
                self.emit("    shlq %cl, %rax");
                self.emit(&format!("    movq %rax, {dst_mem}"));
            }
            IrBinOp::ShiftRight => {
                self.emit(&format!("    movq {lhs_mem}, %rax"));
                self.emit(&format!("    movq {rhs_mem}, %rcx"));
                self.emit("    sarq %cl, %rax"); // arithmetic shift right
                self.emit(&format!("    movq %rax, {dst_mem}"));
            }
        }
    }

    fn gen_terminator(&mut self, term: &Terminator, func_name: &str) {
        match term {
            Terminator::Return(vreg) => {
                let mem = self.vreg_mem(*vreg);
                self.emit(&format!("    movq {mem}, %rax"));
                self.emit("    movq %rbp, %rsp");
                self.emit("    popq %rbp");
                self.emit("    ret");
            }
            Terminator::ReturnVoid => {
                self.emit("    movq %rbp, %rsp");
                self.emit("    popq %rbp");
                self.emit("    ret");
            }
            Terminator::Jump(label) => {
                self.emit(&format!("    jmp .L{func_name}_bb{label}"));
            }
            Terminator::Branch(cond, then_label, else_label) => {
                let cond_mem = self.vreg_mem(*cond);
                self.emit(&format!("    cmpq $0, {cond_mem}"));
                self.emit(&format!("    jne .L{func_name}_bb{then_label}"));
                self.emit(&format!("    jmp .L{func_name}_bb{else_label}"));
            }
            Terminator::None => {
                // Should not happen in well-formed IR
                self.emit("    ud2");
            }
        }
    }
}

impl Default for CodeGen {
    fn default() -> Self {
        Self::new()
    }
}

fn escape_string_for_asm(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\0' => out.push_str("\\0"),
            c if c.is_ascii_graphic() || c == ' ' => out.push(c),
            c => {
                // Emit as octal escape
                let b = c as u8;
                out.push_str(&format!("\\{b:03o}"));
            }
        }
    }
    out
}
