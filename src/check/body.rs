use std::collections::BTreeMap;

use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticGroupId, Severity};
use crate::lexer::TokenKind;
use crate::source::Span;
use crate::syntax::{SyntaxKind, SyntaxNodeId, SyntaxTokenId};

use super::cst::{CstView, TokenText};
use super::resolve::{ItemId, ResolvedGraph};
use super::summary::MemberSummary;
use super::types::{
    BuiltinType, CheckedParam, SignatureCheck, TypeId, TypeKind, TypeTable, UnknownReason,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ScopeId(u32);

#[derive(Clone, Debug)]
pub struct BodyCheckResult {
    diagnostics: Vec<Diagnostic>,
}

#[derive(Clone, Debug)]
struct ValueSymbol {
    ty: TypeId,
}

#[derive(Clone, Debug)]
struct TableBinding {
    param_index: u32,
    name: String,
    item: ItemId,
    rows: u64,
}

#[derive(Default)]
struct UnknownNameTracker {
    entries: BTreeMap<(u32, String), UnknownNameEntry>,
    next_group: u32,
}

struct UnknownNameEntry {
    group: DiagnosticGroupId,
    name: String,
    first_span: Span,
    related_spans: Vec<Span>,
}

impl UnknownNameTracker {
    fn record(&mut self, scope: ScopeId, name: &str, span: Span) {
        let key = (scope.0, name.to_string());
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.related_spans.push(span);
            return;
        }
        let group = DiagnosticGroupId::new(self.next_group);
        self.next_group += 1;
        self.entries.insert(
            key,
            UnknownNameEntry {
                group,
                name: name.to_string(),
                first_span: span,
                related_spans: Vec::new(),
            },
        );
    }

    fn flush(self, diagnostics: &mut Vec<Diagnostic>) {
        let mut entries: Vec<_> = self.entries.into_values().collect();
        entries.sort_by_key(|entry| (entry.first_span.file_id().raw(), entry.first_span.start()));
        for entry in entries {
            let mut builder = Diagnostic::builder(
                Severity::Error,
                DiagnosticCode::ResolveUnknownName,
                format!("unknown name `{}`", entry.name),
            )
            .primary(entry.first_span, "name is not in scope")
            .root_cause_group(entry.group);
            for span in entry.related_spans {
                builder = builder.related(span, "same unresolved name is used again here");
            }
            diagnostics.push(builder.finish());
        }
    }
}

impl BodyCheckResult {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

pub struct BodyCheckContext<'a> {
    pub modules: &'a [super::ModuleInput<'a>],
    pub graph: &'a ResolvedGraph,
    pub signatures: &'a SignatureCheck,
}

pub fn check_bodies(context: BodyCheckContext<'_>) -> BodyCheckResult {
    let mut checker = BodyChecker {
        signatures: context.signatures,
        type_table: context.signatures.type_table().clone(),
        diagnostics: Vec::new(),
        next_scope: 0,
        unknown_names: UnknownNameTracker::default(),
    };

    for module in context.modules {
        let view = CstView::new(module.parsed.tree(), module.lexed, module.source);
        for item in module.summary.items() {
            for member in item.members() {
                if let Some(body) = member.body() {
                    checker.check_member_body(&view, member, body);
                }
            }
        }
    }

    checker.unknown_names.flush(&mut checker.diagnostics);

    BodyCheckResult {
        diagnostics: checker.diagnostics,
    }
}

struct BodyChecker<'a> {
    signatures: &'a SignatureCheck,
    type_table: TypeTable,
    diagnostics: Vec<Diagnostic>,
    next_scope: u32,
    unknown_names: UnknownNameTracker,
}

