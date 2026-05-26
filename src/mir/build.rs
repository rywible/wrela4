use std::collections::BTreeMap;

use crate::check::CheckResult;
use crate::check::cst::CstView;
use crate::check::resolve::ItemId;
use crate::check::summary::{ItemKind, MemberSummary};
use crate::check::types::{CheckedParam, TypeKind};
use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::lexer::{LexedFile, Punct, TokenKind};
use crate::source::SourceFile;
use crate::syntax::{ParsedSyntax, SyntaxKind, SyntaxNodeId};

use super::facts::OwnershipMode;
use super::report::report_for_module;
use super::semantic::{
    infer_mir_effects, make_operation, make_operation_with_facts, ownership_for_access,
};
use super::{
    BlockData, BlockId, EffectSet, MirModule, MirReport, MirType, OperationKind, PlaceData,
    RegionData, ScalarType, ValueData, ValueId,
};

#[derive(Clone, Debug)]
pub struct MirBuildResult {
    module: Option<MirModule>,
    diagnostics: Vec<Diagnostic>,
    report: MirReport,
}

impl MirBuildResult {
    pub fn failed(diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            module: None,
            diagnostics,
            report: MirReport::default(),
        }
    }

    pub fn new(module: MirModule, report: MirReport) -> Self {
        Self {
            module: Some(module),
            diagnostics: Vec::new(),
            report,
        }
    }

    pub fn module(&self) -> Option<&MirModule> {
        self.module.as_ref()
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn report(&self) -> &MirReport {
        &self.report
    }

    pub fn ok(&self) -> bool {
        self.module.is_some() && self.diagnostics.is_empty()
    }
}

pub fn build_mir(check: &CheckResult) -> MirBuildResult {
    if !check.ok() {
        return MirBuildResult::failed(check.diagnostics().to_vec());
    }

    let parsed_by_file = check
        .parsed()
        .iter()
        .map(|parsed| (parsed.file_id(), parsed))
        .collect::<BTreeMap<_, _>>();
    let lexed_by_file = check
        .lexed_files()
        .iter()
        .map(|lexed| (lexed.file_id(), lexed))
        .collect::<BTreeMap<_, _>>();
    let source_by_file = check
        .source_map()
        .files()
        .iter()
        .map(|source| (source.id(), source))
        .collect::<BTreeMap<_, _>>();

    let mut summaries = check.summaries().iter().collect::<Vec<_>>();
    summaries.sort_by(|left, right| {
        left.file_id().raw().cmp(&right.file_id().raw()).then(
            left.module_path()
                .as_dotted()
                .cmp(&right.module_path().as_dotted()),
        )
    });

    let mut module = MirModule::new();
    let mut mir_diagnostics = Vec::new();

    for summary in summaries {
        let Some(parsed) = parsed_by_file.get(&summary.file_id()).copied() else {
            continue;
        };
        let Some(lexed) = lexed_by_file.get(&summary.file_id()).copied() else {
            continue;
        };
        let Some(source) = source_by_file.get(&summary.file_id()).copied() else {
            continue;
        };
        let module_path = summary.module_path().as_dotted();

        for item in summary.items() {
            match item.kind() {
                ItemKind::Image | ItemKind::HostImage => {
                    let symbol = format!("{module_path}.{}", item.name().text());
                    module.push_region(RegionData::omega(symbol));
                }
                _ => {}
            }

            for member in item.members() {
                let Some(body) = member.body() else {
                    continue;
                };
                let member_name = member
                    .name()
                    .map(|name| name.text().to_string())
                    .unwrap_or_else(|| "constructor".to_string());
                let symbol = format!("{module_path}.{}.{}", item.name().text(), member_name);
                let region = module.push_region(RegionData::lambda(symbol));
                let block = module.push_block(region, BlockData::new("entry".to_string()));
                let mut locals = BTreeMap::new();

                if let Some(signature) = check.signature_check().member_signature(member) {
                    for param in signature.params() {
                        let ty = mir_type_for_param(check, member, param);
                        let arg_value = module.push_value(ValueData::new(ty.clone()));
                        module.push_block_argument(block, arg_value);
                        let _place = module.push_place(PlaceData::new(
                            param.name().text().to_string(),
                            ty.clone(),
                        ));
                        let loaded = module.push_value(ValueData::new(ty));
                        module.push_operation(
                            block,
                            make_operation(
                                OperationKind::ReadValue(param.name().text().to_string()),
                                Vec::new(),
                                vec![loaded],
                                EffectSet::empty(),
                                vec![ownership_for_access(param.access())],
                            ),
                        );
                        locals.insert(param.name().text().to_string(), loaded);
                    }
                }

                let mut lowerer = BodyLowerer::new(
                    check,
                    member,
                    parsed,
                    lexed,
                    source,
                    &mut module,
                    block,
                    locals,
                );
                lowerer.lower_block(body);
                mir_diagnostics.extend(lowerer.into_diagnostics());
            }
        }
    }

    if !mir_diagnostics.is_empty() {
        return MirBuildResult::failed(mir_diagnostics);
    }

    let report = report_for_module(&module);
    MirBuildResult::new(module, report)
}

