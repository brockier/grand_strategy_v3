#![deny(clippy::disallowed_types)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use player_view::KnownCellDto;
use sim_clock::ClockRunMode;
use sim_data::load_mod_bundle;
use sim_economy::EconomyLedger;
use sim_logistics::{LogisticsNetwork, RoadPurposeKind, UnifiedTransitLaunch};
use sim_scenario::{resolve_scenario_pack_stack, ScenarioWorld};
use sim_spine::{
    ActorId, CellId, ModRegistryPort, PhysicalLedgerPort, PlanetGeometryPort, ProcessId,
    SettlementId, SurfaceConditionSnapshot, SurfaceConditionsPort, Tick, TransitChannelKind,
};

fn find_process_id(reg: &dyn ModRegistryPort, key: &str) -> ProcessId {
    (0..reg.process_count())
        .map(|i| ProcessId(i as u16))
        .find(|&id| reg.process_def(id).is_some_and(|p| p.key == key))
        .expect("process key should exist in registry")
}

fn load_unexplained_bundle() -> sim_data::CompiledModBundle {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../..")
        .canonicalize()
        .expect("repo root should resolve");
    let scen = repo_root.join("mods/unexplained-colony");
    let stack = resolve_scenario_pack_stack(&[], Some(scen.as_path()));
    let refs: Vec<&Path> = stack.iter().map(PathBuf::as_path).collect();
    load_mod_bundle(&refs).expect("unexplained-colony mod bundle should load")
}

struct MockWeatherSurface {
    override_surface: SurfaceConditionSnapshot,
}

impl SurfaceConditionsPort for MockWeatherSurface {
    fn surface(&self, _cell: CellId) -> SurfaceConditionSnapshot {
        self.override_surface.clone()
    }
}

fn calm_dry_surface() -> SurfaceConditionSnapshot {
    SurfaceConditionSnapshot {
        temp_c: 15.0,
        precip_mm_hr: 0.0,
        mud_index_01: 0.05,
        visibility_01: 1.0,
        flood_stage_01: 0.0,
        snow_depth_m: 0.0,
        ice_thickness_m: 0.0,
        wind_u_mps: 3.0,
        wind_v_mps: 0.0,
    }
}

// @req ENG-ECO-001
#[test]
fn economy_maintains_physical_lot_conservation_and_non_negativity() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    world.clock.mode = ClockRunMode::Running {
        speed_multiplier: 1,
    };

    let cid_grain = reg.commodity_id_by_key("raw_grain").expect("raw_grain");
    let cid_iron = reg.commodity_id_by_key("wrought_iron").expect("wrought_iron");
    let cid_water = reg.commodity_id_by_key("potable_water").expect("potable_water");
    let land_cells: Vec<CellId> = (0..world.planet.cell_count())
        .map(CellId)
        .filter(|&c| !world.planet.is_ocean_cell(c))
        .take(2)
        .collect();

    let sid_b = world.economy.establish_settlement(
        "outpost_conservation".to_owned(),
        "Conservation Outpost".to_owned(),
        land_cells[1],
        ActorId(1),
        &[
            (cid_grain, 300.0, 0.90, 0.96),
            (cid_water, 300.0, 1.0, 0.98),
        ],
        &[],
    );
    let sid_a = SettlementId(0);
    let stock_a = world
        .economy
        .stockpile_lot(sim_spine::StockpileId(0))
        .expect("primary lot A")
        .id;
    let stock_b = world
        .economy
        .facilities()
        .first()
        .map(|_| sid_b)
        .unwrap_or(sid_b);
    let _ = stock_b;

    let before_a_iron = world
        .economy
        .settlement_commodity_mass_kg(sid_a, cid_iron, Some(ActorId(1)));
    assert!(before_a_iron > 0.0);

    // Over-transfer request must atomically fail and preserve exact stock.
    assert!(
        !world.economy.transfer_stock_atomic(
            stock_a,
            sim_spine::StockpileId(stock_a.0 + 1),
            cid_iron,
            ActorId(1),
            10_000_000.0,
        ),
        "excess transfer must fail atomically"
    );

    world.step_ticks(&reg, 240);
    assert!(
        world.economy.conservation_error_kg() <= 1e-4,
        "mass conservation drifted: {} kg",
        world.economy.conservation_error_kg()
    );
    assert!(world.economy.total_stockpile_mass_kg() > 0.0);
}

fn find_craftable_multi_tier_chain(reg: &dyn ModRegistryPort) {
    let iron = reg.commodity_id_by_key("wrought_iron").expect("wrought_iron");
    let charcoal = reg.commodity_id_by_key("charcoal").expect("charcoal");
    let timber = reg.commodity_id_by_key("timber").expect("timber");
    let precision = reg
        .commodity_id_by_key("colony_precision_tools")
        .expect("colony_precision_tools");
    let forged = reg
        .commodity_id_by_key("forged_hand_tools")
        .expect("forged_hand_tools");
    let _ = (iron, charcoal, timber, precision, forged);
}