impl BodyChecker<'_> {
    fn check_member_body(
        &mut self,
        view: &CstView<'_>,
        member: &MemberSummary,
        body: SyntaxNodeId,
    ) {
        let Some(signature) = self.signatures.member_signature(member) else {
            return;
        };
        let params = signature.params().to_vec();
        let scope_id = self.fresh_scope();
        let mut scope = BTreeMap::<String, ValueSymbol>::new();
        for param in &params {
            scope.insert(
                param.name().text().to_string(),
                ValueSymbol { ty: param.ty() },
            );
        }
        self.check_block(
            view,
            body,
            signature.return_type(),
            &params,
            scope_id,
            &mut scope,
        );
    }

    fn fresh_scope(&mut self) -> ScopeId {
        let id = ScopeId(self.next_scope);
        self.next_scope += 1;
        id
    }

    fn check_block(
        &mut self,
        view: &CstView<'_>,
        block: SyntaxNodeId,
        expected_return: TypeId,
        params: &[CheckedParam],
        scope_id: ScopeId,
        scope: &mut BTreeMap<String, ValueSymbol>,
    ) {
        for stmt in view.child_nodes(block) {
            match view.node_kind(stmt) {
                SyntaxKind::LetStmt => self.check_let(view, stmt, params, scope_id, scope),
                SyntaxKind::ReturnStmt => {
                    self.check_return(view, stmt, expected_return, params, scope_id, scope)
                }
                SyntaxKind::ExprStmt => {
                    if let Some(expr) = first_expr_child(view, stmt) {
                        self.check_expr(view, expr, params, scope_id, scope);
                    }
                }
                SyntaxKind::MatchStmt
                | SyntaxKind::RepeatStmt
                | SyntaxKind::ForStmt
                | SyntaxKind::DrainStmt
                | SyntaxKind::LoopStmt
                | SyntaxKind::AssertStmt => {
                    self.unsupported(view, stmt, "statement form is not checked yet")
                }
                _ => {}
            }
        }
    }

    fn check_return(
        &mut self,
        view: &CstView<'_>,
        stmt: SyntaxNodeId,
        expected: TypeId,
        params: &[CheckedParam],
        scope_id: ScopeId,
        scope: &mut BTreeMap<String, ValueSymbol>,
    ) {
        let actual = first_expr_child(view, stmt)
            .map(|expr| self.check_expr(view, expr, params, scope_id, scope))
            .unwrap_or_else(|| self.type_table.builtin(BuiltinType::None));
        if !self.should_compare_types(actual, expected)
            || self.type_table.types_compatible(expected, actual)
        {
            return;
        }
        self.diagnostics.push(
            Diagnostic::builder(
                Severity::Error,
                DiagnosticCode::TypeReturn,
                "return type does not match signature",
            )
            .primary(view.node_span(stmt), "return expression has the wrong type")
            .note(format!(
                "expected `{}`, found `{}`",
                self.type_name(expected),
                self.type_name(actual)
            ))
            .finish(),
        );
    }

    fn check_let(
        &mut self,
        view: &CstView<'_>,
        stmt: SyntaxNodeId,
        params: &[CheckedParam],
        scope_id: ScopeId,
        scope: &mut BTreeMap<String, ValueSymbol>,
    ) {
        let Some(name_token) = view.first_identifier_text(stmt) else {
            return;
        };
        let Some(expr) = first_expr_child(view, stmt) else {
            return;
        };
        let ty = self.check_expr(view, expr, params, scope_id, scope);
        let annotation = view
            .first_child(stmt, SyntaxKind::TypeRef)
            .and_then(|type_ref| self.builtin_type_ref(view, type_ref));
        if let Some(expected) = annotation {
            if self.should_compare_types(expected, ty)
                && !self.type_table.types_compatible(expected, ty)
            {
                self.diagnostics.push(
                    Diagnostic::builder(
                        Severity::Error,
                        DiagnosticCode::TypeMismatch,
                        "let annotation does not match initializer",
                    )
                    .primary(
                        view.node_span(stmt),
                        format!("let annotation expects `{}`", self.type_name(expected)),
                    )
                    .finish(),
                );
            }
        }
        scope.insert(
            name_token.text().to_string(),
            ValueSymbol {
                ty: annotation.unwrap_or(ty),
            },
        );
    }

    fn check_expr(
        &mut self,
        view: &CstView<'_>,
        expr: SyntaxNodeId,
        params: &[CheckedParam],
        scope_id: ScopeId,
        scope: &mut BTreeMap<String, ValueSymbol>,
    ) -> TypeId {
        match view.node_kind(expr) {
            SyntaxKind::LiteralExpr => self.literal_type(view, expr),
            SyntaxKind::NameExpr => self.name_type(view, expr, scope_id, scope),
            SyntaxKind::ParenExpr => first_expr_child(view, expr)
                .map(|child| self.check_expr(view, child, params, scope_id, scope))
                .unwrap_or_else(|| self.unknown(UnknownReason::UnsupportedExpression)),
            SyntaxKind::BinaryExpr => self.binary_type(view, expr, params, scope_id, scope),
            SyntaxKind::PrefixExpr => {
                self.unsupported(view, expr, "prefix expressions are not checked yet");
                self.unknown(UnknownReason::UnsupportedExpression)
            }
            SyntaxKind::CallExpr => self
                .record_unsupported(view.node_span(expr), "call expressions are not checked yet"),
            SyntaxKind::FieldExpr | SyntaxKind::IndexExpr => self.record_unsupported(
                view.node_span(expr),
                "field and index expressions are not checked yet",
            ),
            SyntaxKind::TryExpr => {
                self.unsupported(view, expr, "try expressions are not checked yet");
                self.unknown(UnknownReason::UnsupportedExpression)
            }
            SyntaxKind::ReduceExpr => self.check_reduce_expr(view, expr, params, scope_id, scope),
            SyntaxKind::ScanExpr => self
                .record_unsupported(view.node_span(expr), "scan expressions are not checked yet"),
            _ => self.unknown(UnknownReason::UnsupportedExpression),
        }
    }

    fn literal_type(&self, view: &CstView<'_>, expr: SyntaxNodeId) -> TypeId {
        let text = view
            .first_identifier_text(expr)
            .map(|token| token.text().to_string());
        match text.as_deref() {
            Some("true") | Some("false") => self.type_table.builtin(BuiltinType::Bool),
            Some("None") => self.type_table.builtin(BuiltinType::None),
            _ => {
                let span_text = view
                    .child_tokens(expr)
                    .first()
                    .map(|token| view.token_text(*token).text().to_string())
                    .unwrap_or_default();
                if span_text.starts_with('"') {
                    self.type_table.builtin(BuiltinType::String)
                } else {
                    self.type_table.builtin(BuiltinType::I64)
                }
            }
        }
    }

    fn name_type(
        &mut self,
        view: &CstView<'_>,
        expr: SyntaxNodeId,
        scope_id: ScopeId,
        scope: &mut BTreeMap<String, ValueSymbol>,
    ) -> TypeId {
        let Some(token) = view.first_identifier_text(expr) else {
            return self.unknown(UnknownReason::UnresolvedExpressionName);
        };
        match token.text() {
            "true" | "false" => return self.type_table.builtin(BuiltinType::Bool),
            "None" => return self.type_table.builtin(BuiltinType::None),
            _ => {}
        }
        if let Some(symbol) = scope.get(token.text()) {
            return symbol.ty;
        }
        self.record_unknown_name(scope_id, token.text(), token.span());
        self.unknown(UnknownReason::UnresolvedExpressionName)
    }

    fn binary_type(
        &mut self,
        view: &CstView<'_>,
        expr: SyntaxNodeId,
        params: &[CheckedParam],
        scope_id: ScopeId,
        scope: &mut BTreeMap<String, ValueSymbol>,
    ) -> TypeId {
        let children = expr_children(view, expr);
        if children.len() != 2 {
            return self.unknown(UnknownReason::UnsupportedExpression);
        }
        let left = self.check_expr(view, children[0], params, scope_id, scope);
        let right = self.check_expr(view, children[1], params, scope_id, scope);
        if self.type_table.is_unknown(left) || self.type_table.is_unknown(right) {
            return self.unknown(UnknownReason::UnsupportedExpression);
        }
        if self.type_table.is_integer(left) && self.type_table.is_integer(right) {
            return left;
        }
        self.diagnostics.push(
            Diagnostic::builder(
                Severity::Error,
                DiagnosticCode::TypeMismatch,
                "binary operands have incompatible types",
            )
            .primary(view.node_span(expr), "operands must be compatible")
            .finish(),
        );
        self.unknown(UnknownReason::UnsupportedExpression)
    }

    fn record_unsupported(&mut self, span: Span, message: impl Into<String>) -> TypeId {
        let message = message.into();
        self.diagnostics.push(
            Diagnostic::builder(Severity::Error, DiagnosticCode::CheckUnsupported, &message)
                .primary(span, message)
                .finish(),
        );
        self.unknown(UnknownReason::UnsupportedExpression)
    }

    fn check_reduce_expr(
        &mut self,
        view: &CstView<'_>,
        expr: SyntaxNodeId,
        params: &[CheckedParam],
        scope_id: ScopeId,
        scope: &mut BTreeMap<String, ValueSymbol>,
    ) -> TypeId {
        let span = view.node_span(expr);
        let Some(source) = expr_children(view, expr).into_iter().next() else {
            return self.record_unsupported(span, "reduce expression is missing a source");
        };
        let Some((field_expr, mask_expr)) = parse_rows_call(view, source) else {
            return self.record_unsupported(
                view.node_span(source),
                "reduce source must be `table.rows(mask)`",
            );
        };
        let Some(table_name) = parse_rows_receiver(view, field_expr) else {
            return self.record_unsupported(
                view.node_span(field_expr),
                "reduce source must call `.rows` on a table parameter",
            );
        };
        let Some(table) = self.table_binding(params, table_name) else {
            return self.record_unsupported(
                view.node_span(field_expr),
                format!("`{table_name}` must be a table parameter"),
            );
        };
        let Some(mask_name) = view.first_identifier_text(mask_expr) else {
            return self.record_unsupported(
                view.node_span(mask_expr),
                "reduce mask must be a parameter name",
            );
        };
        if self
            .mask_binding(params, mask_name.text(), &table)
            .is_none()
        {
            return self.record_unsupported(
                view.node_span(mask_expr),
                format!(
                    "`{}` must be a mask for table `{table_name}`",
                    mask_name.text()
                ),
            );
        }

        let Some(row_name) = reduce_row_name(view, expr) else {
            return self.record_unsupported(span, "reduce requires `as row` row binding");
        };
        if row_name.text() != "row" {
            return self
                .record_unsupported(row_name.span(), "reduce row binding must be named `row`");
        }

        let Some(acc_type) = reduce_accumulator_type(view, expr, &self.type_table) else {
            return self
                .record_unsupported(span, "reduce accumulator type must be U64, U32, or I64");
        };

        let Some(init_expr) = reduce_init_expr(view, expr) else {
            return self.record_unsupported(span, "reduce is missing an accumulator initializer");
        };
        let init_type = self.check_expr(view, init_expr, params, scope_id, scope);
        if self.should_compare_types(acc_type, init_type)
            && !self.type_table.types_compatible(acc_type, init_type)
        {
            self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::TypeMismatch,
                    "reduce accumulator initializer has the wrong type",
                )
                .primary(view.node_span(init_expr), "initializer type mismatch")
                .note(format!(
                    "expected `{}`, found `{}`",
                    self.type_name(acc_type),
                    self.type_name(init_type)
                ))
                .finish(),
            );
        }

        let Some(body) = view.first_child(expr, SyntaxKind::Block) else {
            return self.record_unsupported(span, "reduce is missing a body block");
        };
        let acc_name = reduce_accumulator_name(view, expr).unwrap_or_else(|| "acc".to_string());
        self.check_reduce_body(
            view,
            body,
            params,
            scope,
            &table,
            row_name.text(),
            &acc_name,
            acc_type,
        );
        acc_type
    }

    #[allow(clippy::too_many_arguments)]
    fn check_reduce_body(
        &mut self,
        view: &CstView<'_>,
        block: SyntaxNodeId,
        params: &[CheckedParam],
        scope: &BTreeMap<String, ValueSymbol>,
        table: &TableBinding,
        row_name: &str,
        acc_name: &str,
        acc_type: TypeId,
    ) {
        let mut body_scope = scope.clone();
        body_scope.insert(acc_name.to_string(), ValueSymbol { ty: acc_type });

        for stmt in view.child_nodes(block) {
            match view.node_kind(stmt) {
                SyntaxKind::ReturnStmt => self.check_reduce_return(
                    view,
                    stmt,
                    params,
                    &body_scope,
                    table,
                    row_name,
                    acc_type,
                ),
                SyntaxKind::ReduceExpr | SyntaxKind::ScanExpr => {
                    let _ = self.record_unsupported(
                        view.node_span(stmt),
                        "nested data-plane expressions are not supported",
                    );
                }
                _ => {
                    let _ = self.record_unsupported(
                        view.node_span(stmt),
                        "reduce body only supports return statements",
                    );
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn check_reduce_return(
        &mut self,
        view: &CstView<'_>,
        stmt: SyntaxNodeId,
        params: &[CheckedParam],
        scope: &BTreeMap<String, ValueSymbol>,
        table: &TableBinding,
        row_name: &str,
        acc_type: TypeId,
    ) {
        let Some(expr) = first_expr_child(view, stmt) else {
            self.record_unsupported(view.node_span(stmt), "reduce return requires an expression");
            return;
        };
        let actual =
            self.check_reduce_return_expr(view, expr, params, scope, table, row_name, acc_type);
        if !self.should_compare_types(actual, acc_type)
            || self.type_table.types_compatible(acc_type, actual)
        {
            return;
        }
        self.diagnostics.push(
            Diagnostic::builder(
                Severity::Error,
                DiagnosticCode::TypeMismatch,
                "reduce return expression has the wrong type",
            )
            .primary(view.node_span(expr), "return expression type mismatch")
            .note(format!(
                "expected `{}`, found `{}`",
                self.type_name(acc_type),
                self.type_name(actual)
            ))
            .finish(),
        );
    }

    #[allow(clippy::too_many_arguments, clippy::only_used_in_recursion)]
    fn check_reduce_return_expr(
        &mut self,
        view: &CstView<'_>,
        expr: SyntaxNodeId,
        params: &[CheckedParam],
        scope: &BTreeMap<String, ValueSymbol>,
        table: &TableBinding,
        row_name: &str,
        acc_type: TypeId,
    ) -> TypeId {
        match view.node_kind(expr) {
            SyntaxKind::LiteralExpr => self.literal_type(view, expr),
            SyntaxKind::NameExpr => self.name_type_from_scope(view, expr, scope),
            SyntaxKind::ParenExpr => first_expr_child(view, expr)
                .map(|child| {
                    self.check_reduce_return_expr(
                        view, child, params, scope, table, row_name, acc_type,
                    )
                })
                .unwrap_or_else(|| self.unknown(UnknownReason::UnsupportedExpression)),
            SyntaxKind::BinaryExpr => {
                let children = expr_children(view, expr);
                if children.len() != 2 {
                    return self.record_unsupported(
                        view.node_span(expr),
                        "invalid binary expression in reduce return",
                    );
                }
                let left = self.check_reduce_return_expr(
                    view,
                    children[0],
                    params,
                    scope,
                    table,
                    row_name,
                    acc_type,
                );
                let right = self.check_reduce_return_expr(
                    view,
                    children[1],
                    params,
                    scope,
                    table,
                    row_name,
                    acc_type,
                );
                if self.type_table.is_unknown(left) || self.type_table.is_unknown(right) {
                    return self.unknown(UnknownReason::UnsupportedExpression);
                }
                if self.type_table.is_integer(left) && self.type_table.is_integer(right) {
                    return acc_type;
                }
                self.diagnostics.push(
                    Diagnostic::builder(
                        Severity::Error,
                        DiagnosticCode::TypeMismatch,
                        "reduce return operands have incompatible types",
                    )
                    .primary(view.node_span(expr), "operands must be compatible")
                    .finish(),
                );
                self.unknown(UnknownReason::UnsupportedExpression)
            }
            SyntaxKind::IndexExpr => self.check_table_index_field(view, expr, table, row_name),
            SyntaxKind::FieldExpr
            | SyntaxKind::CallExpr
            | SyntaxKind::ReduceExpr
            | SyntaxKind::ScanExpr
            | SyntaxKind::PrefixExpr
            | SyntaxKind::TryExpr => self.record_unsupported(
                view.node_span(expr),
                "expression is not supported in reduce return",
            ),
            _ => self.unknown(UnknownReason::UnsupportedExpression),
        }
    }

    fn check_table_index_field(
        &mut self,
        view: &CstView<'_>,
        expr: SyntaxNodeId,
        table: &TableBinding,
        row_name: &str,
    ) -> TypeId {
        let Some((table_name, field_name, index_name)) = parse_table_index_field(view, expr) else {
            return self.record_unsupported(
                view.node_span(expr),
                "expected `table.field[row]` index expression",
            );
        };
        if table_name != table.name {
            return self.record_unsupported(
                view.node_span(expr),
                format!(
                    "table index must use reduce table `{name}`",
                    name = table.name
                ),
            );
        }
        if index_name != row_name {
            return self.record_unsupported(
                view.node_span(expr),
                format!("table index must use reduce row binding `{row_name}`"),
            );
        }
        if let Some(field_type) = self.signatures.data_field_type(table.item, field_name) {
            return field_type;
        }
        self.record_unsupported(
            view.node_span(expr),
            format!("unknown field `{field_name}` on table row type"),
        )
    }

    fn table_binding(&self, params: &[CheckedParam], name: &str) -> Option<TableBinding> {
        for (index, param) in params.iter().enumerate() {
            if param.name().text() != name {
                continue;
            }
            if let Some(TypeKind::Table { item, rows }) = self.type_table.kind(param.ty()) {
                return Some(TableBinding {
                    param_index: index as u32,
                    name: name.to_string(),
                    item,
                    rows,
                });
            }
            return None;
        }
        None
    }

    fn mask_binding(
        &self,
        params: &[CheckedParam],
        name: &str,
        table: &TableBinding,
    ) -> Option<TypeId> {
        for param in params {
            if param.name().text() != name {
                continue;
            }
            if let Some(TypeKind::Mask { table_param, rows }) = self.type_table.kind(param.ty()) {
                if table_param == table.param_index && rows == table.rows {
                    return Some(param.ty());
                }
            }
            return None;
        }
        None
    }

    fn name_type_from_scope(
        &mut self,
        view: &CstView<'_>,
        expr: SyntaxNodeId,
        scope: &BTreeMap<String, ValueSymbol>,
    ) -> TypeId {
        let Some(token) = view.first_identifier_text(expr) else {
            return self.unknown(UnknownReason::UnresolvedExpressionName);
        };
        match token.text() {
            "true" | "false" => return self.type_table.builtin(BuiltinType::Bool),
            "None" => return self.type_table.builtin(BuiltinType::None),
            _ => {}
        }
        if let Some(symbol) = scope.get(token.text()) {
            return symbol.ty;
        }
        self.record_unsupported(
            token.span(),
            format!("unknown name `{}` in reduce body", token.text()),
        )
    }

    fn unknown(&mut self, reason: UnknownReason) -> TypeId {
        self.type_table.push_unknown(reason)
    }

    fn unsupported(&mut self, view: &CstView<'_>, node: SyntaxNodeId, message: &'static str) {
        self.record_unsupported(view.node_span(node), message);
    }

    fn record_unknown_name(&mut self, scope_id: ScopeId, name: &str, span: Span) {
        self.unknown_names.record(scope_id, name, span);
    }

    fn type_name(&self, ty: TypeId) -> &'static str {
        match self.type_table.kind(ty) {
            Some(TypeKind::Builtin(BuiltinType::Bool)) => "Bool",
            Some(TypeKind::Builtin(BuiltinType::I64)) => "I64",
            Some(TypeKind::Builtin(BuiltinType::U32)) => "U32",
            Some(TypeKind::Builtin(BuiltinType::U64)) => "U64",
            Some(TypeKind::Builtin(BuiltinType::String)) => "String",
            Some(TypeKind::Builtin(BuiltinType::None)) => "None",
            Some(TypeKind::Item(_, _)) => "item",
            Some(TypeKind::Table { .. }) => "Table",
            Some(TypeKind::Mask { .. }) => "Mask",
            Some(TypeKind::RowToken { .. }) => "RowToken",
            Some(TypeKind::Unknown(_)) | None => "unknown",
        }
    }

    fn builtin_type_ref(&self, view: &CstView<'_>, type_ref: SyntaxNodeId) -> Option<TypeId> {
        let name = view.first_identifier_text(type_ref)?.text();
        match name {
            "Bool" => Some(self.type_table.builtin(BuiltinType::Bool)),
            "I64" => Some(self.type_table.builtin(BuiltinType::I64)),
            "U32" => Some(self.type_table.builtin(BuiltinType::U32)),
            "U64" => Some(self.type_table.builtin(BuiltinType::U64)),
            "String" => Some(self.type_table.builtin(BuiltinType::String)),
            "None" => Some(self.type_table.builtin(BuiltinType::None)),
            _ => None,
        }
    }

    fn should_compare_types(&self, left: TypeId, right: TypeId) -> bool {
        !self.type_table.is_unknown(left) && !self.type_table.is_unknown(right)
    }
}

fn is_expr_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::NameExpr
            | SyntaxKind::LiteralExpr
            | SyntaxKind::ParenExpr
            | SyntaxKind::PrefixExpr
            | SyntaxKind::BinaryExpr
            | SyntaxKind::CallExpr
            | SyntaxKind::FieldExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::TryExpr
            | SyntaxKind::ReduceExpr
            | SyntaxKind::ScanExpr
    )
}

