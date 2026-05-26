#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MaskValueKind {
    Input,
    AllTrue,
    AllFalse,
    Derived,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MaskProvenance {
    name: String,
    table: String,
    rows: u64,
    kind: MaskValueKind,
}

impl MaskProvenance {
    pub fn new(name: String, table: String, rows: u64) -> Self {
        Self {
            name,
            table,
            rows,
            kind: MaskValueKind::Input,
        }
    }

    pub fn constant(table: String, rows: u64, kind: MaskValueKind) -> Self {
        Self {
            name: "<constant>".to_string(),
            table,
            rows,
            kind,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn table(&self) -> &str {
        &self.table
    }

    pub fn rows(&self) -> u64 {
        self.rows
    }

    pub fn kind(&self) -> MaskValueKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RowTokenFact {
    table: String,
    rows: u64,
    escapes: bool,
}

#[cfg(test)]
mod tests {
    #[test]
    fn dataplane_testing_builders_create_verifiable_modules() {
        let module = super::testing::mask_and_true_module("packets", 256);
        assert!(crate::mir::verify_module(&module).ok());
    }
}

#[cfg(test)]
pub mod testing {
    use super::{MaskProvenance, MaskValueKind};
    use crate::mir::{EffectSet, MirModule, MirType, OperationData, OperationKind, ValueData};

    pub fn mask_and_true_module(table: &str, rows: u64) -> MirModule {
        let mut module = MirModule::empty_for_test();
        let mask = module.push_value(ValueData::new(MirType::Mask {
            table: table.to_string(),
            rows,
        }));
        let all_true = module.push_value(ValueData::new(MirType::Mask {
            table: table.to_string(),
            rows,
        }));
        let result = module.push_value(ValueData::new(MirType::Mask {
            table: table.to_string(),
            rows,
        }));
        module.push_test_operation(OperationData::new(
            OperationKind::MaskRead {
                name: "valid".to_string(),
                table: table.to_string(),
                rows,
            },
            Vec::new(),
            vec![mask],
            EffectSet::empty(),
        ));
        module.push_test_operation(OperationData::new(
            OperationKind::MaskAllTrue {
                table: table.to_string(),
                rows,
            },
            Vec::new(),
            vec![all_true],
            EffectSet::empty(),
        ));
        module.push_test_operation(OperationData::new(
            OperationKind::MaskAnd,
            vec![mask, all_true],
            vec![result],
            EffectSet::empty(),
        ));
        module.push_test_operation(OperationData::new(
            OperationKind::Return,
            vec![result],
            Vec::new(),
            EffectSet::empty(),
        ));
        let _ = MaskProvenance::constant(table.to_string(), rows, MaskValueKind::AllTrue);
        module
    }

    pub fn mask_and_mismatched_true_module(
        table: &str,
        left_rows: u64,
        right_rows: u64,
    ) -> MirModule {
        let mut module = mask_and_true_module(table, left_rows);
        module.replace_operation_kind_for_test(
            1,
            OperationKind::MaskAllTrue {
                table: table.to_string(),
                rows: right_rows,
            },
        );
        module
    }

    pub fn mask_double_not_module(table: &str, rows: u64) -> MirModule {
        let mut module = MirModule::empty_for_test();
        let mask = module.push_value(ValueData::new(MirType::Mask {
            table: table.to_string(),
            rows,
        }));
        let once = module.push_value(ValueData::new(MirType::Mask {
            table: table.to_string(),
            rows,
        }));
        let twice = module.push_value(ValueData::new(MirType::Mask {
            table: table.to_string(),
            rows,
        }));
        module.push_test_operation(OperationData::new(
            OperationKind::MaskRead {
                name: "valid".to_string(),
                table: table.to_string(),
                rows,
            },
            Vec::new(),
            vec![mask],
            EffectSet::empty(),
        ));
        module.push_test_operation(OperationData::new(
            OperationKind::MaskNot,
            vec![mask],
            vec![once],
            EffectSet::empty(),
        ));
        module.push_test_operation(OperationData::new(
            OperationKind::MaskNot,
            vec![once],
            vec![twice],
            EffectSet::empty(),
        ));
        module.push_test_operation(OperationData::new(
            OperationKind::Return,
            vec![twice],
            Vec::new(),
            EffectSet::empty(),
        ));
        module
    }
}