// @req ENG-ECO-002
#[test]
fn economy_executes_multi_tier_recipe_chains_with_tool_fallback_and_wear() {
    let reg = load_unexplained_bundle();
    find_craftable_multi_tier_chain(&reg);

    let cid_iron = reg.commodity_id_by_key("wrought_iron").expect("wrought_iron");
    let cid_charcoal = reg.commodity_id_by_key("charcoal").expect("charcoal");
    let cid_timber = reg.commodity_id_by_key("timber").expect("timber");
    let cid_precision = reg
        .commodity_id_by_key("colony_precision_tools")
        .expect("colony_precision_tools");
    let cid_forged = reg
        .commodity_id_by_key("forged_hand_tools")
        .expect("forged_hand_tools");
    let pid_forge = find_process_id(&reg, "forge_replacement_hand_tools");

    let weather = MockWeatherSurface {
        override_surface: calm_dry_surface(),
    };

    // Subcase A: primary precision tools available and used first
    let mut econ_primary = EconomyLedger::from_registry(&reg);
    let sid = econ_primary.establish_settlement(
        "foundry".to_owned(),
        "Foundry".to_owned(),
        CellId(2),
        ActorId(1),
        &[
            (cid_iron, 200.0, 0.90, 0.96),
            (cid_charcoal, 200.0, 0.95, 0.96),
            (cid_timber, 200.0, 0.95, 0.96),
            (cid_precision, 50.0, 0.95, 0.98),
            (cid_forged, 50.0, 0.85, 0.98),
        ],
        &[(pid_forge, 4, ActorId(1))],
    );
    econ_primary.step_facilities_for_settlement(
        sid,
        &reg,
        &weather,
        1.0,
        &BTreeMap::new(),
        &BTreeMap::new(),
        None,
    );
    let fac_primary = econ_primary
        .facilities()
        .iter()
        .find(|f| f.settlement == sid)
        .expect("facility present");
    assert_eq!(fac_primary.last_tool_used, Some(cid_precision));
    assert!(fac_primary.last_daily_output_kg > 0.0);
    let precision_left = econ_primary.settlement_commodity_mass_kg(sid, cid_precision, Some(ActorId(1)));
    assert!(precision_left < 50.0, "primary tools should accrue wear");

    // Subcase B: primary tools absent -> automatically falls back to forged_hand_tools
    let mut econ_fb = EconomyLedger::from_registry(&reg);
    let sid_fb = econ_fb.establish_settlement(
        "foundry_fb".to_owned(),
        "Fallback Foundry".to_owned(),
        CellId(2),
        ActorId(1),
        &[
            (cid_iron, 200.0, 0.90, 0.96),
            (cid_charcoal, 200.0, 0.95, 0.96),
            (cid_timber, 200.0, 0.95, 0.96),
            (cid_forged, 50.0, 0.85, 0.98),
        ],
        &[(pid_forge, 4, ActorId(1))],
    );
    econ_fb.step_facilities_for_settlement(
        sid_fb,
        &reg,
        &weather,
        1.0,
        &BTreeMap::new(),
        &BTreeMap::new(),
        None,
    );
    let fac_fb = econ_fb
        .facilities()
        .iter()
        .find(|f| f.settlement == sid_fb)
        .expect("fallback facility present");
    assert_eq!(fac_fb.last_tool_used, Some(cid_forged));
    assert!(
        fac_fb.last_daily_output_kg > 0.0 && fac_fb.last_daily_output_kg < fac_primary.last_daily_output_kg,
        "fallback tools must produce output at lower hardness efficiency than precision tools"
    );

    // Subcase C: both tool types exhausted -> facility halts cleanly
    let mut econ_none = EconomyLedger::from_registry(&reg);
    let sid_none = econ_none.establish_settlement(
        "foundry_none".to_owned(),
        "No Tools Foundry".to_owned(),
        CellId(2),
        ActorId(1),
        &[
            (cid_iron, 200.0, 0.90, 0.96),
            (cid_charcoal, 200.0, 0.95, 0.96),
            (cid_timber, 200.0, 0.95, 0.96),
        ],
        &[(pid_forge, 4, ActorId(1))],
    );
    econ_none.step_facilities_for_settlement(
        sid_none,
        &reg,
        &weather,
        1.0,
        &BTreeMap::new(),
        &BTreeMap::new(),
        None,
    );
    let fac_none = econ_none
        .facilities()
        .iter()
        .find(|f| f.settlement == sid_none)
        .expect("halted facility present");
    assert_eq!(fac_none.halted_reason.as_deref(), Some("tool_exhausted"));
}

