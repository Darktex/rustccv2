//! Tests for the SSA IR and code generator.

// We need to make the crate a library to test it.
// For now, we test through the module structure.

use rustcc::ir::*;

#[test]
fn test_vreg_display() {
    assert_eq!(format!("{}", VReg(0)), "%0");
    assert_eq!(format!("{}", VReg(42)), "%42");
}

#[test]
fn test_block_id_display() {
    assert_eq!(format!("{}", BlockId(0)), "bb0");
    assert_eq!(format!("{}", BlockId(3)), "bb3");
}

#[test]
fn test_binop_display() {
    assert_eq!(format!("{}", BinOp::Add), "add");
    assert_eq!(format!("{}", BinOp::Sub), "sub");
    assert_eq!(format!("{}", BinOp::Mul), "mul");
    assert_eq!(format!("{}", BinOp::Div), "div");
    assert_eq!(format!("{}", BinOp::Mod), "mod");
}

#[test]
fn test_cmpop_display() {
    assert_eq!(format!("{}", CmpOp::Eq), "eq");
    assert_eq!(format!("{}", CmpOp::Ne), "ne");
    assert_eq!(format!("{}", CmpOp::Lt), "lt");
}

#[test]
fn test_builder_constant() {
    let mut builder = IrBuilder::new();
    builder.begin_function("main");
    let bb = builder.create_block();
    builder.switch_to_block(bb);

    let v = builder.build_constant(42);
    builder.build_ret(Some(v));

    let module = builder.finish();
    assert_eq!(module.functions.len(), 1);
    let func = &module.functions[0];
    assert_eq!(func.name, "main");
    assert_eq!(func.blocks.len(), 1);
    assert_eq!(func.blocks[0].instructions.len(), 1);

    match &func.blocks[0].instructions[0] {
        Instruction::Constant { dst, value } => {
            assert_eq!(*value, 42);
            assert_eq!(*dst, v);
        }
        _ => panic!("Expected Constant instruction"),
    }
}

#[test]
fn test_builder_arithmetic() {
    let mut builder = IrBuilder::new();
    builder.begin_function("add_nums");
    let bb = builder.create_block();
    builder.switch_to_block(bb);

    let a = builder.build_constant(10);
    let b = builder.build_constant(20);
    let sum = builder.build_binop(BinOp::Add, a, b);
    builder.build_ret(Some(sum));

    let module = builder.finish();
    let func = &module.functions[0];
    assert_eq!(func.blocks[0].instructions.len(), 3); // const, const, add
}

#[test]
fn test_builder_comparison() {
    let mut builder = IrBuilder::new();
    builder.begin_function("cmp_test");
    let bb = builder.create_block();
    builder.switch_to_block(bb);

    let a = builder.build_constant(5);
    let b = builder.build_constant(10);
    let result = builder.build_cmp(CmpOp::Lt, a, b);
    builder.build_ret(Some(result));

    let module = builder.finish();
    let func = &module.functions[0];
    assert_eq!(func.blocks[0].instructions.len(), 3);
}

#[test]
fn test_builder_branching() {
    let mut builder = IrBuilder::new();
    builder.begin_function("branch_test");

    let entry = builder.create_block();
    let then_bb = builder.create_block();
    let else_bb = builder.create_block();
    let merge_bb = builder.create_block();

    builder.switch_to_block(entry);
    let cond = builder.build_constant(1);
    builder.build_cond_branch(cond, then_bb, else_bb);

    builder.switch_to_block(then_bb);
    let v1 = builder.build_constant(1);
    builder.build_branch(merge_bb);

    builder.switch_to_block(else_bb);
    let v2 = builder.build_constant(0);
    builder.build_branch(merge_bb);

    builder.switch_to_block(merge_bb);
    let _phi = builder.build_phi(vec![(v1, then_bb), (v2, else_bb)]);
    builder.build_ret(Some(_phi));

    let module = builder.finish();
    let func = &module.functions[0];
    assert_eq!(func.blocks.len(), 4);

    // Check entry block terminator
    match func.blocks[0].terminator.as_ref().unwrap() {
        Terminator::CondBranch {
            true_bb, false_bb, ..
        } => {
            assert_eq!(*true_bb, then_bb);
            assert_eq!(*false_bb, else_bb);
        }
        _ => panic!("Expected CondBranch"),
    }
}

