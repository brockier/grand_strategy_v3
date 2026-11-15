#![deny(clippy::disallowed_types)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use player_view::{perturb_reported_quantity, ReportedLedgerRowDto};
use sim_data::load_mod_bundle;
use sim_logistics::UnifiedTransitLaunch;
use sim_plug_private_eco::PrivateEconomyPlugin;
use sim_scenario::{resolve_scenario_pack_stack, ScenarioWorld};
use sim_spine::{
    ActorId, CellId, CharacterId, CharacterSnapshotView, GovernanceFrictionPlugPort,
    ModRegistryPort, NonStateActorKind, NonStateEconomyPlugPort, PhysicalLedgerPort,
    PlanetGeometryPort, SettlementId, SpinePlayerCommand, Tick, TransitChannelKind,
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

// @req ENG-CMD-006.1
// @req ENG-CMD-006.2
#[test]
fn hierarchy_hop_lag_sycophancy_filtering_and_audit_detection() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);

    // Baseline expedition council has modest hierarchy hop latency.
    let baseline_gov = world
        .private_eco
        .governance_state()
        .expect("governance state available");
    world.apply_player_command_now(
        &reg,
        SpinePlayerCommand::SwitchGovernanceModel {
            model_key: "feudal_vassalage".to_owned(),
        },
    );
    let feudal_gov = world
        .private_eco
        .governance_state()
        .expect("feudal governance state available");
    assert!(
        feudal_gov.hierarchy_hop_latency_hours >= baseline_gov.hierarchy_hop_latency_hours,
        "feudal vassalage should preserve or increase hierarchy hop reporting lag"
    );

    // Goodhart quota gaming and upward sycophantic filtering mask actual shortfall until direct audit.
    let sycophant = CharacterSnapshotView {
        id: CharacterId(401),
        key: "sycophant_intendant".to_owned(),
        display_name: "Clerk Vance".to_owned(),
        role_title: "Depot Intendant".to_owned(),
        home_settlement: SettlementId(0),
        competence: 0.72,
        precision_01: 0.85,
        honesty_vs_sycophancy: -0.92,
        caution_vs_alarmism: -0.10,
    };
    let true_kg = 900.0;
    let (reported_kg, min_kg, max_kg, _band) =
        perturb_reported_quantity(42, 48, 0, 0, true_kg, &sycophant, 0.42);
    assert!(
        reported_kg > true_kg && max_kg >= reported_kg && min_kg <= reported_kg,
        "sycophantic quota inflation must exceed physical ground-truth stock: reported={reported_kg}, true={true_kg}"
    );

    world.archive.record_remote_ledger_report(ReportedLedgerRowDto {
        settlement_id: 0,
        settlement_name: "Haven Landing Perimeter".to_owned(),
        commodity_key: "raw_grain".to_owned(),
        commodity_name: "Raw Grain".to_owned(),
        reported_mass_kg: reported_kg,
        min_est_kg: min_kg,
        max_est_kg: max_kg,
        observed_tick: 48,
        provenance_label: "Dispatched Quota Ledger".to_owned(),
        category: "food".to_owned(),
        confidence_01: 0.85,
        reporter_name: sycophant.display_name.clone(),
    });
    world.archive.audit_settlement_against_direct_sight(
        Tick(60),
        0,
        sycophant.id.0,
        &sycophant.display_name,
        "raw_grain",
        true_kg,
    );
    let discrepancies = world.archive.audit_discrepancies();
    assert!(
        !discrepancies.is_empty(),
        "direct audit must detect Goodhart/sycophantic quota inflation discrepancy"
    );
    assert!(discrepancies[0].discrepancy_kg > 100.0);

    // High skimming samples increment bureaucratic capture counter inside PrivateEconomyPlugin.
    let before_captures = world
        .private_eco
        .governance_state()
        .expect("gov state")
        .bureaucratic_capture_events;
    world.private_eco.step_with_world_context(
        Tick(24),
        &world.economy,
        &world.logistics,
        &world.planet,
        &reg,
        &[(CharacterId(1), 0.45)],
        SettlementId(0),
        2.0,
    );
    let after_gov = world.private_eco.governance_state().expect("gov state");
    assert!(after_gov.bureaucratic_capture_events > before_captures);
    assert!(after_gov.skimming_leakage_fraction > 0.05);
}

