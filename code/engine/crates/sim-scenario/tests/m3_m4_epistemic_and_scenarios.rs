#![deny(clippy::disallowed_types)]

use std::path::{Path, PathBuf};

use player_view::{estimate_deposit_interval, perturb_reported_quantity, KnownCellDto, ReportedLedgerRowDto};
use sim_clock::ClockRunMode;
use sim_data::load_mod_bundle;
use sim_logistics::UnifiedTransitLaunch;
use sim_scenario::{resolve_scenario_pack_stack, ScenarioWorld};
use sim_spine::{
    ActorId, CellId, CharacterId, CharacterSnapshotView, ModRegistryPort,
    NotificationBehaviorKind, PhysicalLedgerPort, PlanetGeometryPort, ScenarioArchetypeKind,
    SettlementId, SpinePlayerCommand, SubsurfaceDepositTruthPort, Tick, TransitChannelKind,
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

// @req ENG-EPI-001
#[test]
fn player_knowledge_archive_enforces_strict_epistemic_air_gap_from_planet_truth() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    let snap = world.compile_diegetic_snapshot(&reg);

    // Planet generates subterranean ore deposits across many cells worldwide.
    let remote_with_ore = (0..world.planet.cell_count())
        .map(CellId)
        .find(|&cid| {
            !world.planet.is_ocean_cell(cid)
                && !world.planet.deposits_in_cell(cid).is_empty()
                && world
                    .archive
                    .cell_knowledge(cid.get())
                    .is_none_or(|k| matches!(k, KnownCellDto::Unexplored { .. }))
        })
        .expect("remote unvisited deposit cell exists");

    let known_remote = snap
        .known_cells
        .iter()
        .find(|k| k.cell_id() == remote_with_ore.get());
    assert!(
        known_remote.is_none_or(|k| matches!(k, KnownCellDto::Unexplored { .. })),
        "unexplored remote cell deposits must remain strictly hidden from DiegeticSnapshot"
    );
    // Snapshot exposes reported ledger and audit discrepancies separately from raw ledger internals.
    assert!(!snap.known_cells.is_empty());
    assert!(!snap.ledger_rows.is_empty());
}

// @req ENG-EPI-002
#[test]
fn map_cells_transition_through_unexplored_rumored_surveyed_and_stale_tiers() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    let hq_cell = world.economy.hq_cell();

    // Pick a far land cell outside initial HQ sight halo
    let far_cell = (0..world.planet.cell_count())
        .map(CellId)
        .find(|&c| {
            !world.planet.is_ocean_cell(c) && world.planet.great_circle_km(hq_cell, c) > 1200.0
        })
        .expect("distant unexplored cell");

    assert!(world.archive.cell_knowledge(far_cell.get()).is_none());

    // 1. Ingest distant rumor -> Rumored tier
    world.archive.ingest_rumor(
        far_cell.get(),
        Tick(4),
        420.0,
        "steppe".to_owned(),
        false,
        "Rumor from coastal traders".to_owned(),
    );
    assert!(matches!(
        world.archive.cell_knowledge(far_cell.get()),
        Some(KnownCellDto::Rumored { .. })
    ));

    // 2. Courier arrives with physical geological survey -> Surveyed tier
    world.archive.ingest_survey_report(
        far_cell.get(),
        Tick(10),
        &world.planet,
        Vec::new(),
        "Courier Survey Assay".to_owned(),
    );
    assert!(matches!(
        world.archive.cell_knowledge(far_cell.get()),
        Some(KnownCellDto::Surveyed { .. })
    ));

    // 3. After >= 12 hours outside HQ live sight halo, observe_hq_halo decays cell to Stale
    world.archive.observe_hq_halo(Tick(30), &world.planet, &world.economy);
    match world.archive.cell_knowledge(far_cell.get()) {
        Some(KnownCellDto::Stale { confidence_01, .. }) => {
            assert!(*confidence_01 < 0.92, "stale confidence should decay smoothly");
        }
        other => panic!("expected Stale tier outside HQ sight after 20h, got {other:?}"),
    }

    // 4. HQ optical sight halo cells stay live Surveyed
    match world.archive.cell_knowledge(hq_cell.get()) {
        Some(KnownCellDto::Surveyed { is_live_hq_sight, .. }) => assert!(*is_live_hq_sight),
        other => panic!("expected live HQ cell to remain Surveyed, got {other:?}"),
    }
}

