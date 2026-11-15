#![deny(clippy::disallowed_types)]

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use player_view::ValidatedPlayerCommand;
use sim_data::load_mod_bundle;
use sim_plug_research::KnowledgeEvolutionPlugin;
use sim_scenario::{resolve_scenario_pack_stack, ScenarioWorld};

const VALID_MODES: [&str; 4] = [
    "guild_secrecy",
    "state_arsenal",
    "open_scholarly_patronage",
    "private_patent_exclusivity",
];
use sim_spine::{
    ActorId, KnowledgeCarrierKind, KnowledgeModifierPlugPort, ModRegistryPort, ProcessId,
    SettlementId, SpinePlayerCommand, Tick,
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

// @req ENG-RES-001.1
// @req ENG-RES-001.2
#[test]
fn zero_science_point_pool_and_rejection_of_direct_tech_purchase_commands() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    let snap = world.compile_diegetic_snapshot(&reg);

    // Snapshot exposes solely settlement empirical Know-How, scholarly Know-Why, and physical carriers.
    assert!(!snap.knowledge.settlements.is_empty());
    assert_eq!(snap.knowledge.available_modes.len(), VALID_MODES.len());
    for mode in VALID_MODES {
        assert!(snap.knowledge.available_modes.iter().any(|m| m == mode));
    }

    // Direct tech-tree purchase commands must be rejected cleanly at IPC boundary and plugin layer.
    let ipc_err = ValidatedPlayerCommand::BuyOrSelectTechTarget {
        tech_key: "coke_blast_hearth".to_owned(),
    }
    .to_spine_command()
    .expect_err("direct technology target purchase must fail validation");
    assert!(
        ipc_err.starts_with("ERR_BOUGHT_TECH_FORBIDDEN:"),
        "unexpected error prefix: {ipc_err}"
    );

    let plug_err = world
        .research
        .reject_bought_tech_request()
        .expect_err("research plugin must refuse bought tech queries");
    assert!(plug_err.contains("ERR_BOUGHT_TECH_FORBIDDEN"));
}

// @req ENG-RES-002.1
// @req ENG-RES-002.2
#[test]
fn bottleneck_workplace_know_how_emergence_and_cross_settlement_pollination() {
    let reg = load_unexplained_bundle();
    let sid0 = SettlementId(0);
    let sid1 = SettlementId(1);
    let comp_id = reg
        .tech_components()
        .first()
        .expect("tech components loaded")
        .id;

    let mut plug_idle = KnowledgeEvolutionPlugin::from_registry(&reg, &[sid0, sid1]);
    let mut plug_busy = KnowledgeEvolutionPlugin::from_registry(&reg, &[sid0, sid1]);
    let base_kh = plug_busy.settlement_know_how(sid0, comp_id);

    for t in 1..=80u64 {
        plug_idle.step_settlement_knowledge(Tick(t), sid0, 0.0, 0.85, 0.0);
        plug_busy.step_settlement_knowledge(Tick(t), sid0, 60.0, 0.85, 0.0);
    }

    let idle_kh = plug_idle.settlement_know_how(sid0, comp_id);
    let busy_kh = plug_busy.settlement_know_how(sid0, comp_id);
    assert!(
        (idle_kh - base_kh).abs() < 1e-5,
        "idle workshops with zero active production hours must not gain empirical Know-How"
    );
    assert!(
        busy_kh > base_kh + 0.10 && busy_kh >= 0.45,
        "pressured active workshops must raise Know-How and unlock physical prototypes: busy={busy_kh}, base={base_kh}"
    );
    assert!(
        plug_busy
            .knowledge_carriers()
            .iter()
            .any(|c| c.settlement == sid0 && c.carrier_kind == KnowledgeCarrierKind::WorkingPrototypeArtifact),
        "crossing prototype threshold must record physical WorkingPrototypeArtifact carriers"
    );

    // Diffuse proven physical artifact/master across settlements (`ENG-RES-002.2`).
    let dst_before = plug_busy.settlement_know_how(sid1, comp_id);
    plug_busy.diffuse_knowledge_carrier(sid0, sid1, 0.80);
    let dst_after = plug_busy.settlement_know_how(sid1, comp_id);
    assert!(
        dst_after > dst_before,
        "physical carrier migration must cross-pollinate Know-How to target settlement"
    );
}