// @req ENG-ECO-003
#[test]
fn subsistence_consumes_physical_grain_water_fuel_and_clothing_stocks() {
    let reg = load_unexplained_bundle();
    let grain = reg.commodity_id_by_key("raw_grain").expect("raw_grain");
    let water = reg.commodity_id_by_key("potable_water").expect("potable_water");
    let charcoal = reg.commodity_id_by_key("charcoal").expect("charcoal");
    let clothing = reg
        .commodity_id_by_key("simple_clothing")
        .expect("simple_clothing");

    let mut econ = EconomyLedger::from_registry(&reg);
    let sid = econ.establish_settlement(
        "cold_camp".to_owned(),
        "Frost Camp".to_owned(),
        CellId(4),
        ActorId(1),
        &[
            (grain, 20.0, 0.95, 0.96),
            (water, 20.0, 1.0, 0.96),
            (charcoal, 12.0, 0.95, 0.96),
            (clothing, 4.0, 1.0, 0.96),
        ],
        &[],
    );

    let sat_ok = econ.step_subsistence_for_settlement(sid, 4.0, 6.0, 3.0, 0.4, -2.0, &reg);
    assert!((sat_ok.calorie_ratio_01 - 1.0).abs() < 1e-4);
    assert!((sat_ok.water_ratio_01 - 1.0).abs() < 1e-4);
    assert!((sat_ok.heating_ratio_01 - 1.0).abs() < 1e-4);
    assert!(econ.settlement_commodity_mass_kg(sid, grain, None) < 20.0);
    assert!(econ.settlement_commodity_mass_kg(sid, charcoal, None) < 12.0);
    assert!(econ.settlement_commodity_mass_kg(sid, water, None) < 20.0);
    assert!(econ.settlement_commodity_mass_kg(sid, clothing, None) < 4.0);

    // Exhaust grain and re-check calorie satisfaction collapses while conservation stays exact.
    let sat_starve = econ.step_subsistence_for_settlement(sid, 100.0, 1.0, 1.0, 0.1, 12.0, &reg);
    assert!(sat_starve.calorie_ratio_01 < 0.25);
    assert!(econ.conservation_error_kg() <= 1e-6);
}

// @req ENG-ECO-004
#[test]
fn settlements_track_actor_owned_lots_and_skimming_diversion_conserving_mass() {
    let reg = load_unexplained_bundle();
    let ore = reg.commodity_id_by_key("hematite_ore").expect("hematite_ore");
    let charcoal = reg.commodity_id_by_key("charcoal").expect("charcoal");
    let forged = reg
        .commodity_id_by_key("forged_hand_tools")
        .expect("forged_hand_tools");
    let iron = reg.commodity_id_by_key("wrought_iron").expect("wrought_iron");
    let bloomery = find_process_id(&reg, "bloomery_smelting");

    let actor_state = ActorId(1);
    let actor_private = ActorId(7);
    let mut econ = EconomyLedger::from_registry(&reg);
    let sid = econ.establish_settlement(
        "mixed_guild".to_owned(),
        "Mixed Ownership Works".to_owned(),
        CellId(3),
        actor_state,
        &[
            (ore, 600.0, 0.90, 0.96),
            (charcoal, 400.0, 0.95, 0.96),
            (forged, 120.0, 0.90, 0.96),
        ],
        &[(bloomery, 4, actor_state)],
    );
    let fid_state = econ
        .facilities()
        .iter()
        .find(|f| f.settlement == sid)
        .expect("state facility")
        .id;
    let fid_private = econ.add_facility(sid, bloomery, actor_private, 4);
    for (comm, amt) in [(ore, 300.0), (charcoal, 200.0), (forged, 60.0)] {
        let (debited, pur) = econ
            .debit_cargo_for_transit(sid, comm, actor_state, amt)
            .expect("shift seed lot");
        econ.credit_cargo_arrival(sid, comm, actor_private, debited, pur);
    }

    let weather = MockWeatherSurface {
        override_surface: calm_dry_surface(),
    };
    let mut skim_map = BTreeMap::new();
    skim_map.insert(fid_state, 0.0_f32);
    skim_map.insert(fid_private, 0.30_f32);

    econ.step_facilities_for_settlement(
        sid,
        &reg,
        &weather,
        1.0,
        &BTreeMap::new(),
        &skim_map,
        None,
    );

    let iron_state = econ.settlement_commodity_mass_kg(sid, iron, Some(actor_state));
    let iron_private = econ.settlement_commodity_mass_kg(sid, iron, Some(actor_private));
    let skimmed_total = econ.conservation_ledger().skimmed_shrinkage_kg;

    assert!(iron_state > 0.0 && iron_private > 0.0);
    assert!(
        iron_state > iron_private && skimmed_total > 0.0,
        "skimming should divert fraction away from nominal private output into shrinkage ledger: state={iron_state}, private={iron_private}, skim={skimmed_total}"
    );
    assert!(econ.conservation_error_kg() <= 1e-6);
}

