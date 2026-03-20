/// Code generator with x86-64 and AArch64 backends.
/// Naive: every virtual register gets a stack slot.
use crate::ir::{
    BasicBlock, CmpOp, Function, Instruction, IrBinOp, IrUnaryOp, Module, Operand, Terminator, VReg,
};
use std::collections::HashMap;
use std::fmt::Write;

/// Dispatch to the appropriate backend based on target architecture.
pub fn generate(module: &Module) -> String {
    if cfg!(target_arch = "aarch64") {
        let mut codegen = Aarch64CodeGen::new();
        codegen.generate_module(module);
        codegen.output
    } else {
        let mut codegen = X86CodeGen::new();
        codegen.generate_module(module);
        codegen.output
    }
}

fn escape_string_for_gas(s: &str) -> String {
    let mut result = String::new();
    for c in s.chars() {
        match c {
            '\n' => result.push_str("\\n"),
            '\t' => result.push_str("\\t"),
            '\r' => result.push_str("\\r"),
            '\\' => result.push_str("\\\\"),
            '"' => result.push_str("\\\""),
            '\0' => result.push_str("\\0"),
            c if c.is_ascii_graphic() || c == ' ' => result.push(c),
            c => {
                let b = c as u32;
                write!(result, "\\{:03o}", b).unwrap();
            }
        }
    }
    result
}

// ============================================================================
// x86-64 Code Generator (AT&T syntax, System V AMD64 ABI)
// ============================================================================

