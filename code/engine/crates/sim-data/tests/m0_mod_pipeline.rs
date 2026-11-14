//! Automated Milestone 0 tests for Draft 2020-12 JSON Schema export,
//! line-accurate diagnostics on malformed mod files, and deterministic 5-layer overrides.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sim_data::{DiagnosticSeverity, export_json_schemas, load_mod_bundle};
use sim_spine::ModRegistryPort;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .expect("sim-data sits at code/engine/crates/sim-data")
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

fn copy_dir_recursive(src: &Path, dst: &Path, reverse_order: bool) {
    fs::create_dir_all(dst).unwrap();
    let mut entries: Vec<PathBuf> = fs::read_dir(src)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    if reverse_order {
        entries.reverse();
    }
    for p in entries {
        let name = p.file_name().unwrap();
        let target = dst.join(name);
        if p.is_dir() {
            copy_dir_recursive(&p, &target, reverse_order);
        } else {
            fs::copy(&p, &target).unwrap();
        }
    }
}

// @req ENG-MOD-002
#[test]
fn schema_export_produces_draft_2020_12_and_matches_committed_schemas_byte_for_byte() {
    let root = repo_root();
    let committed_dir = root.join("schemas");
    let tmp = unique_temp_dir("ng_schema_roundtrip");

    let generated = export_json_schemas(&tmp).expect("export_json_schemas must succeed");
    assert_eq!(generated.len(), 12);

    for gen_path in generated {
        let file_name = gen_path.file_name().unwrap();
        let gen_bytes = fs::read(&gen_path).unwrap();
        let schema_json: serde_json::Value = serde_json::from_slice(&gen_bytes).unwrap();
        assert_eq!(
            schema_json["$schema"].as_str(),
            Some("https://json-schema.org/draft/2020-12/schema"),
            "schema {} must declare Draft 2020-12",
            file_name.to_string_lossy()
        );
        let committed_path = committed_dir.join(file_name);
        let committed_bytes = fs::read(&committed_path).unwrap_or_else(|_| {
            panic!("Missing committed schema {}", committed_path.display())
        });
        assert_eq!(
            gen_bytes,
            committed_bytes,
            "committed schema {} drifted from Rust struct definitions",
            file_name.to_string_lossy()
        );
    }

    let _ = fs::remove_dir_all(&tmp);
}

// @req ENG-MOD-003
#[test]
fn malformed_toml_and_json_mod_files_emit_exact_line_col_diagnostics_without_panics() {
    let root = repo_root();
    let tmp = unique_temp_dir("ng_bad_mods");
    let core_copy = tmp.join("core");
    copy_dir_recursive(&root.join("mods/core-schemas"), &core_copy, false);

    // Case 1: Out-of-range negative mass on an explicit known line in a layer override pack
    let bad_layer = tmp.join("bad_layer");
    fs::create_dir_all(bad_layer.join("commodities")).unwrap();
    fs::write(
        bad_layer.join("pack.toml"),
        "schema_version = 1\npack_id = \"bad-test\"\ndisplay_name = \"Bad Test\"\nlayer = \"UserMod\"\nload_priority = 10\n",
    )
    .unwrap();
    let bad_toml_path = bad_layer.join("commodities/broken.toml");
    fs::write(
        &bad_toml_path,
        "[[commodities]]\nid = \"raw_grain\"\ndisplay_name = \"Bad Grain\"\ncategory = \"Food\"\nmass_per_unit_kg = -5.0\nvolume_per_unit_m3 = 0.001\nspoilage_rate_per_hour = 0.0\n",
    )
    .unwrap();

    let errs = load_mod_bundle(&[core_copy.as_path(), bad_layer.as_path()])
        .expect_err("negative mass must be rejected");
    let mass_diag = errs
        .iter()
        .find(|d| d.field_path.contains("mass_per_unit_kg"))
        .expect("expected diagnostic for mass_per_unit_kg");
    assert_eq!(mass_diag.severity, DiagnosticSeverity::Error);
    assert_eq!(mass_diag.line, 5);
    assert!(mass_diag.file_path.ends_with("broken.toml"));

    // Case 2: Unknown field rejected by deny_unknown_fields with exact line number
    fs::write(
        &bad_toml_path,
        "[[commodities]]\nid = \"raw_grain\"\n_patch = true\nbogus_unknown_key = 99\n",
    )
    .unwrap();
    let errs_unknown = load_mod_bundle(&[core_copy.as_path(), bad_layer.as_path()])
        .expect_err("unknown field must be rejected");
    let unk_diag = errs_unknown
        .iter()
        .find(|d| d.message.contains("bogus_unknown_key") || d.field_path.contains("bogus_unknown_key"))
        .expect("expected unknown field diagnostic");
    assert_eq!(unk_diag.line, 4);

    // Case 3: Type mismatch inside TOML field
    fs::write(
        &bad_toml_path,
        "[[commodities]]\nid = \"raw_grain\"\n_patch = true\nmass_per_unit_kg = \"not-a-float\"\n",
    )
    .unwrap();
    let errs_type = load_mod_bundle(&[core_copy.as_path(), bad_layer.as_path()])
        .expect_err("type mismatch must be rejected");
    let type_diag = errs_type
        .iter()
        .find(|d| d.field_path.contains("mass_per_unit_kg"))
        .expect("expected type mismatch diagnostic");
    assert_eq!(type_diag.line, 4);

    // Case 4: Broken TOML syntax + broken JSON syntax
    fs::write(&bad_toml_path, "[[commodities]\nbroken = [1, 2\n").unwrap();
    let errs_syn = load_mod_bundle(&[core_copy.as_path(), bad_layer.as_path()])
        .expect_err("TOML syntax error must not panic");
    assert_eq!(errs_syn[0].line, 1);

    fs::remove_file(&bad_toml_path).unwrap();
    let bad_json_path = bad_layer.join("commodities/broken.json");
    fs::write(&bad_json_path, "{\n  \"commodities\": [\n    broken_json\n  ]\n}").unwrap();
    let errs_json = load_mod_bundle(&[core_copy.as_path(), bad_layer.as_path()])
        .expect_err("JSON syntax error must return exact line");
    assert_eq!(errs_json[0].line, 3);

    let _ = fs::remove_dir_all(&tmp);
}