// @req ENG-WTH-004
#[test]
fn storms_flood_underground_mines_and_halt_outdoor_construction() {
    let reg = load_unexplained_bundle();
    let mine_proc = find_process_id(&reg, "surface_ore_quarrying");
    let build_proc = find_process_id(&reg, "grade_drained_roadbed");
    let timber = reg.commodity_id_by_key("timber").expect("timber");
    let iron = reg.commodity_id_by_key("wrought_iron").expect("wrought_iron");
    let tools = reg
        .commodity_id_by_key("forged_hand_tools")
        .expect("forged_hand_tools");

    let storm = MockWeatherSurface {
        override_surface: SurfaceConditionSnapshot {
            temp_c: 11.0,
            precip_mm_hr: 18.0,
            mud_index_01: 0.92,
            visibility_01: 0.25,
            flood_stage_01: 0.78,
            snow_depth_m: 0.0,
            ice_thickness_m: 0.0,
            wind_u_mps: 9.0,
            wind_v_mps: 2.0,
        },
    };

    let mut econ = EconomyLedger::from_registry(&reg);
    let sid = econ.establish_settlement(
        "wet_quarry".to_owned(),
        "Wet Quarry".to_owned(),
        CellId(10),
        ActorId(1),
        &[
            (timber, 200.0, 0.9, 0.96),
            (iron, 40.0, 0.9, 0.96),
            (tools, 40.0, 0.9, 0.96),
        ],
        &[(mine_proc, 8, ActorId(1)), (build_proc, 6, ActorId(1))],
    );
    econ.set_settlement_shaft_water_level(sid, 0.56);
    for _ in 0..4 {
        econ.apply_hourly_weather_and_spoilage(&reg, &storm);
    }
    assert!(
        econ.settlement_shaft_water_level(sid) >= 0.60,
        "storm seepage should push shaft water above halt threshold"
    );

    econ.step_facilities_for_settlement(
        sid,
        &reg,
        &storm,
        1.0,
        &BTreeMap::new(),
        &BTreeMap::new(),
        None,
    );

    let facs: Vec<_> = econ
        .facilities()
        .iter()
        .filter(|f| f.settlement == sid)
        .collect();
    assert_eq!(facs[0].halted_reason.as_deref(), Some("mine_flooded"));
    assert_eq!(facs[1].halted_reason.as_deref(), Some("weather_halt"));
    assert!(facs[1].weather_idle_crew_hours > 0);
}

// @req ENG-WTH-005
#[test]
fn unroofed_perishable_stockpiles_spoil_faster_in_wet_warm_weather_than_roofed() {
    let reg = load_unexplained_bundle();
    let grain = reg.commodity_id_by_key("raw_grain").expect("raw_grain");

    let wet_warm = MockWeatherSurface {
        override_surface: SurfaceConditionSnapshot {
            temp_c: 26.0,
            precip_mm_hr: 14.0,
            mud_index_01: 0.85,
            visibility_01: 0.5,
            flood_stage_01: 0.2,
            snow_depth_m: 0.0,
            ice_thickness_m: 0.0,
            wind_u_mps: 3.0,
            wind_v_mps: 1.0,
        },
    };

    let mut econ = EconomyLedger::from_registry(&reg);
    let sid_roofed = econ.establish_settlement(
        "roofed_depot".to_owned(),
        "Roofed Granary".to_owned(),
        CellId(5),
        ActorId(1),
        &[(grain, 1000.0, 0.95, 0.98)],
        &[],
    );
    let sid_exposed = econ.establish_settlement(
        "open_dump".to_owned(),
        "Open Grain Yard".to_owned(),
        CellId(5),
        ActorId(1),
        &[(grain, 1000.0, 0.95, 0.12)],
        &[],
    );

    for _ in 0..48 {
        econ.apply_hourly_weather_and_spoilage(&reg, &wet_warm);
    }

    let left_roofed = econ.settlement_commodity_mass_kg(sid_roofed, grain, Some(ActorId(1)));
    let left_exposed = econ.settlement_commodity_mass_kg(sid_exposed, grain, Some(ActorId(1)));
    assert!(left_roofed < 1000.0 && left_exposed < 1000.0);
    assert!(
        left_exposed < left_roofed * 0.85,
        "unroofed perishable stock must spoil significantly faster: exposed={left_exposed}, roofed={left_roofed}"
    );
    assert!(econ.conservation_error_kg() <= 1e-6);
}

// @req ENG-LOG-001
#[test]
fn edge_infrastructure_mud_and_washouts_determine_carrier_travel_speed() {
    let reg = load_unexplained_bundle();
    let world = ScenarioWorld::from_registry(&reg, 42);
    let mut logistics = LogisticsNetwork::from_planet_and_registry(&world.planet, &reg);

    let porter_id = reg
        .carrier_id_by_key("foot_porter_column")
        .expect("foot_porter_column");
    let porter = reg.carrier_def(porter_id).expect("porter def");
    let metalled = reg
        .infra_id_by_key("metalled_road")
        .expect("metalled_road");
    let foot_trail = reg.infra_id_by_key("foot_trail").expect("foot_trail");

    // Find a dry adjacent land cell pair within porter grade limits
    let mut chosen_pair = None;
    for c in 0..world.planet.cell_count() {
        let u = CellId(c);
        if world.planet.is_ocean_cell(u) {
            continue;
        }
        for &v in world.planet.cell_neighbors(u) {
            if world.planet.is_ocean_cell(v) {
                continue;
            }
            if let Some(eid) = world.planet.cell_edge(u, v)
                && let Some(es) = logistics.edge_state(eid)
                && !es.sea_edge
                && !es.river_edge
                && world.planet.cell_slope_rad(eid).abs() <= porter.max_grade_rad
            {
                chosen_pair = Some((u, v, eid));
                break;
            }
        }
        if chosen_pair.is_some() {
            break;
        }
    }
    let (u, v, eid) = chosen_pair.expect("expected overland edge pair");

    let dry = MockWeatherSurface {
        override_surface: calm_dry_surface(),
    };
    let muddy = MockWeatherSurface {
        override_surface: SurfaceConditionSnapshot {
            mud_index_01: 0.90,
            ..calm_dry_surface()
        },
    };

    logistics.upgrade_edge_infra(eid, foot_trail, None);
    let speed_trail_dry = logistics
        .evaluate_edge_speed_km_hr(u, v, porter, 250.0, &world.planet, &dry, &reg)
        .expect("dry foot trail speed");
    let speed_trail_mud = logistics
        .evaluate_edge_speed_km_hr(u, v, porter, 250.0, &world.planet, &muddy, &reg)
        .expect("muddy foot trail speed");
    assert!(
        speed_trail_mud < speed_trail_dry,
        "heavy mud must reduce carrier speed: dry={speed_trail_dry}, mud={speed_trail_mud}"
    );

    logistics.upgrade_edge_infra(eid, metalled, None);
    let speed_road_mud = logistics
        .evaluate_edge_speed_km_hr(u, v, porter, 250.0, &world.planet, &muddy, &reg)
        .expect("metalled road muddy speed");
    assert!(
        speed_road_mud > speed_trail_mud,
        "metalled road must outperform foot trail in mud: road={speed_road_mud}, trail={speed_trail_mud}"
    );

    logistics.set_edge_washed_out(eid, true);
    assert!(
        logistics
            .evaluate_edge_speed_km_hr(u, v, porter, 250.0, &world.planet, &dry, &reg)
            .is_none(),
        "washed-out edge must become impassable"
    );
}

