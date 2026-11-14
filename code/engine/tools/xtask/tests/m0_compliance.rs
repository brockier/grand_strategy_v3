//! Automated Milestone 0 tests for licensing, Steam isolation, AI asset provenance,
//! third-party notice aggregation, and `check-reqs` traceability.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use xtask::{
    bundle_notices, check_isolation, check_licenses, check_requirements_traceability,
    verify_assets,
};

fn engine_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("xtask sits inside code/engine/tools/xtask")
        .to_path_buf()
}

fn repo_root() -> PathBuf {
    engine_dir()
        .parent()
        .and_then(|p| p.parent())
        .expect("engine sits inside code/engine")
        .to_path_buf()
}

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos));
    fs::create_dir_all(&dir).unwrap();
    dir
}

// @req ENG-LIC-001
#[test]
fn license_allowlist_passes_workspace_and_rejects_poisoned_crates() {
    let eng = engine_dir();
    let deny = eng.join("deny.toml");
    let count = check_licenses(&eng, &deny, None).expect("clean workspace must pass deny.toml");
    assert!(count > 0, "expected external crates in lockfile");

    for bad_license_json in [
        r#"{"packages":[{"id":"ws#0.1.0","name":"ws","version":"0.1.0","license":"MIT","license_file":null,"repository":null,"authors":[],"manifest_path":"/tmp/Cargo.toml","dependencies":[]},{"id":"bad#1.0.0","name":"bad-gpl","version":"1.0.0","license":"GPL-3.0-only","license_file":null,"repository":null,"authors":[],"manifest_path":"/tmp/bad/Cargo.toml","dependencies":[]}],"workspace_members":["ws#0.1.0"]}"#,
        r#"{"packages":[{"id":"ws#0.1.0","name":"ws","version":"0.1.0","license":"MIT","license_file":null,"repository":null,"authors":[],"manifest_path":"/tmp/Cargo.toml","dependencies":[]},{"id":"bad#1.0.0","name":"bad-agpl","version":"1.0.0","license":"AGPL-3.0-or-later","license_file":null,"repository":null,"authors":[],"manifest_path":"/tmp/bad/Cargo.toml","dependencies":[]}],"workspace_members":["ws#0.1.0"]}"#,
        r#"{"packages":[{"id":"ws#0.1.0","name":"ws","version":"0.1.0","license":"MIT","license_file":null,"repository":null,"authors":[],"manifest_path":"/tmp/Cargo.toml","dependencies":[]},{"id":"bad#1.0.0","name":"bad-nc","version":"1.0.0","license":"CC-BY-NC-4.0","license_file":null,"repository":null,"authors":[],"manifest_path":"/tmp/bad/Cargo.toml","dependencies":[]}],"workspace_members":["ws#0.1.0"]}"#,
        r#"{"packages":[{"id":"ws#0.1.0","name":"ws","version":"0.1.0","license":"MIT","license_file":null,"repository":null,"authors":[],"manifest_path":"/tmp/Cargo.toml","dependencies":[]},{"id":"bad#1.0.0","name":"missing-spdx","version":"1.0.0","license":null,"license_file":null,"repository":null,"authors":[],"manifest_path":"/tmp/bad/Cargo.toml","dependencies":[]}],"workspace_members":["ws#0.1.0"]}"#,
    ] {
        let err = check_licenses(&eng, &deny, Some(bad_license_json))
            .expect_err("poisoned dependency metadata must be rejected");
        assert!(!err.is_empty());
    }
}

