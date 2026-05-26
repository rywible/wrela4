# MIR 05 Certified Optimization Memory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Turn certified release rewrites into durable optimization memory by adding stable identity V1, an authority algebra engine, certificate versioning, hardened measurement, bounded persistent ledger storage, certified table-loop fusion, debug/why output, auto-reduction, failure containment, and ledger-informed cost feedback.

**Architecture:** MIR 05 builds on MIR 03's certificates and MIR 04's data-plane facts. Table-loop fusion is the first high-impact rewrite, but it is allowed to land only after the durability layer exists: stable keys, versioned proof artifacts, authority queries, bounded ledger persistence, parameterized generated-code A/B measurement, reducer artifacts, and fail-closed infrastructure behavior. The compiler remains zero-dependency and stores optimizer memory as deterministic line-oriented text under `target/wrela/ledger/`.

**Tech Stack:** Rust 2024, standard library only, existing MIR/check/perf/codegen modules, handwritten hash/key serialization, handwritten ledger parser, handwritten C harness generation, handwritten reducer.

---

## Locked Decisions

- Stable identity V1 keys include `region_hash`, `target_profile`, `pass_config_hash`, `compiler_version`, and `certificate_version`.
- Certificate schema remains versioned. Stale certificates and stale ledger entries never justify a rewrite.
- The authority algebra engine owns legality facts for table-loop fusion: same table, same row count, compatible masks, disjoint mutation places, non-escaping row tokens, effect absence, independent ordering domain, state-edge preservation, and trap-order preservation.
- Generated-code data-plane runtime claims require parameterized baseline/candidate execution, matching checksums, at least seven samples per side, `p90 / p10 <= 1.10`, and at least 3% median movement. Otherwise the result is `inconclusive`.
- Persistent ledger storage is bounded to `4096` entries and `4 MiB` per target profile directory. Eviction is deterministic oldest-entry-first by recorded sequence number.
- Corrupt, stale-version, unknown-status, mismatched-key, or unparsable ledger entries are ignored and reported. They are never trusted.
- Trident infrastructure failures are fail-closed: bad certificate, bad hash, bad ledger, bad measurement, or bad reducer disables the affected optimization path for that run and preserves an artifact.
- Cost feedback is explicit and simple: a `known-winner` ledger entry can override static extraction cost only for the exact same stable region shape, target profile, pass config, compiler version, and certificate version.
- No external crate dependencies are introduced.

## Planned File Structure

```text
src/mir/hash.rs            # stable identity V1 key construction
src/mir/cert.rs            # certificate versioning, migration/invalidation behavior
src/mir/authority.rs       # authority algebra engine
src/mir/ledger.rs          # bounded line-oriented persistent optimization ledger
src/mir/hotness.rs         # static hotness seeds used in ledger keys and reports
src/mir/reduce.rs          # MIR reducer for rewrite/checksum/measurement failures
src/mir/rewrite.rs         # certified table-loop fusion and cost-feedback hook
src/mir/cost.rs            # static costs + ledger override result
src/mir/lir.rs             # data-plane loop LIR opcodes
src/mir/lower.rs           # data-plane MIR to LIR lowering
src/mir/emit.rs            # AArch64 scalar loop emission for data-plane fixture
src/mir/perf.rs            # parameterized data-plane generated-code harness and measurement classification
src/command.rs             # `wrela debug mir --why-rewrite`
tests/mir_identity.rs
tests/mir_authority.rs
tests/mir_ledger.rs
tests/mir_dataplane.rs
tests/mir_perf.rs
fixtures/perf/filter_sum.wrela
scripts/perf-compare.sh
```

## Parallel Work Map

- Task 1 must run first because stable identity V1 keys are used by certificates, ledger storage, measurement, and cost feedback.
- Task 2 depends on Task 1 and hardens certificate versioning/invalidation.
- Task 3 depends on Task 1 and owns authority algebra queries.
- Task 4 depends on Tasks 1-2 and owns persistent ledger storage.
- Task 5 depends on Tasks 1 and 4 and owns measurement hardening.
- Task 6 depends on MIR 04 data-plane IR and owns generated-code loop execution.
- Task 7 depends on Tasks 2-6 and owns certified table-loop fusion.
- Task 8 depends on Tasks 4 and 7 and owns debug/why output.
- Task 9 depends on Tasks 5 and 7 and owns reducer artifacts and Trident failure handling.
- Task 10 depends on Tasks 4, 5, and 7 and owns cost feedback.
- Task 11 runs last and owns quality gate plus Phase A handoff.
- Integration-owned shared files: `src/mir/rewrite.rs`, `src/mir/perf.rs`, `src/mir/lower.rs`, `src/mir/emit.rs`, `src/command.rs`, and `tests/mir_perf.rs`. Only one subagent edits each at a time.

---

### Task 1: Stable Identity V1

**Files:**
- Modify: `src/mir/hash.rs`
- Create: `src/mir/hotness.rs`
- Modify: `src/mir/mod.rs`
- Create: `tests/mir_identity.rs`

**Description:** Add durable optimizer identity keys for regions, pass configs, target profiles, compiler version, and certificate version. These keys are the foundation for ledger persistence and debug/why output.

- [ ] **Step 1: Add failing identity tests**

Create `tests/mir_identity.rs`:

```rust
#[test]
fn optimizer_key_changes_when_pass_config_changes() {
    let region = wrela::mir::StableHash::new(10);
    let target = wrela::mir::TargetProfile::GenericAArch64;
    let scalar = wrela::mir::PassSet::only(wrela::mir::Pass::ScalarPeephole);
    let mask = wrela::mir::PassSet::only(wrela::mir::Pass::MaskAlgebra);

    let a = wrela::mir::OptimizerKey::new(region, target, &scalar, "test-compiler", 1);
    let b = wrela::mir::OptimizerKey::new(region, target, &mask, "test-compiler", 1);

    assert_ne!(a.stable_hash(), b.stable_hash());
    assert_ne!(a.as_line(), b.as_line());
}

#[test]
fn optimizer_key_rejects_empty_compiler_version() {
    let pass_set = wrela::mir::PassSet::only(wrela::mir::Pass::ScalarPeephole);
    let key = wrela::mir::OptimizerKey::try_new(
        wrela::mir::StableHash::new(1),
        wrela::mir::TargetProfile::GenericAArch64,
        &pass_set,
        "",
        1,
    );

    assert!(key.is_err());
}

#[test]
fn static_hotness_seed_is_deterministic() {
    let hot = wrela::mir::StaticHotnessSeed::new("image.main", 3, 2);
    assert_eq!(hot.score(), 132);
    assert_eq!(hot.reason(), "root-distance=3 loop-depth=2");
}
```

- [ ] **Step 2: Run test and verify failure**

Run:

```bash
cargo test --test mir_identity
```

Expected: fail because `OptimizerKey` is not implemented.

- [ ] **Step 3: Implement optimizer key**

Append to `src/mir/hash.rs`:

```rust
use crate::mir::{PassSet, TargetProfile};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptimizerKey {
    region_hash: StableHash,
    target_profile: TargetProfile,
    pass_config_hash: StableHash,
    compiler_version: String,
    certificate_version: u16,
    stable_hash: StableHash,
}

impl OptimizerKey {
    pub fn try_new(
        region_hash: StableHash,
        target_profile: TargetProfile,
        pass_set: &PassSet,
        compiler_version: &str,
        certificate_version: u16,
    ) -> Result<Self, String> {
        if compiler_version.is_empty() {
            return Err("compiler version must not be empty".to_string());
        }
        let pass_config_hash = pass_set.stable_hash();
        let line = format!(
            "v1|region={}|target={}|passes={}|compiler={}|cert={}",
            region_hash,
            target_profile.name(),
            pass_config_hash,
            compiler_version,
            certificate_version
        );
        let stable_hash = stable_hash_text("wmir.optimizer-key.v1", &line);
        Ok(Self {
            region_hash,
            target_profile,
            pass_config_hash,
            compiler_version: compiler_version.to_string(),
            certificate_version,
            stable_hash,
        })
    }

    pub fn new(
        region_hash: StableHash,
        target_profile: TargetProfile,
        pass_set: &PassSet,
        compiler_version: &str,
        certificate_version: u16,
    ) -> Self {
        Self::try_new(region_hash, target_profile, pass_set, compiler_version, certificate_version)
            .expect("valid optimizer key")
    }

    pub fn stable_hash(&self) -> StableHash { self.stable_hash }

    pub fn as_line(&self) -> String {
        format!(
            "key={} region={} target={} passes={} compiler={} cert={}",
            self.stable_hash,
            self.region_hash,
            self.target_profile.name(),
            self.pass_config_hash,
            self.compiler_version,
            self.certificate_version
        )
    }
}
```

