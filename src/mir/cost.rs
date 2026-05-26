#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TargetProfile {
    GenericAArch64,
    CortexA78,
    NeoverseV2,
    AppleFirestorm,
}

use super::OptimizerKey;
use super::ledger::{LedgerStatus, OptimizationLedger};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CostDecision {
    UseStaticEstimate,
    UseMeasuredWinner,
    AvoidMeasuredLoser,
    Remeasure,
}

impl CostDecision {
    pub fn from_ledger(ledger: &OptimizationLedger, key: &OptimizerKey) -> Self {
        match ledger.lookup(key).map(|entry| entry.status()) {
            Some(LedgerStatus::KnownWinner) => CostDecision::UseMeasuredWinner,
            Some(LedgerStatus::KnownLoser) => CostDecision::AvoidMeasuredLoser,
            Some(LedgerStatus::NeedsRemeasure) => CostDecision::Remeasure,
            None => CostDecision::UseStaticEstimate,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BranchSelectCost {
    pub csel_latency: u8,
    pub csel_throughput_per_cycle: u8,
    pub max_pure_ops: u8,
    pub predictable_branch_preferred_in_loop: bool,
}

impl TargetProfile {
    pub const fn name(self) -> &'static str {
        match self {
            TargetProfile::GenericAArch64 => "generic-aarch64",
            TargetProfile::CortexA78 => "cortex-a78",
            TargetProfile::NeoverseV2 => "neoverse-v2",
            TargetProfile::AppleFirestorm => "apple-firestorm",
        }
    }

    pub const fn costs(self) -> BranchSelectCost {
        match self {
            TargetProfile::GenericAArch64 => BranchSelectCost {
                csel_latency: 1,
                csel_throughput_per_cycle: 2,
                max_pure_ops: 3,
                predictable_branch_preferred_in_loop: true,
            },
            TargetProfile::CortexA78
            | TargetProfile::NeoverseV2
            | TargetProfile::AppleFirestorm => BranchSelectCost {
                csel_latency: 1,
                csel_throughput_per_cycle: 4,
                max_pure_ops: 3,
                predictable_branch_preferred_in_loop: true,
            },
        }
    }
}

impl BranchSelectCost {
    pub const fn should_if_convert(
        self,
        in_loop: bool,
        predictable_branch: bool,
        pure_ops: u8,
    ) -> bool {
        pure_ops <= self.max_pure_ops
            && !(in_loop && predictable_branch && self.predictable_branch_preferred_in_loop)
    }
}
