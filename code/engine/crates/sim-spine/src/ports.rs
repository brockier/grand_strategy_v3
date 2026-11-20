//! Narrow read-only views, port traits, and command queues connecting cartridges cleanly.

use crate::ids::{
    ActorId, CarrierClassId, CellId, CharacterId, CohortId, CommodityId, ConvoyId, DispatchId,
    EdgeId, EventClassId, FormationId, InfraClassId, MandateId, ProceduralMaterialId, ProcessId,
    SettlementId, StockpileId, TechComponentId, TheaterId, TheoryDomainId, Tick, VocationId,
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

/// Unified continental and marine tectonic provinces (`ENG-GEO-010`).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub enum GeologyProvinceKind {
    ShieldCraton = 0,
    OldOrogen = 1,
    YoungOrogen = 2,
    VolcanicArc = 3,
    SedimentaryBasin = 4,
    ContinentalRift = 5,
    #[default]
    Platform = 6,
    AlluvialPlain = 7,
    ContinentalShelf = 8,
    ContinentalSlope = 9,
    MidOceanRidge = 10,
    OceanicTrench = 11,
    AbyssalPlain = 12,
}

impl GeologyProvinceKind {
    /// Canonical string identifier matching externalized `[[geology_provinces]]` IDs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ShieldCraton => "shield_craton",
            Self::OldOrogen => "old_orogen",
            Self::YoungOrogen => "young_orogen",
            Self::VolcanicArc => "volcanic_arc",
            Self::SedimentaryBasin => "sedimentary_basin",
            Self::ContinentalRift => "continental_rift",
            Self::Platform => "platform",
            Self::AlluvialPlain => "alluvial_plain",
            Self::ContinentalShelf => "continental_shelf",
            Self::ContinentalSlope => "continental_slope",
            Self::MidOceanRidge => "mid_ocean_ridge",
            Self::OceanicTrench => "oceanic_trench",
            Self::AbyssalPlain => "abyssal_plain",
        }
    }

    /// Parses canonical province string into typed enum.
    #[must_use]
    pub fn from_key(raw: &str) -> Self {
        match raw.trim() {
            "shield_craton" => Self::ShieldCraton,
            "old_orogen" => Self::OldOrogen,
            "young_orogen" => Self::YoungOrogen,
            "volcanic_arc" => Self::VolcanicArc,
            "sedimentary_basin" => Self::SedimentaryBasin,
            "continental_rift" => Self::ContinentalRift,
            "alluvial_plain" => Self::AlluvialPlain,
            "continental_shelf" => Self::ContinentalShelf,
            "continental_slope" => Self::ContinentalSlope,
            "mid_ocean_ridge" => Self::MidOceanRidge,
            "oceanic_trench" => Self::OceanicTrench,
            "abyssal_plain" => Self::AbyssalPlain,
            _ => Self::Platform,
        }
    }

    /// Stable numeric code for binary codecs and shader LUT textures.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Decodes a stored numeric province code.
    #[must_use]
    pub const fn from_code(code: u8) -> Self {
        match code {
            0 => Self::ShieldCraton,
            1 => Self::OldOrogen,
            2 => Self::YoungOrogen,
            3 => Self::VolcanicArc,
            4 => Self::SedimentaryBasin,
            5 => Self::ContinentalRift,
            7 => Self::AlluvialPlain,
            8 => Self::ContinentalShelf,
            9 => Self::ContinentalSlope,
            10 => Self::MidOceanRidge,
            11 => Self::OceanicTrench,
            12 => Self::AbyssalPlain,
            _ => Self::Platform,
        }
    }

    /// Returns true if the province represents submarine oceanic crust or shelf margin.
    #[must_use]
    pub const fn is_marine(self) -> bool {
        matches!(
            self,
            Self::ContinentalShelf
                | Self::ContinentalSlope
                | Self::MidOceanRidge
                | Self::OceanicTrench
                | Self::AbyssalPlain
        )
    }
}

/// Hydrological lake outlet classification (`ENG-GEO-012`, `ENG-WTH-006`).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub enum LakeKind {
    #[default]
    None = 0,
    Exorheic = 1,
    Endorheic = 2,
}