// @req ENG-LIC-002
#[test]
fn core_crates_compile_isolated_from_steamworks_and_siblings() {
    let eng = engine_dir();
    check_isolation(&eng, None).expect("clean workspace isolation check must pass");

    let steam_leak_fixture = r#"{
        "packages":[
            {"id":"sim-core#0.1.0","name":"sim-core","version":"0.1.0","license":"MIT","license_file":null,"repository":null,"authors":[],"manifest_path":"/tmp/Cargo.toml","dependencies":[{"name":"steam-adapter","kind":null}]},
            {"id":"steam-adapter#0.1.0","name":"steam-adapter","version":"0.1.0","license":"MIT","license_file":null,"repository":null,"authors":[],"manifest_path":"/tmp/Cargo.toml","dependencies":[]}
        ],
        "workspace_members":["sim-core#0.1.0","steam-adapter#0.1.0"]
    }"#;
    let err = check_isolation(&eng, Some(steam_leak_fixture))
        .expect_err("sim-core depending on steam-adapter must be rejected");
    assert!(err.iter().any(|m| m.contains("Steamworks") || m.contains("violates")));

    let sibling_leak_fixture = r#"{
        "packages":[
            {"id":"player-view#0.1.0","name":"player-view","version":"0.1.0","license":"MIT","license_file":null,"repository":null,"authors":[],"manifest_path":"/tmp/Cargo.toml","dependencies":[{"name":"sim-core","kind":null}]},
            {"id":"sim-core#0.1.0","name":"sim-core","version":"0.1.0","license":"MIT","license_file":null,"repository":null,"authors":[],"manifest_path":"/tmp/Cargo.toml","dependencies":[]}
        ],
        "workspace_members":["player-view#0.1.0","sim-core#0.1.0"]
    }"#;
    let err2 = check_isolation(&eng, Some(sibling_leak_fixture))
        .expect_err("player-view depending on sibling sim-core must be rejected");
    assert!(err2.iter().any(|m| m.contains("violates Spine isolation")));
}

// @req ENG-LIC-003
#[test]
fn ai_asset_provenance_validates_repo_and_rejects_unverified_or_orphan_assets() {
    let root = repo_root();
    let tmp = unique_temp_dir("ng_assets_test");
    let out1 = tmp.join("disclosure1.json");
    let out2 = tmp.join("disclosure2.json");

    let count = verify_assets(
        &root,
        &root.join("tools/asset-pipeline/manifests/provenance.toml"),
        &out1,
    )
    .expect("repo provenance manifest must verify cleanly");
    assert!(count >= 3);
    verify_assets(
        &root,
        &root.join("tools/asset-pipeline/manifests/provenance.toml"),
        &out2,
    )
    .unwrap();
    assert_eq!(fs::read(&out1).unwrap(), fs::read(&out2).unwrap(), "disclosure JSON must be deterministic");

    // Synthetic workspace with non-commercial flag and orphan binary PNG
    let fake_root = tmp.join("fake_repo");
    fs::create_dir_all(fake_root.join("skins/test_skin")).unwrap();
    fs::write(fake_root.join("skins/test_skin/tile.png"), b"PNG").unwrap();
    fs::write(fake_root.join("skins/test_skin/orphan.png"), b"PNG").unwrap();
    let bad_manifest = fake_root.join("bad_provenance.toml");
    fs::write(
        &bad_manifest,
        r#"schema_version = 1
[[assets]]
asset_rel_path = "skins/test_skin/tile.png"
generator_model = ""
generator_version = "1.0"
license_tier = "NonCommercial"
commercial_rights_verified = false
human_post_processing = "None"
steam_disclosure_category = "VisualTexture"
"#,
    )
    .unwrap();

    let errs = verify_assets(&fake_root, &bad_manifest, &tmp.join("bad_out.json"))
        .expect_err("non-commercial and orphan assets must fail audit");
    assert!(errs.iter().any(|e| e.contains("commercial_rights_verified")));
    assert!(errs.iter().any(|e| e.contains("generator_model")));
    assert!(errs.iter().any(|e| e.contains("orphan.png")));
    let _ = fs::remove_dir_all(&tmp);
}

