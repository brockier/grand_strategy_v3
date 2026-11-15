//! Narrow read-only views, port traits, and command queues connecting cartridges cleanly.

use crate::ids::{
    ActorId, CarrierClassId, CellId, CharacterId, CohortId, CommodityId, ConvoyId, DispatchId,
    EdgeId, EventClassId, InfraClassId, MandateId, ProceduralMaterialId, ProcessId, SettlementId,
    StockpileId, Tick, VocationId,
};

/// Ordered execution phases for each discrete hourly simulation tick.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum TickPhase {
    IngestPlayerCommands = 1,
    HourlyWeatherAndHydrology = 2,
    HourlyCombatStep = 3,
    HourlyCourierAndSignalStep = 4,
    DailyBucketLogisticsAndMarkets = 5,
    DailyBucketExtractionProductionConstruction = 6,
    WeeklyMonthlyBucketDemographicsMandatesResearch = 7,
    CompileDiegeticPlayerViewAndNotifications = 8,
}

impl TickPhase {
    /// Returns all eight phases in deterministic sequential barrier order.
    #[must_use]
    pub const fn ordered() -> [Self; 8] {
        [
            Self::IngestPlayerCommands,
            Self::HourlyWeatherAndHydrology,
            Self::HourlyCombatStep,
            Self::HourlyCourierAndSignalStep,
            Self::DailyBucketLogisticsAndMarkets,
            Self::DailyBucketExtractionProductionConstruction,
            Self::WeeklyMonthlyBucketDemographicsMandatesResearch,
            Self::CompileDiegeticPlayerViewAndNotifications,
        ]
    }

    /// Returns the zero-based phase index (`0..8`).
    #[must_use]
    pub const fn index(self) -> usize {
        (self as usize) - 1
    }

    /// Returns a stable machine-readable subsystem key for profiling telemetry.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::IngestPlayerCommands => "ingest_player_commands",
            Self::HourlyWeatherAndHydrology => "weather",
            Self::HourlyCombatStep => "combat",
            Self::HourlyCourierAndSignalStep => "transport",
            Self::DailyBucketLogisticsAndMarkets => "logistics_and_markets",
            Self::DailyBucketExtractionProductionConstruction => "production",
            Self::WeeklyMonthlyBucketDemographicsMandatesResearch => "population",
            Self::CompileDiegeticPlayerViewAndNotifications => "player_view",
        }
    }
}

/// Physical bounds controlling procedural generation of planetary ores, fluxes, fuels, crops, fauna, and pathogens.
#[derive(Clone, Debug, PartialEq)]
pub struct NaturalBoundsView {
    pub ore_count: u16,
    pub flux_count: u16,
    pub fuel_count: u16,
    pub crop_count: u16,
    pub fauna_count: u16,
    pub pathogen_count: u16,
    pub ore_melting_k_range: [f32; 2],
    pub ore_purity_range: [f32; 2],
    pub fuel_enthalpy_mj_kg_range: [f32; 2],
    pub crop_opt_temp_c_range: [f32; 2],
}

/// Read-only planetary generation parameter view loaded from `sim-data`.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanetGenConfigView {
    pub id: String,
    pub goldberg_subdivision: u8,
    pub tectonic_plate_count: u16,
    pub land_fraction_target: f32,
    pub axial_tilt_deg: f32,
    pub solar_constant_w_m2: f32,
    pub rotation_period_hours: f32,
    pub mean_radius_km: f32,
    pub base_sea_level_m: f32,
    pub max_orogenic_uplift_m: f32,
    pub planetary_density_kg_m3: f32,
    pub natural_bounds: NaturalBoundsView,
}

/// Read-only view of an interned external commodity definition.
#[derive(Clone, Debug, PartialEq)]
pub struct CommodityDefView {
    pub id: CommodityId,
    pub key: String,
    pub display_name: String,
    pub category: String,
    pub mass_per_unit_kg: f64,
    pub volume_per_unit_m3: f64,
    pub spoilage_rate_per_hour: f64,
    pub is_tool: bool,
    pub tool_wear_per_hour: f64,
}

/// Commodity quantity binding inside a production or extraction recipe.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessQuantityView {
    pub commodity: CommodityId,
    pub amount_kg: f64,
}

