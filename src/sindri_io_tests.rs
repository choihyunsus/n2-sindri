//! A quality gate must prove it parsed source, including TSX and explicit failures.
use super::*;

struct TestTree(PathBuf);
impl TestTree {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("n2-sindri-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }
}
impl Drop for TestTree {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

#[test]
fn syntax_errors_cannot_pass_even_with_lint_suppression() {
    let tree = TestTree::new();
    for prefix in ["// purpose", "// sindri:ignore-file -- only lint rules"] {
        let path = tree.write("broken.rs", &format!("{prefix}\nfn missing( {{"));
        let result = scan_files(path.to_str().unwrap());
        assert!(!result.passed);
        assert!(result.blockers.iter().any(|v| v.rule == "PARSE_ERROR"));
    }
}

#[test]
fn valid_tsx_uses_the_jsx_aware_typescript_grammar() {
    let tree = TestTree::new();
    let path = tree.write("view.tsx", "// A small typed view.\nexport function View(): JSX.Element { return <div>Hello</div>; }\n");
    let result = scan_files(path.to_str().unwrap());
    assert!(result.passed, "{}", result.report);
    assert_eq!(result.scanned_files, 1);
}

#[test]
fn unsafe_declaration_checks_ignore_body_strings_and_comments() {
    let tree=TestTree::new();
    for body in [r#"let _ = " unsafe fn ";"#, "// unsafe fn example() {}\nlet _ = 1;"] {
        let path=tree.write("safe.rs",&format!("// Safe fixture\nfn safe() {{ {body} }}"));
        let report=scan_files(path.to_str().unwrap());
        assert!(!report.blockers.iter().any(|v|v.rule=="B1_UNSAFE_FN"),"{}",report.report);
    }
}

#[test]
fn unsafe_declaration_checks_cover_qualifiers_and_exact_nested_owner() {
    let tree=TestTree::new();
    for declaration in ["pub(crate) unsafe extern \"C\" fn callback() {}",
        "pub unsafe fn callback() {}", "pub async unsafe fn callback() {}",
        "fn safe_outer() { unsafe fn callback() {} }"] {
        let path=tree.write("unsafe.rs",&format!("// Unsafe fixture\n{declaration}"));
        let report=scan_files(path.to_str().unwrap());
        assert_eq!(report.blockers.iter().filter(|v|v.rule=="B1_UNSAFE_FN").count(),1,"{}",report.report);
    }
}

#[test]
fn malformed_source_diagnostic_identifies_a_real_location_without_echoing_source() {
    let tree=TestTree::new();
    let path=tree.write("broken.rs","// SECRET_FIXTURE_DO_NOT_ECHO\nfn missing( {\n");
    let report=scan_files(path.to_str().unwrap());
    let errors:Vec<_>=report.blockers.iter().filter(|v|v.rule=="PARSE_ERROR").collect();
    assert!(!errors.is_empty());
    assert!(errors.iter().all(|e|e.line.is_some_and(|line|line>0)));
    assert!(!report.report.contains("SECRET_FIXTURE_DO_NOT_ECHO"));
}

#[test]
fn missing_unreadable_and_unmatched_inputs_do_not_claim_a_successful_scan() {
    let tree = TestTree::new();
    let good = tree.write("good.rs", "// A valid small fixture.\nfn main() {}\n");
    let missing = tree.0.join("missing.rs");
    let result = scan_files(&format!("{},{}", good.display(), missing.display()));
    assert!(!result.passed);
    assert_eq!(result.scanned_files, 1);
    let unreadable = tree.write("invalid-utf8.rs", "");
    std::fs::write(&unreadable, [0xff, 0xfe]).unwrap();
    assert!(!scan_files(unreadable.to_str().unwrap()).passed);
    assert!(!scan_files(tree.0.join("missing*.rs").to_str().unwrap()).passed);
    assert!(!scan_files("[").passed);
}

#[test]
fn directory_scan_excludes_dependency_trees_and_handles_invalid_roots() {
    let tree = TestTree::new();
    tree.write("good.rs", "// Source fixture.\nfn main() {}\n");
    let vendor = tree.0.join("vendor");
    std::fs::create_dir(&vendor).unwrap();
    std::fs::write(vendor.join("broken.rs"), "fn broken(").unwrap();
    let result = scan_dir(tree.0.to_str().unwrap());
    assert!(result.passed, "{}", result.report);
    assert_eq!(result.scanned_files, 1);
    assert!(!scan_dir(tree.0.join("missing").to_str().unwrap()).passed);
}