// @req ENG-CMD-006.3
// @req ENG-CMD-006.4
#[test]
fn clerical_overhead_draw_and_institutional_reform_inertia_recovery() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    let target_cell = world.economy.hq_cell();

    let overhead_initial = world.command.clerical_overhead_total();
    world.apply_player_command_now(
        &reg,
        SpinePlayerCommand::IssueMandate {
            assignee: CharacterId(0),
            objective_kind: "MaintainAgriculture".to_owned(),
            target_cell,
            priority_weight: 5,
            authority_tier: "OperationalGoal".to_owned(),
        },
    );
    let overhead_one_mandate = world.command.clerical_overhead_total();
    assert!(
        overhead_one_mandate > overhead_initial,
        "active mandates must draw non-productive clerical overhead ({overhead_one_mandate} vs {overhead_initial})"
    );

    // Reform switch induces immediate adaptation shock and reduces oversight efficiency multiplier.
    let steady_oversight = world
        .private_eco
        .oversight_efficiency_multiplier(SettlementId(0), CharacterId(0));
    world.apply_player_command_now(
        &reg,
        SpinePlayerCommand::SwitchGovernanceModel {
            model_key: "central_planning_commissariat".to_owned(),
        },
    );
    let post_switch = world
        .private_eco
        .governance_state()
        .expect("governance after switch");
    assert!((post_switch.adaptation_progress_01 - 0.20).abs() < 1e-4);
    assert!(post_switch.previous_model_key.is_some());
    let shocked_oversight = world
        .private_eco
        .oversight_efficiency_multiplier(SettlementId(0), CharacterId(0));
    assert!(
        shocked_oversight < steady_oversight,
        "institutional reform transition must inflict oversight friction before recovery"
    );

    // Stepping 125 hourly ticks lets adaptation recover monotonically to 1.0 and clears previous model key.
    let mid_world = {
        world.step_ticks(&reg, 30);
        world
            .private_eco
            .governance_state()
            .expect("mid transition")
            .adaptation_progress_01
    };
    assert!(mid_world > post_switch.adaptation_progress_01 && mid_world < 1.0);

    world.step_ticks(&reg, 100);
    let recovered = world
        .private_eco
        .governance_state()
        .expect("recovered state");
    assert!((recovered.adaptation_progress_01 - 1.0).abs() < 1e-3);
    assert!(recovered.previous_model_key.is_none());
}

// @req ENG-CMD-007.1
// @req ENG-CMD-007.2
#[test]
fn externalized_governance_models_enforce_non_eliminable_trade_off_matrix() {
    let reg = load_unexplained_bundle();
    let models = reg.governance_models();
    assert!(
        models.len() >= 5,
        "at least 5 externalized governance models must load from mods/core-schemas"
    );

    let mut world = ScenarioWorld::from_registry(&reg, 42);
    let mut attention_caps = Vec::new();
    let mut clerical_overheads = Vec::new();
    let mut hop_latencies = Vec::new();

    for m in models {
        world.apply_player_command_now(
            &reg,
            SpinePlayerCommand::SwitchGovernanceModel {
                model_key: m.key.clone(),
            },
        );
        let gov = world
            .private_eco
            .governance_state()
            .expect("switched governance state");
        assert_eq!(gov.active_model_key, m.key);
        attention_caps.push(gov.effective_attention_capacity);
        clerical_overheads.push(gov.clerical_overhead_per_mandate);
        hop_latencies.push(gov.hierarchy_hop_latency_hours);
    }

    let min_attn = attention_caps.iter().copied().fold(f32::INFINITY, f32::min);
    let max_attn = attention_caps
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let min_ovh = clerical_overheads
        .iter()
        .copied()
        .fold(f32::INFINITY, f32::min);
    let max_ovh = clerical_overheads
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let min_hop = hop_latencies.iter().copied().min().unwrap_or(0);
    let max_hop = hop_latencies.iter().copied().max().unwrap_or(0);

    assert!(max_attn > min_attn, "attention capacity must vary across governance models");
    assert!(max_ovh > min_ovh, "clerical overhead must vary across governance models");
    assert!(max_hop > min_hop, "hierarchy hop latency must vary across governance models");

    // Verify no single model simultaneously minimizes overhead, maximizes attention, and minimizes latency.
    let strictly_dominant = models.iter().find(|m| {
        m.base_attention_capacity >= max_attn - 1e-4
            && m.clerical_overhead_per_mandate <= min_ovh + 1e-4
            && m.hierarchy_hop_latency_hours <= min_hop
    });
    assert!(
        strictly_dominant.is_none(),
        "governance matrix must preserve non-eliminable trade-offs without a strictly dominant model"
    );
}

