#![deny(clippy::disallowed_types)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use sim_clock::ClockRunMode;
use sim_command::{CommandHierarchy, MandateStatus};
use sim_data::load_mod_bundle;
use sim_pop::PopulationRegistry;
use sim_scenario::{resolve_scenario_pack_stack, ScenarioWorld};
use sim_spine::{
    ActorId, CellId, CharacterId, CommandPort, ModRegistryPort, PhysicalLedgerPort,
    PlanetGeometryPort, PopulationPort, SettlementId, SpinePlayerCommand, Tick, VocationId,
};

fn find_vocation_id(reg: &dyn ModRegistryPort, key: &str) -> VocationId {
    (0..reg.vocation_count())
        .map(|i| VocationId(i as u16))
        .find(|&id| reg.vocation_def(id).is_some_and(|v| v.key == key))
        .unwrap_or(VocationId(0))
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

// @req ENG-POP-001
#[test]
fn population_cohorts_stay_bounded_and_conserve_exact_integer_headcount() {
    let reg = load_unexplained_bundle();
    let mut pop = PopulationRegistry::new();
    let voc = find_vocation_id(&reg, "agronomist");

    let adult_arch = (0..reg.cohort_archetype_count())
        .find(|&i| {
            reg.cohort_archetype_def(i)
                .is_some_and(|a| a.key == "founding_colonist_adult")
        })
        .expect("founding_colonist_adult archetype");

    // Scale integer headcount dramatically across cohorts without allocating per-person records.
    pop.seed_settlement_cohorts(
        SettlementId(0),
        &[(voc, 50_000, adult_arch), (VocationId(1), 50_000, adult_arch)],
        &reg,
        "colony_charter",
        0.55,
    );
    assert_eq!(pop.all_cohorts().len(), 2);
    assert_eq!(pop.total_population(), 100_000);

    for hr in 0..720_u64 {
        pop.step_settlement_hourly(SettlementId(0), Tick(hr), 1.0, 1.0, 1.0, 1.0, 14.0, &reg);
    }

    assert_eq!(
        pop.all_cohorts().len(),
        2,
        "cohort cardinality must remain O(cohorts), not O(individuals)"
    );
    assert_eq!(pop.conservation_discrepancy(), 0);
    assert!(pop.cumulative_births() > 0);
    assert!(pop.cumulative_deaths() > 0);
}

// @req ENG-POP-002
#[test]
fn starvation_and_thirst_spike_mortality_while_migration_preserves_headcount() {
    let reg = load_unexplained_bundle();
    let mut pop_fed = PopulationRegistry::new();
    let mut pop_starved = PopulationRegistry::new();
    let voc = VocationId(0);

    pop_fed.seed_settlement_cohorts(SettlementId(0), &[(voc, 2_000, 0)], &reg, "fed", 0.5);
    pop_starved.seed_settlement_cohorts(SettlementId(0), &[(voc, 2_000, 0)], &reg, "starved", 0.5);

    for hr in 0..240_u64 {
        pop_fed.step_settlement_hourly(SettlementId(0), Tick(hr), 1.0, 1.0, 1.0, 1.0, 15.0, &reg);
        pop_starved.step_settlement_hourly(
            SettlementId(0),
            Tick(hr),
            0.05,
            0.05,
            0.20,
            0.20,
            2.0,
            &reg,
        );
    }

    assert!(
        pop_starved.cumulative_deaths() > pop_fed.cumulative_deaths() * 4,
        "severe starvation, thirst, and cold must multiply deaths: fed={}, starved={}",
        pop_fed.cumulative_deaths(),
        pop_starved.cumulative_deaths()
    );
    assert!(pop_fed.cumulative_births() >= pop_starved.cumulative_births());
    assert_eq!(pop_fed.conservation_discrepancy(), 0);
    assert_eq!(pop_starved.conservation_discrepancy(), 0);

    // Physical cohort migration splits headcount across settlements while keeping exact total sum.
    let src_id = pop_fed.all_cohorts()[0].id;
    let total_before = pop_fed.total_population();
    let dest_cid = pop_fed
        .migrate_cohort_segment(src_id, SettlementId(1), 250, &reg)
        .expect("migration splits segment");
    assert_ne!(dest_cid, src_id);
    assert_eq!(pop_fed.settlement_population(SettlementId(1)), 250);
    assert_eq!(pop_fed.total_population(), total_before);
    assert_eq!(pop_fed.conservation_discrepancy(), 0);
}

// @req ENG-POP-003
#[test]
fn cohort_skill_accrues_faster_when_working_and_decays_when_idle() {
    let reg = load_unexplained_bundle();
    let mut pop = PopulationRegistry::new();
    let voc = find_vocation_id(&reg, "smith");
    pop.seed_settlement_cohorts(
        SettlementId(0),
        &[(voc, 120, 0), (voc, 120, 0)],
        &reg,
        "guilds",
        0.50,
    );
    let working_id = pop.all_cohorts()[0].id;
    let idle_id = pop.all_cohorts()[1].id;
    pop.set_cohort_working(working_id, true);
    pop.set_cohort_working(idle_id, false);

    let initial_working = pop.cohort(working_id).unwrap().mean_skill_01;
    let initial_idle = pop.cohort(idle_id).unwrap().mean_skill_01;

    for hr in 0..720_u64 {
        pop.step_settlement_hourly(SettlementId(0), Tick(hr), 1.0, 1.0, 1.0, 1.0, 15.0, &reg);
    }

    let after_working = pop.cohort(working_id).unwrap().mean_skill_01;
    let after_idle = pop.cohort(idle_id).unwrap().mean_skill_01;
    assert!(
        after_working > initial_working,
        "working cohorts must gain skill: before={initial_working}, after={after_working}"
    );
    assert!(
        after_idle < initial_idle,
        "idle cohorts must lose skill via attrition decay: before={initial_idle}, after={after_idle}"
    );
}

// @req ENG-POP-004
#[test]
fn grievance_drives_graduated_unrest_stages_and_reduces_labor_efficiency() {
    let reg = load_unexplained_bundle();
    let mut pop = PopulationRegistry::new();
    pop.seed_settlement_cohorts(
        SettlementId(0),
        &[(VocationId(0), 250, 0)],
        &reg,
        "miners",
        0.50,
    );
    assert_eq!(
        pop.unrest_stage_for_settlement(SettlementId(0)),
        sim_spine::UnrestStage::Compliant
    );
    let base_eff = pop.labor_efficiency_multiplier(SettlementId(0));

    // Heavy taxation plus grain and water deficits escalate through Slowdown and Strike into OutMigration+.
    pop.set_settlement_tax_burden(SettlementId(0), 0.90);
    let mut saw_slowdown = false;
    let mut saw_strike = false;
    for hr in 0..160_u64 {
        let out = pop.step_settlement_hourly(
            SettlementId(0),
            Tick(hr),
            0.25,
            0.40,
            0.40,
            0.50,
            5.0,
            &reg,
        );
        if out.unrest_stage == sim_spine::UnrestStage::WorkSlowdown {
            saw_slowdown = true;
        }
        if out.unrest_stage == sim_spine::UnrestStage::LaborStrike {
            saw_strike = true;
        }
    }

    let final_stage = pop.unrest_stage_for_settlement(SettlementId(0));
    let unrest_eff = pop.labor_efficiency_multiplier(SettlementId(0));
    assert!(saw_slowdown, "unrest should pass through WorkSlowdown");
    assert!(saw_strike, "unrest should escalate through LaborStrike");
    assert!(final_stage >= sim_spine::UnrestStage::OutMigration);
    assert!(
        unrest_eff < base_eff * 0.5,
        "unrest must severely degrade labor efficiency: base={base_eff}, unrest={unrest_eff}"
    );
    assert_eq!(pop.conservation_discrepancy(), 0);
}

// @req ENG-POP-005
#[test]
fn promoting_named_individual_decrements_cohort_headcount_by_one_deterministically() {
    let reg = load_unexplained_bundle();
    let mut pop = PopulationRegistry::new();
    pop.seed_settlement_cohorts(
        SettlementId(0),
        &[(VocationId(1), 40, 0)],
        &reg,
        "settlers",
        0.62,
    );
    let cid = pop.all_cohorts()[0].id;
    let char_count_before = pop.all_characters().len();
    let total_pop_before = pop.total_population();

    let officer_id = pop
        .promote_from_cohort(
            cid,
            "promoted_warden_1".to_owned(),
            "Sergeant Maren Vos".to_owned(),
            "Foundry Overseer".to_owned(),
            99,
        )
        .expect("promote should succeed");

    assert_eq!(pop.cohort(cid).unwrap().headcount, 39);
    assert_eq!(pop.total_population(), total_pop_before - 1);
    assert_eq!(pop.all_characters().len(), char_count_before + 1);
    assert_eq!(pop.conservation_discrepancy(), 0);

    let view = pop.character_view(officer_id).expect("officer view");
    assert_eq!(view.display_name, "Sergeant Maren Vos");
    assert!(view.competence >= 0.10 && view.competence <= 0.98);
}

// @req ENG-CMD-001
#[test]
fn mandates_decompose_broad_objectives_and_support_doctrine_tiers() {
    let reg = load_unexplained_bundle();
    let world = ScenarioWorld::from_registry(&reg, 42);
    let chars = world.pop.all_characters().to_vec();
    let assignee = chars.first().map(|c| c.id).unwrap_or(CharacterId(1));
    let mut settlement_cells = BTreeMap::new();
    settlement_cells.insert(SettlementId(0), world.economy.settlements()[0].cell_id);

    let mut cmd = CommandHierarchy::default();
    cmd.apply_player_command(
        Tick(0),
        SpinePlayerCommand::IssueMandate {
            assignee,
            objective_kind: "stabilize_food".to_owned(),
            target_cell: world.economy.settlements()[0].cell_id,
            priority_weight: 6,
            authority_tier: "priorities".to_owned(),
        },
        &chars,
        &settlement_cells,
    );

    let mandate = cmd.all_mandates().first().expect("mandate created");
    assert_eq!(mandate.assignee, assignee);
    assert_eq!(mandate.authority_tier, "priorities");
    assert!(
        mandate.subtasks.len() >= 2,
        "high-level mandate must decompose into operational subtasks"
    );
    assert!(!cmd.active_mandates_for(CharacterId(0)).is_empty());
}

// @req ENG-CMD-002
#[test]
fn excessive_mandates_and_micromanagement_trigger_convex_attention_overload() {
    let reg = load_unexplained_bundle();
    let world = ScenarioWorld::from_registry(&reg, 42);
    let chars = world.pop.all_characters().to_vec();
    let assignee = chars.first().map(|c| c.id).unwrap_or(CharacterId(1));
    let hq_cell = world.economy.settlements()[0].cell_id;
    let mut settlement_cells = BTreeMap::new();
    settlement_cells.insert(SettlementId(0), hq_cell);

    let mut light = CommandHierarchy::with_config(100.0, 8.0, 4, BTreeMap::new());
    light.apply_player_command(
        Tick(0),
        SpinePlayerCommand::IssueMandate {
            assignee,
            objective_kind: "stabilize_food".to_owned(),
            target_cell: hq_cell,
            priority_weight: 4,
            authority_tier: "priorities".to_owned(),
        },
        &chars,
        &settlement_cells,
    );
    let m1 = light.all_mandates()[0].id;
    light.mark_order_delivered(m1, Tick(1));
    light.recompute_attention_and_efficiencies(&chars);
    let single_attn = light.attention_summary(CharacterId(0));
    let single_eff = light.all_mandates()[0].realized_efficiency;

    // Issue 7 strict no_retreat_operational mandates -> exceed bounded capacity & convex dilution.
    let mut heavy = CommandHierarchy::with_config(100.0, 8.0, 4, BTreeMap::new());
    for i in 0..7_u32 {
        heavy.apply_player_command(
            Tick(u64::from(i)),
            SpinePlayerCommand::IssueMandate {
                assignee,
                objective_kind: "repair_mine".to_owned(),
                target_cell: hq_cell,
                priority_weight: 9,
                authority_tier: "no_retreat_operational".to_owned(),
            },
            &chars,
            &settlement_cells,
        );
        let mid = heavy.all_mandates().last().unwrap().id;
        heavy.mark_order_delivered(mid, Tick(u64::from(i) + 1));
    }
    heavy.recompute_attention_and_efficiencies(&chars);
    let heavy_attn = heavy.attention_summary(CharacterId(0));
    let heavy_eff = heavy.all_mandates()[0].realized_efficiency;

    assert!(heavy_attn.used_attention > heavy_attn.capacity_attention);
    assert!(
        heavy_eff < single_eff * 0.50,
        "overloaded operational micromanagement must degrade realized efficiency convexly: single={single_eff}, heavy={heavy_eff}, used={}/{}",
        heavy_attn.used_attention,
        heavy_attn.capacity_attention
    );
    assert!(single_attn.used_attention < single_attn.capacity_attention);
}

// @req ENG-CMD-003
#[test]
fn mandates_travel_via_physical_courier_before_subordinate_activation() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    world.clock.mode = ClockRunMode::Running {
        speed_multiplier: 1,
    };

    let hq_cell = world.economy.hq_cell();
    let courier_carrier = world.logistics.default_courier_carrier();
    let (remote_cell, est_hours) = world
        .planet
        .cell_neighbors(hq_cell)
        .iter()
        .copied()
        .filter_map(|c| {
            if world.planet.is_ocean_cell(c) || c == hq_cell {
                return None;
            }
            world
                .logistics
                .plan_route(
                    hq_cell,
                    c,
                    courier_carrier,
                    0.0,
                    &world.planet,
                    &world.weather,
                    &reg,
                )
                .map(|(_, h)| (c, h))
        })
        .next()
        .expect("reachable remote land cell");
    let grain = reg.commodity_id_by_key("raw_grain").unwrap();
    let remote_sid = world.economy.establish_settlement(
        "far_outpost".to_owned(),
        "Far Outpost".to_owned(),
        remote_cell,
        ActorId(1),
        &[(grain, 200.0, 0.95, 0.96)],
        &[],
    );
    let remote_char_id = world.pop.seed_character_from_scenario(
        &sim_spine::ScenarioCharacterSeed {
            key: "remote_warden".to_owned(),
            display_name: "Warden Kell".to_owned(),
            role_title: "Outpost Commander".to_owned(),
            home_settlement_key: "far_outpost".to_owned(),
            competence: 0.75,
            precision_01: 0.82,
            honesty_vs_sycophancy: 0.5,
            caution_vs_alarmism: 0.1,
            initiative_01: 0.7,
            adaptability_01: 0.6,
            ambition_01: 0.3,
            loyalty_01: 0.85,
            greed_skimming_01: 0.02,
        },
        remote_sid,
    );

    world.enqueue_command(
        Tick(1),
        SpinePlayerCommand::IssueMandate {
            assignee: remote_char_id,
            objective_kind: "fortify_logistics".to_owned(),
            target_cell: remote_cell,
            priority_weight: 7,
            authority_tier: "priorities".to_owned(),
        },
    );

    world.step_one_tick(&reg);
    world.step_one_tick(&reg);
    let m0 = world
        .command
        .all_mandates()
        .first()
        .expect("mandate queued");
    assert_eq!(m0.status, MandateStatus::InTransitCourier);
    assert!(
        world
            .logistics
            .convoys()
            .iter()
            .any(|c| c.mandate_payload_id == Some(m0.id)),
        "order must travel aboard an explicit logistics courier convoy"
    );

    let max_wait = (est_hours.ceil() as u64) + 72;
    for _ in 0..max_wait {
        world.step_one_tick(&reg);
        if world.command.all_mandates()[0].status == MandateStatus::Active {
            break;
        }
    }
    assert_eq!(world.command.all_mandates()[0].status, MandateStatus::Active);
}

