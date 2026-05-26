use super::StableHash;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Pass {
    ScalarPeephole,
    DeadCode,
    MaskAlgebra,
    BranchSelect,
    TableLoopFusion,
}

impl Pass {
    pub const ALL: [Pass; 5] = [
        Pass::ScalarPeephole,
        Pass::DeadCode,
        Pass::MaskAlgebra,
        Pass::BranchSelect,
        Pass::TableLoopFusion,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Pass::ScalarPeephole => "scalar-peephole",
            Pass::DeadCode => "dead-code",
            Pass::MaskAlgebra => "mask-algebra",
            Pass::BranchSelect => "branch-select",
            Pass::TableLoopFusion => "table-loop-fusion",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|pass| pass.name() == name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PassSet {
    enabled: Vec<Pass>,
}

impl PassSet {
    pub fn empty() -> Self {
        Self {
            enabled: Vec::new(),
        }
    }

    pub fn release_default() -> Self {
        Self {
            enabled: Pass::ALL.to_vec(),
        }
    }

    pub fn only(pass: Pass) -> Self {
        Self {
            enabled: vec![pass],
        }
    }

    pub fn enabled(&self, pass: Pass) -> bool {
        self.enabled.contains(&pass)
    }

    pub fn with_disabled(mut self, pass: Pass) -> Self {
        self.enabled.retain(|enabled| *enabled != pass);
        self
    }

    pub fn with_enabled(mut self, pass: Pass) -> Self {
        if !self.enabled.contains(&pass) {
            self.enabled.push(pass);
            self.enabled.sort();
        }
        self
    }

    pub fn names(&self) -> Vec<&'static str> {
        self.enabled.iter().map(|pass| pass.name()).collect()
    }

    pub fn stable_hash(&self) -> StableHash {
        let names = self.names().join(",");
        super::stable_hash_text("wmir.pass-set.v0", &names)
    }
}

#[cfg(test)]
mod tests {
    use super::{Pass, PassSet};

    #[test]
    fn pass_set_can_enable_disable_and_select_only() {
        let default = PassSet::release_default();
        assert!(default.enabled(Pass::ScalarPeephole));

        let disabled = default.with_disabled(Pass::ScalarPeephole);
        assert!(!disabled.enabled(Pass::ScalarPeephole));

        let only = PassSet::only(Pass::MaskAlgebra);
        assert!(only.enabled(Pass::MaskAlgebra));
        assert!(!only.enabled(Pass::ScalarPeephole));

        let enabled = PassSet::empty().with_enabled(Pass::ScalarPeephole);
        assert!(enabled.enabled(Pass::ScalarPeephole));
        assert_eq!(enabled.names(), vec!["scalar-peephole"]);
    }

    #[test]
    fn unknown_pass_names_are_rejected() {
        assert_eq!(
            Pass::from_name("scalar-peephole"),
            Some(Pass::ScalarPeephole)
        );
        assert_eq!(Pass::from_name("not-a-pass"), None);
    }
}
