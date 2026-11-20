#![deny(clippy::disallowed_types)]

use std::path::{Path, PathBuf};

use player_view::{KnownCellDto, ValidatedPlayerCommand};
use sim_data::load_mod_bundle;
use sim_scenario::{resolve_scenario_pack_stack, ScenarioWorld};
use sim_spine::{
    CellId, GeologyProvinceKind, ModRegistryPort, PhysicalLedgerPort, PlanetGeometryPort,
    SpinePlayerCommand, SubsurfaceDepositTruthPort, SurfaceConditionsPort, Tick,
};

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

// @req ENG-GEO-011
#[test]
fn externalized_geology_affinities_produce_tiered_offshore_placers_and_starter_coastal_reserves_and_dam_snapshot() {
    let bundle = load_unexplained_bundle();
    let cfg = bundle.planet_gen_config();
    assert_eq!(
        cfg.geology_provinces.len(),
        13,
        "core-schemas must export all 13 land and marine provinces"
    );
    assert!(
        cfg.deposit_affinities.len() >= 12,
        "core-schemas must load externalized mineral deposit affinity rules"
    );
    assert!(
        !cfg.fluids.is_empty(),
        "core-schemas must load fluid thermophysical constants"
    );

    let mut world = ScenarioWorld::from_registry(&bundle, 42);
    let n = world.planet.cell_count();
    let mut offshore_reserves = 0usize;
    let mut alluvial_placers = 0usize;
    let mut tier1_deposits = 0usize;
    let mut tier3_deposits = 0usize;

    for idx in 0..n {
        let cid = CellId(idx);
        let deposits = world.planet.deposits_in_cell(cid);
        for d in deposits {
            if d.is_offshore {
                offshore_reserves += 1;
                assert!(
                    world.planet.cell_geology_province(cid)
                        == GeologyProvinceKind::ContinentalShelf
                        || world.planet.is_ocean_cell(cid),
                    "offshore deposit must lie on submarine shelf/ocean"
                );
            }
            if d.is_alluvial_placer {
                alluvial_placers += 1;
            }
            match d.accessibility_tier {
                1 => tier1_deposits += 1,
                2 => {}
                3 => tier3_deposits += 1,
                other => panic!("unexpected accessibility tier {other}"),
            }
        }
    }

    assert!(offshore_reserves >= 1, "expected continental shelf offshore mineral reserves");
    assert!(alluvial_placers >= 1, "expected downstream alluvial placer accumulations");
    assert!(tier1_deposits >= 10, "expected plentiful accessible Tier-1 starter deposits");
    assert!(tier3_deposits >= 1, "expected deeper Tier-3 lithology deposits");

    // Verify starter coastal guarantee around temperate landing coasts and HQ region.
    let hq_cell = world.economy.hq_cell();
    let mut near_tier1 = 0usize;
    for idx in 0..n {
        let cid = CellId(idx);
        if world.planet.great_circle_km(hq_cell, cid) <= 800.0 {
            near_tier1 += world
                .planet
                .deposits_in_cell(cid)
                .iter()
                .filter(|d| d.accessibility_tier == 1)
                .count();
        }
    }
    assert!(
        near_tier1 >= 1,
        "regional landing halo around HQ #{} should contain Tier-1 starter reserves",
        hq_cell.get()
    );
    let top_coast = (0..n)
        .map(CellId)
        .filter(|&c| {
            !world.planet.is_ocean_cell(c)
                && world.planet.cell_neighbors(c).iter().any(|&nb| world.planet.is_ocean_cell(nb))
                && (8.0..=24.0).contains(&world.planet.cell_mean_annual_temp_c(c))
                && world.planet.cell_elevation_m(c) < 300.0
        })
        .max_by(|&a, &b| {
            (22.0 - (world.planet.cell_mean_annual_temp_c(a) - 15.0).abs())
                .total_cmp(&(22.0 - (world.planet.cell_mean_annual_temp_c(b) - 15.0).abs()))
        })
        .expect("temperate coastal cell must exist");
    let coast_t1 = (0..n)
        .map(CellId)
        .filter(|&c| world.planet.great_circle_km(top_coast, c) <= 720.0)
        .flat_map(|c| world.planet.deposits_in_cell(c).iter())
        .filter(|d| d.accessibility_tier == 1)
        .count();
    assert!(coast_t1 >= 1, "top temperate coastal landing site must guarantee Tier-1 deposits");

    // Find a perennial river reach near HQ (prefer already inside HQ sight, else relocate HQ there).
    let target_river = (0..n)
        .map(CellId)
        .find(|&c| {
            !world.planet.is_ocean_cell(c)
                && world.planet.cell_river_channel_downstream(c).is_some()
                && (c == hq_cell || world.planet.cell_neighbors(hq_cell).contains(&c))
        })
        .unwrap_or_else(|| {
            (0..n)
                .map(CellId)
                .filter(|&c| {
                    !world.planet.is_ocean_cell(c)
                        && world.planet.cell_river_channel_downstream(c).is_some()
                        && world.planet.great_circle_km(hq_cell, c) <= 650.0
                })
                .max_by(|&a, &b| {
                    world
                        .planet
                        .cell_river_discharge_m3_s(a)
                        .total_cmp(&world.planet.cell_river_discharge_m3_s(b))
                })
                .expect("should find river near HQ for dam construction test")
        });

    if !world.planet.cell_neighbors(hq_cell).contains(&target_river) && hq_cell != target_river {
        world.enqueue_command(
            Tick(0),
            SpinePlayerCommand::RelocateHq {
                destination_cell: target_river,
            },
        );
        for _ in 0..72 {
            world.step_one_tick(&bundle);
            if world.economy.hq_cell() == target_river {
                break;
            }
        }
    }

    let build_tick = world.clock.current_tick;
    let cmd = ValidatedPlayerCommand::ConstructDam {
        target_cell: target_river.get(),
        rule_curve: "flood_control".to_owned(),
    }
    .to_spine_command()
    .expect("constructDam IPC command must validate");
    world.enqueue_command(build_tick, cmd);
    for _ in 0..8 {
        world.step_one_tick(&bundle);
    }

    let dam_view = world
        .weather
        .dam_at_cell(target_river)
        .expect("commissioned dam must exist in SurfaceConditionsPort");
    assert_eq!(dam_view.rule_curve.as_str(), "flood_control");
    assert!(dam_view.reservoir_capacity_m3 > 0.0);

    let sw_tick = world.clock.current_tick;
    let sw = ValidatedPlayerCommand::SetDamRuleCurve {
        target_cell: target_river.get(),
        rule_curve: "irrigation_supply".to_owned(),
    }
    .to_spine_command()
    .expect("setDamRuleCurve must validate");
    world.enqueue_command(sw_tick, sw);
    world.step_one_tick(&bundle);

    let dam_updated = world.weather.dam_at_cell(target_river).expect("dam remains present");
    assert_eq!(dam_updated.rule_curve.as_str(), "irrigation_supply");

    let snap = world.compile_diegetic_snapshot(&bundle);
    let cell_dto = snap
        .known_cells
        .iter()
        .find(|c| c.cell_id() == target_river.get())
        .expect("target river cell must be in snapshot");
    match cell_dto {
        KnownCellDto::Surveyed {
            geology_province,
            river_navigability,
            dam_status,
            ..
        } => {
            assert!(!geology_province.is_empty());
            assert!(!river_navigability.is_empty());
            let dto = dam_status
                .as_ref()
                .expect("dam_status must appear on live surveyed river cell");
            assert_eq!(dto.rule_curve, "irrigation_supply");
        }
        other => panic!("expected live Surveyed dam cell, got {other:?}"),
    }
}
