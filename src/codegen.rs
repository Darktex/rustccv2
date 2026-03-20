use crate::parser::{
    BinaryOp, Block, Declaration, Expr, Function, Program, Stmt, Type, UnaryOp, VarDecl,
};
use std::collections::HashMap;

/// Label counter for unique labels.
struct CodeGen {
    output: String,
    label_counter: usize,
    /// Maps variable name -> stack offset (negative from rbp)
    locals: HashMap<String, i64>,
    /// Current stack offset for next variable allocation
    stack_offset: i64,
    /// String literals: (label, content)
    string_literals: Vec<(String, String)>,
    /// String literal counter
    string_counter: usize,
    /// Break label stack (for loops)
    break_labels: Vec<String>,
    /// Continue label stack (for loops)
    continue_labels: Vec<String>,
}

impl CodeGen {
    fn new() -> Self {
        Self {
            output: String::new(),
            label_counter: 0,
            locals: HashMap::new(),
            stack_offset: 0,
            string_literals: Vec::new(),
            string_counter: 0,
            break_labels: Vec::new(),
            continue_labels: Vec::new(),
        }
    }

    fn new_label(&mut self) -> String {
        let label = format!(".L{}", self.label_counter);
        self.label_counter += 1;
        label
    }

    fn new_string_label(&mut self) -> String {
        let label = format!(".LC{}", self.string_counter);
        self.string_counter += 1;
        label
    }

    fn emit(&mut self, line: &str) {
        self.output.push_str(line);
        self.output.push('\n');
    }

    fn emit_indent(&mut self, line: &str) {
        self.output.push_str("    ");
        self.output.push_str(line);
        self.output.push('\n');
    }

    fn alloc_local(&mut self, name: &str) -> i64 {
        self.stack_offset -= 8;
        let offset = self.stack_offset;
        self.locals.insert(name.to_string(), offset);
        offset
    }

    fn generate_program(&mut self, program: &Program) {
        // Forward scan for string literals
        for decl in &program.declarations {
            if let Declaration::Function(func) = decl {
                if let Some(body) = &func.body {
                    self.collect_strings_block(body);
                }
            }
        }

        // Emit string literals in .rodata
        if !self.string_literals.is_empty() {
            self.emit("    .section .rodata");
            let literals: Vec<_> = self.string_literals.clone();
            for (label, content) in &literals {
                self.emit(&format!("{label}:"));
                // Escape the string for assembly
                let escaped = escape_for_asm(content);
                self.emit_indent(&format!(".string \"{escaped}\""));
            }
        }

        self.emit("    .text");

        for decl in &program.declarations {
            match decl {
                Declaration::Function(func) => {
                    if func.body.is_some() {
                        self.generate_function(func);
                    }
                }
                Declaration::GlobalVar(var) => {
                    self.generate_global_var(var);
                }
            }
        }
    }

