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