fn first_expr_child(view: &CstView<'_>, node: SyntaxNodeId) -> Option<SyntaxNodeId> {
    view.child_nodes(node)
        .into_iter()
        .find(|child| is_expr_kind(view.node_kind(*child)))
}

fn expr_children(view: &CstView<'_>, node: SyntaxNodeId) -> Vec<SyntaxNodeId> {
    view.child_nodes(node)
        .into_iter()
        .filter(|child| is_expr_kind(view.node_kind(*child)))
        .collect()
}

fn parse_rows_call(view: &CstView<'_>, expr: SyntaxNodeId) -> Option<(SyntaxNodeId, SyntaxNodeId)> {
    if view.node_kind(expr) != SyntaxKind::CallExpr {
        return None;
    }
    let field_expr = expr_children(view, expr).into_iter().next()?;
    if view.node_kind(field_expr) != SyntaxKind::FieldExpr {
        return None;
    }
    if parse_field_name(view, field_expr)? != "rows" {
        return None;
    }
    let arg_list = view.first_child(expr, SyntaxKind::ArgList)?;
    let arg = view.first_child(arg_list, SyntaxKind::Arg)?;
    let mask_expr = first_expr_child(view, arg)?;
    Some((field_expr, mask_expr))
}

fn parse_rows_receiver<'a>(view: &'a CstView<'a>, field_expr: SyntaxNodeId) -> Option<&'a str> {
    if parse_field_name(view, field_expr)? != "rows" {
        return None;
    }
    field_receiver_name(view, field_expr)
}

