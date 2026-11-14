//! Headless simulation CLI and composition root wiring `sim-data`, `sim-core`, and `player-view`.

use std::env;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use player_view::{ModDiagnosticDto, ModValidationFailureState, PlayerKnowledgeArchive};
use sim_core::SimWorld;
use sim_data::{export_json_schemas, load_mod_bundle};
use sim_spine::ModRegistryPort;

fn to_diag_dtos(diags: Vec<sim_data::ModDiagnostic>) -> Vec<ModDiagnosticDto> {
    diags
        .into_iter()
        .map(|d| ModDiagnosticDto {
            severity: format!("{:?}", d.severity),
            layer: d.layer,
            file_path: d.file_path,
            line: d.line,
            column: d.column,
            field_path: d.field_path,
            message: d.message,
        })
        .collect()
}

fn parse_packs(args: &[String]) -> Vec<PathBuf> {
    let mut packs = Vec::new();
    let mut idx = 0usize;
    while idx < args.len() {
        if (args[idx] == "--pack" || args[idx] == "--scenario") && idx + 1 < args.len() {
            packs.push(PathBuf::from(&args[idx + 1]));
            idx += 2;
        } else {
            idx += 1;
        }
    }
    packs
}

fn parse_u64_flag(args: &[String], flag: &str, default: u64) -> u64 {
    let mut idx = 0usize;
    while idx + 1 < args.len() {
        if args[idx] == flag {
            return args[idx + 1].parse().unwrap_or(default);
        }
        idx += 1;
    }
    default
}

fn parse_str_flag(args: &[String], flag: &str) -> Option<String> {
    let mut idx = 0usize;
    while idx + 1 < args.len() {
        if args[idx] == flag {
            return Some(args[idx + 1].clone());
        }
        idx += 1;
    }
    None
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: sim-cli <export-schemas|validate-mods|dump-snapshot|run> [options]"
        );
        return ExitCode::from(2);
    }

    match args[1].as_str() {
        "export-schemas" => {
            let out_dir = parse_str_flag(&args[2..], "--out").unwrap_or_else(|| "schemas".to_string());
            match export_json_schemas(Path::new(&out_dir)) {
                Ok(files) => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": true,
                            "schemaCount": files.len(),
                            "outDir": out_dir
                        })
                    );
                    ExitCode::SUCCESS
                }
                Err(err) => {
                    eprintln!("{err}");
                    ExitCode::FAILURE
                }
            }
        }
        "validate-mods" => {
            let packs = parse_packs(&args[2..]);
            let pack_refs: Vec<&Path> = packs.iter().map(PathBuf::as_path).collect();
            match load_mod_bundle(&pack_refs) {
                Ok(bundle) => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": true,
                            "bundleHash": bundle.blake3_digest(),
                            "scenario": bundle.scenario_init().scenario_key
                        })
                    );
                    ExitCode::SUCCESS
                }
                Err(diags) => {
                    let payload = ModValidationFailureState {
                        ok: false,
                        diagnostics: to_diag_dtos(diags),
                    };
                    println!("{}", serde_json::to_string_pretty(&payload).unwrap_or_default());
                    ExitCode::FAILURE
                }
            }
        }
        "dump-snapshot" => {
            let packs = parse_packs(&args[2..]);
            let ticks = parse_u64_flag(&args[2..], "--ticks", 0);
            let seed = parse_u64_flag(&args[2..], "--seed", 42);
            let pack_refs: Vec<&Path> = packs.iter().map(PathBuf::as_path).collect();
            match load_mod_bundle(&pack_refs) {
                Ok(bundle) => {
                    let mut world = SimWorld::from_registry(&bundle, seed);
                    world.step_ticks(&bundle, ticks);
                    let mut archive = PlayerKnowledgeArchive::new();
                    let snap = archive.compile_snapshot(
                        world.current_tick(),
                        world.is_paused(),
                        &bundle,
                        &world,
                        &world,
                        &world,
                        &world,
                    );
                    println!("{}", serde_json::to_string_pretty(&snap).unwrap_or_default());
                    ExitCode::SUCCESS
                }
                Err(diags) => {
                    let payload = ModValidationFailureState {
                        ok: false,
                        diagnostics: to_diag_dtos(diags),
                    };
                    eprintln!("{}", serde_json::to_string_pretty(&payload).unwrap_or_default());
                    ExitCode::FAILURE
                }
            }
        }
        "run" => {
            let packs = parse_packs(&args[2..]);
            let ticks = parse_u64_flag(&args[2..], "--ticks", 720);
            let seed = parse_u64_flag(&args[2..], "--seed", 42);
            let pack_refs: Vec<&Path> = packs.iter().map(PathBuf::as_path).collect();
            match load_mod_bundle(&pack_refs) {
                Ok(bundle) => {
                    let started = Instant::now();
                    let mut world = SimWorld::from_registry(&bundle, seed);
                    world.step_ticks(&bundle, ticks);
                    let elapsed = started.elapsed().as_secs_f64().max(1e-9);
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": true,
                            "scenario": bundle.scenario_init().scenario_key,
                            "ticksSimulated": world.current_tick().get(),
                            "stateDigest": world.state_digest_hex(),
                            "ticksPerSecond": (ticks as f64 / elapsed).round()
                        })
                    );
                    ExitCode::SUCCESS
                }
                Err(diags) => {
                    let payload = ModValidationFailureState {
                        ok: false,
                        diagnostics: to_diag_dtos(diags),
                    };
                    eprintln!("{}", serde_json::to_string_pretty(&payload).unwrap_or_default());
                    ExitCode::FAILURE
                }
            }
        }
        other => {
            eprintln!("Unknown subcommand: {other}");
            ExitCode::from(2)
        }
    }
}
