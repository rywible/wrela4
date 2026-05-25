use std::collections::BTreeMap;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::source::{FileId, Span};

use super::summary::{CheckImportSummary, CheckModuleSummary, ItemKind, Name};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ModuleId(u32);

impl ModuleId {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ItemId {
    module: ModuleId,
    index: u32,
}

impl ItemId {
    pub const fn new(module: ModuleId, index: u32) -> Self {
        Self { module, index }
    }

    pub const fn module(self) -> ModuleId {
        self.module
    }

    pub const fn index(self) -> u32 {
        self.index
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SymbolKind {
    Data,
    LayoutData,
    Interface,
    Class,
    UniqueClass,
    Error,
    Image,
    HostImage,
    BuiltinType,
}

impl SymbolKind {
    pub const fn is_type(self) -> bool {
        matches!(
            self,
            SymbolKind::Data
                | SymbolKind::LayoutData
                | SymbolKind::Interface
                | SymbolKind::Class
                | SymbolKind::UniqueClass
                | SymbolKind::Error
                | SymbolKind::BuiltinType
        )
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedItem {
    id: ItemId,
    name: Name,
    kind: SymbolKind,
    public: bool,
    span: Span,
}

impl ResolvedItem {
    pub fn id(&self) -> ItemId {
        self.id
    }

    pub fn name(&self) -> &Name {
        &self.name
    }

    pub fn kind(&self) -> SymbolKind {
        self.kind
    }

    pub fn is_public(&self) -> bool {
        self.public
    }

    pub fn span(&self) -> Span {
        self.span
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedModule {
    id: ModuleId,
    file_id: FileId,
    path: String,
    items: Vec<ResolvedItem>,
    local_symbols: BTreeMap<String, ItemId>,
    imported_symbols: BTreeMap<String, ItemId>,
}

impl ResolvedModule {
    pub fn id(&self) -> ModuleId {
        self.id
    }

    pub fn file_id(&self) -> FileId {
        self.file_id
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn items(&self) -> &[ResolvedItem] {
        &self.items
    }

    pub fn resolve_local(&self, name: &str) -> Option<ItemId> {
        self.local_symbols.get(name).copied()
    }

    pub fn resolve_imported(&self, name: &str) -> Option<ItemId> {
        self.imported_symbols.get(name).copied()
    }

    pub fn resolve_any(&self, name: &str) -> Option<ItemId> {
        self.resolve_local(name)
            .or_else(|| self.resolve_imported(name))
    }

    pub fn visible_names(&self) -> Vec<&str> {
        let mut names = self
            .local_symbols
            .keys()
            .map(String::as_str)
            .chain(self.imported_symbols.keys().map(String::as_str))
            .collect::<Vec<_>>();
        names.sort();
        names.dedup();
        names
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedGraph {
    modules: Vec<ResolvedModule>,
    path_to_module: BTreeMap<String, ModuleId>,
    diagnostics: Vec<Diagnostic>,
}

impl ResolvedGraph {
    pub fn modules(&self) -> &[ResolvedModule] {
        &self.modules
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn module_by_path(&self, path: &str) -> Option<&ResolvedModule> {
        let module_id = self.path_to_module.get(path)?;
        self.modules.get(module_id.raw() as usize)
    }

    pub fn module_for_summary(&self, summary: &CheckModuleSummary) -> Option<&ResolvedModule> {
        self.module_by_path(&summary.module_path().as_dotted())
    }

    pub fn item(&self, id: ItemId) -> Option<&ResolvedItem> {
        self.modules
            .get(id.module().raw() as usize)
            .and_then(|module| module.items().get(id.index() as usize))
    }

    pub fn resolve_visible_type(
        &self,
        module: &ResolvedModule,
        name: &str,
    ) -> Option<(&ResolvedModule, &ResolvedItem)> {
        let item_id = module.resolve_any(name)?;
        let item = self.item(item_id)?;
        if item.kind().is_type() {
            let owner = self.modules().get(item_id.module().raw() as usize)?;
            Some((owner, item))
        } else {
            None
        }
    }

    pub fn resolve_visible_any(
        &self,
        module: &ResolvedModule,
        name: &str,
    ) -> Option<(&ResolvedModule, &ResolvedItem)> {
        let item_id = module.resolve_any(name)?;
        let item = self.item(item_id)?;
        let owner = self.modules().get(item_id.module().raw() as usize)?;
        Some((owner, item))
    }
}

pub fn resolve_modules(summaries: &[CheckModuleSummary]) -> ResolvedGraph {
    let valid_summaries = summaries
        .iter()
        .filter(|summary| summary.structurally_valid())
        .collect::<Vec<_>>();
    let mut diagnostics = Vec::new();
    let mut path_to_module = BTreeMap::<String, ModuleId>::new();
    let mut modules = Vec::new();

    for summary in valid_summaries.iter() {
        let module_id = ModuleId::new(modules.len() as u32);
        let path = summary.module_path().as_dotted();
        if let Some(first) = path_to_module.get(&path).copied() {
            diagnostics.push(duplicate_module(summary, first));
            continue;
        }
        path_to_module.insert(path.clone(), module_id);
        modules.push(index_module(module_id, summary, &mut diagnostics));
    }

    validate_imports(
        &mut modules,
        &path_to_module,
        &valid_summaries,
        &mut diagnostics,
    );

    ResolvedGraph {
        modules,
        path_to_module,
        diagnostics,
    }
}

fn index_module(
    module_id: ModuleId,
    summary: &CheckModuleSummary,
    diagnostics: &mut Vec<Diagnostic>,
) -> ResolvedModule {
    let mut local_symbols: BTreeMap<String, ItemId> = BTreeMap::new();
    let mut items: Vec<ResolvedItem> = Vec::new();
    for item in summary.items() {
        let item_id = ItemId::new(module_id, items.len() as u32);
        if let Some(first_id) = local_symbols.get(item.name().text()).copied() {
            let first_span = items[first_id.index() as usize].span();
            diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::ResolveDuplicate,
                    format!("duplicate item `{}`", item.name().text()),
                )
                .primary(item.name().span(), "duplicate declaration")
                .secondary(first_span, "first declaration is here")
                .finish(),
            );
        } else {
            local_symbols.insert(item.name().text().to_string(), item_id);
        }
        items.push(ResolvedItem {
            id: item_id,
            name: item.name().clone(),
            kind: symbol_kind(item.kind()),
            public: item.is_public(),
            span: item.span(),
        });
    }
    ResolvedModule {
        id: module_id,
        file_id: summary.file_id(),
        path: summary.module_path().as_dotted(),
        items,
        local_symbols,
        imported_symbols: BTreeMap::new(),
    }
}

fn symbol_kind(kind: ItemKind) -> SymbolKind {
    match kind {
        ItemKind::Data => SymbolKind::Data,
        ItemKind::LayoutData => SymbolKind::LayoutData,
        ItemKind::Class => SymbolKind::Class,
        ItemKind::UniqueClass => SymbolKind::UniqueClass,
        ItemKind::Interface => SymbolKind::Interface,
        ItemKind::Error => SymbolKind::Error,
        ItemKind::Image => SymbolKind::Image,
        ItemKind::HostImage => SymbolKind::HostImage,
    }
}

fn validate_imports(
    modules: &mut [ResolvedModule],
    path_to_module: &BTreeMap<String, ModuleId>,
    summaries: &[&CheckModuleSummary],
    diagnostics: &mut Vec<Diagnostic>,
) {
    for summary in summaries {
        let Some(module_index) = path_to_module
            .get(&summary.module_path().as_dotted())
            .map(|id| id.raw() as usize)
        else {
            continue;
        };
        for import in summary.imports() {
            validate_one_import(module_index, import, modules, path_to_module, diagnostics);
        }
    }
}

fn validate_one_import(
    importing_index: usize,
    import: &CheckImportSummary,
    modules: &mut [ResolvedModule],
    path_to_module: &BTreeMap<String, ModuleId>,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let imported_path = import.module_path().as_dotted();
    let Some(imported_id) = path_to_module.get(&imported_path).copied() else {
        diagnostics.push(
            Diagnostic::builder(
                Severity::Error,
                DiagnosticCode::ResolveImport,
                format!("module `{imported_path}` is not reachable"),
            )
            .primary(import.module_path().span(), "imported module was not found")
            .finish(),
        );
        return;
    };

    let imported_index = imported_id.raw() as usize;
    for binder in import.binders() {
        let imported_item = modules[imported_index]
            .resolve_local(binder.text())
            .and_then(|id| modules[imported_index].items().get(id.index() as usize))
            .map(|item| (item.id(), item.is_public(), item.span()));

        match imported_item {
            Some((item_id, true, _item_span)) => {
                if modules[importing_index]
                    .resolve_local(binder.text())
                    .is_some()
                    || modules[importing_index]
                        .resolve_imported(binder.text())
                        .is_some()
                {
                    diagnostics.push(
                        Diagnostic::builder(
                            Severity::Error,
                            DiagnosticCode::ResolveDuplicate,
                            format!("duplicate imported name `{}`", binder.text()),
                        )
                        .primary(binder.span(), "import conflicts with an existing name")
                        .finish(),
                    );
                } else {
                    modules[importing_index]
                        .imported_symbols
                        .insert(binder.text().to_string(), item_id);
                }
            }
            Some((_item_id, false, item_span)) => diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::ResolvePrivate,
                    format!("`{}` is private in module `{imported_path}`", binder.text()),
                )
                .primary(binder.span(), "private item imported here")
                .secondary(item_span, "item is declared without `pub`")
                .finish(),
            ),
            None => diagnostics.push(
                Diagnostic::builder(
                    Severity::Error,
                    DiagnosticCode::ResolveImport,
                    format!(
                        "module `{imported_path}` does not export `{}`",
                        binder.text()
                    ),
                )
                .primary(binder.span(), "missing exported item")
                .finish(),
            ),
        }
    }
}

fn duplicate_module(summary: &CheckModuleSummary, _first: ModuleId) -> Diagnostic {
    Diagnostic::builder(
        Severity::Error,
        DiagnosticCode::ResolveDuplicate,
        format!("duplicate module `{}`", summary.module_path().as_dotted()),
    )
    .primary(summary.module_path().span(), "module path is already used")
    .finish()
}