impl LakeKind {
    /// Canonical string identifier for snapshots and inspection.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Exorheic => "exorheic",
            Self::Endorheic => "endorheic",
        }
    }

    /// Stable numeric codec identifier.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Decodes numeric lake classification code.
    #[must_use]
    pub const fn from_code(code: u8) -> Self {
        match code {
            1 => Self::Exorheic,
            2 => Self::Endorheic,
            _ => Self::None,
        }
    }
}

/// River reach navigation and fordability classification (`ENG-GEO-012`).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub enum RiverNavigabilityKind {
    #[default]
    None = 0,
    ShallowFord = 1,
    Rapids = 2,
    DeepNavigable = 3,
}

impl RiverNavigabilityKind {
    /// Canonical string identifier for snapshots and logistics UI.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ShallowFord => "shallow_ford",
            Self::Rapids => "rapids",
            Self::DeepNavigable => "deep_navigable",
        }
    }

    /// Stable numeric codec identifier.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Decodes numeric navigability code.
    #[must_use]
    pub const fn from_code(code: u8) -> Self {
        match code {
            1 => Self::ShallowFord,
            2 => Self::Rapids,
            3 => Self::DeepNavigable,
            _ => Self::None,
        }
    }
}

/// Reservoir dam regulation policy mode (`ENG-GEO-014`).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub enum DamRuleCurveKind {
    #[default]
    BalancedPower = 0,
    FloodControl = 1,
    IrrigationSupply = 2,
}

impl DamRuleCurveKind {
    /// Canonical snake_case identifier.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BalancedPower => "balanced_power",
            Self::FloodControl => "flood_control",
            Self::IrrigationSupply => "irrigation_supply",
        }
    }

    /// Parses string rule curve identifier safely.
    #[must_use]
    pub fn from_key(raw: &str) -> Option<Self> {
        match raw.trim() {
            "balanced_power" | "BalancedPower" => Some(Self::BalancedPower),
            "flood_control" | "FloodControl" => Some(Self::FloodControl),
            "irrigation_supply" | "IrrigationSupply" => Some(Self::IrrigationSupply),
            _ => None,
        }
    }

    /// Stable numeric codec tag.
    #[must_use]
    pub const fn code(self) -> u8 {
        self as u8
    }

    /// Decodes numeric rule curve code.
    #[must_use]
    pub const fn from_code(code: u8) -> Self {
        match code {
            1 => Self::FloodControl,
            2 => Self::IrrigationSupply,
            _ => Self::BalancedPower,
        }
    }
}

/// Live reservoir and dam operational view exposed across `SurfaceConditionsPort` (`ENG-GEO-014`).
#[derive(Clone, Debug, PartialEq)]
pub struct DamStatusView {
    pub cell_id: CellId,
    pub rule_curve: DamRuleCurveKind,
    pub reservoir_storage_m3: f64,
    pub reservoir_capacity_m3: f64,
    pub reservoir_fill_01: f32,
    pub reservoir_head_m: f32,
    pub spillway_release_m3_s: f32,
    pub live_hydro_kw: f32,
    pub sediment_silt_01: f32,
    pub condition_01: f32,
    pub breached: bool,
    pub impounded_upstream_cells: Vec<CellId>,
}

/// Externalized fluid thermophysical constants (`ENG-WTH-006`).
#[derive(Clone, Debug, PartialEq)]
pub struct FluidDefView {
    pub id: String,
    pub display_name: String,
    pub density_kg_m3: f32,
    pub freeze_point_c: f32,
    pub boil_point_c: f32,
    pub latent_heat_evap_kj_kg: f32,
    pub specific_heat_j_kg_k: f32,
}

impl Default for FluidDefView {
    fn default() -> Self {
        Self {
            id: "water".to_owned(),
            display_name: "Terrestrial Water".to_owned(),
            density_kg_m3: 1000.0,
            freeze_point_c: 0.0,
            boil_point_c: 100.0,
            latent_heat_evap_kj_kg: 2260.0,
            specific_heat_j_kg_k: 4184.0,
        }
    }
}

