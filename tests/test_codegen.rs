//! Tests for the x86-64 code generator.

use rustcc::codegen::X86_64Generator;
use rustcc::ir::*;

#[test]
fn test_codegen_return_constant() {
    let mut builder = IrBuilder::new();
    builder.begin_function("main");
    let bb = builder.create_block();
    builder.switch_to_block(bb);
    let v = builder.build_constant(42);
    builder.build_ret(Some(v));
    let module = builder.finish();

    let mut gen = X86_64Generator::new();
    let asm = gen.generate(&module);

    // Should contain function label
    assert!(asm.contains(".globl main"), "Missing .globl main");
    assert!(asm.contains("main:"), "Missing main:");

    // Should have prologue
    assert!(asm.contains("pushq %rbp"), "Missing pushq %rbp");
    assert!(asm.contains("movq %rsp, %rbp"), "Missing stack frame setup");

    // Should load constant 42
    assert!(asm.contains("$42"), "Missing constant 42");

    // Should have return
    assert!(asm.contains("ret"), "Missing ret");
}

#[test]
fn test_codegen_arithmetic() {
    let mut builder = IrBuilder::new();
    builder.begin_function("arith");
    let bb = builder.create_block();
    builder.switch_to_block(bb);

    let a = builder.build_constant(10);
    let b = builder.build_constant(20);
    let sum = builder.build_binop(BinOp::Add, a, b);
    builder.build_ret(Some(sum));
    let module = builder.finish();

    let mut gen = X86_64Generator::new();
    let asm = gen.generate(&module);

    assert!(asm.contains("addq"), "Missing add instruction");
    assert!(asm.contains("$10"), "Missing constant 10");
    assert!(asm.contains("$20"), "Missing constant 20");
}

#[test]
fn test_codegen_subtraction() {
    let mut builder = IrBuilder::new();
    builder.begin_function("sub_test");
    let bb = builder.create_block();
    builder.switch_to_block(bb);

    let a = builder.build_constant(30);
    let b = builder.build_constant(10);
    let diff = builder.build_binop(BinOp::Sub, a, b);
    builder.build_ret(Some(diff));
    let module = builder.finish();

    let mut gen = X86_64Generator::new();
    let asm = gen.generate(&module);
    assert!(asm.contains("subq"), "Missing sub instruction");
}

#[test]
fn test_codegen_multiply() {
    let mut builder = IrBuilder::new();
    builder.begin_function("mul_test");
    let bb = builder.create_block();
    builder.switch_to_block(bb);

    let a = builder.build_constant(6);
    let b = builder.build_constant(7);
    let prod = builder.build_binop(BinOp::Mul, a, b);
    builder.build_ret(Some(prod));
    let module = builder.finish();

    let mut gen = X86_64Generator::new();
    let asm = gen.generate(&module);
    assert!(asm.contains("imulq"), "Missing imul instruction");
}

#[test]
fn test_codegen_division() {
    let mut builder = IrBuilder::new();
    builder.begin_function("div_test");
    let bb = builder.create_block();
    builder.switch_to_block(bb);

    let a = builder.build_constant(42);
    let b = builder.build_constant(6);
    let quot = builder.build_binop(BinOp::Div, a, b);
    builder.build_ret(Some(quot));
    let module = builder.finish();

    let mut gen = X86_64Generator::new();
    let asm = gen.generate(&module);
    assert!(asm.contains("idivq"), "Missing idiv instruction");
    assert!(asm.contains("cqto"), "Missing sign-extend for division");
}

#[test]
fn test_codegen_comparison() {
    let mut builder = IrBuilder::new();
    builder.begin_function("cmp_test");
    let bb = builder.create_block();
    builder.switch_to_block(bb);

    let a = builder.build_constant(5);
    let b = builder.build_constant(10);
    let result = builder.build_cmp(CmpOp::Lt, a, b);
    builder.build_ret(Some(result));
    let module = builder.finish();

    let mut gen = X86_64Generator::new();
    let asm = gen.generate(&module);
    assert!(asm.contains("cmpq"), "Missing cmp instruction");
    assert!(asm.contains("setl"), "Missing setl for less-than");
    assert!(asm.contains("movzbq"), "Missing zero-extend");
}