// @req ENG-ECO-005.1
// @req ENG-ECO-005.2
// @req ENG-ECO-005.3
// @req ENG-ECO-005.4
#[test]
fn non_state_actors_retains_subsistence_hoards_arbitrages_and_accumulates_capital() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    let grain_id = reg
        .commodity_id_by_key("raw_grain")
        .expect("raw_grain commodity exists");

    // Establish a food-scarce second settlement connected across passable land.
    let cell2 = find_reachable_land_cell(&world, 2);
    let sid2 = world.economy.establish_settlement(
        "frontier_outpost".to_owned(),
        "Frontier Outpost".to_owned(),
        cell2,
        ActorId(0),
        &[(grain_id, 25.0, 0.90, 0.90)],
        &[],
    );
    world.private_eco = PrivateEconomyPlugin::from_registry(&reg, &[SettlementId(0), sid2]);

    // Enact grain tithe to verify estate/smallholder retention and hoarding friction alongside arbitrage.
    world.apply_player_command_now(
        &reg,
        SpinePlayerCommand::EnactEconomicPolicy {
            policy_key: "grain_tithe".to_owned(),
            rate_override_01: Some(0.28),
        },
    );

    let actors_before = world.private_eco.non_state_actors().to_vec();
    let merchant_cap_before: f64 = actors_before
        .iter()
        .filter(|a| a.kind == NonStateActorKind::MerchantHouse)
        .map(|a| a.working_capital_coins)
        .sum();

    // Advance 48 ticks so daily arbitrage cadence (tick % 24 == 0) fires multiple times.
    let mut observed_arbitrage_dispatches = 0usize;
    for t in 1..=48u64 {
        world.private_eco.step_with_world_context(
            Tick(t),
            &world.economy,
            &world.logistics,
            &world.planet,
            &reg,
            &[],
            SettlementId(0),
            2.0,
        );
        let reqs = world.private_eco.drain_arbitrage_requests();
        observed_arbitrage_dispatches += reqs.len();
        for req in reqs {
            assert_ne!(req.origin_settlement, req.destination_settlement);
            assert!(req.mass_kg >= 10.0);
        }
    }

    let actors_after = world.private_eco.non_state_actors();
    assert!(
        actors_after
            .iter()
            .any(|a| a.kind == NonStateActorKind::IndependentSmallholders && a.held_mass_kg > 0.0),
        "smallholders must retain subsistence grain stocks (`ENG-ECO-005.1`)"
    );
    assert!(
        actors_after
            .iter()
            .any(|a| a.kind == NonStateActorKind::ClanEstate && a.hoarded_mass_kg > 0.0 && a.compliance_01 < 0.95),
        "clans/estates must divert stock into hoards under tax pressure (`ENG-ECO-005.2`)"
    );
    let merchant_cap_after: f64 = actors_after
        .iter()
        .filter(|a| a.kind == NonStateActorKind::MerchantHouse)
        .map(|a| a.working_capital_coins)
        .sum();
    assert!(
        observed_arbitrage_dispatches > 0 && merchant_cap_after > merchant_cap_before,
        "merchant houses must dispatch inter-settlement arbitrage and realize price margins (`ENG-ECO-005.3`)"
    );
    assert!(
        actors_after.iter().any(|a| {
            matches!(
                a.kind,
                NonStateActorKind::CraftGuild | NonStateActorKind::CharteredCompany
            ) && a.private_carriers_owned > 0
                && a.working_capital_coins > 0.0
        }),
        "guilds/chartered companies must maintain capital and private carrier capacity (`ENG-ECO-005.4`)"
    );
    assert!(
        world.economy.conservation_error_kg().abs() < 1e-6,
        "global ledger conservation error must stay below 1e-6 kg"
    );
}