// @req ENG-RES-003.1
// @req ENG-RES-003.2
#[test]
fn surplus_leisure_unlocks_know_why_synthesis_which_accelerates_and_lifts_know_how() {
    let reg = load_unexplained_bundle();
    let sid = SettlementId(0);

    let mut no_leisure = KnowledgeEvolutionPlugin::from_registry(&reg, &[sid]);
    let mut rich_leisure = KnowledgeEvolutionPlugin::from_registry(&reg, &[sid]);

    for t in 1..=260u64 {
        no_leisure.step_settlement_knowledge(Tick(t), sid, 70.0, 0.90, 0.05);
        rich_leisure.step_settlement_knowledge(Tick(t), sid, 70.0, 0.90, 0.72);
    }

    let view_no = no_leisure
        .settlement_knowledge_views()
        .iter()
        .find(|s| s.settlement == sid)
        .expect("settlement view");
    let view_rich = rich_leisure
        .settlement_knowledge_views()
        .iter()
        .find(|s| s.settlement == sid)
        .expect("settlement view");

    let sum_kw = |vals: &[(sim_spine::TechComponentId, String, f32)]| -> f32 {
        vals.iter().map(|(_, _, v)| *v).sum()
    };
    let sum_why = |vals: &[(sim_spine::TheoryDomainId, String, f32)]| -> f32 {
        vals.iter().map(|(_, _, v)| *v).sum()
    };

    let low_why = sum_why(&view_no.know_why_levels);
    let high_why = sum_why(&view_rich.know_why_levels);
    assert!(
        high_why > low_why + 0.25,
        "genuine specialist leisure surplus must grow scholarly Know-Why: rich={high_why}, cramped={low_why}"
    );
    assert!(
        rich_leisure
            .knowledge_carriers()
            .iter()
            .any(|c| c.carrier_kind == KnowledgeCarrierKind::PhysicalTreatise),
        "scholarly Know-Why synthesis must author PhysicalTreatise carriers"
    );

    // Know-Why lifts directed engineering Know-How ceiling past the purely empirical 0.72 cap.
    let low_kh_sum = sum_kw(&view_no.know_how_levels);
    let high_kh_sum = sum_kw(&view_rich.know_how_levels);
    let max_cramped = view_no
        .know_how_levels
        .iter()
        .map(|(_, _, v)| *v)
        .fold(0.0_f32, f32::max);
    let max_scholarly = view_rich
        .know_how_levels
        .iter()
        .map(|(_, _, v)| *v)
        .fold(0.0_f32, f32::max);
    assert!(
        max_cramped <= 0.725 && max_scholarly > 0.74 && high_kh_sum > low_kh_sum,
        "Know-Why must raise Know-How ceiling beyond 0.72 empirical asymptote and accelerate output"
    );
}

// @req ENG-RES-004.1
// @req ENG-RES-004.2
#[test]
fn four_innovation_modes_gate_capabilities_and_enforce_patent_expiry() {
    let reg = load_unexplained_bundle();
    let mut world = ScenarioWorld::from_registry(&reg, 42);
    let sid0 = SettlementId(0);
    let sid1 = SettlementId(1);

    // Switch through all 4 valid institutional modes via player commands.
    for mode in VALID_MODES {
        world.apply_player_command_now(
            &reg,
            SpinePlayerCommand::SetInnovationMode {
                settlement: sid0,
                mode_key: (*mode).to_owned(),
            },
        );
        let view = world
            .research
            .settlement_knowledge_views()
            .iter()
            .find(|s| s.settlement == sid0)
            .expect("settlement knowledge view");
        assert_eq!(view.innovation_mode_key, *mode);
    }

    // Compare diffusion barriers: guild_secrecy vs open_scholarly_patronage.
    let comp_id = reg.tech_components().first().expect("component").id;
    let mut plug_guild = KnowledgeEvolutionPlugin::from_registry(&reg, &[sid0, sid1]);
    let mut plug_open = KnowledgeEvolutionPlugin::from_registry(&reg, &[sid0, sid1]);
    plug_guild
        .set_innovation_mode(sid0, "guild_secrecy", Tick(1), None)
        .expect("guild_secrecy valid");
    plug_open
        .set_innovation_mode(sid0, "open_scholarly_patronage", Tick(1), None)
        .expect("open_scholarly_patronage valid");

    for t in 1..=120u64 {
        plug_guild.step_settlement_knowledge(Tick(t), sid0, 65.0, 0.85, 0.55);
        plug_open.step_settlement_knowledge(Tick(t), sid0, 65.0, 0.85, 0.55);
    }
    let dst_guild_before = plug_guild.settlement_know_how(sid1, comp_id);
    let dst_open_before = plug_open.settlement_know_how(sid1, comp_id);
    plug_guild.diffuse_knowledge_carrier(sid0, sid1, 0.85);
    plug_open.diffuse_knowledge_carrier(sid0, sid1, 0.85);
    let guild_gain = plug_guild.settlement_know_how(sid1, comp_id) - dst_guild_before;
    let open_gain = plug_open.settlement_know_how(sid1, comp_id) - dst_open_before;
    assert!(
        open_gain > guild_gain,
        "open scholarly patronage must diffuse faster than guild secrecy: open={open_gain}, guild={guild_gain}"
    );

    // Capability gate check on gated heavy processes.
    let gated_pid = (0..reg.process_count())
        .map(|i| ProcessId(i as u16))
        .find(|&p| world.research.process_gate(p).is_some())
        .expect("at least one advanced process must be capability-gated");
    let (req_comp, req_lvl) = world.research.process_gate(gated_pid).expect("gate");
    assert!(req_lvl > 0.0);
    let holder = ActorId(100);
    let rival = ActorId(202);

    // Private patent exclusivity blocks non-holder until expiry tick, then opens to rivals.
    let mut plug_pat = KnowledgeEvolutionPlugin::from_registry(&reg, &[sid0]);
    plug_pat
        .set_innovation_mode(sid0, "private_patent_exclusivity", Tick(10), Some(holder))
        .expect("private_patent_exclusivity valid");
    let pat_view = plug_pat
        .settlement_knowledge_views()
        .iter()
        .find(|s| s.settlement == sid0)
        .expect("patented settlement");
    let exp_tick = pat_view
        .patent_expiry_tick
        .expect("patent expiry tick recorded");
    assert_eq!(pat_view.active_patent_holder, Some(holder));
    assert!(plug_pat.actor_patent_permitted(sid0, req_comp, holder));
    assert!(!plug_pat.actor_patent_permitted(sid0, req_comp, rival));

    // Stepping up to expiry opens use to rival actors.
    plug_pat.step_settlement_knowledge(exp_tick, sid0, 10.0, 0.2, 0.1);
    assert!(plug_pat.actor_patent_permitted(sid0, req_comp, rival));
}

