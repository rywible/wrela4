# MIR 03 Certified Scalar Aegraph Release Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Wrela's first release optimizer slice with bounded aegraph overlays, stable hash V0, versioned rewrite certificates, bounded rewrite events, one certified scalar rewrite family, scoped elaboration back to W-MIR, and generated-code A/B evidence.

**Architecture:** MIR 03 keeps W-MIR as the source of truth. Release optimization builds ephemeral aegraph overlays for pure subgraphs inside lambda regions, applies certified scalar identity rewrites through that overlay with deterministic fuel, verifies the candidate W-MIR, records a bounded event, then reuses MIR 02 LIR/codegen. This is the first Trident slice: every applied rewrite must be explainable by facts, measurable when codegen support exists, and recorded for later debugging.

**Tech Stack:** Rust 2024, standard library only, W-MIR from MIR 01, dev codegen/perf tooling from MIR 02, handwritten stable hash, handwritten certificate/event records, handwritten aegraph overlay, handwritten rewrite scheduler.

---

## Locked Decisions

- The aegraph is an overlay, not a replacement for W-MIR.
- Aegraph overlays include only pure subgraphs. State-edge ops, capacity-token ops, trap producers, and non-empty effect-set ops remain in the W-MIR skeleton.
- Saturation is bounded by deterministic per-region fuel.
- Exact extraction is out of scope for MIR 03. Scoped greedy elaboration is the extraction strategy.
- Every release pass is controllable by name for A/B benchmarking.
- MIR 03 introduces `StableHash` V0 using handwritten FNV-1a 64-bit over canonical text fragments. It is deterministic, not cryptographic, and not a trust boundary.
- MIR 03 introduces `RewriteCertificate` schema version `1`. Every applied rewrite must have at least one legality fact and a verifier result of `Passed`.
- The rewrite event log is in-memory and bounded at `4096` events per release run. When full, later rewrites are skipped and `events_truncated` is set in the report.
- Generated-code A/B benchmarks require matching output checksums before runtime comparisons count.
- Runtime speed claims require at least seven samples per side, `p90 / p10 <= 1.10`, and at least 3% median movement. Otherwise the result is reported as `inconclusive`.
- MIR 03 implements only scalar release rewrites. Mask algebra, branch/select, table loop fusion, persistent ledger storage, and authority algebra are later plans.
- No external crate dependencies are introduced.

## Planned File Structure

```text
src/mir/
  hash.rs                 # stable hash V0 for text fragments, modules, pass configs
  cert.rs                 # rewrite certificate and bounded event log V0
  aegraph.rs              # region-local pure-subgraph overlay
  rewrite.rs              # release optimizer, certified scalar rewrite, report
  pass_control.rs         # stable pass names and pass-set controls
  perf.rs                 # release optimizer reports in perf code JSON
  build.rs
  text.rs
tests/
  mir_release.rs
  mir_perf.rs
fixtures/
  mir/
    scalar_identity.wrela
  perf/
    scalar_identity.wrela
```

## Public API Shape

```rust
pub struct mir::pass_control::PassSet;
pub fn mir::optimize_release(module: &mir::MirModule, passes: &PassSet) -> mir::ReleaseOptimizeResult;

pub struct mir::ReleaseOptimizeResult {
    pub fn module(&self) -> &mir::MirModule;
    pub fn report(&self) -> &mir::ReleaseOptimizeReport;
}

pub struct mir::RewriteCertificate;
pub struct mir::RewriteEvent;
pub struct mir::StableHash;
```

## Parallel Work Map

- Task 1 must run first because pass controls are used by every subsequent task.
- Task 2 depends on Task 1 only and creates stable hash V0.
- Task 3 depends on Task 2 and creates certificate/event V0.
- Task 4 depends on Tasks 1-3 and defines the aegraph overlay.
- Task 5 depends on Task 4 and creates release optimize reports without changing MIR.
- Task 6 depends on Task 5 and owns the first certified scalar rewrite.
- Task 7 depends on Task 6 and wires release data into generated-code perf JSON.
- Task 8 runs last and owns quality gate plus Phase A handoff.
- Integration-owned shared files: `src/mir/mod.rs`, `src/mir/rewrite.rs`, `src/mir/perf.rs`, `src/command.rs`, `tests/mir_release.rs`, and `tests/mir_perf.rs`. Only one subagent edits each at a time.

---

### Task 1: Release Pass Controls

**Files:**
- Create: `src/mir/pass_control.rs`
- Modify: `src/mir/mod.rs`
- Modify: `src/command.rs`
- Test: unit tests in `src/mir/pass_control.rs`

**Description:** Add named pass controls used by release optimization and perf A/B comparisons. Pass parsing is deterministic and left-to-right.

- [ ] **Step 1: Write failing pass-control tests**