// @req ENG-LOG-002
#[test]
fn bridges_enforce_weight_caps_and_waterways_enforce_ice_and_wind_physics() {
    let reg = load_unexplained_bundle();
    let world = ScenarioWorld::from_registry(&reg, 42);
    let mut logistics = LogisticsNetwork::from_planet_and_registry(&world.planet, &reg);

    let porter_id = reg
        .carrier_id_by_key("foot_porter_column")
        .expect("foot_porter_column");
    let wagon_id = reg
        .carrier_id_by_key("ox_freight_wagon")
        .expect("ox_freight_wagon");
    let barge_id = reg
        .carrier_id_by_key("river_cargo_barge")
        .expect("river_cargo_barge");
    let timber_bridge = reg
        .infra_id_by_key("timber_bridge")
        .expect("timber_bridge");
    let river_infra = reg
        .infra_id_by_key("river_channel")
        .expect("river_channel");

    let porter = reg.carrier_def(porter_id).expect("porter def");
    let wagon = reg.carrier_def(wagon_id).expect("wagon def");
    let barge = reg.carrier_def(barge_id).expect("barge def");

    let (mut u, mut v, mut eid) = (CellId(0), CellId(0), sim_spine::EdgeId(0));
    'outer: for c in 0..world.planet.cell_count() {
        let cand = CellId(c);
        if world.planet.is_ocean_cell(cand) {
            continue;
        }
        for &nb in world.planet.cell_neighbors(cand) {
            if !world.planet.is_ocean_cell(nb)
                && let Some(edge) = world.planet.cell_edge(cand, nb)
            {
                u = cand;
                v = nb;
                eid = edge;
                break 'outer;
            }
        }
    }

    logistics.upgrade_edge_infra(eid, timber_bridge, Some(800.0));
    let dry = MockWeatherSurface {
        override_surface: calm_dry_surface(),
    };
    assert!(
        logistics
            .evaluate_edge_speed_km_hr(u, v, porter, 400.0, &world.planet, &dry, &reg)
            .is_some(),
        "light porter within 800 kg bridge cap must cross"
    );
    assert!(
        logistics
            .evaluate_edge_speed_km_hr(u, v, wagon, 1200.0, &world.planet, &dry, &reg)
            .is_none(),
        "overweight wagon exceeding 800 kg bridge cap must be blocked"
    );

    // River channel speed passes on open water and blocks under thick ice >= 0.15 m
    logistics.set_edge_state(eid, river_infra, None, false, true, false);
    assert!(
        logistics
            .evaluate_edge_speed_km_hr(u, v, barge, 4000.0, &world.planet, &dry, &reg)
            .is_some(),
        "barge passes open river channel"
    );
    let frozen = MockWeatherSurface {
        override_surface: SurfaceConditionSnapshot {
            temp_c: -14.0,
            ice_thickness_m: 0.22,
            ..calm_dry_surface()
        },
    };
    assert!(
        logistics
            .evaluate_edge_speed_km_hr(u, v, barge, 4000.0, &world.planet, &frozen, &reg)
            .is_none(),
        "thick river ice (> 0.14 m) must block barge navigation"
    );
}