// @req ENG-RES-005.1
// @req ENG-RES-005.2
#[test]
fn four_carrier_accounting_and_genuine_regression_on_total_carrier_loss() {
    let reg = load_unexplained_bundle();
    let sid = SettlementId(0);
    let mut plug = KnowledgeEvolutionPlugin::from_registry(&reg, &[sid, SettlementId(1)]);

    for t in 1..=160u64 {
        plug.step_settlement_knowledge(Tick(t), sid, 75.0, 0.90, 0.70);
    }
    plug.diffuse_knowledge_carrier(sid, SettlementId(1), 0.75);
    plug.diffuse_knowledge_carrier(SettlementId(1), sid, 0.75);

    let kinds: BTreeSet<u8> = plug
        .knowledge_carriers()
        .iter()
        .map(|c| c.carrier_kind as u8)
        .collect();
    assert!(
        kinds.contains(&(KnowledgeCarrierKind::PractitionerCohort as u8))
            && kinds.contains(&(KnowledgeCarrierKind::MasterIndividual as u8))
            && kinds.contains(&(KnowledgeCarrierKind::PhysicalTreatise as u8))
            && kinds.contains(&(KnowledgeCarrierKind::WorkingPrototypeArtifact as u8)),
        "all four physical knowledge carrier classes must be represented: observed tags={kinds:?}"
    );

    // Partial disaster: lose living practitioners, retain treatises/prototypes -> partial survival (`> 0.12`).
    let mut partial = plug.clone();
    partial.apply_carrier_loss(sid, true, false);
    let surviving_kh = partial
        .settlement_knowledge_views()
        .iter()
        .find(|s| s.settlement == sid)
        .expect("sid")
        .know_how_levels
        .iter()
        .map(|(_, _, v)| *v)
        .fold(0.0_f32, f32::max);
    assert!(
        surviving_kh > 0.12,
        "physical treatises/artifacts preserve base recoverable knowledge after losing masters: surviving_kh={surviving_kh}"
    );

    // Total disaster: lose BOTH living practitioners AND treatises/prototypes -> genuine regression!
    let gated_pid = (0..reg.process_count())
        .map(|i| ProcessId(i as u16))
        .find(|&p| plug.process_gate(p).is_some() && plug.process_prerequisites_met(sid, p))
        .expect("previously unlocked gated process exists");

    plug.apply_carrier_loss(sid, true, true);
    let view_collapsed = plug
        .settlement_knowledge_views()
        .iter()
        .find(|s| s.settlement == sid)
        .expect("collapsed settlement");
    assert!(view_collapsed.regression_losses >= 1);
    for (_, _, lvl) in &view_collapsed.know_how_levels {
        assert!(
            *lvl <= 0.125,
            "all Know-How levels must collapse upon total physical carrier extinction: lvl={lvl}"
        );
    }
    assert!(
        !plug.process_prerequisites_met(sid, gated_pid),
        "capability gate must regress to locked when all physical carriers are lost"
    );
}
