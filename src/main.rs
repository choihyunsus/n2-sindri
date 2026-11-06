//! Command-line entry point for explicitly selected local source paths.
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() == 1 && matches!(args[0].as_str(), "--help" | "-h") {
        println!("n2-sindri --dir DIRECTORY\nn2-sindri --files PATH_OR_GLOB[,PATH...]");
        return ExitCode::SUCCESS;
    }
    if args.len() != 2 || args[1].trim().is_empty() {
        eprintln!("Expected --dir DIRECTORY or --files PATH_OR_GLOB[,PATH...]");
        return ExitCode::from(2);
    }
    let report = match args[0].as_str() {
        "--dir" => n2_sindri::scan_dir(&args[1]),
        "--files" => n2_sindri::scan_files(&args[1]),
        _ => {
            eprintln!("Unknown option; use --help");
            return ExitCode::from(2);
        }
    };
    match serde_json::to_string_pretty(&report) {
        Ok(json) => println!("{json}"),
        Err(error) => {
            eprintln!("Cannot serialize scan result: {error}");
            return ExitCode::from(2);
        }
    }
    ExitCode::from(u8::from(!report.passed))
}
