// N2 Sindri -- Universal AST-based code quality gate (Hecaton handler)
// Tree-sitter multi-lang: Rust, TypeScript, JavaScript, Python, Go, Java.

use std::path::Path;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use tree_sitter::{Parser, Node};
// tracing used by other modules but not directly here
#[path = "sindri_io.rs"]
mod scan_io;
#[path = "sindri_syntax.rs"]
mod syntax;
pub use scan_io::{scan_files, scan_dir};

// -- Types --

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    #[serde(rename = "BLOCKER")]
    Blocker,
    #[serde(rename = "WARNING")]
    Warning,
    #[serde(rename = "INFO")]
    Info,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Violation {
    pub rule: String,
    pub severity: Severity,
    pub file: String,
    pub line: Option<usize>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanResult {
    pub passed: bool,
    pub blockers: Vec<Violation>,
    pub warnings: Vec<Violation>,
    pub info: Vec<Violation>,
    pub scanned_files: usize,
    pub report: String,
    #[serde(skip)]
    pub scanned_paths: Vec<String>,
}

// -- Constants --

const MAX_FILE_LINES: usize = 500;
const MAX_FUNCTION_LINES: usize = 50;
const WARNING_THRESHOLD: usize = 3;

// -- Language detection --

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Lang { Rust, TypeScript, Tsx, JavaScript, Python, Go, Java, Unknown }

fn detect_lang(path: &str) -> Lang {
    let extension = Path::new(path).extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("rs") => Lang::Rust,
        Some("ts" | "mts" | "cts") => Lang::TypeScript,
        Some("tsx") => Lang::Tsx,
        Some("js" | "jsx" | "mjs" | "cjs") => Lang::JavaScript,
        Some("py" | "pyi") => Lang::Python,
        Some("go") => Lang::Go,
        Some("java") => Lang::Java,
        _ => Lang::Unknown,
    }
}

pub fn supports_path(path: &str) -> bool { detect_lang(path) != Lang::Unknown }

fn get_parser(lang: Lang) -> Option<Parser> {
    let mut parser = Parser::new();
    let language = match lang {
        Lang::Rust => tree_sitter_rust::LANGUAGE,
        Lang::TypeScript => tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        Lang::Tsx => tree_sitter_typescript::LANGUAGE_TSX,
        Lang::JavaScript => tree_sitter_javascript::LANGUAGE,
        Lang::Python => tree_sitter_python::LANGUAGE,
        Lang::Go => tree_sitter_go::LANGUAGE,
        Lang::Java => tree_sitter_java::LANGUAGE,
        Lang::Unknown => return None,
    };
    parser.set_language(&language.into()).ok()?;
    Some(parser)
}

// -- File-level checks --

/// Check if a source line above the node contains `// sindri:ignore`
fn is_suppressed(node: Node, source: &[u8]) -> bool {
    let row = node.start_position().row;
    if row == 0 { return false; }
    let lines: Vec<&str> = std::str::from_utf8(source).unwrap_or("").lines().collect();
    // Check the line directly above for suppression comment
    lines.get(row.wrapping_sub(1)).is_some_and(|l| l.contains("sindri:ignore"))
}

/// RT12-SI-01: Check if file has `sindri:ignore-file` directive in first 5 lines.
/// This allows core files (sindri.rs, db.rs) to be maintained without self-blocking.
fn has_file_ignore(content: &str) -> bool {
    content.lines().take(5).any(|l| l.contains("sindri:ignore-file"))
}

fn check_file_metrics(content: &str, basename: &str) -> Vec<Violation> {
    let mut v = Vec::new();
    let lines: Vec<&str> = content.lines().collect();

    // RT12-SI-01: Skip file-length check if sindri:ignore-file is present
    if lines.len() > MAX_FILE_LINES && !has_file_ignore(content) {
        v.push(Violation {
            rule: "B3_FILE_TOO_LONG".into(), severity: Severity::Blocker,
            file: basename.into(), line: None,
            message: format!("File is {} lines (max {})", lines.len(), MAX_FILE_LINES),
        });
    }

    let first = lines.first().map(|l| l.trim()).unwrap_or("");
    if !first.starts_with("//") && !first.starts_with("/*") && !first.starts_with('#') {
        v.push(Violation {
            rule: "I3_NO_FIRST_COMMENT".into(), severity: Severity::Info,
            file: basename.into(), line: Some(1),
            message: "Missing first-line purpose comment".into(),
        });
    }

    let todo_re = regex::Regex::new(r"\b(TODO|FIXME|HACK|XXX)\b").unwrap();
    for (i, line) in lines.iter().enumerate() {
        if todo_re.is_match(line) && (line.contains("//") || line.contains('#')) {
            v.push(Violation {
                rule: "I4_TODO_REMNANT".into(), severity: Severity::Info,
                file: basename.into(), line: Some(i + 1),
                message: format!("TODO/FIXME at line {}", i + 1),
            });
            break;
        }
    }
    v
}