/// Externalized land or marine geology province metadata (`ENG-GEO-010`).
#[derive(Clone, Debug, PartialEq)]
pub struct GeologyProvinceDefView {
    pub id: String,
    pub display_name: String,
    pub is_marine: bool,
    pub typical_age_myr: f32,
    pub base_roughness_m: f32,
    pub fault_instability_01: f32,
    pub volcanic_hazard_01: f32,
    pub soil_fertility_bias: f32,
    pub marine_upwelling_bias: f32,
}

/// Externalized province-affinity subsurface mineral deposit rule (`ENG-GEO-011`).
#[derive(Clone, Debug, PartialEq)]
pub struct DepositAffinityDefView {
    pub id: String,
    pub province_id: String,
    pub category_key: String,
    pub is_offshore: bool,
    pub accessibility_tier: u8,
    pub spawn_weight: f32,
    pub min_tonnes: f64,
    pub max_tonnes: f64,
    pub min_purity_01: f32,
    pub max_purity_01: f32,
}

/// Externalized hydrological, aquifer, navigability, and dam regulation policy constants (`ENG-GEO-012`, `ENG-GEO-014`, `ENG-WTH-006`).
#[derive(Clone, Debug, PartialEq)]
pub struct HydrologyRulesView {
    pub active_fluid_id: String,
    pub infiltration_rate_01: f32,
    pub evapotranspiration_scale: f32,
    pub aquifer_diffusivity: f32,
    pub aquifer_capacity_mm: f32,
    pub spring_emergence_threshold_01: f32,
    pub estuary_max_elev_m: f32,
    pub shallow_ford_max_q_m3_s: f32,
    pub deep_navigable_min_q_m3_s: f32,
    pub rapids_min_slope_rad: f32,
    pub dam_min_discharge_m3_s: f32,
    pub dam_min_relief_m: f32,
    pub dam_turbine_efficiency_01: f32,
    pub dam_siltation_rate_per_day: f32,
    pub dam_breach_flood_threshold_01: f32,
}

