//! `cargo xtask` CLI entrypoint for licensing, isolation, asset provenance, notices, and traceability checks.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use xtask::{
    bundle_notices, check_isolation, check_licenses, check_requirements_traceability,
    parse_milestone_num, verify_assets,
};

fn default_engine_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("xtask sits at code/engine/tools/xtask")
        .to_path_buf()
}

fn default_repo_root(engine_dir: &Path) -> PathBuf {
    engine_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("engine sits at code/engine")
        .to_path_buf()
}

fn flag_val(args: &[String], key: &str) -> Option<String> {
    let mut i = 0;
    while i + 1 < args.len() {
        if args[i] == key {
            return Some(args[i + 1].clone());
        }
        i += 1;
    }
    None
}

fn print_errors(errors: &[String]) {
    for err in errors {
        eprintln!("ERROR: {err}");
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: cargo xtask <check-licenses|check-isolation|verify-assets|bundle-notices|check-reqs>"
        );
        return ExitCode::from(2);
    }

    let engine_dir = default_engine_dir();
    let repo_root = flag_val(&args[2..], "--repo-root")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_repo_root(&engine_dir));

    match args[1].as_str() {
        "check-licenses" => {
            let deny_toml = flag_val(&args[2..], "--deny-toml")
                .map(PathBuf::from)
                .unwrap_or_else(|| engine_dir.join("deny.toml"));
            let meta_override = flag_val(&args[2..], "--metadata-json")
                .and_then(|p| fs::read_to_string(p).ok());
            match check_licenses(&engine_dir, &deny_toml, meta_override.as_deref()) {
                Ok(count) => {
                    println!("check-licenses PASS ({count} external crates verified against allowlist)");
                    ExitCode::SUCCESS
                }
                Err(errs) => {
                    print_errors(&errs);
                    ExitCode::FAILURE
                }
            }
        }
        "check-isolation" => {
            let meta_override = flag_val(&args[2..], "--metadata-json")
                .and_then(|p| fs::read_to_string(p).ok());
            match check_isolation(&engine_dir, meta_override.as_deref()) {
                Ok(()) => {
                    println!("check-isolation PASS (Spine DAG and zero Steamworks core boundary intact)");
                    ExitCode::SUCCESS
                }
                Err(errs) => {
                    print_errors(&errs);
                    ExitCode::FAILURE
                }
            }
        }
        "verify-assets" => {
            let manifest = flag_val(&args[2..], "--manifest")
                .map(PathBuf::from)
                .unwrap_or_else(|| repo_root.join("tools/asset-pipeline/manifests/provenance.toml"));
            let out_file = flag_val(&args[2..], "--out")
                .map(PathBuf::from)
                .unwrap_or_else(|| repo_root.join("dist/steam_ai_disclosure.json"));
            match verify_assets(&repo_root, &manifest, &out_file) {
                Ok(count) => {
                    println!(
                        "verify-assets PASS ({count} commercial-verified assets -> {})",
                        out_file.display()
                    );
                    ExitCode::SUCCESS
                }
                Err(errs) => {
                    print_errors(&errs);
                    ExitCode::FAILURE
                }
            }
        }
        "bundle-notices" => {
            let out_file = flag_val(&args[2..], "--out")
                .map(PathBuf::from)
                .unwrap_or_else(|| repo_root.join("dist/THIRD_PARTY_NOTICES.txt"));
            match bundle_notices(&engine_dir, &out_file, None) {
                Ok(count) => {
                    println!(
                        "bundle-notices PASS ({count} crates written to {})",
                        out_file.display()
                    );
                    ExitCode::SUCCESS
                }
                Err(errs) => {
                    print_errors(&errs);
                    ExitCode::FAILURE
                }
            }
        }
        "check-reqs" => {
            let ms_str = flag_val(&args[2..], "--up-to-milestone").unwrap_or_else(|| "M0".to_string());
            let milestone = match parse_milestone_num(&ms_str) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("ERROR: {e}");
                    return ExitCode::from(2);
                }
            };
            let eng_path = repo_root.join("requirements/engine_requirements.md");
            let ui_path = repo_root.join("requirements/ui_requirements.md");
            let eng_raw = match fs::read_to_string(&eng_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("ERROR: Failed reading {}: {e}", eng_path.display());
                    return ExitCode::FAILURE;
                }
            };
            let ui_raw = match fs::read_to_string(&ui_path) {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("ERROR: Failed reading {}: {e}", ui_path.display());
                    return ExitCode::FAILURE;
                }
            };
            let code_engine = repo_root.join("code/engine");
            let code_ui = repo_root.join("code/ui");
            let roots = [code_engine.as_path(), code_ui.as_path()];
            match check_requirements_traceability(
                &[
                    ("engine_requirements.md", &eng_raw),
                    ("ui_requirements.md", &ui_raw),
                ],
                &roots,
                milestone,
            ) {
                Ok((reqs, cites)) => {
                    println!(
                        "check-reqs PASS ({reqs} Active M0..M{milestone} requirements bound across {cites} test citations)"
                    );
                    ExitCode::SUCCESS
                }
                Err(errs) => {
                    print_errors(&errs);
                    ExitCode::FAILURE
                }
            }
        }
        other => {
            eprintln!("Unknown xtask subcommand: {other}");
            ExitCode::from(2)
        }
    }
}
