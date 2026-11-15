#![deny(clippy::disallowed_types)]

use std::fs;
use std::path::{Path, PathBuf};

use sim_data::{load_mod_bundle, DiagnosticSeverity};

fn repo_mods_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../../mods")
        .canonicalize()
        .expect("mods directory should resolve")
}

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "ng_{label}_{}",
            std::process::id()
        ));
        if dir.exists() {
            fs::remove_dir_all(&dir).expect("clean stale temp dir");
        }
        fs::create_dir_all(&dir).expect("create temp dir");
        Self { path: dir }
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("create target dir");
    for entry in fs::read_dir(src).expect("read source dir") {
        let entry = entry.expect("dir entry");
        let ty = entry.file_type().expect("file type");
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).expect("copy file");
        }
    }
}

fn load_three_pack_bundle(mods_root: &Path) -> Result<sim_data::CompiledModBundle, Vec<sim_data::ModDiagnostic>> {
    let core = mods_root.join("core-schemas");
    let era = mods_root.join("baseline-era");
    let scen = mods_root.join("unexplained-colony");
    load_mod_bundle(&[core.as_path(), era.as_path(), scen.as_path()])
}

// @req ENG-MOD-005
#[test]
fn mod_loader_rejects_broken_cross_domain_foreign_keys_before_world_init() {
    let src_mods = repo_mods_dir();

    // Case 1: missing process input commodity
    {
        let tmp = TempDirGuard::new("eng_mod_005_missing_input");
        copy_dir_recursive(&src_mods, &tmp.path);
        let recipes_path = tmp.path.join("core-schemas/processes/recipes.toml");
        let recipes = fs::read_to_string(&recipes_path).expect("read recipes");
        let mutated = recipes.replacen(
            "commodity = \"timber\"",
            "commodity = \"nonexistent_ether_ore\"",
            1,
        );
        assert_ne!(recipes, mutated, "replacement should modify recipes.toml");
        fs::write(&recipes_path, mutated).expect("write mutated recipes");

        let diags = load_three_pack_bundle(&tmp.path)
            .expect_err("broken process input commodity must be rejected");
        assert!(
            diags.iter().any(|d| {
                d.severity == DiagnosticSeverity::Error
                    && d.message.contains("undefined commodity 'nonexistent_ether_ore'")
            }),
            "expected undefined input commodity diagnostic, got {diags:?}"
        );
    }

    // Case 2: undefined process required_vocation
    {
        let tmp = TempDirGuard::new("eng_mod_005_missing_vocation");
        copy_dir_recursive(&src_mods, &tmp.path);
        let recipes_path = tmp.path.join("core-schemas/processes/recipes.toml");
        let recipes = fs::read_to_string(&recipes_path).expect("read recipes");
        let mutated = recipes.replacen(
            "required_vocation = \"miner\"",
            "required_vocation = \"astral_alchemist\"",
            1,
        );
        assert_ne!(recipes, mutated, "replacement should modify vocation");
        fs::write(&recipes_path, mutated).expect("write mutated recipes");

        let diags = load_three_pack_bundle(&tmp.path)
            .expect_err("undefined process vocation must be rejected");
        assert!(
            diags.iter().any(|d| {
                d.severity == DiagnosticSeverity::Error
                    && d.message.contains("undefined vocation 'astral_alchemist'")
            }),
            "expected undefined vocation diagnostic, got {diags:?}"
        );
    }

    // Case 3: unknown carrier fuel commodity
    {
        let tmp = TempDirGuard::new("eng_mod_005_missing_carrier_fuel");
        copy_dir_recursive(&src_mods, &tmp.path);
        let carriers_path = tmp.path.join("core-schemas/logistics/carriers.toml");
        let carriers = fs::read_to_string(&carriers_path).expect("read carriers");
        let mutated = carriers.replacen(
            "fodder_or_fuel_commodity = \"raw_grain\"",
            "fodder_or_fuel_commodity = \"phantom_kerosene\"",
            1,
        );
        assert_ne!(carriers, mutated, "replacement should modify carrier fuel");
        fs::write(&carriers_path, mutated).expect("write mutated carriers");

        let diags = load_three_pack_bundle(&tmp.path)
            .expect_err("unknown carrier fuel commodity must be rejected");
        assert!(
            diags.iter().any(|d| {
                d.severity == DiagnosticSeverity::Error
                    && d.message.contains("undefined fuel commodity 'phantom_kerosene'")
            }),
            "expected undefined fuel commodity diagnostic, got {diags:?}"
        );
    }
}

// @req ENG-MOD-006
#[test]
fn all_eleven_externalized_domains_are_populated_and_required() {
    let src_mods = repo_mods_dir();
    let bundle = load_three_pack_bundle(&src_mods).expect("baseline three-pack bundle loads cleanly");
    let doc = &bundle.merged_doc;

    assert!(doc.planet.planet_generation.is_some(), "domain 1 planet missing");
    assert!(!doc.commodities_and_knowledge.commodities.is_empty(), "domain 2 commodities missing");
    assert!(!doc.commodities_and_knowledge.tech_components.is_empty(), "domain 2 tech_components missing");
    assert!(!doc.commodities_and_knowledge.theory_domains.is_empty(), "domain 2 theory_domains missing");
    assert!(!doc.processes.processes.is_empty(), "domain 3 processes missing");
    assert!(!doc.logistics.carriers.is_empty(), "domain 4 carriers missing");
    assert!(!doc.logistics.infra_classes.is_empty(), "domain 4 infra_classes missing");
    assert!(!doc.population.cohort_archetypes.is_empty(), "domain 5 cohort_archetypes missing");
    assert!(!doc.population.vocations.is_empty(), "domain 5 vocations missing");
    assert!(!doc.characters.traits.is_empty(), "domain 6 traits missing");
    assert!(!doc.characters.succession_rules.is_empty(), "domain 6 succession_rules missing");
    assert!(!doc.governance.governance_models.is_empty(), "domain 7 governance_models missing");
    assert!(!doc.governance.policy_laws.is_empty(), "domain 7 policy_laws missing");
    assert!(!doc.warfare.formation_templates.is_empty(), "domain 8 formation_templates missing");
    assert!(!doc.warfare.doctrines.is_empty(), "domain 8 doctrines missing");
    assert!(doc.scenario_and_calendar.calendar.is_some(), "domain 9 calendar missing");
    assert!(doc.scenario_and_calendar.scenario.is_some(), "domain 9 scenario missing");
    assert!(
        !doc.scenario_and_calendar.starting_settlements.is_empty(),
        "domain 9 starting_settlements missing"
    );
    assert!(
        !doc.scenario_and_calendar.starting_characters.is_empty(),
        "domain 9 starting_characters missing"
    );
    assert!(!doc.notifications_and_loc.event_classes.is_empty(), "domain 10 event_classes missing");
    assert!(!doc.notifications_and_loc.strings.is_empty(), "domain 10 strings missing");
    assert!(doc.skin_ref.skin_reference.is_some(), "domain 11 skin_reference missing");

    // Deleting a required domain manifest fails cleanly with structured diagnostics.
    let tmp = TempDirGuard::new("eng_mod_006_missing_domain");
    copy_dir_recursive(&src_mods, &tmp.path);
    fs::remove_file(tmp.path.join("core-schemas/processes/recipes.toml"))
        .expect("remove recipes.toml");

    let diags = load_three_pack_bundle(&tmp.path)
        .expect_err("deleting a required domain manifest must fail validation cleanly");
    assert!(
        diags.iter().any(|d| d.severity == DiagnosticSeverity::Error),
        "expected error diagnostic when required domain is missing, got {diags:?}"
    );
}
