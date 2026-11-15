#![deny(clippy::disallowed_types)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use player_view::ValidatedPlayerCommand;
use sim_data::load_mod_bundle;
use sim_scenario::{resolve_scenario_pack_stack, ScenarioWorld};
use sim_spine::{
    great_circle_arc_km, CellId, CharacterId, ModRegistryPort, OperationalCombatPlugPort,
    PhysicalLedgerPort, PlanetGeometryPort, SettlementId, SpinePlayerCommand, SuccessionPlugPort,
    SurfaceConditionSnapshot, SurfaceConditionsPort, TheaterId, Tick,
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

fn find_reachable_land_cell(world: &ScenarioWorld, min_hops: usize) -> CellId {
    let hq = world.economy.hq_cell();
    let mut dist: BTreeMap<CellId, usize> = BTreeMap::new();
    let mut frontier = std::collections::VecDeque::new();
    dist.insert(hq, 0);
    frontier.push_back(hq);
    let mut candidate = None;
    while let Some(u) = frontier.pop_front() {
        let d = dist[&u];
        if d >= min_hops && !world.planet.is_ocean_cell(u) {
            candidate = Some(u);
            break;
        }
        for &v in world.planet.cell_neighbors(u) {
            if !world.planet.is_ocean_cell(v) && !dist.contains_key(&v) {
                dist.insert(v, d + 1);
                frontier.push_back(v);
            }
        }
    }
    candidate.expect("planet must contain a reachable land cell")
}

#[derive(Clone, Debug)]
struct FixedWeatherPort {
    surface_snapshot: SurfaceConditionSnapshot,
}

impl SurfaceConditionsPort for FixedWeatherPort {
    fn surface(&self, _cell: CellId) -> SurfaceConditionSnapshot {
        self.surface_snapshot.clone()
    }
}

// @req ENG-WAR-001.1
#[test]
fn tactical_unit_micromanagement_commands_are_strictly_rejected_at_ipc_boundary() {
    let reg = load_unexplained_bundle();
    let world = ScenarioWorld::from_registry(&reg, 42);

    let err = ValidatedPlayerCommand::IssueTacticalMicroCommand { unit_id: 7 }
        .to_spine_command()
        .expect_err("direct tactical unit micro command must be forbidden");
    assert!(
        err.starts_with("ERR_TACTICAL_MICRO_FORBIDDEN:"),
        "unexpected error code: {err}"
    );

    let plug_err = world
        .warfare
        .reject_tactical_micro("select individual squad")
        .expect_err("warfare plugin must reject direct unit micro");
    assert!(plug_err.contains("ERR_TACTICAL_MICRO_FORBIDDEN"));
}

// @req ENG-WAR-002.1
// @req ENG-WAR-002.2
#[test]
fn theater_directive_uses_low_attention_and_general_maneuvers_around_threats() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    let obj_cell = find_reachable_land_cell(&world, 2);

    // Issue high-level Churchill theater directive -> records TheaterDirective with moderate attention usage.
    let attention_before = world.command.clerical_overhead_total();
    world.apply_player_command_now(
        &reg,
        SpinePlayerCommand::IssueTheaterDirective {
            theater_id: TheaterId(1),
            commander: CharacterId(0),
            objective_cell: obj_cell,
            supply_priority_01: 0.85,
        },
    );
    let theaters = world.warfare.theaters();
    assert!(!theaters.is_empty());
    assert_eq!(theaters[0].objective_cell, obj_cell);
    // Theater directive adds no operational battle-plan micromanagement overhead spike.
    assert!((world.command.clerical_overhead_total() - attention_before).abs() < 1e-4);

    // Muster a field formation, march toward front, then apply hostile pincer pressure (`>= 0.70`).
    let hq_cell = world.economy.hq_cell();
    let fid = world
        .warfare
        .muster_formation(
            &reg,
            &world.planet,
            "line_infantry_battalion",
            TheaterId(1),
            CharacterId(0),
            SettlementId(0),
            hq_cell,
        )
        .expect("formation muster succeeds");

    // Advance formation slightly away from home before pressure triggers autonomous elastic withdrawal.
    world
        .warfare
        .issue_operational_battle_plan(
            fid,
            obj_cell,
            "mission_command_elastic".to_owned(),
            false,
            Tick(1),
            1,
            &world.planet,
        )
        .expect("elastic plan queued");
    world.step_ticks(&reg, 4);

    let form_cell = world
        .warfare
        .formations()
        .iter()
        .find(|f| f.formation_id == fid)
        .expect("formation exists")
        .current_cell;
    world.warfare.set_hostile_pressure(form_cell, 0.85);
    world.warfare.set_hostile_pressure(obj_cell, 0.85);

    let dry = FixedWeatherPort {
        surface_snapshot: SurfaceConditionSnapshot::default(),
    };
    world
        .warfare
        .step_with_world_context(Tick(10), &world.planet, &dry, &reg, true);

    let form_after = world
        .warfare
        .formations()
        .iter()
        .find(|f| f.formation_id == fid)
        .expect("formation exists")
        .clone();
    assert!(!form_after.encircled, "Churchill elastic commander must avoid pocket encirclement");
    assert!(
        world
            .warfare
            .theaters()
            .iter()
            .any(|t| t.autonomous_withdrawals >= 1),
        "hostile pincer without No Retreat order must increment autonomous theater withdrawals"
    );
}

