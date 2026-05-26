use super::{MirModule, MirType};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MirReport {
    regions: usize,
    blocks: usize,
    operations: usize,
    values: usize,
    places: usize,
    state_tokens: usize,
    capacity_tokens: usize,
}

impl MirReport {
    pub const fn new(
        regions: usize,
        blocks: usize,
        operations: usize,
        values: usize,
        places: usize,
        state_tokens: usize,
        capacity_tokens: usize,
    ) -> Self {
        Self {
            regions,
            blocks,
            operations,
            values,
            places,
            state_tokens,
            capacity_tokens,
        }
    }

    pub const fn regions(&self) -> usize {
        self.regions
    }

    pub const fn blocks(&self) -> usize {
        self.blocks
    }

    pub const fn operations(&self) -> usize {
        self.operations
    }

    pub const fn values(&self) -> usize {
        self.values
    }

    pub const fn places(&self) -> usize {
        self.places
    }

    pub const fn state_tokens(&self) -> usize {
        self.state_tokens
    }

    pub const fn capacity_tokens(&self) -> usize {
        self.capacity_tokens
    }
}

pub fn report_for_module(module: &MirModule) -> MirReport {
    let state_tokens = module
        .values()
        .iter()
        .filter(|value| matches!(value.ty(), MirType::StateToken(_)))
        .count();
    let capacity_tokens = module
        .values()
        .iter()
        .filter(|value| matches!(value.ty(), MirType::CapacityToken { .. }))
        .count();

    MirReport::new(
        module.regions().len(),
        module.blocks().len(),
        module.operations().len(),
        module.values().len(),
        module.places().len(),
        state_tokens,
        capacity_tokens,
    )
}

#[cfg(test)]
mod tests {
    use super::MirReport;

    #[test]
    fn report_counts_are_stable() {
        let report = MirReport::new(2, 3, 5, 8, 13, 21, 34);
        assert_eq!(report.regions(), 2);
        assert_eq!(report.blocks(), 3);
        assert_eq!(report.operations(), 5);
        assert_eq!(report.values(), 8);
        assert_eq!(report.places(), 13);
        assert_eq!(report.state_tokens(), 21);
        assert_eq!(report.capacity_tokens(), 34);
    }
}
