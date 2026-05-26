pub mod aarch64;
pub mod aegraph;
pub mod authority;
pub mod build;
pub mod cert;
pub mod cost;
pub mod dataplane;
mod dataplane_codegen;
pub mod effect;
pub mod emit;
pub mod facts;
pub mod hash;
pub mod hotness;
pub mod id;
pub mod ir;
pub mod ledger;
pub mod lir;
pub mod lir_verify;
pub mod lower;
pub mod module_eq;
pub mod parse_text;
pub mod pass_control;
pub mod perf;
pub mod reduce;
pub mod regalloc;
pub mod report;
pub mod rewrite;
pub mod semantic;
pub mod text;
pub mod ty;
pub mod type_text;
pub mod verify;

pub use aegraph::{AeGraph, EClassId, Enode};
pub use authority::{
    AuthorityEngine, AuthorityFact, AuthorityQuery, LoopAuthorityFacts, NamedAuthorityFact,
};
pub use build::{MirBuildResult, build_mir};
pub use cert::{
    CERTIFICATE_VERSION, CertificateValidation, MAX_REWRITE_EVENTS, RewriteCertificate,
    RewriteEvent, RewriteEventLog, RewriteFact, RewriteOutcome, RewriteRule,
};
pub use cost::{BranchSelectCost, CostDecision, TargetProfile};
pub use effect::{Effect, EffectSet};
pub use facts::{CapabilityPath, OperationFacts, OwnershipMode, TrapProvenance};
pub use hash::{OptimizerKey, StableHash, stable_hash_bytes, stable_hash_text};
pub use hotness::StaticHotnessSeed;
pub use id::{BlockId, OperationId, PlaceId, RegionId, TypeId, ValueId};
pub use ir::{
    BlockData, MirModule, OperationData, OperationKind, PlaceData, RegionData, RegionKind,
    ThetaKind, ValueData,
};
pub use ledger::{
    LEDGER_MAX_BYTES, LEDGER_MAX_ENTRIES, LedgerEntry, LedgerStatus, OptimizationLedger,
};
pub use lir::{
    LirBlockId, LirFunctionId, LirInst, LirInstId, LirModule, LirOpcode, LirOperand, MemoryOrder,
    RegisterClass, SideEffectClass, TargetEffects, VirtualReg,
};
pub use lir_verify::{LirVerifyResult, verify_lir};
pub use lower::lower_to_lir;
pub use module_eq::modules_semantically_equal;
pub use parse_text::{TextParseError, parse_module};
pub use pass_control::{Pass, PassSet};
pub use perf::{
    CompareCodeReport, MeasurementClassification, MeasurementResult, SampleSet,
    classify_measurement, compare_code,
};
pub use reduce::{ReductionRequest, preserve_failure_artifact};
pub use report::MirReport;
pub use rewrite::{ReleaseOptimizeReport, ReleaseOptimizeResult, optimize_release};
pub use ty::{AccessMode, MirType, ScalarType, StateTokenKind};
pub use verify::{VerifyResult, verify_module};