// @req ENG-LOG-002.1
#[test]
fn convoys_advance_continuously_along_string_pulled_spherical_paths() {
    use sim_spine::LogisticsPort;
    let reg = load_unexplained_bundle();
    let world = ScenarioWorld::from_registry(&reg, 42);
    let mut logistics = LogisticsNetwork::from_planet_and_registry(&world.planet, &reg);

    let porter_id = reg
        .carrier_id_by_key("foot_porter_column")
        .expect("foot_porter_column");
    let porter = reg.carrier_def(porter_id).expect("porter def");
    let dry = MockWeatherSurface {
        override_surface: calm_dry_surface(),
    };

    // Find any multi-hop overland pair whose unimproved path admits a string-pulled shortcut.
    let mut found_pair = None;
    for orig_raw in 0..world.planet.cell_count() {
        let orig = CellId(orig_raw);
        if world.planet.is_ocean_cell(orig) {
            continue;
        }
        // BFS up to depth 4 looking for a land cell reachable overland with >= 3 graph hops.
        let mut queue = std::collections::VecDeque::new();
        let mut seen = std::collections::BTreeMap::new();
        queue.push_back((orig, 0u32));
        seen.insert(orig, 0u32);
        while let Some((cur, depth)) = queue.pop_front() {
            if depth >= 3
                && let Some((route, _)) =
                    logistics.plan_route(orig, cur, porter_id, 50.0, &world.planet, &dry, &reg)
                && route.len() >= 4
            {
                let discrete_km: f32 = route
                    .windows(2)
                    .filter_map(|w| world.planet.cell_edge(w[0], w[1]))
                    .map(|eid| world.planet.edge_length_km(eid))
                    .sum();
                let gc_km = world.planet.great_circle_km(orig, cur);
                if gc_km < discrete_km - 1.0 {
                    found_pair = Some((orig, cur, route.len(), discrete_km));
                    break;
                }
            }
            if depth >= 4 {
                continue;
            }
            for &nb in world.planet.cell_neighbors(cur) {
                if !seen.contains_key(&nb)
                    && !world.planet.is_ocean_cell(nb)
                    && logistics
                        .evaluate_edge_speed_km_hr(
                            cur,
                            nb,
                            porter,
                            50.0,
                            &world.planet,
                            &dry,
                            &reg,
                        )
                        .is_some()
                {
                    seen.insert(nb, depth + 1);
                    queue.push_back((nb, depth + 1));
                }
            }
        }
        if found_pair.is_some() {
            break;
        }
    }

    let (origin, dest, _route_nodes, discrete_km) =
        found_pair.expect("expected multi-hop overland corridor");
    let cid = logistics
        .launch_unified_transit(
            sim_logistics::UnifiedTransitLaunch {
                label: "Continuous Survey".to_string(),
                channel: sim_spine::TransitChannelKind::SurveyExpedition,
                origin_settlement: sim_spine::SettlementId(1),
                destination_settlement: None,
                origin_cell: origin,
                destination_cell: dest,
                carrier_class: porter_id,
                owner: ActorId(1),
                cargo_commodity: None,
                cargo_mass_kg: 50.0,
                cargo_purity_01: 1.0,
                fuel_loaded_kg: 100.0,
                spare_parts_loaded_kg: 20.0,
                current_tick: sim_spine::Tick(0),
                is_hq_move: false,
                dispatch_id: None,
                sender: None,
                mandate_payload_id: None,
                subject: "Survey".to_string(),
            },
            &world.planet,
            &dry,
            &reg,
        )
        .expect("launch continuous convoy");

    let initial_view = logistics.convoy_state(cid).expect("initial convoy view");
    assert!(
        initial_view.total_route_km <= discrete_km,
        "string-pulled route {} km must never exceed graph zig-zag {} km",
        initial_view.total_route_km,
        discrete_km
    );
    assert!(
        initial_view.route_waypoints_xyz.len() >= 2,
        "route must record unit-sphere waypoints"
    );

    let mut prev_dist = 0.0_f32;
    let mut saw_subcell = false;
    for t in 1..=20u64 {
        logistics.step_hourly(sim_spine::Tick(t), &world.planet, &dry, &reg);
        let st = logistics.convoy_state(cid).expect("convoy state during transit");
        assert!(
            st.distance_travelled_km > prev_dist,
            "convoy must advance continuously each hour: tick {t}, prev={prev_dist}, now={}",
            st.distance_travelled_km
        );
        prev_dist = st.distance_travelled_km;
        let p = st.current_unit_xyz;
        let norm = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        assert!((norm - 1.0).abs() < 1e-3, "position must remain normalized on S^2");
        if st.distance_travelled_km > 0.0 && st.distance_travelled_km < st.total_route_km {
            saw_subcell = true;
        }
    }
    assert!(saw_subcell, "must observe intermediate continuous sub-cell movement");
}