// @req ENG-WAR-003.1
// @req ENG-WAR-003.2
#[test]
fn no_retreat_operational_battle_plan_exposes_encirclement_vs_insubordination() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    let hq_cell = world.economy.hq_cell();
    let front_cell = find_reachable_land_cell(&world, 2);

    // Commander A: Obedient low-initiative loyalist obeys stale "No Retreat" -> gets encircled.
    // Commander B: High-initiative field general commits insubordinate withdrawal to survive.
    let cmd_obedient = CharacterId(10);
    let cmd_rebel = CharacterId(11);
    world.warfare.set_commander_profile(cmd_obedient, 0.25, 0.92);
    world.warfare.set_commander_profile(cmd_rebel, 0.94, 0.45);

    let f_obedient = world
        .warfare
        .muster_formation(
            &reg,
            &world.planet,
            "line_infantry_battalion",
            TheaterId(1),
            cmd_obedient,
            SettlementId(0),
            front_cell,
        )
        .expect("obedient formation");
    let f_rebel = world
        .warfare
        .muster_formation(
            &reg,
            &world.planet,
            "mounted_recon_squadron",
            TheaterId(1),
            cmd_rebel,
            SettlementId(0),
            front_cell,
        )
        .expect("rebel formation");

    world.apply_player_command_now(
        &reg,
        SpinePlayerCommand::IssueOperationalBattlePlan {
            formation_id: f_obedient,
            target_cell: hq_cell,
            doctrine_key: "rigid_positional_no_retreat".to_owned(),
            no_retreat: true,
        },
    );
    world.apply_player_command_now(
        &reg,
        SpinePlayerCommand::IssueOperationalBattlePlan {
            formation_id: f_rebel,
            target_cell: hq_cell,
            doctrine_key: "rigid_positional_no_retreat".to_owned(),
            no_retreat: true,
        },
    );

    // Step until courier orders arrive, then clamp hostile encirclement threat >= 0.85.
    let courier_carrier = world.logistics.default_courier_carrier();
    let courier_ticks = world
        .logistics
        .plan_route(
            hq_cell,
            front_cell,
            courier_carrier,
            0.0,
            &world.planet,
            &world.weather,
            &reg,
        )
        .map(|(_, h)| (h.ceil() as u64) + 4)
        .unwrap_or(24);
    world.step_ticks(&reg, courier_ticks);
    world.warfare.set_hostile_pressure(front_cell, 0.88);
    world.warfare.set_hostile_pressure(hq_cell, 0.88);

    let clear = FixedWeatherPort {
        surface_snapshot: SurfaceConditionSnapshot::default(),
    };
    let personnel_before = world
        .warfare
        .formations()
        .iter()
        .find(|f| f.formation_id == f_obedient)
        .expect("obedient")
        .personnel_count;
    let base_t = world.clock.current_tick.get();
    for t in (base_t + 1)..=(base_t + 4) {
        world
            .warfare
            .step_with_world_context(Tick(t), &world.planet, &clear, &reg, true);
    }

    let after_obedient = world
        .warfare
        .formations()
        .iter()
        .find(|f| f.formation_id == f_obedient)
        .expect("obedient")
        .clone();
    let after_rebel = world
        .warfare
        .formations()
        .iter()
        .find(|f| f.formation_id == f_rebel)
        .expect("rebel")
        .clone();

    assert!(after_obedient.no_retreat_order, "courier must deliver Hitler No Retreat flag");
    assert!(
        after_obedient.encircled && after_obedient.stale_order_ticks >= 1,
        "obedient unit held by stale No Retreat order must become encircled under flanking threat"
    );
    assert!(
        after_obedient.personnel_count < personnel_before,
        "encircled formation must suffer severe personnel attrition"
    );

    assert!(
        after_rebel.insubordinate_withdrawal && !after_rebel.no_retreat_order && !after_rebel.encircled,
        "high-initiative general must override suicidal No Retreat order via insubordinate withdrawal"
    );
}