// @req ENG-CMD-004
#[test]
fn subordinates_escalate_arbitration_proposals_and_apply_chosen_consequences() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    world.clock.mode = ClockRunMode::Running {
        speed_multiplier: 1,
    };

    // Trigger mine shaft groundwater seepage bottleneck cleanly on SettlementId(0)
    world
        .economy
        .set_settlement_shaft_water_level(SettlementId(0), 0.62);
    world.pop.adjust_settlement_grievance(SettlementId(0), 0.25);
    let grievance_before = world
        .pop
        .settlement_welfare(SettlementId(0))
        .expect("welfare")
        .grievance_01;

    world.step_one_tick(&reg);
    let open = world.command.open_proposals();
    assert!(
        !open.is_empty(),
        "mine seepage threshold must escalate a structured arbitration proposal"
    );
    let prop_id = open[0].proposal_id;
    assert!(open[0].options.len() >= 2);

    // Clear emergency seepage so tick 2 resolves cleanly without immediately re-escalating a second proposal
    world
        .economy
        .set_settlement_shaft_water_level(SettlementId(0), 0.10);

    // Choose option 0 (relief delta + priority boost)
    world.enqueue_command(
        Tick(2),
        SpinePlayerCommand::ResolveArbitrationProposal {
            proposal_id: prop_id,
            chosen_option_index: 0,
        },
    );
    world.step_one_tick(&reg);

    assert!(world.command.open_proposals().is_empty());
    let grievance_after = world
        .pop
        .settlement_welfare(SettlementId(0))
        .expect("welfare")
        .grievance_01;
    assert!(
        grievance_after < grievance_before,
        "resolving with welfare-relief option must lower settlement grievance: before={grievance_before}, after={grievance_after}"
    );
}