    fn collect_strings_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.collect_strings_stmt(stmt);
        }
    }

    fn collect_strings_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Return(Some(expr)) | Stmt::Expr(expr) => self.collect_strings_expr(expr),
            Stmt::VarDecl(v) => {
                if let Some(init) = &v.init {
                    self.collect_strings_expr(init);
                }
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.collect_strings_expr(cond);
                self.collect_strings_stmt(then_branch);
                if let Some(eb) = else_branch {
                    self.collect_strings_stmt(eb);
                }
            }
            Stmt::While { cond, body } => {
                self.collect_strings_expr(cond);
                self.collect_strings_stmt(body);
            }
            Stmt::For {
                init,
                cond,
                update,
                body,
            } => {
                if let Some(i) = init {
                    self.collect_strings_stmt(i);
                }
                if let Some(c) = cond {
                    self.collect_strings_expr(c);
                }
                if let Some(u) = update {
                    self.collect_strings_expr(u);
                }
                self.collect_strings_stmt(body);
            }
            Stmt::DoWhile { body, cond } => {
                self.collect_strings_stmt(body);
                self.collect_strings_expr(cond);
            }
            Stmt::Block(b) => self.collect_strings_block(b),
            _ => {}
        }
    }

    fn collect_strings_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::StringLiteral(s) => {
                let label = self.new_string_label();
                self.string_literals.push((label, s.clone()));
            }
            Expr::Binary(_, l, r) | Expr::Assign(l, r) | Expr::CompoundAssign(_, l, r) => {
                self.collect_strings_expr(l);
                self.collect_strings_expr(r);
            }
            Expr::Unary(_, e)
            | Expr::PreIncrement(e)
            | Expr::PreDecrement(e)
            | Expr::PostIncrement(e)
            | Expr::PostDecrement(e)
            | Expr::Deref(e)
            | Expr::AddrOf(e) => {
                self.collect_strings_expr(e);
            }
            Expr::Call(_, args) => {
                for a in args {
                    self.collect_strings_expr(a);
                }
            }
            Expr::Ternary(c, t, e) => {
                self.collect_strings_expr(c);
                self.collect_strings_expr(t);
                self.collect_strings_expr(e);
            }
            Expr::ArrayAccess(a, i) => {
                self.collect_strings_expr(a);
                self.collect_strings_expr(i);
            }
            Expr::Cast(_, e) => self.collect_strings_expr(e),
            _ => {}
        }
    }

    fn generate_global_var(&mut self, var: &VarDecl) {
        self.emit("    .data");
        self.emit(&format!("    .globl {}", var.name));
        self.emit(&format!("{}:", var.name));
        if let Some(Expr::IntLiteral(n)) = &var.init {
            self.emit_indent(&format!(".quad {n}"));
        } else {
            self.emit_indent(".quad 0");
        }
    }

    fn generate_function(&mut self, func: &Function) {
        self.locals.clear();
        self.stack_offset = 0;

        self.emit(&format!("    .globl {}", func.name));
        self.emit(&format!("{}:", func.name));

        // Prologue
        self.emit_indent("pushq %rbp");
        self.emit_indent("movq %rsp, %rbp");

        // Count locals to pre-allocate stack space
        let local_count = count_locals_in_body(func.body.as_ref().unwrap()) + func.params.len();
        let stack_size = align16((local_count as i64) * 8);
        if stack_size > 0 {
            self.emit_indent(&format!("subq ${stack_size}, %rsp"));
        }

        // Store parameters
        let param_regs = ["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];
        for (i, param) in func.params.iter().enumerate() {
            if i < param_regs.len() {
                let offset = self.alloc_local(&param.name);
                self.emit_indent(&format!("movq {}, {offset}(%rbp)", param_regs[i]));
            }
        }

        // Generate body
        let body = func.body.as_ref().unwrap();
        self.generate_block(body);

        // Default return 0 (for main)
        self.emit_indent("movq $0, %rax");
        self.emit_indent("movq %rbp, %rsp");
        self.emit_indent("popq %rbp");
        self.emit_indent("ret");
    }

    fn generate_block(&mut self, block: &Block) {
        for stmt in &block.stmts {
            self.generate_stmt(stmt);
        }
    }

    fn generate_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.generate_expr(e);
                } else {
                    self.emit_indent("movq $0, %rax");
                }
                self.emit_indent("movq %rbp, %rsp");
                self.emit_indent("popq %rbp");
                self.emit_indent("ret");
            }
            Stmt::Expr(expr) => {
                self.generate_expr(expr);
            }
            Stmt::VarDecl(var) => {
                let offset = self.alloc_local(&var.name);
                if let Some(init) = &var.init {
                    self.generate_expr(init);
                    self.emit_indent(&format!("movq %rax, {offset}(%rbp)"));
                } else {
                    self.emit_indent(&format!("movq $0, {offset}(%rbp)"));
                }
            }
            Stmt::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let else_label = self.new_label();
                let end_label = self.new_label();

                self.generate_expr(cond);
                self.emit_indent("cmpq $0, %rax");
                if else_branch.is_some() {
                    self.emit_indent(&format!("je {else_label}"));
                } else {
                    self.emit_indent(&format!("je {end_label}"));
                }

                self.generate_stmt(then_branch);

                if else_branch.is_some() {
                    self.emit_indent(&format!("jmp {end_label}"));
                    self.emit(&format!("{else_label}:"));
                    self.generate_stmt(else_branch.as_ref().unwrap());
                }

                self.emit(&format!("{end_label}:"));
            }
            Stmt::While { cond, body } => {
                let start_label = self.new_label();
                let end_label = self.new_label();

                self.continue_labels.push(start_label.clone());
                self.break_labels.push(end_label.clone());

                self.emit(&format!("{start_label}:"));
                self.generate_expr(cond);
                self.emit_indent("cmpq $0, %rax");
                self.emit_indent(&format!("je {end_label}"));

                self.generate_stmt(body);
                self.emit_indent(&format!("jmp {start_label}"));
                self.emit(&format!("{end_label}:"));

                self.continue_labels.pop();
                self.break_labels.pop();
            }
            Stmt::For {
                init,
                cond,
                update,
                body,
            } => {
                let start_label = self.new_label();
                let continue_label = self.new_label();
                let end_label = self.new_label();

                if let Some(init) = init {
                    self.generate_stmt(init);
                }

                self.continue_labels.push(continue_label.clone());
                self.break_labels.push(end_label.clone());

                self.emit(&format!("{start_label}:"));
                if let Some(cond) = cond {
                    self.generate_expr(cond);
                    self.emit_indent("cmpq $0, %rax");
                    self.emit_indent(&format!("je {end_label}"));
                }

                self.generate_stmt(body);

                self.emit(&format!("{continue_label}:"));
                if let Some(update) = update {
                    self.generate_expr(update);
                }
                self.emit_indent(&format!("jmp {start_label}"));
                self.emit(&format!("{end_label}:"));

                self.continue_labels.pop();
                self.break_labels.pop();
            }
            Stmt::DoWhile { body, cond } => {
                let start_label = self.new_label();
                let continue_label = self.new_label();
                let end_label = self.new_label();

                self.continue_labels.push(continue_label.clone());
                self.break_labels.push(end_label.clone());

                self.emit(&format!("{start_label}:"));
                self.generate_stmt(body);

                self.emit(&format!("{continue_label}:"));
                self.generate_expr(cond);
                self.emit_indent("cmpq $0, %rax");
                self.emit_indent(&format!("jne {start_label}"));
                self.emit(&format!("{end_label}:"));

                self.continue_labels.pop();
                self.break_labels.pop();
            }
            Stmt::Block(block) => {
                self.generate_block(block);
            }
            Stmt::Break => {
                if let Some(label) = self.break_labels.last() {
                    let label = label.clone();
                    self.emit_indent(&format!("jmp {label}"));
                }
            }
            Stmt::Continue => {
                if let Some(label) = self.continue_labels.last() {
                    let label = label.clone();
                    self.emit_indent(&format!("jmp {label}"));
                }
            }
            Stmt::Empty => {}
        }
    }

    fn generate_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::IntLiteral(n) => {
                self.emit_indent(&format!("movq ${n}, %rax"));
            }
            Expr::CharLiteral(c) => {
                self.emit_indent(&format!("movq ${}, %rax", *c as i64));
            }
            Expr::StringLiteral(s) => {
                // Find the label for this string
                let label = self
                    .string_literals
                    .iter()
                    .find(|(_, content)| content == s)
                    .map(|(l, _)| l.clone())
                    .unwrap_or_else(|| ".LC_unknown".to_string());
                self.emit_indent(&format!("leaq {label}(%rip), %rax"));
            }
            Expr::Ident(name) => {
                if let Some(&offset) = self.locals.get(name) {
                    self.emit_indent(&format!("movq {offset}(%rbp), %rax"));
                } else {
                    // Global variable
                    self.emit_indent(&format!("movq {name}(%rip), %rax"));
                }
            }
            Expr::Assign(lhs, rhs) => {
                self.generate_expr(rhs);
                match lhs.as_ref() {
                    Expr::Ident(name) => {
                        if let Some(&offset) = self.locals.get(name) {
                            self.emit_indent(&format!("movq %rax, {offset}(%rbp)"));
                        } else {
                            self.emit_indent(&format!("movq %rax, {name}(%rip)"));
                        }
                    }
                    Expr::Deref(inner) => {
                        self.emit_indent("pushq %rax"); // save rhs value
                        self.generate_expr(inner);
                        self.emit_indent("movq %rax, %rcx"); // address in rcx
                        self.emit_indent("popq %rax"); // restore rhs value
                        self.emit_indent("movq %rax, (%rcx)");
                    }
                    Expr::ArrayAccess(base, index) => {
                        self.emit_indent("pushq %rax"); // save rhs value
                        self.generate_expr(index);
                        self.emit_indent("pushq %rax"); // save index
                        self.generate_expr(base);
                        self.emit_indent("popq %rcx"); // index
                        self.emit_indent("leaq (%rax,%rcx,8), %rcx"); // address
                        self.emit_indent("popq %rax"); // restore rhs value
                        self.emit_indent("movq %rax, (%rcx)");
                    }
                    _ => {}
                }
            }
            Expr::CompoundAssign(op, lhs, rhs) => {
                // Evaluate lhs, push, evaluate rhs, perform op, store back
                self.generate_expr(rhs);
                self.emit_indent("pushq %rax");
                self.generate_expr(lhs);
                self.emit_indent("popq %rcx");
                self.generate_binary_op(*op);
                // Store back
                if let Expr::Ident(name) = lhs.as_ref() {
                    if let Some(&offset) = self.locals.get(name) {
                        self.emit_indent(&format!("movq %rax, {offset}(%rbp)"));
                    } else {
                        self.emit_indent(&format!("movq %rax, {name}(%rip)"));
                    }
                }
            }
            Expr::Binary(op, lhs, rhs) => {
                // Short-circuit for && and ||
                match op {
                    BinaryOp::And => {
                        let false_label = self.new_label();
                        let end_label = self.new_label();
                        self.generate_expr(lhs);
                        self.emit_indent("cmpq $0, %rax");
                        self.emit_indent(&format!("je {false_label}"));
                        self.generate_expr(rhs);
                        self.emit_indent("cmpq $0, %rax");
                        self.emit_indent(&format!("je {false_label}"));
                        self.emit_indent("movq $1, %rax");
                        self.emit_indent(&format!("jmp {end_label}"));
                        self.emit(&format!("{false_label}:"));
                        self.emit_indent("movq $0, %rax");
                        self.emit(&format!("{end_label}:"));
                        return;
                    }
                    BinaryOp::Or => {
                        let true_label = self.new_label();
                        let end_label = self.new_label();
                        self.generate_expr(lhs);
                        self.emit_indent("cmpq $0, %rax");
                        self.emit_indent(&format!("jne {true_label}"));
                        self.generate_expr(rhs);
                        self.emit_indent("cmpq $0, %rax");
                        self.emit_indent(&format!("jne {true_label}"));
                        self.emit_indent("movq $0, %rax");
                        self.emit_indent(&format!("jmp {end_label}"));
                        self.emit(&format!("{true_label}:"));
                        self.emit_indent("movq $1, %rax");
                        self.emit(&format!("{end_label}:"));
                        return;
                    }
                    _ => {}
                }

                self.generate_expr(rhs);
                self.emit_indent("pushq %rax");
                self.generate_expr(lhs);
                self.emit_indent("popq %rcx");
                self.generate_binary_op(*op);
            }
            Expr::Unary(op, inner) => {
                self.generate_expr(inner);
                match op {
                    UnaryOp::Neg => self.emit_indent("negq %rax"),
                    UnaryOp::BitNot => self.emit_indent("notq %rax"),
                    UnaryOp::LogNot => {
                        self.emit_indent("cmpq $0, %rax");
                        self.emit_indent("sete %al");
                        self.emit_indent("movzbq %al, %rax");
                    }
                }
            }
            Expr::PreIncrement(inner) => {
                if let Expr::Ident(name) = inner.as_ref() {
                    if let Some(&offset) = self.locals.get(name) {
                        self.emit_indent(&format!("addq $1, {offset}(%rbp)"));
                        self.emit_indent(&format!("movq {offset}(%rbp), %rax"));
                    }
                }
            }
            Expr::PreDecrement(inner) => {
                if let Expr::Ident(name) = inner.as_ref() {
                    if let Some(&offset) = self.locals.get(name) {
                        self.emit_indent(&format!("subq $1, {offset}(%rbp)"));
                        self.emit_indent(&format!("movq {offset}(%rbp), %rax"));
                    }
                }
            }
            Expr::PostIncrement(inner) => {
                if let Expr::Ident(name) = inner.as_ref() {
                    if let Some(&offset) = self.locals.get(name) {
                        self.emit_indent(&format!("movq {offset}(%rbp), %rax"));
                        self.emit_indent(&format!("addq $1, {offset}(%rbp)"));
                    }
                }
            }
            Expr::PostDecrement(inner) => {
                if let Expr::Ident(name) = inner.as_ref() {
                    if let Some(&offset) = self.locals.get(name) {
                        self.emit_indent(&format!("movq {offset}(%rbp), %rax"));
                        self.emit_indent(&format!("subq $1, {offset}(%rbp)"));
                    }
                }
            }
            Expr::Call(name, args) => {
                // System V AMD64 ABI: rdi, rsi, rdx, rcx, r8, r9
                let arg_regs = ["%rdi", "%rsi", "%rdx", "%rcx", "%r8", "%r9"];

                // Align stack for function call (must be 16-byte aligned before call)
                let stack_args = if args.len() > 6 { args.len() - 6 } else { 0 };
                let needs_alignment = stack_args % 2 != 0;
                if needs_alignment {
                    self.emit_indent("subq $8, %rsp");
                }

                // Push stack args in reverse order (args 7+)
                for i in (6..args.len()).rev() {
                    self.generate_expr(&args[i]);
                    self.emit_indent("pushq %rax");
                }

                // Evaluate register args in reverse, pushing to stack
                let reg_count = args.len().min(6);
                for i in (0..reg_count).rev() {
                    self.generate_expr(&args[i]);
                    self.emit_indent("pushq %rax");
                }
                // Pop into registers in forward order
                for reg in arg_regs.iter().take(reg_count) {
                    self.emit_indent(&format!("popq {reg}"));
                }

                // For variadic functions (like printf), set %al = 0 (no floating point args)
                self.emit_indent("movq $0, %rax");
                self.emit_indent(&format!("call {name}"));

                // Clean up stack args
                if stack_args > 0 || needs_alignment {
                    let cleanup = (stack_args * 8) + if needs_alignment { 8 } else { 0 };
                    self.emit_indent(&format!("addq ${cleanup}, %rsp"));
                }
            }
            Expr::Ternary(cond, then_expr, else_expr) => {
                let else_label = self.new_label();
                let end_label = self.new_label();
                self.generate_expr(cond);
                self.emit_indent("cmpq $0, %rax");
                self.emit_indent(&format!("je {else_label}"));
                self.generate_expr(then_expr);
                self.emit_indent(&format!("jmp {end_label}"));
                self.emit(&format!("{else_label}:"));
                self.generate_expr(else_expr);
                self.emit(&format!("{end_label}:"));
            }
            Expr::Sizeof(ty) => {
                let size = type_size(ty);
                self.emit_indent(&format!("movq ${size}, %rax"));
            }
            Expr::Cast(_, inner) => {
                // For now, just generate the inner expression
                self.generate_expr(inner);
            }
            Expr::Deref(inner) => {
                self.generate_expr(inner);
                self.emit_indent("movq (%rax), %rax");
            }
            Expr::AddrOf(inner) => {
                if let Expr::Ident(name) = inner.as_ref() {
                    if let Some(&offset) = self.locals.get(name) {
                        self.emit_indent(&format!("leaq {offset}(%rbp), %rax"));
                    } else {
                        self.emit_indent(&format!("leaq {name}(%rip), %rax"));
                    }
                }
            }
            Expr::ArrayAccess(base, index) => {
                self.generate_expr(index);
                self.emit_indent("pushq %rax");
                self.generate_expr(base);
                self.emit_indent("popq %rcx");
                self.emit_indent("movq (%rax,%rcx,8), %rax");
            }
        }
    }

    fn generate_binary_op(&mut self, op: BinaryOp) {
        // At this point: lhs in %rax, rhs in %rcx
        match op {
            BinaryOp::Add => self.emit_indent("addq %rcx, %rax"),
            BinaryOp::Sub => self.emit_indent("subq %rcx, %rax"),
            BinaryOp::Mul => self.emit_indent("imulq %rcx, %rax"),
            BinaryOp::Div => {
                self.emit_indent("cqto"); // sign-extend rax into rdx:rax
                self.emit_indent("idivq %rcx");
            }
            BinaryOp::Mod => {
                self.emit_indent("cqto");
                self.emit_indent("idivq %rcx");
                self.emit_indent("movq %rdx, %rax"); // remainder is in rdx
            }
            BinaryOp::BitAnd => self.emit_indent("andq %rcx, %rax"),
            BinaryOp::BitOr => self.emit_indent("orq %rcx, %rax"),
            BinaryOp::BitXor => self.emit_indent("xorq %rcx, %rax"),
            BinaryOp::Shl => self.emit_indent("shlq %cl, %rax"),
            BinaryOp::Shr => self.emit_indent("sarq %cl, %rax"),
            BinaryOp::Eq => {
                self.emit_indent("cmpq %rcx, %rax");
                self.emit_indent("sete %al");
                self.emit_indent("movzbq %al, %rax");
            }
            BinaryOp::Neq => {
                self.emit_indent("cmpq %rcx, %rax");
                self.emit_indent("setne %al");
                self.emit_indent("movzbq %al, %rax");
            }
            BinaryOp::Lt => {
                self.emit_indent("cmpq %rcx, %rax");
                self.emit_indent("setl %al");
                self.emit_indent("movzbq %al, %rax");
            }
            BinaryOp::Gt => {
                self.emit_indent("cmpq %rcx, %rax");
                self.emit_indent("setg %al");
                self.emit_indent("movzbq %al, %rax");
            }
            BinaryOp::Le => {
                self.emit_indent("cmpq %rcx, %rax");
                self.emit_indent("setle %al");
                self.emit_indent("movzbq %al, %rax");
            }
            BinaryOp::Ge => {
                self.emit_indent("cmpq %rcx, %rax");
                self.emit_indent("setge %al");
                self.emit_indent("movzbq %al, %rax");
            }
            // And/Or are handled with short-circuit above
            BinaryOp::And | BinaryOp::Or => unreachable!(),
        }
    }
}