// @req ENG-LIC-004
#[test]
fn third_party_notices_aggregates_100_percent_of_external_crates_deterministically() {
    let eng = engine_dir();
    let tmp = unique_temp_dir("ng_notices_test");
    let out1 = tmp.join("NOTICES_1.txt");
    let out2 = tmp.join("NOTICES_2.txt");

    let count1 = bundle_notices(&eng, &out1, None).expect("bundle-notices must succeed");
    let count2 = bundle_notices(&eng, &out2, None).expect("second bundle-notices run must succeed");
    assert_eq!(count1, count2);
    assert!(count1 > 0);

    let text1 = fs::read_to_string(&out1).unwrap();
    let text2 = fs::read_to_string(&out2).unwrap();
    assert_eq!(text1, text2, "notices output must be byte-identical");

    // Verify 100% match against cargo metadata non-workspace crate list
    let output = std::process::Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--locked"])
        .current_dir(&eng)
        .output()
        .unwrap();
    assert!(output.status.success());
    let val: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let ws: std::collections::BTreeSet<&str> = val["workspace_members"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    let mut external_count = 0usize;
    for pkg in val["packages"].as_array().unwrap() {
        let id = pkg["id"].as_str().unwrap();
        if ws.contains(id) {
            continue;
        }
        external_count += 1;
        let name = pkg["name"].as_str().unwrap();
        let ver = pkg["version"].as_str().unwrap();
        let header = format!("--- {name} v{ver} [");
        assert!(text1.contains(&header), "missing crate notice entry for {header}");
    }
    assert_eq!(count1, external_count);
    let _ = fs::remove_dir_all(&tmp);
}

fn synthetic_req_doc(entries: &[(&str, &str, &str, &str)]) -> String {
    let mut out = String::new();
    for (id, status, milestone, lineage) in entries {
        out.push_str(&format!(
            "### {id}: Synthetic Req\n- **ID:** `{id}`\n- **Status:** `{status}`\n- **Milestone:** `{milestone}`\n- **Verification Layer:** `Unit`\n- **Lineage:** `{lineage}`\n- **Requirement:** Must verify cleanly.\n- **Acceptance Criteria:** Exits cleanly.\n\n"
        ));
    }
    out
}

fn run_synthetic_traceability(doc: &str, test_body: &str, up_to: u8) -> Result<(usize, usize), Vec<String>> {
    let tmp = unique_temp_dir("ng_check_reqs");
    fs::write(tmp.join("sample_test.rs"), test_body).unwrap();
    let roots: [&Path; 1] = [tmp.as_path()];
    let res = check_requirements_traceability(&[("synthetic.md", doc)], &roots, up_to);
    let _ = fs::remove_dir_all(&tmp);
    res
}

// @req ENG-SCN-005
#[test]
fn check_reqs_auditor_enforces_coverage_and_rejects_stale_or_duplicate_citations() {
    let tag = format!("// {}", "@req");
    let valid_doc = synthetic_req_doc(&[
        ("ENG-LIC-001", "Active", "M0", "Origin"),
        ("ENG-LIC-002", "Draft", "M1", "Origin"),
    ]);
    let valid_test = format!("{tag} ENG-LIC-001\n#[test]\nfn ok() {{}}\n");
    let (reqs, cites) = run_synthetic_traceability(&valid_doc, &valid_test, 0).expect("valid M0 trace passes");
    assert_eq!(reqs, 1);
    assert_eq!(cites, 1);

    // 1. Orphan undefined ID fails
    let orphan_test = format!("{tag} ENG-LIC-001\n{tag} ENG-NOPE-999\n");
    let e_orphan = run_synthetic_traceability(&valid_doc, &orphan_test, 0).unwrap_err();
    assert!(e_orphan.iter().any(|s| s.contains("ENG-NOPE-999")));

    // 2. Uncovered Active M0 ID fails
    let e_uncov = run_synthetic_traceability(&valid_doc, "", 0).unwrap_err();
    assert!(e_uncov.iter().any(|s| s.contains("ENG-LIC-001") && s.contains("zero bound")));

    // 3. Duplicate ID fails
    let dup_doc = synthetic_req_doc(&[
        ("ENG-LIC-001", "Active", "M0", "Origin"),
        ("ENG-LIC-001", "Active", "M0", "Origin"),
    ]);
    let e_dup = run_synthetic_traceability(&dup_doc, &valid_test, 0).unwrap_err();
    assert!(e_dup.iter().any(|s| s.contains("Duplicate requirement ID")));

    // 4. Superseded / Removed ID citations fail
    let stale_doc = synthetic_req_doc(&[
        ("ENG-LIC-001", "Superseded", "M0", "Superseded-By: ENG-LIC-001.1"),
        ("ENG-LIC-001.1", "Active", "M0", "Origin"),
        ("ENG-LIC-003", "Removed", "M0", "Removed-Reason: Replaced"),
    ]);
    let stale_test = format!("{tag} ENG-LIC-001\n{tag} ENG-LIC-001.1\n{tag} ENG-LIC-003\n");
    let e_stale = run_synthetic_traceability(&stale_doc, &stale_test, 0).unwrap_err();
    assert!(e_stale.iter().any(|s| s.contains("Superseded")));
    assert!(e_stale.iter().any(|s| s.contains("Removed")));
}