Create `src/mir/pass_control.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::{Pass, PassSet};

    #[test]
    fn pass_set_can_enable_disable_and_select_only() {
        let default = PassSet::release_default();
        assert!(default.enabled(Pass::ScalarPeephole));

        let disabled = default.with_disabled(Pass::ScalarPeephole);
        assert!(!disabled.enabled(Pass::ScalarPeephole));

        let only = PassSet::only(Pass::CopyProp);
        assert!(only.enabled(Pass::CopyProp));
        assert!(!only.enabled(Pass::ScalarPeephole));

        let enabled = PassSet::empty().with_enabled(Pass::ScalarPeephole);
        assert!(enabled.enabled(Pass::ScalarPeephole));
        assert_eq!(enabled.names(), vec!["scalar-peephole"]);
    }

    #[test]
    fn unknown_pass_names_are_rejected() {
        assert_eq!(Pass::from_name("scalar-peephole"), Some(Pass::ScalarPeephole));
        assert_eq!(Pass::from_name("not-a-pass"), None);
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test mir::pass_control::tests::pass_set_can_enable_disable_and_select_only
cargo test mir::pass_control::tests::unknown_pass_names_are_rejected
```

Expected: fail because pass controls are not implemented.

- [ ] **Step 3: Implement pass controls**

Replace `src/mir/pass_control.rs` with:

```rust
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Pass {
    ScalarPeephole,
    DeadCode,
    CopyProp,
}

impl Pass {
    pub const ALL: [Pass; 3] = [
        Pass::ScalarPeephole,
        Pass::DeadCode,
        Pass::CopyProp,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Pass::ScalarPeephole => "scalar-peephole",
            Pass::DeadCode => "dead-code",
            Pass::CopyProp => "copy-prop",
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
        Self { enabled: Vec::new() }
    }

    pub fn release_default() -> Self {
        Self {
            enabled: Pass::ALL.to_vec(),
        }
    }

    pub fn only(pass: Pass) -> Self {
        Self { enabled: vec![pass] }
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
}
```

- [ ] **Step 4: Export pass controls**

Modify `src/mir/mod.rs`:

```rust
pub mod pass_control;

pub use pass_control::{Pass, PassSet};
```

- [ ] **Step 5: Extend CLI parsing for pass flags**

In `src/command.rs`, extend `perf code` argument parsing to accept:

```text
--enable-pass <name>
--disable-pass <name>
--only-pass <name>
```

The precedence rule is left-to-right:

```rust
let mut pass_set = wrela::mir::PassSet::release_default();
// --only-pass replaces pass_set.
// --disable-pass removes from the current pass_set.
// --enable-pass adds to the current pass_set.
```

