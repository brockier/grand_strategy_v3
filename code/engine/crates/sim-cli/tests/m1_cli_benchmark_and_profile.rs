#![deny(clippy::disallowed_types)]

use std::path::PathBuf;
use std::process::Command;

fn scenario_rel_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("../../../../mods/unexplained-colony")
}

// @req ENG-SCN-004
#[test]
fn headless_multi_decade_simulation_benchmark_completes_with_valid_metrics() {
    let bin = env!("CARGO_BIN_EXE_sim-cli");
    let sc_dir = scenario_rel_dir();

    let output = Command::new(bin)
        .args([
            "run",
            "--scenario",
            sc_dir.to_str().expect("utf8 path"),
            "--ticks",
            "87600",
            "--seed",
            "42",
        ])
        .output()
        .expect("spawning sim-cli must succeed");

    assert!(
        output.status.success(),
        "sim-cli exited non-zero: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("stdout must be valid JSON");
    assert_eq!(json["ok"], true);
    assert_eq!(json["ticksSimulated"], 87600);
    assert_eq!(json["assertionsPassed"], true);
    assert!(
        json["populationSummary"]["totalPopulation"]
            .as_u64()
            .unwrap_or(0)
            > 0
    );
    assert!(
        json["ticksPerSecond"].as_f64().unwrap_or(0.0) > 0.0,
        "ticksPerSecond must be positive"
    );
    assert!(json["stateDigest"].as_str().is_some_and(|s| s.len() == 64));
}

// @req ENG-CLK-006
#[test]
fn profiling_telemetry_outputs_subsystem_breakdowns_without_changing_state_hash() {
    let bin = env!("CARGO_BIN_EXE_sim-cli");
    let sc_dir = scenario_rel_dir();
    let sc_str = sc_dir.to_str().expect("utf8 path");

    let plain_out = Command::new(bin)
        .args(["run", "--scenario", sc_str, "--ticks", "720", "--seed", "42"])
        .output()
        .expect("spawning unprofiled sim-cli");
    assert!(plain_out.status.success());
    let plain_json: serde_json::Value =
        serde_json::from_slice(&plain_out.stdout).expect("plain JSON");

    let prof_out = Command::new(bin)
        .args([
            "run",
            "--scenario",
            sc_str,
            "--ticks",
            "720",
            "--seed",
            "42",
            "--profile",
            "--profile-human",
        ])
        .output()
        .expect("spawning profiled sim-cli");
    assert!(prof_out.status.success());
    let prof_json: serde_json::Value =
        serde_json::from_slice(&prof_out.stdout).expect("profiled JSON");

    assert_eq!(
        plain_json["stateDigest"], prof_json["stateDigest"],
        "enabling telemetry profiling must never mutate deterministic simulation hash"
    );

    let telem = &prof_json["telemetry"];
    assert_eq!(telem["ticksProfiled"], 720);
    for key in [
        "weatherMs",
        "transportMs",
        "productionMs",
        "populationMs",
        "playerViewMs",
    ] {
        assert!(
            telem[key].as_f64().is_some(),
            "missing JSON telemetry field '{key}'"
        );
    }

    let stderr_text = String::from_utf8_lossy(&prof_out.stderr);
    assert!(
        stderr_text.contains("weather") && stderr_text.contains("player_view"),
        "human-readable telemetry report missing expected phase labels: {stderr_text}"
    );
}