// @req ENG-ECO-006.1
// @req ENG-ECO-006.2
#[test]
fn era_universal_barter_tribute_and_coinage_clearing_across_institutions() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);

    // Barter / Tribute / Rationing clearing in early tribal_chiefdom without coinage minting.
    world.apply_player_command_now(
        &reg,
        SpinePlayerCommand::SwitchGovernanceModel {
            model_key: "tribal_chiefdom".to_owned(),
        },
    );
    world.apply_player_command_now(
        &reg,
        SpinePlayerCommand::EnactEconomicPolicy {
            policy_key: "grain_tithe".to_owned(),
            rate_override_01: Some(0.25),
        },
    );
    world.step_ticks(&reg, 24);
    let markets_tribute = world.private_eco.informal_markets().to_vec();
    assert!(
        !markets_tribute.is_empty(),
        "tribute/barter clearing must track informal market clearing indices"
    );
    assert!(
        markets_tribute
            .iter()
            .any(|m| m.clearing_price_index > 0.45 && m.hoarded_reserve_kg >= 0.0)
    );

    // Coinage / Commercial clearing under constitutional_cabinet and customs_toll.
    world.apply_player_command_now(
        &reg,
        SpinePlayerCommand::SwitchGovernanceModel {
            model_key: "constitutional_cabinet".to_owned(),
        },
    );
    world.apply_player_command_now(
        &reg,
        SpinePlayerCommand::EnactEconomicPolicy {
            policy_key: "customs_toll".to_owned(),
            rate_override_01: Some(0.32),
        },
    );
    world.step_ticks(&reg, 24);
    let markets_commercial = world.private_eco.informal_markets().to_vec();
    assert!(
        markets_commercial
            .iter()
            .any(|m| m.black_market_premium_01 > 0.0 && m.black_market_leakage_kg >= 0.0),
        "commercial customs regime must calculate clearing indices and smuggling leakage"
    );
    assert!(world.economy.conservation_error_kg().abs() < 1e-6);
}

// @req ENG-ECO-007.1
// @req ENG-ECO-007.2
#[test]
fn cross_era_economic_policy_levers_and_structural_side_effects() {
    let reg = load_unexplained_bundle();
    let policies = reg.policy_laws();
    assert!(
        policies.len() >= 4,
        "at least 4 cross-era economic policy laws must be externalized"
    );

    let mut world = ScenarioWorld::from_registry(&reg, 42);
    for p in policies {
        world.apply_player_command_now(
            &reg,
            SpinePlayerCommand::EnactEconomicPolicy {
                policy_key: p.key.clone(),
                rate_override_01: None,
            },
        );
        let gov = world
            .private_eco
            .governance_state()
            .expect("policy enacted governance state");
        assert_eq!(gov.active_policy_key, p.key);
    }

    // Coercive override produces black-market leakage, hoarding surge, and oligarchic charter entrenchment.
    world.apply_player_command_now(
        &reg,
        SpinePlayerCommand::EnactEconomicPolicy {
            policy_key: "emergency_rationing".to_owned(),
            rate_override_01: Some(0.65),
        },
    );
    world.step_ticks(&reg, 12);
    let coerced_markets = world.private_eco.informal_markets();
    assert!(
        coerced_markets
            .iter()
            .any(|m| m.black_market_premium_01 > 0.30 && m.black_market_leakage_kg > 0.0 && m.hoarded_reserve_kg > 0.0),
        "heavy rationing/tax rate override must spawn black-market premium, hoarding, and leakage"
    );

    // Enacting inventor_patent_act sets non-zero patent duration window.
    world.apply_player_command_now(
        &reg,
        SpinePlayerCommand::EnactEconomicPolicy {
            policy_key: "inventor_patent_act".to_owned(),
            rate_override_01: Some(0.15),
        },
    );
    let pat_gov = world.private_eco.governance_state().expect("patent policy");
    assert!(pat_gov.patent_duration_years > 0);
}