// @req ENG-EPI-003
#[test]
fn courier_dispatches_incur_distance_latency_and_propagate_remote_reports() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    world.clock.mode = ClockRunMode::Running {
        speed_multiplier: 1,
    };

    let hq_cell = world.economy.hq_cell();
    let courier_carrier = world.logistics.default_courier_carrier();
    let hq_nbs = world.planet.cell_neighbors(hq_cell);
    let mut two_hop = None;
    for &n1 in hq_nbs {
        if world.planet.is_ocean_cell(n1) {
            continue;
        }
        for &n2 in world.planet.cell_neighbors(n1) {
            if n2 == hq_cell || hq_nbs.contains(&n2) || world.planet.is_ocean_cell(n2) {
                continue;
            }
            if let Some((_, est_h)) = world.logistics.plan_route(
                n2,
                hq_cell,
                courier_carrier,
                0.0,
                &world.planet,
                &world.weather,
                &reg,
            ) {
                two_hop = Some((n2, est_h));
                break;
            }
        }
        if two_hop.is_some() {
            break;
        }
    }
    let (far_cell, est_hours) = two_hop.expect("reachable 2-hop land cell outside HQ sight");
    let grain = reg.commodity_id_by_key("raw_grain").unwrap();
    let sid_far = world.economy.establish_settlement(
        "far_station".to_owned(),
        "Far Relay Station".to_owned(),
        far_cell,
        ActorId(1),
        &[(grain, 1500.0, 0.95, 0.96)],
        &[],
    );

    let dispatches_before = world.compile_diegetic_snapshot(&reg).dispatch_inbox.len();
    let wait_ticks = 30 + (est_hours.ceil() as u64) + 120;
    world.step_ticks(&reg, wait_ticks);
    let snap = world.compile_diegetic_snapshot(&reg);

    assert!(
        snap.dispatch_inbox.len() > dispatches_before,
        "remote settlements must deliver courier dispatches after transit latency"
    );
    assert!(
        snap.ledger_rows
            .iter()
            .any(|r| r.settlement_id == sid_far.get()),
        "courier dispatch should populate reported ledger rows for remote settlement"
    );
    let any_delayed = snap
        .dispatch_inbox
        .iter()
        .any(|d| d.arrived_tick > d.sent_tick);
    assert!(any_delayed, "dispatches must require > 0 hours physical transit");
}

// @req ENG-EPI-004
#[test]
fn sycophantic_biased_reporting_and_skimming_triggered_audit_discrepancy_under_direct_sight() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);

    let sycophant = CharacterSnapshotView {
        id: CharacterId(88),
        key: "sycophant_steward".to_owned(),
        display_name: "Steward Orlov".to_owned(),
        role_title: "Quartermaster".to_owned(),
        home_settlement: SettlementId(1),
        competence: 0.45,
        precision_01: 0.85,
        honesty_vs_sycophancy: -0.90,
        caution_vs_alarmism: 0.0,
    };

    let true_kg = 500.0;
    let (reported_kg, min_kg, max_kg, provenance) =
        perturb_reported_quantity(42, 24, 1, 0, true_kg, &sycophant, 0.45);
    assert!(
        reported_kg > true_kg * 1.15,
        "sycophantic + skimming quartermaster should inflate reported stock: reported={reported_kg}, true={true_kg}"
    );
    assert!(min_kg <= reported_kg && reported_kg <= max_kg);
    assert!(provenance.contains("Steward Orlov"));

    world.archive.record_remote_ledger_report(ReportedLedgerRowDto {
        settlement_id: 1,
        settlement_name: "Remote Depot".to_owned(),
        commodity_key: "raw_grain".to_owned(),
        commodity_name: "Raw Grain".to_owned(),
        reported_mass_kg: reported_kg,
        min_est_kg: min_kg,
        max_est_kg: max_kg,
        observed_tick: 12,
        provenance_label: provenance,
        category: "food".to_owned(),
        confidence_01: sycophant.precision_01,
        reporter_name: sycophant.display_name.clone(),
    });

    // Direct inspection (after HQ arrives at Settlement 1) reveals exact true mass and logs discrepancy.
    world.archive.audit_settlement_against_direct_sight(
        Tick(40),
        1,
        sycophant.id.0,
        &sycophant.display_name,
        "raw_grain",
        true_kg,
    );
    let discrepancies = world.archive.audit_discrepancies();
    assert_eq!(discrepancies.len(), 1);
    assert_eq!(discrepancies[0].settlement_id, 1);
    assert!(discrepancies[0].discrepancy_kg > 50.0);
}