fn parse_table_index_field<'a>(
    view: &'a CstView<'a>,
    expr: SyntaxNodeId,
) -> Option<(&'a str, &'a str, &'a str)> {
    if view.node_kind(expr) != SyntaxKind::IndexExpr {
        return None;
    }
    let children = expr_children(view, expr);
    let base = children.first()?;
    if view.node_kind(*base) != SyntaxKind::FieldExpr {
        return None;
    }
    let table_name = field_receiver_name(view, *base)?;
    let field_name = parse_field_name(view, *base)?;
    let index_expr = children.get(1)?;
    let index_name = view.first_identifier_text(*index_expr)?.text();
    Some((table_name, field_name, index_name))
}

fn parse_field_name<'a>(view: &'a CstView<'a>, field_expr: SyntaxNodeId) -> Option<&'a str> {
    for token in view.child_tokens(field_expr) {
        if is_identifier_token(view, token) {
            return Some(view.token_text(token).text());
        }
    }
    None
}

fn field_receiver_name<'a>(view: &'a CstView<'a>, field_expr: SyntaxNodeId) -> Option<&'a str> {
    let receiver = expr_children(view, field_expr).into_iter().next()?;
    if view.node_kind(receiver) != SyntaxKind::NameExpr {
        return None;
    }
    view.first_identifier_text(receiver)
        .map(|token| token.text())
}

