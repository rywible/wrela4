#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipMode {
    Read,
    Mut,
    Own,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityPath {
    class_name: String,
    path: Vec<String>,
}

impl CapabilityPath {
    pub fn new(class_name: String, path: Vec<String>) -> Self {
        Self { class_name, path }
    }

    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    pub fn path(&self) -> &[String] {
        &self.path
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrapProvenance {
    reason: String,
}

impl TrapProvenance {
    pub fn new(reason: String) -> Self {
        Self { reason }
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }
}

use super::OperationId;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OperationFacts {
    ownership: Vec<OwnershipMode>,
    capabilities: Vec<CapabilityPath>,
    trap: Option<TrapProvenance>,
    state_edge: bool,
    capacity_token: bool,
    fused_with: Option<OperationId>,
    fused_into: Option<OperationId>,
}

impl OperationFacts {
    pub fn empty() -> Self {
        Self::default()
    }

    pub fn has_state_edge(&self) -> bool {
        self.state_edge
    }

    pub fn has_capacity_token(&self) -> bool {
        self.capacity_token
    }

    pub fn mark_fused_with(&mut self, other: OperationId) {
        self.fused_with = Some(other);
    }

    pub fn mark_fused_into(&mut self, other: OperationId) {
        self.fused_into = Some(other);
    }

    pub fn fused_with(&self) -> Option<OperationId> {
        self.fused_with
    }

    pub fn fused_into(&self) -> Option<OperationId> {
        self.fused_into
    }

    pub fn ownership(&self) -> &[OwnershipMode] {
        &self.ownership
    }

    pub fn capabilities(&self) -> &[CapabilityPath] {
        &self.capabilities
    }

    pub fn trap(&self) -> Option<&TrapProvenance> {
        self.trap.as_ref()
    }

    pub fn set_ownership(&mut self, modes: Vec<OwnershipMode>) {
        self.ownership = modes;
    }

    pub fn push_capability(&mut self, capability: CapabilityPath) {
        self.capabilities.push(capability);
    }

    pub fn set_trap(&mut self, trap: Option<TrapProvenance>) {
        self.trap = trap;
    }

    pub fn set_state_edge(&mut self, enabled: bool) {
        self.state_edge = enabled;
    }

    pub fn set_capacity_token(&mut self, enabled: bool) {
        self.capacity_token = enabled;
    }
}

impl OwnershipMode {
    pub fn as_str(self) -> &'static str {
        match self {
            OwnershipMode::Read => "read",
            OwnershipMode::Mut => "mut",
            OwnershipMode::Own => "own",
        }
    }

    pub fn parse_name(text: &str) -> Option<Self> {
        match text {
            "read" => Some(OwnershipMode::Read),
            "mut" => Some(OwnershipMode::Mut),
            "own" => Some(OwnershipMode::Own),
            _ => None,
        }
    }
}
