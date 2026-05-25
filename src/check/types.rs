use std::collections::BTreeMap;

use crate::diagnostic::{
    Applicability, Diagnostic, DiagnosticCode, Severity, SourceEdit,
    source_edits_are_sorted_and_disjoint,
};
use crate::source::Span;

use super::resolve::{ItemId, ResolvedGraph, ResolvedModule, SymbolKind};
use super::suggest::nearest_name;
use super::summary::{CheckModuleSummary, MemberSummary, Name, TypeRefSummary};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct TypeId(u32);

impl TypeId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum BuiltinType {
    Bool,
    I64,
    U32,
    U64,
    String,
    None,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnknownReason {
    UnresolvedTypeName,
    UnresolvedExpressionName,
    UnsupportedExpression,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeKind {
    Builtin(BuiltinType),
    Item(ItemId, SymbolKind),
    Unknown(UnknownReason),
}

#[derive(Clone, Debug)]
pub struct TypeTable {
    kinds: Vec<TypeKind>,
    builtin_ids: BTreeMap<BuiltinType, TypeId>,
}

impl TypeTable {
    pub fn with_builtins() -> Self {
        let mut table = Self {
            kinds: Vec::new(),
            builtin_ids: BTreeMap::new(),
        };
        for builtin in [
            BuiltinType::Bool,
            BuiltinType::I64,
            BuiltinType::U32,
            BuiltinType::U64,
            BuiltinType::String,
            BuiltinType::None,
        ] {
            let id = table.push(TypeKind::Builtin(builtin));
            table.builtin_ids.insert(builtin, id);
        }
        table
    }

    pub fn push(&mut self, kind: TypeKind) -> TypeId {
        let id = TypeId::new(self.kinds.len() as u32);
        self.kinds.push(kind);
        id
    }

    pub fn push_unknown(&mut self, reason: UnknownReason) -> TypeId {
        self.push(TypeKind::Unknown(reason))
    }

    pub fn builtin(&self, builtin: BuiltinType) -> TypeId {
        self.builtin_ids[&builtin]
    }

    pub fn kind(&self, id: TypeId) -> Option<TypeKind> {
        self.kinds.get(id.raw() as usize).copied()
    }

    pub fn is_unknown(&self, id: TypeId) -> bool {
        matches!(self.kind(id), Some(TypeKind::Unknown(_)))
    }

    pub fn is_integer(&self, id: TypeId) -> bool {
        matches!(
            self.kind(id),
            Some(TypeKind::Builtin(
                BuiltinType::I64 | BuiltinType::U32 | BuiltinType::U64
            ))
        )
    }

    pub fn types_compatible(&self, expected: TypeId, actual: TypeId) -> bool {
        if expected == actual {
            return true;
        }
        self.is_integer(expected) && self.is_integer(actual)
    }
}

#[derive(Clone, Debug)]
pub struct CheckedParam {
    name: Name,
    access: super::summary::AccessMode,
    ty: TypeId,
    span: Span,
}

#[derive(Clone, Debug)]
pub struct CheckedSignature {
    params: Vec<CheckedParam>,
    return_type: TypeId,
}

#[derive(Clone, Debug)]
pub struct SignatureCheck {
    type_table: TypeTable,
    field_types: BTreeMap<u32, TypeId>,
    member_signatures: BTreeMap<u32, CheckedSignature>,
    diagnostics: Vec<Diagnostic>,
}

impl CheckedParam {
    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn access(&self) -> super::summary::AccessMode {
        self.access
    }

    pub fn ty(&self) -> TypeId {
        self.ty
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

impl CheckedSignature {
    pub fn params(&self) -> &[CheckedParam] {
        &self.params
    }

    pub fn return_type(&self) -> TypeId {
        self.return_type
    }
}

impl SignatureCheck {
    pub fn type_table(&self) -> &TypeTable {
        &self.type_table
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn field_type(&self, field: &MemberSummary) -> Option<TypeId> {
        self.field_types.get(&field.node_id().raw()).copied()
    }

    pub fn member_signature(&self, member: &MemberSummary) -> Option<&CheckedSignature> {
        self.member_signatures.get(&member.node_id().raw())
    }
}

pub fn check_signatures(summaries: &[CheckModuleSummary], graph: &ResolvedGraph) -> SignatureCheck {
    let mut checker = SignatureChecker {
        graph,
        type_table: TypeTable::with_builtins(),
        field_types: BTreeMap::new(),
        member_signatures: BTreeMap::new(),
        item_type_ids: BTreeMap::new(),
        diagnostics: Vec::new(),
    };

    for summary in summaries {
        let module = graph.module_for_summary(summary);
        for item in summary.items() {
            for implements in item.implements() {
                checker.resolve_type_ref(module, implements);
            }
            for member in item.members() {
                if let Some(field_type) = member.field_type() {
                    let ty = checker.resolve_type_ref(module, field_type);
                    checker.field_types.insert(member.node_id().raw(), ty);
                }
                if let Some(signature) = member.signature() {
                    let checked = checker.check_signature(module, signature);
                    checker
                        .member_signatures
                        .insert(member.node_id().raw(), checked);
                }
            }
        }
    }

    SignatureCheck {
        type_table: checker.type_table,
        field_types: checker.field_types,
        member_signatures: checker.member_signatures,
        diagnostics: checker.diagnostics,
    }
}

struct SignatureChecker<'a> {
    graph: &'a ResolvedGraph,
    type_table: TypeTable,
    field_types: BTreeMap<u32, TypeId>,
    member_signatures: BTreeMap<u32, CheckedSignature>,
    item_type_ids: BTreeMap<ItemId, TypeId>,
    diagnostics: Vec<Diagnostic>,
}

impl SignatureChecker<'_> {
    fn check_signature(
        &mut self,
        module: Option<&ResolvedModule>,
        signature: &super::summary::SignatureSummary,
    ) -> CheckedSignature {
        let mut params = Vec::new();
        for param in signature.params() {
            let ty = self.resolve_type_ref(module, param.ty());
            params.push(CheckedParam {
                name: param.name().clone(),
                access: param.access(),
                ty,
                span: param.span(),
            });
        }
        let return_type = signature
            .return_type()
            .map(|ty| self.resolve_type_ref(module, ty))
            .unwrap_or_else(|| self.type_table.builtin(BuiltinType::None));
        CheckedSignature {
            params,
            return_type,
        }
    }

    fn resolve_type_ref(&mut self, module: Option<&ResolvedModule>, ty: &TypeRefSummary) -> TypeId {
        let name = ty.path_text();
        if let Some(builtin) = builtin_by_name(&name) {
            return self.type_table.builtin(builtin);
        }
        let Some(module) = module else {
            let unknown = self
                .type_table
                .push_unknown(UnknownReason::UnresolvedTypeName);
            self.diagnostics
                .push(self.unknown_type_diagnostic_unscoped(ty, &name));
            return unknown;
        };
        if let Some((_owner, item)) = self.graph.resolve_visible_any(module, &name) {
            if !item.kind().is_type() {
                self.diagnostics.push(
                    Diagnostic::builder(
                        Severity::Error,
                        DiagnosticCode::ResolveWrongKind,
                        format!("`{name}` is not a type"),
                    )
                    .primary(ty.span(), "this name cannot be used as a type")
                    .secondary(item.span(), "name resolves here")
                    .finish(),
                );
                return self
                    .type_table
                    .push_unknown(UnknownReason::UnresolvedTypeName);
            }
            if let Some(&type_id) = self.item_type_ids.get(&item.id()) {
                return type_id;
            }
            let type_id = self.type_table.push(TypeKind::Item(item.id(), item.kind()));
            self.item_type_ids.insert(item.id(), type_id);
            return type_id;
        }
        let unknown = self
            .type_table
            .push_unknown(UnknownReason::UnresolvedTypeName);
        self.diagnostics
            .push(self.unknown_type_diagnostic(module, ty, &name));
        unknown
    }

    fn unknown_type_diagnostic_unscoped(&self, ty: &TypeRefSummary, name: &str) -> Diagnostic {
        Diagnostic::builder(
            Severity::Error,
            DiagnosticCode::TypeUnknownType,
            format!("unknown type `{name}`"),
        )
        .primary(ty.span(), "type name is not in scope")
        .finish()
    }

    fn unknown_type_diagnostic(
        &self,
        module: &ResolvedModule,
        ty: &TypeRefSummary,
        name: &str,
    ) -> Diagnostic {
        let mut builder = Diagnostic::builder(
            Severity::Error,
            DiagnosticCode::TypeUnknownType,
            format!("unknown type `{name}`"),
        )
        .primary(ty.span(), "type name is not in scope");

        if let Some(suggestion) = nearest_name(name, module.visible_names()) {
            let edits = vec![SourceEdit::replace(ty.span(), suggestion)];
            if source_edits_are_sorted_and_disjoint(&edits) {
                builder = builder
                    .help(format!("did you mean `{suggestion}`?"))
                    .suggestion(
                        format!("replace with `{suggestion}`"),
                        Applicability::Likely,
                        edits,
                    );
            } else {
                builder = builder.note("suggested fix edits overlap");
            }
        }

        builder.finish()
    }
}

fn builtin_by_name(name: &str) -> Option<BuiltinType> {
    match name {
        "Bool" => Some(BuiltinType::Bool),
        "I64" => Some(BuiltinType::I64),
        "U32" => Some(BuiltinType::U32),
        "U64" => Some(BuiltinType::U64),
        "String" => Some(BuiltinType::String),
        "None" => Some(BuiltinType::None),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtins_have_stable_type_ids() {
        let table = TypeTable::with_builtins();

        assert_eq!(table.builtin(BuiltinType::Bool), TypeId::new(0));
        assert_eq!(table.builtin(BuiltinType::I64), TypeId::new(1));
        assert_eq!(table.builtin(BuiltinType::U32), TypeId::new(2));
        assert_eq!(table.builtin(BuiltinType::U64), TypeId::new(3));
        assert_eq!(table.builtin(BuiltinType::String), TypeId::new(4));
        assert_eq!(table.builtin(BuiltinType::None), TypeId::new(5));
        assert!(table.is_integer(table.builtin(BuiltinType::U32)));
    }

    #[test]
    fn unknown_types_are_distinct_and_marked_unknown() {
        let mut table = TypeTable::with_builtins();
        let first = table.push_unknown(UnknownReason::UnresolvedTypeName);
        let second = table.push_unknown(UnknownReason::UnresolvedExpressionName);

        assert_ne!(first, second);
        assert!(table.is_unknown(first));
        assert!(table.is_unknown(second));
    }
}