/// Read-only view of an interned external production/extraction process definition.
#[derive(Clone, Debug, PartialEq)]
pub struct ProcessDefView {
    pub id: ProcessId,
    pub key: String,
    pub display_name: String,
    pub required_vocation: VocationId,
    pub cycle_hours: u32,
    pub workforce_hours_required: f64,
    pub tool_commodity: Option<CommodityId>,
    pub weather_halt_mud_threshold: f32,
    pub inputs: Vec<ProcessQuantityView>,
    pub outputs: Vec<ProcessQuantityView>,
}

/// Read-only view of an interned carrier class definition.
#[derive(Clone, Debug, PartialEq)]
pub struct CarrierDefView {
    pub id: CarrierClassId,
    pub key: String,
    pub display_name: String,
    pub max_payload_kg: f64,
    pub max_payload_m3: f64,
    pub base_speed_km_hr: f32,
    pub max_grade_rad: f32,
    pub mud_penalty_factor: f32,
    pub crew_required: u32,
    pub fodder_or_fuel_commodity: CommodityId,
    pub fuel_burn_kg_per_hour: f64,
}

/// Read-only view of an interned infrastructure class definition.
#[derive(Clone, Debug, PartialEq)]
pub struct InfraDefView {
    pub id: InfraClassId,
    pub key: String,
    pub display_name: String,
    pub throughput_tonnes_per_hour: f32,
    pub speed_multiplier: f32,
    pub mud_shield_factor: f32,
}

/// Read-only view of an interned occupational vocation definition.
#[derive(Clone, Debug, PartialEq)]
pub struct VocationDefView {
    pub id: VocationId,
    pub key: String,
    pub display_name: String,
    pub base_skill_gain_per_day: f32,
    pub skill_decay_per_day: f32,
}

/// Read-only scenario calendar configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct CalendarConfigView {
    pub hours_per_day: u32,
    pub days_per_year: u32,
    pub year_prefix: String,
    pub starting_year: i32,
    pub month_names: Vec<String>,
    pub month_lengths: Vec<u32>,
}

/// Starting settlement seed extracted strictly from external scenario files.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioSettlementSeed {
    pub key: String,
    pub display_name: String,
    pub cell_index: u32,
    pub initial_population: u32,
    pub stockpiles: Vec<(CommodityId, f64, f32)>,
    pub cohorts: Vec<(VocationId, u32)>,
    pub initial_facilities: Vec<(ProcessId, u32)>,
}

/// Starting named individual seed extracted strictly from external scenario files.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioCharacterSeed {
    pub key: String,
    pub display_name: String,
    pub role_title: String,
    pub home_settlement_key: String,
    pub competence: f32,
    pub precision_01: f32,
    pub honesty_vs_sycophancy: f32,
    pub caution_vs_alarmism: f32,
}

/// Scenario launch descriptor passed from `sim-data` through `sim-spine` into `sim-core`.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioInitView {
    pub scenario_key: String,
    pub display_name: String,
    pub hq_settlement_key: String,
    pub hq_sight_radius_cells: u8,
    pub unexplored_surrounding_cells: bool,
    pub goldberg_subdivision: u8,
    pub settlements: Vec<ScenarioSettlementSeed>,
    pub characters: Vec<ScenarioCharacterSeed>,
}

/// Read-only registry port populated by `sim-data` and consumed by engine cartridges.
pub trait ModRegistryPort: Send + Sync {
    fn commodity_def(&self, id: CommodityId) -> Option<&CommodityDefView>;
    fn commodity_count(&self) -> usize;
    fn commodity_id_by_key(&self, key: &str) -> Option<CommodityId>;

    fn process_def(&self, id: ProcessId) -> Option<&ProcessDefView>;
    fn process_count(&self) -> usize;

    fn carrier_def(&self, id: CarrierClassId) -> Option<&CarrierDefView>;
    fn infra_def(&self, id: InfraClassId) -> Option<&InfraDefView>;
    fn vocation_def(&self, id: VocationId) -> Option<&VocationDefView>;
    fn event_class_id(&self, key: &str) -> Option<EventClassId>;
    fn calendar_config(&self) -> &CalendarConfigView;
    fn planet_gen_config(&self) -> &PlanetGenConfigView;
    fn bundle_blake3_bytes(&self) -> [u8; 32];
    fn scenario_init(&self) -> &ScenarioInitView;
}

/// Surface rock geological strata classification visible cleanly across public surface ports.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum RockStrataClass {
    OceanicBasalt = 0,
    IgneousOrogenic = 1,
    VolcanicArc = 2,
    MetamorphicShield = 3,
    SedimentaryBasin = 4,
    AlluvialPlain = 5,
}

