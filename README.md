# N2 Sindri

A local AST-based code quality scanner for Rust, TypeScript/TSX, JavaScript/JSX, Python, Go and Java. Includes a Rust library and a JSON command-line interface.

[한국어 안내](README.ko.md)

## Install

Requires Rust 1.98 or later and a native C toolchain for the bundled Tree-sitter grammars. On Windows, install Visual Studio C++ Build Tools.

```sh
cargo install --git https://github.com/choihyunsus/n2-sindri --tag v1.0.0 --locked
```

## Scan

```sh
n2-sindri --dir ./src
n2-sindri --files "src/main.rs,src/lib.rs"
n2-sindri --files "src/**/*.ts"
```

The CLI writes a JSON `ScanResult` to stdout. Exit status 0 means the configured gate passed, 1 means the scan failed the gate, and 2 means invalid CLI usage or an output serialization error. Quote globs so the scanner receives the pattern. Commas separate file selections.

```rust
let result = n2_sindri::scan_dir("src");
println!("{}", result.report);
```

Public functions are `scan_dir`, `scan_files`, and `supports_path`. The report preserves Daebung's field names `passed`, `blockers`, `warnings`, `info`, `scanned_files`, and `report`. Rust callers also receive `scanned_paths`; this field is omitted from serialized reports.

## Checks and limits

The scanner reports syntax errors, selected dangerous constructs, excessive file/function length, repeated numeric literals and source hygiene findings. Read the rule identifiers in the report when configuring your quality gate. Lint suppressions use `sindri:ignore` above a node or `sindri:ignore-file` in the first five lines. Syntax and read failures remain blocking.

Each scan processes up to 5,000 candidate files and accepts files up to 2 MiB. Directory scanning skips symlink entries and dependency/build directories. Empty scans, unreadable files and malformed syntax fail the gate. Unknown file extensions are skipped.

This CLI reads paths selected by its caller. An application exposing it as an agent tool must apply that application's workspace and permission policy before invocation. Functional correctness, runtime behavior and dependency vulnerabilities require their own tests.

## Soul integration

Scanning is stateless and requires no database. A host can save its outcome through Soul 9.2's `n2_kv_save` MCP tool using the existing fields below. Populate the summary from the actual report, use the caller's agent/project scope and check Soul's save response.

```json
{
  "agent": "example-agent",
  "project": "example-project",
  "summary": "Sindri completed a source scan. Attach the actual pass/fail outcome and findings here.",
  "decisions": [],
  "todo": []
}
```

## Development

```sh
cargo test --locked
cargo package --locked
```

Tests cover AST rules, syntax errors, TSX parsing, missing files, invalid encoding, scan exclusions and CLI exit statuses. The source was extracted from N2 Daebung with the local tool-registry adapter separated from the standalone scanner.

MIT license. Copyright 2026 N2 AI Family. Dependencies retain their respective licenses.
