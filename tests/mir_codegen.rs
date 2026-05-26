use std::path::PathBuf;

fn fixture(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("mir")
        .join(rel)
}

#[test]
fn lower_data_flow_fixture_to_lir() {
    let check = wrela::check::check_root(fixture("data_flow.wrela"));
    assert!(check.ok());
    let mir = wrela::mir::build_mir(&check);
    assert!(mir.ok());

    let lir = wrela::mir::lower_to_lir(mir.module().unwrap());

    assert_eq!(lir.functions().len(), 1);
    assert!(
        lir.instructions()
            .iter()
            .any(|inst| inst.opcode() == wrela::mir::LirOpcode::Add)
    );
    assert!(
        lir.instructions()
            .iter()
            .any(|inst| inst.opcode() == wrela::mir::LirOpcode::Ret)
    );
}

#[test]
fn emit_aarch64_for_data_flow_fixture() {
    let check = wrela::check::check_root(fixture("data_flow.wrela"));
    assert!(check.ok());
    let mir = wrela::mir::build_mir(&check);
    let lir = wrela::mir::lower_to_lir(mir.module().unwrap());
    let allocated = wrela::mir::regalloc::allocate_registers(&lir).unwrap();
    let asm = wrela::mir::emit::emit_aarch64(&allocated);

    assert!(asm.contains(".text"));
    assert!(asm.contains("_app_flow_Calculator_add:"));
    assert!(asm.contains("add "));
    assert!(asm.contains("ret"));
    assert!(
        asm.contains("mov x1") || asm.contains("mov\tx1"),
        "parameter reads must move from ABI argument registers: {asm}"
    );
}

#[test]
fn dump_asm_command_prints_aarch64() {
    let root = fixture("data_flow.wrela");
    let mut out = Vec::new();
    let mut err = Vec::new();

    let code = wrela::command::run_with_io(
        vec![
            "wrela".to_string(),
            "dump".to_string(),
            "asm".to_string(),
            root.to_string_lossy().to_string(),
        ],
        &mut out,
        &mut err,
    );

    assert_eq!(code, 0);
    assert!(err.is_empty());
    let asm = String::from_utf8(out).unwrap();
    assert!(asm.contains(".text"));
    assert!(asm.contains("ret"));
}
