//! Automated Milestone 0 test proving zero hardcoded game content in compiled Rust crates (`ENG-MOD-001`).

use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use sim_core::SimWorld;
use sim_data::load_mod_bundle;
use sim_spine::{ModRegistryPort, PhysicalLedgerPort};

fn unique_temp_dir(prefix: &str) -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("{prefix}_{}_{}", std::process::id(), nanos));
    fs::create_dir_all(&dir).unwrap();
    dir
}

// @req ENG-MOD-001
#[test]
fn alien_mod_pack_simulates_720_ticks_with_zero_hardcoded_engine_content() {
    // 1. Static audit confirming `sim-core` contains zero standard scenario/commodity keys.
    let sim_core_src = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join("sim-core/src/lib.rs");
    let core_text = fs::read_to_string(&sim_core_src).unwrap();
    for forbidden_literal in [
        "raw_grain",
        "hematite_ore",
        "wrought_iron",
        "haven_landing",
        "unexplained_colony",
        "mara_chen",
    ] {
        assert!(
            !core_text.contains(forbidden_literal),
            "sim-core must never hardcode content identifier '{forbidden_literal}'"
        );
    }

    // 2. Construct completely alien mod bundle replacing every Earth identifier.
    let tmp = unique_temp_dir("ng_alien_mod");
    fs::write(
        tmp.join("pack.toml"),
        "schema_version = 1\npack_id = \"xenon-world\"\ndisplay_name = \"Xenon World\"\nlayer = \"ScenarioPack\"\n",
    )
    .unwrap();
    fs::write(
        tmp.join("world.toml"),
        r#"[planet_generation]
id = "xenon_orb"
goldberg_subdivision = 2
tectonic_plate_count = 6
land_fraction_target = 0.45
axial_tilt_deg = 12.0
solar_constant_w_m2 = 900.0
rotation_period_hours = 18.0
mean_radius_km = 4200.0
base_sea_level_m = 0.0
max_orogenic_uplift_m = 5000.0
planetary_density_kg_m3 = 4800.0

[[commodities]]
id = "crystal_shards"
display_name = "Crystal Shards"
category = "Ore"
mass_per_unit_kg = 2.0
volume_per_unit_m3 = 0.001
spoilage_rate_per_hour = 0.0

[[commodities]]
id = "luminous_gel"
display_name = "Luminous Gel"
category = "Food"
mass_per_unit_kg = 1.0
volume_per_unit_m3 = 0.001
spoilage_rate_per_hour = 0.0001

[[vocations]]
id = "xenon_harvester"
display_name = "Xenon Harvester"
base_skill_gain_per_day = 0.002
skill_decay_per_day = 0.0002

[[processes]]
id = "thermal_extractor"
display_name = "Thermal Extractor"
required_vocation = "xenon_harvester"
cycle_hours = 24
workforce_hours_required = 10.0
weather_halt_mud_threshold = 0.9
[[processes.inputs]]
commodity = "crystal_shards"
amount_kg = 1.0
[[processes.outputs]]
commodity = "luminous_gel"
amount_kg = 3.0

[[carriers]]
id = "hover_sled"
display_name = "Hover Sled"
max_payload_kg = 900.0
max_payload_m3 = 2.0
base_speed_km_hr = 25.0
max_grade_rad = 0.5
mud_penalty_factor = 0.1
crew_required = 2
fodder_or_fuel_commodity = "crystal_shards"
fuel_burn_kg_per_hour = 0.2

[[infra_classes]]
id = "mag_track"
display_name = "Mag Track"
throughput_tonnes_per_hour = 40.0
speed_multiplier = 2.0
mud_shield_factor = 0.95
maintenance_timber_kg_per_week = 1.0

[[cohort_archetypes]]
id = "pods_adult"
display_name = "Pods Adult"
age_min_years = 10
age_max_years = 80
daily_grain_need_kg = 0.4
base_fertility_annual = 0.02
base_mortality_annual = 0.01
initial_literacy = 0.9

[[traits]]
id = "hive_focus"
display_name = "Hive Focus"
competence_delta = 0.1
precision_delta = 0.1
honesty_delta = 0.2
caution_delta = 0.0

[[succession_rules]]
id = "synth_inherit"
display_name = "Synthetic Transfer"
continuity_fraction = 1.0
transition_disruption_hours = 1

[[governance_models]]
id = "node_consensus"
display_name = "Node Consensus"
base_attention_capacity = 120.0
clerical_overhead_per_mandate = 2.0
hierarchy_hop_latency_hours = 1

[[policy_laws]]
id = "shared_pool"
display_name = "Shared Pool"
tax_or_tribute_rate = 0.0
patent_duration_years = 0

[[formation_templates]]
id = "drone_screen"
display_name = "Drone Screen"
personnel_count = 12
required_vocation = "xenon_harvester"
equipment_commodity = "crystal_shards"
equipment_kg_per_troop = 3.0

[[doctrines]]
id = "lattice_defense"
display_name = "Lattice Defense"
initiative_bonus = 0.2
strict_obedience_weight = 0.9
rain_penalty_mitigation = 0.5

[calendar]
id = "xenon_cycle"
hours_per_day = 24
days_per_year = 300
year_prefix = "Cycle"
starting_year = 400
month_names = ["Alpha", "Beta", "Gamma"]
month_lengths = [100, 100, 100]

[scenario]
id = "alien_outpost"
display_name = "Xenon Outpost"
starting_profile = "ExpeditionColony"
governance_model = "node_consensus"
succession_rule = "synth_inherit"
active_policy = "shared_pool"
hq_settlement_id = "xenon_outpost"
hq_sight_radius_cells = 1
unexplored_surrounding_cells = true

[[starting_settlements]]
id = "xenon_outpost"
display_name = "Xenon Outpost Alpha"
cell_index = 0
initial_population = 50
[[starting_settlements.stockpiles]]
commodity = "crystal_shards"
mass_kg = 500.0
purity = 1.0
[[starting_settlements.cohorts]]
archetype = "pods_adult"
vocation = "xenon_harvester"
count = 50
[[starting_settlements.initial_facilities]]
process = "thermal_extractor"
workers = 10

[[starting_characters]]
id = "zeta_nine"
display_name = "Zeta Nine"
role_title = "Node Steward"
home_settlement = "xenon_outpost"
competence = 0.85
precision_01 = 0.90
honesty_vs_sycophancy = 0.80
caution_vs_alarmism = 0.0
traits = ["hive_focus"]

[[event_classes]]
id = "crystal_alert"
display_name = "Crystal Alert"
default_tier = "LogOnly"
template_key = "tpl.crystal"

[[strings]]
id = "tpl.crystal"
text = "Thermal extraction cycled cleanly."

[skin_reference]
id = "alien_skin"
default_skin_id = "cartographer-parchment"
skins_root_rel = "skins"
"#,
    )
    .unwrap();

    let bundle = load_mod_bundle(&[tmp.as_path()]).expect("alien bundle must load cleanly");
    let mut world = SimWorld::from_registry(&bundle, 99);
    let gel_id = bundle.commodity_id_by_key("luminous_gel").unwrap();
    let stock_id = world.settlements()[0].id;
    let primary_stock = world.settlement_stockpiles(stock_id)[0];
    let initial_gel = world.stockpile_mass_kg(primary_stock, gel_id, None);
    assert_eq!(initial_gel, 0.0);

    world.step_ticks(&bundle, 720);
    assert_eq!(world.current_tick().get(), 720);
    let after_gel = world.stockpile_mass_kg(primary_stock, gel_id, None);
    assert!(after_gel > 0.0, "alien extractor process must have produced luminous_gel");

    let _ = fs::remove_dir_all(&tmp);
}
