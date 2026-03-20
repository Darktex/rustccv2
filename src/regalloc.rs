//! Linear scan register allocator.
//!
//! Computes live intervals for each virtual register, then allocates
//! physical registers using the linear scan algorithm. When registers
//! are exhausted, the interval with the furthest end point is spilled
//! to a stack slot.

use crate::ir::{Function, Instruction, Label, Module, Operand, Terminator, VReg};
use std::collections::{BTreeSet, HashMap};

/// A live interval: vreg is live from `start` to `end` (inclusive).
/// Positions are numbered sequentially: each instruction gets 2 positions
/// (one for use/read, one for def/write), terminators get positions too.
#[derive(Debug, Clone)]
pub struct LiveInterval {
    pub vreg: VReg,
    pub start: u32,
    pub end: u32,
}

/// Physical register or spill slot assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Location {
    /// Allocated to a physical register (index into the arch's allocatable set).
    Reg(u8),
    /// Spilled to a stack slot (offset from frame pointer, negative on x86).
    Spill(i32),
}

/// Result of register allocation for one function.
#[derive(Debug)]
pub struct RegAllocResult {
    /// Map from vreg to its assigned location.
    pub assignments: HashMap<VReg, Location>,
    /// Number of spill slots used.
    #[allow(dead_code)]
    pub num_spill_slots: u32,
}

/// Compute a linearised position for each instruction in a function.
/// Returns a map from (block_index, instr_index) to position,
/// and also the block-start and block-end positions.
fn compute_positions(func: &Function) -> (Vec<u32>, u32) {
    // block_starts[i] = position where block i starts
    let mut block_starts = Vec::new();
    let mut pos: u32 = 0;
    for block in &func.blocks {
        block_starts.push(pos);
        // Each instruction: 2 positions (use then def)
        pos += (block.instructions.len() as u32) * 2;
        // Terminator: 2 positions
        pos += 2;
    }
    (block_starts, pos)
}

/// Collect all vregs used/defined in an operand.
fn operand_vregs(op: &Operand) -> Option<VReg> {
    match op {
        Operand::VReg(v) => Some(*v),
        Operand::Immediate(_) => None,
    }
}

/// Collect vregs used by an instruction (reads).
fn instruction_uses(inst: &Instruction) -> Vec<VReg> {
    let mut uses = Vec::new();
    match inst {
        Instruction::LoadImm { .. } => {}
        Instruction::BinOp { left, right, .. } => {
            if let Some(v) = operand_vregs(left) {
                uses.push(v);
            }
            if let Some(v) = operand_vregs(right) {
                uses.push(v);
            }
        }
        Instruction::UnaryOp { operand, .. } => {
            if let Some(v) = operand_vregs(operand) {
                uses.push(v);
            }
        }
        Instruction::Cmp { left, right, .. } => {
            if let Some(v) = operand_vregs(left) {
                uses.push(v);
            }
            if let Some(v) = operand_vregs(right) {
                uses.push(v);
            }
        }
        Instruction::Copy { src, .. } => {
            if let Some(v) = operand_vregs(src) {
                uses.push(v);
            }
        }
        Instruction::Call { args, .. } => {
            for arg in args {
                if let Some(v) = operand_vregs(arg) {
                    uses.push(v);
                }
            }
        }
        Instruction::LoadStringAddr { .. } => {}
        Instruction::Alloca { .. } => {}
        Instruction::Store { addr, value } => {
            uses.push(*addr);
            if let Some(v) = operand_vregs(value) {
                uses.push(v);
            }
        }
        Instruction::Load { addr, .. } => {
            uses.push(*addr);
        }
    }
    uses
}

/// Collect vregs defined by an instruction (writes).
fn instruction_defs(inst: &Instruction) -> Vec<VReg> {
    match inst {
        Instruction::LoadImm { dest, .. }
        | Instruction::BinOp { dest, .. }
        | Instruction::UnaryOp { dest, .. }
        | Instruction::Cmp { dest, .. }
        | Instruction::Copy { dest, .. }
        | Instruction::LoadStringAddr { dest, .. }
        | Instruction::Alloca { dest, .. }
        | Instruction::Load { dest, .. } => vec![*dest],
        Instruction::Call { dest, .. } => dest.iter().copied().collect(),
        Instruction::Store { .. } => vec![],
    }
}

/// Collect vregs used by a terminator.
fn terminator_uses(term: &Terminator) -> Vec<VReg> {
    match term {
        Terminator::Return(Some(op)) => operand_vregs(op).into_iter().collect(),
        Terminator::Branch { condition, .. } => operand_vregs(condition).into_iter().collect(),
        Terminator::Return(None) | Terminator::Jump(_) | Terminator::None => vec![],
    }
}

/// Collect the successor block labels from a terminator.
fn terminator_targets(term: &Terminator) -> Vec<Label> {
    match term {
        Terminator::Jump(label) => vec![*label],
        Terminator::Branch {
            true_label,
            false_label,
            ..
        } => vec![*true_label, *false_label],
        Terminator::Return(_) | Terminator::None => vec![],
    }
}