impl Default for HydrologyRulesView {
    fn default() -> Self {
        Self {
            active_fluid_id: "water".to_owned(),
            infiltration_rate_01: 0.34,
            evapotranspiration_scale: 1.0,
            aquifer_diffusivity: 0.085,
            aquifer_capacity_mm: 420.0,
            spring_emergence_threshold_01: 0.88,
            estuary_max_elev_m: 14.0,
            shallow_ford_max_q_m3_s: 35.0,
            deep_navigable_min_q_m3_s: 45.0,
            rapids_min_slope_rad: 0.032,
            dam_min_discharge_m3_s: 18.0,
            dam_min_relief_m: 18.0,
            dam_turbine_efficiency_01: 0.82,
            dam_siltation_rate_per_day: 0.00015,
            dam_breach_flood_threshold_01: 0.96,
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

impl Default for NaturalBoundsView {
    fn default() -> Self {
        Self {
            ore_count: 6,
            flux_count: 3,
            fuel_count: 4,
            crop_count: 5,
            fauna_count: 4,
            pathogen_count: 3,
            ore_melting_k_range: [1150.0, 1850.0],
            ore_purity_range: [0.18, 0.72],
            fuel_enthalpy_mj_kg_range: [14.0, 44.0],
            crop_opt_temp_c_range: [8.0, 30.0],
        }
    }
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
    pub hydrology_rules: HydrologyRulesView,
    pub fluids: Vec<FluidDefView>,
    pub geology_provinces: Vec<GeologyProvinceDefView>,
    pub deposit_affinities: Vec<DepositAffinityDefView>,
}

impl Default for PlanetGenConfigView {
    fn default() -> Self {
        Self {
            id: "default_terrestrial_planet".to_owned(),
            goldberg_subdivision: 8,
            tectonic_plate_count: 14,
            land_fraction_target: 0.34,
            axial_tilt_deg: 23.4,
            solar_constant_w_m2: 1361.0,
            rotation_period_hours: 24.0,
            mean_radius_km: 6371.0,
            base_sea_level_m: 0.0,
            max_orogenic_uplift_m: 6800.0,
            planetary_density_kg_m3: 5514.0,
            natural_bounds: NaturalBoundsView::default(),
            hydrology_rules: HydrologyRulesView::default(),
            fluids: vec![FluidDefView::default()],
            geology_provinces: Vec::new(),
            deposit_affinities: Vec::new(),
        }
    }
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
    pub energy_mj_per_kg: f64,
    pub structural_strength: f64,
    pub is_tool: bool,
    pub tool_wear_per_hour: f64,
    pub caloric_mcal_per_kg: f64,
    pub potable_liters_per_kg: f64,
    pub heating_mj_per_kg: f64,
    pub is_clothing: bool,
    pub tool_hardness_rating: f32,
    pub is_slag_or_waste: bool,
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
    pub fallback_tool_commodity: Option<CommodityId>,
    pub waste_commodity: Option<CommodityId>,
    pub weather_halt_mud_threshold: f32,
    pub is_outdoor_construction: bool,
    pub is_underground_mine: bool,
    pub is_crop_cultivation: bool,
    pub is_mine_drainage: bool,
    pub min_temp_c: f32,
    pub max_temp_c: f32,
    pub optimal_moisture_01: f32,
    pub tool_wear_intensity: f64,
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
    pub allowed_infra_classes: Vec<InfraClassId>,
    pub is_watercraft: bool,
    pub uses_wind_sail: bool,
    pub max_ice_thickness_m: f32,
    pub spare_parts_commodity: Option<CommodityId>,
    pub spare_parts_wear_per_km: f64,
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

/// Read-only view of an interned demographic cohort archetype definition.
#[derive(Clone, Debug, PartialEq)]
pub struct CohortArchetypeDefView {
    pub index: u16,
    pub key: String,
    pub display_name: String,
    pub daily_grain_need_kg: f64,
    pub daily_water_need_kg: f64,
    pub daily_fuel_need_kg_below_10c: f64,
    pub daily_clothing_wear_kg: f64,
    pub base_fertility_annual: f32,
    pub base_mortality_annual: f32,
    pub initial_literacy: f32,
    pub dependents_share_01: f32,
    pub elders_share_01: f32,
}

/// Scenario starting archetype selector (`ENG-SCN-001`).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub enum ScenarioArchetypeKind {
    EmergentDawn,
    #[default]
    ExpeditionColony,
    GovernorSuccessor,
}

impl ScenarioArchetypeKind {
    /// Parses external scenario profile key into typed archetype enum.
    #[must_use]
    pub fn from_profile_str(raw: &str) -> Self {
        match raw.trim() {
            "EmergentDawn" | "emergent_dawn" => Self::EmergentDawn,
            "GovernorSuccessor" | "governor_successor" | "SuccessorGovernor" => {
                Self::GovernorSuccessor
            }
            _ => Self::ExpeditionColony,
        }
    }

    /// Stable machine label for telemetry and inspection.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmergentDawn => "EmergentDawn",
            Self::ExpeditionColony => "ExpeditionColony",
            Self::GovernorSuccessor => "GovernorSuccessor",
        }
    }
}

/// Per-event-class notification and auto-pause tier (`ENG-SCN-003`, `UI-HUD-006`).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub enum NotificationBehaviorKind {
    #[default]
    LogOnly = 0,
    BannerAlert = 1,
    DispatchPopup = 2,
    DispatchPopupAndAutoPause = 3,
}

impl NotificationBehaviorKind {
    /// Parses external mod string representation into a strict tier.
    #[must_use]
    pub fn from_mod_str(raw: &str) -> Self {
        match raw.trim() {
            "BannerAlert" | "banner_alert" => Self::BannerAlert,
            "DispatchPopup" | "dispatch_popup" => Self::DispatchPopup,
            "DispatchPopupAutoPause"
            | "DispatchPopupAndAutoPause"
            | "dispatch_popup_and_auto_pause" => Self::DispatchPopupAndAutoPause,
            _ => Self::LogOnly,
        }
    }

    /// Canonical snake_case identifier shared with IPC and telemetry.
    #[must_use]
    pub const fn as_snake_str(self) -> &'static str {
        match self {
            Self::LogOnly => "log_only",
            Self::BannerAlert => "banner_alert",
            Self::DispatchPopup => "dispatch_popup",
            Self::DispatchPopupAndAutoPause => "dispatch_popup_and_auto_pause",
        }
    }
}

/// Movement channel sharing unified graph routing physics (`ENG-LOG-003`).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum TransitChannelKind {
    FreightCargo = 0,
    CohortMigration = 1,
    MilitaryMarch = 2,
    HqRetinue = 3,
    SurveyExpedition = 4,
    PhysicalCourier = 5,
}

