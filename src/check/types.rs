use std::collections::BTreeMap;

use crate::diagnostic::{
    Applicability, Diagnostic, DiagnosticCode, Severity, SourceEdit,
    source_edits_are_sorted_and_disjoint,
};
use crate::source::Span;

use super::resolve::{ItemId, ResolvedGraph, ResolvedModule, SymbolKind};
use super::suggest::nearest_name;
use super::summary::{
    CheckModuleSummary, ItemKind, MemberSummary, Name, TypeArgSummary, TypeRefSummary,
};

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
    Table { item: ItemId, rows: u64 },
    Mask { table_param: u32, rows: u64 },
    RowToken { table_param: u32 },
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

    pub fn table(&mut self, item: ItemId, rows: u64) -> TypeId {
        self.push(TypeKind::Table { item, rows })
    }

    pub fn mask(&mut self, table_param: u32, rows: u64) -> TypeId {
        self.push(TypeKind::Mask { table_param, rows })
    }

    pub fn row_token(&mut self, table_param: u32) -> TypeId {
        self.push(TypeKind::RowToken { table_param })
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
    data_field_types: BTreeMap<(ItemId, String), TypeId>,
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

    pub fn data_field_type(&self, item: ItemId, field: &str) -> Option<TypeId> {
        self.data_field_types
            .get(&(item, field.to_string()))
            .copied()
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
        data_field_types: BTreeMap::new(),
        member_signatures: BTreeMap::new(),
        item_type_ids: BTreeMap::new(),
        diagnostics: Vec::new(),
    };

    for summary in summaries {
        let module = graph.module_for_summary(summary);
        for item in summary.items() {
            let item_id = module.and_then(|module| module.resolve_local(item.name().text()));
            for implements in item.implements() {
                checker.resolve_type_ref(module, implements);
            }
            for member in item.members() {
                if let Some(field_type) = member.field_type() {
                    let ty = checker.resolve_type_ref(module, field_type);
                    checker.field_types.insert(member.node_id().raw(), ty);
                    if item.kind() == ItemKind::Data {
                        if let (Some(item_id), Some(name)) = (item_id, member.name()) {
                            checker
                                .data_field_types
                                .insert((item_id, name.text().to_string()), ty);
                        }
                    }
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
        data_field_types: checker.data_field_types,
        member_signatures: checker.member_signatures,
        diagnostics: checker.diagnostics,
    }
}

struct SignatureChecker<'a> {
    graph: &'a ResolvedGraph,
    type_table: TypeTable,
    field_types: BTreeMap<u32, TypeId>,
    data_field_types: BTreeMap<(ItemId, String), TypeId>,
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
        for (param_index, param) in signature.params().iter().enumerate() {
            let ty = self.resolve_param_type(module, param.ty(), &params, param_index);
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

    fn resolve_param_type(
        &mut self,
        module: Option<&ResolvedModule>,
        ty: &TypeRefSummary,
        prior_params: &[CheckedParam],
        param_index: usize,
    ) -> TypeId {
        match ty.path_text().as_str() {
            "Table" => self.resolve_table_type(module, ty),
            "Mask" => self.resolve_mask_type(module, ty, prior_params, param_index),
            _ => self.resolve_type_ref(module, ty),
        }
    }

    fn resolve_table_type(
        &mut self,
        module: Option<&ResolvedModule>,
        ty: &TypeRefSummary,
    ) -> TypeId {
        let args = ty.args();
        if args.len() != 2 {
            self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::TypeArgument,
                    "`Table` requires exactly two generic arguments: a row type and a positive row count",
                )
                .primary(ty.span(), "invalid `Table` generic arguments")
                .finish(),
            );
            return self
                .type_table
                .push_unknown(UnknownReason::UnresolvedTypeName);
        }
        let Some(TypeArgSummary::Type(item_ref)) = args.first() else {
            self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::TypeArgument,
                    "`Table` row type must be a type name",
                )
                .primary(ty.span(), "expected type argument")
                .finish(),
            );
            return self
                .type_table
                .push_unknown(UnknownReason::UnresolvedTypeName);
        };
        let item_ty = self.resolve_type_ref(module, item_ref);
        let Some(TypeKind::Item(item, kind)) = self.type_table.kind(item_ty) else {
            self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::TypeArgument,
                    "`Table` row type must name a data type",
                )
                .primary(item_ref.span(), "not a table row type")
                .finish(),
            );
            return self
                .type_table
                .push_unknown(UnknownReason::UnresolvedTypeName);
        };
        if kind != SymbolKind::Data {
            self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::TypeArgument,
                    "`Table` row type must name a data type",
                )
                .primary(item_ref.span(), "not a data type")
                .finish(),
            );
            return self
                .type_table
                .push_unknown(UnknownReason::UnresolvedTypeName);
        }
        let rows = match args.get(1) {
            Some(TypeArgSummary::Int { value, .. }) if *value > 0 => *value,
            _ => {
                self.diagnostics.push(
                    Diagnostic::builder(
                        Severity::Error,
                        DiagnosticCode::TypeArgument,
                        "`Table` row count must be a positive integer",
                    )
                    .primary(ty.span(), "invalid row count")
                    .finish(),
                );
                return self
                    .type_table
                    .push_unknown(UnknownReason::UnresolvedTypeName);
            }
        };
        self.type_table.table(item, rows)
    }

    fn resolve_mask_type(
        &mut self,
        module: Option<&ResolvedModule>,
        ty: &TypeRefSummary,
        prior_params: &[CheckedParam],
        param_index: usize,
    ) -> TypeId {
        let args = ty.args();
        if args.len() != 2 {
            self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::TypeArgument,
                    "`Mask` requires exactly two generic arguments: a table parameter and a row count",
                )
                .primary(ty.span(), "invalid `Mask` generic arguments")
                .finish(),
            );
            return self
                .type_table
                .push_unknown(UnknownReason::UnresolvedTypeName);
        }
        let table_param_index = match args.first() {
            Some(TypeArgSummary::ValueName(name)) => {
                self.table_param_index_for_name(name.text(), prior_params, ty.span())
            }
            Some(TypeArgSummary::Type(type_ref))
                if type_ref.path().len() == 1 && type_ref.args().is_empty() =>
            {
                let name = type_ref.path()[0].text();
                if module.is_some_and(|module| {
                    self.graph
                        .resolve_visible_any(module, name)
                        .is_some_and(|(_, item)| item.kind().is_type())
                }) {
                    self.diagnostics.push(
                        Diagnostic::builder(
                            Severity::Error,
                            DiagnosticCode::TypeArgument,
                            format!(
                                "`Mask[{name}, N]` must refer to an earlier table parameter, not a type name"
                            ),
                        )
                        .primary(type_ref.span(), "mask lacks table-parameter provenance")
                        .finish(),
                    );
                    return self
                        .type_table
                        .push_unknown(UnknownReason::UnresolvedTypeName);
                }
                self.table_param_index_for_name(name, prior_params, type_ref.span())
            }
            Some(TypeArgSummary::Type(type_ref)) => {
                self.diagnostics.push(
                    Diagnostic::builder(
                        Severity::Error,
                        DiagnosticCode::TypeArgument,
                        "`Mask` table argument must name an earlier table parameter",
                    )
                    .primary(type_ref.span(), "invalid mask table reference")
                    .finish(),
                );
                return self
                    .type_table
                    .push_unknown(UnknownReason::UnresolvedTypeName);
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::builder(
                        Severity::Error,
                        DiagnosticCode::TypeArgument,
                        "`Mask` table argument must name an earlier table parameter",
                    )
                    .primary(ty.span(), "invalid mask table reference")
                    .finish(),
                );
                return self
                    .type_table
                    .push_unknown(UnknownReason::UnresolvedTypeName);
            }
        };
        let Some(table_param_index) = table_param_index else {
            return self
                .type_table
                .push_unknown(UnknownReason::UnresolvedTypeName);
        };
        if table_param_index >= param_index {
            self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::TypeArgument,
                    "`Mask` cannot refer to a later or same parameter",
                )
                .primary(
                    ty.span(),
                    "mask table parameter must appear earlier in the signature",
                )
                .finish(),
            );
            return self
                .type_table
                .push_unknown(UnknownReason::UnresolvedTypeName);
        }
        let rows = match args.get(1) {
            Some(TypeArgSummary::Int { value, .. }) if *value > 0 => *value,
            _ => {
                self.diagnostics.push(
                    Diagnostic::builder(
                        Severity::Error,
                        DiagnosticCode::TypeArgument,
                        "`Mask` row count must be a positive integer",
                    )
                    .primary(ty.span(), "invalid row count")
                    .finish(),
                );
                return self
                    .type_table
                    .push_unknown(UnknownReason::UnresolvedTypeName);
            }
        };
        match self.type_table.kind(prior_params[table_param_index].ty()) {
            Some(TypeKind::Table {
                rows: table_rows, ..
            }) if table_rows == rows => self.type_table.mask(table_param_index as u32, rows),
            Some(TypeKind::Table {
                rows: table_rows, ..
            }) => {
                self.diagnostics.push(
                    Diagnostic::builder(
                        Severity::Error,
                        DiagnosticCode::TypeMismatch,
                        format!(
                            "mask row count `{rows}` does not match table row count `{table_rows}`"
                        ),
                    )
                    .primary(ty.span(), "row count mismatch")
                    .finish(),
                );
                self.type_table
                    .push_unknown(UnknownReason::UnresolvedTypeName)
            }
            _ => {
                self.diagnostics.push(
                    Diagnostic::builder(
                        Severity::Error,
                        DiagnosticCode::TypeArgument,
                        "`Mask` must refer to an earlier `Table` parameter",
                    )
                    .primary(ty.span(), "table parameter is not a table")
                    .finish(),
                );
                self.type_table
                    .push_unknown(UnknownReason::UnresolvedTypeName)
            }
        }
    }

    fn table_param_index_for_name(
        &mut self,
        name: &str,
        prior_params: &[CheckedParam],
        span: Span,
    ) -> Option<usize> {
        prior_params
            .iter()
            .position(|param| param.name().text() == name)
            .or_else(|| {
                self.diagnostics.push(
                    Diagnostic::builder(
                        Severity::Error,
                        DiagnosticCode::TypeArgument,
                        format!("unknown table parameter `{name}` for `Mask`"),
                    )
                    .primary(span, "table parameter is not in scope")
                    .finish(),
                );
                None
            })
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