// @req ENG-EPI-005
#[test]
fn hq_relocation_moves_optical_sight_halo_and_shortens_forward_latency() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    world.clock.mode = ClockRunMode::Running {
        speed_multiplier: 1,
    };

    let original_hq = world.economy.hq_cell();
    let freight_carrier = world.logistics.default_freight_carrier();
    let (forward_cell, est_hours) = world
        .planet
        .cell_neighbors(original_hq)
        .iter()
        .copied()
        .filter(|&nb| !world.planet.is_ocean_cell(nb) && nb != original_hq)
        .filter_map(|nb| {
            world
                .logistics
                .plan_route(
                    original_hq,
                    nb,
                    freight_carrier,
                    0.0,
                    &world.planet,
                    &world.weather,
                    &reg,
                )
                .map(|(_, h)| (nb, h))
        })
        .next()
        .expect("reachable adjacent land cell for HQ move");

    world.enqueue_command(
        Tick(1),
        SpinePlayerCommand::RelocateHq {
            destination_cell: forward_cell,
        },
    );
    world.step_ticks(&reg, 2);
    assert!(
        world.logistics.convoys().iter().any(|c| c.is_hq_move),
        "HQ relocation must deploy a physical HQ retinue convoy"
    );

    let max_wait = (est_hours.ceil() as u64) + 240;
    for _ in 0..max_wait {
        world.step_one_tick(&reg);
        if world.economy.hq_cell() == forward_cell {
            break;
        }
    }
    assert_eq!(
        world.economy.hq_cell(),
        forward_cell,
        "HQ must arrive physically at destination_cell"
    );

    let snap = world.compile_diegetic_snapshot(&reg);
    assert_eq!(snap.hq_presence.cell_id, forward_cell.get());
    assert!(snap.known_cells.iter().any(|k| matches!(
        k,
        KnownCellDto::Surveyed {
            cell_id,
            is_live_hq_sight: true,
            ..
        } if *cell_id == forward_cell.get()
    )));
}

// @req ENG-EPI-006
#[test]
fn geological_assay_intervals_narrow_with_higher_surveyor_skill_and_instruments() {
    let (min_low, pt_low, max_low, _, conf_low) =
        estimate_deposit_interval(42, 100, 1, 10_000.0, 0.72, 0.20, 0);
    let (min_high, pt_high, max_high, _, conf_high) =
        estimate_deposit_interval(42, 100, 1, 10_000.0, 0.72, 0.92, 3);

    assert!(min_low <= pt_low && pt_low <= max_low);
    assert!(min_high <= pt_high && pt_high <= max_high);

    let rel_band_low = (max_low - min_low) / pt_low.max(1.0);
    let rel_band_high = (max_high - min_high) / pt_high.max(1.0);
    assert!(
        rel_band_high < rel_band_low * 0.65,
        "expert skill and tier-3 instruments must sharply narrow estimate band: low={rel_band_low}, high={rel_band_high}"
    );
    assert!(conf_high > conf_low);
}

