use super::summary::CheckModuleSummary;

#[derive(Clone, Debug, Default)]
pub struct SemanticReport {
    checked_modules: usize,
    checked_items: usize,
    checked_bodies: usize,
    ownership_diagnostics: usize,
    effect_diagnostics: usize,
    layout_diagnostics: usize,
}

impl SemanticReport {
    pub const fn new(
        checked_modules: usize,
        checked_items: usize,
        checked_bodies: usize,
        ownership_diagnostics: usize,
        effect_diagnostics: usize,
        layout_diagnostics: usize,
    ) -> Self {
        Self {
            checked_modules,
            checked_items,
            checked_bodies,
            ownership_diagnostics,
            effect_diagnostics,
            layout_diagnostics,
        }
    }

    pub const fn checked_modules(&self) -> usize {
        self.checked_modules
    }

    pub const fn checked_items(&self) -> usize {
        self.checked_items
    }

    pub const fn checked_bodies(&self) -> usize {
        self.checked_bodies
    }

    pub const fn ownership_diagnostics(&self) -> usize {
        self.ownership_diagnostics
    }

    pub const fn effect_diagnostics(&self) -> usize {
        self.effect_diagnostics
    }

    pub const fn layout_diagnostics(&self) -> usize {
        self.layout_diagnostics
    }
}

pub fn build_semantic_report(
    summaries: &[CheckModuleSummary],
    ownership_diagnostics: usize,
    effect_diagnostics: usize,
    layout_diagnostics: usize,
) -> SemanticReport {
    let checked_modules = summaries.len();
    let checked_items = summaries.iter().map(|summary| summary.items().len()).sum();
    let checked_bodies = summaries
        .iter()
        .flat_map(|summary| summary.items())
        .flat_map(|item| item.members())
        .filter(|member| member.body().is_some())
        .count();

    SemanticReport::new(
        checked_modules,
        checked_items,
        checked_bodies,
        ownership_diagnostics,
        effect_diagnostics,
        layout_diagnostics,
    )
}