/// System V AMD64 ABI argument registers
const X86_ARG_REGS: [&str; 6] = ["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];

struct X86CodeGen {
    output: String,
    stack_slots: HashMap<VReg, i32>,
    stack_size: i32,
}

impl X86CodeGen {
    fn new() -> Self {
        X86CodeGen {
            output: String::new(),
            stack_slots: HashMap::new(),
            stack_size: 0,
        }
    }

    fn slot_for(&mut self, vreg: VReg) -> i32 {
        if let Some(&offset) = self.stack_slots.get(&vreg) {
            return offset;
        }
        self.stack_size += 8;
        let offset = -self.stack_size;
        self.stack_slots.insert(vreg, offset);
        offset
    }

    fn emit_line(&mut self, line: &str) {
        writeln!(self.output, "    {}", line).unwrap();
    }

    fn emit_label(&mut self, label: &str) {
        writeln!(self.output, "{}:", label).unwrap();
    }

    fn emit_raw(&mut self, line: &str) {
        writeln!(self.output, "{}", line).unwrap();
    }

    fn load_operand_rax(&mut self, op: &Operand) {
        match op {
            Operand::Immediate(val) => {
                self.emit_line(&format!("movq ${}, %rax", val));
            }
            Operand::VReg(vreg) => {
                let offset = self.slot_for(*vreg);
                self.emit_line(&format!("movq {}(%rbp), %rax", offset));
            }
        }
    }

    fn load_operand_rcx(&mut self, op: &Operand) {
        match op {
            Operand::Immediate(val) => {
                self.emit_line(&format!("movq ${}, %rcx", val));
            }
            Operand::VReg(vreg) => {
                let offset = self.slot_for(*vreg);
                self.emit_line(&format!("movq {}(%rbp), %rcx", offset));
            }
        }
    }

    fn store_rax(&mut self, vreg: VReg) {
        let offset = self.slot_for(vreg);
        self.emit_line(&format!("movq %rax, {}(%rbp)", offset));
    }

    fn generate_module(&mut self, module: &Module) {
        if !module.string_literals.is_empty() {
            self.emit_raw("    .section .rodata");
            for (label, value) in &module.string_literals {
                self.emit_label(label);
                let escaped = escape_string_for_gas(value);
                self.emit_line(&format!(".string \"{}\"", escaped));
            }
            self.emit_raw("");
        }

        self.emit_raw("    .text");
        for func in &module.functions {
            if func.is_defined {
                self.generate_function(func);
            }
        }
    }

    fn generate_function(&mut self, func: &Function) {
        self.stack_slots.clear();
        self.stack_size = 0;

        for vreg in 0..func.num_vregs {
            self.slot_for(vreg);
        }

        let aligned_stack = (self.stack_size + 15) & !15;

        self.emit_raw(&format!("    .globl {}", func.name));
        self.emit_label(&func.name);

        self.emit_line("pushq %rbp");
        self.emit_line("movq %rsp, %rbp");
        if aligned_stack > 0 {
            self.emit_line(&format!("subq ${}, %rsp", aligned_stack));
        }

        for (i, param_name) in func.params.iter().enumerate() {
            if i < X86_ARG_REGS.len() && !param_name.is_empty() {
                if let Some(&vreg) = func.locals.get(param_name) {
                    let offset = self.slot_for(vreg);
                    self.emit_line(&format!("movq {}, {}(%rbp)", X86_ARG_REGS[i], offset));
                }
            }
        }

        for block in &func.blocks {
            self.generate_block(block, &func.name);
        }
    }

    fn generate_block(&mut self, block: &BasicBlock, func_name: &str) {
        self.emit_label(&format!(".L{}_{}", func_name, block.label));

        for inst in &block.instructions {
            self.generate_instruction(inst);
        }

        self.generate_terminator(&block.terminator, func_name);
    }

    fn generate_instruction(&mut self, inst: &Instruction) {
        match inst {
            Instruction::LoadImm { dest, value } => {
                self.emit_line(&format!("movq ${}, %rax", value));
                self.store_rax(*dest);
            }
            Instruction::BinOp {
                dest,
                op,
                left,
                right,
            } => {
                self.load_operand_rax(left);
                self.load_operand_rcx(right);
                match op {
                    IrBinOp::Add => self.emit_line("addq %rcx, %rax"),
                    IrBinOp::Sub => self.emit_line("subq %rcx, %rax"),
                    IrBinOp::Mul => self.emit_line("imulq %rcx, %rax"),
                    IrBinOp::Div => {
                        self.emit_line("cqto");
                        self.emit_line("idivq %rcx");
                    }
                    IrBinOp::Mod => {
                        self.emit_line("cqto");
                        self.emit_line("idivq %rcx");
                        self.emit_line("movq %rdx, %rax");
                    }
                    IrBinOp::And => self.emit_line("andq %rcx, %rax"),
                    IrBinOp::Or => self.emit_line("orq %rcx, %rax"),
                    IrBinOp::Xor => self.emit_line("xorq %rcx, %rax"),
                    IrBinOp::Shl => self.emit_line("shlq %cl, %rax"),
                    IrBinOp::Shr => self.emit_line("sarq %cl, %rax"),
                }
                self.store_rax(*dest);
            }
            Instruction::UnaryOp { dest, op, operand } => {
                self.load_operand_rax(operand);
                match op {
                    IrUnaryOp::Neg => self.emit_line("negq %rax"),
                    IrUnaryOp::Not => {
                        self.emit_line("testq %rax, %rax");
                        self.emit_line("sete %al");
                        self.emit_line("movzbq %al, %rax");
                    }
                    IrUnaryOp::BitwiseNot => self.emit_line("notq %rax"),
                }
                self.store_rax(*dest);
            }
            Instruction::Cmp {
                dest,
                op,
                left,
                right,
            } => {
                self.load_operand_rax(left);
                self.load_operand_rcx(right);
                self.emit_line("cmpq %rcx, %rax");
                let set_inst = match op {
                    CmpOp::Eq => "sete",
                    CmpOp::Ne => "setne",
                    CmpOp::Lt => "setl",
                    CmpOp::Le => "setle",
                    CmpOp::Gt => "setg",
                    CmpOp::Ge => "setge",
                };
                self.emit_line(&format!("{} %al", set_inst));
                self.emit_line("movzbq %al, %rax");
                self.store_rax(*dest);
            }
            Instruction::Copy { dest, src } => {
                self.load_operand_rax(src);
                self.store_rax(*dest);
            }
            Instruction::Call { dest, func, args } => {
                for arg in args {
                    self.load_operand_rax(arg);
                    self.emit_line("pushq %rax");
                }
                for i in (0..args.len()).rev() {
                    if i < X86_ARG_REGS.len() {
                        self.emit_line(&format!("popq {}", X86_ARG_REGS[i]));
                    } else {
                        self.emit_line("popq %rax");
                    }
                }
                self.emit_line("xorl %eax, %eax");
                self.emit_line(&format!("call {}", func));
                if let Some(dest) = dest {
                    self.store_rax(*dest);
                }
            }
            Instruction::LoadStringAddr { dest, label } => {
                self.emit_line(&format!("leaq {}(%rip), %rax", label));
                self.store_rax(*dest);
            }
            Instruction::Alloca { dest, size } => {
                self.emit_line(&format!("subq ${}, %rsp", size));
                self.emit_line("movq %rsp, %rax");
                self.store_rax(*dest);
            }
            Instruction::Store { addr, value } => {
                self.load_operand_rax(value);
                let addr_offset = self.slot_for(*addr);
                self.emit_line(&format!("movq {}(%rbp), %rcx", addr_offset));
                self.emit_line("movq %rax, (%rcx)");
            }
            Instruction::Load { dest, addr } => {
                let addr_offset = self.slot_for(*addr);
                self.emit_line(&format!("movq {}(%rbp), %rax", addr_offset));
                self.emit_line("movq (%rax), %rax");
                self.store_rax(*dest);
            }
        }
    }

    fn generate_terminator(&mut self, term: &Terminator, func_name: &str) {
        match term {
            Terminator::Return(val) => {
                if let Some(val) = val {
                    self.load_operand_rax(val);
                }
                self.emit_line("movq %rbp, %rsp");
                self.emit_line("popq %rbp");
                self.emit_line("ret");
            }
            Terminator::Jump(label) => {
                self.emit_line(&format!("jmp .L{}_{}", func_name, label));
            }
            Terminator::Branch {
                condition,
                true_label,
                false_label,
            } => {
                self.load_operand_rax(condition);
                self.emit_line("testq %rax, %rax");
                self.emit_line(&format!("jne .L{}_{}", func_name, true_label));
                self.emit_line(&format!("jmp .L{}_{}", func_name, false_label));
            }
            Terminator::None => {}
        }
    }
}

// ============================================================================
// AArch64 Code Generator (AAPCS64)
// ============================================================================

/// AAPCS64: first 8 integer args in x0-x7
const AARCH64_ARG_REGS: [&str; 8] = ["x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7"];

struct Aarch64CodeGen {
    output: String,
    /// Map from vreg to stack offset (positive from sp)
    stack_slots: HashMap<VReg, i32>,
    /// Total stack frame size (including saved fp/lr)
    stack_size: i32,
}

impl Aarch64CodeGen {
    fn new() -> Self {
        Aarch64CodeGen {
            output: String::new(),
            stack_slots: HashMap::new(),
            stack_size: 16, // Reserve 16 bytes for saved fp (x29) and lr (x30)
        }
    }

    fn slot_for(&mut self, vreg: VReg) -> i32 {
        if let Some(&offset) = self.stack_slots.get(&vreg) {
            return offset;
        }
        let offset = self.stack_size;
        self.stack_size += 8;
        self.stack_slots.insert(vreg, offset);
        offset
    }

    fn emit_line(&mut self, line: &str) {
        writeln!(self.output, "    {}", line).unwrap();
    }

    fn emit_label(&mut self, label: &str) {
        writeln!(self.output, "{}:", label).unwrap();
    }

    fn emit_raw(&mut self, line: &str) {
        writeln!(self.output, "{}", line).unwrap();
    }

    /// Load an operand into x0
    fn load_operand_x0(&mut self, op: &Operand) {
        match op {
            Operand::Immediate(val) => {
                self.load_immediate("x0", *val);
            }
            Operand::VReg(vreg) => {
                let offset = self.slot_for(*vreg);
                self.emit_line(&format!("ldr x0, [x29, #{}]", offset));
            }
        }
    }

    /// Load an operand into x1
    fn load_operand_x1(&mut self, op: &Operand) {
        match op {
            Operand::Immediate(val) => {
                self.load_immediate("x1", *val);
            }
            Operand::VReg(vreg) => {
                let offset = self.slot_for(*vreg);
                self.emit_line(&format!("ldr x1, [x29, #{}]", offset));
            }
        }
    }

    /// Load an immediate value into a register (handles large constants)
    fn load_immediate(&mut self, reg: &str, val: i64) {
        if (-65536..=65535).contains(&val) {
            self.emit_line(&format!("mov {}, #{}", reg, val));
        } else {
            // Large constants: use movz + movk sequence
            let uval = val as u64;
            self.emit_line(&format!("movz {}, #0x{:x}", reg, uval & 0xFFFF));
            if (uval >> 16) & 0xFFFF != 0 {
                self.emit_line(&format!(
                    "movk {}, #0x{:x}, lsl #16",
                    reg,
                    (uval >> 16) & 0xFFFF
                ));
            }
            if (uval >> 32) & 0xFFFF != 0 {
                self.emit_line(&format!(
                    "movk {}, #0x{:x}, lsl #32",
                    reg,
                    (uval >> 32) & 0xFFFF
                ));
            }
            if (uval >> 48) & 0xFFFF != 0 {
                self.emit_line(&format!(
                    "movk {}, #0x{:x}, lsl #48",
                    reg,
                    (uval >> 48) & 0xFFFF
                ));
            }
        }
    }

    /// Store x0 to a vreg's stack slot (use x29/fp for stability across calls)
    fn store_x0(&mut self, vreg: VReg) {
        let offset = self.slot_for(vreg);
        self.emit_line(&format!("str x0, [x29, #{}]", offset));
    }

    fn generate_module(&mut self, module: &Module) {
        if !module.string_literals.is_empty() {
            self.emit_raw("    .section .rodata");
            for (label, value) in &module.string_literals {
                self.emit_label(label);
                let escaped = escape_string_for_gas(value);
                self.emit_line(&format!(".string \"{}\"", escaped));
            }
            self.emit_raw("");
        }

        self.emit_raw("    .text");
        for func in &module.functions {
            if func.is_defined {
                self.generate_function(func);
            }
        }
    }

    fn generate_function(&mut self, func: &Function) {
        self.stack_slots.clear();
        self.stack_size = 16; // Reset: 16 bytes for fp + lr

        // Pre-allocate all vreg stack slots
        for vreg in 0..func.num_vregs {
            self.slot_for(vreg);
        }

        // Align stack to 16 bytes
        let aligned_stack = (self.stack_size + 15) & !15;

        self.emit_raw(&format!("    .globl {}", func.name));
        self.emit_label(&func.name);

        // Prologue: allocate frame, save fp and lr
        self.emit_line(&format!("sub sp, sp, #{}", aligned_stack));
        self.emit_line("stp x29, x30, [sp]");
        self.emit_line("mov x29, sp");

        // Save arguments to stack slots
        for (i, param_name) in func.params.iter().enumerate() {
            if i < AARCH64_ARG_REGS.len() && !param_name.is_empty() {
                if let Some(&vreg) = func.locals.get(param_name) {
                    let offset = self.slot_for(vreg);
                    self.emit_line(&format!("str {}, [x29, #{}]", AARCH64_ARG_REGS[i], offset));
                }
            }
        }

        // Generate basic blocks
        for block in &func.blocks {
            self.generate_block(block, &func.name);
        }
    }

    fn generate_block(&mut self, block: &BasicBlock, func_name: &str) {
        self.emit_label(&format!(".L{}_{}", func_name, block.label));

        for inst in &block.instructions {
            self.generate_instruction(inst);
        }

        self.generate_terminator(&block.terminator, func_name);
    }

    fn generate_instruction(&mut self, inst: &Instruction) {
        match inst {
            Instruction::LoadImm { dest, value } => {
                self.load_immediate("x0", *value);
                self.store_x0(*dest);
            }
            Instruction::BinOp {
                dest,
                op,
                left,
                right,
            } => {
                self.load_operand_x0(left);
                self.load_operand_x1(right);
                match op {
                    IrBinOp::Add => self.emit_line("add x0, x0, x1"),
                    IrBinOp::Sub => self.emit_line("sub x0, x0, x1"),
                    IrBinOp::Mul => self.emit_line("mul x0, x0, x1"),
                    IrBinOp::Div => self.emit_line("sdiv x0, x0, x1"),
                    IrBinOp::Mod => {
                        self.emit_line("sdiv x2, x0, x1");
                        self.emit_line("msub x0, x2, x1, x0");
                    }
                    IrBinOp::And => self.emit_line("and x0, x0, x1"),
                    IrBinOp::Or => self.emit_line("orr x0, x0, x1"),
                    IrBinOp::Xor => self.emit_line("eor x0, x0, x1"),
                    IrBinOp::Shl => self.emit_line("lsl x0, x0, x1"),
                    IrBinOp::Shr => self.emit_line("asr x0, x0, x1"),
                }
                self.store_x0(*dest);
            }
            Instruction::UnaryOp { dest, op, operand } => {
                self.load_operand_x0(operand);
                match op {
                    IrUnaryOp::Neg => self.emit_line("neg x0, x0"),
                    IrUnaryOp::Not => {
                        self.emit_line("cmp x0, #0");
                        self.emit_line("cset x0, eq");
                    }
                    IrUnaryOp::BitwiseNot => self.emit_line("mvn x0, x0"),
                }
                self.store_x0(*dest);
            }
            Instruction::Cmp {
                dest,
                op,
                left,
                right,
            } => {
                self.load_operand_x0(left);
                self.load_operand_x1(right);
                self.emit_line("cmp x0, x1");
                let cond = match op {
                    CmpOp::Eq => "eq",
                    CmpOp::Ne => "ne",
                    CmpOp::Lt => "lt",
                    CmpOp::Le => "le",
                    CmpOp::Gt => "gt",
                    CmpOp::Ge => "ge",
                };
                self.emit_line(&format!("cset x0, {}", cond));
                self.store_x0(*dest);
            }
            Instruction::Copy { dest, src } => {
                self.load_operand_x0(src);
                self.store_x0(*dest);
            }
            Instruction::Call { dest, func, args } => {
                // Load arguments into registers (AAPCS64)
                // Evaluate each arg and save to a scratch area below sp,
                // then load into arg registers. We use x29-relative addressing
                // for operand loads so sp modifications are safe.
                let num_args = args.len().min(AARCH64_ARG_REGS.len());

                // Allocate scratch space for args (16-byte aligned)
                let scratch_size = ((args.len() * 8 + 15) & !15) as i32;
                if scratch_size > 0 {
                    self.emit_line(&format!("sub sp, sp, #{}", scratch_size));
                }

                // Evaluate each arg and store to scratch area on stack
                for (idx, arg) in args.iter().enumerate() {
                    self.load_operand_x0(arg);
                    self.emit_line(&format!("str x0, [sp, #{}]", idx * 8));
                }

                // Load from scratch area into arg registers
                for (i, reg) in AARCH64_ARG_REGS
                    .iter()
                    .enumerate()
                    .take(args.len().min(AARCH64_ARG_REGS.len()))
                {
                    self.emit_line(&format!("ldr {}, [sp, #{}]", reg, i * 8));
                }

                // Deallocate scratch space
                if scratch_size > 0 {
                    self.emit_line(&format!("add sp, sp, #{}", scratch_size));
                }

                let _ = num_args;

                self.emit_line(&format!("bl {}", func));

                if let Some(dest) = dest {
                    self.store_x0(*dest);
                }
            }
            Instruction::LoadStringAddr { dest, label } => {
                self.emit_line(&format!("adrp x0, {}", label));
                self.emit_line(&format!("add x0, x0, :lo12:{}", label));
                self.store_x0(*dest);
            }
            Instruction::Alloca { dest, size } => {
                // Align size to 16 bytes for AArch64 sp alignment
                let aligned_size = (size + 15) & !15;
                self.emit_line(&format!("sub sp, sp, #{}", aligned_size));
                self.emit_line("mov x0, sp");
                self.store_x0(*dest);
            }
            Instruction::Store { addr, value } => {
                self.load_operand_x0(value);
                let addr_offset = self.slot_for(*addr);
                self.emit_line(&format!("ldr x1, [x29, #{}]", addr_offset));
                self.emit_line("str x0, [x1]");
            }
            Instruction::Load { dest, addr } => {
                let addr_offset = self.slot_for(*addr);
                self.emit_line(&format!("ldr x0, [x29, #{}]", addr_offset));
                self.emit_line("ldr x0, [x0]");
                self.store_x0(*dest);
            }
        }
    }

    fn generate_terminator(&mut self, term: &Terminator, func_name: &str) {
        match term {
            Terminator::Return(val) => {
                if let Some(val) = val {
                    self.load_operand_x0(val);
                }
                // Epilogue: restore fp, lr, deallocate frame
                self.emit_line("ldp x29, x30, [sp]");
                let aligned_stack = (self.stack_size + 15) & !15;
                self.emit_line(&format!("add sp, sp, #{}", aligned_stack));
                self.emit_line("ret");
            }
            Terminator::Jump(label) => {
                self.emit_line(&format!("b .L{}_{}", func_name, label));
            }
            Terminator::Branch {
                condition,
                true_label,
                false_label,
            } => {
                self.load_operand_x0(condition);
                self.emit_line("cmp x0, #0");
                self.emit_line(&format!("b.ne .L{}_{}", func_name, true_label));
                self.emit_line(&format!("b .L{}_{}", func_name, false_label));
            }
            Terminator::None => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir;
    use crate::lexer;
    use crate::parser;

    #[test]
    fn test_codegen_return_42() {
        let tokens = lexer::lex("int main() { return 42; }").unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = ir::lower(&program);
        let asm = generate(&module);
        assert!(asm.contains(".globl main"));
        assert!(asm.contains("main:"));
        assert!(asm.contains("42"));
        assert!(asm.contains("ret"));
    }

    #[test]
    fn test_codegen_with_printf() {
        let source = r#"
            int printf(const char *fmt, ...);
            int main() {
                printf("Hello, World!\n");
                return 0;
            }
        "#;
        let tokens = lexer::lex(source).unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = ir::lower(&program);
        let asm = generate(&module);
        assert!(asm.contains("printf"));
        assert!(asm.contains("Hello, World!"));
    }

    #[test]
    fn test_codegen_arithmetic() {
        let tokens = lexer::lex("int main() { return 2 + 3 * 4; }").unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = ir::lower(&program);
        let asm = generate(&module);
        assert!(asm.contains("ret"));
    }

    #[test]
    fn test_codegen_if_else() {
        let source = "int main() { if (1 > 0) { return 1; } else { return 0; } }";
        let tokens = lexer::lex(source).unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = ir::lower(&program);
        let asm = generate(&module);
        assert!(asm.contains("cmp") || asm.contains("cmpq"));
    }

    #[test]
    fn test_codegen_while_loop() {
        let source = "int main() { int i = 0; while (i < 10) { i = i + 1; } return i; }";
        let tokens = lexer::lex(source).unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = ir::lower(&program);
        let asm = generate(&module);
        // Should have loop labels
        assert!(asm.contains(".L"));
    }
}

#[cfg(test)]
mod label_tests {
    use super::*;
    use crate::ir;
    use crate::lexer;
    use crate::parser;

    fn verify_labels(asm: &str) {
        let mut defined = std::collections::HashSet::new();
        let mut referenced = std::collections::HashSet::new();
        for line in asm.lines() {
            let trimmed = line.trim();
            if trimmed.ends_with(':') && !trimmed.starts_with('.')
                || (trimmed.starts_with('.') && trimmed.ends_with(':'))
            {
                let label = trimmed.trim_end_matches(':').to_string();
                defined.insert(label);
            }
            for prefix in &["jmp ", "jne ", "je ", "jg ", "jl ", "jge ", "jle "] {
                if let Some(rest) = trimmed.strip_prefix(prefix) {
                    let label = rest.trim().to_string();
                    referenced.insert(label);
                }
            }
        }
        let missing: Vec<_> = referenced.difference(&defined).collect();
        assert!(
            missing.is_empty(),
            "Missing labels in assembly:\n{}\nMissing: {:?}",
            asm,
            missing
        );
    }

    #[test]
    fn test_nested_if_labels() {
        let source = "int main() { int x = 15; if (x > 10) { if (x > 20) { return 3; } else { return 2; } } else { return 1; } }";
        let tokens = lexer::lex(source).unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = ir::lower(&program);
        let asm = generate(&module);
        verify_labels(&asm);
    }

    #[test]
    fn test_while_loop_labels() {
        let source = "int main() { int x = 10; while (x > 0) { x = x - 1; } return x; }";
        let tokens = lexer::lex(source).unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = ir::lower(&program);
        let asm = generate(&module);
        verify_labels(&asm);
    }

    #[test]
    fn test_for_loop_labels() {
        let source = "int main() { int s = 0; for (int i = 0; i < 10; i++) { s += i; } return s; }";
        let tokens = lexer::lex(source).unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = ir::lower(&program);
        let asm = generate(&module);
        verify_labels(&asm);
    }
}
