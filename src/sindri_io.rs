//! Bounded scan discovery, explicit read errors and workspace path boundaries.
use super::{analyze_file, build_report, detect_lang, v, Lang, ScanResult, Severity, Violation};
use std::path::PathBuf;

const MAX_FILES: usize = 5000;
const MAX_BYTES: u64 = 2 * 1024 * 1024;
const SKIP_DIRS: &[&str] = &[".git", "node_modules", "target", "vendor", "dist", "out", "__pycache__"];

fn expand_paths(input: &str) -> (Vec<PathBuf>, Vec<Violation>) {
    let mut paths = Vec::new();
    let mut violations = Vec::new();
    for item in input.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if !std::path::Path::new(item).exists() && (item.contains('*') || item.contains('?') || item.contains('[')) {
            match glob::glob(item) {
                Ok(entries) => for entry in entries {
                    match entry {
                        Ok(path) => paths.push(path),
                        Err(error) => violations.push(v("SCAN_ERROR", Severity::Blocker, "scan", 0, error.to_string())),
                    }
                    if paths.len() > MAX_FILES { break; }
                },
                Err(error) => violations.push(v("INVALID_GLOB", Severity::Blocker, "scan", 0, error.to_string())),
            }
        } else { paths.push(PathBuf::from(item)); }
        if paths.len() > MAX_FILES { break; }
    }
    (paths, violations)
}

fn scan_paths(paths: Vec<PathBuf>, mut violations: Vec<Violation>) -> ScanResult {
    let mut scanned = 0;
    let mut seen = std::collections::HashSet::new();
    let mut scanned_paths = Vec::new();
    if paths.len() > MAX_FILES {
        violations.push(v("SCAN_LIMIT", Severity::Blocker, "scan", 0, "Too many files; select a smaller source directory".into()));
    }
    for path in paths.into_iter().take(MAX_FILES) {
        let input_path = path.to_string_lossy();
        let resolved = path.canonicalize();
        let path = match resolved {
            Ok(path) => path,
            Err(error) => {
                violations.push(v("PATH_ERROR", Severity::Blocker, &input_path, 0, error.to_string()));
                continue;
            }
        };
        let path_text = path.to_string_lossy();
        if !seen.insert(path.clone()) || detect_lang(&path_text) == Lang::Unknown { continue; }
        match path.metadata() {
            Ok(meta) if meta.is_file() && meta.len() <= MAX_BYTES => {
                violations.extend(analyze_file(&path_text));
                scanned += 1;
                scanned_paths.push(path_text.to_string());
            }
            _ => violations.push(v("FILE_LIMIT", Severity::Blocker, &path_text, 0, "Not a regular readable source file or exceeds 2 MiB".into())),
        }
    }
    let mut report = build_report(&violations, scanned);
    report.scanned_paths = scanned_paths;
    report
}

pub fn scan_files(input: &str) -> ScanResult {
    let (paths, violations) = expand_paths(input);
    scan_paths(paths, violations)
}

#[cfg(test)]
#[path = "sindri_io_tests.rs"]
mod tests;

pub fn scan_dir(directory: &str) -> ScanResult {
    let mut stack = vec![PathBuf::from(directory)];
    let mut paths = Vec::new();
    let mut violations = Vec::new();
    while let Some(directory) = stack.pop() {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) => {
                violations.push(v("SCAN_ERROR", Severity::Blocker, &directory.to_string_lossy(), 0, error.to_string()));
                continue;
            }
        };
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => { violations.push(v("SCAN_ERROR", Severity::Blocker, "scan", 0, error.to_string())); continue; }
            };
            match entry.file_type() {
                Ok(kind) if kind.is_symlink() => {
                    violations.push(v("SYMLINK_SKIPPED", Severity::Info, &entry.path().to_string_lossy(), 0, "Directory scans do not follow symlinks".into()));
                }
                Ok(kind) if kind.is_dir() => {
                    if !SKIP_DIRS.contains(&entry.file_name().to_string_lossy().as_ref()) { stack.push(entry.path()); }
                }
                Ok(kind) if kind.is_file() && detect_lang(&entry.path().to_string_lossy()) != Lang::Unknown => paths.push(entry.path()),
                Ok(_) => {}
                Err(error) => violations.push(v("SCAN_ERROR", Severity::Blocker, "scan", 0, error.to_string())),
            }
            if paths.len() > MAX_FILES { break; }
        }
        if paths.len() > MAX_FILES { break; }
    }
    paths.sort();
    scan_paths(paths, violations)
}