fn escape_for_asm(s: &str) -> String {
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
            c => out.push_str(&format!("\\{:03o}", c as u32)),
        }
    }
    out
}

fn type_size(ty: &Type) -> i64 {
    match ty {
        Type::Char => 1,
        Type::Short => 2,
        Type::Int | Type::Unsigned => 4,
        Type::Long | Type::Pointer(_) => 8,
        Type::Void => 0,
    }
}

fn align16(n: i64) -> i64 {
    (n + 15) & !15
}

fn count_locals_in_body(block: &Block) -> usize {
    let mut count = 0;
    for stmt in &block.stmts {
        count += count_locals_in_stmt(stmt);
    }
    count
}

fn count_locals_in_stmt(stmt: &Stmt) -> usize {
    match stmt {
        Stmt::VarDecl(_) => 1,
        Stmt::Block(b) => count_locals_in_body(b),
        Stmt::If {
            then_branch,
            else_branch,
            ..
        } => {
            count_locals_in_stmt(then_branch)
                + else_branch.as_ref().map_or(0, |e| count_locals_in_stmt(e))
        }
        Stmt::While { body, .. } => count_locals_in_stmt(body),
        Stmt::For { init, body, .. } => {
            init.as_ref().map_or(0, |i| count_locals_in_stmt(i)) + count_locals_in_stmt(body)
        }
        Stmt::DoWhile { body, .. } => count_locals_in_stmt(body),
        _ => 0,
    }
}