/// Continuous cohort grievance & unrest progression stage (`ENG-POP-005`).
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Default)]
pub enum UnrestStage {
    #[default]
    Compliant = 0,
    WorkSlowdown = 1,
    LaborStrike = 2,
    OutMigration = 3,
    Banditry = 4,
    LocalRevolt = 5,
}

impl UnrestStage {
    /// Human-readable label for UI & dispatches.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Compliant => "Compliant",
            Self::WorkSlowdown => "WorkSlowdown",
            Self::LaborStrike => "LaborStrike",
            Self::OutMigration => "OutMigration",
            Self::Banditry => "Banditry",
            Self::LocalRevolt => "LocalRevolt",
        }
    }
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
    pub stockpile_roofs_01: Vec<f32>,
    pub cohorts: Vec<(VocationId, u32)>,
    pub cohort_archetype_indices: Vec<usize>,
    pub initial_facilities: Vec<(ProcessId, u32)>,
    pub initial_facility_owners: Vec<ActorId>,
    pub initial_shaft_water_01: f32,
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
    pub initiative_01: f32,
    pub adaptability_01: f32,
    pub ambition_01: f32,
    pub loyalty_01: f32,
    pub greed_skimming_01: f32,
}

/// Scenario launch descriptor passed from `sim-data` through `sim-spine` into engine cartridges.
#[derive(Clone, Debug, PartialEq)]
pub struct ScenarioInitView {
    pub scenario_key: String,
    pub display_name: String,
    pub archetype: ScenarioArchetypeKind,
    pub hq_settlement_key: String,
    pub hq_sight_radius_cells: u8,
    pub unexplored_surrounding_cells: bool,
    pub goldberg_subdivision: u8,
    pub governance_base_attention: f32,
    pub clerical_overhead_per_mandate: f32,
    pub hierarchy_hop_latency_hours: u32,
    pub settlements: Vec<ScenarioSettlementSeed>,
    pub characters: Vec<ScenarioCharacterSeed>,
}

/// Read-only governance institutional model definition (`ENG-CMD-006`).
#[derive(Clone, Debug, PartialEq)]
pub struct GovernanceModelDefView {
    pub id: u16,
    pub key: String,
    pub display_name: String,
    pub base_attention_capacity: f32,
    pub clerical_overhead_per_mandate: f32,
    pub hierarchy_hop_latency_hours: u32,
}

/// Read-only fiscal / economic policy law definition (`ENG-ECO-007`).
#[derive(Clone, Debug, PartialEq)]
pub struct PolicyLawDefView {
    pub id: u16,
    pub key: String,
    pub display_name: String,
    pub tax_or_tribute_rate: f32,
    pub patent_duration_years: u32,
}

/// Read-only empirical technical component ("Know-How") primitive (`ENG-RES-001`).
#[derive(Clone, Debug, PartialEq)]
pub struct TechComponentDefView {
    pub id: TechComponentId,
    pub key: String,
    pub display_name: String,
    pub domain_key: String,
    pub empirical_difficulty: f32,
}

/// Read-only theoretical inquiry ("Know-Why") domain (`ENG-RES-002`).
#[derive(Clone, Debug, PartialEq)]
pub struct TheoryDomainDefView {
    pub id: TheoryDomainId,
    pub key: String,
    pub display_name: String,
    pub abstraction_depth: u8,
}

/// Read-only military formation template (`ENG-WAR-001`).
#[derive(Clone, Debug, PartialEq)]
pub struct FormationTemplateDefView {
    pub id: u16,
    pub key: String,
    pub display_name: String,
    pub personnel_count: u32,
    pub required_vocation: VocationId,
    pub equipment_commodity: CommodityId,
    pub equipment_kg_per_troop: f64,
}

/// Read-only operational doctrine definition (`ENG-WAR-002`).
#[derive(Clone, Debug, PartialEq)]
pub struct DoctrineDefView {
    pub id: u16,
    pub key: String,
    pub display_name: String,
    pub initiative_bonus: f32,
    pub strict_obedience_weight: f32,
    pub rain_penalty_mitigation: f32,
}