fn reduce_row_name<'a>(view: &CstView<'a>, reduce: SyntaxNodeId) -> Option<TokenText<'a>> {
    let mut after_as = false;
    for token in view.child_tokens(reduce) {
        let text = view.token_text(token);
        if text.text() == "as" {
            after_as = true;
            continue;
        }
        if after_as && is_identifier_token(view, token) {
            return Some(text);
        }
    }
    None
}

fn reduce_accumulator_type(
    view: &CstView<'_>,
    reduce: SyntaxNodeId,
    type_table: &TypeTable,
) -> Option<TypeId> {
    let type_ref = view.first_child(reduce, SyntaxKind::TypeRef)?;
    let name = view.first_identifier_text(type_ref)?.text();
    match name {
        "U64" => Some(type_table.builtin(BuiltinType::U64)),
        "U32" => Some(type_table.builtin(BuiltinType::U32)),
        "I64" => Some(type_table.builtin(BuiltinType::I64)),
        _ => None,
    }
}

fn reduce_init_expr(view: &CstView<'_>, reduce: SyntaxNodeId) -> Option<SyntaxNodeId> {
    let exprs = expr_children(view, reduce);
    if exprs.len() >= 2 {
        return Some(exprs[1]);
    }
    None
}

fn reduce_accumulator_name(view: &CstView<'_>, reduce: SyntaxNodeId) -> Option<String> {
    let mut after_comma = false;
    for token in view.child_tokens(reduce) {
        let text = view.token_text(token).text();
        if text == "," {
            after_comma = true;
            continue;
        }
        if after_comma && is_identifier_token(view, token) {
            return Some(text.to_string());
        }
    }
    None
}

fn is_identifier_token(view: &CstView<'_>, token: SyntaxTokenId) -> bool {
    let syntax_token = view.tree().token(token);
    let raw = view.lexed().tokens()[syntax_token.token().raw() as usize];
    matches!(raw.kind(), TokenKind::Identifier)
}
