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
    )
    .unwrap();
    ledger
        .record(wrela::mir::LedgerEntry::known_winner(
            key.clone(),
            7,
            -12,
            104,
        ))
        .unwrap();
    ledger.flush().unwrap();

    let loaded =
        wrela::mir::OptimizationLedger::open(dir, wrela::mir::TargetProfile::GenericAArch64)
            .unwrap();
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

    let ledger =
        wrela::mir::OptimizationLedger::open(dir, wrela::mir::TargetProfile::GenericAArch64)
            .unwrap();
    assert_eq!(ledger.corrupt_lines(), 1);
}

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
    let mut ledger =
        wrela::mir::OptimizationLedger::open(dir, wrela::mir::TargetProfile::GenericAArch64)
            .unwrap();
    ledger
        .record(wrela::mir::LedgerEntry::known_winner(
            key.clone(),
            7,
            -9,
            64,
        ))
        .unwrap();

    let decision = wrela::mir::CostDecision::from_ledger(&ledger, &key);
    assert_eq!(decision, wrela::mir::CostDecision::UseMeasuredWinner);
}