impl RockStrataClass {
    /// Returns a human-readable strata identifier string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OceanicBasalt => "OceanicBasalt",
            Self::IgneousOrogenic => "IgneousOrogenic",
            Self::VolcanicArc => "VolcanicArc",
            Self::MetamorphicShield => "MetamorphicShield",
            Self::SedimentaryBasin => "SedimentaryBasin",
            Self::AlluvialPlain => "AlluvialPlain",
        }
    }
}

/// Geological cell surface summary without revealing private planet arrays.
#[derive(Clone, Debug, PartialEq)]
pub struct CellGeologyView {
    pub cell_id: CellId,
    pub elevation_m: f32,
    pub strata_label: String,
    pub biome_key: String,
    pub is_ocean: bool,
}

/// Read-only planet geometry and hydrology port.
pub trait PlanetGeometryPort: Send + Sync {
    fn cell_count(&self) -> u32;
    fn cell_neighbors(&self, cell: CellId) -> &[CellId];
    fn cell_edge(&self, from: CellId, to: CellId) -> Option<EdgeId>;
    fn cell_elevation_m(&self, cell: CellId) -> f32;
    fn cell_slope_rad(&self, edge: EdgeId) -> f32;
    fn cell_biome_and_strata(&self, cell: CellId) -> CellGeologyView;
    fn watershed_downstream(&self, cell: CellId) -> Option<CellId>;
    fn great_circle_km(&self, a: CellId, b: CellId) -> f32;
    fn cell_unit_xyz(&self, cell: CellId) -> [f32; 3];
    fn cell_lat_lon_rad(&self, cell: CellId) -> [f32; 2];
    fn cell_area_km2(&self, cell: CellId) -> f32;
    fn cell_drainage_area_km2(&self, cell: CellId) -> f32;
    fn cell_base_precip_mm_yr(&self, cell: CellId) -> f32;
    fn edge_length_km(&self, edge: EdgeId) -> f32;
    fn topo_sorted_land_cells(&self) -> &[CellId];
}

/// Procedural subsurface commodity family stored in sealed mineral deposits.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum ProceduralDepositCategory {
    MetallicOre = 0,
    SmeltingFlux = 1,
    SolidFuel = 2,
    LiquidFuel = 3,
}

/// Ground-truth hidden mineral/fuel deposit record sealed behind `SubsurfaceDepositTruthPort`.
#[derive(Clone, Debug, PartialEq)]
pub struct HiddenDepositView {
    pub cell: CellId,
    pub material_id: ProceduralMaterialId,
    pub category: ProceduralDepositCategory,
    pub total_mass_tonnes: f64,
    pub average_purity_01: f32,
    pub overburden_depth_m: f32,
    pub aquifer_flood_risk_01: f32,
}

/// Ground-truth subsurface mineral veins and fossil beds.
/// Readable ONLY by `sim-core` extraction and survey mechanics — never exposed to `player-view`.
pub trait SubsurfaceDepositTruthPort: Send + Sync {
    fn deposits_in_cell(&self, cell: CellId) -> &[HiddenDepositView];
    fn all_deposits(&self) -> &[HiddenDepositView];
}

/// Hourly physical surface state reported by `sim-weather`.
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceConditionSnapshot {
    pub precip_mm_hr: f32,
    pub mud_index_01: f32,
    pub flood_stage_01: f32,
    pub snow_depth_m: f32,
    pub ice_thickness_m: f32,
    pub temp_c: f32,
    pub wind_u_mps: f32,
    pub wind_v_mps: f32,
    pub visibility_01: f32,
}

impl Default for SurfaceConditionSnapshot {
    fn default() -> Self {
        Self {
            precip_mm_hr: 0.0,
            mud_index_01: 0.1,
            flood_stage_01: 0.0,
            snow_depth_m: 0.0,
            ice_thickness_m: 0.0,
            temp_c: 14.0,
            wind_u_mps: 2.0,
            wind_v_mps: 1.0,
            visibility_01: 1.0,
        }
    }
}

/// Read-only surface weather condition port.
pub trait SurfaceConditionsPort: Send + Sync {
    fn surface(&self, cell: CellId) -> SurfaceConditionSnapshot;
}

/// Atomic state mutation submitted to `PhysicalLedgerPort`.
#[derive(Clone, Debug, PartialEq)]
pub struct LedgerCommand {
    pub stockpile: StockpileId,
    pub commodity: CommodityId,
    pub owner: ActorId,
    pub delta_kg: f64,
}