Unknown pass names are bad usage and exit `2` with a message containing `unknown release pass`.

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test mir::pass_control::tests::
cargo test --test mir_perf perf_code_json_reports_checksum_or_unsupported_host
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add src/mir/mod.rs src/mir/pass_control.rs src/command.rs
git commit -m "feat: add release pass controls -Codex Automated"
```

**Acceptance criteria:**
- Release pass names are stable strings.
- Unknown pass names are CLI errors with exit `2`.
- Pass flags parse for `perf code`.

---

### Task 2: Stable Hash V0

**Files:**
- Create: `src/mir/hash.rs`
- Modify: `src/mir/mod.rs`
- Test: unit tests in `src/mir/hash.rs`

**Description:** Add deterministic stable hashes for pass configs and canonical text fragments. MIR 03 uses these hashes in rewrite certificates and perf reports; MIR 05 extends them into persistent ledger keys.

- [ ] **Step 1: Write failing stable-hash tests**

Create `src/mir/hash.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::{stable_hash_text, StableHash};
    use crate::mir::{Pass, PassSet};

    #[test]
    fn stable_hash_text_is_deterministic_and_domain_separated() {
        let a = stable_hash_text("wmir.module.v0", "op binary +");
        let b = stable_hash_text("wmir.module.v0", "op binary +");
        let c = stable_hash_text("wmir.region.v0", "op binary +");

        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, StableHash::ZERO);
    }

    #[test]
    fn pass_set_hash_depends_on_enabled_passes() {
        let scalar = PassSet::only(Pass::ScalarPeephole).stable_hash();
        let copy = PassSet::only(Pass::CopyProp).stable_hash();

        assert_ne!(scalar, copy);
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test mir::hash::tests::
```

Expected: fail because stable hash V0 is not implemented.

- [ ] **Step 3: Implement FNV-1a stable hash**

Replace `src/mir/hash.rs` with:

```rust
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct StableHash(u64);

impl StableHash {
    pub const ZERO: StableHash = StableHash(0);

    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for StableHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x00000100000001b3;

pub fn stable_hash_bytes(domain: &str, bytes: &[u8]) -> StableHash {
    let mut hash = FNV_OFFSET;
    for byte in domain.as_bytes().iter().chain([0u8].iter()).chain(bytes.iter()) {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    StableHash::new(hash)
}

pub fn stable_hash_text(domain: &str, text: &str) -> StableHash {
    stable_hash_bytes(domain, text.as_bytes())
}
```

- [ ] **Step 4: Add pass-set hash**

Modify `src/mir/pass_control.rs`:

```rust
impl PassSet {
    pub fn stable_hash(&self) -> crate::mir::StableHash {
        let names = self.names().join(",");
        crate::mir::stable_hash_text("wmir.pass-set.v0", &names)
    }
}
```

- [ ] **Step 5: Export hash API**

Modify `src/mir/mod.rs`:

```rust
pub mod hash;

pub use hash::{stable_hash_bytes, stable_hash_text, StableHash};
```

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test mir::hash::tests::
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add src/mir/mod.rs src/mir/hash.rs src/mir/pass_control.rs
git commit -m "feat: add MIR stable hash v0 -Codex Automated"
```

**Acceptance criteria:**
- Stable hash is deterministic and domain-separated.
- Pass-set hash changes when enabled passes change.
- Hash implementation uses only the standard library.
- The plan and code state clearly that this is not a cryptographic hash.

---

### Task 3: Rewrite Certificate And Event V0

**Files:**
- Create: `src/mir/cert.rs`
- Modify: `src/mir/mod.rs`
- Test: unit tests in `src/mir/cert.rs`

**Description:** Add versioned rewrite certificates and a bounded rewrite event log. Rewrites cannot be counted as applied unless their certificate validates.

- [ ] **Step 1: Write failing certificate tests**

Create `src/mir/cert.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::{
        CertificateValidation, RewriteCertificate, RewriteEventLog, RewriteFact,
        RewriteOutcome, RewriteRule, CERTIFICATE_VERSION, MAX_REWRITE_EVENTS,
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
            vec![RewriteFact::LiteralZero { value: "v1".to_string() }],
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
            vec![RewriteFact::LiteralZero { value: "v1".to_string() }],
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
            vec![RewriteFact::PureOperation { op: OperationId::new(0) }],
            RewriteOutcome::PassedVerifier,
        );
        let mut log = RewriteEventLog::with_limit(1);

        assert!(log.push(cert.clone()));
        assert!(!log.push(cert));
        assert!(log.truncated());
        assert_eq!(log.events().len(), 1);
        assert!(MAX_REWRITE_EVENTS >= 1);
    }
}
```

- [ ] **Step 2: Run tests and verify failure**

Run:

```bash
cargo test mir::cert::tests::
```

Expected: fail because certificate types are not implemented.

- [ ] **Step 3: Implement certificate types**

Replace `src/mir/cert.rs` with:

```rust
use super::{OperationId, Pass, RegionId, StableHash};

pub const CERTIFICATE_VERSION: u16 = 1;
pub const MAX_REWRITE_EVENTS: usize = 4096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RewriteRule {
    ScalarAddZero,
}

impl RewriteRule {
    pub const fn name(self) -> &'static str {
        match self {
            RewriteRule::ScalarAddZero => "scalar-add-zero",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RewriteFact {
    PureOperation { op: OperationId },
    LiteralZero { value: String },
    EffectAbsent { effect: &'static str },
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

    pub fn rule(&self) -> RewriteRule { self.rule }
    pub fn pass(&self) -> Pass { self.pass }
    pub fn region(&self) -> RegionId { self.region }
    pub fn source_ops(&self) -> &[OperationId] { &self.source_ops }
    pub fn before_hash(&self) -> StableHash { self.before_hash }
    pub fn after_hash(&self) -> StableHash { self.after_hash }
    pub fn facts(&self) -> &[RewriteFact] { &self.facts }
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

impl RewriteEventLog {
    pub fn new() -> Self {
        Self::with_limit(MAX_REWRITE_EVENTS)
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
```

- [ ] **Step 4: Export certificate API**

Modify `src/mir/mod.rs`:

```rust
pub mod cert;

pub use cert::{
    CertificateValidation, RewriteCertificate, RewriteEvent, RewriteEventLog, RewriteFact,
    RewriteOutcome, RewriteRule, CERTIFICATE_VERSION, MAX_REWRITE_EVENTS,
};
```

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test mir::cert::tests::
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/mir/mod.rs src/mir/cert.rs
git commit -m "feat: add rewrite certificates v0 -Codex Automated"
```

**Acceptance criteria:**
- Certificate schema has an explicit version.
- Certificates without facts are invalid.
- Certificates without a passed verifier result are invalid.
- Event log is bounded and exposes truncation.

---

### Task 4: Region-Local Aegraph Overlay

**Files:**
- Create: `src/mir/aegraph.rs`
- Modify: `src/mir/mod.rs`
- Test: unit tests in `src/mir/aegraph.rs`

**Description:** Add an ephemeral acyclic equality structure for pure W-MIR operations. This task builds the overlay and reports what it contains; it does not rewrite yet.

- [ ] **Step 1: Write failing aegraph tests**

Create `src/mir/aegraph.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::{AeGraph, Enode};
    use crate::mir::{OperationId, OperationKind, ValueId};

    #[test]
    fn aegraph_interns_identical_pure_enodes() {
        let mut graph = AeGraph::new();
        let a = graph.intern(Enode::new(
            OperationKind::Binary("+".to_string()),
            vec![ValueId::new(0), ValueId::new(1)],
            OperationId::new(0),
        ));
        let b = graph.intern(Enode::new(
            OperationKind::Binary("+".to_string()),
            vec![ValueId::new(0), ValueId::new(1)],
            OperationId::new(1),
        ));

        assert_eq!(a, b);
        assert_eq!(graph.enode_count(), 1);
    }
}
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
cargo test mir::aegraph::tests::aegraph_interns_identical_pure_enodes
```

Expected: fail because aegraph is not implemented.

- [ ] **Step 3: Implement overlay data structures**

Replace `src/mir/aegraph.rs` with:

```rust
use std::collections::BTreeMap;

use super::{OperationId, OperationKind, ValueId};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct EClassId(u32);

impl EClassId {
    pub const fn raw(self) -> u32 { self.0 }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct EnodeKey {
    kind: OperationKind,
    operands: Vec<ValueId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Enode {
    key: EnodeKey,
    source: OperationId,
}

impl Enode {
    pub fn new(kind: OperationKind, operands: Vec<ValueId>, source: OperationId) -> Self {
        Self { key: EnodeKey { kind, operands }, source }
    }

    fn key(&self) -> &EnodeKey {
        &self.key
    }

    pub fn source(&self) -> OperationId {
        self.source
    }
}

#[derive(Clone, Debug, Default)]
pub struct AeGraph {
    keys: BTreeMap<EnodeKey, EClassId>,
    enodes: Vec<Enode>,
}

impl AeGraph {
    pub fn new() -> Self { Self::default() }

    pub fn intern(&mut self, enode: Enode) -> EClassId {
        if let Some(id) = self.keys.get(enode.key()).copied() {
            return id;
        }
        let id = EClassId(self.enodes.len() as u32);
        self.keys.insert(enode.key().clone(), id);
        self.enodes.push(enode);
        id
    }

    pub fn enode_count(&self) -> usize {
        self.enodes.len()
    }

    pub fn enodes(&self) -> &[Enode] {
        &self.enodes
    }
}
```

- [ ] **Step 4: Export module**

Modify `src/mir/mod.rs`:

```rust
pub mod aegraph;
pub use aegraph::{AeGraph, EClassId, Enode};
```

- [ ] **Step 5: Run focused test**

Run:

```bash
cargo test mir::aegraph::tests::aegraph_interns_identical_pure_enodes
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/mir/mod.rs src/mir/aegraph.rs
git commit -m "feat: add aegraph overlay core -Codex Automated"
```

**Acceptance criteria:**
- Aegraph interning is deterministic.
- Identical pure enodes share one e-class ID.
- No stateful operation is lifted yet.

---

### Task 5: Release Optimize Result And Pure Subgraph Lifting

**Files:**
- Create: `src/mir/rewrite.rs`
- Modify: `src/mir/mod.rs`
- Test: `tests/mir_release.rs`

**Description:** Build region-local aegraph overlays from pure operations and return release optimization reports without changing code yet. Reports include stable hashes and an empty bounded event log.

- [ ] **Step 1: Write failing release report test**

Create `tests/mir_release.rs`:

```rust
use std::path::PathBuf;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("mir")
        .join(rel)
}

#[test]
fn release_optimizer_reports_lifted_pure_operations_without_rewrites() {
    let check = wrela::check::check_root(fixture("data_flow.wrela"));
    assert!(check.ok(), "{:?}", check.diagnostics());
    let mir = wrela::mir::build_mir(&check);
    let passes = wrela::mir::PassSet::release_default();

    let optimized = wrela::mir::optimize_release(mir.module().unwrap(), &passes);

    assert!(optimized.report().regions_visited() >= 1);
    assert!(optimized.report().pure_ops_lifted() >= 1);
    assert_eq!(optimized.report().rewrite_events().len(), 0);
    assert_eq!(
        wrela::mir::text::render_module(optimized.module()),
        wrela::mir::text::render_module(mir.module().unwrap())
    );
}
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
cargo test --test mir_release release_optimizer_reports_lifted_pure_operations_without_rewrites
```

Expected: fail because release optimizer is not implemented.

- [ ] **Step 3: Implement release optimize result**

Create `src/mir/rewrite.rs`:

```rust
use super::{
    stable_hash_text, AeGraph, Enode, MirModule, OperationKind, PassSet, RewriteEventLog,
    RewriteEvent, StableHash,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseOptimizeReport {
    regions_visited: usize,
    pure_ops_lifted: usize,
    rewrites_applied: usize,
    fuel_used: usize,
    dead_ops_removed: usize,
    before_hash: StableHash,
    after_hash: StableHash,
    events: RewriteEventLog,
}

impl ReleaseOptimizeReport {
    pub fn regions_visited(&self) -> usize { self.regions_visited }
    pub fn pure_ops_lifted(&self) -> usize { self.pure_ops_lifted }
    pub fn rewrites_applied(&self) -> usize { self.rewrites_applied }
    pub fn fuel_used(&self) -> usize { self.fuel_used }
    pub fn dead_ops_removed(&self) -> usize { self.dead_ops_removed }
    pub fn before_hash(&self) -> StableHash { self.before_hash }
    pub fn after_hash(&self) -> StableHash { self.after_hash }
    pub fn rewrite_events(&self) -> &[RewriteEvent] { self.events.events() }
    pub fn events_truncated(&self) -> bool { self.events.truncated() }
}

#[derive(Clone, Debug)]
pub struct ReleaseOptimizeResult {
    module: MirModule,
    report: ReleaseOptimizeReport,
}

impl ReleaseOptimizeResult {
    pub fn module(&self) -> &MirModule { &self.module }
    pub fn report(&self) -> &ReleaseOptimizeReport { &self.report }
}

pub fn optimize_release(module: &MirModule, _passes: &PassSet) -> ReleaseOptimizeResult {
    let before_text = crate::mir::text::render_module(module);
    let before_hash = stable_hash_text("wmir.module.v0", &before_text);
    let mut report = ReleaseOptimizeReport {
        regions_visited: 0,
        pure_ops_lifted: 0,
        rewrites_applied: 0,
        fuel_used: 0,
        dead_ops_removed: 0,
        before_hash,
        after_hash: before_hash,
        events: RewriteEventLog::new(),
    };

    for region in module.regions() {
        report.regions_visited += 1;
        let mut graph = AeGraph::new();
        for block in region.blocks() {
            for op_id in module.block(*block).operations() {
                let op = module.operation(*op_id);
                if is_pure_liftable(op.kind()) && op.effects().bits() == 0 {
                    graph.intern(Enode::new(
                        op.kind().clone(),
                        op.operands().to_vec(),
                        *op_id,
                    ));
                }
            }
        }
        report.pure_ops_lifted += graph.enode_count();
    }

    ReleaseOptimizeResult {
        module: module.clone(),
        report,
    }
}

fn is_pure_liftable(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::Literal(_)
            | OperationKind::ReadValue(_)
            | OperationKind::Binary(_)
            | OperationKind::Let(_)
    )
}
```

- [ ] **Step 4: Export release optimizer**

Modify `src/mir/mod.rs`:

```rust
pub mod rewrite;

pub use rewrite::{ReleaseOptimizeReport, ReleaseOptimizeResult, optimize_release};
```

- [ ] **Step 5: Run focused test**

Run:

```bash
cargo test --test mir_release release_optimizer_reports_lifted_pure_operations_without_rewrites
```

Expected: pass.

- [ ] **Step 6: Commit**

```bash
git add src/mir/mod.rs src/mir/rewrite.rs tests/mir_release.rs
git commit -m "feat: lift pure MIR ops for release optimization -Codex Automated"
```

**Acceptance criteria:**
- Release optimizer reports regions visited and pure ops lifted.
- Initial optimizer is behavior-preserving identity.
- Stateful/effectful ops are not lifted.
- Report includes before/after stable hashes and a bounded event log.

---

### Task 6: Certified Scalar Identity Rewrite

**Files:**
- Modify: `src/mir/rewrite.rs`
- Modify: `src/mir/ir.rs`
- Test: `tests/mir_release.rs`
- Fixture: `fixtures/mir/scalar_identity.wrela`

**Description:** Add the first deterministic certified rewrite family: `x + 0 -> x` and `0 + x -> x`. The rewrite fires only when a certificate with facts validates and the rewritten module verifies.

- [ ] **Step 1: Add scalar identity fixture**

Create `fixtures/mir/scalar_identity.wrela`:

```wrela
module perf.scalar_identity

pub class IdentityBench {
    fn run(read self) -> U32 {
        let value: U32 = 7
        let zero: U32 = 0
        let result: U32 = value + zero
        return result
    }
}
```

- [ ] **Step 2: Add failing certified rewrite test**

Append to `tests/mir_release.rs`:

```rust
#[test]
fn release_optimizer_applies_scalar_identity_with_certificate() {
    let check = wrela::check::check_root(fixture("scalar_identity.wrela"));
    assert!(check.ok(), "{:?}", check.diagnostics());
    let mir = wrela::mir::build_mir(&check);
    let passes = wrela::mir::PassSet::only(wrela::mir::Pass::ScalarPeephole);

    let optimized = wrela::mir::optimize_release(mir.module().unwrap(), &passes);
    let before = wrela::mir::text::render_module(mir.module().unwrap());
    let after = wrela::mir::text::render_module(optimized.module());

    assert!(before.contains("op binary +"));
    assert!(!after.contains("op binary +"));
    assert_eq!(optimized.report().rewrites_applied(), 1);
    assert_eq!(optimized.report().rewrite_events().len(), 1);

    let cert = optimized.report().rewrite_events()[0].certificate();
    assert_eq!(cert.rule().name(), "scalar-add-zero");
    assert_eq!(cert.pass().name(), "scalar-peephole");
    assert!(!cert.facts().is_empty());
    assert_ne!(cert.before_hash(), cert.after_hash());
    assert!(wrela::mir::verify_module(optimized.module()).ok());
}
```

- [ ] **Step 3: Run test and verify failure**

Run:

```bash
cargo test --test mir_release release_optimizer_applies_scalar_identity_with_certificate
```

Expected: fail because no rewrite applies.

- [ ] **Step 4: Add MIR mutation helpers**

In `src/mir/ir.rs`, add these methods near the existing `OperationData` and `MirModule` impls:

```rust
impl OperationData {
    pub fn replace_operand_uses(&mut self, old: ValueId, new: ValueId) {
        for operand in &mut self.operands {
            if *operand == old {
                *operand = new;
            }
        }
    }
}

impl MirModule {
    pub fn replace_value_uses(&mut self, old: ValueId, new: ValueId) {
        for operation in &mut self.operations {
            operation.replace_operand_uses(old, new);
        }
    }

    pub fn set_block_operations(&mut self, block: BlockId, operations: Vec<OperationId>) {
        self.blocks[block.raw() as usize].operations = operations;
    }
}
```

- [ ] **Step 5: Implement certified rewrite loop**

In `src/mir/rewrite.rs`, update `optimize_release` to clone the module and call `apply_scalar_add_zero` when `Pass::ScalarPeephole` is enabled:

```rust
let mut output = module.clone();
let mut fuel = 128usize;

if passes.enabled(super::Pass::ScalarPeephole) {
    apply_scalar_add_zero(&mut output, &mut report, &mut fuel);
}
let removed = remove_unused_pure_operations(&mut output);
report.dead_ops_removed += removed;
let after_text = crate::mir::text::render_module(&output);
report.after_hash = stable_hash_text("wmir.module.v0", &after_text);
```

Add the rewrite helper:

```rust
use std::collections::{BTreeMap, BTreeSet};
use super::{
    CERTIFICATE_VERSION, CertificateValidation, OperationId, RegionId, RewriteCertificate,
    RewriteFact, RewriteOutcome, RewriteRule, ValueId,
};

fn apply_scalar_add_zero(
    module: &mut MirModule,
    report: &mut ReleaseOptimizeReport,
    fuel: &mut usize,
) {
    let zero_values = literal_zero_values(module);
    let overlay = build_pure_overlay(module);
    for enode in overlay.enodes() {
        if *fuel == 0 || report.events_truncated() {
            break;
        }
        let op_id = enode.source();
        let Some(region_id) = containing_region(module, op_id) else {
            continue;
        };
        let op = module.operation(op_id).clone();
        let OperationKind::Binary(operator) = op.kind() else {
            continue;
        };
        if operator != "+" || op.operands().len() != 2 || op.results().len() != 1 {
            continue;
        }

        let left = op.operands()[0];
        let right = op.operands()[1];
        let replacement = if zero_values.contains_key(&right) {
            Some((left, right))
        } else if zero_values.contains_key(&left) {
            Some((right, left))
        } else {
            None
        };
        let Some((replacement_value, zero_value)) = replacement else {
            continue;
        };

        let before_hash = stable_hash_text("wmir.module.v0", &crate::mir::text::render_module(module));
        let mut candidate = module.clone();
        candidate.replace_value_uses(op.results()[0], replacement_value);
        let removed = remove_unused_pure_operations(&mut candidate);
        let verify = crate::mir::verify_module(&candidate);
        let after_hash = stable_hash_text("wmir.module.v0", &crate::mir::text::render_module(&candidate));
        let outcome = if verify.ok() {
            RewriteOutcome::PassedVerifier
        } else {
            RewriteOutcome::FailedVerifier
        };
        let cert = RewriteCertificate::new(
            CERTIFICATE_VERSION,
            RewriteRule::ScalarAddZero,
            crate::mir::Pass::ScalarPeephole,
            region_id,
            vec![op_id],
            before_hash,
            after_hash,
            vec![
                RewriteFact::PureOperation { op: op_id },
                RewriteFact::LiteralZero { value: format!("v{}", zero_value.raw()) },
                RewriteFact::EffectAbsent { effect: "all" },
                RewriteFact::VerifierPassed,
            ],
            outcome,
        );
        if cert.validate() != CertificateValidation::Valid {
            continue;
        }
        if !report.events.push(cert) {
            continue;
        }

        *module = candidate;
        report.rewrites_applied += 1;
        report.fuel_used += 1;
        report.dead_ops_removed += removed;
        *fuel -= 1;
    }
}
```

Add helpers:

```rust
fn containing_region(module: &MirModule, op_id: OperationId) -> Option<RegionId> {
    for (region_index, region) in module.regions().iter().enumerate() {
        for block in region.blocks() {
            if module.block(*block).operations().contains(&op_id) {
                return Some(RegionId::new(region_index as u32));
            }
        }
    }
    None
}

fn literal_zero_values(module: &MirModule) -> BTreeMap<ValueId, OperationId> {
    let mut values = BTreeMap::new();
    for (index, op) in module.operations().iter().enumerate() {
        if matches!(op.kind(), OperationKind::Literal(text) if text == "0") {
            if let Some(value) = op.results().first().copied() {
                values.insert(value, OperationId::new(index as u32));
            }
        }
    }
    values
}

fn build_pure_overlay(module: &MirModule) -> AeGraph {
    let mut graph = AeGraph::new();
    for (index, op) in module.operations().iter().enumerate() {
        if op.effects().bits() == 0 && is_pure_liftable(op.kind()) {
            graph.intern(Enode::new(
                op.kind().clone(),
                op.operands().to_vec(),
                OperationId::new(index as u32),
            ));
        }
    }
    graph
}

fn remove_unused_pure_operations(module: &mut MirModule) -> usize {
    let used = module
        .operations()
        .iter()
        .flat_map(|op| op.operands().iter().copied())
        .collect::<BTreeSet<_>>();
    let mut removed = 0usize;

    for block_index in 0..module.blocks().len() {
        let block = crate::mir::BlockId::new(block_index as u32);
        let retained = module
            .block(block)
            .operations()
            .iter()
            .copied()
            .filter(|op_id| {
                let op = module.operation(*op_id);
                let removable = op.effects().bits() == 0
                    && !op.results().is_empty()
                    && op.results().iter().all(|result| !used.contains(result));
                if removable {
                    removed += 1;
                }
                !removable
            })
            .collect::<Vec<_>>();
        module.set_block_operations(block, retained);
    }

    removed
}
```

- [ ] **Step 6: Run focused test**

Run:

```bash
cargo test --test mir_release release_optimizer_applies_scalar_identity_with_certificate
```

Expected: pass.

- [ ] **Step 7: Commit**

```bash
git add src/mir/ir.rs src/mir/rewrite.rs tests/mir_release.rs fixtures/mir/scalar_identity.wrela
git commit -m "feat: add certified scalar release rewrite -Codex Automated"
```

**Acceptance criteria:**
- Rewrite loop is deterministic and fuel-bounded.
- `x + 0` and `0 + x` are rewritten only when one operand is proven by MIR literal text to be exactly `0`.
- Candidate MIR verifies before the rewrite is recorded or applied.
- Applied rewrite has a valid certificate with non-empty facts.
- Certificate region is the region that actually contains the rewritten operation.
- Event log has exactly one event for the scalar identity fixture.
- Pass controls gate rewrite execution.

---

### Task 7: Release Pipeline In Perf Code And A/B JSON

**Files:**
- Modify: `src/mir/perf.rs`
- Modify: `src/command.rs`
- Test: `tests/mir_perf.rs`
- Fixture: `fixtures/perf/scalar_identity.wrela`

**Description:** Wire release optimization into `wrela perf code`, include pass/hash/certificate counters in JSON, and add the scalar fixture used for A/B evidence.

- [ ] **Step 1: Create scalar identity perf fixture**

Create `fixtures/perf/scalar_identity.wrela`:

```wrela
module perf.scalar_identity

pub class IdentityBench {
    fn run(read self) -> U32 {
        let value: U32 = 7
        let zero: U32 = 0
        let result: U32 = value + zero
        return result
    }
}
```

- [ ] **Step 2: Add failing A/B perf test**

Append to `tests/mir_perf.rs`:

```rust
#[test]
fn perf_code_accepts_release_pass_controls_and_reports_rewrites() {
    let root = fixture("scalar_identity.wrela");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "perf".to_string(),
            "code".to_string(),
            "--mode".to_string(),
            "release".to_string(),
            "--only-pass".to_string(),
            "scalar-peephole".to_string(),
            "--repeat".to_string(),
            "1".to_string(),
            "--json".to_string(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert!(code == 0 || code == 2);
    if code == 0 {
        let json = String::from_utf8(out).unwrap();
        assert!(json.contains("\"mode\":\"release\""));
        assert!(json.contains("\"passes\":[\"scalar-peephole\"]"));
        assert!(json.contains("\"rewriteCount\":1"));
        assert!(json.contains("\"rewriteEventCount\":1"));
        assert!(json.contains("\"eventsTruncated\":false"));
        assert!(json.contains("\"beforeHash\""));
        assert!(json.contains("\"afterHash\""));
    }
}
```

- [ ] **Step 3: Run test and verify failure**

Run:

```bash
cargo test --test mir_perf perf_code_accepts_release_pass_controls_and_reports_rewrites
```

Expected: fail because release optimizer data is not in perf output.

- [ ] **Step 4: Pass release controls into code perf**

Change the MIR 02 function signature in `src/mir/perf.rs`:

```rust
pub fn measure_code(
    path: &str,
    mode: PerfMode,
    repeats: usize,
    pass_set: &crate::mir::PassSet,
) -> Result<CodePerfReport, String>
```

Update the `run_perf_code` caller in `src/command.rs` to parse pass flags into a `PassSet` before calling `measure_code`.

- [ ] **Step 5: Use `optimize_release` in release perf mode**

In `src/mir/perf.rs`, select the module for codegen exactly once:

```rust
let mut optimized_release = None;
let release_report;
let module_for_codegen = match mode {
    PerfMode::Dev => {
        release_report = None;
        mir.module().expect("ok MIR has module")
    }
    PerfMode::Release => {
        let optimized = crate::mir::optimize_release(
            mir.module().expect("ok MIR has module"),
            pass_set,
        );
        release_report = Some(optimized.report().clone());
        optimized_release = Some(optimized);
        optimized_release.as_ref().expect("set above").module()
    }
};
```

Lower `module_for_codegen`, not the original module.

- [ ] **Step 6: Render release fields**

Extend JSON rendering with deterministic string building:

```rust
let pass_json = pass_set
    .names()
    .into_iter()
    .map(|name| format!("\"{name}\""))
    .collect::<Vec<_>>()
    .join(",");
let rewrite_count = release_report
    .as_ref()
    .map(|report| report.rewrites_applied())
    .unwrap_or(0);
let rewrite_event_count = release_report
    .as_ref()
    .map(|report| report.rewrite_events().len())
    .unwrap_or(0);
let events_truncated = release_report
    .as_ref()
    .map(|report| report.events_truncated())
    .unwrap_or(false);
let before_hash = release_report
    .as_ref()
    .map(|report| report.before_hash().to_string())
    .unwrap_or_else(|| "0000000000000000".to_string());
let after_hash = release_report
    .as_ref()
    .map(|report| report.after_hash().to_string())
    .unwrap_or_else(|| "0000000000000000".to_string());
```

JSON fields:

```json
"passes":["scalar-peephole"],
"rewriteCount":1,
"rewriteEventCount":1,
"eventsTruncated":false,
"beforeHash":"...",
"afterHash":"..."
```

Do not add `serde`.

- [ ] **Step 7: Run focused tests and smoke commands**

Run:

```bash
cargo test --test mir_perf perf_code_accepts_release_pass_controls_and_reports_rewrites
cargo run -- perf code --mode release --only-pass scalar-peephole --repeat 7 --json fixtures/perf/scalar_identity.wrela
cargo run -- perf code --mode release --disable-pass scalar-peephole --repeat 7 --json fixtures/perf/scalar_identity.wrela
```

Expected:
- Test passes.
- Perf commands exit `0` on supported hosts or `2` on unsupported generated-code execution hosts.
- On supported hosts, baseline and candidate checksums match before runtime deltas are interpreted.

- [ ] **Step 8: Commit**

```bash
git add src/mir/perf.rs src/command.rs tests/mir_perf.rs fixtures/perf/scalar_identity.wrela
git commit -m "feat: report certified release optimizer perf data -Codex Automated"
```

**Acceptance criteria:**
- `perf code --mode release` runs release optimization before lowering.
- JSON lists active passes in deterministic order.
- JSON reports rewrite count, rewrite-event count, truncation, and before/after hashes.
- Generated-code timing claims still require matching checksums.

---

### Task 8: MIR 03 Final Quality Gate And Phase A Handoff

**Files:**
- No production file edits unless verification reveals a bug.

**Description:** Prove MIR 03 before handoff. The handoff must include Trident evidence for the scalar rewrite.

- [ ] **Step 1: Run full local quality gate**

Run:

```bash
./scripts/quality-gate.sh
```

Expected: pass.

- [ ] **Step 2: Run release optimizer smoke commands**

Run:

```bash
cargo run -- perf code --mode release --only-pass scalar-peephole --repeat 7 --json fixtures/perf/scalar_identity.wrela
cargo run -- perf code --mode release --disable-pass scalar-peephole --repeat 7 --json fixtures/perf/scalar_identity.wrela
```

Expected: exit `0` on supported hosts or `2` with unsupported execution message elsewhere. If supported, checksums match for baseline and candidate.

- [ ] **Step 3: Record Trident evidence**

Add this table to the Phase A review packet:

```markdown
| Rule | Facts | Verifier | Events | Measurement |
|------|-------|----------|--------|-------------|
| scalar-add-zero | pure operation, literal zero, effect absent, verifier passed | passed | one bounded rewrite event | scalar_identity A/B, checksum required before timing |
```

The scalar peephole pass is marked `directional` only if each run has at least seven samples, `p90 / p10 <= 1.10`, checksums match, and median runtime improves by at least 3%. Otherwise it is marked `correctness/canonicalization evidence only in MIR 03`.

- [ ] **Step 4: Run strict gate when ready for final merge**

Run when intended implementation changes are committed:

```bash
QUALITY_GATE_STRICT_CLEAN=1 ./scripts/quality-gate.sh
```

Expected: pass only when the working tree is clean.

- [ ] **Step 5: Complete Phase A self review**

Use the repository review workflow in `docs/implementation/reviews/README.md`. Fix every finding at every priority unless the verdict documents an explicit disagreement.

**Acceptance criteria:**
- `./scripts/quality-gate.sh` passes.
- Release optimizer smoke commands produce expected supported or unsupported-host behavior.
- Phase A verdict is APPROVED in the handoff message before user handoff.
- Handoff includes certificate/event evidence for the first scalar rewrite.

## Self-Review Checklist

- [ ] Aegraph overlays are release-only.
- [ ] Pure subgraphs exclude effectful ops, state-edge ops, capacity-token ops, and trap producers.
- [ ] Stable hash V0 is deterministic, domain-separated, and documented as non-cryptographic.
- [ ] Every applied scalar rewrite has a valid versioned certificate with non-empty facts.
- [ ] Rewrite event log is bounded and exposes truncation.
- [ ] Rewrite loop is bounded by deterministic fuel.
- [ ] Pass controls are stable and tested.
- [ ] `wrela perf code` reports active passes, rewrite counters, event counters, truncation, and before/after hashes.
- [ ] A/B generated-code workflow is documented with executable commands.
- [ ] No external crates were added.