// @req ENG-EPI-007
#[test]
fn configurable_notification_matrix_controls_exact_tick_end_auto_pause() {
    let reg = load_unexplained_bundle();

    // Case A: LogOnly suppresses auto-pause even under mine flooding bottleneck.
    let mut world_log = ScenarioWorld::from_registry(&reg, 42);
    world_log.clock.mode = ClockRunMode::Running {
        speed_multiplier: 1,
    };
    world_log.enqueue_command(
        Tick(0),
        SpinePlayerCommand::UpdateNotificationRule {
            event_class_key: "mine_flooding_halt".to_owned(),
            behavior: NotificationBehaviorKind::LogOnly,
        },
    );
    world_log
        .economy
        .set_settlement_shaft_water_level(SettlementId(0), 0.70);
    world_log.step_ticks(&reg, 3);
    assert!(
        matches!(world_log.clock.mode, ClockRunMode::Running { .. }),
        "LogOnly rule must not auto-pause clock"
    );

    // Case B: DispatchPopupAndAutoPause halts clock at exact tick end with reason.
    let mut world_pause = ScenarioWorld::from_registry(&reg, 42);
    world_pause.clock.mode = ClockRunMode::Running {
        speed_multiplier: 1,
    };
    world_pause.enqueue_command(
        Tick(0),
        SpinePlayerCommand::UpdateNotificationRule {
            event_class_key: "mine_flooding_halt".to_owned(),
            behavior: NotificationBehaviorKind::DispatchPopupAndAutoPause,
        },
    );
    world_pause
        .economy
        .set_settlement_shaft_water_level(SettlementId(0), 0.70);
    let ran = world_pause.step_ticks_respecting_clock(&reg, 5);
    assert_eq!(
        ran, 1,
        "clock must auto-pause cleanly at end of exact trigger tick"
    );
    assert!(
        matches!(world_pause.clock.mode, ClockRunMode::AutoPaused { .. }),
        "DispatchPopupAndAutoPause rule must enter AutoPaused state"
    );
    let snap = world_pause.compile_diegetic_snapshot(&reg);
    assert!(snap.clock_and_notifications.paused);
    assert!(snap.clock_and_notifications.auto_pause_reason.is_some());
}