fn mir_type_for_param(
    check: &CheckResult,
    member: &MemberSummary,
    param: &CheckedParam,
) -> MirType {
    let table = check.signature_check().type_table();
    match table.kind(param.ty()) {
        Some(TypeKind::Table { rows, .. }) => MirType::Table {
            item: param.name().text().to_string(),
            rows,
        },
        Some(TypeKind::Mask { table_param, rows }) => {
            let table = table_param_name(check, member, table_param).unwrap_or_default();
            MirType::Mask { table, rows }
        }
        _ => mir_type_for_checked_type(table, param.ty()),
    }
}

fn table_param_name(
    check: &CheckResult,
    member: &MemberSummary,
    table_param: u32,
) -> Option<String> {
    let signature = check.signature_check().member_signature(member)?;
    signature
        .params()
        .get(table_param as usize)
        .map(|param| param.name().text().to_string())
}

fn mir_type_for_checked_type(
    table: &crate::check::types::TypeTable,
    id: crate::check::types::TypeId,
) -> MirType {
    match table.kind(id) {
        Some(crate::check::types::TypeKind::Builtin(crate::check::types::BuiltinType::Bool)) => {
            MirType::Scalar(ScalarType::Bool)
        }
        Some(crate::check::types::TypeKind::Builtin(crate::check::types::BuiltinType::I64)) => {
            MirType::Scalar(ScalarType::I64)
        }
        Some(crate::check::types::TypeKind::Builtin(crate::check::types::BuiltinType::U32)) => {
            MirType::Scalar(ScalarType::U32)
        }
        Some(crate::check::types::TypeKind::Builtin(crate::check::types::BuiltinType::U64)) => {
            MirType::Scalar(ScalarType::U64)
        }
        Some(crate::check::types::TypeKind::Builtin(crate::check::types::BuiltinType::String)) => {
            MirType::Scalar(ScalarType::String)
        }
        Some(crate::check::types::TypeKind::Builtin(crate::check::types::BuiltinType::None)) => {
            MirType::Scalar(ScalarType::None)
        }
        _ => MirType::Unknown,
    }
}

