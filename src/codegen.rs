/// x86-64 code generator (AT&T syntax).
/// Naive: every virtual register gets a stack slot.
use crate::ir::{
    BasicBlock, CmpOp, Function, Instruction, IrBinOp, IrUnaryOp, Module, Operand, Terminator, VReg,
};
use std::collections::HashMap;
use std::fmt::Write;

/// System V AMD64 ABI argument registers
const ARG_REGS: [&str; 6] = ["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];

struct CodeGen {
    output: String,
    /// Map from vreg to stack offset (negative from rbp)
    stack_slots: HashMap<VReg, i32>,
    stack_size: i32,
}

impl CodeGen {
    fn new() -> Self {
        CodeGen {
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

    /// Load an operand into %rax
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

    /// Load an operand into %rcx
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

    /// Store %rax to a vreg's stack slot
    fn store_rax(&mut self, vreg: VReg) {
        let offset = self.slot_for(vreg);
        self.emit_line(&format!("movq %rax, {}(%rbp)", offset));
    }

    fn generate_module(&mut self, module: &Module) {
        // String literals in .rodata
        if !module.string_literals.is_empty() {
            self.emit_raw("    .section .rodata");
            for (label, value) in &module.string_literals {
                self.emit_label(label);
                // Emit as escaped string
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

        // Pre-allocate all vreg stack slots
        for vreg in 0..func.num_vregs {
            self.slot_for(vreg);
        }

        // Align stack to 16 bytes
        let aligned_stack = (self.stack_size + 15) & !15;

        self.emit_raw(&format!("    .globl {}", func.name));
        self.emit_label(&func.name);

        // Prologue
        self.emit_line("pushq %rbp");
        self.emit_line("movq %rsp, %rbp");
        if aligned_stack > 0 {
            self.emit_line(&format!("subq ${}, %rsp", aligned_stack));
        }

        // Save arguments to stack slots
        for (i, param_name) in func.params.iter().enumerate() {
            if i < ARG_REGS.len() && !param_name.is_empty() {
                if let Some(&vreg) = func.locals.get(param_name) {
                    let offset = self.slot_for(vreg);
                    self.emit_line(&format!("movq {}, {}(%rbp)", ARG_REGS[i], offset));
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
                        self.emit_line("cqto"); // sign-extend rax into rdx:rax
                        self.emit_line("idivq %rcx");
                    }
                    IrBinOp::Mod => {
                        self.emit_line("cqto");
                        self.emit_line("idivq %rcx");
                        self.emit_line("movq %rdx, %rax"); // remainder in rdx
                    }
                    IrBinOp::And => self.emit_line("andq %rcx, %rax"),
                    IrBinOp::Or => self.emit_line("orq %rcx, %rax"),
                    IrBinOp::Xor => self.emit_line("xorq %rcx, %rax"),
                    IrBinOp::Shl => {
                        // shift amount must be in %cl
                        self.emit_line("shlq %cl, %rax");
                    }
                    IrBinOp::Shr => {
                        self.emit_line("sarq %cl, %rax");
                    }
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
                // Push args into argument registers (System V ABI)
                // Need to be careful about register clobbering
                // First, push all args to stack, then pop into arg regs
                let num_reg_args = args.len().min(ARG_REGS.len());

                // Evaluate and store args on stack temporarily
                let mut arg_temps = Vec::new();
                for arg in args {
                    self.load_operand_rax(arg);
                    self.emit_line("pushq %rax");
                    arg_temps.push(());
                }

                // Pop into arg registers in reverse order
                for i in (0..args.len()).rev() {
                    if i < ARG_REGS.len() {
                        self.emit_line(&format!("popq {}", ARG_REGS[i]));
                    } else {
                        // Stack args stay on stack - but we need to handle this properly
                        // For now, just pop and discard extras
                        self.emit_line("popq %rax");
                    }
                }

                // For variadic functions (like printf), set %al = number of vector args (0)
                self.emit_line("xorl %eax, %eax");

                // Align stack to 16 bytes before call if needed
                // The stack is already 16-byte aligned after prologue if we haven't pushed odd items
                let stack_args = if args.len() > ARG_REGS.len() {
                    args.len() - ARG_REGS.len()
                } else {
                    0
                };
                let _ = (stack_args, num_reg_args, arg_temps);

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
            Terminator::None => {
                // Should not happen in well-formed IR
            }
        }
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
                // Emit as octal escape
                let b = c as u32;
                write!(result, "\\{:03o}", b).unwrap();
            }
        }
    }
    result
}

pub fn generate(module: &Module) -> String {
    let mut codegen = CodeGen::new();
    codegen.generate_module(module);
    codegen.output
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
        assert!(asm.contains("$42"));
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
        assert!(asm.contains("call printf"));
        assert!(asm.contains("Hello, World!"));
    }
}
