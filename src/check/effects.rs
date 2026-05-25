use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::syntax::{SyntaxKind, SyntaxNodeId};

use super::cst::CstView;
use super::summary::MemberKind;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EffectSet {
    unknown: bool,
}

impl EffectSet {
    pub const fn pure() -> Self {
        Self { unknown: false }
    }

    pub const fn unknown() -> Self {
        Self { unknown: true }
    }

    pub const fn has_unknown(self) -> bool {
        self.unknown
    }

    pub fn union(self, other: Self) -> Self {
        Self {
            unknown: self.unknown || other.unknown,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EffectCheck {
    diagnostics: Vec<Diagnostic>,
}

impl EffectCheck {
    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }
}

pub struct EffectContext<'a> {
    pub modules: &'a [super::ModuleInput<'a>],
}

pub fn check_effects(context: EffectContext<'_>) -> EffectCheck {
    let mut diagnostics = Vec::new();

    for module in context.modules {
        let view = CstView::new(module.parsed.tree(), module.lexed, module.source);
        for item in module.summary.items() {
            for member in item.members() {
                let Some(body) = member.body() else {
                    continue;
                };
                let effects = infer_block_effects(&view, body);
                if effects.has_unknown()
                    && matches!(member.kind(), MemberKind::Phase | MemberKind::Test)
                {
                    diagnostics.push(
                        Diagnostic::builder(
                            Severity::Error,
                            DiagnosticCode::EffectUnsupported,
                            "body has unknown effects",
                        )
                        .primary(member.span(), context_message(member.kind()))
                        .finish(),
                    );
                }
            }
        }
    }

    EffectCheck { diagnostics }
}

fn context_message(kind: MemberKind) -> &'static str {
    match kind {
        MemberKind::Phase => "phase bodies cannot carry unknown effects",
        MemberKind::Test => "test bodies cannot carry unknown effects",
        _ => "body cannot carry unknown effects",
    }
}

fn infer_block_effects(view: &CstView<'_>, block: SyntaxNodeId) -> EffectSet {
    let mut effects = EffectSet::pure();
    for child in view.child_nodes(block) {
        effects = effects.union(infer_node_effects(view, child));
    }
    effects
}

fn infer_node_effects(view: &CstView<'_>, node: SyntaxNodeId) -> EffectSet {
    match view.node_kind(node) {
        SyntaxKind::CallExpr
        | SyntaxKind::LoopStmt
        | SyntaxKind::TryExpr
        | SyntaxKind::ReduceExpr
        | SyntaxKind::ScanExpr => EffectSet::unknown(),
        _ => {
            let mut effects = EffectSet::pure();
            for child in view.child_nodes(node) {
                effects = effects.union(infer_node_effects(view, child));
            }
            effects
        }
    }
}
