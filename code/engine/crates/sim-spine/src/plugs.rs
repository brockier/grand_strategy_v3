//! Pre-wired M5-M7 extension plug port traits and read-only inspection views.
//! In Milestones M0-M4, scenarios inject zero-cost neutral default implementations.
//! In Milestones M5-M7, concrete isolated plug crates implement these traits cleanly.

use crate::ids::{
    ActorId, CellId, CharacterId, CommodityId, EdgeId, FormationId, ProcessId, SettlementId,
    TechComponentId, TheaterId, TheoryDomainId, Tick,
};

/// Institutional category of a non-state economic actor (`ENG-ECO-005.1`).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum NonStateActorKind {
    MerchantHouse = 0,
    CraftGuild = 1,
    ClanEstate = 2,
    CharteredCompany = 3,
    ReligiousFoundation = 4,
    IndependentSmallholders = 5,
}

impl NonStateActorKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MerchantHouse => "MerchantHouse",
            Self::CraftGuild => "CraftGuild",
            Self::ClanEstate => "ClanEstate",
            Self::CharteredCompany => "CharteredCompany",
            Self::ReligiousFoundation => "ReligiousFoundation",
            Self::IndependentSmallholders => "IndependentSmallholders",
        }
    }
}

/// Public snapshot of an autonomous private or civic economic actor (`ENG-ECO-005`).
#[derive(Clone, Debug, PartialEq)]
pub struct NonStateActorView {
    pub actor_id: ActorId,
    pub name: String,
    pub kind: NonStateActorKind,
    pub home_settlement: SettlementId,
    pub working_capital_coins: f64,
    pub held_commodity: Option<CommodityId>,
    pub held_mass_kg: f64,
    pub hoarded_mass_kg: f64,
    pub private_carriers_owned: u32,
    pub chartered_to_state_carriers: u32,
    pub compliance_01: f32,
}

/// Settlement price discovery and black-market / rationing state (`ENG-ECO-005`, `ENG-ECO-006`).
#[derive(Clone, Debug, PartialEq)]
pub struct InformalMarketView {
    pub settlement: SettlementId,
    pub commodity: CommodityId,
    pub clearing_price_index: f32,
    pub black_market_premium_01: f32,
    pub black_market_leakage_kg: f64,
    pub hoarded_reserve_kg: f64,
}

/// Hourly edge traffic vs capacity congestion telemetry (`ENG-LOG-005`).
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeCongestionView {
    pub edge_id: EdgeId,
    pub hourly_flow_tonnes: f32,
    pub capacity_tonnes: f32,
    pub congestion_multiplier: f32,
}

/// Private inter-settlement arbitrage freight dispatch requested by a non-state actor.
#[derive(Clone, Debug, PartialEq)]
pub struct ArbitrageOrderRequest {
    pub origin_settlement: SettlementId,
    pub destination_settlement: SettlementId,
    pub commodity: CommodityId,
    pub mass_kg: f64,
    pub owner: ActorId,
    pub expected_margin_index: f32,
}

/// Active governance model, institutional adaptation, and fiscal law summary (`ENG-CMD-006`, `ENG-ECO-007`).
#[derive(Clone, Debug, PartialEq)]
pub struct GovernanceStateView {
    pub active_model_key: String,
    pub active_model_name: String,
    pub previous_model_key: Option<String>,
    pub adaptation_progress_01: f32,
    pub active_policy_key: String,
    pub active_policy_name: String,
    pub tax_or_tribute_rate_01: f32,
    pub patent_duration_years: u32,
    pub skimming_leakage_fraction: f32,
    pub effective_attention_capacity: f32,
    pub clerical_overhead_per_mandate: f32,
    pub hierarchy_hop_latency_hours: u32,
    pub chartered_carriers_total: u32,
    pub bureaucratic_capture_events: u64,
}

/// Physical medium preserving empirical or theoretical knowledge (`ENG-RES-004`).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum KnowledgeCarrierKind {
    PractitionerCohort = 0,
    MasterIndividual = 1,
    PhysicalTreatise = 2,
    WorkingPrototypeArtifact = 3,
}

