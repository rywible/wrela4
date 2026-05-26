#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PhysicalReg {
    X(u8),
    W(u8),
    V(u8),
}

pub fn assembly_symbol(function_symbol: &str) -> String {
    format!("_{}", function_symbol.replace('.', "_"))
}

pub fn reg_name(reg: Option<PhysicalReg>) -> String {
    match reg.unwrap_or(PhysicalReg::X(0)) {
        PhysicalReg::X(index) => format!("x{index}"),
        PhysicalReg::W(index) => format!("w{index}"),
        PhysicalReg::V(index) => format!("v{index}.16b"),
    }
}