Export from `src/mir/mod.rs`:

```rust
pub use hash::{OptimizerKey, stable_hash_bytes, stable_hash_text, StableHash};
```

- [ ] **Step 4: Implement static hotness seed**

Create `src/mir/hotness.rs`:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StaticHotnessSeed {
    root_symbol: String,
    root_distance: u32,
    loop_depth: u32,
    score: u32,
}

impl StaticHotnessSeed {
    pub fn new(root_symbol: impl Into<String>, root_distance: u32, loop_depth: u32) -> Self {
        let score = 100u32
            .saturating_sub(root_distance.saturating_mul(8))
            .saturating_add(loop_depth.saturating_mul(28));
        Self {
            root_symbol: root_symbol.into(),
            root_distance,
            loop_depth,
            score,
        }
    }

    pub fn score(&self) -> u32 { self.score }

    pub fn reason(&self) -> String {
        format!("root-distance={} loop-depth={}", self.root_distance, self.loop_depth)
    }
}
```

Export from `src/mir/mod.rs`:

```rust
pub mod hotness;
pub use hotness::StaticHotnessSeed;
```

- [ ] **Step 5: Run focused test**

Run:

```bash
cargo test --test mir_identity
```

Expected: pass.

**Acceptance criteria:**
- Optimizer keys include region hash, target profile, pass config hash, compiler version, and certificate version.
- Empty compiler version is rejected.
- Key text is deterministic.
- Static hotness seed is deterministic and reports its reason string.

---

### Task 2: Certificate Versioning And Invalidation

**Files:**
- Modify: `src/mir/cert.rs`
- Test: unit tests in `src/mir/cert.rs`

**Description:** Harden certificate version behavior so stale certificates cannot justify new rewrites or ledger entries.

- [ ] **Step 1: Add failing invalidation test**

Append to `src/mir/cert.rs` tests:

```rust
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
```

- [ ] **Step 2: Implement invalidation methods**

In `src/mir/cert.rs`, add:

```rust
impl RewriteCertificate {
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
```

- [ ] **Step 3: Run focused tests**

Run:

```bash
cargo test mir::cert::tests::
```

Expected: pass.

**Acceptance criteria:**
- Stale certificate versions are detected by API, not by ad hoc callers.
- Ledger code can call `can_record_in_ledger()` and get fail-closed behavior.

---

### Task 3: Authority Algebra Engine

**Files:**
- Modify: `src/mir/authority.rs`
- Create: `tests/mir_authority.rs`

**Description:** Expand authority query V0 into the MIR 05 authority algebra engine for table-loop fusion legality.

- [ ] **Step 1: Add failing authority engine tests**

Create `tests/mir_authority.rs`:

```rust
#[test]
fn authority_engine_proves_table_fusion_facts() {
    let engine = wrela::mir::AuthorityEngine::new();
    let first = wrela::mir::LoopAuthorityFacts::new("packets", 256)
        .with_mask("valid")
        .with_mutation_place("acc:total")
        .with_effects(wrela::mir::EffectSet::empty())
        .with_row_token_escapes(false)
        .with_state_token("table:packets")
        .with_trap_order_index(10);
    let second = wrela::mir::LoopAuthorityFacts::new("packets", 256)
        .with_mask("valid")
        .with_mutation_place("acc:flagged")
        .with_effects(wrela::mir::EffectSet::empty())
        .with_row_token_escapes(false)
        .with_state_token("table:packets")
        .with_trap_order_index(11);

    let facts = engine.table_fusion_facts(&first, &second).unwrap();

    assert!(facts.iter().any(|fact| fact.name() == "same-table"));
    assert!(facts.iter().any(|fact| fact.name() == "same-row-count"));
    assert!(facts.iter().any(|fact| fact.name() == "compatible-masks"));
    assert!(facts.iter().any(|fact| fact.name() == "disjoint-mutation-places"));
    assert!(facts.iter().any(|fact| fact.name() == "state-edges-preserved"));
    assert!(facts.iter().any(|fact| fact.name() == "trap-order-preserved"));
}

#[test]
fn authority_engine_rejects_escaping_row_token() {
    let engine = wrela::mir::AuthorityEngine::new();
    let first = wrela::mir::LoopAuthorityFacts::new("packets", 256)
        .with_row_token_escapes(true);
    let second = wrela::mir::LoopAuthorityFacts::new("packets", 256);

    assert!(engine.table_fusion_facts(&first, &second).is_none());
}

#[test]
fn authority_engine_rejects_overlapping_mutation_places() {
    let engine = wrela::mir::AuthorityEngine::new();
    let first = wrela::mir::LoopAuthorityFacts::new("packets", 256)
        .with_mask("valid")
        .with_mutation_place("acc:total");
    let second = wrela::mir::LoopAuthorityFacts::new("packets", 256)
        .with_mask("valid")
        .with_mutation_place("acc:total");

    assert!(engine.table_fusion_facts(&first, &second).is_none());
}
```

- [ ] **Step 2: Implement authority engine types**

Append to `src/mir/authority.rs`:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NamedAuthorityFact {
    name: &'static str,
    detail: String,
}

impl NamedAuthorityFact {
    pub fn new(name: &'static str, detail: impl Into<String>) -> Self {
        Self { name, detail: detail.into() }
    }

    pub fn name(&self) -> &'static str { self.name }
    pub fn detail(&self) -> &str { &self.detail }

    pub fn as_rewrite_fact(&self) -> RewriteFact {
        RewriteFact::Authority {
            name: self.name,
            detail: self.detail.clone(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoopAuthorityFacts {
    table: String,
    rows: u64,
    mask: String,
    mutation_places: Vec<String>,
    effects: EffectSet,
    row_token_escapes: bool,
    state_token: String,
    trap_order_index: u32,
}

impl LoopAuthorityFacts {
    pub fn new(table: impl Into<String>, rows: u64) -> Self {
        Self {
            table: table.into(),
            rows,
            mask: String::new(),
            mutation_places: Vec::new(),
            effects: EffectSet::empty(),
            row_token_escapes: false,
            state_token: String::new(),
            trap_order_index: 0,
        }
    }

    pub fn with_mask(mut self, mask: impl Into<String>) -> Self {
        self.mask = mask.into();
        self
    }

    pub fn with_mutation_place(mut self, place: impl Into<String>) -> Self {
        self.mutation_places.push(place.into());
        self.mutation_places.sort();
        self.mutation_places.dedup();
        self
    }

    pub fn with_effects(mut self, effects: EffectSet) -> Self {
        self.effects = effects;
        self
    }

    pub fn with_row_token_escapes(mut self, escapes: bool) -> Self {
        self.row_token_escapes = escapes;
        self
    }

    pub fn with_state_token(mut self, token: impl Into<String>) -> Self {
        self.state_token = token.into();
        self
    }

    pub fn with_trap_order_index(mut self, index: u32) -> Self {
        self.trap_order_index = index;
        self
    }
}

#[derive(Clone, Debug, Default)]
pub struct AuthorityEngine;

impl AuthorityEngine {
    pub fn new() -> Self {
        Self
    }

    pub fn table_fusion_facts(
        &self,
        first: &LoopAuthorityFacts,
        second: &LoopAuthorityFacts,
    ) -> Option<Vec<NamedAuthorityFact>> {
        if first.table != second.table || first.rows != second.rows {
            return None;
        }
        if first.mask != second.mask {
            return None;
        }
        if first
            .mutation_places
            .iter()
            .any(|place| second.mutation_places.contains(place))
        {
            return None;
        }
        if !first.effects.is_empty() || !second.effects.is_empty() {
            return None;
        }
        if first.row_token_escapes || second.row_token_escapes {
            return None;
        }
        if first.state_token != second.state_token {
            return None;
        }
        if first.trap_order_index > second.trap_order_index {
            return None;
        }
        Some(vec![
            NamedAuthorityFact::new("same-table", first.table.clone()),
            NamedAuthorityFact::new("same-row-count", first.rows.to_string()),
            NamedAuthorityFact::new("compatible-masks", first.mask.clone()),
            NamedAuthorityFact::new(
                "disjoint-mutation-places",
                format!("{:?}|{:?}", first.mutation_places, second.mutation_places),
            ),
            NamedAuthorityFact::new("effect-absent", "all"),
            NamedAuthorityFact::new("row-token-non-escaping", first.table.clone()),
            NamedAuthorityFact::new("state-edges-preserved", first.state_token.clone()),
            NamedAuthorityFact::new("trap-order-preserved", format!("{}..{}", first.trap_order_index, second.trap_order_index)),
            NamedAuthorityFact::new("independent-ordering-domain", first.table.clone()),
        ])
    }
}
```

Export from `src/mir/mod.rs`:

```rust
pub use authority::{AuthorityEngine, LoopAuthorityFacts, NamedAuthorityFact};
```

- [ ] **Step 3: Run focused tests**

Run:

```bash
cargo test --test mir_authority
```

Expected: pass.

**Acceptance criteria:**
- Table-loop fusion legality is queried from `AuthorityEngine`.
- Compatible masks and disjoint mutation places are represented as authority facts.
- Escaping row tokens, mismatched table/rows, incompatible masks, overlapping mutation places, non-empty effects, changed state token, and reversed trap order all reject fusion.
- Returned facts can be copied into certificates.

---

### Task 4: Bounded Persistent Ledger Storage

**Files:**
- Create: `src/mir/ledger.rs`
- Modify: `src/mir/mod.rs`
- Create: `tests/mir_ledger.rs`

**Description:** Add bounded persistent optimizer memory as deterministic line-oriented text under `target/wrela/ledger/`. The ledger records known winners, known losers, and entries requiring remeasurement.

- [ ] **Step 1: Add failing ledger tests**

Create `tests/mir_ledger.rs`:

```rust
#[test]
fn ledger_round_trips_known_winner() {
    let dir = std::env::temp_dir().join(format!("wrela-ledger-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let pass_set = wrela::mir::PassSet::only(wrela::mir::Pass::MaskAlgebra);
    let key = wrela::mir::OptimizerKey::new(
        wrela::mir::StableHash::new(99),
        wrela::mir::TargetProfile::GenericAArch64,
        &pass_set,
        "test-compiler",
        wrela::mir::CERTIFICATE_VERSION,
    );
    let mut ledger = wrela::mir::OptimizationLedger::open(
        dir.clone(),
        wrela::mir::TargetProfile::GenericAArch64,
    ).unwrap();
    ledger.record(wrela::mir::LedgerEntry::known_winner(key.clone(), 7, -12, 104)).unwrap();
    ledger.flush().unwrap();

    let loaded = wrela::mir::OptimizationLedger::open(
        dir,
        wrela::mir::TargetProfile::GenericAArch64,
    ).unwrap();
    let entry = loaded.lookup(&key).unwrap();
    assert_eq!(entry.status(), wrela::mir::LedgerStatus::KnownWinner);
    assert_eq!(entry.runtime_delta_percent(), -12);
}

#[test]
fn ledger_ignores_corrupt_lines() {
    let dir = std::env::temp_dir().join(format!("wrela-ledger-corrupt-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("generic-aarch64.ledger"), "not a ledger line\n").unwrap();

    let ledger = wrela::mir::OptimizationLedger::open(
        dir,
        wrela::mir::TargetProfile::GenericAArch64,
    ).unwrap();
    assert_eq!(ledger.corrupt_lines(), 1);
}
```

- [ ] **Step 2: Implement ledger types**

Create `src/mir/ledger.rs`:

```rust
use std::{collections::BTreeMap, fs, path::PathBuf};

use super::{OptimizerKey, StableHash, TargetProfile};

pub const LEDGER_MAX_ENTRIES: usize = 4096;
pub const LEDGER_MAX_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LedgerStatus {
    KnownWinner,
    KnownLoser,
    NeedsRemeasure,
}

impl LedgerStatus {
    fn as_str(self) -> &'static str {
        match self {
            LedgerStatus::KnownWinner => "known-winner",
            LedgerStatus::KnownLoser => "known-loser",
            LedgerStatus::NeedsRemeasure => "needs-remeasure",
        }
    }

    fn from_str(text: &str) -> Option<Self> {
        match text {
            "known-winner" => Some(LedgerStatus::KnownWinner),
            "known-loser" => Some(LedgerStatus::KnownLoser),
            "needs-remeasure" => Some(LedgerStatus::NeedsRemeasure),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LedgerEntry {
    key_line: String,
    key_hash: StableHash,
    status: LedgerStatus,
    samples: u32,
    runtime_delta_percent: i32,
    code_size_delta_bytes: i32,
    sequence: u64,
}

impl LedgerEntry {
    pub fn known_winner(
        key: OptimizerKey,
        samples: u32,
        runtime_delta_percent: i32,
        code_size_delta_bytes: i32,
    ) -> Self {
        Self {
            key_line: key.as_line(),
            key_hash: key.stable_hash(),
            status: LedgerStatus::KnownWinner,
            samples,
            runtime_delta_percent,
            code_size_delta_bytes,
            sequence: 0,
        }
    }

    pub fn status(&self) -> LedgerStatus { self.status }
    pub fn runtime_delta_percent(&self) -> i32 { self.runtime_delta_percent }

    fn to_line(&self) -> String {
        let escaped_key_line = self.key_line.replacen("key=", "key_text=", 1);
        format!(
            "v1\tseq={}\tkey={}\tstatus={}\tsamples={}\truntime_delta_percent={}\tcode_size_delta_bytes={}\t{}",
            self.sequence,
            self.key_hash,
            self.status.as_str(),
            self.samples,
            self.runtime_delta_percent,
            self.code_size_delta_bytes,
            escaped_key_line
        )
    }
}

pub struct OptimizationLedger {
    dir: PathBuf,
    target_profile: TargetProfile,
    entries: BTreeMap<StableHash, LedgerEntry>,
    corrupt_lines: usize,
    next_sequence: u64,
}

impl OptimizationLedger {
    pub fn open(dir: PathBuf, target_profile: TargetProfile) -> Result<Self, String> {
        fs::create_dir_all(&dir).map_err(|err| err.to_string())?;
        let file = dir.join(format!("{}.ledger", target_profile.name()));
        let mut entries = BTreeMap::new();
        let mut corrupt_lines = 0usize;
        let mut next_sequence = 1u64;
        if let Ok(text) = fs::read_to_string(&file) {
            for line in text.lines() {
                match parse_line(line) {
                    Some(entry) => {
                        next_sequence = next_sequence.max(entry.sequence + 1);
                        entries.insert(entry.key_hash, entry);
                    }
                    None => corrupt_lines += 1,
                }
            }
        }
        Ok(Self { dir, target_profile, entries, corrupt_lines, next_sequence })
    }

    pub fn record(&mut self, mut entry: LedgerEntry) -> Result<(), String> {
        entry.sequence = self.next_sequence;
        self.next_sequence += 1;
        self.entries.insert(entry.key_hash, entry);
        while self.entries.len() > LEDGER_MAX_ENTRIES {
            let oldest = self.entries
                .iter()
                .min_by_key(|(_, entry)| entry.sequence)
                .map(|(key, _)| *key)
                .expect("non-empty ledger");
            self.entries.remove(&oldest);
        }
        Ok(())
    }

    pub fn lookup(&self, key: &OptimizerKey) -> Option<&LedgerEntry> {
        self.entries.get(&key.stable_hash())
    }

    pub fn corrupt_lines(&self) -> usize { self.corrupt_lines }

    pub fn flush(&self) -> Result<(), String> {
        let file = self.dir.join(format!("{}.ledger", self.target_profile.name()));
        let mut text = String::new();
        for entry in self.entries.values() {
            text.push_str(&entry.to_line());
            text.push('\n');
        }
        if text.len() as u64 > LEDGER_MAX_BYTES {
            return Err("ledger exceeds byte budget".to_string());
        }
        fs::write(file, text).map_err(|err| err.to_string())
    }
}

fn parse_line(line: &str) -> Option<LedgerEntry> {
    if !line.starts_with("v1\t") {
        return None;
    }
    let mut sequence = None;
    let mut key_hash = None;
    let mut status = None;
    let mut samples = None;
    let mut runtime_delta_percent = None;
    let mut code_size_delta_bytes = None;
    let mut key_line = None;
    for part in line.split('\t').skip(1) {
        let (name, value) = part.split_once('=')?;
        match name {
            "seq" => sequence = value.parse::<u64>().ok(),
            "key" => key_hash = u64::from_str_radix(value, 16).ok().map(StableHash::new),
            "key_text" => key_line = Some(format!("key={value}")),
            "status" => status = LedgerStatus::from_str(value),
            "samples" => samples = value.parse::<u32>().ok(),
            "runtime_delta_percent" => runtime_delta_percent = value.parse::<i32>().ok(),
            "code_size_delta_bytes" => code_size_delta_bytes = value.parse::<i32>().ok(),
            _ => {}
        }
    }
    Some(LedgerEntry {
        key_line: key_line?,
        key_hash: key_hash?,
        status: status?,
        samples: samples?,
        runtime_delta_percent: runtime_delta_percent?,
        code_size_delta_bytes: code_size_delta_bytes?,
        sequence: sequence?,
    })
}
```

Export from `src/mir/mod.rs`:

```rust
pub mod ledger;
pub use ledger::{LedgerEntry, LedgerStatus, OptimizationLedger, LEDGER_MAX_BYTES, LEDGER_MAX_ENTRIES};
```

- [ ] **Step 3: Run focused tests**

Run:

```bash
cargo test --test mir_ledger
```

Expected: pass.

**Acceptance criteria:**
- Ledger persists known-winner entries.
- Corrupt lines are counted and ignored.
- Ledger has entry-count and byte-count bounds.
- Ledger files are per target profile, named `{target_profile.name()}.ledger`.
- Embedded optimizer-key text is written as `key_text=...` so it cannot clobber the primary `key=<hex>` field during parsing.
- Storage format is deterministic line-oriented text.

---

### Task 5: Measurement Hardening

**Files:**
- Modify: `src/mir/perf.rs`
- Modify: `src/mir/ledger.rs`
- Test: `tests/mir_perf.rs`

**Description:** Add measurement classification for generated-code A/B runs: stable win, stable loss, neutral, inconclusive, or checksum mismatch.

- [ ] **Step 1: Add failing measurement tests**

Append to `tests/mir_perf.rs`:

```rust
#[test]
fn measurement_classifies_inconclusive_noise() {
    let baseline = wrela::mir::SampleSet::new(vec![100, 101, 102, 140, 141, 142, 143]).unwrap();
    let candidate = wrela::mir::SampleSet::new(vec![99, 100, 101, 139, 140, 141, 142]).unwrap();
    let result = wrela::mir::classify_measurement(1, 1, &baseline, &candidate);

    assert_eq!(result.classification(), wrela::mir::MeasurementClassification::Inconclusive);
}

#[test]
fn measurement_classifies_stable_win() {
    let baseline = wrela::mir::SampleSet::new(vec![100, 101, 102, 103, 104, 105, 106]).unwrap();
    let candidate = wrela::mir::SampleSet::new(vec![90, 91, 92, 93, 94, 95, 96]).unwrap();
    let result = wrela::mir::classify_measurement(7, 7, &baseline, &candidate);

    assert_eq!(result.classification(), wrela::mir::MeasurementClassification::StableWin);
}
```

- [ ] **Step 2: Implement sample and classification types**

In `src/mir/perf.rs`, add:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SampleSet {
    samples: Vec<u128>,
}

impl SampleSet {
    pub fn new(mut samples: Vec<u128>) -> Result<Self, String> {
        if samples.is_empty() {
            return Err("sample set must not be empty".to_string());
        }
        samples.sort_unstable();
        Ok(Self { samples })
    }