pub fn generate(program: &Program) -> String {
    let mut gen = CodeGen::new();
    gen.generate_program(program);
    gen.output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer;
    use crate::parser;

    fn compile(src: &str) -> String {
        let tokens = lexer::lex(src).unwrap();
        let program = parser::parse(&tokens).unwrap();
        generate(&program)
    }

    #[test]
    fn test_return_constant() {
        let asm = compile("int main() { return 42; }");
        assert!(asm.contains("main:"));
        assert!(asm.contains("$42"));
        assert!(asm.contains("ret"));
    }

    #[test]
    fn test_arithmetic() {
        let asm = compile("int main() { return 2 + 3; }");
        assert!(asm.contains("addq"));
    }

    #[test]
    fn test_function_call() {
        let asm = compile(r#"int main() { printf("hello\n"); return 0; }"#);
        assert!(asm.contains("call printf"));
    }

    #[test]
    fn test_if_else() {
        let asm = compile("int main() { if (1) { return 1; } else { return 0; } }");
        assert!(asm.contains("je"));
        assert!(asm.contains("jmp"));
    }

    #[test]
    fn test_while_loop() {
        let asm = compile("int main() { int i = 0; while (i < 10) { i++; } return i; }");
        assert!(asm.contains(".L"));
    }

    #[test]
    fn test_for_loop() {
        let asm = compile(
            "int main() { int sum = 0; for (int i = 0; i < 10; i++) { sum += i; } return sum; }",
        );
        assert!(asm.contains(".L"));
    }
}