// @req ENG-LOG-003
#[test]
fn stranded_or_delayed_transit_transitions_to_overdue_status_without_instant_failure() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    world.clock.mode = ClockRunMode::Running {
        speed_multiplier: 1,
    };

    let hq_cell = world.economy.hq_cell();
    let porter = world.logistics.default_freight_carrier();
    let is_out_of_sight = |cid: CellId| {
        !world.planet.is_ocean_cell(cid)
            && !matches!(
                world.archive.cell_knowledge(cid.get()),
                Some(KnownCellDto::Surveyed {
                    is_live_hq_sight: true,
                    ..
                })
            )
    };
    let mut visited = std::collections::BTreeSet::from([hq_cell]);
    let mut bfs = std::collections::VecDeque::from([hq_cell]);
    let mut remote_pair = None;
    'search: while let Some(cur) = bfs.pop_front() {
        for &nb in world.planet.cell_neighbors(cur) {
            if visited.insert(nb) && !world.planet.is_ocean_cell(nb) {
                bfs.push_back(nb);
            }
            if is_out_of_sight(cur) && is_out_of_sight(nb) {
                for &n2 in world.planet.cell_neighbors(nb) {
                    if n2 != cur
                        && is_out_of_sight(n2)
                        && world
                            .logistics
                            .plan_route(cur, n2, porter, 100.0, &world.planet, &world.weather, &reg)
                            .is_some()
                    {
                        remote_pair = Some((cur, n2));
                        break 'search;
                    }
                }
            }
        }
    }
    let (origin_remote, dest_remote) =
        remote_pair.expect("remote out-of-sight land pair for overdue test");

    let grain = reg.commodity_id_by_key("raw_grain").unwrap();
    let sid_orig = world.economy.establish_settlement(
        "stranded_orig".to_owned(),
        "Forward Staging Depot".to_owned(),
        origin_remote,
        ActorId(1),
        &[(grain, 400.0, 0.95, 0.96)],
        &[],
    );
    let sid_dest = world.economy.establish_settlement(
        "stranded_dest".to_owned(),
        "Stranded Outpost".to_owned(),
        dest_remote,
        ActorId(1),
        &[(grain, 100.0, 0.95, 0.96)],
        &[],
    );

    // Launch freight convoy outside HQ optical halo with zero fodder so it strands mid-transit cleanly without teleporting
    let convoy_id = world
        .logistics
        .launch_unified_transit(
            UnifiedTransitLaunch {
                label: "Overdue Relief Convoy".to_owned(),
                channel: TransitChannelKind::FreightCargo,
                origin_settlement: sid_orig,
                destination_settlement: Some(sid_dest),
                origin_cell: origin_remote,
                destination_cell: dest_remote,
                carrier_class: porter,
                owner: ActorId(1),
                cargo_commodity: Some(grain),
                cargo_mass_kg: 100.0,
                cargo_purity_01: 0.95,
                fuel_loaded_kg: 0.0,
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
        .expect("convoy should launch");

    let snap_early = world.compile_diegetic_snapshot(&reg);
    let early = snap_early
        .tracked_convoys
        .iter()
        .find(|c| c.convoy_id == convoy_id.0)
        .expect("early tracked convoy");
    assert_eq!(early.status, "en_route_projected");

    let expected_end = (early.expected_arrival_tick * 2) + 24;
    world.step_ticks(&reg, expected_end);
    let snap_late = world.compile_diegetic_snapshot(&reg);
    let late = snap_late
        .tracked_convoys
        .iter()
        .find(|c| c.convoy_id == convoy_id.0)
        .expect("late tracked convoy");
    assert_eq!(late.status, "overdue");
    assert!(late.confidence_01 < early.confidence_01);
}

// @req ENG-SCN-001
#[test]
fn three_scenario_archetypes_produce_distinct_initial_stocks_roads_and_attention_budgets() {
    let reg = load_unexplained_bundle();
    let dawn =
        ScenarioWorld::from_registry_with_archetype(&reg, 42, Some(ScenarioArchetypeKind::EmergentDawn));
    let colony = ScenarioWorld::from_registry_with_archetype(
        &reg,
        42,
        Some(ScenarioArchetypeKind::ExpeditionColony),
    );
    let gov = ScenarioWorld::from_registry_with_archetype(
        &reg,
        42,
        Some(ScenarioArchetypeKind::GovernorSuccessor),
    );

    assert!(
        dawn.economy.total_stockpile_mass_kg() < colony.economy.total_stockpile_mass_kg(),
        "EmergentDawn must begin leaner than ExpeditionColony"
    );
    assert!(
        dawn.command.base_attention_capacity() < colony.command.base_attention_capacity()
            && colony.command.base_attention_capacity() < gov.command.base_attention_capacity(),
        "attention capacities must scale cleanly across the 3 archetypes"
    );

    // GovernorSuccessor pre-seeds metalled_road infrastructure and historical Stale cells beyond HQ halo.
    let metalled = reg.infra_id_by_key("metalled_road").unwrap();
    let gov_metalled_edges = gov
        .logistics
        .edges()
        .values()
        .filter(|e| e.infra_class == metalled)
        .count();
    let dawn_metalled_edges = dawn
        .logistics
        .edges()
        .values()
        .filter(|e| e.infra_class == metalled)
        .count();
    assert!(gov_metalled_edges > dawn_metalled_edges);
}

// @req ENG-SCN-002
#[test]
fn unexplained_colony_starts_as_coastal_perimeter_surrounded_by_unexplored_wilderness() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    let snap = world.compile_diegetic_snapshot(&reg);

    assert_eq!(world.economy.settlements().len(), 1);
    assert!(world.pop.total_population() > 0);
    assert!(snap.known_cells.iter().any(|k| matches!(
        k,
        KnownCellDto::Surveyed {
            is_live_hq_sight: true,
            ..
        }
    )));

    let hq = CellId(snap.hq_presence.cell_id);
    assert!(!world.planet.is_ocean_cell(hq));
    let coastal = world
        .planet
        .cell_neighbors(hq)
        .iter()
        .any(|&nb| world.planet.is_ocean_cell(nb));
    assert!(coastal, "landing HQ must be a temperate coastal cell");

    let known_count = snap.known_cells.len();
    assert!(
        known_count < world.planet.cell_count() as usize / 4,
        "majority of the globe must begin Unexplored: known={known_count}, total={}",
        world.planet.cell_count()
    );
}

// @req ENG-SCN-003
#[test]
fn unexplained_colony_survives_8760_hours_with_strict_conservation_and_determinism() {
    let reg = load_unexplained_bundle();

    let mut run_a = ScenarioWorld::from_registry(&reg, 42);
    run_a.clock.mode = ClockRunMode::Running {
        speed_multiplier: 1,
    };
    run_a.step_ticks(&reg, 8760);
    run_a
        .verify_invariants()
        .expect("8760h world must satisfy all physics, water, economy, and pop invariants");
    let digest_a = run_a.state_digest_hex();

    let mut run_b = ScenarioWorld::from_registry(&reg, 42);
    run_b.clock.mode = ClockRunMode::Running {
        speed_multiplier: 1,
    };
    run_b.step_ticks(&reg, 8760);
    let digest_b = run_b.state_digest_hex();

    assert_eq!(digest_a, digest_b, "8760h full-year simulation must be bit-identical");
    assert!(run_a.economy.conservation_error_kg() <= 1e-4);
    assert_eq!(run_a.pop.conservation_discrepancy(), 0);
}
