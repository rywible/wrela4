use super::{OperationId, Pass, RegionId, StableHash};

pub const CERTIFICATE_VERSION: u16 = 1;
pub const MAX_REWRITE_EVENTS: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RewriteRule {
    ScalarAddZero,
    MaskAndTrue,
    MaskOrFalse,
    MaskAndSelf,
    MaskOrSelf,
    MaskDoubleNot,
    BranchSelect,
    TableLoopFusion,
}

impl RewriteRule {
    pub const fn name(self) -> &'static str {
        match self {
            RewriteRule::ScalarAddZero => "scalar-add-zero",
            RewriteRule::MaskAndTrue => "mask-and-true",
            RewriteRule::MaskOrFalse => "mask-or-false",
            RewriteRule::MaskAndSelf => "mask-and-self",
            RewriteRule::MaskOrSelf => "mask-or-self",
            RewriteRule::MaskDoubleNot => "mask-double-not",
            RewriteRule::BranchSelect => "branch-select",
            RewriteRule::TableLoopFusion => "table-loop-fusion",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RewriteFact {
    PureOperation { op: OperationId },
    LiteralZero { value: String },
    EffectAbsent { effect: &'static str },
    Authority { name: &'static str, detail: String },
    VerifierPassed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RewriteOutcome {
    PassedVerifier,
    FailedVerifier,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CertificateValidation {
    Valid,
    StaleVersion,
    MissingFacts,
    VerifierDidNotPass,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewriteCertificate {
    version: u16,
    rule: RewriteRule,
    pass: Pass,
    region: RegionId,
    source_ops: Vec<OperationId>,
    before_hash: StableHash,
    after_hash: StableHash,
    facts: Vec<RewriteFact>,
    outcome: RewriteOutcome,
}

impl RewriteCertificate {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        version: u16,
        rule: RewriteRule,
        pass: Pass,
        region: RegionId,
        source_ops: Vec<OperationId>,
        before_hash: StableHash,
        after_hash: StableHash,
        facts: Vec<RewriteFact>,
        outcome: RewriteOutcome,
    ) -> Self {
        Self {
            version,
            rule,
            pass,
            region,
            source_ops,
            before_hash,
            after_hash,
            facts,
            outcome,
        }
    }

    pub fn validate(&self) -> CertificateValidation {
        if self.version != CERTIFICATE_VERSION {
            return CertificateValidation::StaleVersion;
        }
        if self.facts.is_empty() {
            return CertificateValidation::MissingFacts;
        }
        if self.outcome != RewriteOutcome::PassedVerifier {
            return CertificateValidation::VerifierDidNotPass;
        }
        CertificateValidation::Valid
    }

    pub fn rule(&self) -> RewriteRule {
        self.rule
    }

    pub fn pass(&self) -> Pass {
        self.pass
    }

    pub fn region(&self) -> RegionId {
        self.region
    }

    pub fn source_ops(&self) -> &[OperationId] {
        &self.source_ops
    }

    pub fn before_hash(&self) -> StableHash {
        self.before_hash
    }

    pub fn after_hash(&self) -> StableHash {
        self.after_hash
    }

    pub fn facts(&self) -> &[RewriteFact] {
        &self.facts
    }

    pub fn can_record_in_ledger(&self) -> bool {
        self.validate() == CertificateValidation::Valid
    }

    pub fn invalidate_reason(&self) -> Option<&'static str> {
        match self.validate() {
            CertificateValidation::Valid => None,
            CertificateValidation::StaleVersion => Some("stale certificate version"),
            CertificateValidation::MissingFacts => Some("missing rewrite facts"),
            CertificateValidation::VerifierDidNotPass => Some("verifier did not pass"),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewriteEvent {
    certificate: RewriteCertificate,
}

impl RewriteEvent {
    pub fn certificate(&self) -> &RewriteCertificate {
        &self.certificate
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RewriteEventLog {
    limit: usize,
    events: Vec<RewriteEvent>,
    truncated: bool,
}

impl Default for RewriteEventLog {
    fn default() -> Self {
        Self::with_limit(MAX_REWRITE_EVENTS)
    }
}

impl RewriteEventLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_limit(limit: usize) -> Self {
        Self {
            limit,
            events: Vec::new(),
            truncated: false,
        }
    }

    pub fn push(&mut self, certificate: RewriteCertificate) -> bool {
        if certificate.validate() != CertificateValidation::Valid {
            return false;
        }
        if self.events.len() >= self.limit {
            self.truncated = true;
            return false;
        }
        self.events.push(RewriteEvent { certificate });
        true
    }

    pub fn events(&self) -> &[RewriteEvent] {
        &self.events
    }

    pub fn truncated(&self) -> bool {
        self.truncated
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CERTIFICATE_VERSION, CertificateValidation, RewriteCertificate, RewriteEventLog,
        RewriteFact, RewriteOutcome, RewriteRule,
    };
    use crate::mir::{OperationId, Pass, RegionId, StableHash};

    #[test]
    fn certificate_requires_current_version_facts_and_passed_verifier() {
        let cert = RewriteCertificate::new(
            CERTIFICATE_VERSION,
            RewriteRule::ScalarAddZero,
            Pass::ScalarPeephole,
            RegionId::new(0),
            vec![OperationId::new(0)],
            StableHash::new(1),
            StableHash::new(2),
            vec![RewriteFact::LiteralZero {
                value: "v1".to_string(),
            }],
            RewriteOutcome::PassedVerifier,
        );

        assert_eq!(cert.validate(), CertificateValidation::Valid);

        let stale = RewriteCertificate::new(
            CERTIFICATE_VERSION + 1,
            RewriteRule::ScalarAddZero,
            Pass::ScalarPeephole,
            RegionId::new(0),
            vec![OperationId::new(0)],
            StableHash::new(1),
            StableHash::new(2),
            vec![RewriteFact::LiteralZero {
                value: "v1".to_string(),
            }],
            RewriteOutcome::PassedVerifier,
        );
        assert_eq!(stale.validate(), CertificateValidation::StaleVersion);

        let empty_facts = RewriteCertificate::new(
            CERTIFICATE_VERSION,
            RewriteRule::ScalarAddZero,
            Pass::ScalarPeephole,
            RegionId::new(0),
            vec![OperationId::new(0)],
            StableHash::new(1),
            StableHash::new(2),
            Vec::new(),
            RewriteOutcome::PassedVerifier,
        );
        assert_eq!(empty_facts.validate(), CertificateValidation::MissingFacts);
    }

    #[test]
    fn rewrite_event_log_is_bounded() {
        let cert = RewriteCertificate::new(
            CERTIFICATE_VERSION,
            RewriteRule::ScalarAddZero,
            Pass::ScalarPeephole,
            RegionId::new(0),
            vec![OperationId::new(0)],
            StableHash::new(1),
            StableHash::new(2),
            vec![RewriteFact::PureOperation {
                op: OperationId::new(0),
            }],
            RewriteOutcome::PassedVerifier,
        );
        let mut log = RewriteEventLog::with_limit(1);

        assert!(log.push(cert.clone()));
        assert!(!log.push(cert));
        assert!(log.truncated());
        assert_eq!(log.events().len(), 1);
    }

    #[test]
    fn stale_certificate_cannot_be_promoted_to_ledger_fact() {
        let cert = RewriteCertificate::new(
            CERTIFICATE_VERSION - 1,
            RewriteRule::ScalarAddZero,
            Pass::ScalarPeephole,
            RegionId::new(0),
            vec![OperationId::new(0)],
            StableHash::new(1),
            StableHash::new(2),
            vec![RewriteFact::VerifierPassed],
            RewriteOutcome::PassedVerifier,
        );

        assert!(!cert.can_record_in_ledger());
        assert_eq!(cert.invalidate_reason(), Some("stale certificate version"));
    }
}
