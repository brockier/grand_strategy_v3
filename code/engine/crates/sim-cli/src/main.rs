#![deny(clippy::disallowed_types)]

//! Headless simulation CLI and composition root wiring `sim-data`, `sim-scenario`, and `player-view`.

use std::env;
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

use player_view::{ModDiagnosticDto, ModValidationFailureState, ValidatedPlayerCommand};
use serde::Deserialize;
use sim_clock::ClockRunMode;
use sim_data::{export_json_schemas, load_mod_bundle};
use sim_scenario::{resolve_scenario_pack_stack, ScenarioWorld};
use sim_spine::{ModRegistryPort, PhysicalLedgerPort, PlanetGeometryPort, Tick};

#[derive(Deserialize)]
struct TimedCommandInput {
    tick: u64,
    command: ValidatedPlayerCommand,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum CommandBatchEntry {
    Timed(TimedCommandInput),
    Immediate(ValidatedPlayerCommand),
}

fn enqueue_commands_json(world: &mut ScenarioWorld, raw: &str) -> Result<(), String> {
    let entries: Vec<CommandBatchEntry> =
        serde_json::from_str(raw).map_err(|err| format!("Invalid --commands-json payload: {err}"))?;
    let default_tick = world.clock.current_tick.get() + 1;
    for entry in entries {
        let (target_tick, validated) = match entry {
            CommandBatchEntry::Timed(item) => (Tick(item.tick.max(1)), item.command),
            CommandBatchEntry::Immediate(cmd) => (Tick(default_tick), cmd),
        };
        let spine_cmd = validated.to_spine_command()?;
        world.enqueue_command(target_tick, spine_cmd);
    }
    Ok(())
}

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

fn parse_explicit_packs(args: &[String]) -> Vec<PathBuf> {
    let mut packs = Vec::new();
    let mut idx = 0usize;
    while idx < args.len() {
        if args[idx] == "--pack" && idx + 1 < args.len() {
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

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn resolve_packs_from_args(args: &[String]) -> Vec<PathBuf> {
    let explicit = parse_explicit_packs(args);
    let scenario = parse_str_flag(args, "--scenario").map(PathBuf::from);
    resolve_scenario_pack_stack(&explicit, scenario.as_deref())
}

fn emit_mod_failure_json(diags: Vec<sim_data::ModDiagnostic>, to_stdout: bool) -> ExitCode {
    let payload = ModValidationFailureState {
        ok: false,
        diagnostics: to_diag_dtos(diags),
    };
    let rendered = serde_json::to_string_pretty(&payload).unwrap_or_default();
    if to_stdout {
        println!("{rendered}");
    } else {
        eprintln!("{rendered}");
    }
    ExitCode::FAILURE
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!(
            "Usage: sim-cli <export-schemas|validate-mods|dump-snapshot|run|verify-determinism|serve-stdio> [options]"
        );
        return ExitCode::from(2);
    }

    match args[1].as_str() {
        "export-schemas" => {
            let out_dir =
                parse_str_flag(&args[2..], "--out").unwrap_or_else(|| "schemas".to_string());
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
            let packs = resolve_packs_from_args(&args[2..]);
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
                Err(diags) => emit_mod_failure_json(diags, true),
            }
        }
        "dump-snapshot" => {
            let sub = &args[2..];
            let packs = resolve_packs_from_args(sub);
            let ticks = parse_u64_flag(sub, "--ticks", 0);
            let seed = parse_u64_flag(sub, "--seed", 42);
            let commands_json = parse_str_flag(sub, "--commands-json");
            let pack_refs: Vec<&Path> = packs.iter().map(PathBuf::as_path).collect();
            match load_mod_bundle(&pack_refs) {
                Ok(bundle) => {
                    let mut world = ScenarioWorld::from_registry(&bundle, seed);
                    if let Some(ref raw_cmds) = commands_json
                        && let Err(err) = enqueue_commands_json(&mut world, raw_cmds)
                    {
                        eprintln!("{err}");
                        return ExitCode::FAILURE;
                    }
                    world.step_ticks(&bundle, ticks);
                    let snap = world.compile_diegetic_snapshot(&bundle);
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&snap).unwrap_or_default()
                    );
                    ExitCode::SUCCESS
                }
                Err(diags) => emit_mod_failure_json(diags, false),
            }
        }
        "run" => {
            let sub = &args[2..];
            let packs = resolve_packs_from_args(sub);
            let ticks = parse_u64_flag(sub, "--ticks", 720);
            let seed = parse_u64_flag(sub, "--seed", 42);
            let threads = parse_u64_flag(sub, "--threads", 0) as usize;
            let profile_json = has_flag(sub, "--profile");
            let profile_human = has_flag(sub, "--profile-human");
            let save_out = parse_str_flag(sub, "--save-out");
            let load_in = parse_str_flag(sub, "--load-in");
            let commands_json = parse_str_flag(sub, "--commands-json");

            let pack_refs: Vec<&Path> = packs.iter().map(PathBuf::as_path).collect();
            match load_mod_bundle(&pack_refs) {
                Ok(bundle) => {
                    let started = Instant::now();
                    let mut world = ScenarioWorld::from_registry(&bundle, seed);
                    if let Some(ref load_path) = load_in
                        && let Err(err) = world.load_from_path(Path::new(load_path), &bundle)
                    {
                        eprintln!("Failed loading snapshot {load_path}: {err}");
                        return ExitCode::FAILURE;
                    }
                    if let Some(ref raw_cmds) = commands_json
                        && let Err(err) = enqueue_commands_json(&mut world, raw_cmds)
                    {
                        eprintln!("{err}");
                        return ExitCode::FAILURE;
                    }
                    if threads > 0
                        && let Err(err) = world.set_worker_threads(threads)
                    {
                        eprintln!("Failed initializing worker pool: {err}");
                        return ExitCode::FAILURE;
                    }
                    if profile_json || profile_human {
                        world.set_profiling_enabled(true);
                    }
                    world.step_ticks(&bundle, ticks);
                    if let Err(err) = world.verify_invariants() {
                        eprintln!("Simulation invariant violation: {err}");
                        return ExitCode::FAILURE;
                    }
                    if let Some(ref out_path) = save_out
                        && let Err(err) = world.save_to_path(Path::new(out_path), &bundle)
                    {
                        eprintln!("Failed writing snapshot {out_path}: {err}");
                        return ExitCode::FAILURE;
                    }

                    let elapsed = started.elapsed().as_secs_f64().max(1e-9);
                    let mut out = serde_json::json!({
                        "ok": true,
                        "scenario": bundle.scenario_init().scenario_key,
                        "ticksSimulated": world.clock.current_tick.get(),
                        "stateDigest": world.state_digest_hex(),
                        "ticksPerSecond": (ticks as f64 / elapsed).round(),
                        "planetCells": world.planet.cell_count(),
                        "populationSummary": {
                            "totalPopulation": world.pop.total_population(),
                            "settlementCount": world.core.settlements().len(),
                            "cohortCount": world.pop.all_cohorts().len()
                        },
                        "economySummary": {
                            "totalStockpileMassKg": world.economy.total_stockpile_mass_kg(),
                            "facilityCount": world.economy.facilities().len(),
                            "conservationErrorKg": world.economy.conservation_error_kg()
                        },
                        "assertionsPassed": true
                    });

                    if profile_json
                        && let Some(obj) = out.as_object_mut()
                    {
                        obj.insert("telemetry".to_string(), world.telemetry.to_json_value());
                    }
                    if profile_human {
                        let report = world.telemetry.to_human_readable_report();
                        eprintln!("{report}");
                        if let Some(obj) = out.as_object_mut() {
                            obj.insert(
                                "profileHuman".to_string(),
                                serde_json::Value::String(report),
                            );
                        }
                    }

                    println!("{out}");
                    ExitCode::SUCCESS
                }
                Err(diags) => emit_mod_failure_json(diags, false),
            }
        }
        "verify-determinism" => {
            let sub = &args[2..];
            let packs = resolve_packs_from_args(sub);
            let ticks = parse_u64_flag(sub, "--ticks", 2160);
            let seed = parse_u64_flag(sub, "--seed", 42);
            let pack_refs: Vec<&Path> = packs.iter().map(PathBuf::as_path).collect();
            match load_mod_bundle(&pack_refs) {
                Ok(bundle) => {
                    let mut w1 = ScenarioWorld::from_registry(&bundle, seed);
                    let _ = w1.set_worker_threads(1);
                    w1.step_ticks(&bundle, ticks);

                    let mut w4 = ScenarioWorld::from_registry(&bundle, seed);
                    let _ = w4.set_worker_threads(4);
                    w4.step_ticks(&bundle, ticks);

                    let d1 = w1.state_digest_hex();
                    let d4 = w4.state_digest_hex();
                    let ok = d1 == d4;
                    println!(
                        "{}",
                        serde_json::json!({
                            "ok": ok,
                            "scenario": bundle.scenario_init().scenario_key,
                            "ticksSimulated": ticks,
                            "digestThreads1": d1,
                            "digestThreads4": d4
                        })
                    );
                    if ok {
                        ExitCode::SUCCESS
                    } else {
                        ExitCode::FAILURE
                    }
                }
                Err(diags) => emit_mod_failure_json(diags, false),
            }
        }
        "serve-stdio" => {
            let sub = &args[2..];
            let packs = resolve_packs_from_args(sub);
            let initial_ticks = parse_u64_flag(sub, "--ticks", 0);
            let seed = parse_u64_flag(sub, "--seed", 42);
            let commands_json = parse_str_flag(sub, "--commands-json");
            let pack_refs: Vec<&Path> = packs.iter().map(PathBuf::as_path).collect();
            match load_mod_bundle(&pack_refs) {
                Ok(bundle) => {
                    let mut world = ScenarioWorld::from_registry(&bundle, seed);
                    if let Some(ref raw_cmds) = commands_json
                        && let Err(err) = enqueue_commands_json(&mut world, raw_cmds)
                    {
                        eprintln!("{err}");
                        return ExitCode::FAILURE;
                    }
                    if initial_ticks > 0 {
                        world.step_ticks(&bundle, initial_ticks);
                    }

                    let stdin = io::stdin();
                    let stdout = io::stdout();
                    let mut out = io::BufWriter::new(stdout.lock());

                    for line_res in stdin.lock().lines() {
                        let Ok(line) = line_res else {
                            break;
                        };
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        let req: serde_json::Value = match serde_json::from_str(trimmed) {
                            Ok(v) => v,
                            Err(err) => {
                                let resp = serde_json::json!({
                                    "ok": false,
                                    "tick": world.clock.current_tick.get(),
                                    "errorCode": "INVALID_JSON",
                                    "message": format!("Invalid JSON request: {err}")
                                });
                                let _ = writeln!(out, "{resp}");
                                let _ = out.flush();
                                continue;
                            }
                        };

                        let op = req.get("op").and_then(|v| v.as_str()).unwrap_or("");
                        match op {
                            "quit" => {
                                let resp = serde_json::json!({
                                    "ok": true,
                                    "tick": world.clock.current_tick.get()
                                });
                                let _ = writeln!(out, "{resp}");
                                let _ = out.flush();
                                break;
                            }
                            "snapshot" => {
                                let snap = world.compile_diegetic_snapshot(&bundle);
                                let resp = serde_json::json!({
                                    "ok": true,
                                    "tick": world.clock.current_tick.get(),
                                    "snapshot": snap
                                });
                                let _ = writeln!(out, "{resp}");
                                let _ = out.flush();
                            }
                            "step" => {
                                let ticks = req
                                    .get("ticks")
                                    .and_then(|v| v.as_u64())
                                    .unwrap_or(1)
                                    .max(1);
                                for _ in 0..ticks {
                                    world.step_one_tick(&bundle);
                                    if matches!(world.clock.mode, ClockRunMode::AutoPaused { .. }) {
                                        break;
                                    }
                                }
                                let snap = world.compile_diegetic_snapshot(&bundle);
                                let resp = serde_json::json!({
                                    "ok": true,
                                    "tick": world.clock.current_tick.get(),
                                    "snapshot": snap
                                });
                                let _ = writeln!(out, "{resp}");
                                let _ = out.flush();
                            }
                            "command" => {
                                let raw_cmd = req
                                    .get("command")
                                    .cloned()
                                    .unwrap_or(serde_json::Value::Null);
                                match serde_json::from_value::<ValidatedPlayerCommand>(raw_cmd) {
                                    Ok(validated) => match validated.to_spine_command() {
                                        Ok(spine_cmd) => {
                                            world.apply_player_command_now(&bundle, spine_cmd);
                                            let snap = world.compile_diegetic_snapshot(&bundle);
                                            let resp = serde_json::json!({
                                                "ok": true,
                                                "tick": world.clock.current_tick.get(),
                                                "snapshot": snap
                                            });
                                            let _ = writeln!(out, "{resp}");
                                            let _ = out.flush();
                                        }
                                        Err(msg) => {
                                            let code = if msg.starts_with("ERR_") {
                                                msg.split(':').next().unwrap_or("INVALID_COMMAND")
                                            } else {
                                                "INVALID_COMMAND"
                                            };
                                            let resp = serde_json::json!({
                                                "ok": false,
                                                "tick": world.clock.current_tick.get(),
                                                "errorCode": code,
                                                "message": msg
                                            });
                                            let _ = writeln!(out, "{resp}");
                                            let _ = out.flush();
                                        }
                                    },
                                    Err(err) => {
                                        let resp = serde_json::json!({
                                            "ok": false,
                                            "tick": world.clock.current_tick.get(),
                                            "errorCode": "INVALID_COMMAND",
                                            "message": format!("Invalid command schema: {err}")
                                        });
                                        let _ = writeln!(out, "{resp}");
                                        let _ = out.flush();
                                    }
                                }
                            }
                            other => {
                                let resp = serde_json::json!({
                                    "ok": false,
                                    "tick": world.clock.current_tick.get(),
                                    "errorCode": "UNKNOWN_OP",
                                    "message": format!("Unknown serve-stdio op '{other}'")
                                });
                                let _ = writeln!(out, "{resp}");
                                let _ = out.flush();
                            }
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(diags) => emit_mod_failure_json(diags, true),
            }
        }
        other => {
            eprintln!("Unknown subcommand: {other}");
            ExitCode::from(2)
        }
    }
}