#[allow(clippy::too_many_arguments)]
fn lower_reduce_rows(
    module: &mut MirModule,
    block: BlockId,
    table: &str,
    mask: &str,
    field: &str,
    rows: u64,
    result_ty: MirType,
    reduce_effects: EffectSet,
) -> ValueId {
    let rows_value = module.push_value(ValueData::new(MirType::Table {
        item: table.to_string(),
        rows,
    }));
    module.push_operation(
        block,
        make_operation(
            OperationKind::TableRows {
                table: table.to_string(),
                rows,
            },
            Vec::new(),
            vec![rows_value],
            EffectSet::empty(),
            Vec::new(),
        ),
    );

    let mask_value = module.push_value(ValueData::new(MirType::Mask {
        table: table.to_string(),
        rows,
    }));
    module.push_operation(
        block,
        make_operation(
            OperationKind::MaskRead {
                name: mask.to_string(),
                table: table.to_string(),
                rows,
            },
            Vec::new(),
            vec![mask_value],
            EffectSet::empty(),
            Vec::new(),
        ),
    );

    let row = module.push_value(ValueData::new(MirType::RowToken {
        table: table.to_string(),
    }));
    module.push_operation(
        block,
        make_operation(
            OperationKind::RowToken {
                table: table.to_string(),
                rows,
            },
            vec![rows_value],
            vec![row],
            EffectSet::empty(),
            vec![OwnershipMode::Read, OwnershipMode::Read],
        ),
    );

    let acc = module.push_value(ValueData::new(result_ty));
    module.push_operation(
        block,
        make_operation_with_facts(
            OperationKind::ReduceRows {
                table: table.to_string(),
                mask: mask.to_string(),
                field: field.to_string(),
                rows,
            },
            vec![rows_value, mask_value, row],
            vec![acc],
            reduce_effects,
            vec![
                OwnershipMode::Read,
                OwnershipMode::Read,
                OwnershipMode::Read,
            ],
            |facts| facts.set_state_edge(true),
        ),
    );
    acc
}

