//! Verify the executable entry point and its observable exit/output contract.
use std::process::Command;

#[test]
fn cli_contract() {
    let root = std::env::temp_dir().join(format!("sindri-cli-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let good = root.join("good.rs");
    let bad = root.join("bad.rs");
    std::fs::write(&good, "// Valid fixture\nfn main() {}\n").unwrap();
    std::fs::write(&bad, "fn main( {\n").unwrap();
    for (path, code) in [(&good, 0), (&bad, 1), (&root.join("missing.rs"), 1)] {
        let output = Command::new(env!("CARGO_BIN_EXE_n2-sindri"))
            .arg("--files").arg(path).output().unwrap();
        assert_eq!(output.status.code(), Some(code));
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["passed"], code == 0);
    }
    let invalid = Command::new(env!("CARGO_BIN_EXE_n2-sindri"))
        .arg("--unknown").arg(&root).output().unwrap();
    assert_eq!(invalid.status.code(), Some(2));
    let help = Command::new(env!("CARGO_BIN_EXE_n2-sindri"))
        .arg("--help").output().unwrap();
    assert!(help.status.success());
    std::fs::remove_dir_all(root).unwrap();
}