impl KnowledgeCarrierKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PractitionerCohort => "PractitionerCohort",
            Self::MasterIndividual => "MasterIndividual",
            Self::PhysicalTreatise => "PhysicalTreatise",
            Self::WorkingPrototypeArtifact => "WorkingPrototypeArtifact",
        }
    }
}

/// Single localized knowledge carrier artifact or living practitioner (`ENG-RES-004`, `ENG-RES-005`).
#[derive(Clone, Debug, PartialEq)]
pub struct KnowledgeCarrierRecordView {
    pub settlement: SettlementId,
    pub component: TechComponentId,
    pub component_key: String,
    pub carrier_kind: KnowledgeCarrierKind,
    pub retention_strength_01: f32,
    pub holder_label: String,
}

/// Settlement-level emergence, patent, Know-How, and Know-Why summary (`ENG-RES-001`..`005`).
#[derive(Clone, Debug, PartialEq)]
pub struct SettlementKnowledgeView {
    pub settlement: SettlementId,
    pub innovation_mode_key: String,
    pub bottleneck_pressure_01: f32,
    pub leisure_surplus_01: f32,
    pub know_how_levels: Vec<(TechComponentId, String, f32)>,
    pub know_why_levels: Vec<(TheoryDomainId, String, f32)>,
    pub active_patent_holder: Option<ActorId>,
    pub patent_expiry_tick: Option<Tick>,
    pub regression_losses: u32,
}

/// High-level theater command directive (Churchill delegated posture) (`ENG-WAR-002`).
#[derive(Clone, Debug, PartialEq)]
pub struct TheaterDirectiveView {
    pub theater_id: TheaterId,
    pub commander: CharacterId,
    pub objective_cell: CellId,
    pub supply_priority_01: f32,
    pub issued_tick: Tick,
    pub autonomous_withdrawals: u32,
}

/// Active physical military formation deployed in continuous spherical space (`ENG-WAR-001`..`004`).
#[derive(Clone, Debug, PartialEq)]
pub struct MilitaryFormationView {
    pub formation_id: FormationId,
    pub label: String,
    pub template_key: String,
    pub theater_id: TheaterId,
    pub commander: CharacterId,
    pub home_settlement: SettlementId,
    pub current_cell: CellId,
    pub current_unit_xyz: [f32; 3],
    pub target_cell: CellId,
    pub target_unit_xyz: [f32; 3],
    pub personnel_count: u32,
    pub equipment_mass_kg: f64,
    pub munitions_days: f32,
    pub rations_days: f32,
    pub cohesion_01: f32,
    pub doctrine_key: String,
    pub no_retreat_order: bool,
    pub encircled: bool,
    pub insubordinate_withdrawal: bool,
    pub stale_order_ticks: u64,
}

/// Mortal character lifecycle state tracked across decades (`ENG-POP-006`).
#[derive(Clone, Debug, PartialEq)]
pub struct SuccessionCharacterView {
    pub character_id: CharacterId,
    pub name: String,
    pub role_title: String,
    pub age_years: f32,
    pub health_01: f32,
    pub is_alive: bool,
    pub is_ruler: bool,
    pub settlement: SettlementId,
}

/// Active or completed succession transition briefing bound strictly to physical HQ archives (`ENG-CMD-007`).
#[derive(Clone, Debug, PartialEq)]
pub struct SuccessionTransitionView {
    pub rule_key: String,
    pub rule_display_name: String,
    pub deceased_ruler_name: String,
    pub successor_name: String,
    pub transition_tick: Tick,
    pub disruption_until_tick: Tick,
    pub inherited_archive_entries: usize,
    pub lost_unrecorded_rapport_count: usize,
    pub awaiting_player_acknowledgement: bool,
}

/// M5 plug hook: autonomous non-state actors, price discovery, hoarding, and edge congestion.
pub trait NonStateEconomyPlugPort: Send + Sync {
    fn edge_congestion_multiplier(&self, _edge: EdgeId) -> f32 {
        1.0
    }