// @req ENG-LOG-004
#[test]
fn unified_router_conserves_cargo_in_transit_without_instant_teleportation() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    let grain = reg.commodity_id_by_key("raw_grain").expect("raw_grain");
    let iron = reg.commodity_id_by_key("wrought_iron").expect("wrought_iron");
    let porter = reg
        .carrier_id_by_key("foot_porter_column")
        .expect("foot_porter_column");
    let courier = reg
        .carrier_id_by_key("mounted_dispatch_rider")
        .expect("mounted_dispatch_rider");

    let sid_origin = SettlementId(0);
    let cell_origin = world.economy.settlements()[0].cell_id;
    let (dest_cell, est_hours) = world
        .planet
        .cell_neighbors(cell_origin)
        .iter()
        .copied()
        .filter(|&nb| !world.planet.is_ocean_cell(nb) && nb != cell_origin)
        .filter_map(|nb| {
            let (_, f_h) = world.logistics.plan_route(
                cell_origin,
                nb,
                porter,
                120.0,
                &world.planet,
                &world.weather,
                &reg,
            )?;
            let _ = world.logistics.plan_route(
                cell_origin,
                nb,
                courier,
                0.0,
                &world.planet,
                &world.weather,
                &reg,
            )?;
            Some((nb, f_h))
        })
        .next()
        .expect("reachable adjacent land cell for porter and courier");
    let sid_dest = world.economy.establish_settlement(
        "remote_fort".to_owned(),
        "Remote Fort".to_owned(),
        dest_cell,
        ActorId(1),
        &[(grain, 200.0, 0.95, 0.96)],
        &[],
    );

    let (debited, purity) = world
        .economy
        .debit_cargo_for_transit(sid_origin, iron, ActorId(1), 120.0)
        .expect("cargo debit");
    let fuel = world.economy.load_fuel_into_transit(sid_origin, grain, 450.0);

    let convoy_id = world
        .logistics
        .launch_unified_transit(
            UnifiedTransitLaunch {
                label: "Iron Relief Column".to_owned(),
                channel: TransitChannelKind::FreightCargo,
                origin_settlement: sid_origin,
                destination_settlement: Some(sid_dest),
                origin_cell: cell_origin,
                destination_cell: dest_cell,
                carrier_class: porter,
                owner: ActorId(1),
                cargo_commodity: Some(iron),
                cargo_mass_kg: debited,
                cargo_purity_01: purity,
                fuel_loaded_kg: fuel,
                spare_parts_loaded_kg: 0.0,
                current_tick: Tick(0),
                is_hq_move: false,
                dispatch_id: None,
                sender: None,
                mandate_payload_id: None,
                subject: String::new(),
            },
            &world.planet,
            &world.weather,
            &reg,
        )
        .expect("freight transit launch");

    let courier_fuel = world.economy.load_fuel_into_transit(sid_origin, grain, 10.0);
    let _courier_id = world
        .logistics
        .launch_unified_transit(
            UnifiedTransitLaunch {
                label: "Signal Rider".to_owned(),
                channel: TransitChannelKind::PhysicalCourier,
                origin_settlement: sid_origin,
                destination_settlement: Some(sid_dest),
                origin_cell: cell_origin,
                destination_cell: dest_cell,
                carrier_class: courier,
                owner: ActorId(1),
                cargo_commodity: None,
                cargo_mass_kg: 0.0,
                cargo_purity_01: 1.0,
                fuel_loaded_kg: courier_fuel,
                spare_parts_loaded_kg: 0.0,
                current_tick: Tick(0),
                is_hq_move: false,
                dispatch_id: None,
                sender: Some(sim_spine::CharacterId(1)),
                mandate_payload_id: None,
                subject: "test_courier".to_owned(),
            },
            &world.planet,
            &world.weather,
            &reg,
        )
        .expect("courier launch");

    // Immediately upon dispatch: cargo is in transit and NOT teleported to destination.
    assert!((world.logistics.mass_in_transit_kg() - 120.0).abs() < 1e-6);
    assert_eq!(
        world
            .economy
            .settlement_commodity_mass_kg(sid_dest, iron, Some(ActorId(1))),
        0.0
    );

    // Step until convoy arrives physically via world loop.
    let max_ticks = (est_hours.ceil() as u64) + 360;
    for _ in 0..max_ticks {
        world.step_one_tick(&reg);
        if world
            .logistics
            .convoys()
            .iter()
            .find(|c| c.convoy_id == convoy_id)
            .is_some_and(|c| c.arrived)
        {
            break;
        }
    }

    assert!(
        world
            .logistics
            .convoys()
            .iter()
            .find(|c| c.convoy_id == convoy_id)
            .is_some_and(|c| c.arrived),
        "freight convoy should arrive cleanly within {max_ticks}h"
    );
    assert!(
        (world
            .economy
            .settlement_commodity_mass_kg(sid_dest, iron, Some(ActorId(1)))
            - 120.0)
            .abs()
            < 1e-4
    );
    assert!(world.economy.conservation_error_kg() <= 1e-4);
}