// @req ENG-CMD-005
#[test]
fn personal_attention_zoom_boosts_focused_mandate_at_cost_of_remote_blind_spot() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    let grain = reg.commodity_id_by_key("raw_grain").unwrap();
    let cell_a = world.economy.settlements()[0].cell_id;
    let cell_b = (0..world.planet.cell_count())
        .map(CellId)
        .find(|&c| !world.planet.is_ocean_cell(c) && c != cell_a)
        .expect("cell B");
    let sid_b = world.economy.establish_settlement(
        "eastern_pit".to_owned(),
        "Eastern Pit".to_owned(),
        cell_b,
        ActorId(1),
        &[(grain, 150.0, 0.95, 0.96)],
        &[],
    );

    let mut chars = world.pop.all_characters().to_vec();
    if chars.len() < 2 {
        world.pop.seed_settlement_cohorts(
            sid_b,
            &[(VocationId(0), 20, 0)],
            &reg,
            "pit_crew",
            0.55,
        );
        let coh_id = world.pop.all_cohorts().last().unwrap().id;
        let _ = world.pop.promote_from_cohort(
            coh_id,
            "east_foreman".to_owned(),
            "Foreman Kael".to_owned(),
            "Pit Overseer".to_owned(),
            17,
        );
        chars = world.pop.all_characters().to_vec();
    }
    let char_a = chars[0].id;
    let char_b = chars[1].id;
    let mut settlement_cells = BTreeMap::new();
    settlement_cells.insert(SettlementId(0), cell_a);
    settlement_cells.insert(sid_b, cell_b);

    world.command.apply_player_command(
        Tick(0),
        SpinePlayerCommand::IssueMandate {
            assignee: char_a,
            objective_kind: "stabilize_food".to_owned(),
            target_cell: cell_a,
            priority_weight: 5,
            authority_tier: "priorities".to_owned(),
        },
        &chars,
        &settlement_cells,
    );
    world.command.apply_player_command(
        Tick(0),
        SpinePlayerCommand::IssueMandate {
            assignee: char_b,
            objective_kind: "repair_mine".to_owned(),
            target_cell: cell_b,
            priority_weight: 5,
            authority_tier: "priorities".to_owned(),
        },
        &chars,
        &settlement_cells,
    );

    let m_a = world.command.all_mandates()[0].id;
    let m_b = world.command.all_mandates()[1].id;
    world.command.mark_order_delivered(m_a, Tick(1));
    world.command.mark_order_delivered(m_b, Tick(1));
    world.command.recompute_attention_and_efficiencies(&chars);
    let baseline_a = world.command.all_mandates()[0].realized_efficiency;
    let baseline_b = world.command.all_mandates()[1].realized_efficiency;

    // Focus 75% personal sovereign attention directly onto mandate A
    world.command.apply_player_command(
        Tick(2),
        SpinePlayerCommand::SetPersonalAttentionZoom {
            target_mandate_id: Some(m_a),
            share_01: 0.75,
        },
        &chars,
        &settlement_cells,
    );
    let zoomed_a = world.command.all_mandates()[0].realized_efficiency;
    let unzoomed_b = world.command.all_mandates()[1].realized_efficiency;

    assert!(
        zoomed_a > baseline_a,
        "focused mandate should rise with attention zoom: base={baseline_a}, zoomed={zoomed_a}"
    );
    assert!(
        unzoomed_b < baseline_b,
        "unfocused mandates must suffer blind-spot efficiency loss: base={baseline_b}, unzoomed={unzoomed_b}"
    );
    assert!(
        world.command.settlement_oversight_multiplier(sid_b) < 1.0,
        "unfocused settlement must incur oversight penalty"
    );
}
