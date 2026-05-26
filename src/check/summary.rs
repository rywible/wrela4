use std::path::Path;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::lexer::LexedFile;
use crate::source::{FileId, SourceFile, Span};
use crate::syntax::{ParsedSyntax, SyntaxKind, SyntaxNodeId};

use super::cst::{CstView, TokenText};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Name {
    text: String,
    span: Span,
}

impl Name {
    pub fn new(text: impl Into<String>, span: Span) -> Self {
        Self {
            text: text.into(),
            span,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModulePathSummary {
    segments: Vec<Name>,
    span: Span,
}

impl ModulePathSummary {
    pub fn from_names(segments: Vec<Name>) -> Self {
        let span = span_covering_names(&segments);
        Self { segments, span }
    }

    pub fn segments(&self) -> &[Name] {
        &self.segments
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn as_dotted(&self) -> String {
        self.segments
            .iter()
            .map(Name::text)
            .collect::<Vec<_>>()
            .join(".")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccessMode {
    Read,
    Mut,
    Own,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeArgSummary {
    Type(TypeRefSummary),
    Int { value: u64, span: Span },
    ValueName(Name),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeRefSummary {
    node_id: SyntaxNodeId,
    access: Option<AccessMode>,
    unique: bool,
    path: Vec<Name>,
    args: Vec<TypeArgSummary>,
    span: Span,
}

impl TypeRefSummary {
    pub fn new(
        node_id: SyntaxNodeId,
        access: Option<AccessMode>,
        unique: bool,
        path: Vec<Name>,
        args: Vec<TypeArgSummary>,
        span: Span,
    ) -> Self {
        Self {
            node_id,
            access,
            unique,
            path,
            args,
            span,
        }
    }

    pub fn node_id(&self) -> SyntaxNodeId {
        self.node_id
    }

    pub fn access(&self) -> Option<AccessMode> {
        self.access
    }

    pub fn unique(&self) -> bool {
        self.unique
    }

    pub fn path(&self) -> &[Name] {
        &self.path
    }

    pub fn args(&self) -> &[TypeArgSummary] {
        &self.args
    }

    pub fn span(&self) -> Span {
        self.span
    }

    pub fn path_text(&self) -> String {
        self.path
            .iter()
            .map(Name::text)
            .collect::<Vec<_>>()
            .join(".")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParamSummary {
    node_id: SyntaxNodeId,
    name: Name,
    access: AccessMode,
    ty: TypeRefSummary,
    span: Span,
}

impl ParamSummary {
    pub fn new(
        node_id: SyntaxNodeId,
        name: Name,
        access: AccessMode,
        ty: TypeRefSummary,
        span: Span,
    ) -> Self {
        Self {
            node_id,
            name,
            access,
            ty,
            span,
        }
    }

    pub fn node_id(&self) -> SyntaxNodeId {
        self.node_id
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn access(&self) -> AccessMode {
        self.access
    }

    pub fn ty(&self) -> &TypeRefSummary {
        &self.ty
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SignatureSummary {
    node_id: SyntaxNodeId,
    params: Vec<ParamSummary>,
    return_type: Option<TypeRefSummary>,
    span: Span,
}

impl SignatureSummary {
    pub fn new(
        node_id: SyntaxNodeId,
        params: Vec<ParamSummary>,
        return_type: Option<TypeRefSummary>,
        span: Span,
    ) -> Self {
        Self {
            node_id,
            params,
            return_type,
            span,
        }
    }

    pub fn node_id(&self) -> SyntaxNodeId {
        self.node_id
    }

    pub fn params(&self) -> &[ParamSummary] {
        &self.params
    }

    pub fn return_type(&self) -> Option<&TypeRefSummary> {
        self.return_type.as_ref()
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberKind {
    Field,
    Method,
    Constructor,
    Test,
    Phase,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemberSummary {
    node_id: SyntaxNodeId,
    name: Option<Name>,
    kind: MemberKind,
    signature: Option<SignatureSummary>,
    field_type: Option<TypeRefSummary>,
    body: Option<SyntaxNodeId>,
    span: Span,
}

impl MemberSummary {
    pub fn new_field(
        node_id: SyntaxNodeId,
        name: Name,
        field_type: TypeRefSummary,
        span: Span,
    ) -> Self {
        Self {
            node_id,
            name: Some(name),
            kind: MemberKind::Field,
            signature: None,
            field_type: Some(field_type),
            body: None,
            span,
        }
    }

    pub fn new_callable(
        node_id: SyntaxNodeId,
        name: Option<Name>,
        kind: MemberKind,
        signature: SignatureSummary,
        body: Option<SyntaxNodeId>,
        span: Span,
    ) -> Self {
        Self {
            node_id,
            name,
            kind,
            signature: Some(signature),
            field_type: None,
            body,
            span,
        }
    }

    pub fn node_id(&self) -> SyntaxNodeId {
        self.node_id
    }

    pub fn name(&self) -> Option<&Name> {
        self.name.as_ref()
    }

    pub fn kind(&self) -> MemberKind {
        self.kind
    }

    pub fn signature(&self) -> Option<&SignatureSummary> {
        self.signature.as_ref()
    }

    pub fn field_type(&self) -> Option<&TypeRefSummary> {
        self.field_type.as_ref()
    }

    pub fn body(&self) -> Option<SyntaxNodeId> {
        self.body
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ItemKind {
    Data,
    LayoutData,
    Class,
    UniqueClass,
    Interface,
    Error,
    Image,
    HostImage,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ItemSummary {
    node_id: SyntaxNodeId,
    name: Name,
    kind: ItemKind,
    public: bool,
    generic_params: Vec<Name>,
    implements: Vec<TypeRefSummary>,
    members: Vec<MemberSummary>,
    span: Span,
}

impl ItemSummary {
    pub fn new(
        node_id: SyntaxNodeId,
        name: Name,
        kind: ItemKind,
        public: bool,
        span: Span,
    ) -> Self {
        Self {
            node_id,
            name,
            kind,
            public,
            generic_params: Vec::new(),
            implements: Vec::new(),
            members: Vec::new(),
            span,
        }
    }

    pub fn with_members(mut self, members: Vec<MemberSummary>) -> Self {
        self.members = members;
        self
    }

    pub fn with_generic_params(mut self, generic_params: Vec<Name>) -> Self {
        self.generic_params = generic_params;
        self
    }

    pub fn with_implements(mut self, implements: Vec<TypeRefSummary>) -> Self {
        self.implements = implements;
        self
    }

    pub fn node_id(&self) -> SyntaxNodeId {
        self.node_id
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn kind(&self) -> ItemKind {
        self.kind
    }

    pub fn is_public(&self) -> bool {
        self.public
    }

    pub fn generic_params(&self) -> &[Name] {
        &self.generic_params
    }

    pub fn implements(&self) -> &[TypeRefSummary] {
        &self.implements
    }

    pub fn members(&self) -> &[MemberSummary] {
        &self.members
    }

    pub fn fields(&self) -> impl Iterator<Item = &MemberSummary> {
        self.members
            .iter()
            .filter(|member| member.kind() == MemberKind::Field)
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckImportSummary {
    node_id: SyntaxNodeId,
    module_path: ModulePathSummary,
    binders: Vec<Name>,
    span: Span,
}

impl CheckImportSummary {
    pub fn new(
        node_id: SyntaxNodeId,
        module_path: ModulePathSummary,
        binders: Vec<Name>,
        span: Span,
    ) -> Self {
        Self {
            node_id,
            module_path,
            binders,
            span,
        }
    }

    pub fn node_id(&self) -> SyntaxNodeId {
        self.node_id
    }

    pub fn module_path(&self) -> &ModulePathSummary {
        &self.module_path
    }

    pub fn binders(&self) -> &[Name] {
        &self.binders
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckModuleSummary {
    file_id: FileId,
    module_path: ModulePathSummary,
    imports: Vec<CheckImportSummary>,
    items: Vec<ItemSummary>,
    structurally_valid: bool,
}

impl CheckModuleSummary {
    pub fn new(
        file_id: FileId,
        module_path: ModulePathSummary,
        imports: Vec<CheckImportSummary>,
        items: Vec<ItemSummary>,
        structurally_valid: bool,
    ) -> Self {
        Self {
            file_id,
            module_path,
            imports,
            items,
            structurally_valid,
        }
    }

    pub fn file_id(&self) -> FileId {
        self.file_id
    }

    pub fn module_path(&self) -> &ModulePathSummary {
        &self.module_path
    }

    pub fn imports(&self) -> &[CheckImportSummary] {
        &self.imports
    }

    pub fn items(&self) -> &[ItemSummary] {
        &self.items
    }

    pub fn structurally_valid(&self) -> bool {
        self.structurally_valid
    }
}

fn span_covering_names(names: &[Name]) -> Span {
    let first = names
        .first()
        .expect("module paths require at least one segment");
    let last = names
        .last()
        .expect("module paths require at least one segment");
    Span::new(
        first.span().file_id(),
        first.span().start(),
        last.span().end(),
    )
}

pub fn summarize_checked_module(
    parsed: &ParsedSyntax,
    lexed: &LexedFile,
    source: &SourceFile,
    source_root: &Path,
) -> (CheckModuleSummary, Vec<Diagnostic>) {
    let view = CstView::new(parsed.tree(), lexed, source);
    let root = parsed.tree().root();
    let mut diagnostics = Vec::new();

    let module_path = find_module_decl(&view, root)
        .and_then(|node| module_path_from_node(&view, node))
        .unwrap_or_else(|| module_path_from_file(source, source_root));

    let mut imports = Vec::new();
    let mut items = Vec::new();
    let mut structurally_valid = true;

    for child in view.child_nodes(root) {
        match view.node_kind(child) {
            SyntaxKind::UseDecl => {
                if let Some(import) = summarize_import(&view, child) {
                    imports.push(import);
                } else {
                    structurally_valid = false;
                    diagnostics.push(invalid_summary(child, &view, "could not summarize import"));
                }
            }
            SyntaxKind::PublicItem => match summarize_public_item(&view, child) {
                Some(item) => items.push(item),
                None => {
                    structurally_valid = false;
                    diagnostics.push(invalid_summary(
                        child,
                        &view,
                        "could not summarize public item",
                    ));
                }
            },
            SyntaxKind::DataDecl
            | SyntaxKind::LayoutDataDecl
            | SyntaxKind::ClassDecl
            | SyntaxKind::UniqueClassDecl
            | SyntaxKind::InterfaceDecl
            | SyntaxKind::ErrorDecl
            | SyntaxKind::ImageDecl
            | SyntaxKind::HostImageDecl => match summarize_item(&view, child, false) {
                Some(item) => items.push(item),
                None => {
                    structurally_valid = false;
                    diagnostics.push(invalid_summary(child, &view, "could not summarize item"));
                }
            },
            SyntaxKind::RecoveryNode => {
                structurally_valid = false;
                diagnostics.push(invalid_summary(
                    child,
                    &view,
                    "parser recovery reached top level",
                ));
            }
            _ => {}
        }
    }

    if contains_recovery(&view, root) {
        structurally_valid = false;
    }

    (
        CheckModuleSummary::new(
            parsed.file_id(),
            module_path,
            imports,
            items,
            structurally_valid,
        ),
        diagnostics,
    )
}

fn find_module_decl(view: &CstView<'_>, root: SyntaxNodeId) -> Option<SyntaxNodeId> {
    view.child_nodes(root)
        .into_iter()
        .find(|node| view.node_kind(*node) == SyntaxKind::ModuleDecl)
}

fn module_path_from_node(view: &CstView<'_>, node: SyntaxNodeId) -> Option<ModulePathSummary> {
    let path_node = view.first_child(node, SyntaxKind::ModulePath)?;
    module_path_from_path_node(view, path_node)
}

fn module_path_from_path_node(
    view: &CstView<'_>,
    path_node: SyntaxNodeId,
) -> Option<ModulePathSummary> {
    let segments = view
        .identifiers_in_node(path_node)
        .into_iter()
        .map(name_from_token)
        .collect::<Vec<_>>();
    if segments.is_empty() {
        None
    } else {
        Some(ModulePathSummary::from_names(segments))
    }
}

fn module_path_from_file(source: &SourceFile, source_root: &Path) -> ModulePathSummary {
    let relative = source
        .path()
        .strip_prefix(source_root)
        .unwrap_or(source.path());
    let mut names = Vec::new();
    for component in relative.with_extension("").components() {
        let text = component.as_os_str().to_string_lossy();
        if !text.is_empty() {
            names.push(Name::new(text, source.span()));
        }
    }
    if names.is_empty() {
        names.push(Name::new("root", source.span()));
    }
    ModulePathSummary::from_names(names)
}

fn summarize_public_item(view: &CstView<'_>, node: SyntaxNodeId) -> Option<ItemSummary> {
    view.child_nodes(node)
        .into_iter()
        .find_map(|child| summarize_item(view, child, true))
}

fn summarize_item(view: &CstView<'_>, node: SyntaxNodeId, public: bool) -> Option<ItemSummary> {
    let kind = item_kind(view.node_kind(node))?;
    let name = item_name(view, node, kind)?;
    let mut item = ItemSummary::new(node, name, kind, public, view.node_span(node));
    let members = summarize_members(view, node, kind);
    item = item.with_members(members);
    let implements = view
        .first_child(node, SyntaxKind::ImplementsClause)
        .map(|implements| {
            view.child_nodes(implements)
                .into_iter()
                .filter(|child| view.node_kind(*child) == SyntaxKind::TypeRef)
                .filter_map(|child| summarize_type_ref(view, child))
                .collect()
        })
        .unwrap_or_default();
    Some(item.with_implements(implements))
}

fn item_kind(kind: SyntaxKind) -> Option<ItemKind> {
    match kind {
        SyntaxKind::DataDecl => Some(ItemKind::Data),
        SyntaxKind::LayoutDataDecl => Some(ItemKind::LayoutData),
        SyntaxKind::ClassDecl => Some(ItemKind::Class),
        SyntaxKind::UniqueClassDecl => Some(ItemKind::UniqueClass),
        SyntaxKind::InterfaceDecl => Some(ItemKind::Interface),
        SyntaxKind::ErrorDecl => Some(ItemKind::Error),
        SyntaxKind::ImageDecl => Some(ItemKind::Image),
        SyntaxKind::HostImageDecl => Some(ItemKind::HostImage),
        _ => None,
    }
}

fn item_name(view: &CstView<'_>, node: SyntaxNodeId, kind: ItemKind) -> Option<Name> {
    let identifiers = view.identifiers_in_node(node);
    let index = if kind == ItemKind::LayoutData { 1 } else { 0 };
    identifiers.get(index).copied().map(name_from_token)
}

fn summarize_members(
    view: &CstView<'_>,
    item: SyntaxNodeId,
    _kind: ItemKind,
) -> Vec<MemberSummary> {
    view.child_nodes(item)
        .into_iter()
        .filter_map(|child| match view.node_kind(child) {
            SyntaxKind::FieldDecl => summarize_field(view, child),
            SyntaxKind::MethodDecl => summarize_callable(view, child, MemberKind::Method),
            SyntaxKind::ConstructorDecl => summarize_callable(view, child, MemberKind::Constructor),
            SyntaxKind::TestDecl => summarize_callable(view, child, MemberKind::Test),
            SyntaxKind::PhaseDecl => summarize_callable(view, child, MemberKind::Phase),
            _ => None,
        })
        .collect()
}

fn summarize_field(view: &CstView<'_>, node: SyntaxNodeId) -> Option<MemberSummary> {
    let name = view.first_identifier_text(node).map(name_from_token)?;
    let ty = view
        .first_child(node, SyntaxKind::TypeRef)
        .and_then(|type_node| summarize_type_ref(view, type_node))?;
    Some(MemberSummary::new_field(
        node,
        name,
        ty,
        view.node_span(node),
    ))
}

fn summarize_callable(
    view: &CstView<'_>,
    node: SyntaxNodeId,
    kind: MemberKind,
) -> Option<MemberSummary> {
    let name = match kind {
        MemberKind::Constructor => None,
        _ => view.first_identifier_text(node).map(name_from_token),
    };
    let signature = summarize_signature(view, node)?;
    let body = view.first_child(node, SyntaxKind::Block);
    Some(MemberSummary::new_callable(
        node,
        name,
        kind,
        signature,
        body,
        view.node_span(node),
    ))
}

fn summarize_signature(view: &CstView<'_>, node: SyntaxNodeId) -> Option<SignatureSummary> {
    let param_list = view.first_child(node, SyntaxKind::ParamList);
    let params = param_list
        .map(|list| {
            view.child_nodes(list)
                .into_iter()
                .filter(|child| view.node_kind(*child) == SyntaxKind::Param)
                .filter_map(|param| summarize_param(view, param))
                .collect()
        })
        .unwrap_or_default();
    let return_type = view
        .first_child(node, SyntaxKind::ReturnType)
        .and_then(|return_node| view.first_child(return_node, SyntaxKind::TypeRef))
        .and_then(|type_node| summarize_type_ref(view, type_node));
    Some(SignatureSummary::new(
        node,
        params,
        return_type,
        view.node_span(node),
    ))
}

fn summarize_param(view: &CstView<'_>, node: SyntaxNodeId) -> Option<ParamSummary> {
    let name = view.first_identifier_text(node).map(name_from_token)?;
    let access = access_in_node(view, node).unwrap_or(AccessMode::Read);
    let ty = view
        .first_child(node, SyntaxKind::TypeRef)
        .and_then(|type_node| summarize_type_ref(view, type_node))?;
    Some(ParamSummary::new(
        node,
        name,
        access,
        ty,
        view.node_span(node),
    ))
}

fn summarize_type_ref(view: &CstView<'_>, node: SyntaxNodeId) -> Option<TypeRefSummary> {
    let generic_args = view
        .first_child(node, SyntaxKind::GenericArgList)
        .map(|arg_list| summarize_generic_args(view, arg_list))
        .unwrap_or_default();

    let path = summarize_direct_type_path(view, node);
    if path.is_empty() {
        return None;
    }

    Some(TypeRefSummary::new(
        node,
        summarize_access_mode(view, node),
        summarize_unique_marker(view, node),
        path,
        generic_args,
        view.node_span(node),
    ))
}

fn summarize_direct_type_path(view: &CstView<'_>, type_ref: SyntaxNodeId) -> Vec<Name> {
    let mut names = Vec::new();
    for token in view.child_tokens(type_ref) {
        let text = view.token_text(token).text();
        if text == "[" {
            break;
        }
        if matches!(text, "read" | "mut" | "own" | "unique" | ".") {
            continue;
        }
        if is_identifier_text(text) {
            names.push(name_from_token(view.token_text(token)));
        }
    }
    names
}

fn summarize_generic_args(view: &CstView<'_>, arg_list: SyntaxNodeId) -> Vec<TypeArgSummary> {
    let mut args = Vec::new();
    for child in view.child_nodes(arg_list) {
        if view.node_kind(child) == SyntaxKind::TypeRef {
            if let Some(ty) = summarize_type_ref(view, child) {
                args.push(TypeArgSummary::Type(ty));
            }
        }
    }
    for token in view.child_tokens(arg_list) {
        let text = view.token_text(token);
        if let Ok(value) = text.text().parse::<u64>() {
            args.push(TypeArgSummary::Int {
                value,
                span: text.span(),
            });
        } else if is_identifier_text(text.text()) {
            args.push(TypeArgSummary::ValueName(name_from_token(text)));
        }
    }
    args
}

fn summarize_access_mode(view: &CstView<'_>, node: SyntaxNodeId) -> Option<AccessMode> {
    access_in_node(view, node)
}

fn summarize_unique_marker(view: &CstView<'_>, node: SyntaxNodeId) -> bool {
    unique_in_node(view, node)
}

fn is_identifier_text(text: &str) -> bool {
    let Some(first) = text.chars().next() else {
        return false;
    };
    (first.is_ascii_alphabetic() || first == '_')
        && text
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn access_in_node(view: &CstView<'_>, node: SyntaxNodeId) -> Option<AccessMode> {
    for token in view.child_tokens(node) {
        let text = view.token_text(token).text();
        match text {
            "read" => return Some(AccessMode::Read),
            "mut" => return Some(AccessMode::Mut),
            "own" => return Some(AccessMode::Own),
            _ => {}
        }
    }
    None
}

fn unique_in_node(view: &CstView<'_>, node: SyntaxNodeId) -> bool {
    view.child_tokens(node)
        .into_iter()
        .any(|token| view.token_text(token).text() == "unique")
}

fn summarize_import(view: &CstView<'_>, node: SyntaxNodeId) -> Option<CheckImportSummary> {
    let module_path = view
        .first_child(node, SyntaxKind::ModulePath)
        .and_then(|path_node| module_path_from_path_node(view, path_node))?;
    let binders = view
        .first_child(node, SyntaxKind::UseBinderList)
        .map(|list| {
            view.child_nodes(list)
                .into_iter()
                .filter(|child| view.node_kind(*child) == SyntaxKind::UseBinder)
                .filter_map(|binder| view.first_identifier_text(binder).map(name_from_token))
                .collect()
        })
        .unwrap_or_default();
    Some(CheckImportSummary::new(
        node,
        module_path,
        binders,
        view.node_span(node),
    ))
}

fn name_from_token(token: TokenText<'_>) -> Name {
    Name::new(token.text(), token.span())
}

fn invalid_summary(node: SyntaxNodeId, view: &CstView<'_>, message: &'static str) -> Diagnostic {
    Diagnostic::builder(
        Severity::Error,
        DiagnosticCode::SummaryInvalidModule,
        message,
    )
    .primary(view.node_span(node), message)
    .finish()
}

fn contains_recovery(view: &CstView<'_>, node: SyntaxNodeId) -> bool {
    view.node_kind(node) == SyntaxKind::RecoveryNode
        || view
            .child_nodes(node)
            .into_iter()
            .any(|child| contains_recovery(view, child))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::{FileId, Span};
    use crate::syntax::SyntaxNodeId;

    #[test]
    fn module_summary_exposes_imports_items_and_structural_validity() {
        let name = Name::new("Console", Span::new(FileId::new(0), 4, 11));
        let item = ItemSummary::new(
            SyntaxNodeId::new(9),
            name.clone(),
            ItemKind::UniqueClass,
            true,
            Span::new(FileId::new(0), 0, 20),
        );
        let module = CheckModuleSummary::new(
            FileId::new(0),
            ModulePathSummary::from_names(vec![Name::new(
                "root",
                Span::new(FileId::new(0), 7, 11),
            )]),
            Vec::new(),
            vec![item],
            true,
        );

        assert!(module.structurally_valid());
        assert_eq!(module.module_path().as_dotted(), "root");
        assert_eq!(module.items()[0].name().text(), "Console");
        assert_eq!(module.items()[0].kind(), ItemKind::UniqueClass);
        assert!(module.items()[0].is_public());
    }
}

#[cfg(test)]
mod extraction_tests {
    use crate::check::cst::CstView;
    use crate::lexer::lex_file;
    use crate::source::{FileId, SourceFile};
    use crate::syntax::{SyntaxKind, parse_file};
    use std::path::PathBuf;

    #[test]
    fn parser_shape_matches_summary_extractor_expectations() {
        let source = SourceFile::new(
            FileId::new(0),
            PathBuf::from("root.wrela"),
            "module root\nuse { Console } from io\npub data Bytes { value: U32 }\n".to_string(),
        );
        let lexed = lex_file(&source);
        let parsed = parse_file(&lexed, &source);
        let view = CstView::new(parsed.tree(), &lexed, &source);

        let root = parsed.tree().root();
        let module_decl = view.first_child(root, SyntaxKind::ModuleDecl).unwrap();
        let use_decl = view.first_child(root, SyntaxKind::UseDecl).unwrap();
        let public_item = view.first_child(root, SyntaxKind::PublicItem).unwrap();
        let data_decl = view.first_child(public_item, SyntaxKind::DataDecl).unwrap();

        assert!(
            view.first_child(module_decl, SyntaxKind::ModulePath)
                .is_some()
        );
        assert!(
            view.first_child(use_decl, SyntaxKind::UseBinderList)
                .is_some()
        );
        assert!(view.first_child(use_decl, SyntaxKind::ModulePath).is_some());
        assert!(view.first_child(data_decl, SyntaxKind::FieldDecl).is_some());
    }
}