/// Build a map from block label to block index.
fn label_to_index(func: &Function) -> HashMap<Label, usize> {
    func.blocks
        .iter()
        .enumerate()
        .map(|(i, b)| (b.label, i))
        .collect()
}

/// Detect loop ranges by finding back-edges in the CFG.
/// Returns a list of (loop_start_pos, loop_end_pos) for each loop detected.
/// A back-edge is a CFG edge where the target block index <= source block index.
fn detect_loop_ranges(func: &Function, block_starts: &[u32]) -> Vec<(u32, u32)> {
    let label_map = label_to_index(func);
    let mut loop_ranges = Vec::new();

    for (src_idx, block) in func.blocks.iter().enumerate() {
        for target_label in terminator_targets(&block.terminator) {
            if let Some(&tgt_idx) = label_map.get(&target_label) {
                if tgt_idx <= src_idx {
                    // Back-edge: src_idx -> tgt_idx where tgt is earlier
                    let loop_start = block_starts[tgt_idx];
                    // End of the source block (terminator position + 1)
                    let loop_end =
                        block_starts[src_idx] + (block.instructions.len() as u32) * 2 + 1;
                    loop_ranges.push((loop_start, loop_end));
                }
            }
        }
    }

    loop_ranges
}

/// Compute live intervals for all vregs in a function.
/// Handles loop back-edges by extending live intervals for vregs that are
/// live across loop boundaries.
pub fn compute_live_intervals(func: &Function) -> Vec<LiveInterval> {
    let mut first_def: HashMap<VReg, u32> = HashMap::new();
    let mut last_use: HashMap<VReg, u32> = HashMap::new();

    let (block_starts, _total_positions) = compute_positions(func);

    for (block_idx, block) in func.blocks.iter().enumerate() {
        let base = block_starts[block_idx];

        for (inst_idx, inst) in block.instructions.iter().enumerate() {
            let use_pos = base + (inst_idx as u32) * 2;
            let def_pos = use_pos + 1;

            // Record uses
            for vreg in instruction_uses(inst) {
                last_use
                    .entry(vreg)
                    .and_modify(|e| *e = (*e).max(use_pos))
                    .or_insert(use_pos);
                // If used before any def, set start to 0 (function parameter)
                first_def.entry(vreg).or_insert(0);
            }

            // Record defs
            for vreg in instruction_defs(inst) {
                first_def.entry(vreg).or_insert(def_pos);
                // A def also extends the live range (the vreg is alive after def)
                last_use
                    .entry(vreg)
                    .and_modify(|e| *e = (*e).max(def_pos))
                    .or_insert(def_pos);
            }
        }

        // Terminator
        let term_pos = base + (block.instructions.len() as u32) * 2;
        for vreg in terminator_uses(&block.terminator) {
            last_use
                .entry(vreg)
                .and_modify(|e| *e = (*e).max(term_pos))
                .or_insert(term_pos);
            first_def.entry(vreg).or_insert(0);
        }
    }

    // Extend live intervals across loop back-edges.
    // For each loop (detected by back-edges), any vreg whose interval overlaps
    // the loop range must be extended to cover the entire loop.
    let loop_ranges = detect_loop_ranges(func, &block_starts);
    for (loop_start, loop_end) in &loop_ranges {
        for (&vreg, def) in &first_def {
            let use_end = last_use.get(&vreg).copied().unwrap_or(*def);
            // If the vreg's interval overlaps with the loop range, extend it
            // to cover the entire loop. A vreg overlaps the loop if it is
            // defined before the loop ends and used after the loop starts.
            if *def <= *loop_end && use_end >= *loop_start {
                last_use
                    .entry(vreg)
                    .and_modify(|e| *e = (*e).max(*loop_end))
                    .or_insert(*loop_end);
            }
        }
    }

    // Build intervals
    let mut intervals: Vec<LiveInterval> = Vec::new();
    for (&vreg, &start) in &first_def {
        let end = last_use.get(&vreg).copied().unwrap_or(start);
        intervals.push(LiveInterval { vreg, start, end });
    }

    // Sort by start position
    intervals.sort_by_key(|i| i.start);
    intervals
}