    pub fn len(&self) -> usize { self.samples.len() }
    pub fn median(&self) -> u128 { self.samples[self.samples.len() / 2] }
    pub fn p10(&self) -> u128 { self.samples[self.samples.len() / 10] }
    pub fn p90(&self) -> u128 { self.samples[(self.samples.len() * 9) / 10] }

    pub fn stable_noise(&self) -> bool {
        self.p10() > 0 && self.p90() * 100 <= self.p10() * 110
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeasurementClassification {
    StableWin,
    StableLoss,
    Neutral,
    Inconclusive,
    ChecksumMismatch,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeasurementResult {
    classification: MeasurementClassification,
    median_delta_percent: i32,
}

impl MeasurementResult {
    pub fn classification(&self) -> MeasurementClassification { self.classification }
    pub fn median_delta_percent(&self) -> i32 { self.median_delta_percent }
}

pub fn classify_measurement(
    baseline_checksum: u64,
    candidate_checksum: u64,
    baseline: &SampleSet,
    candidate: &SampleSet,
) -> MeasurementResult {
    if baseline_checksum != candidate_checksum {
        return MeasurementResult {
            classification: MeasurementClassification::ChecksumMismatch,
            median_delta_percent: 0,
        };
    }
    if baseline.len() < 7 || candidate.len() < 7 || !baseline.stable_noise() || !candidate.stable_noise() {
        return MeasurementResult {
            classification: MeasurementClassification::Inconclusive,
            median_delta_percent: 0,
        };
    }
    let base = baseline.median() as i128;
    let cand = candidate.median() as i128;
    let delta = (((cand - base) * 100) / base) as i32;
    let classification = if delta <= -3 {
        MeasurementClassification::StableWin
    } else if delta >= 3 {
        MeasurementClassification::StableLoss
    } else {
        MeasurementClassification::Neutral
    };
    MeasurementResult {
        classification,
        median_delta_percent: delta,
    }
}
```

Export from `src/mir/mod.rs`:

```rust
pub use perf::{classify_measurement, MeasurementClassification, MeasurementResult, SampleSet};
```

- [ ] **Step 3: Record measurement classifications in the ledger**

In `src/mir/perf.rs`, after an A/B generated-code run has matching benchmark identity and a computed `MeasurementResult`, record the result when a ledger directory is configured:

```rust
fn record_measurement_in_ledger(
    ledger: &mut OptimizationLedger,
    key: OptimizerKey,
    baseline_samples: &SampleSet,
    result: &MeasurementResult,
    code_size_delta_bytes: i32,
) -> Result<(), String> {
    let samples = baseline_samples.len() as u32;
    let entry = match result.classification() {
        MeasurementClassification::StableWin => {
            LedgerEntry::known_winner(
                key,
                samples,
                result.median_delta_percent(),
                code_size_delta_bytes,
            )
        }
        MeasurementClassification::StableLoss => {
            LedgerEntry::known_loser(
                key,
                samples,
                result.median_delta_percent(),
                code_size_delta_bytes,
            )
        }
        MeasurementClassification::Neutral | MeasurementClassification::Inconclusive => {
            LedgerEntry::needs_remeasure(
                key,
                samples,
                result.median_delta_percent(),
                code_size_delta_bytes,
            )
        }
        MeasurementClassification::ChecksumMismatch => {
            return Err("checksum mismatch cannot be recorded as optimization evidence".to_string());
        }
    };
    ledger.record(entry)?;
    ledger.flush()
}
```

Add constructors in `src/mir/ledger.rs`:

```rust
impl LedgerEntry {
    pub fn known_loser(
        key: OptimizerKey,
        samples: u32,
        runtime_delta_percent: i32,
        code_size_delta_bytes: i32,
    ) -> Self {
        Self::new(key, LedgerStatus::KnownLoser, samples, runtime_delta_percent, code_size_delta_bytes)
    }

    pub fn needs_remeasure(
        key: OptimizerKey,
        samples: u32,
        runtime_delta_percent: i32,
        code_size_delta_bytes: i32,
    ) -> Self {
        Self::new(key, LedgerStatus::NeedsRemeasure, samples, runtime_delta_percent, code_size_delta_bytes)
    }

    fn new(
        key: OptimizerKey,
        status: LedgerStatus,
        samples: u32,
        runtime_delta_percent: i32,
        code_size_delta_bytes: i32,
    ) -> Self {
        Self {
            key_line: key.as_line(),
            key_hash: key.stable_hash(),
            status,
            samples,
            runtime_delta_percent,
            code_size_delta_bytes,
            sequence: 0,
        }
    }
}
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test --test mir_perf measurement_classifies_
```

Expected: pass.

**Acceptance criteria:**
- Measurement classification enforces checksum agreement.
- Fewer than seven samples is inconclusive.
- `p90 / p10 > 1.10` is inconclusive.
- Stable win/loss requires at least 3% median movement.
- Stable wins, stable losses, neutral results, and inconclusive results have a ledger-recording path.
- Checksum mismatches are not recorded as optimization evidence.

---

### Task 6: Data-Plane Generated-Code Harness

**Files:**
- Modify: `src/mir/lir.rs`
- Modify: `src/mir/lower.rs`
- Modify: `src/mir/emit.rs`
- Modify: `src/mir/regalloc.rs`
- Modify: `src/mir/perf.rs`
- Test: `tests/mir_perf.rs`
- Fixture: `fixtures/perf/filter_sum.wrela`

**Description:** Add the scalar AArch64 generated-code path needed to execute the MIR 04 data-plane fixture. This is intentionally narrow: AoS `Packet { len: U32, flags: U32 }` tables, mask bits, U64 sums, and deterministic C harness inputs for the exact five-parameter Wrela signature.

- [ ] **Step 1: Add failing generated-code test**

Append to `tests/mir_perf.rs`:

```rust
#[test]
fn perf_code_runs_filter_sum_dataplane_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/perf/filter_sum.wrela");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = wrela::command::run_with_io(
        vec![
            "wrela".into(),
            "perf".into(),
            "code".into(),
            "--mode".into(),
            "release".into(),
            "--only-pass".into(),
            "mask-algebra".into(),
            "--repeat".into(),
            "7".into(),
            "--json".into(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    if cfg!(target_arch = "aarch64") {
        assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&err));
        let json = String::from_utf8(out).unwrap();
        assert!(json.contains("\"checksum\""));
        assert!(json.contains("\"expectedChecksum\""));
        assert!(json.contains("\"checksumMatched\":true"));
        assert!(json.contains("\"sampleCount\":7"));
    } else {
        assert_eq!(code, 2);
        assert!(String::from_utf8(err).unwrap().contains("unsupported generated-code execution host"));
    }
}
```

- [ ] **Step 2: Add LIR operations**

In `src/mir/lir.rs`, add:

```rust
pub enum LirOpcode {
    // existing opcodes...
    LoadU32,
    LoadMaskByte,
    TestMaskBit,
    AddU64,
    BranchIfZero,
    BranchIfLessThan,
}
```

Verifier rules:

```text
LoadU32 reads from a table base pointer plus byte offset.
LoadMaskByte reads from a mask pointer plus byte offset.
TestMaskBit produces a Bool-like integer flag.
AddU64 consumes and produces U64-class values.
BranchIfLessThan compares loop index and row count.
BranchIfZero and BranchIfLessThan targets must name existing LIR blocks.
Every data-plane loop must have one entry block, one active-row block, one next-row block, and one exit block.
```

- [ ] **Step 3: Lower supported data-plane MIR using the fixture ABI**

The supported MIR 05 fixture ABI is:

```text
wrela_run(
    self: *const u8,
    packets: *const Packet,      // Packet = { len: u32, flags: u32 }, stride 8
    valid: *const u8,            // 256-bit mask, 32 bytes
    small: *const Packet,        // 128 rows, stride 8
    small_valid: *const u8,      // 128-bit mask, 16 bytes
) -> u64
```

In `src/mir/lower.rs`, lower each supported `ReduceRows` to an AoS scalar loop:

```text
i = 0
acc = 0
loop:
  mask_byte = load_mask_byte(mask_ptr, i >> 3)
  active = test_mask_bit(mask_byte, i & 7)
  if active == 0 goto next
  field_value = load_u32(table_ptr, i * 8 + field_offset)
  acc = add_u64(acc, zext(field_value))
next:
  i = i + 1
  if i < rows goto loop
```

Supported field offsets are:

```text
len   -> 0
flags -> 4
```

Supported reductions are:

```text
ReduceRows { table: "packets", mask: "valid", field: "len", rows: 256 }
ReduceRows { table: "packets", mask: "valid", field: "flags", rows: 256 }
ReduceRows { table: "small", mask: "small_valid", field: "len", rows: 128 }
```

The lowered function returns `packets_len_sum + packets_flags_sum + small_len_sum`. Unsupported table names, mask names, fields, row counts, or layouts return a structured lowering error `unsupported data-plane ABI`.

- [ ] **Step 4: Lower fused reduce pairs into one loop**

When lowering a `ReduceRows` operation, inspect the operation facts added by Task 7:

```rust
fn lower_reduce_rows_or_fused_pair(
    lower: &mut LowerContext,
    module: &MirModule,
    op_id: OperationId,
) -> Result<(), LowerError> {
    let op = module.operation(op_id);
    if op.facts().fused_into().is_some() {
        return Ok(());
    }
    if let Some(second_id) = op.facts().fused_with() {
        let second = module.operation(second_id);
        lower_two_reduce_rows_in_one_loop(lower, op, second)
    } else {
        lower_one_reduce_rows_loop(lower, op)
    }
}
```

`lower_two_reduce_rows_in_one_loop` emits one mask-byte load, one mask-bit test, and two field loads/adds inside the active-row block:

```text
i = 0
acc0 = 0
acc1 = 0
loop:
  mask_byte = load_mask_byte(mask_ptr, i >> 3)
  active = test_mask_bit(mask_byte, i & 7)
  if active == 0 goto next
  value0 = load_u32(table_ptr, i * 8 + field0_offset)
  value1 = load_u32(table_ptr, i * 8 + field1_offset)
  acc0 = add_u64(acc0, zext(value0))
  acc1 = add_u64(acc1, zext(value1))
next:
  i = i + 1
  if i < rows goto loop
```

The successor reduction with `fused_into = Some(first)` emits no loop. Its result value is bound to the second accumulator produced by the fused loop. This is the codegen behavior that turns the certificate marker into a real optimization.

- [ ] **Step 5: Emit AArch64 scalar loop**

In `src/mir/emit.rs`, emit AArch64 using:

```asm
ldrb  wMask, [xMask, xByteIndex]
tst   wMask, wBit
b.eq  .next
add   xAddr, xTable, xIndex, lsl #3
ldr   wValue, [xAddr, #field_offset]
add   xAcc, xAcc, xValue
```

For fused pairs, emit:

```asm
ldrb  wMask, [xMask, xByteIndex]
tst   wMask, wBit
b.eq  .next
add   xAddr, xTable, xIndex, lsl #3
ldr   wValue0, [xAddr, #field0_offset]
ldr   wValue1, [xAddr, #field1_offset]
add   xAcc0, xAcc0, xValue0
add   xAcc1, xAcc1, xValue1
```

Use caller-provided pointers from the generated C harness. Keep register allocation fail-hard: if the narrow loop runs out of registers, return a codegen error instead of spilling silently.

- [ ] **Step 6: Generate deterministic C harness inputs**

In `src/mir/perf.rs`, for `fixtures/perf/filter_sum.wrela`, generate an AoS harness that matches the Wrela signature:

```c
typedef struct Packet {
  uint32_t len;
  uint32_t flags;
} Packet;

static uint8_t self_object[1];
static Packet packets[256];
static uint8_t valid_mask[32];
static Packet small[128];
static uint8_t small_valid_mask[16];

static void init(void) {
  for (uint32_t i = 0; i < 256; i++) {
    packets[i].len = i + 1;
    packets[i].flags = i ^ 0x55u;
    if ((i % 3) != 0) valid_mask[i >> 3] |= (uint8_t)(1u << (i & 7));
  }
  for (uint32_t i = 0; i < 128; i++) {
    small[i].len = i + 5;
    small[i].flags = i ^ 0x33u;
    if ((i % 4) != 1) small_valid_mask[i >> 3] |= (uint8_t)(1u << (i & 7));
  }
}

static uint64_t expected_checksum(void) {
  uint64_t total = 0;
  uint64_t flagged = 0;
  uint64_t small_total = 0;
  for (uint32_t i = 0; i < 256; i++) {
    if ((valid_mask[i >> 3] & (uint8_t)(1u << (i & 7))) != 0) {
      total += packets[i].len;
      flagged += packets[i].flags;
    }
  }
  for (uint32_t i = 0; i < 128; i++) {
    if ((small_valid_mask[i >> 3] & (uint8_t)(1u << (i & 7))) != 0) {
      small_total += small[i].len;
    }
  }
  return total + flagged + small_total;
}
```

Checksum is:

```c
uint64_t expected = expected_checksum();
uint64_t checksum = wrela_run(self_object, packets, valid_mask, small, small_valid_mask);
printf("%llu %llu\n", (unsigned long long)checksum, (unsigned long long)expected);
```

`wrela perf code` parses both integers, reports `checksum`, `expectedChecksum`, and `checksumMatched`, and refuses to classify timing when they differ.

- [ ] **Step 7: Run focused test**

Run:

```bash
cargo test --test mir_perf perf_code_runs_filter_sum_dataplane_fixture
```

Expected: pass on supported AArch64 hosts. On non-AArch64 hosts the test must use the existing unsupported-host path and assert exit `2`; do not emit invented runtime numbers.

**Acceptance criteria:**
- Data-plane fixture executes on supported AArch64 hosts.
- Unsupported hosts return exit `2` with existing unsupported-host behavior.
- Checksums are deterministic.
- Fused reduce facts change generated code by emitting one loop with two accumulators instead of two separate loops.
- The old MIR 04 `data-plane generated-code execution is MIR 05 work` message no longer appears for `filter_sum.wrela`.

---

### Task 7: Certified Table Loop Fusion

**Files:**
- Modify: `src/mir/pass_control.rs`
- Modify: `src/mir/cert.rs`
- Modify: `src/mir/facts.rs`
- Modify: `src/mir/ir.rs`
- Modify: `src/mir/rewrite.rs`
- Test: `tests/mir_dataplane.rs`

**Description:** Add the first high-impact certified data-plane rewrite: fuse adjacent compatible row reductions over the same table. The rewrite uses authority algebra facts and emits a certificate before it is recorded or applied.

- [ ] **Step 1: Add failing fusion tests**

Append to `tests/mir_dataplane.rs`:

```rust
#[test]
fn table_loop_fusion_emits_authority_certificate() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/perf/filter_sum.wrela");
    let check = wrela::check::check_root(root);
    assert!(check.ok(), "{:?}", check.diagnostics());
    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok(), "{:?}", mir.diagnostics());

    let passes = wrela::mir::PassSet::only(wrela::mir::Pass::TableLoopFusion);
    let optimized = wrela::mir::optimize_release(mir.module().unwrap(), &passes);

    assert!(optimized.report().rewrites_applied() >= 1);
    let cert = optimized.report().rewrite_events()[0].certificate();
    assert_eq!(cert.rule().name(), "table-loop-fusion");
    assert!(cert.facts().iter().any(|fact| format!("{fact:?}").contains("same-table")));
    assert!(cert.facts().iter().any(|fact| format!("{fact:?}").contains("state-edges-preserved")));
    assert!(cert.facts().iter().any(|fact| format!("{fact:?}").contains("trap-order-preserved")));
    assert!(wrela::mir::verify_module(optimized.module()).ok());
}
```

- [ ] **Step 2: Add pass and rule**

Extend `Pass`:

```rust
TableLoopFusion,
```

Pass name:

```rust
Pass::TableLoopFusion => "table-loop-fusion",
```

Extend `RewriteRule`:

```rust
TableLoopFusion,
```

Rule name:

```rust
RewriteRule::TableLoopFusion => "table-loop-fusion",
```

- [ ] **Step 3: Extract loop facts**

In `src/mir/rewrite.rs`, add:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
struct ReduceFacts {
    op: OperationId,
    table: String,
    mask: String,
    field: String,
    rows: u64,
    authority: crate::mir::LoopAuthorityFacts,
}

fn reduce_facts(module: &MirModule) -> Vec<ReduceFacts> {
    let mut facts = Vec::new();
    for (index, op) in module.operations().iter().enumerate() {
        if let OperationKind::ReduceRows { table, mask, field, rows } = op.kind() {
            facts.push(ReduceFacts {
                op: OperationId::new(index as u32),
                table: table.clone(),
                mask: mask.clone(),
                field: field.clone(),
                rows: *rows,
                authority: crate::mir::LoopAuthorityFacts::new(table.clone(), *rows)
                    .with_mask(mask.clone())
                    .with_mutation_place(format!("acc:{field}"))
                    .with_effects(op.effects())
                    .with_row_token_escapes(false)
                    .with_state_token(format!("table:{table}"))
                    .with_trap_order_index(index as u32),
            });
        }
    }
    facts
}
```

- [ ] **Step 4: Implement certified fusion**

In `src/mir/rewrite.rs`, add `apply_table_loop_fusion`:

```rust
fn apply_table_loop_fusion(
    module: &mut MirModule,
    report: &mut ReleaseOptimizeReport,
    fuel: &mut usize,
) {
    let engine = crate::mir::AuthorityEngine::new();
    let facts = reduce_facts(module);
    for pair in facts.windows(2) {
        if *fuel == 0 || report.events_truncated() {
            break;
        }
        let first = &pair[0];
        let second = &pair[1];
        let Some(authority_facts) = engine.table_fusion_facts(&first.authority, &second.authority) else {
            continue;
        };
        let before_hash = stable_hash_text("wmir.module.v0", &crate::mir::text::render_module(module));
        let mut candidate = module.clone();
        candidate.mark_reduce_pair_fused(first.op, second.op);
        let Some(region_id) = containing_region(module, first.op) else {
            continue;
        };
        let verify = crate::mir::verify_module(&candidate);
        let after_hash = stable_hash_text("wmir.module.v0", &crate::mir::text::render_module(&candidate));
        let outcome = if verify.ok() {
            RewriteOutcome::PassedVerifier
        } else {
            RewriteOutcome::FailedVerifier
        };
        let mut cert_facts = authority_facts
            .iter()
            .map(|fact| fact.as_rewrite_fact())
            .collect::<Vec<_>>();
        cert_facts.push(RewriteFact::VerifierPassed);
        let cert = RewriteCertificate::new(
            CERTIFICATE_VERSION,
            RewriteRule::TableLoopFusion,
            crate::mir::Pass::TableLoopFusion,
            region_id,
            vec![first.op, second.op],
            before_hash,
            after_hash,
            cert_facts,
            outcome,
        );
        if !cert.can_record_in_ledger() {
            continue;
        }
        if !report.events.push(cert) {
            continue;
        }
        *module = candidate;
        report.rewrites_applied += 1;
        report.fuel_used += 1;
        *fuel -= 1;
    }
}
```

Reuse the `containing_region(module, op_id)` helper introduced in MIR 03 so the certificate points at the actual region containing the fused reductions.

Add fused-reduce facts in `src/mir/facts.rs`:

```rust
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
}
```

Add mutable accessors in `src/mir/ir.rs`:

```rust
impl OperationData {
    pub fn facts_mut(&mut self) -> &mut OperationFacts {
        &mut self.facts
    }
}

impl MirModule {
    pub fn operation_mut(&mut self, id: OperationId) -> &mut OperationData {
        &mut self.operations[id.raw() as usize]
    }
}
```

Then add `mark_reduce_pair_fused` in `src/mir/ir.rs`:

```rust
impl MirModule {
    pub fn mark_reduce_pair_fused(
        &mut self,
        first: OperationId,
        second: OperationId,
    ) {
        self.operation_mut(first).facts_mut().mark_fused_with(second);
        self.operation_mut(second).facts_mut().mark_fused_into(first);
    }
}
```

The fused marker is consumed by MIR 05 lowering to emit one loop body that computes both reductions. It is not a comment or report-only flag; it changes the lowering contract.

- [ ] **Step 5: Run focused test**

Run:

```bash
cargo test --test mir_dataplane table_loop_fusion_emits_authority_certificate
```

Expected: pass.

**Acceptance criteria:**
- Fusion requires authority facts from `AuthorityEngine`.
- Fusion emits a valid certificate with state-edge and trap-order facts.
- Fused MIR verifies.
- The fused marker changes lowering behavior for the data-plane generated-code path.

---

### Task 8: Debug Why-Rewrite Output

**Files:**
- Modify: `src/command.rs`
- Modify: `src/mir/rewrite.rs`
- Test: `tests/mir_dataplane.rs`

**Description:** Add a human-readable way to inspect rewrite certificates from the CLI.

- [ ] **Step 1: Add failing debug CLI test**

Append to `tests/mir_dataplane.rs`:

```rust
#[test]
fn debug_mir_why_rewrite_prints_certificate_facts() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/perf/filter_sum.wrela");
    let mut out = Vec::new();
    let mut err = Vec::new();
    let code = wrela::command::run_with_io(
        vec![
            "wrela".into(),
            "debug".into(),
            "mir".into(),
            "--why-rewrite".into(),
            "0".into(),
            "--only-pass".into(),
            "table-loop-fusion".into(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 0, "stderr: {}", String::from_utf8_lossy(&err));
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("rule: table-loop-fusion"));
    assert!(text.contains("same-table"));
    assert!(text.contains("before:"));
    assert!(text.contains("after:"));
}
```

- [ ] **Step 2: Implement CLI command**

In `src/command.rs`, add:

```text
wrela debug mir --why-rewrite <index> [pass flags] <root.wrela>
```

Implementation steps:

```rust
let check = crate::check::check_root(root);
let mir = crate::mir::build_mir(&check);
let optimized = crate::mir::optimize_release(mir.module().expect("ok MIR has module"), &pass_set);
let Some(event) = optimized.report().rewrite_events().get(index) else {
    writeln!(err, "rewrite event index out of range")?;
    return 2;
};
let cert = event.certificate();
writeln!(out, "rule: {}", cert.rule().name())?;
writeln!(out, "pass: {}", cert.pass().name())?;
writeln!(out, "before: {}", cert.before_hash())?;
writeln!(out, "after: {}", cert.after_hash())?;
for fact in cert.facts() {
    writeln!(out, "fact: {:?}", fact)?;
}
```

- [ ] **Step 3: Run focused test**

Run:

```bash
cargo test --test mir_dataplane debug_mir_why_rewrite_prints_certificate_facts
```

Expected: pass.

**Acceptance criteria:**
- `debug mir --why-rewrite` reruns release optimization deterministically and prints the selected certificate.
- Unknown rewrite index exits `2`.
- Output includes rule, pass, before hash, after hash, and facts.

---

### Task 9: Auto-Reducer And Trident Failure Handling

**Files:**
- Create: `src/mir/reduce.rs`
- Modify: `src/mir/mod.rs`
- Modify: `src/mir/rewrite.rs`
- Test: unit tests in `src/mir/reduce.rs`

**Description:** Add the first reducer path for rewrite verifier failures, checksum mismatches, measurement sanity failures, and Trident infrastructure failures. The reducer writes deterministic W-MIR text artifacts under `target/wrela/reductions/`.

- [ ] **Step 1: Add failing reducer test**

Add this unit test to `src/mir/reduce.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::ReductionRequest;

    #[test]
    fn reducer_writes_artifact_for_failed_rewrite() {
        let module = crate::mir::dataplane::testing::mask_and_true_module("packets", 256);
        let dir = std::env::temp_dir().join(format!("wrela-reductions-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);

        let artifact = ReductionRequest::new(
            "rewrite-verifier-failure",
            crate::mir::StableHash::new(42),
            module,
        )
        .write_to(&dir)
        .unwrap();

        let text = std::fs::read_to_string(artifact).unwrap();
        assert!(text.contains("failure=rewrite-verifier-failure"));
        assert!(text.contains("hash=000000000000002a"));
    }
}
```

- [ ] **Step 2: Implement reducer artifact writer**

Create `src/mir/reduce.rs`:

```rust
use std::{fs, path::{Path, PathBuf}};

use super::{MirModule, StableHash};

#[derive(Clone, Debug)]
pub struct ReductionRequest {
    failure: String,
    hash: StableHash,
    module: MirModule,
}

impl ReductionRequest {
    pub fn new(failure: impl Into<String>, hash: StableHash, module: MirModule) -> Self {
        Self {
            failure: failure.into(),
            hash,
            module,
        }
    }

    pub fn write_to(&self, dir: &Path) -> Result<PathBuf, String> {
        fs::create_dir_all(dir).map_err(|err| err.to_string())?;
        let path = dir.join(format!("{}-{}.wmir", self.failure, self.hash));
        let mut text = String::new();
        text.push_str(&format!("; failure={}\n", self.failure));
        text.push_str(&format!("; hash={}\n", self.hash));
        text.push_str(&crate::mir::text::render_module(&self.module));
        fs::write(&path, text).map_err(|err| err.to_string())?;
        Ok(path)
    }
}
```

Export from `src/mir/mod.rs`:

```rust
pub mod reduce;
pub use reduce::ReductionRequest;
```

- [ ] **Step 3: Add a common failure-artifact helper**

Add this helper in `src/mir/reduce.rs`:

```rust
pub fn preserve_failure_artifact(
    failure: &str,
    domain: &str,
    module: &MirModule,
) -> Result<PathBuf, String> {
    let hash = crate::mir::stable_hash_text(domain, &crate::mir::text::render_module(module));
    let request = ReductionRequest::new(failure, hash, module.clone());
    let reduction_dir = PathBuf::from("target").join("wrela").join("reductions");
    request.write_to(&reduction_dir)
}
```

Export it from `src/mir/mod.rs`:

```rust
pub use reduce::{preserve_failure_artifact, ReductionRequest};
```

- [ ] **Step 4: Use reducer on failed certified rewrites**

In `src/mir/rewrite.rs`, when a candidate rewrite verifier fails:

```rust
let _ = crate::mir::preserve_failure_artifact(
    "rewrite-verifier-failure",
    "wmir.failed-rewrite.v1",
    &candidate,
);
continue;
```

The failed candidate is never assigned back to the output module.

- [ ] **Step 5: Add reducer hooks for every Trident failure class**

Use `preserve_failure_artifact` at each failure site:

```rust
// src/mir/rewrite.rs: failed table-loop fusion verifier
let _ = crate::mir::preserve_failure_artifact(
    "table-fusion-verifier-failure",
    "wmir.failed-table-fusion.v1",
    &candidate,
);

// src/mir/perf.rs: generated-code checksum mismatch
let _ = crate::mir::preserve_failure_artifact(
    "checksum-mismatch",
    "wmir.checksum-mismatch.v1",
    module_for_codegen,
);

// src/mir/perf.rs: measurement is too noisy to classify
let _ = crate::mir::preserve_failure_artifact(
    "measurement-inconclusive",
    "wmir.measurement-inconclusive.v1",
    module_for_codegen,
);

// src/mir/ledger.rs: corrupt ledger lines were observed
if ledger.corrupt_lines() > 0 {
    let _ = crate::mir::preserve_failure_artifact(
        "ledger-corrupt",
        "wmir.ledger-corrupt.v1",
        module_for_codegen,
    );
}
```

Checksum mismatch and measurement-inconclusive artifacts are written only when a W-MIR module is available. If a failure happens before MIR exists, emit a structured perf report field instead:

```json
"tridentFailure":"ledger-corrupt-before-mir"
```

- [ ] **Step 6: Run focused test**

Run:

```bash
cargo test mir::reduce::tests::reducer_writes_artifact_for_failed_rewrite
```

Expected: pass.

**Acceptance criteria:**
- Reducer writes deterministic W-MIR artifacts.
- Rewrite verifier failures leave the original module unchanged.
- Failure artifacts include failure kind and stable hash.
- Checksum mismatch, measurement-inconclusive, ledger-corrupt, and table-fusion verifier failures all have reducer or structured-report hooks.
- Reducer failure does not panic; it is reported and the optimization path is disabled for that run.

---

### Task 10: Cost Feedback Loop

**Files:**
- Modify: `src/mir/cost.rs`
- Modify: `src/mir/rewrite.rs`
- Test: `tests/mir_ledger.rs`

**Description:** Allow measured known-winner ledger entries to override static cost estimates for exact matching optimizer keys.

- [ ] **Step 1: Add failing cost-feedback test**

Append to `tests/mir_ledger.rs`:

```rust
#[test]
fn known_winner_overrides_static_cost_for_matching_key() {
    let dir = std::env::temp_dir().join(format!("wrela-ledger-feedback-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let pass_set = wrela::mir::PassSet::only(wrela::mir::Pass::TableLoopFusion);
    let key = wrela::mir::OptimizerKey::new(
        wrela::mir::StableHash::new(123),
        wrela::mir::TargetProfile::GenericAArch64,
        &pass_set,
        "test-compiler",
        wrela::mir::CERTIFICATE_VERSION,
    );
    let mut ledger = wrela::mir::OptimizationLedger::open(
        dir,
        wrela::mir::TargetProfile::GenericAArch64,
    ).unwrap();
    ledger.record(wrela::mir::LedgerEntry::known_winner(key.clone(), 7, -9, 64)).unwrap();

    let decision = wrela::mir::CostDecision::from_ledger(&ledger, &key);
    assert_eq!(decision, wrela::mir::CostDecision::UseMeasuredWinner);
}
```

- [ ] **Step 2: Implement cost decision**

In `src/mir/cost.rs`, add:

```rust
use crate::mir::{LedgerStatus, OptimizationLedger, OptimizerKey};

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
```

Export from `src/mir/mod.rs`:

```rust
pub use cost::{BranchSelectCost, CostDecision, TargetProfile};
```

- [ ] **Step 3: Add release-report detail storage**

In `src/mir/rewrite.rs`, extend `ReleaseOptimizeReport`:

```rust
pub struct ReleaseOptimizeReport {
    // existing MIR 03 fields...
    details: Vec<String>,
}

impl ReleaseOptimizeReport {
    pub fn push_detail(&mut self, detail: impl Into<String>) {
        self.details.push(detail.into());
    }

    pub fn details(&self) -> &[String] {
        &self.details
    }
}
```

Initialize `details: Vec::new()` in every `ReleaseOptimizeReport` constructor or struct literal introduced by MIR 03.

- [ ] **Step 4: Thread cost decision into fusion extraction**

In `src/mir/rewrite.rs`, compute an optimizer key for the candidate region and record the reason in `ReleaseOptimizeReport`:

```rust
let key = crate::mir::OptimizerKey::new(
    before_hash,
    crate::mir::TargetProfile::GenericAArch64,
    &crate::mir::PassSet::only(crate::mir::Pass::TableLoopFusion),
    env!("CARGO_PKG_VERSION"),
    crate::mir::CERTIFICATE_VERSION,
);
let decision = crate::mir::CostDecision::from_ledger(&ledger, &key);
report.push_detail(format!("cost-decision:{decision:?}"));
```

If no ledger is configured, use `CostDecision::UseStaticEstimate`.

- [ ] **Step 5: Run focused test**

Run:

```bash
cargo test --test mir_ledger known_winner_overrides_static_cost_for_matching_key
```

Expected: pass.

**Acceptance criteria:**
- Known winners can override static cost only for exact key matches.
- Known losers can block a candidate for exact key matches.
- Missing ledger entries fall back to static estimate.
- Cost decisions are reported.

---

### Task 11: MIR 05 Final Quality Gate And Phase A Handoff

**Files:**
- Modify only if verification reveals a bug.

**Description:** Prove MIR 05 before handoff with correctness, measurement, ledger, reducer, and Trident evidence.

- [ ] **Step 1: Run full local quality gate**

Run:

```bash
./scripts/quality-gate.sh
```

Expected: pass.

- [ ] **Step 2: Run table-loop fusion A/B smoke**

Run:

```bash
cargo run -- perf code --mode release --disable-pass table-loop-fusion --repeat 7 --json fixtures/perf/filter_sum.wrela
cargo run -- perf code --mode release --only-pass table-loop-fusion --repeat 7 --json fixtures/perf/filter_sum.wrela
```

Expected:
- Exit `0` on supported AArch64 hosts.
- Baseline and candidate checksums match.
- Measurement classification is one of `stable-win`, `stable-loss`, `neutral`, or `inconclusive`.
- No runtime claim is made when classification is `inconclusive`.

- [ ] **Step 3: Run debug/why smoke**

Run:

```bash
cargo run -- debug mir --why-rewrite 0 --only-pass table-loop-fusion fixtures/perf/filter_sum.wrela
```

Expected: output contains `rule: table-loop-fusion`, `same-table`, `state-edges-preserved`, `trap-order-preserved`, `before:`, and `after:`.

- [ ] **Step 4: Inspect ledger output**

Run:

```bash
find target/wrela/ledger -maxdepth 2 -type f -print
```

Expected: at least one ledger file exists after supported A/B run. Ledger size stays below `4 MiB` per target profile directory.

- [ ] **Step 5: Complete Phase A self review**

Use the repository review workflow in `docs/implementation/reviews/README.md`. Fix every finding at every priority unless the handoff message documents an explicit disagreement.

**Acceptance criteria:**
- `./scripts/quality-gate.sh` passes.
- Table-loop fusion has certificate facts, verifier evidence, and debug/why output.
- Data-plane generated-code A/B path executes on supported AArch64 hosts with checksum discipline.
- Persistent ledger stores bounded entries and ignores corrupt entries.
- Reducer artifact path works for failed rewrites.
- Phase A verdict is APPROVED in the handoff message before user handoff.

## Self-Review Checklist

- [ ] Stable identity V1 includes region hash, target profile, pass config hash, compiler version, and certificate version.
- [ ] Stale certificates and stale ledger entries cannot justify rewrites.
- [ ] Authority algebra engine owns fusion legality facts.
- [ ] Authority loop facts include compatible masks and disjoint mutation places.
- [ ] Measurement classification enforces checksum, sample count, noise, and 3% movement rules.
- [ ] Data-plane harness ABI matches the five-parameter `filter_sum.wrela` fixture and computes the same checksum, including `small_total`.
- [ ] Fused reduce markers change lowering by emitting one loop with two accumulators.
- [ ] Persistent ledger is bounded, deterministic, per-target-profile, and corruption-tolerant.
- [ ] A/B measurement results are written to the ledger as known winner, known loser, or needs remeasure.
- [ ] Table-loop fusion emits certificates with authority facts and verifier evidence.
- [ ] Debug/why output prints certificate facts and hashes.
- [ ] Reducer writes deterministic W-MIR artifacts for rewrite, checksum, measurement, and ledger failure classes and never applies failed candidates.
- [ ] Cost feedback uses measured winners only on exact stable-key matches.
- [ ] No external crates were added.