// @req ENG-MOD-004
#[test]
fn five_layer_override_priority_and_directory_permutation_yield_identical_hashes() {
    let root = repo_root();
    let tmp = unique_temp_dir("ng_five_layers");

    let run_with_order = |sub: &str, reverse: bool| {
        let base = tmp.join(sub);
        let l0 = base.join("l0_core");
        let l1 = base.join("l1_era");
        let l2 = base.join("l2_scenario");
        let l3 = base.join("l3_user_mod");
        let l4 = base.join("l4_ui_skin");

        copy_dir_recursive(&root.join("mods/core-schemas"), &l0, reverse);
        copy_dir_recursive(&root.join("mods/baseline-era"), &l1, reverse);
        copy_dir_recursive(&root.join("mods/unexplained-colony"), &l2, reverse);

        fs::create_dir_all(l3.join("commodities")).unwrap();
        fs::write(
            l3.join("pack.toml"),
            "schema_version = 1\npack_id = \"custom-user\"\ndisplay_name = \"User Mod\"\nlayer = \"UserMod\"\nload_priority = 5\n",
        )
        .unwrap();
        fs::write(
            l3.join("commodities/patch.toml"),
            "[[commodities]]\nid = \"wrought_iron\"\n_patch = true\nstructural_strength = 0.91\n",
        )
        .unwrap();

        fs::create_dir_all(&l4).unwrap();
        fs::write(
            l4.join("pack.toml"),
            "schema_version = 1\npack_id = \"custom-skin-ref\"\ndisplay_name = \"Skin Ref\"\nlayer = \"UiSkin\"\nload_priority = 0\n",
        )
        .unwrap();
        fs::write(
            l4.join("skin.toml"),
            "[skin_reference]\nid = \"default_ui_skin\"\n_patch = true\ndefault_skin_id = \"cartographer-parchment-alt\"\n",
        )
        .unwrap();

        // Pass packs in intentionally scrambled order to prove layer sorting normalizes load order
        load_mod_bundle(&[
            l4.as_path(),
            l2.as_path(),
            l0.as_path(),
            l3.as_path(),
            l1.as_path(),
        ])
        .expect("5-layer merge must succeed")
    };

    let b1 = run_with_order("forward", false);
    let b2 = run_with_order("reversed", true);

    assert_eq!(b1.blake3_digest(), b2.blake3_digest(), "merged hash must be order-invariant");
    assert_eq!(b1.scenario_init().scenario_key, "unexplained_colony_v1");
    let iron = b1
        .merged_doc
        .commodities_and_knowledge
        .commodities
        .iter()
        .find(|c| c.id == "wrought_iron")
        .unwrap();
    assert!((iron.structural_strength - 0.91).abs() < 1e-6);
    assert_eq!(
        b1.merged_doc
            .skin_ref
            .skin_reference
            .as_ref()
            .unwrap()
            .default_skin_id,
        "cartographer-parchment-alt"
    );

    let _ = fs::remove_dir_all(&tmp);
}
