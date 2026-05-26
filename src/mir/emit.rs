use std::fmt::Write;

use super::aarch64;
use super::lir::{LirBlockId, LirOpcode, LirOperand};
use super::regalloc::AllocatedProgram;

pub fn emit_aarch64(program: &AllocatedProgram) -> String {
    let mut out = String::new();
    out.push_str(".text\n");
    for function in program.lir().functions() {
        let symbol = assembly_symbol(function.symbol());
        let _ = writeln!(out, ".global {symbol}");
        let _ = writeln!(out, "{symbol}:");
        for block_id in function.blocks() {
            let _ = writeln!(out, "{}:", block_label(*block_id));
            for inst_id in program.lir().block(*block_id).instructions() {
                let inst = program.lir().inst(*inst_id);
                match inst.opcode() {
                    LirOpcode::Mov => {
                        if let (Some(operand), Some(result)) =
                            (inst.operands().first(), inst.results().first())
                        {
                            let dst = reg_name(program, *result);
                            match operand {
                                LirOperand::ImmI64(value) => {
                                    let _ = writeln!(out, "  mov {dst}, #{value}");
                                }
                                LirOperand::Reg(src) => {
                                    let src_name = reg_name(program, *src);
                                    let _ = writeln!(out, "  mov {dst}, {src_name}");
                                }
                                LirOperand::Label(_) => {}
                            }
                        }
                    }
                    LirOpcode::Add => {
                        if let [LirOperand::Reg(left), LirOperand::Reg(right)] = inst.operands() {
                            let dst = reg_name(program, inst.results()[0]);
                            let lhs = reg_name(program, *left);
                            let rhs = reg_name(program, *right);
                            let _ = writeln!(out, "  add {dst}, {lhs}, {rhs}");
                        } else if let [LirOperand::Reg(left), LirOperand::ImmI64(imm)] =
                            inst.operands()
                        {
                            let dst = reg_name(program, inst.results()[0]);
                            let lhs = reg_name(program, *left);
                            let _ = writeln!(out, "  add {dst}, {lhs}, #{imm}");
                        }
                    }
                    LirOpcode::AddU64 => {
                        if let [LirOperand::Reg(acc), LirOperand::Reg(value)] = inst.operands() {
                            let dst = reg_name(program, inst.results()[0]);
                            let lhs = reg_name(program, *acc);
                            let rhs = reg_name(program, *value);
                            let _ = writeln!(out, "  add {dst}, {lhs}, {rhs}, uxtw");
                        }
                    }
                    LirOpcode::LoadU32 => {
                        if let [
                            LirOperand::Reg(table),
                            LirOperand::Reg(index),
                            LirOperand::ImmI64(off),
                        ] = inst.operands()
                        {
                            let dst = reg_name(program, inst.results()[0]);
                            let tbl = reg_name(program, *table);
                            let idx = reg_name(program, *index);
                            let _ = writeln!(out, "  add x16, {tbl}, {idx}, lsl #3");
                            let _ = writeln!(out, "  ldr {dst}, [x16, #{off}]", off = off);
                        }
                    }
                    LirOpcode::LoadMaskByte => {
                        if let [LirOperand::Reg(mask), LirOperand::Reg(index)] = inst.operands() {
                            let dst = reg_name(program, inst.results()[0]);
                            let m = reg_name(program, *mask);
                            let idx = reg_name(program, *index);
                            let _ = writeln!(out, "  lsr x16, {idx}, #3");
                            let _ = writeln!(out, "  ldrb {dst}, [{m}, x16]");
                        }
                    }
                    LirOpcode::TestMaskBit => {
                        if let [LirOperand::Reg(mask_byte), LirOperand::Reg(index)] =
                            inst.operands()
                        {
                            let dst = reg_name(program, inst.results()[0]);
                            let mask = reg_name(program, *mask_byte);
                            let idx = reg_name(program, *index);
                            let idx_w = idx
                                .strip_prefix('x')
                                .map(|n| format!("w{n}"))
                                .unwrap_or(idx);
                            let _ = writeln!(out, "  and w16, {idx_w}, #7");
                            let _ = writeln!(out, "  mov w17, #1");
                            let _ = writeln!(out, "  lsl w17, w17, w16");
                            let _ = writeln!(out, "  tst {mask}, w17");
                            let _ = writeln!(out, "  cset {dst}, ne");
                        }
                    }
                    LirOpcode::BranchIfZero => {
                        if let [LirOperand::Reg(reg), LirOperand::Label(label)] = inst.operands() {
                            let src = reg_name(program, *reg);
                            let _ = writeln!(out, "  cbz {src}, {label}");
                        }
                    }
                    LirOpcode::BranchIfLessThan => {
                        if let [left, right, LirOperand::Label(label)] = inst.operands() {
                            match (left, right) {
                                (LirOperand::Reg(lhs), LirOperand::Reg(rhs)) => {
                                    let l = reg_name(program, *lhs);
                                    let r = reg_name(program, *rhs);
                                    let _ = writeln!(out, "  cmp {l}, {r}");
                                }
                                (LirOperand::Reg(lhs), LirOperand::ImmI64(imm)) => {
                                    let l = reg_name(program, *lhs);
                                    let _ = writeln!(out, "  cmp {l}, #{imm}");
                                }
                                _ => {}
                            }
                            let _ = writeln!(out, "  b.lt {label}");
                        }
                    }
                    LirOpcode::Branch => {
                        if let [LirOperand::Label(label)] = inst.operands() {
                            let _ = writeln!(out, "  b {label}");
                        }
                    }
                    LirOpcode::Ret => {
                        if let Some(LirOperand::Reg(value)) = inst.operands().first() {
                            let src = reg_name(program, *value);
                            if src != "x0" {
                                let _ = writeln!(out, "  mov x0, {src}");
                            }
                        }
                        out.push_str("  ret\n");
                    }
                }
            }
        }
    }
    out
}

fn reg_name(program: &AllocatedProgram, reg: super::VirtualReg) -> String {
    aarch64::reg_name(program.physical(reg))
}

fn block_label(block: LirBlockId) -> String {
    format!(".L{}", block.raw())
}

pub fn assembly_symbol(function_symbol: &str) -> String {
    aarch64::assembly_symbol(function_symbol)
}