#[test]
fn test_builder_function_call() {
    let mut builder = IrBuilder::new();
    builder.begin_function("call_test");
    let bb = builder.create_block();
    builder.switch_to_block(bb);

    let str_name = builder.add_string_literal("Hello, World!\n");
    let str_ref = builder.build_global_ref(&str_name);
    builder.build_call_void("printf", vec![str_ref]);
    let zero = builder.build_constant(0);
    builder.build_ret(Some(zero));

    let module = builder.finish();
    assert_eq!(module.globals.len(), 1);
    assert_eq!(module.functions[0].blocks[0].instructions.len(), 3); // global_ref, call, const
}

#[test]
fn test_builder_alloca_load_store() {
    let mut builder = IrBuilder::new();
    builder.begin_function("alloca_test");
    let bb = builder.create_block();
    builder.switch_to_block(bb);

    let addr = builder.build_alloca(8);
    let val = builder.build_constant(42);
    builder.build_store(addr, val);
    let loaded = builder.build_load(addr);
    builder.build_ret(Some(loaded));

    let module = builder.finish();
    let func = &module.functions[0];
    assert_eq!(func.blocks[0].instructions.len(), 4); // alloca, const, store, load
}

#[test]
fn test_builder_params() {
    let mut builder = IrBuilder::new();
    builder.begin_function("add");
    let p0 = builder.add_param();
    let p1 = builder.add_param();

    let bb = builder.create_block();
    builder.switch_to_block(bb);
    let sum = builder.build_binop(BinOp::Add, p0, p1);
    builder.build_ret(Some(sum));

    let module = builder.finish();
    let func = &module.functions[0];
    assert_eq!(func.params.len(), 2);
    assert_eq!(func.params[0], p0);
    assert_eq!(func.params[1], p1);
}

#[test]
fn test_module_display() {
    let mut builder = IrBuilder::new();
    builder.begin_function("main");
    let bb = builder.create_block();
    builder.switch_to_block(bb);
    let v = builder.build_constant(42);
    builder.build_ret(Some(v));

    let module = builder.finish();
    let output = format!("{}", module);
    assert!(output.contains("fn main"));
    assert!(output.contains("const 42"));
    assert!(output.contains("ret %0"));
}

#[test]
fn test_instruction_dst_and_uses() {
    let inst = Instruction::BinOp {
        dst: VReg(2),
        op: BinOp::Add,
        lhs: VReg(0),
        rhs: VReg(1),
    };
    assert_eq!(inst.dst(), Some(VReg(2)));
    assert_eq!(inst.uses(), vec![VReg(0), VReg(1)]);

    let store = Instruction::Store {
        addr: VReg(0),
        value: VReg(1),
    };
    assert_eq!(store.dst(), None);
    assert_eq!(store.uses(), vec![VReg(0), VReg(1)]);
}

#[test]
fn test_terminator_successors() {
    let ret = Terminator::Ret(Some(VReg(0)));
    assert!(ret.successors().is_empty());
    assert_eq!(ret.uses(), vec![VReg(0)]);

    let br = Terminator::Branch(BlockId(1));
    assert_eq!(br.successors(), vec![BlockId(1)]);
    assert!(br.uses().is_empty());

    let condbr = Terminator::CondBranch {
        cond: VReg(0),
        true_bb: BlockId(1),
        false_bb: BlockId(2),
    };
    assert_eq!(condbr.successors(), vec![BlockId(1), BlockId(2)]);
    assert_eq!(condbr.uses(), vec![VReg(0)]);
}

#[test]
fn test_unary_ops() {
    let mut builder = IrBuilder::new();
    builder.begin_function("unary_test");
    let bb = builder.create_block();
    builder.switch_to_block(bb);

    let val = builder.build_constant(5);
    let neg = builder.build_unaryop(UnaryOp::Neg, val);
    let not = builder.build_unaryop(UnaryOp::Not, val);
    let bitnot = builder.build_unaryop(UnaryOp::BitNot, val);
    let _ = (neg, not, bitnot);
    builder.build_ret(None);

    let module = builder.finish();
    let func = &module.functions[0];
    // const, neg, not, bitnot
    assert_eq!(func.blocks[0].instructions.len(), 4);
}

#[test]
fn test_ir_type_sizes() {
    assert_eq!(IrType::Void.size_bytes(), 0);
    assert_eq!(IrType::i8().size_bytes(), 1);
    assert_eq!(IrType::i32().size_bytes(), 4);
    assert_eq!(IrType::i64().size_bytes(), 8);
    assert_eq!(IrType::ptr(IrType::i32()).size_bytes(), 8);
}

#[test]
fn test_ir_type_display() {
    assert_eq!(format!("{}", IrType::Void), "void");
    assert_eq!(format!("{}", IrType::i32()), "i32");
    assert_eq!(format!("{}", IrType::ptr(IrType::i8())), "i8*");
}