    fn step_private_actors(&mut self, _tick: Tick) {}

    fn non_state_actors(&self) -> &[NonStateActorView] {
        &[]
    }

    fn informal_markets(&self) -> &[InformalMarketView] {
        &[]
    }

    fn congested_edges(&self) -> &[EdgeCongestionView] {
        &[]
    }

    fn drain_arbitrage_requests(&mut self) -> Vec<ArbitrageOrderRequest> {
        Vec::new()
    }

    fn total_chartered_carriers(&self) -> u32 {
        0
    }
}

/// M5 plug hook: governance model switching, bureaucratic capture, and institutional transition friction.
pub trait GovernanceFrictionPlugPort: Send + Sync {
    fn oversight_efficiency_multiplier(
        &self,
        _settlement: SettlementId,
        _overseer: CharacterId,
    ) -> f32 {
        1.0
    }

    fn governance_state(&self) -> Option<GovernanceStateView> {
        None
    }
}

/// M6 plug hook: empirical Know-How, theoretical Know-Why, innovation modes, and fragile knowledge carriers.
pub trait KnowledgeModifierPlugPort: Send + Sync {
    fn process_prerequisites_met(&self, _settlement: SettlementId, _process: ProcessId) -> bool {
        true
    }

    fn settlement_knowledge_views(&self) -> &[SettlementKnowledgeView] {
        &[]
    }

    fn knowledge_carriers(&self) -> &[KnowledgeCarrierRecordView] {
        &[]
    }

    fn actor_patent_permitted(
        &self,
        _settlement: SettlementId,
        _component: TechComponentId,
        _actor: ActorId,
    ) -> bool {
        true
    }
}

/// M7 plug hook: high-level operational warfare without tactical unit micro.
pub trait OperationalCombatPlugPort: Send + Sync {
    fn step_hourly_combat(&mut self, _tick: Tick) {}

    fn theaters(&self) -> &[TheaterDirectiveView] {
        &[]
    }

    fn formations(&self) -> &[MilitaryFormationView] {
        &[]
    }
}

/// M7 plug hook: mortal ruler aging/demise and archive-only succession continuity.
pub trait SuccessionPlugPort: Send + Sync {
    fn check_succession_transition(&mut self, _tick: Tick, _hq_cell: CellId) -> bool {
        false
    }

    fn roster_characters(&self) -> &[SuccessionCharacterView] {
        &[]
    }

    fn pending_transition(&self) -> Option<&SuccessionTransitionView> {
        None
    }

    fn effective_command_multiplier(&self, _tick: Tick) -> f32 {
        1.0
    }
}

/// Neutral M0-M4 no-op non-state economy cartridge.
#[derive(Clone, Copy, Debug, Default)]
pub struct NeutralNonStateEconomyPlug;
impl NonStateEconomyPlugPort for NeutralNonStateEconomyPlug {}

/// Neutral M0-M4 no-op governance cartridge.
#[derive(Clone, Copy, Debug, Default)]
pub struct NeutralGovernanceFrictionPlug;
impl GovernanceFrictionPlugPort for NeutralGovernanceFrictionPlug {}

/// Neutral M0-M4 no-op knowledge cartridge.
#[derive(Clone, Copy, Debug, Default)]
pub struct NeutralKnowledgeModifierPlug;
impl KnowledgeModifierPlugPort for NeutralKnowledgeModifierPlug {}

/// Neutral M0-M4 no-op operational warfare cartridge.
#[derive(Clone, Copy, Debug, Default)]
pub struct NeutralOperationalCombatPlug;
impl OperationalCombatPlugPort for NeutralOperationalCombatPlug {}

/// Neutral M0-M4 no-op succession cartridge.
#[derive(Clone, Copy, Debug, Default)]
pub struct NeutralSuccessionPlug;
impl SuccessionPlugPort for NeutralSuccessionPlug {}