// -- AST checks --

fn check_function_length(node: Node, source: &[u8], basename: &str, violations: &mut Vec<Violation>) {
    check_function_length_inner(node, source, basename, violations, 0);
}

fn check_function_length_inner(node: Node, source: &[u8], basename: &str, violations: &mut Vec<Violation>, depth: usize) {
    if depth > MAX_AST_DEPTH { return; }
    if matches!(node.kind(),
        "function_item" | "function_declaration" | "method_definition" |
        "arrow_function" | "function_definition" | "method_declaration"
    ) {
        let start = node.start_position().row + 1;
        let end = node.end_position().row + 1;
        let body_lines = end - start + 1;
        if body_lines > MAX_FUNCTION_LINES && !is_suppressed(node, source) {
            let name = node.child_by_field_name("name")
                .and_then(|n| n.utf8_text(source).ok())
                .unwrap_or("<anonymous>");
            violations.push(Violation {
                rule: "B4_FUNCTION_TOO_LONG".into(), severity: Severity::Blocker,
                file: basename.into(), line: Some(start),
                message: format!("Function '{}' is {} lines (max {})", name, body_lines, MAX_FUNCTION_LINES),
            });
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_function_length_inner(child, source, basename, violations, depth + 1);
    }
}

fn v(rule: &str, sev: Severity, file: &str, line: usize, msg: String) -> Violation {
    Violation { rule: rule.into(), severity: sev, file: file.into(), line: Some(line), message: msg }
}

fn has_empty_body(node: Node, field: &str) -> bool {
    node.child_by_field_name(field).is_some_and(|b| {
        // A block `{}` with no statements, or a unit expression `()`
        matches!(b.kind(),"block"|"statement_block"|"unit_expression") && b.named_child_count() == 0
    })
}

fn check_rust(kind: &str, node: Node, source: &[u8], basename: &str, line: usize, out: &mut Vec<Violation>) {
    if kind == "unsafe_block" {
        out.push(v("B1_UNSAFE", Severity::Blocker, basename, line, format!("unsafe block at line {}", line)));
    }
    if syntax::unsafe_function(node) && !is_suppressed(node,source) {
        out.push(v("B1_UNSAFE_FN",Severity::Blocker,basename,line,
            format!("unsafe fn declaration at line {line}")));
    }
    // W4: Empty match arm -- skip if pattern contains wildcard or enum variants (intentional no-op)
    if kind == "match_arm" && has_empty_body(node, "value") {
        let pattern = node.child_by_field_name("pattern")
            .and_then(|p| p.utf8_text(source).ok()).unwrap_or("_");
        // Skip: wildcard `_`, enum catch-all `Foo | Bar => {}`, `None => {}`
        let is_intentional = pattern.contains('_') || pattern.contains('|') || pattern.contains("::");
        if !is_intentional {
            out.push(v("W4_EMPTY_MATCH_ARM", Severity::Warning, basename, line, format!("Empty match arm at line {}", line)));
        }
    }
}

fn check_ts_js(kind: &str, node: Node, source: &[u8], basename: &str, line: usize, out: &mut Vec<Violation>) {
    if kind == "predefined_type"
        && node.utf8_text(source).ok() == Some("any") {
            out.push(v("B1_ANY_TYPE", Severity::Blocker, basename, line, format!("'any' type at line {}", line)));
        }
    if kind == "catch_clause" && has_empty_body(node, "body") {
        out.push(v("W4_EMPTY_CATCH", Severity::Warning, basename, line, format!("Empty catch block at line {}", line)));
    }
    if kind == "call_expression" {
        if let Some(func) = node.child_by_field_name("function") {
            if func.utf8_text(source).ok() == Some("console.log") {
                out.push(v("I2_CONSOLE_LOG", Severity::Info, basename, line, format!("console.log at line {}", line)));
            }
        }
    }
}

/// RT12-SI-03: Max AST traversal depth - prevents stack overflow DoS
/// from deeply nested files (e.g., [[[[...]]]] 5000 levels).
const MAX_AST_DEPTH: usize = 200;

fn check_dangerous_patterns(node: Node, source: &[u8], basename: &str, lang: Lang, violations: &mut Vec<Violation>) {
    check_dangerous_patterns_inner(node, source, basename, lang, violations, 0);
}

fn check_dangerous_patterns_inner(node: Node, source: &[u8], basename: &str, lang: Lang, violations: &mut Vec<Violation>, depth: usize) {
    if depth > MAX_AST_DEPTH { return; }
    let kind = node.kind();
    let line = node.start_position().row + 1;

    match lang {
        Lang::Rust => check_rust(kind, node, source, basename, line, violations),
        Lang::TypeScript | Lang::Tsx | Lang::JavaScript => check_ts_js(kind, node, source, basename, line, violations),
        Lang::Python => {
            if kind == "except_clause" && node.child_by_field_name("value").is_none() {
                violations.push(v("W4_BARE_EXCEPT", Severity::Warning, basename, line,
                    format!("Bare 'except:' at line {} -- specify exception type", line)));
            }
        }
        Lang::Java => {
            if kind == "catch_clause" && has_empty_body(node, "body") {
                violations.push(v("W4_EMPTY_CATCH", Severity::Warning, basename, line,
                    format!("Empty catch block at line {}", line)));
            }
        }
        Lang::Go | Lang::Unknown => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        check_dangerous_patterns_inner(child, source, basename, lang, violations, depth + 1);
    }
}

/// Return true if this AST node is a named-constant declaration whose
/// literal children should be excluded from magic-number counting.
/// Rust: const/static items. Go: const_declaration/const_spec.
/// TS/JS: lexical_declaration with leading `const` token (skips `let`/`var`).
fn is_const_declaration(node: Node, source: &[u8]) -> bool {
    match node.kind() {
        "const_item" | "static_item"           // Rust
        | "const_declaration" | "const_spec"   // Go
        => true,
        "lexical_declaration" => {
            // TS/JS: first child is the keyword token ("const" | "let")
            node.child(0)
                .and_then(|c| c.utf8_text(source).ok())
                .map(|t| t == "const")
                .unwrap_or(false)
        }
        _ => false,
    }
}

fn collect_numeric_literals(node: Node, source: &[u8], numbers: &mut HashMap<String, Vec<usize>>) {
    collect_numeric_literals_inner(node, source, numbers, 0);
}

/// RT12-SI-04: Parse hex (0x2A), binary (0b101010), octal (0o52), and type
/// suffixes (42u32, 3.14f64) into their actual numeric value for dedup.
/// Old code used f64::parse which fails on all non-decimal formats -> 0.0 -> skip.
fn parse_numeric_value(text: &str) -> f64 {
    let clean = text.replace('_', "");
    // Extract only valid digit characters for each base
    if clean.starts_with("0x") || clean.starts_with("0X") {
        let hex: String = clean[2..].chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        return u64::from_str_radix(&hex, 16).unwrap_or(0) as f64;
    }
    if clean.starts_with("0b") || clean.starts_with("0B") {
        let bin: String = clean[2..].chars().take_while(|c| *c == '0' || *c == '1').collect();
        return u64::from_str_radix(&bin, 2).unwrap_or(0) as f64;
    }
    if clean.starts_with("0o") || clean.starts_with("0O") {
        let oct: String = clean[2..].chars().take_while(|c| c.is_ascii_digit() && *c < '8').collect();
        return u64::from_str_radix(&oct, 8).unwrap_or(0) as f64;
    }
    // Decimal: strip Rust/Go type suffixes (u8, u32, i64, f64, usize, isize, etc.)
    // Take only the leading numeric portion (digits, dots, minus)
    let numeric: String = clean.chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    if numeric.is_empty() { return 0.0; }
    numeric.parse::<f64>().unwrap_or(0.0)
}

fn collect_numeric_literals_inner(node: Node, source: &[u8], numbers: &mut HashMap<String, Vec<usize>>, depth: usize) {
    if depth > MAX_AST_DEPTH { return; }
    if is_const_declaration(node, source) { return; }

    if matches!(node.kind(), "integer_literal" | "float_literal" | "number") {
        if let Ok(text) = node.utf8_text(source) {
            let val = parse_numeric_value(text);
            if ![0.0, 1.0, -1.0, 2.0, 100.0].contains(&val) && val > 10.0 {
                // RT12-SI-04: Use normalized value as key (not raw text)
                // so 42, 0x2A, 0b101010, 42u32 all count as the same number.
                let key = if val.fract() == 0.0 { format!("{}", val as i64) } else { format!("{}", val) };
                numbers.entry(key).or_default().push(node.start_position().row + 1);
            }
        }
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect_numeric_literals_inner(child, source, numbers, depth + 1);
    }
}

// -- Single file analysis --

fn analyze_file(path: &str) -> Vec<Violation> {
    let mut out = Vec::new();
    let basename = Path::new(path).file_name().and_then(|n| n.to_str()).unwrap_or(path);
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => { out.push(v("READ_ERROR", Severity::Blocker, basename, 0, "Cannot read file".into())); return out; }
    };

    let lang = detect_lang(path);
    if lang == Lang::Unknown { return out; }
    let mut parser = match get_parser(lang) { Some(p) => p, None => {
        out.push(v("PARSER_UNAVAILABLE", Severity::Blocker, basename, 0, "Cannot initialize parser".into())); return out;
    } };
    let tree = match parser.parse(&content, None) { Some(t) => t, None => {
        out.push(v("PARSE_ERROR", Severity::Blocker, basename, 0, "Parser did not return a syntax tree".into())); return out;
    } };
    if tree.root_node().has_error() {
        out.extend(syntax::parse_errors(tree.root_node(),basename));
    }
    // Suppression applies to lint rules, never unreadable or malformed source.
    if has_file_ignore(&content) { return out; }
    out.extend(check_file_metrics(&content, basename));

    let source = content.as_bytes();
    let root = tree.root_node();
    check_function_length(root, source, basename, &mut out);
    check_dangerous_patterns(root, source, basename, lang, &mut out);
    check_magic_numbers(root, source, basename, &mut out);
    out
}