struct BodyLowerer<'a> {
    check: &'a CheckResult,
    member: &'a MemberSummary,
    view: CstView<'a>,
    module: &'a mut MirModule,
    block: BlockId,
    locals: BTreeMap<String, ValueId>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> BodyLowerer<'a> {
    #[allow(clippy::too_many_arguments)]
    fn new(
        check: &'a CheckResult,
        member: &'a MemberSummary,
        parsed: &'a ParsedSyntax,
        lexed: &'a LexedFile,
        source: &'a SourceFile,
        module: &'a mut MirModule,
        block: BlockId,
        locals: BTreeMap<String, ValueId>,
    ) -> Self {
        Self {
            check,
            member,
            view: CstView::new(parsed.tree(), lexed, source),
            module,
            block,
            locals,
            diagnostics: Vec::new(),
        }
    }

    fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn lower_block(&mut self, body: SyntaxNodeId) {
        for stmt in self.view.child_nodes(body) {
            match self.view.node_kind(stmt) {
                SyntaxKind::LetStmt => self.lower_let(stmt),
                SyntaxKind::ReturnStmt => self.lower_return(stmt),
                SyntaxKind::ExprStmt => {
                    if let Some(expr) = first_expr_child(&self.view, stmt) {
                        self.lower_expr(expr);
                    }
                }
                _ => {}
            }
        }
    }

    fn lower_let(&mut self, stmt: SyntaxNodeId) {
        let Some(name) = self.view.first_identifier_text(stmt) else {
            return;
        };
        let Some(expr) = first_expr_child(&self.view, stmt) else {
            return;
        };
        let effects = infer_mir_effects(&self.view, expr);
        let value = self.lower_expr(expr);
        let binding = name.text().to_string();
        let ty = self.module.value(value).ty().clone();
        let _place = self.module.push_place(PlaceData::new(binding.clone(), ty));
        self.locals.insert(binding.clone(), value);
        self.module.push_operation(
            self.block,
            make_operation(
                OperationKind::Let(binding),
                vec![value],
                Vec::new(),
                effects,
                vec![OwnershipMode::Own],
            ),
        );
    }

    fn lower_return(&mut self, stmt: SyntaxNodeId) {
        if let Some(expr) = first_expr_child(&self.view, stmt) {
            let effects = infer_mir_effects(&self.view, expr);
            let value = self.lower_expr(expr);
            self.module.push_operation(
                self.block,
                make_operation(
                    OperationKind::Return,
                    vec![value],
                    Vec::new(),
                    effects,
                    vec![OwnershipMode::Read],
                ),
            );
        } else {
            self.module.push_operation(
                self.block,
                make_operation(
                    OperationKind::Return,
                    Vec::new(),
                    Vec::new(),
                    EffectSet::empty(),
                    Vec::new(),
                ),
            );
        }
    }

    fn lower_expr(&mut self, expr: SyntaxNodeId) -> ValueId {
        match self.view.node_kind(expr) {
            SyntaxKind::LiteralExpr => self.lower_literal(expr),
            SyntaxKind::NameExpr => self.lower_name(expr),
            SyntaxKind::ParenExpr => first_expr_child(&self.view, expr)
                .map(|child| self.lower_expr(child))
                .unwrap_or_else(|| self.none_value()),
            SyntaxKind::BinaryExpr => self.lower_binary(expr),
            SyntaxKind::ReduceExpr => self.lower_reduce(expr),
            _ => self.unsupported_expr(expr),
        }
    }

    fn lower_reduce(&mut self, expr: SyntaxNodeId) -> ValueId {
        let Some(source) = self
            .view
            .child_nodes(expr)
            .into_iter()
            .find(|child| self.view.node_kind(*child) == SyntaxKind::CallExpr)
        else {
            return self.unsupported_reduce(expr, "missing reduce source expression");
        };
        let Some((table, mask, rows)) =
            parse_rows_call(self.check, self.member, &self.view, source)
        else {
            return self.unsupported_reduce(expr, "reduce source must be `table.rows(mask)`");
        };
        let Some(field) = reduce_field_name(&self.view, expr) else {
            return self
                .unsupported_reduce(expr, "reduce body must return `acc + table.field[row]`");
        };
        let result_ty = reduce_result_type(self.check, &self.view, expr);
        let effects = infer_mir_effects(&self.view, expr);
        lower_reduce_rows(
            self.module,
            self.block,
            &table,
            &mask,
            &field,
            rows,
            result_ty,
            effects,
        )
    }

    fn lower_literal(&mut self, expr: SyntaxNodeId) -> ValueId {
        let text = self
            .view
            .child_tokens(expr)
            .first()
            .map(|token| self.view.token_text(*token).text().to_string())
            .unwrap_or_else(|| "None".to_string());
        let ty = if text == "None" {
            MirType::Scalar(ScalarType::None)
        } else if text == "true" || text == "false" {
            MirType::Scalar(ScalarType::Bool)
        } else if text.starts_with('"') {
            MirType::Scalar(ScalarType::String)
        } else {
            MirType::Scalar(ScalarType::I64)
        };
        let value = self.module.push_value(ValueData::new(ty));
        self.module.push_operation(
            self.block,
            make_operation(
                OperationKind::Literal(text),
                Vec::new(),
                vec![value],
                EffectSet::empty(),
                Vec::new(),
            ),
        );
        value
    }

    fn lower_name(&mut self, expr: SyntaxNodeId) -> ValueId {
        let Some(name) = self.view.first_identifier_text(expr) else {
            return self.none_value();
        };
        match name.text() {
            "None" => return self.lower_builtin_name("None", ScalarType::None),
            "true" | "false" => return self.lower_builtin_name(name.text(), ScalarType::Bool),
            _ => {}
        }
        if let Some(value) = self.locals.get(name.text()).copied() {
            return value;
        }
        let value = self.module.push_value(ValueData::new(MirType::Unknown));
        self.module.push_operation(
            self.block,
            make_operation(
                OperationKind::ReadValue(name.text().to_string()),
                Vec::new(),
                vec![value],
                EffectSet::empty(),
                vec![OwnershipMode::Read],
            ),
        );
        self.locals.insert(name.text().to_string(), value);
        value
    }

    fn lower_builtin_name(&mut self, text: &str, scalar: ScalarType) -> ValueId {
        let value = self
            .module
            .push_value(ValueData::new(MirType::Scalar(scalar)));
        self.module.push_operation(
            self.block,
            make_operation(
                OperationKind::Literal(text.to_string()),
                Vec::new(),
                vec![value],
                EffectSet::empty(),
                Vec::new(),
            ),
        );
        value
    }

    fn lower_binary(&mut self, expr: SyntaxNodeId) -> ValueId {
        let children = expr_children(&self.view, expr);
        if children.len() != 2 {
            return self.none_value();
        }
        let effects = infer_mir_effects(&self.view, expr);
        let left = self.lower_expr(children[0]);
        let right = self.lower_expr(children[1]);
        let Some(operator) = binary_operator(&self.view, expr) else {
            self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::CheckUnsupported,
                    "MIR 01 cannot lower binary expression without an operator",
                )
                .primary(self.view.node_span(expr), "missing binary operator")
                .finish(),
            );
            return self.none_value();
        };
        if operator != "+" {
            self.diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::CheckUnsupported,
                    format!("MIR 01 cannot lower binary operator `{operator}`"),
                )
                .primary(self.view.node_span(expr), "unsupported binary operator")
                .finish(),
            );
            return self.none_value();
        }
        let value = self
            .module
            .push_value(ValueData::new(MirType::Scalar(ScalarType::U64)));
        self.module.push_operation(
            self.block,
            make_operation(
                OperationKind::Binary(operator),
                vec![left, right],
                vec![value],
                effects,
                vec![OwnershipMode::Read, OwnershipMode::Read],
            ),
        );
        value
    }

    fn none_value(&mut self) -> ValueId {
        self.module
            .push_value(ValueData::new(MirType::Scalar(ScalarType::None)))
    }

    fn unsupported_expr(&mut self, expr: SyntaxNodeId) -> ValueId {
        let kind = self.view.node_kind(expr);
        self.diagnostics.push(
            Diagnostic::builder(
                Severity::Error,
                DiagnosticCode::CheckUnsupported,
                format!("MIR 01 cannot lower {kind:?}"),
            )
            .primary(self.view.node_span(expr), "unsupported in MIR 01")
            .finish(),
        );
        self.module.push_value(ValueData::new(MirType::Unknown))
    }

    fn unsupported_reduce(&mut self, expr: SyntaxNodeId, message: &str) -> ValueId {
        self.diagnostics.push(
            Diagnostic::builder(
                Severity::Error,
                DiagnosticCode::CheckUnsupported,
                message.to_string(),
            )
            .primary(self.view.node_span(expr), message)
            .finish(),
        );
        self.module.push_value(ValueData::new(MirType::Unknown))
    }
}