// @req ENG-WAR-004.1
// @req ENG-WAR-004.2
#[test]
fn hourly_mud_miring_flooded_crossings_and_rain_fouling_degrade_operations() {
    let reg = load_unexplained_bundle();
    let mut world_dry = ScenarioWorld::from_registry(&reg, 42);
    let mut world_storm = ScenarioWorld::from_registry(&reg, 42);
    let hq_cell = world_dry.economy.hq_cell();
    let target_cell = find_reachable_land_cell(&world_dry, 2);

    let f_dry = world_dry
        .warfare
        .muster_formation(
            &reg,
            &world_dry.planet,
            "pioneer_sapper_company",
            TheaterId(1),
            CharacterId(0),
            SettlementId(0),
            hq_cell,
        )
        .expect("dry formation");
    let f_storm = world_storm
        .warfare
        .muster_formation(
            &reg,
            &world_storm.planet,
            "pioneer_sapper_company",
            TheaterId(1),
            CharacterId(0),
            SettlementId(0),
            hq_cell,
        )
        .expect("storm formation");

    world_dry
        .warfare
        .issue_operational_battle_plan(
            f_dry,
            target_cell,
            "cautious_reconnaissance".to_owned(),
            false,
            Tick(1),
            1,
            &world_dry.planet,
        )
        .expect("plan dry");
    world_storm
        .warfare
        .issue_operational_battle_plan(
            f_storm,
            target_cell,
            "rigid_positional_no_retreat".to_owned(),
            false,
            Tick(1),
            1,
            &world_storm.planet,
        )
        .expect("plan storm");

    let dry_weather = FixedWeatherPort {
        surface_snapshot: SurfaceConditionSnapshot {
            precip_mm_hr: 0.0,
            mud_index_01: 0.0,
            flood_stage_01: 0.0,
            visibility_01: 1.0,
            ..SurfaceConditionSnapshot::default()
        },
    };
    let storm_weather = FixedWeatherPort {
        surface_snapshot: SurfaceConditionSnapshot {
            precip_mm_hr: 22.0,
            mud_index_01: 0.85,
            flood_stage_01: 0.65,
            visibility_01: 0.22,
            ..SurfaceConditionSnapshot::default()
        },
    };

    for t in 1..=6u64 {
        world_dry
            .warfare
            .step_with_world_context(Tick(t), &world_dry.planet, &dry_weather, &reg, true);
        world_storm.warfare.step_with_world_context(
            Tick(t),
            &world_storm.planet,
            &storm_weather,
            &reg,
            true,
        );
    }

    let unit_dry = world_dry
        .warfare
        .formations()
        .iter()
        .find(|f| f.formation_id == f_dry)
        .expect("dry unit");
    let unit_storm = world_storm
        .warfare
        .formations()
        .iter()
        .find(|f| f.formation_id == f_storm)
        .expect("storm unit");

    let r_km = world_dry.planet.radius_km();
    let remaining_dry_km = great_circle_arc_km(unit_dry.current_unit_xyz, unit_dry.target_unit_xyz, r_km);
    let remaining_storm_km =
        great_circle_arc_km(unit_storm.current_unit_xyz, unit_storm.target_unit_xyz, r_km);

    assert!(
        remaining_storm_km > remaining_dry_km + 0.1,
        "mud miring and flooded crossings must significantly slow operational march: rem_storm={remaining_storm_km}km, rem_dry={remaining_dry_km}km"
    );
    assert!(
        unit_storm.cohesion_01 < unit_dry.cohesion_01,
        "storm precipitation and mud stress must degrade combat cohesion relative to dry ground"
    );

    // WeatherEngine storm rainfall injection lowers visibility and raises mud index for recon degradation (`ENG-WAR-004.2`).
    world_storm.weather.inject_storm_rainfall(target_cell, 55.0);
    world_storm.weather.step_hourly(Tick(7));
    let cell_surf = world_storm.weather.surface(target_cell);
    assert!(cell_surf.precip_mm_hr > 0.0 || cell_surf.mud_index_01 > 0.0 || cell_surf.visibility_01 < 1.0);
}

