use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};

use super::summary::{CheckModuleSummary, ItemKind};
use super::types::{BuiltinType, SignatureCheck, TypeId, TypeKind};

#[derive(Clone, Debug)]
pub struct LayoutCheck {
    diagnostics: Vec<Diagnostic>,
}

impl LayoutCheck {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

pub fn check_layouts(summaries: &[CheckModuleSummary], signatures: &SignatureCheck) -> LayoutCheck {
    let mut diagnostics = Vec::new();
    for summary in summaries {
        for item in summary.items() {
            if item.kind() != ItemKind::LayoutData {
                continue;
            }
            for field in item.fields() {
                let Some(ty) = signatures.field_type(field) else {
                    continue;
                };
                let qualified = field
                    .field_type()
                    .is_some_and(|field_type| field_type.access().is_some() || field_type.unique());
                if qualified || !is_layout_c_scalar(signatures, ty) {
                    diagnostics.push(
                        Diagnostic::builder(
                            Severity::Error,
                            DiagnosticCode::LayoutInvalid,
                            "invalid layout C field type",
                        )
                        .primary(
                            field.span(),
                            "layout C fields must be fixed primitive scalars",
                        )
                        .finish(),
                    );
                }
            }
        }
    }
    LayoutCheck { diagnostics }
}

fn is_layout_c_scalar(signatures: &SignatureCheck, ty: TypeId) -> bool {
    matches!(
        signatures.type_table().kind(ty),
        Some(TypeKind::Builtin(
            BuiltinType::Bool | BuiltinType::I64 | BuiltinType::U32 | BuiltinType::U64
        ))
    )
}
