use std::collections::BTreeMap;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::source::Span;
use crate::syntax::{SyntaxKind, SyntaxNodeId};

use super::cst::CstView;
use super::summary::{AccessMode, MemberSummary, Name};
use super::types::{BuiltinType, CheckedSignature, SignatureCheck};

#[derive(Clone, Debug)]
pub struct OwnershipCheck {
    diagnostics: Vec<Diagnostic>,
}

impl OwnershipCheck {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

pub struct OwnershipContext<'a> {
    pub modules: &'a [super::ModuleInput<'a>],
    pub signatures: &'a SignatureCheck,
}

#[derive(Clone, Debug)]
struct OwnershipValue {
    name: Name,
    access: AccessMode,
    moved_at: Option<Span>,
}

pub fn check_ownership(context: OwnershipContext<'_>) -> OwnershipCheck {
    let mut checker = OwnershipChecker {
        signatures: context.signatures,
        diagnostics: Vec::new(),
    };

    for module in context.modules {
        let view = CstView::new(module.parsed.tree(), module.lexed, module.source);
        for item in module.summary.items() {
            for member in item.members() {
                if let Some(body) = member.body() {
                    checker.check_member(&view, member, body);
                }
            }
        }
    }

    OwnershipCheck {
        diagnostics: checker.diagnostics,
    }
}

struct OwnershipChecker<'a> {
    signatures: &'a SignatureCheck,
    diagnostics: Vec<Diagnostic>,
}

impl OwnershipChecker<'_> {
    fn check_member(&mut self, view: &CstView<'_>, member: &MemberSummary, body: SyntaxNodeId) {
        let Some(signature) = self.signatures.member_signature(member) else {
            return;
        };
        let mut values = BTreeMap::<String, OwnershipValue>::new();
        for param in signature.params() {
            values.insert(
                param.name().text().to_string(),
                OwnershipValue {
                    name: param.name().clone(),
                    access: param.access(),
                    moved_at: None,
                },
            );
        }
        self.check_block(view, body, signature, &mut values);
    }

    fn check_block(
        &mut self,
        view: &CstView<'_>,
        block: SyntaxNodeId,
        signature: &CheckedSignature,
        values: &mut BTreeMap<String, OwnershipValue>,
    ) {
        for stmt in view.child_nodes(block) {
            match view.node_kind(stmt) {
                SyntaxKind::LetStmt => self.check_let(view, stmt, values),
                SyntaxKind::ReturnStmt => self.check_return(view, stmt, signature, values),
                _ => {}
            }
        }
    }

    fn check_let(
        &mut self,
        view: &CstView<'_>,
        stmt: SyntaxNodeId,
        values: &mut BTreeMap<String, OwnershipValue>,
    ) {
        let Some(name_token) = view.first_identifier_text(stmt) else {
            return;
        };
        if let Some(expr) = first_expr_child(view, stmt) {
            self.consume_expr_as_move(view, expr, values);
        }
        values.insert(
            name_token.text().to_string(),
            OwnershipValue {
                name: Name::new(name_token.text(), name_token.span()),
                access: AccessMode::Own,
                moved_at: None,
            },
        );
    }

    fn check_return(
        &mut self,
        view: &CstView<'_>,
        stmt: SyntaxNodeId,
        signature: &CheckedSignature,
        values: &mut BTreeMap<String, OwnershipValue>,
    ) {
        if signature.return_type() == self.signatures.type_table().builtin(BuiltinType::None) {
            return;
        }
        if let Some(expr) = first_expr_child(view, stmt) {
            self.consume_expr_as_move(view, expr, values);
        }
    }

    fn consume_expr_as_move(
        &mut self,
        view: &CstView<'_>,
        expr: SyntaxNodeId,
        values: &mut BTreeMap<String, OwnershipValue>,
    ) {
        if view.node_kind(expr) != SyntaxKind::NameExpr {
            return;
        }
        let Some(token) = view.first_identifier_text(expr) else {
            return;
        };
        let Some(value) = values.get_mut(token.text()) else {
            return;
        };
        if let Some(moved_at) = value.moved_at {
            self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::OwnershipMove,
                    format!("value `{}` used after move", token.text()),
                )
                .primary(token.span(), "value used after it was moved")
                .secondary(moved_at, "value was moved here")
                .finish(),
            );
            return;
        }
        match value.access {
            AccessMode::Own => value.moved_at = Some(token.span()),
            AccessMode::Read | AccessMode::Mut => self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::OwnershipAccess,
                    format!(
                        "`{:?}` value `{}` cannot be moved",
                        value.access,
                        token.text()
                    ),
                )
                .primary(token.span(), "`read` value cannot be moved")
                .secondary(value.name.span(), "access mode is declared here")
                .finish(),
            ),
        }
    }
}

fn first_expr_child(view: &CstView<'_>, node: SyntaxNodeId) -> Option<SyntaxNodeId> {
    view.child_nodes(node).into_iter().find(|child| {
        matches!(
            view.node_kind(*child),
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
    })
}