// @req ENG-LOG-005.1
// @req ENG-LOG-005.2
#[test]
fn shared_corridor_axle_congestion_slows_speed_and_accrues_physical_wear() {
    let reg = load_unexplained_bundle();
    let mut world_clear = ScenarioWorld::from_registry(&reg, 42);
    let hq_cell = world_clear.economy.hq_cell();
    let dst_cell = find_reachable_land_cell(&world_clear, 2);
    let porter = world_clear.logistics.default_freight_carrier();
    let wagon = reg
        .carrier_id_by_key("ox_freight_wagon")
        .expect("ox_freight_wagon");
    let metalled = reg
        .infra_id_by_key("metalled_road")
        .expect("metalled_road");
    let (route_cells, _) = world_clear
        .logistics
        .plan_route(
            hq_cell,
            dst_cell,
            porter,
            0.0,
            &world_clear.planet,
            &world_clear.weather,
            &reg,
        )
        .expect("land corridor route exists");
    let grain = reg.commodity_id_by_key("raw_grain").expect("raw_grain");

    // Uncongested single convoy baseline on improved road.
    for pair in route_cells.windows(2) {
        if let Some(eid) = world_clear.planet.cell_edge(pair[0], pair[1]) {
            world_clear.logistics.upgrade_edge_infra(eid, metalled, None);
        }
    }
    let solo_id = world_clear
        .logistics
        .launch_unified_transit(
            UnifiedTransitLaunch {
                label: "Solo Freight".to_owned(),
                channel: TransitChannelKind::FreightCargo,
                origin_settlement: SettlementId(0),
                destination_settlement: None,
                origin_cell: hq_cell,
                destination_cell: dst_cell,
                carrier_class: wagon,
                owner: ActorId(1),
                cargo_commodity: Some(grain),
                cargo_mass_kg: 120.0,
                cargo_purity_01: 0.9,
                fuel_loaded_kg: 40.0,
                spare_parts_loaded_kg: 25.0,
                current_tick: Tick(1),
                is_hq_move: false,
                dispatch_id: None,
                sender: None,
                mandate_payload_id: None,
                subject: "Solo Grain Shipment".to_owned(),
            },
            &world_clear.planet,
            &world_clear.weather,
            &reg,
        )
        .expect("solo convoy launch succeeds");

    // Congested multi-convoy corridor sharing identical road edges.
    let mut world_busy = ScenarioWorld::from_registry(&reg, 42);
    for pair in route_cells.windows(2) {
        if let Some(eid) = world_busy.planet.cell_edge(pair[0], pair[1]) {
            world_busy.logistics.upgrade_edge_infra(eid, metalled, None);
        }
    }
    let mut tracked_busy_id = solo_id;
    for idx in 0..8u32 {
        let cid = world_busy
            .logistics
            .launch_unified_transit(
                UnifiedTransitLaunch {
                    label: format!("Column {idx}"),
                    channel: TransitChannelKind::FreightCargo,
                    origin_settlement: SettlementId(0),
                    destination_settlement: None,
                    origin_cell: hq_cell,
                    destination_cell: dst_cell,
                    carrier_class: wagon,
                    owner: ActorId(1),
                    cargo_commodity: Some(grain),
                    cargo_mass_kg: 480.0,
                    cargo_purity_01: 0.9,
                    fuel_loaded_kg: 60.0,
                    spare_parts_loaded_kg: 40.0,
                    current_tick: Tick(1),
                    is_hq_move: false,
                    dispatch_id: None,
                    sender: None,
                    mandate_payload_id: None,
                    subject: format!("Convoy Column {idx}"),
                },
                &world_busy.planet,
                &world_busy.weather,
                &reg,
            )
            .expect("busy convoy launch succeeds");
        if idx == 0 {
            tracked_busy_id = cid;
        }
    }

    let mut total_wear_solo = 0.0;
    let mut total_wear_busy = 0.0;
    for t in 1..=3u64 {
        world_clear.private_eco.step_with_world_context(
            Tick(t),
            &world_clear.economy,
            &world_clear.logistics,
            &world_clear.planet,
            &reg,
            &[],
            SettlementId(0),
            2.0,
        );
        let s_solo = world_clear.logistics.step_hourly_with_congestion(
            Tick(t),
            &world_clear.planet,
            &world_clear.weather,
            &reg,
            Some(&world_clear.private_eco),
        );
        total_wear_solo += s_solo.total_spares_worn_kg;

        world_busy.private_eco.step_with_world_context(
            Tick(t),
            &world_busy.economy,
            &world_busy.logistics,
            &world_busy.planet,
            &reg,
            &[],
            SettlementId(0),
            2.0,
        );
        let s_busy = world_busy.logistics.step_hourly_with_congestion(
            Tick(t),
            &world_busy.planet,
            &world_busy.weather,
            &reg,
            Some(&world_busy.private_eco),
        );
        total_wear_busy += s_busy.total_spares_worn_kg;
    }

    let congested = world_busy.private_eco.congested_edges();
    assert!(
        congested.iter().any(|e| e.congestion_multiplier > 1.05),
        "shared corridor traffic above throughput capacity must report congestion_multiplier > 1.05"
    );

    let solo_dist = world_clear
        .logistics
        .convoys()
        .iter()
        .find(|c| c.convoy_id == solo_id)
        .expect("solo convoy present")
        .distance_travelled_km;
    let busy_dist = world_busy
        .logistics
        .convoys()
        .iter()
        .find(|c| c.convoy_id == tracked_busy_id)
        .expect("busy convoy present")
        .distance_travelled_km;
    assert!(
        busy_dist < solo_dist,
        "congested corridor must slow hourly travel progress: busy={busy_dist}km vs solo={solo_dist}km"
    );
    assert!(
        total_wear_busy > total_wear_solo && total_wear_busy > 0.0,
        "heavy multi-axle corridor traffic must accrue physical spare-parts/rig wear: busy={total_wear_busy} vs solo={total_wear_solo}"
    );
}