fn parse_rows_call(
    check: &CheckResult,
    member: &MemberSummary,
    view: &CstView<'_>,
    expr: SyntaxNodeId,
) -> Option<(String, String, u64)> {
    let (field_expr, mask_expr) = parse_rows_call_nodes(view, expr)?;
    let table = field_receiver_name(view, field_expr)?.to_string();
    let mask = view.first_identifier_text(mask_expr)?.text().to_string();
    let (rows, _item) = table_param_info(check, member, &table)?;
    Some((table, mask, rows))
}

fn parse_rows_call_nodes(
    view: &CstView<'_>,
    expr: SyntaxNodeId,
) -> Option<(SyntaxNodeId, SyntaxNodeId)> {
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

fn parse_field_name<'a>(view: &'a CstView<'a>, field_expr: SyntaxNodeId) -> Option<&'a str> {
    for token in view.child_tokens(field_expr) {
        if is_identifier_token(view, token) {
            return Some(view.token_text(token).text());
        }
    }
    None
}

fn is_identifier_token(view: &CstView<'_>, token: crate::syntax::SyntaxTokenId) -> bool {
    let syntax_token = view.tree().token(token);
    let raw = view.lexed().tokens()[syntax_token.token().raw() as usize];
    matches!(raw.kind(), TokenKind::Identifier)
}

fn field_receiver_name<'a>(view: &'a CstView<'a>, field_expr: SyntaxNodeId) -> Option<&'a str> {
    let receiver = expr_children(view, field_expr).into_iter().next()?;
    if view.node_kind(receiver) != SyntaxKind::NameExpr {
        return None;
    }
    view.first_identifier_text(receiver)
        .map(|token| token.text())
}

fn table_param_info(
    check: &CheckResult,
    member: &MemberSummary,
    table_name: &str,
) -> Option<(u64, ItemId)> {
    let signature = check.signature_check().member_signature(member)?;
    let param = signature
        .params()
        .iter()
        .find(|param| param.name().text() == table_name)?;
    match check.signature_check().type_table().kind(param.ty()) {
        Some(TypeKind::Table { item, rows }) => Some((rows, item)),
        _ => None,
    }
}

fn reduce_field_name(view: &CstView<'_>, reduce: SyntaxNodeId) -> Option<String> {
    let body = reduce_body(view, reduce)?;
    let return_stmt = view
        .child_nodes(body)
        .into_iter()
        .find(|node| view.node_kind(*node) == SyntaxKind::ReturnStmt)?;
    let return_expr = first_expr_child(view, return_stmt)?;
    let index_expr = find_index_expr(view, return_expr)?;
    parse_index_field_name(view, index_expr)
}

fn find_index_expr(view: &CstView<'_>, expr: SyntaxNodeId) -> Option<SyntaxNodeId> {
    if view.node_kind(expr) == SyntaxKind::IndexExpr {
        return Some(expr);
    }
    for child in expr_children(view, expr) {
        if let Some(found) = find_index_expr(view, child) {
            return Some(found);
        }
    }
    None
}

fn parse_index_field_name(view: &CstView<'_>, index_expr: SyntaxNodeId) -> Option<String> {
    let field_expr = expr_children(view, index_expr).first().copied()?;
    if view.node_kind(field_expr) != SyntaxKind::FieldExpr {
        return None;
    }
    parse_field_name(view, field_expr).map(str::to_string)
}

fn reduce_result_type(_check: &CheckResult, view: &CstView<'_>, expr: SyntaxNodeId) -> MirType {
    for child in view.child_nodes(expr) {
        if view.node_kind(child) == SyntaxKind::TypeRef {
            let name = view.first_identifier_text(child).map(|token| token.text());
            return match name {
                Some("U64") => MirType::Scalar(ScalarType::U64),
                Some("U32") => MirType::Scalar(ScalarType::U32),
                Some("I64") => MirType::Scalar(ScalarType::I64),
                _ => MirType::Unknown,
            };
        }
    }
    MirType::Scalar(ScalarType::U64)
}

fn reduce_body(view: &CstView<'_>, expr: SyntaxNodeId) -> Option<SyntaxNodeId> {
    view.child_nodes(expr)
        .into_iter()
        .find(|child| view.node_kind(*child) == SyntaxKind::Block)
}

fn expr_children(view: &CstView<'_>, node: SyntaxNodeId) -> Vec<SyntaxNodeId> {
    view.child_nodes(node)
        .into_iter()
        .filter(|child| is_expr_kind(view.node_kind(*child)))
        .collect()
}

fn first_expr_child(view: &CstView<'_>, node: SyntaxNodeId) -> Option<SyntaxNodeId> {
    view.child_nodes(node)
        .into_iter()
        .find(|child| is_expr_kind(view.node_kind(*child)))
}

fn binary_operator(view: &CstView<'_>, node: SyntaxNodeId) -> Option<String> {
    for token in view.child_tokens(node) {
        let syntax_token = view.tree().token(token);
        let raw = view.lexed().tokens()[syntax_token.token().raw() as usize];
        if let TokenKind::Punct(Punct::Plus) = raw.kind() {
            return Some("+".to_string());
        }
    }
    None
}

fn is_expr_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::LiteralExpr
            | SyntaxKind::NameExpr
            | SyntaxKind::ParenExpr
            | SyntaxKind::BinaryExpr
            | SyntaxKind::CallExpr
            | SyntaxKind::FieldExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::ReduceExpr
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::check_root;
    use crate::syntax::parse_file;
    use std::path::PathBuf;

    #[test]
    fn filter_sum_table_param_info_resolves_packets() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/mir/dataplane/filter_sum.wrela");
        let check = check_root(&path);
        assert!(check.ok(), "{:?}", check.diagnostics());
        let summary = &check.summaries()[0];
        let item = summary
            .items()
            .iter()
            .find(|item| item.name().text() == "FilterSumBench")
            .expect("bench class");
        let member = item
            .members()
            .iter()
            .find(|member| member.name().is_some_and(|name| name.text() == "run"))
            .expect("run member");
        let (rows, _) = table_param_info(&check, member, "packets").expect("packets table");
        assert_eq!(rows, 256);

        let file_id = summary.file_id();
        let parsed = check
            .parsed()
            .iter()
            .find(|parsed| parsed.file_id() == file_id)
            .expect("parsed file");
        let lexed = check
            .lexed_files()
            .iter()
            .find(|lexed| lexed.file_id() == file_id)
            .expect("lexed file");
        let source = check.source_map().get(file_id).expect("source file");
        assert!(
            parsed.diagnostics().is_empty(),
            "parse diagnostics: {:?}",
            parsed.diagnostics()
        );
        let reparsed = parse_file(lexed, source);
        assert!(reparsed.diagnostics().is_empty());
        let view = CstView::new(reparsed.tree(), lexed, source);
        fn find_first_reduce(view: &CstView<'_>, node: SyntaxNodeId) -> Option<SyntaxNodeId> {
            if view.node_kind(node) == SyntaxKind::ReduceExpr {
                return Some(node);
            }
            for child in view.child_nodes(node) {
                if let Some(found) = find_first_reduce(view, child) {
                    return Some(found);
                }
            }
            None
        }

        let body = member.body().expect("run body");
        let reduce = find_first_reduce(&view, body).expect("reduce expr");
        assert_eq!(view.node_kind(reduce), SyntaxKind::ReduceExpr);
        let source_expr = view
            .child_nodes(reduce)
            .into_iter()
            .find(|child| view.node_kind(*child) == SyntaxKind::CallExpr)
            .expect("reduce source");
        assert_eq!(view.node_kind(source_expr), SyntaxKind::CallExpr);
        let call_children: Vec<_> = expr_children(&view, source_expr)
            .into_iter()
            .map(|child| view.node_kind(child))
            .collect();
        assert_eq!(
            call_children,
            vec![SyntaxKind::FieldExpr],
            "unexpected call expression children"
        );
        assert!(
            parse_rows_call_nodes(&view, source_expr).is_some(),
            "expected parse_rows_call_nodes to succeed"
        );
        assert!(
            parse_rows_call(&check, member, &view, source_expr).is_some(),
            "expected parse_rows_call to succeed"
        );
    }

    #[test]
    fn build_mir_wires_ownership_effects_places_and_round_trips() {
        let data_flow =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/mir/data_flow.wrela");
        let check = check_root(&data_flow);
        assert!(check.ok());
        let built = build_mir(&check);
        assert!(built.ok(), "{:?}", built.diagnostics());
        let module = built.module().expect("module");
        assert!(!module.places().is_empty());
        let read_left = module
            .operations()
            .iter()
            .find(|op| matches!(op.kind(), OperationKind::ReadValue(name) if name == "left"));
        let read_left = read_left.expect("left read");
        assert_eq!(read_left.facts().ownership(), &[OwnershipMode::Read]);
        assert!(read_left.effects().is_empty());

        let filter_sum = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/mir/dataplane/filter_sum.wrela");
        let check = check_root(&filter_sum);
        let built = build_mir(&check);
        let module = built.module().expect("filter_sum module");
        let reduce = module
            .operations()
            .iter()
            .find(|op| matches!(op.kind(), OperationKind::ReduceRows { .. }))
            .expect("reduce_rows");
        assert!(reduce.effects().contains(crate::mir::Effect::Mutate));
        assert!(reduce.facts().has_state_edge());

        let text = crate::mir::text::render_module(module);
        let parsed = crate::mir::parse_text::parse_module(&text).expect("parse");
        assert!(crate::mir::module_eq::modules_semantically_equal(
            module, &parsed
        ));
    }

    #[test]
    fn filter_sum_build_mir_contains_dataplane_ops() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/mir/dataplane/filter_sum.wrela");
        let check = check_root(&path);
        assert!(check.ok(), "{:?}", check.diagnostics());
        let mir = build_mir(&check);
        assert!(mir.ok(), "{:?}", mir.diagnostics());
        let text = crate::mir::text::render_module(mir.module().unwrap());
        assert!(
            text.contains("table_rows packets rows=256"),
            "missing table_rows in MIR text:\n{text}"
        );
    }
}