/// Outcome returned from `PhysicalLedgerPort::apply_ledger_commands`.
#[derive(Clone, Debug, PartialEq)]
pub struct LedgerReceipt {
    pub applied: bool,
    pub resulting_mass_kg: f64,
}

/// Spatial settlement metadata visible across spine boundaries.
#[derive(Clone, Debug, PartialEq)]
pub struct SettlementSummaryView {
    pub id: SettlementId,
    pub key: String,
    pub display_name: String,
    pub cell_id: CellId,
    pub owner: ActorId,
}

/// Stockpile entry summary for reporting.
#[derive(Clone, Debug, PartialEq)]
pub struct StockpileEntryView {
    pub stockpile: StockpileId,
    pub settlement: SettlementId,
    pub commodity: CommodityId,
    pub owner: ActorId,
    pub mass_kg: f64,
    pub average_purity: f32,
}

/// Read-only and queued ledger access port.
pub trait PhysicalLedgerPort: Send + Sync {
    fn settlements(&self) -> &[SettlementSummaryView];
    fn hq_settlement(&self) -> Option<SettlementId>;
    fn hq_cell(&self) -> CellId;
    fn hq_sight_radius_cells(&self) -> u8;
    fn stockpile_mass_kg(&self, stock: StockpileId, commodity: CommodityId, owner: Option<ActorId>) -> f64;
    fn settlement_stockpiles(&self, settlement: SettlementId) -> &[StockpileId];
    fn settlement_inventory_entries(&self, settlement: SettlementId) -> Vec<StockpileEntryView>;
    fn apply_ledger_commands(&mut self, cmds: &[LedgerCommand]) -> Vec<LedgerReceipt>;
}

/// Transit state of a moving physical convoy or courier.
#[derive(Clone, Debug, PartialEq)]
pub struct ConvoyTransitView {
    pub convoy_id: ConvoyId,
    pub label: String,
    pub current_cell: CellId,
    pub destination_cell: CellId,
    pub planned_route: Vec<CellId>,
    pub departure_tick: Tick,
    pub expected_arrival_tick: Tick,
    pub is_hq_move: bool,
}

/// Request to spawn a physical cargo shipment.
#[derive(Clone, Debug, PartialEq)]
pub struct ShipmentRequest {
    pub origin_settlement: SettlementId,
    pub destination_settlement: SettlementId,
    pub carrier_class: CarrierClassId,
    pub owner: ActorId,
    pub commodity: CommodityId,
    pub mass_kg: f64,
}

/// Outgoing physical courier envelope submitted by settlement subordinates or HQ.
#[derive(Clone, Debug, PartialEq)]
pub struct CourierEnvelope {
    pub origin_settlement: SettlementId,
    pub origin_cell: CellId,
    pub sender: CharacterId,
    pub sent_tick: Tick,
    pub subject: String,
}

/// Delivered courier packet arriving at HQ cell during `TickPhase::HourlyCourierAndSignalStep`.
#[derive(Clone, Debug, PartialEq)]
pub struct ArrivedCourierEnvelope {
    pub dispatch_id: DispatchId,
    pub origin_settlement: SettlementId,
    pub sender: CharacterId,
    pub sent_tick: Tick,
    pub arrived_tick: Tick,
    pub subject: String,
}

/// Logistics port for convoys and physical courier dispatches.
pub trait LogisticsPort: Send + Sync {
    fn convoy_state(&self, convoy: ConvoyId) -> Option<ConvoyTransitView>;
    fn enqueue_shipment(&mut self, req: ShipmentRequest) -> ConvoyId;
    fn enqueue_courier(&mut self, env: CourierEnvelope) -> DispatchId;
    fn arrived_envelopes(&self) -> &[ArrivedCourierEnvelope];
}

/// Snapshot view of a demographic cohort.
#[derive(Clone, Debug, PartialEq)]
pub struct CohortSnapshotView {
    pub id: CohortId,
    pub settlement: SettlementId,
    pub vocation: VocationId,
    pub headcount: u32,
    pub mean_skill_01: f32,
}

/// Snapshot view of a named character.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterSnapshotView {
    pub id: CharacterId,
    pub key: String,
    pub display_name: String,
    pub role_title: String,
    pub home_settlement: SettlementId,
    pub competence: f32,
    pub precision_01: f32,
    pub honesty_vs_sycophancy: f32,
    pub caution_vs_alarmism: f32,
}