fn check_magic_numbers(root: Node, source: &[u8], basename: &str, violations: &mut Vec<Violation>) {
    let mut numbers: HashMap<String, Vec<usize>> = HashMap::new();
    collect_numeric_literals(root, source, &mut numbers);
    for (val, lines) in &numbers {
        if lines.len() >= 3 {
            violations.push(v("W3_MAGIC_NUMBER", Severity::Warning, basename,
                lines.first().copied().unwrap_or(0),
                format!("Magic number {} appears {} times -- extract to constant", val, lines.len())));
        }
    }
}

// -- Report builder --

fn build_report(violations: &[Violation], scanned_files: usize) -> ScanResult {
    let mut blockers: Vec<_> = violations.iter().filter(|v| v.severity == Severity::Blocker).cloned().collect();
    if scanned_files == 0 {
        blockers.push(v("NO_SOURCE_FILES", Severity::Blocker, "scan", 0, "No supported source files were scanned".into()));
    }
    let warnings: Vec<_> = violations.iter().filter(|v| v.severity == Severity::Warning).cloned().collect();
    let info: Vec<_> = violations.iter().filter(|v| v.severity == Severity::Info).cloned().collect();
    let passed = blockers.is_empty() && warnings.len() <= WARNING_THRESHOLD;

    let mut lines = Vec::new();
    lines.push(if passed {
        format!("=== N2 SINDRI: PASSED ({} files) ===", scanned_files)
    } else {
        format!("=== N2 SINDRI: BLOCKED ({} files) ===", scanned_files)
    });
    lines.push(String::new());

    if !blockers.is_empty() {
        lines.push(format!("BLOCKERS ({}):", blockers.len()));
        for v in &blockers {
            let loc = v.line.map(|l| format!(":{l}")).unwrap_or_default();
            lines.push(format!("  [{}] {}{} -- {}", v.rule, v.file, loc, v.message));
        }
        lines.push(String::new());
    }
    if !warnings.is_empty() {
        lines.push(format!("WARNINGS ({}/{} threshold):", warnings.len(), WARNING_THRESHOLD));
        for v in &warnings {
            let loc = v.line.map(|l| format!(":{l}")).unwrap_or_default();
            lines.push(format!("  [{}] {}{} -- {}", v.rule, v.file, loc, v.message));
        }
        lines.push(String::new());
    }
    if !info.is_empty() {
        lines.push(format!("INFO ({}):", info.len()));
        for v in info.iter().take(10) {
            let loc = v.line.map(|l| format!(":{l}")).unwrap_or_default();
            lines.push(format!("  [{}] {}{} -- {}", v.rule, v.file, loc, v.message));
        }
        if info.len() > 10 { lines.push(format!("  ... and {} more", info.len() - 10)); }
        lines.push(String::new());
    }
    if !passed { lines.push("Fix all BLOCKERS and reduce WARNINGS below threshold, then retry.".into()); }

    ScanResult { passed, blockers, warnings, info, scanned_files, report: lines.join("\n"), scanned_paths: Vec::new() }
}

// -- Tests --

#[cfg(test)]
#[path = "sindri_tests.rs"]
mod tests;