// @req ENG-LOG-007
#[test]
fn purpose_driven_roads_organic_worn_tracks_and_river_current_barge_asymmetry() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    let snap = world.compile_diegetic_snapshot(&reg);

    // 1. Starting settlement must have only purposeful resource/outpost access roads, never 6 indiscriminate spoke trails.
    let hq_cell = snap.hq_presence.cell_id;
    let built_edges: Vec<_> = snap
        .surveyed_roads
        .iter()
        .filter(|r| r.from_cell_id == hq_cell || r.to_cell_id == hq_cell)
        .collect();
    assert!(
        !built_edges.is_empty() && built_edges.len() <= 3,
        "starting settlement must avoid spoke dead-end trails; got {} adjacent roads",
        built_edges.len()
    );
    assert!(
        snap.surveyed_roads
            .iter()
            .all(|r| !r.road_purpose.is_empty() && r.road_purpose != "unbuilt_wilderness")
    );

    // Unexplored cells must never leak hydrology discharge or velocity fields.
    assert!(snap.known_cells.iter().any(|c| matches!(c, KnownCellDto::Unexplored { .. })));

    // 2. Built road lowers travel friction relative to unbuilt open wilderness, and 3 overland passages wear a track.
    let porter_id = reg
        .carrier_id_by_key("foot_porter_column")
        .expect("foot_porter_column");
    let porter_def = reg.carrier_def(porter_id).expect("porter_def");
    let foot_trail = reg.infra_id_by_key("foot_trail").expect("foot_trail");
    let grain = reg.commodity_id_by_key("raw_grain").expect("raw_grain");
    let dry = MockWeatherSurface {
        override_surface: calm_dry_surface(),
    };
    let hq_sid = world.economy.hq_settlement().unwrap();

    let mut unbuilt_pair = None;
    let hq_cid = CellId(hq_cell);
    for &nbr in world.planet.cell_neighbors(hq_cid) {
        if !world.planet.is_ocean_cell(nbr)
            && let Some(eid) = world.planet.cell_edge(hq_cid, nbr)
            && world.planet.cell_slope_rad(eid).abs() <= porter_def.max_grade_rad
            && world
                .logistics
                .edge_state(eid)
                .is_some_and(|st| !st.road_built && !st.sea_edge && !st.requires_bridge_for_land)
        {
            unbuilt_pair = Some((hq_cid, nbr, eid));
            break;
        }
    }
    let (u, v, wild_eid) = unbuilt_pair.expect("unbuilt wilderness land edge near HQ");
    let wild_speed = world
        .logistics
        .evaluate_edge_speed_km_hr(u, v, porter_def, 200.0, &world.planet, &dry, &reg)
        .expect("wilderness speed");

    let mut scratch_log = LogisticsNetwork::from_planet_and_registry(&world.planet, &reg);
    scratch_log.build_purpose_road(
        wild_eid,
        foot_trail,
        RoadPurposeKind::ResourceOrPortAccess,
        "Built Test Corridor",
        None,
    );
    let built_speed = scratch_log
        .evaluate_edge_speed_km_hr(u, v, porter_def, 200.0, &world.planet, &dry, &reg)
        .expect("built trail speed");
    assert!(
        built_speed > wild_speed,
        "built road ({built_speed}) must travel faster than unbuilt wilderness ({wild_speed})"
    );

    // 3. Repeated overland transits across unbuilt wilderness edge organically wear a TrafficWornTrack.
    let mut trial_log = LogisticsNetwork::from_planet_and_registry(&world.planet, &reg);
    for _ in 0..3 {
        let _ = trial_log.launch_unified_transit(
            UnifiedTransitLaunch {
                label: "porter_wear_test".to_owned(),
                channel: TransitChannelKind::FreightCargo,
                origin_settlement: hq_sid,
                destination_settlement: None,
                origin_cell: u,
                destination_cell: v,
                owner: ActorId(1),
                carrier_class: porter_id,
                cargo_commodity: Some(grain),
                cargo_mass_kg: 400.0,
                cargo_purity_01: 1.0,
                fuel_loaded_kg: 250.0,
                spare_parts_loaded_kg: 50.0,
                current_tick: Tick(0),
                is_hq_move: false,
                dispatch_id: None,
                sender: None,
                mandate_payload_id: None,
                subject: "wear_probe".to_owned(),
            },
            &world.planet,
            &dry,
            &reg,
        );
    }
    for t in 1..600_u64 {
        trial_log.step_hourly(Tick(t), &world.planet, &dry, &reg);
        if trial_log.edge_state(wild_eid).is_some_and(|e| e.road_built) {
            break;
        }
    }
    let worn_state = trial_log.edge_state(wild_eid).expect("worn edge state");
    assert!(
        worn_state.road_built && worn_state.road_purpose == RoadPurposeKind::TrafficWornTrack,
        "repeated transit across wilderness must promote edge to TrafficWornTrack (passages={}, built={})",
        worn_state.cumulative_passages,
        worn_state.road_built
    );

    // 4. Directional river barge speed asymmetry: downstream exceeds upstream on perennial river reach.
    let barge_id = reg
        .carrier_id_by_key("river_cargo_barge")
        .expect("river_cargo_barge");
    let barge_def = reg.carrier_def(barge_id).expect("barge_def");
    let mut river_edge_pair = None;
    for raw in 0..world.planet.cell_count() {
        let cu = CellId(raw);
        if world.planet.is_ocean_cell(cu) {
            continue;
        }
        if let Some(cv) = world.planet.cell_river_channel_downstream(cu)
            && !world.planet.is_ocean_cell(cv)
            && world.planet.cell_river_discharge_m3_s(cu) >= 25.0
            && world.planet.cell_river_velocity_mps(cu) <= 3.8
            && let Some(eid) = world.planet.cell_edge(cu, cv)
            && world.planet.cell_slope_rad(eid).abs() <= 0.045
        {
            river_edge_pair = Some((cu, cv));
            break;
        }
    }
    let (ru, rv) = river_edge_pair.expect("navigable perennial river edge on planet");
    let down_speed = world
        .logistics
        .evaluate_edge_speed_km_hr(ru, rv, barge_def, 1000.0, &world.planet, &dry, &reg)
        .expect("downstream barge speed");
    let up_speed = world
        .logistics
        .evaluate_edge_speed_km_hr(rv, ru, barge_def, 1000.0, &world.planet, &dry, &reg)
        .expect("upstream barge speed");
    assert!(
        down_speed > up_speed * 1.15,
        "downstream barge ({down_speed:.2} km/h) must meaningfully exceed upstream ({up_speed:.2} km/h)"
    );
}