/// Incremental cohort adjustment applied cleanly across ports.
#[derive(Clone, Debug, PartialEq)]
pub struct CohortDelta {
    pub cohort: CohortId,
    pub delta_headcount: i32,
    pub delta_skill: f32,
}

/// Read-only and queued population access port.
pub trait PopulationPort: Send + Sync {
    fn settlement_cohorts(&self, settlement: SettlementId) -> &[CohortId];
    fn cohort_view(&self, id: CohortId) -> Option<CohortSnapshotView>;
    fn character_view(&self, id: CharacterId) -> Option<CharacterSnapshotView>;
    fn all_characters(&self) -> &[CharacterSnapshotView];
    fn apply_labor_and_welfare(&mut self, updates: &[CohortDelta]);
}

/// Attention utilization view for a commander.
#[derive(Clone, Debug, PartialEq)]
pub struct AttentionLoadView {
    pub owner: CharacterId,
    pub used_attention: f32,
    pub capacity_attention: f32,
    pub active_mandate_count: usize,
}

/// Read-only view of an active mandate.
#[derive(Clone, Debug, PartialEq)]
pub struct MandateView {
    pub id: MandateId,
    pub issuer: CharacterId,
    pub assignee: CharacterId,
    pub target_settlement: SettlementId,
    pub summary: String,
    pub status: String,
}

/// Decoupled command input handed to `sim-command` or `sim-core` via Spine.
#[derive(Clone, Debug, PartialEq)]
pub enum SpinePlayerCommand {
    SetPaused(bool),
    StepSingleTick,
    SetSpeedMultiplier(u32),
    AcknowledgeNotification(u64),
    StubPing { tag: String },
}

/// Command hierarchy and player input port.
pub trait CommandPort: Send + Sync {
    fn attention_summary(&self, who: CharacterId) -> AttentionLoadView;
    fn active_mandates_for(&self, who: CharacterId) -> &[MandateView];
    fn ingest_player_command(&mut self, tick: Tick, cmd: SpinePlayerCommand);
}

/// Error returned from optional platform integration adapters.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlatformError {
    pub message: String,
}

/// Optional store integration boundary isolated behind `steam-adapter`.
pub trait PlatformIntegrationPort: Send + Sync {
    fn ensure_launched_via_store(&self) -> Result<(), PlatformError>;
    fn unlock_milestone_flag(&self, key: &str);
    fn tick_callbacks(&mut self);
}

/// Projected date/time derived cleanly from `Tick` and scenario `CalendarConfigView`.
#[derive(Clone, Debug, PartialEq)]
pub struct CalendarDateTime {
    pub tick: Tick,
    pub year: i32,
    pub month_index: usize,
    pub month_name: String,
    pub day_of_year: u32,
    pub day_of_month: u32,
    pub hour_of_day: u32,
    pub solar_longitude_rad: f64,
    pub diurnal_angle_rad: f64,
}

/// Player or coordinator control command adjusting simulation clock rate or state.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ClockControlCommand {
    Pause,
    Resume,
    StepSingleHour,
    SetSpeedMultiplier(u32),
}

/// Signal emitted when a notification triggers automatic pause at the exact tick end.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub struct AutoPauseSignal {
    pub reason_event: EventClassId,
    pub tick: Tick,
}

/// Timing and bucket sample captured for a single completed hourly tick.
#[derive(Clone, Debug, PartialEq)]
pub struct PhaseTimingSample {
    pub tick: Tick,
    pub phase_ns: [u64; 8],
    pub total_ns: u64,
    pub bucket_entities_processed: [u32; 8],
}

/// Exportable summary telemetry returned by `TickTelemetryPort`.
#[derive(Clone, Debug, PartialEq)]
pub struct TickTelemetrySummary {
    pub sample_count: usize,
    pub total_ticks_profiled: u64,
    pub cumulative_phase_ns: [u64; 8],
    pub latest_sample: Option<PhaseTimingSample>,
}

/// Profiling port exposed by `sim-clock` for CLI and diagnostic inspection.
pub trait TickTelemetryPort: Send + Sync {
    fn telemetry_summary(&self) -> TickTelemetrySummary;
}

/// Canonical byte-level serialization port allowing lossless deterministic save/load across independent cartridges.
pub trait CanonicalSaveablePort {
    fn cartridge_tag(&self) -> &'static str;
    fn write_canonical_state(&self, out: &mut Vec<u8>);
    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String>;
}