/// Read-only succession protocol definition (`ENG-CMD-007`, `ENG-POP-006`).
#[derive(Clone, Debug, PartialEq)]
pub struct SuccessionRuleDefView {
    pub id: u16,
    pub key: String,
    pub display_name: String,
    pub continuity_fraction: f32,
    pub transition_disruption_hours: u32,
}

/// Read-only registry port populated by `sim-data` and consumed by engine cartridges.
pub trait ModRegistryPort: Send + Sync {
    fn commodity_def(&self, id: CommodityId) -> Option<&CommodityDefView>;
    fn commodity_count(&self) -> usize;
    fn commodity_id_by_key(&self, key: &str) -> Option<CommodityId>;

    fn process_def(&self, id: ProcessId) -> Option<&ProcessDefView>;
    fn process_count(&self) -> usize;

    fn carrier_def(&self, id: CarrierClassId) -> Option<&CarrierDefView>;
    fn carrier_count(&self) -> usize;
    fn carrier_id_by_key(&self, key: &str) -> Option<CarrierClassId>;

    fn infra_def(&self, id: InfraClassId) -> Option<&InfraDefView>;
    fn infra_count(&self) -> usize;
    fn infra_id_by_key(&self, key: &str) -> Option<InfraClassId>;

    fn vocation_def(&self, id: VocationId) -> Option<&VocationDefView>;
    fn vocation_count(&self) -> usize;

    fn cohort_archetype_count(&self) -> usize;
    fn cohort_archetype_def(&self, idx: usize) -> Option<&CohortArchetypeDefView>;

    fn event_class_id(&self, key: &str) -> Option<EventClassId>;
    fn notification_default_behavior(&self, key: &str) -> Option<NotificationBehaviorKind>;
    fn notification_classes(&self) -> Vec<(EventClassId, String, String, NotificationBehaviorKind)>;
    fn localization_template(&self, key: &str) -> Option<&str>;

    fn calendar_config(&self) -> &CalendarConfigView;
    fn planet_gen_config(&self) -> &PlanetGenConfigView;
    fn bundle_blake3_bytes(&self) -> [u8; 32];
    fn scenario_init(&self) -> &ScenarioInitView;

    fn governance_models(&self) -> &[GovernanceModelDefView] {
        &[]
    }

    fn policy_laws(&self) -> &[PolicyLawDefView] {
        &[]
    }

    fn tech_components(&self) -> &[TechComponentDefView] {
        &[]
    }

    fn theory_domains(&self) -> &[TheoryDomainDefView] {
        &[]
    }

    fn formation_templates(&self) -> &[FormationTemplateDefView] {
        &[]
    }

    fn doctrines(&self) -> &[DoctrineDefView] {
        &[]
    }

    fn succession_rules(&self) -> &[SuccessionRuleDefView] {
        &[]
    }
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

    fn nearest_sample_cell(&self, _unit_xyz: [f32; 3]) -> CellId {
        CellId(0)
    }

    fn sample_elevation_at_unit_xyz(&self, unit_xyz: [f32; 3]) -> f32 {
        self.cell_elevation_m(self.nearest_sample_cell(unit_xyz))
    }

    fn icosphere_triangles(&self) -> &[[u32; 3]] {
        &[]
    }

    fn watershed_id(&self, _cell: CellId) -> crate::ids::WatershedId {
        crate::ids::WatershedId(0)
    }

    fn region_label(&self, cell: CellId) -> String {
        format!("Watershed #{}", self.watershed_id(cell).0)
    }

    fn radius_km(&self) -> f32 {
        6371.0
    }

    fn cell_mean_annual_temp_c(&self, _cell: CellId) -> f32 {
        12.0
    }

    fn cell_aridity_index_01(&self, _cell: CellId) -> f32 {
        0.5
    }

    fn cell_soil_fertility_01(&self, _cell: CellId) -> f32 {
        0.5
    }

    fn cell_strahler_order(&self, _cell: CellId) -> u8 {
        1
    }

    fn cell_strata_code(&self, _cell: CellId) -> u8 {
        4
    }

    fn cell_is_lake(&self, _cell: CellId) -> bool {
        false
    }

    fn cell_river_channel_downstream(&self, _cell: CellId) -> Option<CellId> {
        None
    }