// @req ENG-POP-006.1
// @req ENG-POP-006.2
#[test]
fn mortal_ruler_succession_preserves_archives_wipes_unrecorded_rapport_and_saves_deterministically() {
    let reg = load_unexplained_bundle();
    let rules = reg.succession_rules();
    assert!(
        rules.len() >= 3,
        "at least 3 externalized succession rules must load from mods/core-schemas"
    );

    let mut world = ScenarioWorld::from_registry(&reg, 42);
    for rule in rules {
        world
            .succession
            .set_succession_rule(&rule.key, &reg)
            .expect("rule valid");
    }

    // Record durable physical HQ document while keeping informal unrecorded personal rapport with Character 1.
    world
        .succession
        .record_physical_hq_archive("Northern Basin Treaty Ratification".to_owned());
    let expected_archives = world.succession.archive_entry_count();
    assert!(expected_archives >= 2);
    assert!(
        world.succession.personal_rapport(CharacterId(1)) > 0.1,
        "living predecessor holds informal unrecorded personal rapport"
    );

    // Advance aging until mortal demise or trigger demise directly cleanly tests ruler mortality continuity.
    let aged_transition = world.succession.step_aging(Tick(500), 24 * 365 * 45);
    if !aged_transition {
        assert!(world.succession.trigger_ruler_demise(Tick(500)));
    }

    let transition = world
        .succession
        .pending_transition()
        .expect("pending succession transition generated upon ruler demise")
        .clone();
    assert!(transition.awaiting_player_acknowledgement);
    assert_eq!(transition.inherited_archive_entries, expected_archives);
    assert!(transition.lost_unrecorded_rapport_count >= 1);
    assert_ne!(transition.deceased_ruler_name, transition.successor_name);

    // Unrecorded rapport with prior contacts is wiped cleanly (`0.0`).
    assert_eq!(world.succession.personal_rapport(CharacterId(1)), 0.0);

    // Temporary institutional command disruption applies until `disruption_until_tick`.
    assert!(
        world.succession.effective_command_multiplier(Tick(500)) < 1.0,
        "succession disruption must temporarily dampen command multiplier"
    );

    // Snapshot & AcknowledgeSuccession IPC flow works cleanly.
    let snap_modal = world.compile_diegetic_snapshot(&reg);
    assert!(
        snap_modal
            .succession
            .pending_transition
            .as_ref()
            .is_some_and(|t| t.awaiting_player_acknowledgement),
        "DiegeticSnapshot must surface awaiting_player_acknowledgement modal transition"
    );

    world.apply_player_command_now(&reg, SpinePlayerCommand::AcknowledgeSuccession);
    assert!(
        !world
            .succession
            .pending_transition()
            .expect("transition retained in history")
            .awaiting_player_acknowledgement
    );

    // Canonical binary save/load round-trip across ALL 12 cartridges preserves deterministic state digest (`ENG-CLK-005`).
    let save_path = std::env::temp_dir().join(format!(
        "ng_m7_canonical_save_{}.ngsave",
        std::process::id()
    ));
    world.save_to_path(&save_path, &reg).expect("save succeeds");
    let mut reloaded = ScenarioWorld::from_registry(&reg, 999);
    reloaded
        .load_from_path(&save_path, &reg)
        .expect("load succeeds");
    let _ = std::fs::remove_file(&save_path);

    world.step_ticks(&reg, 24);
    reloaded.step_ticks(&reg, 24);
    assert_eq!(
        world.state_digest_hex(),
        reloaded.state_digest_hex(),
        "loaded world must remain bit-identical across all 12 cartridges including M5-M7 plugs"
    );
}
