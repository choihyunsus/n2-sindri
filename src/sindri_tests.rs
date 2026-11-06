// AST rule and numeric literal regression tests.
use super::*;

#[test]
fn test_scan_nonexistent_file() {
    let result = scan_files("/tmp/nonexistent_12345.rs");
    assert!(!result.passed);
    assert_eq!(result.scanned_files, 0);
}

#[test]
fn test_severity_serialize() {
    let json = serde_json::to_string(&Severity::Blocker).unwrap();
    assert_eq!(json, "\"BLOCKER\"");
}

#[test]
fn test_scan_empty_input() {
    let result = scan_files("");
    assert!(!result.passed);
    assert_eq!(result.scanned_files, 0);
}

fn count_magic_numbers(source: &str, lang: Lang) -> Vec<Violation> {
    let mut parser = get_parser(lang).expect("parser");
    let tree = parser.parse(source, None).expect("parse");
    let mut violations = Vec::new();
    check_magic_numbers(tree.root_node(), source.as_bytes(), "test.rs", &mut violations);
    violations
}

#[test]
fn test_magic_number_ignores_rust_const_and_static() {
    // Three const defs with the same literal 30 used to trip W3 even
    // though each literal is already symbolically named. Regression:
    // const_item/static_item subtrees must be excluded.
    let source = r#"
        const SEVERITY_LOW: u32 = 30;
        const SEVERITY_MED: u32 = 30;
        const SEVERITY_HIGH: u32 = 30;
        static DEFAULT_PORT: u16 = 8080;
        fn noop() {}
    "#;
    let violations = count_magic_numbers(source, Lang::Rust);
    assert!(
        violations.is_empty(),
        "const/static literals must not count as magic numbers: {:?}",
        violations,
    );
}

#[test]
fn test_magic_number_still_catches_inline_literals() {
    // Positive: literals inside function bodies still count.
    let source = r#"
        fn foo() { let x = 42; }
        fn bar() { let y = 42; }
        fn baz() { let z = 42; }
    "#;
    let violations = count_magic_numbers(source, Lang::Rust);
    assert_eq!(violations.len(), 1, "inline duplicated literal should still warn");
    assert!(violations[0].message.contains("42"));
}

#[test]
fn test_magic_number_ignores_ts_const_only() {
    // `const` must be skipped, `let` must still count.
    let source = r#"
        const MAX_RETRIES = 50;
        const MAX_TIMEOUT = 50;
        const MAX_BACKOFF = 50;
        function inline() {
            let a = 77;
            let b = 77;
            let c = 77;
        }
    "#;
    let violations = count_magic_numbers(source, Lang::TypeScript);
    let has_50 = violations.iter().any(|v| v.message.contains("50"));
    let has_77 = violations.iter().any(|v| v.message.contains("77"));
    assert!(!has_50, "TS const literals must be excluded");
    assert!(has_77, "TS let literals must still count");
}

#[test]
fn scan_packaged_sources() {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let result = scan_dir(directory.to_str().unwrap());
    assert!(result.scanned_files >= 5);
    assert!(!result.blockers.iter().any(|item| item.rule == "PARSE_ERROR"));
}

// RT12-SI-01: sindri:ignore-file skips file-length BLOCKER
#[test]
fn test_file_ignore_directive() {
    assert!(has_file_ignore("// sindri:ignore-file -- core module\nfn main() {}"));
    assert!(!has_file_ignore("// normal comment\nfn main() {}"));
}

// RT12-SI-04: Hex/binary/suffix numbers parsed correctly
#[test]
fn test_parse_numeric_value() {
    let expected_integer = 42.0;
    for literal in ["42", "0x2A", "0b101010", "0o52", "42u32"] {
        assert_eq!(parse_numeric_value(literal), expected_integer, "{literal}");
    }
    assert_eq!(parse_numeric_value("3.25f64"), 3.25);
    assert_eq!(parse_numeric_value("1_000"), 1000.0);
}