    fn cell_is_perennial_river(&self, cell: CellId) -> bool {
        self.cell_river_channel_downstream(cell).is_some()
    }

    fn cell_river_discharge_m3_s(&self, _cell: CellId) -> f32 {
        0.0
    }

    fn cell_river_velocity_mps(&self, _cell: CellId) -> f32 {
        0.0
    }

    fn cell_hydro_potential_kw(&self, _cell: CellId) -> f32 {
        0.0
    }

    fn cell_geology_province(&self, cell: CellId) -> GeologyProvinceKind {
        if self.cell_elevation_m(cell) < 0.0 {
            GeologyProvinceKind::AbyssalPlain
        } else {
            GeologyProvinceKind::Platform
        }
    }

    fn cell_roughness_m(&self, _cell: CellId) -> f32 {
        28.0
    }

    fn cell_basin_depth_m(&self, _cell: CellId) -> f32 {
        0.0
    }

    fn cell_fault_instability_01(&self, _cell: CellId) -> f32 {
        0.1
    }

    fn cell_volcanic_hazard_01(&self, _cell: CellId) -> f32 {
        0.05
    }

    fn cell_soil_drainage_01(&self, _cell: CellId) -> f32 {
        0.5
    }

    fn cell_soil_clay_01(&self, _cell: CellId) -> f32 {
        0.35
    }

    fn cell_marine_upwelling_01(&self, _cell: CellId) -> f32 {
        0.0
    }

    fn cell_shelf_nutrient_01(&self, _cell: CellId) -> f32 {
        0.0
    }

    fn cell_channel_bed_elev_m(&self, cell: CellId) -> f32 {
        self.cell_elevation_m(cell)
    }

    fn cell_water_surface_elev_m(&self, cell: CellId) -> f32 {
        self.cell_elevation_m(cell).max(0.0)
    }

    fn cell_lake_kind(&self, cell: CellId) -> LakeKind {
        if self.cell_is_lake(cell) {
            LakeKind::Exorheic
        } else {
            LakeKind::None
        }
    }

    fn cell_lake_salinity_per_mille(&self, cell: CellId) -> f32 {
        if self.cell_elevation_m(cell) < 0.0 {
            35.0
        } else if self.cell_lake_kind(cell) == LakeKind::Endorheic {
            48.0
        } else {
            0.2
        }
    }

    fn cell_is_estuary(&self, _cell: CellId) -> bool {
        false
    }

    fn cell_river_navigability(&self, cell: CellId) -> RiverNavigabilityKind {
        if self.cell_river_channel_downstream(cell).is_none() {
            RiverNavigabilityKind::None
        } else if self.cell_river_discharge_m3_s(cell) >= 45.0 {
            RiverNavigabilityKind::DeepNavigable
        } else {
            RiverNavigabilityKind::ShallowFord
        }
    }
}

/// Normalizes a 3D direction vector to the unit sphere.
#[must_use]
pub fn normalize_unit_xyz(v: [f32; 3]) -> [f32; 3] {
    let x = f64::from(v[0]);
    let y = f64::from(v[1]);
    let z = f64::from(v[2]);
    let len = (x * x + y * y + z * z).sqrt();
    if len <= 1e-12 {
        [0.0, 1.0, 0.0]
    } else {
        [(x / len) as f32, (y / len) as f32, (z / len) as f32]
    }
}

/// Computes exact spherical great-circle surface arc distance in kilometers between two unit vectors.
#[must_use]
pub fn great_circle_arc_km(a: [f32; 3], b: [f32; 3], radius_km: f32) -> f32 {
    let dot = (f64::from(a[0]) * f64::from(b[0])
        + f64::from(a[1]) * f64::from(b[1])
        + f64::from(a[2]) * f64::from(b[2]))
    .clamp(-1.0, 1.0);
    (dot.acos() * f64::from(radius_km.max(1.0))) as f32
}