#[test]
fn test_codegen_conditional_branch() {
    let mut builder = IrBuilder::new();
    builder.begin_function("branch_test");

    let entry = builder.create_block();
    let then_bb = builder.create_block();
    let else_bb = builder.create_block();

    builder.switch_to_block(entry);
    let cond = builder.build_constant(1);
    builder.build_cond_branch(cond, then_bb, else_bb);

    builder.switch_to_block(then_bb);
    let v1 = builder.build_constant(1);
    builder.build_ret(Some(v1));

    builder.switch_to_block(else_bb);
    let v2 = builder.build_constant(0);
    builder.build_ret(Some(v2));

    let module = builder.finish();

    let mut gen = X86_64Generator::new();
    let asm = gen.generate(&module);

    assert!(asm.contains("jne"), "Missing conditional jump");
    assert!(asm.contains("jmp"), "Missing unconditional jump");
}

#[test]
fn test_codegen_function_call() {
    let mut builder = IrBuilder::new();
    builder.begin_function("call_test");
    let bb = builder.create_block();
    builder.switch_to_block(bb);

    let str_name = builder.add_string_literal("Hello\n");
    let str_ref = builder.build_global_ref(&str_name);
    builder.build_call_void("printf", vec![str_ref]);
    let zero = builder.build_constant(0);
    builder.build_ret(Some(zero));

    let module = builder.finish();

    let mut gen = X86_64Generator::new();
    let asm = gen.generate(&module);

    // Should have .rodata section with string
    assert!(asm.contains(".section .rodata"), "Missing .rodata section");
    assert!(asm.contains(".asciz"), "Missing string literal");
    assert!(asm.contains("Hello\\n"), "Missing string content");

    // Should call printf via PLT
    assert!(asm.contains("call printf@PLT"), "Missing printf call");

    // Should pass arg in %rdi
    assert!(asm.contains("%rdi"), "Missing argument register usage");
}

#[test]
fn test_codegen_unary_neg() {
    let mut builder = IrBuilder::new();
    builder.begin_function("neg_test");
    let bb = builder.create_block();
    builder.switch_to_block(bb);

    let v = builder.build_constant(5);
    let neg = builder.build_unaryop(UnaryOp::Neg, v);
    builder.build_ret(Some(neg));
    let module = builder.finish();

    let mut gen = X86_64Generator::new();
    let asm = gen.generate(&module);
    assert!(asm.contains("negq"), "Missing neg instruction");
}

#[test]
fn test_codegen_logical_not() {
    let mut builder = IrBuilder::new();
    builder.begin_function("not_test");
    let bb = builder.create_block();
    builder.switch_to_block(bb);

    let v = builder.build_constant(1);
    let not = builder.build_unaryop(UnaryOp::Not, v);
    builder.build_ret(Some(not));
    let module = builder.finish();

    let mut gen = X86_64Generator::new();
    let asm = gen.generate(&module);
    assert!(asm.contains("sete"), "Missing sete for logical not");
}

#[test]
fn test_codegen_with_params() {
    let mut builder = IrBuilder::new();
    builder.begin_function("add");
    let p0 = builder.add_param();
    let p1 = builder.add_param();

    let bb = builder.create_block();
    builder.switch_to_block(bb);
    let sum = builder.build_binop(BinOp::Add, p0, p1);
    builder.build_ret(Some(sum));
    let module = builder.finish();

    let mut gen = X86_64Generator::new();
    let asm = gen.generate(&module);

    // Parameters should be moved from arg registers to stack
    assert!(asm.contains("%rdi"), "Missing first arg register");
    assert!(asm.contains("%rsi"), "Missing second arg register");
}

#[test]
fn test_codegen_string_literal_global() {
    let mut builder = IrBuilder::new();

    // Add two string literals
    let s1 = builder.add_string_literal("hello");
    let s2 = builder.add_string_literal("world");

    builder.begin_function("test");
    let bb = builder.create_block();
    builder.switch_to_block(bb);
    let r1 = builder.build_global_ref(&s1);
    let r2 = builder.build_global_ref(&s2);
    let _ = (r1, r2);
    builder.build_ret(None);

    let module = builder.finish();

    let mut gen = X86_64Generator::new();
    let asm = gen.generate(&module);

    assert!(asm.contains(".str.0:"), "Missing first string label");
    assert!(asm.contains(".str.1:"), "Missing second string label");
    assert!(asm.contains("hello"), "Missing first string content");
    assert!(asm.contains("world"), "Missing second string content");
}