// @req ENG-LOG-006.1
// @req ENG-LOG-006.2
#[test]
fn state_carrier_chartering_scales_pool_and_crowds_out_civilian_freight() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    let grain_id = reg.commodity_id_by_key("raw_grain").expect("raw_grain");
    let cell2 = find_reachable_land_cell(&world, 2);
    let sid2 = world.economy.establish_settlement(
        "outpost_west".to_owned(),
        "Outpost West".to_owned(),
        cell2,
        ActorId(0),
        &[(grain_id, 10.0, 0.90, 0.90)],
        &[],
    );

    // Baseline unchartered world yields larger private merchant arbitrage cargoes.
    let mut baseline = ScenarioWorld::from_registry(&reg, 42);
    baseline.private_eco = PrivateEconomyPlugin::from_registry(&reg, &[SettlementId(0), sid2]);
    baseline.private_eco.step_with_world_context(
        Tick(24),
        &world.economy,
        &baseline.logistics,
        &baseline.planet,
        &reg,
        &[],
        SettlementId(0),
        2.0,
    );
    let uncrowded_mass: f64 = baseline
        .private_eco
        .drain_arbitrage_requests()
        .iter()
        .map(|r| r.mass_kg)
        .sum();

    // State requisitions regional private carriers -> available pool drops, civilian arbitrage shrinks.
    world.private_eco = PrivateEconomyPlugin::from_registry(&reg, &[SettlementId(0), sid2]);
    let avail_before = world.private_eco.available_private_carriers();
    assert!(avail_before > 2);

    world.apply_player_command_now(
        &reg,
        SpinePlayerCommand::CharterPrivateCarriers {
            target_settlement: SettlementId(0),
            count: avail_before,
        },
    );

    let avail_after = world.private_eco.available_private_carriers();
    let chartered_total = world.private_eco.total_chartered_carriers();
    assert!(avail_after < avail_before, "chartering must draw down available regional carrier pool");
    assert_eq!(chartered_total, avail_before - avail_after);
    assert!(
        world
            .private_eco
            .non_state_actors()
            .iter()
            .any(|a| a.chartered_to_state_carriers > 0)
    );

    world.private_eco.step_with_world_context(
        Tick(24),
        &world.economy,
        &world.logistics,
        &world.planet,
        &reg,
        &[],
        SettlementId(0),
        2.0,
    );
    let crowded_mass: f64 = world
        .private_eco
        .drain_arbitrage_requests()
        .iter()
        .map(|r| r.mass_kg)
        .sum();
    assert!(
        crowded_mass < uncrowded_mass,
        "state carrier chartering must crowd out civilian arbitrage capacity: crowded={crowded_mass} vs uncrowded={uncrowded_mass}"
    );
}
