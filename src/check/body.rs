use std::collections::BTreeMap;

use crate::diagnostic::{Diagnostic, DiagnosticCode, DiagnosticGroupId, Severity};
use crate::source::Span;
use crate::syntax::{SyntaxKind, SyntaxNodeId};

use super::cst::CstView;
use super::resolve::ResolvedGraph;
use super::summary::MemberSummary;
use super::types::{BuiltinType, SignatureCheck, TypeId, TypeTable, UnknownReason};

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
        let scope_id = self.fresh_scope();
        let mut scope = BTreeMap::<String, ValueSymbol>::new();
        for param in signature.params() {
            scope.insert(
                param.name().text().to_string(),
                ValueSymbol { ty: param.ty() },
            );
        }
        self.check_block(view, body, signature.return_type(), scope_id, &mut scope);
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
        scope_id: ScopeId,
        scope: &mut BTreeMap<String, ValueSymbol>,
    ) {
        for stmt in view.child_nodes(block) {
            match view.node_kind(stmt) {
                SyntaxKind::LetStmt => self.check_let(view, stmt, scope_id, scope),
                SyntaxKind::ReturnStmt => {
                    self.check_return(view, stmt, expected_return, scope_id, scope)
                }
                SyntaxKind::ExprStmt => {
                    if let Some(expr) = first_expr_child(view, stmt) {
                        self.check_expr(view, expr, scope_id, scope);
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
        scope_id: ScopeId,
        scope: &mut BTreeMap<String, ValueSymbol>,
    ) {
        let actual = first_expr_child(view, stmt)
            .map(|expr| self.check_expr(view, expr, scope_id, scope))
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
        scope_id: ScopeId,
        scope: &mut BTreeMap<String, ValueSymbol>,
    ) {
        let Some(name_token) = view.first_identifier_text(stmt) else {
            return;
        };
        let Some(expr) = first_expr_child(view, stmt) else {
            return;
        };
        let ty = self.check_expr(view, expr, scope_id, scope);
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
        scope_id: ScopeId,
        scope: &mut BTreeMap<String, ValueSymbol>,
    ) -> TypeId {
        match view.node_kind(expr) {
            SyntaxKind::LiteralExpr => self.literal_type(view, expr),
            SyntaxKind::NameExpr => self.name_type(view, expr, scope_id, scope),
            SyntaxKind::ParenExpr => first_expr_child(view, expr)
                .map(|child| self.check_expr(view, child, scope_id, scope))
                .unwrap_or_else(|| self.unknown(UnknownReason::UnsupportedExpression)),
            SyntaxKind::BinaryExpr => self.binary_type(view, expr, scope_id, scope),
            SyntaxKind::PrefixExpr => {
                self.unsupported(view, expr, "prefix expressions are not checked yet");
                self.unknown(UnknownReason::UnsupportedExpression)
            }
            SyntaxKind::CallExpr => {
                self.unsupported(view, expr, "call expressions are not checked yet");
                self.unknown(UnknownReason::UnsupportedExpression)
            }
            SyntaxKind::FieldExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::TryExpr
            | SyntaxKind::ReduceExpr
            | SyntaxKind::ScanExpr => {
                self.unsupported(view, expr, "expression form is not checked yet");
                self.unknown(UnknownReason::UnsupportedExpression)
            }
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
        scope_id: ScopeId,
        scope: &mut BTreeMap<String, ValueSymbol>,
    ) -> TypeId {
        let children = view
            .child_nodes(expr)
            .into_iter()
            .filter(|child| is_expr_kind(view.node_kind(*child)))
            .collect::<Vec<_>>();
        if children.len() != 2 {
            return self.unknown(UnknownReason::UnsupportedExpression);
        }
        let left = self.check_expr(view, children[0], scope_id, scope);
        let right = self.check_expr(view, children[1], scope_id, scope);
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

    fn unknown(&mut self, reason: UnknownReason) -> TypeId {
        self.type_table.push_unknown(reason)
    }

    fn unsupported(&mut self, view: &CstView<'_>, node: SyntaxNodeId, message: &'static str) {
        self.diagnostics.push(
            Diagnostic::builder(Severity::Error, DiagnosticCode::CheckUnsupported, message)
                .primary(view.node_span(node), message)
                .finish(),
        );
    }

    fn record_unknown_name(&mut self, scope_id: ScopeId, name: &str, span: Span) {
        self.unknown_names.record(scope_id, name, span);
    }

    fn type_name(&self, ty: TypeId) -> &'static str {
        match self.type_table.kind(ty) {
            Some(super::types::TypeKind::Builtin(BuiltinType::Bool)) => "Bool",
            Some(super::types::TypeKind::Builtin(BuiltinType::I64)) => "I64",
            Some(super::types::TypeKind::Builtin(BuiltinType::U32)) => "U32",
            Some(super::types::TypeKind::Builtin(BuiltinType::U64)) => "U64",
            Some(super::types::TypeKind::Builtin(BuiltinType::String)) => "String",
            Some(super::types::TypeKind::Builtin(BuiltinType::None)) => "None",
            Some(super::types::TypeKind::Item(_, _)) => "item",
            Some(super::types::TypeKind::Unknown(_)) | None => "unknown",
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