/// Spherical linear interpolation along the shortest great-circle arc between unit vectors `a` and `b`.
#[must_use]
pub fn slerp_unit_xyz(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    let clamped = f64::from(t.clamp(0.0, 1.0));
    let ax = f64::from(a[0]);
    let ay = f64::from(a[1]);
    let az = f64::from(a[2]);
    let bx = f64::from(b[0]);
    let by = f64::from(b[1]);
    let bz = f64::from(b[2]);
    let dot = (ax * bx + ay * by + az * bz).clamp(-1.0, 1.0);
    let omega = dot.acos();
    if omega < 1e-6 {
        return normalize_unit_xyz([
            (ax + (bx - ax) * clamped) as f32,
            (ay + (by - ay) * clamped) as f32,
            (az + (bz - az) * clamped) as f32,
        ]);
    }
    let sin_omega = omega.sin();
    let s0 = ((1.0 - clamped) * omega).sin() / sin_omega;
    let s1 = (clamped * omega).sin() / sin_omega;
    normalize_unit_xyz([
        (s0 * ax + s1 * bx) as f32,
        (s0 * ay + s1 * by) as f32,
        (s0 * az + s1 * bz) as f32,
    ])
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
    pub accessibility_tier: u8,
    pub is_offshore: bool,
    pub is_alluvial_placer: bool,
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

    fn cell_groundwater_m3(&self, _cell: CellId) -> f64 {
        0.0
    }

    fn cell_is_spring_oasis(&self, _cell: CellId) -> bool {
        false
    }

    fn cell_salinity_per_mille(&self, _cell: CellId) -> f32 {
        0.2
    }

    fn cell_live_discharge_m3_s(&self, _cell: CellId) -> f32 {
        0.0
    }

    fn cell_live_navigability(&self, _cell: CellId) -> RiverNavigabilityKind {
        RiverNavigabilityKind::None
    }

    fn active_dams(&self) -> Vec<DamStatusView> {
        Vec::new()
    }

    fn dam_at_cell(&self, _cell: CellId) -> Option<DamStatusView> {
        None
    }
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
    pub channel: TransitChannelKind,
    pub origin_cell: CellId,
    pub current_cell: CellId,
    pub destination_cell: CellId,
    pub planned_route: Vec<CellId>,
    pub origin_unit_xyz: [f32; 3],
    pub current_unit_xyz: [f32; 3],
    pub destination_unit_xyz: [f32; 3],
    pub route_waypoints_xyz: Vec<[f32; 3]>,
    pub distance_travelled_km: f32,
    pub total_route_km: f32,
    pub departure_tick: Tick,
    pub expected_arrival_tick: Tick,
    pub grace_ticks: u64,
    pub cargo_mass_kg: f64,
    pub is_hq_move: bool,
    pub stranded_no_fuel: bool,
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
    fn all_active_convoys(&self) -> Vec<ConvoyTransitView> {
        Vec::new()
    }
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
    fn unrest_stage_for_settlement(&self, _settlement: SettlementId) -> UnrestStage {
        UnrestStage::Compliant
    }
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
    IssueMandate {
        assignee: CharacterId,
        objective_kind: String,
        target_cell: CellId,
        priority_weight: u32,
        authority_tier: String,
    },
    ResolveArbitrationProposal {
        proposal_id: u64,
        chosen_option_index: u32,
    },
    SetPersonalAttentionZoom {
        target_mandate_id: Option<MandateId>,
        share_01: f32,
    },
    RelocateHq {
        destination_cell: CellId,
    },
    UpdateNotificationRule {
        event_class_key: String,
        behavior: NotificationBehaviorKind,
    },
    SwitchGovernanceModel {
        model_key: String,
    },
    EnactEconomicPolicy {
        policy_key: String,
        rate_override_01: Option<f32>,
    },
    CharterPrivateCarriers {
        target_settlement: SettlementId,
        count: u32,
    },
    SetInnovationMode {
        settlement: SettlementId,
        mode_key: String,
    },
    IssueTheaterDirective {
        theater_id: TheaterId,
        commander: CharacterId,
        objective_cell: CellId,
        supply_priority_01: f32,
    },
    IssueOperationalBattlePlan {
        formation_id: FormationId,
        target_cell: CellId,
        doctrine_key: String,
        no_retreat: bool,
    },
    AcknowledgeSuccession,
    ConstructDam {
        target_cell: CellId,
        rule_curve: DamRuleCurveKind,
    },
    SetDamRuleCurve {
        target_cell: CellId,
        rule_curve: DamRuleCurveKind,
    },
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