/// Run linear scan register allocation.
///
/// `num_regs` is the number of allocatable general-purpose registers
/// available on the target architecture.
pub fn linear_scan(func: &Function, num_regs: usize) -> RegAllocResult {
    let intervals = compute_live_intervals(func);
    let mut assignments: HashMap<VReg, Location> = HashMap::new();

    // Active intervals, sorted by end position (using BTreeSet of (end, vreg))
    let mut active: BTreeSet<(u32, VReg)> = BTreeSet::new();
    // Map from vreg to assigned physical register
    let mut vreg_to_reg: HashMap<VReg, u8> = HashMap::new();
    // Free registers (pool)
    let mut free_regs: Vec<u8> = (0..num_regs as u8).rev().collect(); // stack, pop from end
    let mut num_spill_slots: u32 = 0;

    for interval in &intervals {
        // Expire old intervals: remove any whose end < current start
        let expired: Vec<(u32, VReg)> = active
            .iter()
            .take_while(|(end, _)| *end < interval.start)
            .cloned()
            .collect();
        for (end, vreg) in expired {
            active.remove(&(end, vreg));
            if let Some(reg) = vreg_to_reg.remove(&vreg) {
                free_regs.push(reg);
            }
        }

        if free_regs.is_empty() {
            // Spill: pick the interval with the furthest end point
            // This could be the current interval or one in the active set
            let last_active = active.iter().next_back().cloned();
            if let Some((last_end, last_vreg)) = last_active {
                if last_end > interval.end {
                    // Spill the one with furthest endpoint, give its reg to current
                    let reg = vreg_to_reg.remove(&last_vreg).unwrap();
                    active.remove(&(last_end, last_vreg));

                    // Spill last_vreg
                    let spill_slot = -(((num_spill_slots + 1) * 8) as i32);
                    num_spill_slots += 1;
                    assignments.insert(last_vreg, Location::Spill(spill_slot));

                    // Assign reg to current interval
                    vreg_to_reg.insert(interval.vreg, reg);
                    assignments.insert(interval.vreg, Location::Reg(reg));
                    active.insert((interval.end, interval.vreg));
                } else {
                    // Spill the current interval
                    let spill_slot = -(((num_spill_slots + 1) * 8) as i32);
                    num_spill_slots += 1;
                    assignments.insert(interval.vreg, Location::Spill(spill_slot));
                }
            } else {
                // No active intervals and no free regs — shouldn't happen
                let spill_slot = -(((num_spill_slots + 1) * 8) as i32);
                num_spill_slots += 1;
                assignments.insert(interval.vreg, Location::Spill(spill_slot));
            }
        } else {
            // Allocate a free register
            let reg = free_regs.pop().unwrap();
            vreg_to_reg.insert(interval.vreg, reg);
            assignments.insert(interval.vreg, Location::Reg(reg));
            active.insert((interval.end, interval.vreg));
        }
    }

    RegAllocResult {
        assignments,
        num_spill_slots,
    }
}

/// Run register allocation for all functions in a module.
pub fn allocate_registers(module: &Module, num_regs: usize) -> HashMap<String, RegAllocResult> {
    let mut results = HashMap::new();
    for func in &module.functions {
        if func.is_defined {
            let result = linear_scan(func, num_regs);
            results.insert(func.name.clone(), result);
        }
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir;
    use crate::lexer;
    use crate::parser;

    #[test]
    fn test_live_intervals_return_42() {
        let tokens = lexer::lex("int main() { return 42; }").unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = ir::lower(&program);
        let intervals = compute_live_intervals(&module.functions[0]);
        // return 42 uses Immediate, so no vreg intervals needed
        assert!(intervals.is_empty());
    }

    #[test]
    fn test_live_intervals_variables() {
        let tokens =
            lexer::lex("int main() { int a = 5; int b = 7; int c = a + b; return c; }").unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = ir::lower(&program);
        let intervals = compute_live_intervals(&module.functions[0]);
        // Should have intervals for a, b, c (and temp for a+b)
        assert!(intervals.len() >= 3);
    }

    #[test]
    fn test_linear_scan_simple() {
        let tokens =
            lexer::lex("int main() { int a = 5; int b = 7; int c = a + b; return c; }").unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = ir::lower(&program);
        // Use 4 registers - should be enough, no spills
        let result = linear_scan(&module.functions[0], 4);
        assert_eq!(result.num_spill_slots, 0);
        // All vregs should be assigned to registers
        for (_vreg, loc) in &result.assignments {
            assert!(matches!(loc, Location::Reg(_)));
        }
    }

    #[test]
    fn test_linear_scan_with_spill() {
        let tokens =
            lexer::lex("int main() { int a = 5; int b = 7; int c = a + b; return c; }").unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = ir::lower(&program);
        // Use only 1 register - should force spills
        let result = linear_scan(&module.functions[0], 1);
        assert!(result.num_spill_slots > 0);
    }

    #[test]
    fn test_regalloc_while_loop() {
        let src = "int main() { int x = 10; int count = 0; while (x > 0) { x = x - 1; count = count + 1; } return count; }";
        let tokens = lexer::lex(src).unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = ir::lower(&program);
        let result = linear_scan(&module.functions[0], 6);
        // Should successfully allocate all intervals
        assert!(!result.assignments.is_empty());
    }

    #[test]
    fn test_regalloc_fibonacci() {
        let src = r#"
            int fib(int n) {
                if (n <= 1) return n;
                return fib(n - 1) + fib(n - 2);
            }
            int main() { return fib(10); }
        "#;
        let tokens = lexer::lex(src).unwrap();
        let program = parser::parse(tokens).unwrap();
        let module = ir::lower(&program);
        // Allocate for fib function
        let result = linear_scan(&module.functions[0], 6);
        assert!(!result.assignments.is_empty());
    }
}
