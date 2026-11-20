//! Strict Epistemic Air-Gap & IPC DTO Boundary (`player-view`).
//! The **only** simulation crate allowed to emit IPC JSON payloads to `code/ui`.
#![deny(clippy::disallowed_types)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use serde::{Deserialize, Serialize};
use sim_spine::{
    codec::{
        read_f32_bits, read_f64_bits, read_str, read_u16_le, read_u32_le, read_u64_le, read_u8,
        write_f32_bits, write_f64_bits, write_str, write_u16_le, write_u32_le, write_u64_le,
        write_u8,
    },
    slerp_unit_xyz, CanonicalSaveablePort, CellId, CharacterId, CharacterSnapshotView,
    CommandPort, DamRuleCurveKind, DamStatusView, FormationId, GovernanceFrictionPlugPort,
    KnowledgeModifierPlugPort, LogisticsPort, MandateId, ModRegistryPort,
    NonStateEconomyPlugPort, NotificationBehaviorKind, OperationalCombatPlugPort,
    PhysicalLedgerPort, PlanetGeometryPort, PopulationPort, SettlementId, SpinePlayerCommand,
    SuccessionPlugPort, SurfaceConditionsPort, TheaterId, Tick,
};

/// IPC-safe mod diagnostic payload compatible with `UI-SHL-004`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModDiagnosticDto {
    pub severity: String,
    pub layer: String,
    pub file_path: String,
    pub line: usize,
    pub column: usize,
    pub field_path: String,
    pub message: String,
}

/// Top-level error screen DTO sent when mod validation blocks world launch.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModValidationFailureState {
    pub ok: bool,
    pub diagnostics: Vec<ModDiagnosticDto>,
}

/// Probabilistic geological deposit survey estimate never exposing raw hidden deposits (`ENG-EPI-006`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DepositEstimateDto {
    pub material_id: u16,
    pub material_label: String,
    pub category_key: String,
    pub min_tonnes: f64,
    pub point_est_tonnes: f64,
    pub max_tonnes: f64,
    pub purity_est_01: f32,
    pub confidence_01: f32,
    pub surveyor_skill_01: f32,
    pub instrument_tier: u8,
    pub observed_tick: u64,
    pub provenance_label: String,
    #[serde(default)]
    pub accessibility_tier: u8,
    #[serde(default)]
    pub is_offshore: bool,
    #[serde(default)]
    pub is_alluvial_placer: bool,
}

/// Live dam and reservoir impoundment state surfaced on surveyed river cells (`ENG-GEO-014`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DamStatusDto {
    pub cell_id: u32,
    pub rule_curve: String,
    pub reservoir_storage_m3: f64,
    pub reservoir_capacity_m3: f64,
    pub reservoir_fill_01: f32,
    pub reservoir_head_m: f32,
    pub spillway_release_m3_s: f32,
    pub live_hydro_kw: f32,
    pub sediment_silt_01: f32,
    pub condition_01: f32,
    pub breached: bool,
}

impl From<DamStatusView> for DamStatusDto {
    fn from(v: DamStatusView) -> Self {
        Self {
            cell_id: v.cell_id.get(),
            rule_curve: v.rule_curve.as_str().to_owned(),
            reservoir_storage_m3: v.reservoir_storage_m3,
            reservoir_capacity_m3: v.reservoir_capacity_m3,
            reservoir_fill_01: v.reservoir_fill_01,
            reservoir_head_m: v.reservoir_head_m,
            spillway_release_m3_s: v.spillway_release_m3_s,
            live_hydro_kw: v.live_hydro_kw,
            sediment_silt_01: v.sediment_silt_01,
            condition_01: v.condition_01,
            breached: v.breached,
        }
    }
}

fn default_mean_temp_c() -> f32 {
    12.0
}

fn default_aridity_01() -> f32 {
    0.55
}

fn default_soil_fertility_01() -> f32 {
    0.55
}

fn default_strata_code() -> u8 {
    4
}

fn default_strahler_order() -> u8 {
    1
}

/// Knowledge state of a planetary cell inside `DiegeticSnapshot` across 4 epistemic tiers (`ENG-EPI-002`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tier", rename_all = "camelCase")]
pub enum KnownCellDto {
    Unexplored {
        #[serde(rename = "cellId")]
        cell_id: u32,
    },
    Rumored {
        #[serde(rename = "cellId")]
        cell_id: u32,
        #[serde(rename = "observedTick")]
        observed_tick: u64,
        #[serde(rename = "elevationM")]
        elevation_m: f32,
        #[serde(rename = "confidence01")]
        confidence_01: f32,
        #[serde(rename = "provenanceLabel")]
        provenance_label: String,
        #[serde(default, rename = "biomeKey")]
        biome_key: String,
        #[serde(default, rename = "isOcean")]
        is_ocean: bool,
        #[serde(default, rename = "watershedId")]
        watershed_id: u32,
        #[serde(default, rename = "regionLabel")]
        region_label: String,
    },
    Stale {
        #[serde(rename = "cellId")]
        cell_id: u32,
        #[serde(rename = "observedTick")]
        observed_tick: u64,
        #[serde(rename = "lastLiveTick")]
        last_live_tick: u64,
        #[serde(rename = "elevationM")]
        elevation_m: f32,
        #[serde(rename = "confidence01")]
        confidence_01: f32,
        #[serde(rename = "provenanceLabel")]
        provenance_label: String,
        #[serde(default, rename = "biomeKey")]
        biome_key: String,
        #[serde(default, rename = "isOcean")]
        is_ocean: bool,
        #[serde(default, rename = "riverDownstreamCellId")]
        river_downstream_cell_id: Option<u32>,
        #[serde(default, rename = "riverFlowNorm01")]
        river_flow_norm_01: f32,
        #[serde(default, rename = "depositEstimates")]
        deposit_estimates: Vec<DepositEstimateDto>,
        #[serde(default, rename = "watershedId")]
        watershed_id: u32,
        #[serde(default, rename = "regionLabel")]
        region_label: String,
        #[serde(default = "default_mean_temp_c", rename = "meanAnnualTempC")]
        mean_annual_temp_c: f32,
        #[serde(default, rename = "annualPrecipMm")]
        annual_precip_mm: f32,
        #[serde(default = "default_aridity_01", rename = "aridityIndex01")]
        aridity_index_01: f32,
        #[serde(default = "default_soil_fertility_01", rename = "soilFertility01")]
        soil_fertility_01: f32,
        #[serde(default = "default_strata_code", rename = "rockStrataCode")]
        rock_strata_code: u8,
        #[serde(default, rename = "drainageAreaKm2")]
        drainage_area_km2: f32,
        #[serde(default = "default_strahler_order", rename = "strahlerOrder")]
        strahler_order: u8,
        #[serde(default, rename = "isLake")]
        is_lake: bool,
        #[serde(default, rename = "riverChannelDownstreamCellId")]
        river_channel_downstream_cell_id: Option<u32>,
        #[serde(default, rename = "riverDischargeM3S")]
        river_discharge_m3_s: f32,
        #[serde(default, rename = "riverVelocityMps")]
        river_velocity_mps: f32,
        #[serde(default, rename = "hydroPotentialKw")]
        hydro_potential_kw: f32,
        #[serde(default, rename = "geologyProvince")]
        geology_province: String,
        #[serde(default, rename = "roughnessM")]
        roughness_m: f32,
        #[serde(default, rename = "basinDepthM")]
        basin_depth_m: f32,
        #[serde(default, rename = "faultInstability01")]
        fault_instability_01: f32,
        #[serde(default, rename = "volcanicHazard01")]
        volcanic_hazard_01: f32,
        #[serde(default, rename = "soilDrainage01")]
        soil_drainage_01: f32,
        #[serde(default, rename = "soilClay01")]
        soil_clay_01: f32,
        #[serde(default, rename = "marineUpwelling01")]
        marine_upwelling_01: f32,
        #[serde(default, rename = "shelfNutrient01")]
        shelf_nutrient_01: f32,
        #[serde(default, rename = "lakeKind")]
        lake_kind: String,
        #[serde(default, rename = "lakeSalinityPerMille")]
        lake_salinity_per_mille: f32,
        #[serde(default, rename = "isEstuary")]
        is_estuary: bool,
        #[serde(default, rename = "riverNavigability")]
        river_navigability: String,
        #[serde(default, rename = "isSpringOasis")]
        is_spring_oasis: bool,
        #[serde(default, rename = "groundwaterM3")]
        groundwater_m3: f64,
        #[serde(default, rename = "damStatus")]
        dam_status: Option<DamStatusDto>,
    },
    Surveyed {
        #[serde(rename = "cellId")]
        cell_id: u32,
        #[serde(rename = "observedTick")]
        observed_tick: u64,
        #[serde(rename = "isLiveHqSight")]
        is_live_hq_sight: bool,
        #[serde(rename = "elevationM")]
        elevation_m: f32,
        #[serde(rename = "confidence01")]
        confidence_01: f32,
        #[serde(rename = "provenanceLabel")]
        provenance_label: String,
        #[serde(default, rename = "biomeKey")]
        biome_key: String,
        #[serde(default, rename = "isOcean")]
        is_ocean: bool,
        #[serde(default, rename = "riverDownstreamCellId")]
        river_downstream_cell_id: Option<u32>,
        #[serde(default, rename = "riverFlowNorm01")]
        river_flow_norm_01: f32,
        #[serde(default, rename = "depositEstimates")]
        deposit_estimates: Vec<DepositEstimateDto>,
        #[serde(default, rename = "watershedId")]
        watershed_id: u32,
        #[serde(default, rename = "regionLabel")]
        region_label: String,
        #[serde(default = "default_mean_temp_c", rename = "meanAnnualTempC")]
        mean_annual_temp_c: f32,
        #[serde(default, rename = "annualPrecipMm")]
        annual_precip_mm: f32,
        #[serde(default = "default_aridity_01", rename = "aridityIndex01")]
        aridity_index_01: f32,
        #[serde(default = "default_soil_fertility_01", rename = "soilFertility01")]
        soil_fertility_01: f32,
        #[serde(default = "default_strata_code", rename = "rockStrataCode")]
        rock_strata_code: u8,
        #[serde(default, rename = "drainageAreaKm2")]
        drainage_area_km2: f32,
        #[serde(default = "default_strahler_order", rename = "strahlerOrder")]
        strahler_order: u8,
        #[serde(default, rename = "isLake")]
        is_lake: bool,
        #[serde(default, rename = "riverChannelDownstreamCellId")]
        river_channel_downstream_cell_id: Option<u32>,
        #[serde(default, rename = "riverDischargeM3S")]
        river_discharge_m3_s: f32,
        #[serde(default, rename = "riverVelocityMps")]
        river_velocity_mps: f32,
        #[serde(default, rename = "hydroPotentialKw")]
        hydro_potential_kw: f32,
        #[serde(default, rename = "geologyProvince")]
        geology_province: String,
        #[serde(default, rename = "roughnessM")]
        roughness_m: f32,
        #[serde(default, rename = "basinDepthM")]
        basin_depth_m: f32,
        #[serde(default, rename = "faultInstability01")]
        fault_instability_01: f32,
        #[serde(default, rename = "volcanicHazard01")]
        volcanic_hazard_01: f32,
        #[serde(default, rename = "soilDrainage01")]
        soil_drainage_01: f32,
        #[serde(default, rename = "soilClay01")]
        soil_clay_01: f32,
        #[serde(default, rename = "marineUpwelling01")]
        marine_upwelling_01: f32,
        #[serde(default, rename = "shelfNutrient01")]
        shelf_nutrient_01: f32,
        #[serde(default, rename = "lakeKind")]
        lake_kind: String,
        #[serde(default, rename = "lakeSalinityPerMille")]
        lake_salinity_per_mille: f32,
        #[serde(default, rename = "isEstuary")]
        is_estuary: bool,
        #[serde(default, rename = "riverNavigability")]
        river_navigability: String,
        #[serde(default, rename = "isSpringOasis")]
        is_spring_oasis: bool,
        #[serde(default, rename = "groundwaterM3")]
        groundwater_m3: f64,
        #[serde(default, rename = "damStatus")]
        dam_status: Option<DamStatusDto>,
    },
}

impl KnownCellDto {
    /// Returns the planetary cell identifier regardless of knowledge tier.
    #[must_use]
    pub fn cell_id(&self) -> u32 {
        match *self {
            KnownCellDto::Unexplored { cell_id }
            | KnownCellDto::Rumored { cell_id, .. }
            | KnownCellDto::Stale { cell_id, .. }
            | KnownCellDto::Surveyed { cell_id, .. } => cell_id,
        }
    }

    fn deposit_estimates(&self) -> &[DepositEstimateDto] {
        match self {
            KnownCellDto::Surveyed {
                deposit_estimates, ..
            }
            | KnownCellDto::Stale {
                deposit_estimates, ..
            } => deposit_estimates,
            KnownCellDto::Unexplored { .. } | KnownCellDto::Rumored { .. } => &[],
        }
    }
}

/// Known physical settlement visible inside `DiegeticSnapshot`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnownSettlementDto {
    pub settlement_id: u32,
    pub settlement_name: String,
    pub cell_id: u32,
    pub is_hq: bool,
}

/// Surveyed road or bridge spline segment connecting two adjacent surveyed land cells.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SurveyedRoadEdgeDto {
    pub edge_id: u32,
    pub from_cell_id: u32,
    pub to_cell_id: u32,
    pub infra_key: String,
    #[serde(default)]
    pub washed_out: bool,
    #[serde(default)]
    pub river_edge: bool,
    #[serde(default)]
    pub bridge_cap_kg: Option<f64>,
    #[serde(default)]
    pub road_purpose: String,
    #[serde(default)]
    pub purpose_label: String,
    #[serde(default)]
    pub cumulative_passages: u32,
    #[serde(default)]
    pub cumulative_tonnes_passed: f64,
}

/// Communication delay contour sample from current HQ across known settlements/cells.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommLatencyContourDto {
    pub cell_id: u32,
    pub estimated_courier_hours: f32,
}

/// Preview option showing expected courier latency tradeoff for relocating physical HQ (`UI-HUD-005`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HqRelocationPreviewDto {
    pub target_cell_id: u32,
    pub label: String,
    pub travel_hours: f32,
    pub projected_haven_latency_hours: f32,
}

/// Physical HQ presence DTO including transit state, courier contours, and relocation previews (`ENG-EPI-003`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HqPresenceDto {
    pub mode: String,
    pub cell_id: u32,
    pub settlement_name: String,
    pub sight_radius_cells: u8,
    #[serde(default)]
    pub in_transit: bool,
    #[serde(default)]
    pub destination_cell_id: Option<u32>,
    #[serde(default)]
    pub eta_tick: Option<u64>,
    #[serde(default)]
    pub communication_contours: Vec<CommLatencyContourDto>,
    #[serde(default)]
    pub relocation_previews: Vec<HqRelocationPreviewDto>,
}

/// Globe topological metadata for buffer creation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobeTopologyDto {
    pub subdivision: u8,
    pub cell_count: u32,
    #[serde(default)]
    pub triangles: Vec<[u32; 3]>,
}

/// Observed hourly weather strictly filtered to live HQ or arrived report cells (`UI-MAP-004`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObservedWeatherDto {
    pub cell_id: u32,
    pub observed_tick: u64,
    pub precip_mm_hr: f32,
    pub mud_index_01: f32,
    pub temp_c: f32,
    pub visibility_01: f32,
    #[serde(default)]
    pub flood_stage_01: f32,
    #[serde(default)]
    pub snow_depth_m: f32,
    #[serde(default)]
    pub ice_thickness_m: f32,
    #[serde(default)]
    pub wind_u_mps: f32,
    #[serde(default)]
    pub wind_v_mps: f32,
}

/// Reported commodity ledger row with explicit epistemic provenance bounds (`ENG-EPI-007`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReportedLedgerRowDto {
    pub settlement_id: u32,
    pub settlement_name: String,
    pub commodity_key: String,
    pub commodity_name: String,
    pub reported_mass_kg: f64,
    pub min_est_kg: f64,
    pub max_est_kg: f64,
    pub observed_tick: u64,
    pub provenance_label: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub confidence_01: f32,
    #[serde(default)]
    pub reporter_name: String,
}

/// Reported demographic cohort ledger row (`UI-DAT-002`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PopulationCohortRowDto {
    pub cohort_id: u32,
    pub settlement_id: u32,
    pub settlement_name: String,
    pub vocation_key: String,
    pub vocation_name: String,
    pub reported_headcount: u32,
    pub min_headcount: u32,
    pub max_headcount: u32,
    pub mean_skill_01: f32,
    pub literacy_01: f32,
    pub nutrition_01: f32,
    pub unrest_stage: String,
    pub observed_tick: u64,
    pub confidence_01: f32,
    pub provenance_label: String,
}

/// Reported extraction or production facility ledger row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FacilityRowDto {
    pub facility_id: u32,
    pub settlement_id: u32,
    pub settlement_name: String,
    pub process_key: String,
    pub process_name: String,
    pub assigned_workers: u32,
    pub tool_condition_01: f32,
    pub weather_halted: bool,
    pub shaft_water_01: f32,
    pub observed_tick: u64,
    pub provenance_label: String,
}

/// Single time-series observation with uncertainty bounds (`UI-DAT-003`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSeriesPointDto {
    pub tick: u64,
    pub value: f64,
    pub min_bound: f64,
    pub max_bound: f64,
    pub is_confirmed: bool,
}

/// Epistemically sampled historical curve (`UI-DAT-003`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeSeriesDto {
    pub series_key: String,
    pub label: String,
    pub unit: String,
    pub points: Vec<TimeSeriesPointDto>,
}

/// Mandate entry exposed across delegation tree & focus controls (`UI-DAT-004`, `UI-HUD-004`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MandateSummaryDto {
    pub mandate_id: u64,
    pub assignee_id: u32,
    pub target_settlement_id: u32,
    pub target_cell_id: u32,
    pub objective_kind: String,
    pub authority_tier: String,
    pub summary: String,
    pub priority_weight: u32,
    pub status: String,
    pub realized_efficiency: f32,
    pub progress_01: f32,
    pub subtasks: Vec<String>,
}

/// Subordinate node inside the delegation hierarchy.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubordinateSummaryDto {
    pub character_id: u32,
    pub display_name: String,
    pub role_title: String,
    pub competence: f32,
    #[serde(default)]
    pub home_settlement_id: u32,
    #[serde(default)]
    pub home_cell_id: u32,
    #[serde(default)]
    pub precision_01: f32,
    #[serde(default)]
    pub loyalty_label: String,
    #[serde(default)]
    pub allocated_attention: f32,
    #[serde(default)]
    pub assigned_mandates: Vec<MandateSummaryDto>,
}

/// Authority delegation overview including bounded sovereign attention budget (`UI-DAT-004`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegationTreeDto {
    pub leader_title: String,
    pub capacity_attention: f32,
    pub used_attention: f32,
    pub subordinates: Vec<SubordinateSummaryDto>,
    #[serde(default)]
    pub clerical_overhead: f32,
    #[serde(default)]
    pub overload_ratio: f32,
    #[serde(default)]
    pub focused_mandate_id: Option<u64>,
    #[serde(default)]
    pub focused_attention_share: f32,
    #[serde(default)]
    pub active_mandates: Vec<MandateSummaryDto>,
}

/// Tracked out-of-sight dead-reckoned or live-sighted convoy marker (`ENG-EPI-005`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackedConvoyDto {
    pub convoy_id: u64,
    pub label: String,
    pub status: String,
    pub projected_cell_id: u32,
    #[serde(default)]
    pub channel: String,
    #[serde(default)]
    pub origin_cell_id: u32,
    #[serde(default)]
    pub destination_cell_id: u32,
    #[serde(default)]
    pub route_cells: Vec<u32>,
    #[serde(default)]
    pub origin_unit_xyz: [f32; 3],
    #[serde(default)]
    pub current_unit_xyz: [f32; 3],
    #[serde(default)]
    pub destination_unit_xyz: [f32; 3],
    #[serde(default)]
    pub route_waypoints_xyz: Vec<[f32; 3]>,
    #[serde(default)]
    pub distance_travelled_km: f32,
    #[serde(default)]
    pub total_route_km: f32,
    #[serde(default)]
    pub departure_tick: u64,
    #[serde(default)]
    pub expected_arrival_tick: u64,
    #[serde(default)]
    pub confidence_01: f32,
    #[serde(default)]
    pub provenance_label: String,
}

/// Interactive entity reference token embedded inside dispatch narrative prose (`UI-DAT-005`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DispatchTokenDto {
    pub text: String,
    pub entity_kind: Option<String>,
    pub entity_id: Option<u64>,
    pub target_cell_id: Option<u32>,
}

/// Arrived physical dispatch entry with tokenized cross-links (`UI-DAT-005`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiegeticDispatchDto {
    pub dispatch_id: u64,
    pub title: String,
    pub sent_tick: u64,
    pub arrived_tick: u64,
    pub origin_label: String,
    #[serde(default)]
    pub sender_id: u32,
    #[serde(default)]
    pub sender_name: String,
    #[serde(default)]
    pub origin_cell_id: u32,
    #[serde(default)]
    pub event_class_key: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub confidence_01: f32,
    #[serde(default)]
    pub body_tokens: Vec<DispatchTokenDto>,
}

/// Selectable resolution option inside an `ArbitrationProposalDto` (`UI-HUD-003`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArbitrationOptionDto {
    pub index: u32,
    pub label: String,
    pub description: String,
    pub attention_cost: f32,
}

/// Subordinate conflict or bottleneck proposal awaiting player arbitration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArbitrationProposalDto {
    pub proposal_id: u64,
    pub settlement_id: u32,
    pub origin_cell_id: u32,
    pub raised_by_id: u32,
    pub raised_by_name: String,
    pub title: String,
    pub summary: String,
    pub created_tick: u64,
    pub deadline_tick: u64,
    pub options: Vec<ArbitrationOptionDto>,
}

/// Notification escalation policy item exposed in `NotificationMatrixWindow` (`UI-HUD-006`).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationRuleDto {
    pub event_class_key: String,
    pub display_name: String,
    pub behavior: String,
}

/// Verified inventory discrepancy exposed when an audit catches skimming or biased reporting (`ENG-EPI-004`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditDiscrepancyDto {
    pub settlement_id: u32,
    pub character_id: u32,
    pub character_name: String,
    pub commodity_key: String,
    pub claimed_mass_kg: f64,
    pub verified_mass_kg: f64,
    pub discrepancy_kg: f64,
    pub detected_tick: u64,
}

/// Selectable governance institutional model metadata (`ENG-CMD-006`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernanceModelOptionDto {
    pub model_key: String,
    pub display_name: String,
    pub base_attention_capacity: f32,
    pub clerical_overhead_per_mandate: f32,
    pub hierarchy_hop_latency_hours: u32,
}

/// Selectable economic or fiscal policy law metadata (`ENG-ECO-007`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicyLawOptionDto {
    pub policy_key: String,
    pub display_name: String,
    pub tax_or_tribute_rate_01: f32,
    pub patent_duration_years: u32,
}

/// Active governance institutional posture, transition friction, and fiscal parameters (`ENG-CMD-006`, `ENG-ECO-007`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GovernanceSnapshotDto {
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
    #[serde(default)]
    pub available_models: Vec<GovernanceModelOptionDto>,
    #[serde(default)]
    pub available_policies: Vec<PolicyLawOptionDto>,
}

/// Reported autonomous non-state economic actor summary (`ENG-ECO-005`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NonStateActorDto {
    pub actor_id: u32,
    pub name: String,
    pub kind: String,
    pub home_settlement_id: u32,
    pub settlement_name: String,
    pub working_capital_coins: f64,
    pub held_commodity_key: Option<String>,
    pub held_mass_kg: f64,
    pub hoarded_mass_kg: f64,
    pub private_carriers_owned: u32,
    pub chartered_to_state_carriers: u32,
    pub compliance_01: f32,
    pub observed_tick: u64,
    pub confidence_01: f32,
    pub provenance_label: String,
}

/// Local settlement price discovery and informal/black-market summary (`ENG-ECO-005`, `ENG-ECO-006`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InformalMarketDto {
    pub settlement_id: u32,
    pub settlement_name: String,
    pub commodity_key: String,
    pub commodity_name: String,
    pub clearing_price_index: f32,
    pub black_market_premium_01: f32,
    pub black_market_leakage_kg: f64,
    pub hoarded_reserve_kg: f64,
    pub observed_tick: u64,
    pub confidence_01: f32,
    pub provenance_label: String,
}

/// Shared infrastructure edge congestion readout (`ENG-LOG-005`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EdgeCongestionDto {
    pub edge_id: u32,
    pub from_cell_id: u32,
    pub to_cell_id: u32,
    pub hourly_flow_tonnes: f32,
    pub capacity_tonnes: f32,
    pub congestion_multiplier: f32,
}

/// Snapshot slice for non-state actors, informal markets, and shared freight congestion (`UI-MAP-006`, `UI-DAT-006`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NonStateEconomySnapshotDto {
    #[serde(default)]
    pub actors: Vec<NonStateActorDto>,
    #[serde(default)]
    pub informal_markets: Vec<InformalMarketDto>,
    #[serde(default)]
    pub congested_edges: Vec<EdgeCongestionDto>,
}

/// Empirical technical Know-How component score (`ENG-RES-001`, `ENG-RES-002`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeComponentScoreDto {
    pub component_id: u16,
    pub component_key: String,
    pub display_name: String,
    pub know_how_01: f32,
}

/// Theoretical inquiry Know-Why score (`ENG-RES-002`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TheoryDomainScoreDto {
    pub domain_id: u16,
    pub domain_key: String,
    pub display_name: String,
    pub know_why_01: f32,
}

/// Fragile physical carrier storing empirical or practical knowledge (`ENG-RES-004`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeCarrierDto {
    pub settlement_id: u32,
    pub settlement_name: String,
    pub component_key: String,
    pub carrier_kind: String,
    pub retention_strength_01: f32,
    pub holder_label: String,
}

/// Per-settlement emergence, patent, Know-How, and Know-Why status (`ENG-RES-001`..`005`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementKnowledgeDto {
    pub settlement_id: u32,
    pub settlement_name: String,
    pub innovation_mode_key: String,
    pub bottleneck_pressure_01: f32,
    pub leisure_surplus_01: f32,
    #[serde(default)]
    pub know_how: Vec<KnowledgeComponentScoreDto>,
    #[serde(default)]
    pub know_why: Vec<TheoryDomainScoreDto>,
    pub active_patent_holder_actor_id: Option<u32>,
    pub patent_expiry_tick: Option<u64>,
    pub regression_losses: u32,
    pub observed_tick: u64,
    pub confidence_01: f32,
    pub provenance_label: String,
}

/// Knowledge evolution and fragile archive snapshot (`UI-DAT-007`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeSnapshotDto {
    #[serde(default)]
    pub settlements: Vec<SettlementKnowledgeDto>,
    #[serde(default)]
    pub carriers: Vec<KnowledgeCarrierDto>,
    #[serde(default)]
    pub available_modes: Vec<String>,
}

/// Operational combat doctrine metadata (`ENG-WAR-002`, `ENG-WAR-003`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DoctrineOptionDto {
    pub doctrine_key: String,
    pub display_name: String,
    pub initiative_bonus: f32,
    pub strict_obedience_weight: f32,
    pub rain_penalty_mitigation: f32,
}

/// High-level theater command directive (`ENG-WAR-002`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TheaterDirectiveDto {
    pub theater_id: u32,
    pub commander_id: u32,
    pub commander_name: String,
    pub objective_cell_id: u32,
    pub supply_priority_01: f32,
    pub issued_tick: u64,
    pub autonomous_withdrawals: u32,
}

/// Deployed physical military formation in continuous spherical coordinates (`ENG-WAR-001`..`004`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MilitaryFormationDto {
    pub formation_id: u32,
    pub label: String,
    pub template_key: String,
    pub theater_id: u32,
    pub commander_id: u32,
    pub commander_name: String,
    pub home_settlement_id: u32,
    pub current_cell_id: u32,
    pub current_unit_xyz: [f32; 3],
    pub target_cell_id: u32,
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
    pub observed_tick: u64,
    pub confidence_01: f32,
    pub provenance_label: String,
}

/// Operational warfare and supply-line overlay snapshot (`UI-MAP-007`, `UI-HUD-007`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WarfareSnapshotDto {
    #[serde(default)]
    pub theaters: Vec<TheaterDirectiveDto>,
    #[serde(default)]
    pub formations: Vec<MilitaryFormationDto>,
    #[serde(default)]
    pub available_doctrines: Vec<DoctrineOptionDto>,
}

/// Mortal ruler and officeholder lifecycle readout (`ENG-POP-006`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuccessionCharacterDto {
    pub character_id: u32,
    pub name: String,
    pub role_title: String,
    pub age_years: f32,
    pub health_01: f32,
    pub is_alive: bool,
    pub is_ruler: bool,
    pub settlement_id: u32,
    pub settlement_name: String,
}

/// Active or completed succession transition briefing bound strictly to physical HQ archives (`ENG-CMD-007`, `UI-HUD-007`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuccessionTransitionDto {
    pub rule_key: String,
    pub rule_display_name: String,
    pub deceased_ruler_name: String,
    pub successor_name: String,
    pub transition_tick: u64,
    pub disruption_until_tick: u64,
    pub inherited_archive_entries: usize,
    pub lost_unrecorded_rapport_count: usize,
    pub awaiting_player_acknowledgement: bool,
}

/// Roster lifecycle and succession transition state (`UI-HUD-007`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuccessionSnapshotDto {
    #[serde(default)]
    pub roster: Vec<SuccessionCharacterDto>,
    #[serde(default)]
    pub pending_transition: Option<SuccessionTransitionDto>,
    pub effective_command_multiplier: f32,
}

impl Default for SuccessionSnapshotDto {
    fn default() -> Self {
        Self {
            roster: Vec::new(),
            pending_transition: None,
            effective_command_multiplier: 1.0,
        }
    }
}

/// Simulation clock and notification status DTO.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockAndNotificationsDto {
    pub tick: u64,
    pub calendar_text: String,
    pub paused: bool,
    pub speed_multiplier: u32,
    pub active_alert_count: u32,
    #[serde(default)]
    pub auto_pause_reason: Option<String>,
}

/// Sole authorized IPC snapshot crossing from Rust into Electron renderer (`ENG-EPI-001`, `UI-SHL-002`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiegeticSnapshot {
    pub schema_version: u32,
    pub scenario_key: String,
    pub scenario_name: String,
    pub globe_topology: GlobeTopologyDto,
    pub known_cells: Vec<KnownCellDto>,
    pub hq_presence: HqPresenceDto,
    #[serde(default)]
    pub known_settlements: Vec<KnownSettlementDto>,
    #[serde(default)]
    pub surveyed_roads: Vec<SurveyedRoadEdgeDto>,
    pub tracked_convoys: Vec<TrackedConvoyDto>,
    pub observed_weather: Vec<ObservedWeatherDto>,
    pub ledger_rows: Vec<ReportedLedgerRowDto>,
    pub delegation_tree: DelegationTreeDto,
    pub dispatch_inbox: Vec<DiegeticDispatchDto>,
    pub clock_and_notifications: ClockAndNotificationsDto,
    #[serde(default)]
    pub population_rows: Vec<PopulationCohortRowDto>,
    #[serde(default)]
    pub facility_rows: Vec<FacilityRowDto>,
    #[serde(default)]
    pub time_series: Vec<TimeSeriesDto>,
    #[serde(default)]
    pub open_proposals: Vec<ArbitrationProposalDto>,
    #[serde(default)]
    pub notification_rules: Vec<NotificationRuleDto>,
    #[serde(default)]
    pub audit_discrepancies: Vec<AuditDiscrepancyDto>,
    #[serde(default)]
    pub governance: GovernanceSnapshotDto,
    #[serde(default)]
    pub non_state_economy: NonStateEconomySnapshotDto,
    #[serde(default)]
    pub knowledge: KnowledgeSnapshotDto,
    #[serde(default)]
    pub warfare: WarfareSnapshotDto,
    #[serde(default)]
    pub succession: SuccessionSnapshotDto,
}

/// Validated player command envelope received across IPC from Electron (`UI-SHL-002`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ValidatedPlayerCommand {
    SetPaused {
        paused: bool,
    },
    StepSingleTick,
    SetSpeedMultiplier {
        multiplier: u32,
    },
    PingBridge {
        tag: String,
    },
    IssueMandate {
        assignee: u32,
        #[serde(rename = "objectiveKind")]
        objective_kind: String,
        #[serde(rename = "targetCell")]
        target_cell: u32,
        #[serde(rename = "priorityWeight")]
        priority_weight: u32,
        #[serde(rename = "authorityTier")]
        authority_tier: String,
    },
    ResolveArbitrationProposal {
        #[serde(rename = "proposalId")]
        proposal_id: u64,
        #[serde(rename = "chosenOptionIndex")]
        chosen_option_index: u32,
    },
    SetPersonalAttentionZoom {
        #[serde(rename = "targetMandateId")]
        target_mandate_id: Option<u64>,
        #[serde(rename = "share01")]
        share_01: f32,
    },
    RelocateHq {
        #[serde(rename = "destinationCell")]
        destination_cell: u32,
    },
    UpdateNotificationRule {
        #[serde(rename = "eventClassKey")]
        event_class_key: String,
        behavior: String,
    },
    SwitchGovernanceModel {
        #[serde(rename = "modelKey")]
        model_key: String,
    },
    EnactEconomicPolicy {
        #[serde(rename = "policyKey")]
        policy_key: String,
        #[serde(default, rename = "rateOverride01")]
        rate_override_01: Option<f32>,
    },
    CharterPrivateCarriers {
        #[serde(rename = "targetSettlement")]
        target_settlement: u32,
        count: u32,
    },
    SetInnovationMode {
        settlement: u32,
        #[serde(rename = "modeKey")]
        mode_key: String,
    },
    IssueTheaterDirective {
        #[serde(rename = "theaterId")]
        theater_id: u32,
        commander: u32,
        #[serde(rename = "objectiveCell")]
        objective_cell: u32,
        #[serde(rename = "supplyPriority01")]
        supply_priority_01: f32,
    },
    IssueOperationalBattlePlan {
        #[serde(rename = "formationId")]
        formation_id: u32,
        #[serde(rename = "targetCell")]
        target_cell: u32,
        #[serde(rename = "doctrineKey")]
        doctrine_key: String,
        #[serde(default, rename = "noRetreat")]
        no_retreat: bool,
    },
    AcknowledgeSuccession,
    IssueTacticalMicroCommand {
        #[serde(default, rename = "unitId")]
        unit_id: u64,
    },
    BuyOrSelectTechTarget {
        #[serde(default, rename = "techKey")]
        tech_key: String,
    },
    ConstructDam {
        #[serde(rename = "targetCell")]
        target_cell: u32,
        #[serde(default, rename = "ruleCurve")]
        rule_curve: String,
    },
    SetDamRuleCurve {
        #[serde(rename = "targetCell")]
        target_cell: u32,
        #[serde(rename = "ruleCurve")]
        rule_curve: String,
    },
}

impl ValidatedPlayerCommand {
    /// Converts validated IPC command into strong-typed `SpinePlayerCommand`.
    pub fn to_spine_command(&self) -> Result<SpinePlayerCommand, String> {
        match self {
            ValidatedPlayerCommand::SetPaused { paused } => {
                Ok(SpinePlayerCommand::SetPaused(*paused))
            }
            ValidatedPlayerCommand::StepSingleTick => Ok(SpinePlayerCommand::StepSingleTick),
            ValidatedPlayerCommand::SetSpeedMultiplier { multiplier } => {
                Ok(SpinePlayerCommand::SetSpeedMultiplier(*multiplier))
            }
            ValidatedPlayerCommand::PingBridge { tag } => Ok(SpinePlayerCommand::StubPing {
                tag: tag.clone(),
            }),
            ValidatedPlayerCommand::IssueMandate {
                assignee,
                objective_kind,
                target_cell,
                priority_weight,
                authority_tier,
            } => Ok(SpinePlayerCommand::IssueMandate {
                assignee: CharacterId(*assignee),
                objective_kind: objective_kind.clone(),
                target_cell: CellId(*target_cell),
                priority_weight: (*priority_weight).clamp(1, 10),
                authority_tier: authority_tier.clone(),
            }),
            ValidatedPlayerCommand::ResolveArbitrationProposal {
                proposal_id,
                chosen_option_index,
            } => Ok(SpinePlayerCommand::ResolveArbitrationProposal {
                proposal_id: *proposal_id,
                chosen_option_index: *chosen_option_index,
            }),
            ValidatedPlayerCommand::SetPersonalAttentionZoom {
                target_mandate_id,
                share_01,
            } => Ok(SpinePlayerCommand::SetPersonalAttentionZoom {
                target_mandate_id: target_mandate_id.map(MandateId),
                share_01: share_01.clamp(0.0, 0.85),
            }),
            ValidatedPlayerCommand::RelocateHq { destination_cell } => {
                Ok(SpinePlayerCommand::RelocateHq {
                    destination_cell: CellId(*destination_cell),
                })
            }
            ValidatedPlayerCommand::UpdateNotificationRule {
                event_class_key,
                behavior,
            } => {
                let parsed = match behavior.as_str() {
                    "LogOnly" | "log_only" => NotificationBehaviorKind::LogOnly,
                    "BannerAlert" | "banner_alert" => NotificationBehaviorKind::BannerAlert,
                    "DispatchPopup" | "dispatch_popup" => NotificationBehaviorKind::DispatchPopup,
                    "DispatchPopupAndAutoPause" | "dispatch_popup_and_auto_pause" => {
                        NotificationBehaviorKind::DispatchPopupAndAutoPause
                    }
                    other => return Err(format!("Unsupported notification behavior '{other}'")),
                };
                Ok(SpinePlayerCommand::UpdateNotificationRule {
                    event_class_key: event_class_key.clone(),
                    behavior: parsed,
                })
            }
            ValidatedPlayerCommand::SwitchGovernanceModel { model_key } => {
                Ok(SpinePlayerCommand::SwitchGovernanceModel {
                    model_key: model_key.clone(),
                })
            }
            ValidatedPlayerCommand::EnactEconomicPolicy {
                policy_key,
                rate_override_01,
            } => Ok(SpinePlayerCommand::EnactEconomicPolicy {
                policy_key: policy_key.clone(),
                rate_override_01: rate_override_01.map(|v| v.clamp(0.0, 1.0)),
            }),
            ValidatedPlayerCommand::CharterPrivateCarriers {
                target_settlement,
                count,
            } => Ok(SpinePlayerCommand::CharterPrivateCarriers {
                target_settlement: SettlementId(*target_settlement),
                count: (*count).min(1000),
            }),
            ValidatedPlayerCommand::SetInnovationMode {
                settlement,
                mode_key,
            } => Ok(SpinePlayerCommand::SetInnovationMode {
                settlement: SettlementId(*settlement),
                mode_key: mode_key.clone(),
            }),
            ValidatedPlayerCommand::IssueTheaterDirective {
                theater_id,
                commander,
                objective_cell,
                supply_priority_01,
            } => Ok(SpinePlayerCommand::IssueTheaterDirective {
                theater_id: TheaterId((*theater_id).min(u32::from(u16::MAX)) as u16),
                commander: CharacterId(*commander),
                objective_cell: CellId(*objective_cell),
                supply_priority_01: supply_priority_01.clamp(0.0, 1.0),
            }),
            ValidatedPlayerCommand::IssueOperationalBattlePlan {
                formation_id,
                target_cell,
                doctrine_key,
                no_retreat,
            } => Ok(SpinePlayerCommand::IssueOperationalBattlePlan {
                formation_id: FormationId(*formation_id),
                target_cell: CellId(*target_cell),
                doctrine_key: doctrine_key.clone(),
                no_retreat: *no_retreat,
            }),
            ValidatedPlayerCommand::AcknowledgeSuccession => {
                Ok(SpinePlayerCommand::AcknowledgeSuccession)
            }
            ValidatedPlayerCommand::IssueTacticalMicroCommand { unit_id } => Err(format!(
                "ERR_TACTICAL_MICRO_FORBIDDEN: Individual tactical micro for unit #{unit_id} is forbidden; issue operational directives or battle plans."
            )),
            ValidatedPlayerCommand::BuyOrSelectTechTarget { tech_key } => Err(format!(
                "ERR_BOUGHT_TECH_FORBIDDEN: Technology '{tech_key}' cannot be bought or picked from a deterministic tree; foster empirical production or leisure conditions."
            )),
            ValidatedPlayerCommand::ConstructDam {
                target_cell,
                rule_curve,
            } => {
                let parsed = if rule_curve.trim().is_empty() {
                    DamRuleCurveKind::BalancedPower
                } else {
                    DamRuleCurveKind::from_key(rule_curve).ok_or_else(|| {
                        format!("Unsupported dam rule curve '{rule_curve}'")
                    })?
                };
                Ok(SpinePlayerCommand::ConstructDam {
                    target_cell: CellId(*target_cell),
                    rule_curve: parsed,
                })
            }
            ValidatedPlayerCommand::SetDamRuleCurve {
                target_cell,
                rule_curve,
            } => {
                let parsed = DamRuleCurveKind::from_key(rule_curve).ok_or_else(|| {
                    format!("Unsupported dam rule curve '{rule_curve}'")
                })?;
                Ok(SpinePlayerCommand::SetDamRuleCurve {
                    target_cell: CellId(*target_cell),
                    rule_curve: parsed,
                })
            }
        }
    }
}

/// Typed IPC acknowledgement returned by backend command handlers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandAck {
    pub accepted: bool,
    pub tick: u64,
    pub error_code: Option<String>,
    pub message: String,
}

/// Edge road metadata passed from `ScenarioWorld` into `player-view` without exposing logistics internals.
#[derive(Clone, Debug)]
pub struct EdgeViewInput {
    pub edge_id: u32,
    pub from_cell_id: u32,
    pub to_cell_id: u32,
    pub infra_key: String,
    pub washed_out: bool,
    pub river_edge: bool,
    pub bridge_cap_kg: Option<f64>,
    pub road_built: bool,
    pub road_purpose: String,
    pub purpose_label: String,
    pub cumulative_passages: u32,
    pub cumulative_tonnes_passed: f64,
}

/// Auxiliary M5-M7 optional plug trait references supplied through `sim-spine` ports (`ENG-EPI-001`).
#[derive(Clone, Copy, Default)]
pub struct M5ToM7PlugRefs<'a> {
    pub non_state_eco: Option<&'a dyn NonStateEconomyPlugPort>,
    pub governance: Option<&'a dyn GovernanceFrictionPlugPort>,
    pub knowledge: Option<&'a dyn KnowledgeModifierPlugPort>,
    pub warfare: Option<&'a dyn OperationalCombatPlugPort>,
    pub succession: Option<&'a dyn SuccessionPlugPort>,
}

/// Auxiliary subsystem view inputs supplied by composition root (`sim-scenario`) when compiling snapshots.
#[derive(Clone, Debug, Default)]
pub struct SnapshotSubsystemInputs {
    pub world_seed: u64,
    pub clerical_overhead: f32,
    pub overload_ratio: f32,
    pub focused_mandate_id: Option<u64>,
    pub focused_attention_share: f32,
    pub mandates: Vec<MandateSummaryDto>,
    pub open_proposals: Vec<ArbitrationProposalDto>,
    pub notification_rules: Vec<NotificationRuleDto>,
    pub facility_rows: Vec<FacilityRowDto>,
    pub road_edges: BTreeMap<u32, EdgeViewInput>,
    pub unrest_by_settlement: BTreeMap<u32, String>,
    pub relocation_previews: Vec<HqRelocationPreviewDto>,
}

fn splitmix64_epistemic(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn signed_noise_05(seed: u64) -> f64 {
    let unit = ((splitmix64_epistemic(seed) >> 40) as f64) / 16_777_216.0;
    unit - 0.5
}

/// Computes probabilistic geological confidence intervals narrowing with surveyor skill & instrument tier (`ENG-EPI-006`).
#[must_use]
pub fn estimate_deposit_interval(
    seed: u64,
    tick: u64,
    material_id: u16,
    true_mass_tonnes: f64,
    true_purity_01: f32,
    surveyor_skill_01: f32,
    instrument_tier: u8,
) -> (f64, f64, f64, f32, f32) {
    let eff = (0.20
        + 0.55 * f64::from(surveyor_skill_01.clamp(0.0, 1.0))
        + 0.14 * f64::from(instrument_tier.min(3)))
    .clamp(0.20, 1.35);
    let rel_half = (0.58 / eff).clamp(0.06, 1.85);
    let h = splitmix64_epistemic(
        seed ^ tick.wrapping_mul(0x9E37_79B9) ^ u64::from(material_id).wrapping_mul(0x517C_C1B7),
    );
    let noise = signed_noise_05(h);
    let point_est = (true_mass_tonnes * (1.0 + noise * 0.45 * rel_half)).max(1.0);
    let min_tonnes = (point_est * (1.0 - rel_half)).max(0.0);
    let max_tonnes = point_est * (1.0 + rel_half);
    let purity_est = (true_purity_01 + (noise as f32) * 0.12 * (rel_half as f32)).clamp(0.05, 0.99);
    let confidence_01 = (1.0 / (1.0 + (rel_half as f32))).clamp(0.15, 0.96);
    (min_tonnes, point_est, max_tonnes, purity_est, confidence_01)
}

/// Perturbs reported physical inventory values according to subordinate honesty, caution, precision, and skimming (`ENG-EPI-004`).
#[must_use]
pub fn perturb_reported_quantity(
    seed: u64,
    tick: u64,
    settlement_id: u32,
    commodity_id: u16,
    true_mass_kg: f64,
    reporter: &CharacterSnapshotView,
    skimming_01: f32,
) -> (f64, f64, f64, String) {
    let h = splitmix64_epistemic(
        seed ^ tick
            ^ (u64::from(settlement_id) << 16)
            ^ u64::from(commodity_id)
            ^ u64::from(reporter.id.0).wrapping_mul(0x94D0_49BB_1331_11EB),
    );
    let noise = signed_noise_05(h);
    let precision = reporter.precision_01.clamp(0.05, 0.98);
    let half_band = ((1.05 - precision) * 0.48).clamp(0.02, 0.60);

    // Negative `honesty_vs_sycophancy` means sycophant: inflates positive reports to please HQ.
    let sycophancy_bias = if reporter.honesty_vs_sycophancy < 0.0 {
        f64::from((-reporter.honesty_vs_sycophancy).clamp(0.0, 1.0)) * 0.24
    } else {
        0.0
    };
    // Negative `caution_vs_alarmism` means alarmist: pulls conservative estimate downward when tight.
    let caution_bias = if reporter.caution_vs_alarmism < 0.0 {
        -f64::from((-reporter.caution_vs_alarmism).clamp(0.0, 1.0)) * 0.18
    } else {
        -f64::from(reporter.caution_vs_alarmism.clamp(0.0, 1.0)) * 0.05
    };
    // Skimming masks diverted quantity inside inflated paperwork until audited.
    let skim_conceal = f64::from(skimming_01.clamp(0.0, 0.8)) * 0.16;

    let bias = 1.0 + sycophancy_bias + caution_bias + skim_conceal + noise * f64::from(half_band) * 0.5;
    let reported = (true_mass_kg * bias.max(0.25)).max(0.0);
    let min_est = (reported * (1.0 - f64::from(half_band))).max(0.0);
    let max_est = reported * (1.0 + f64::from(half_band));
    let provenance = format!(
        "Dispatched by {} ({})",
        reporter.display_name, reporter.role_title
    );
    (reported, min_est, max_est, provenance)
}

/// Maintains the player's persistent epistemic archive and compiles air-gapped IPC snapshots.
#[derive(Default, Debug)]
pub struct PlayerKnowledgeArchive {
    surveyed_cells: BTreeMap<u32, KnownCellDto>,
    archived_dispatches: Vec<DiegeticDispatchDto>,
    remote_ledger_reports: BTreeMap<(u32, String), ReportedLedgerRowDto>,
    remote_pop_reports: BTreeMap<u32, PopulationCohortRowDto>,
    remote_weather_reports: BTreeMap<u32, ObservedWeatherDto>,
    time_series_history: BTreeMap<String, Vec<TimeSeriesPointDto>>,
    audit_discrepancies: Vec<AuditDiscrepancyDto>,
    next_dispatch_seq: u64,
}

impl PlayerKnowledgeArchive {
    /// Creates an empty cartographic knowledge archive prior to initial HQ observation.
    #[must_use]
    pub fn new() -> Self {
        Self {
            surveyed_cells: BTreeMap::new(),
            archived_dispatches: Vec::new(),
            remote_ledger_reports: BTreeMap::new(),
            remote_pop_reports: BTreeMap::new(),
            remote_weather_reports: BTreeMap::new(),
            time_series_history: BTreeMap::new(),
            audit_discrepancies: Vec::new(),
            next_dispatch_seq: 1,
        }
    }

    /// Formats a tick index cleanly according to scenario calendar rules.
    #[must_use]
    pub fn format_calendar_tick(registry: &dyn ModRegistryPort, tick: Tick) -> String {
        let cal = registry.calendar_config();
        let hours_per_day = u64::from(cal.hours_per_day.max(1));
        let days_per_year = u64::from(cal.days_per_year.max(1));
        let total_days = tick.get() / hours_per_day;
        let hour = tick.get() % hours_per_day;
        let year_offset = (total_days / days_per_year) as i32;
        let mut day_of_year = (total_days % days_per_year) as u32;

        let mut month_name = "Cycle";
        for (name, len) in cal.month_names.iter().zip(cal.month_lengths.iter()) {
            if day_of_year < *len {
                month_name = name.as_str();
                break;
            }
            day_of_year -= *len;
        }
        format!(
            "{} {}, {} {:02} ({:02}:00)",
            cal.year_prefix,
            cal.starting_year + year_offset,
            month_name,
            day_of_year + 1,
            hour
        )
    }

    /// Records a unverified traveler/scout rumor on a currently unexplored cell (`ENG-EPI-002`).
    pub fn ingest_rumor(
        &mut self,
        cell_id: u32,
        tick: Tick,
        elevation_m: f32,
        biome_key: String,
        is_ocean: bool,
        provenance_label: String,
    ) {
        if matches!(
            self.surveyed_cells.get(&cell_id),
            Some(KnownCellDto::Surveyed { .. } | KnownCellDto::Stale { .. })
        ) {
            return;
        }
        self.surveyed_cells.insert(
            cell_id,
            KnownCellDto::Rumored {
                cell_id,
                observed_tick: tick.get(),
                elevation_m,
                confidence_01: 0.32,
                provenance_label,
                biome_key,
                is_ocean,
                watershed_id: 0,
                region_label: String::new(),
            },
        );
    }

    /// Records a courier-delivered geological & terrain survey report for a remote cell (`ENG-EPI-002`, `ENG-EPI-006`).
    pub fn ingest_survey_report(
        &mut self,
        cell_id: u32,
        tick: Tick,
        planet: &dyn PlanetGeometryPort,
        deposit_estimates: Vec<DepositEstimateDto>,
        provenance_label: String,
    ) {
        let cid = CellId(cell_id);
        let elev = planet.cell_elevation_m(cid);
        let geo = planet.cell_biome_and_strata(cid);
        let watershed_id = planet.watershed_id(cid).0;
        let region_label = planet.region_label(cid);
        let river_downstream_cell_id = planet.watershed_downstream(cid).map(CellId::get);
        let river_flow_norm_01 = if geo.is_ocean {
            0.0
        } else {
            (planet.cell_drainage_area_km2(cid) / 2500.0).clamp(0.0, 1.0)
        };
        let conf = deposit_estimates
            .first()
            .map(|d| d.confidence_01)
            .unwrap_or(0.86);
        self.surveyed_cells.insert(
            cell_id,
            KnownCellDto::Surveyed {
                cell_id,
                observed_tick: tick.get(),
                is_live_hq_sight: false,
                elevation_m: elev,
                confidence_01: conf,
                provenance_label,
                biome_key: geo.biome_key,
                is_ocean: geo.is_ocean,
                river_downstream_cell_id,
                river_flow_norm_01,
                deposit_estimates,
                watershed_id,
                region_label,
                mean_annual_temp_c: planet.cell_mean_annual_temp_c(cid),
                annual_precip_mm: planet.cell_base_precip_mm_yr(cid),
                aridity_index_01: planet.cell_aridity_index_01(cid),
                soil_fertility_01: planet.cell_soil_fertility_01(cid),
                rock_strata_code: planet.cell_strata_code(cid),
                drainage_area_km2: planet.cell_drainage_area_km2(cid),
                strahler_order: planet.cell_strahler_order(cid),
                is_lake: planet.cell_is_lake(cid),
                river_channel_downstream_cell_id: planet
                    .cell_river_channel_downstream(cid)
                    .map(CellId::get),
                river_discharge_m3_s: planet.cell_river_discharge_m3_s(cid),
                river_velocity_mps: planet.cell_river_velocity_mps(cid),
                hydro_potential_kw: planet.cell_hydro_potential_kw(cid),
                geology_province: planet.cell_geology_province(cid).as_str().to_owned(),
                roughness_m: planet.cell_roughness_m(cid),
                basin_depth_m: planet.cell_basin_depth_m(cid),
                fault_instability_01: planet.cell_fault_instability_01(cid),
                volcanic_hazard_01: planet.cell_volcanic_hazard_01(cid),
                soil_drainage_01: planet.cell_soil_drainage_01(cid),
                soil_clay_01: planet.cell_soil_clay_01(cid),
                marine_upwelling_01: planet.cell_marine_upwelling_01(cid),
                shelf_nutrient_01: planet.cell_shelf_nutrient_01(cid),
                lake_kind: planet.cell_lake_kind(cid).as_str().to_owned(),
                lake_salinity_per_mille: planet.cell_lake_salinity_per_mille(cid),
                is_estuary: planet.cell_is_estuary(cid),
                river_navigability: planet.cell_river_navigability(cid).as_str().to_owned(),
                is_spring_oasis: false,
                groundwater_m3: 0.0,
                dam_status: None,
            },
        );
    }

    /// Pre-seeds historical surveyed knowledge marked `Stale` (used by `GovernorSuccessor` archetype).
    pub fn seed_historical_stale_cell(
        &mut self,
        cell_id: u32,
        tick: Tick,
        planet: &dyn PlanetGeometryPort,
        provenance_label: String,
    ) {
        let cid = CellId(cell_id);
        let elev = planet.cell_elevation_m(cid);
        let geo = planet.cell_biome_and_strata(cid);
        let watershed_id = planet.watershed_id(cid).0;
        let region_label = planet.region_label(cid);
        let river_downstream_cell_id = planet.watershed_downstream(cid).map(CellId::get);
        let river_flow_norm_01 = if geo.is_ocean {
            0.0
        } else {
            (planet.cell_drainage_area_km2(cid) / 2500.0).clamp(0.0, 1.0)
        };
        self.surveyed_cells.insert(
            cell_id,
            KnownCellDto::Stale {
                cell_id,
                observed_tick: tick.get(),
                last_live_tick: 0,
                elevation_m: elev,
                confidence_01: 0.62,
                provenance_label,
                biome_key: geo.biome_key,
                is_ocean: geo.is_ocean,
                river_downstream_cell_id,
                river_flow_norm_01,
                deposit_estimates: Vec::new(),
                watershed_id,
                region_label,
                mean_annual_temp_c: planet.cell_mean_annual_temp_c(cid),
                annual_precip_mm: planet.cell_base_precip_mm_yr(cid),
                aridity_index_01: planet.cell_aridity_index_01(cid),
                soil_fertility_01: planet.cell_soil_fertility_01(cid),
                rock_strata_code: planet.cell_strata_code(cid),
                drainage_area_km2: planet.cell_drainage_area_km2(cid),
                strahler_order: planet.cell_strahler_order(cid),
                is_lake: planet.cell_is_lake(cid),
                river_channel_downstream_cell_id: planet
                    .cell_river_channel_downstream(cid)
                    .map(CellId::get),
                river_discharge_m3_s: planet.cell_river_discharge_m3_s(cid),
                river_velocity_mps: planet.cell_river_velocity_mps(cid),
                hydro_potential_kw: planet.cell_hydro_potential_kw(cid),
                geology_province: planet.cell_geology_province(cid).as_str().to_owned(),
                roughness_m: planet.cell_roughness_m(cid),
                basin_depth_m: planet.cell_basin_depth_m(cid),
                fault_instability_01: planet.cell_fault_instability_01(cid),
                volcanic_hazard_01: planet.cell_volcanic_hazard_01(cid),
                soil_drainage_01: planet.cell_soil_drainage_01(cid),
                soil_clay_01: planet.cell_soil_clay_01(cid),
                marine_upwelling_01: planet.cell_marine_upwelling_01(cid),
                shelf_nutrient_01: planet.cell_shelf_nutrient_01(cid),
                lake_kind: planet.cell_lake_kind(cid).as_str().to_owned(),
                lake_salinity_per_mille: planet.cell_lake_salinity_per_mille(cid),
                is_estuary: planet.cell_is_estuary(cid),
                river_navigability: planet.cell_river_navigability(cid).as_str().to_owned(),
                is_spring_oasis: false,
                groundwater_m3: 0.0,
                dam_status: None,
            },
        );
    }

    /// Appends an arrived courier dispatch envelope into the persistent player archive inbox (`UI-DAT-005`).
    pub fn record_arrived_dispatch(&mut self, mut dispatch: DiegeticDispatchDto) -> u64 {
        if dispatch.dispatch_id == 0 {
            dispatch.dispatch_id = self.next_dispatch_seq;
        }
        self.next_dispatch_seq = self.next_dispatch_seq.max(dispatch.dispatch_id + 1);
        self.archived_dispatches.push(dispatch);
        if self.archived_dispatches.len() > 250 {
            let drain_n = self.archived_dispatches.len() - 250;
            self.archived_dispatches.drain(0..drain_n);
        }
        self.next_dispatch_seq - 1
    }

    /// Stores a courier-delivered ledger row inside archive and returns any prior claimed discrepancy on live audit.
    pub fn record_remote_ledger_report(&mut self, row: ReportedLedgerRowDto) {
        self.remote_ledger_reports
            .insert((row.settlement_id, row.commodity_key.clone()), row);
    }

    /// Stores a courier-delivered weather observation.
    pub fn record_remote_weather_report(&mut self, obs: ObservedWeatherDto) {
        self.remote_weather_reports.insert(obs.cell_id, obs);
    }

    /// Stores a courier-delivered cohort snapshot row.
    pub fn record_remote_population_report(&mut self, row: PopulationCohortRowDto) {
        self.remote_pop_reports.insert(row.cohort_id, row);
    }

    /// Appends a daily epistemic sample point into a named historical time-series buffer (`UI-DAT-003`).
    pub fn record_time_series_sample(
        &mut self,
        series_key: &str,
        point: TimeSeriesPointDto,
    ) {
        let vec = self
            .time_series_history
            .entry(series_key.to_owned())
            .or_default();
        vec.push(point);
        if vec.len() > 730 {
            vec.remove(0);
        }
    }

    /// Audits previously claimed remote subordinate report against directly observed settlement mass (`ENG-EPI-004`).
    pub fn audit_settlement_against_direct_sight(
        &mut self,
        tick: Tick,
        settlement_id: u32,
        reporter_id: u32,
        reporter_name: &str,
        commodity_key: &str,
        verified_mass_kg: f64,
    ) {
        if let Some(prev) = self
            .remote_ledger_reports
            .get(&(settlement_id, commodity_key.to_owned()))
        {
            let diff = prev.reported_mass_kg - verified_mass_kg;
            if diff.abs() > 25.0 && prev.observed_tick < tick.get() {
                let already = self.audit_discrepancies.iter().any(|a| {
                    a.settlement_id == settlement_id
                        && a.commodity_key == commodity_key
                        && a.detected_tick == tick.get()
                });
                if !already {
                    self.audit_discrepancies.push(AuditDiscrepancyDto {
                        settlement_id,
                        character_id: reporter_id,
                        character_name: reporter_name.to_owned(),
                        commodity_key: commodity_key.to_owned(),
                        claimed_mass_kg: prev.reported_mass_kg,
                        verified_mass_kg,
                        discrepancy_kg: diff,
                        detected_tick: tick.get(),
                    });
                }
            }
        }
    }

    /// Refreshes the player's local survey archive within the HQ optical sight radius (`ENG-EPI-002`).
    /// Transitions previously `Surveyed` cells outside sight for `>= 12h` to decaying `Stale`.
    pub fn observe_hq_halo(
        &mut self,
        tick: Tick,
        planet: &dyn PlanetGeometryPort,
        ledger: &dyn PhysicalLedgerPort,
    ) -> BTreeSet<u32> {
        let hq_cell = ledger.hq_cell();
        let sight_radius = ledger.hq_sight_radius_cells();

        let mut live_cells = BTreeSet::new();
        live_cells.insert(hq_cell.get());
        if sight_radius >= 1 {
            let direct_ring_max_km = planet
                .cell_neighbors(hq_cell)
                .iter()
                .map(|&nb| planet.great_circle_km(hq_cell, nb))
                .fold(0.0_f32, f32::max);
            let max_radius_km = direct_ring_max_km * (f32::from(sight_radius) + 0.12);
            let mut frontier = VecDeque::new();
            frontier.push_back((hq_cell, 0u8));
            while let Some((cid, dist)) = frontier.pop_front() {
                if dist < sight_radius {
                    for &nbr in planet.cell_neighbors(cid) {
                        if (dist == 0 || planet.great_circle_km(hq_cell, nbr) <= max_radius_km)
                            && live_cells.insert(nbr.get())
                        {
                            frontier.push_back((nbr, dist + 1));
                        }
                    }
                }
            }
        }

        let keys: Vec<u32> = self.surveyed_cells.keys().copied().collect();
        for cid_u32 in keys {
            if live_cells.contains(&cid_u32) {
                continue;
            }
            let Some(existing) = self.surveyed_cells.get(&cid_u32).cloned() else {
                continue;
            };
            match existing {
                KnownCellDto::Surveyed {
                    cell_id,
                    observed_tick,
                    elevation_m,
                    biome_key,
                    is_ocean,
                    river_downstream_cell_id,
                    river_flow_norm_01,
                    deposit_estimates,
                    watershed_id,
                    region_label,
                    mean_annual_temp_c,
                    annual_precip_mm,
                    aridity_index_01,
                    soil_fertility_01,
                    rock_strata_code,
                    drainage_area_km2,
                    strahler_order,
                    is_lake,
                    river_channel_downstream_cell_id,
                    river_discharge_m3_s,
                    river_velocity_mps,
                    hydro_potential_kw,
                    geology_province,
                    roughness_m,
                    basin_depth_m,
                    fault_instability_01,
                    volcanic_hazard_01,
                    soil_drainage_01,
                    soil_clay_01,
                    marine_upwelling_01,
                    shelf_nutrient_01,
                    lake_kind,
                    lake_salinity_per_mille,
                    is_estuary,
                    river_navigability,
                    is_spring_oasis,
                    groundwater_m3,
                    dam_status,
                    ..
                } => {
                    let age = tick.get().saturating_sub(observed_tick);
                    if age >= 12 {
                        let conf = (1.0 / (1.0 + (age as f32) / 168.0)).clamp(0.15, 0.92);
                        self.surveyed_cells.insert(
                            cid_u32,
                            KnownCellDto::Stale {
                                cell_id,
                                observed_tick,
                                last_live_tick: observed_tick,
                                elevation_m,
                                confidence_01: conf,
                                provenance_label: format!("Stale Survey ({age}h old)"),
                                biome_key,
                                is_ocean,
                                river_downstream_cell_id,
                                river_flow_norm_01,
                                deposit_estimates,
                                watershed_id,
                                region_label,
                                mean_annual_temp_c,
                                annual_precip_mm,
                                aridity_index_01,
                                soil_fertility_01,
                                rock_strata_code,
                                drainage_area_km2,
                                strahler_order,
                                is_lake,
                                river_channel_downstream_cell_id,
                                river_discharge_m3_s,
                                river_velocity_mps,
                                hydro_potential_kw,
                                geology_province,
                                roughness_m,
                                basin_depth_m,
                                fault_instability_01,
                                volcanic_hazard_01,
                                soil_drainage_01,
                                soil_clay_01,
                                marine_upwelling_01,
                                shelf_nutrient_01,
                                lake_kind,
                                lake_salinity_per_mille,
                                is_estuary,
                                river_navigability,
                                is_spring_oasis,
                                groundwater_m3,
                                dam_status,
                            },
                        );
                    } else if let Some(KnownCellDto::Surveyed {
                        is_live_hq_sight,
                        ..
                    }) = self.surveyed_cells.get_mut(&cid_u32)
                    {
                        *is_live_hq_sight = false;
                    }
                }
                KnownCellDto::Stale {
                    cell_id,
                    observed_tick,
                    last_live_tick,
                    elevation_m,
                    biome_key,
                    is_ocean,
                    river_downstream_cell_id,
                    river_flow_norm_01,
                    deposit_estimates,
                    watershed_id,
                    region_label,
                    mean_annual_temp_c,
                    annual_precip_mm,
                    aridity_index_01,
                    soil_fertility_01,
                    rock_strata_code,
                    drainage_area_km2,
                    strahler_order,
                    is_lake,
                    river_channel_downstream_cell_id,
                    river_discharge_m3_s,
                    river_velocity_mps,
                    hydro_potential_kw,
                    geology_province,
                    roughness_m,
                    basin_depth_m,
                    fault_instability_01,
                    volcanic_hazard_01,
                    soil_drainage_01,
                    soil_clay_01,
                    marine_upwelling_01,
                    shelf_nutrient_01,
                    lake_kind,
                    lake_salinity_per_mille,
                    is_estuary,
                    river_navigability,
                    is_spring_oasis,
                    groundwater_m3,
                    dam_status,
                    ..
                } => {
                    let age = tick.get().saturating_sub(observed_tick);
                    let conf = (1.0 / (1.0 + (age as f32) / 168.0)).clamp(0.15, 0.92);
                    self.surveyed_cells.insert(
                        cid_u32,
                        KnownCellDto::Stale {
                            cell_id,
                            observed_tick,
                            last_live_tick,
                            elevation_m,
                            confidence_01: conf,
                            provenance_label: format!("Stale Survey ({age}h old)"),
                            biome_key,
                            is_ocean,
                            river_downstream_cell_id,
                            river_flow_norm_01,
                            deposit_estimates,
                            watershed_id,
                            region_label,
                            mean_annual_temp_c,
                            annual_precip_mm,
                            aridity_index_01,
                            soil_fertility_01,
                            rock_strata_code,
                            drainage_area_km2,
                            strahler_order,
                            is_lake,
                            river_channel_downstream_cell_id,
                            river_discharge_m3_s,
                            river_velocity_mps,
                            hydro_potential_kw,
                            geology_province,
                            roughness_m,
                            basin_depth_m,
                            fault_instability_01,
                            volcanic_hazard_01,
                            soil_drainage_01,
                            soil_clay_01,
                            marine_upwelling_01,
                            shelf_nutrient_01,
                            lake_kind,
                            lake_salinity_per_mille,
                            is_estuary,
                            river_navigability,
                            is_spring_oasis,
                            groundwater_m3,
                            dam_status,
                        },
                    );
                }
                KnownCellDto::Unexplored { .. } | KnownCellDto::Rumored { .. } => {}
            }
        }

        for &cid_u32 in &live_cells {
            if let Some(KnownCellDto::Surveyed {
                observed_tick,
                is_live_hq_sight,
                confidence_01,
                provenance_label,
                ..
            }) = self.surveyed_cells.get_mut(&cid_u32)
            {
                *observed_tick = tick.get();
                *is_live_hq_sight = true;
                *confidence_01 = 1.0;
                if provenance_label != "Direct Observation (HQ)" {
                    provenance_label.clear();
                    provenance_label.push_str("Direct Observation (HQ)");
                }
                continue;
            }
            let prev_estimates = self
                .surveyed_cells
                .get(&cid_u32)
                .map(|c| c.deposit_estimates().to_vec())
                .unwrap_or_default();
            let cid = CellId(cid_u32);
            let elev = planet.cell_elevation_m(cid);
            let geo = planet.cell_biome_and_strata(cid);
            let watershed_id = planet.watershed_id(cid).0;
            let region_label = planet.region_label(cid);
            let river_downstream_cell_id =
                planet.watershed_downstream(cid).map(CellId::get);
            let river_flow_norm_01 = if geo.is_ocean {
                0.0
            } else {
                (planet.cell_drainage_area_km2(cid) / 2500.0).clamp(0.0, 1.0)
            };
            self.surveyed_cells.insert(
                cid_u32,
                KnownCellDto::Surveyed {
                    cell_id: cid_u32,
                    observed_tick: tick.get(),
                    is_live_hq_sight: true,
                    elevation_m: elev,
                    confidence_01: 1.0,
                    provenance_label: "Direct Observation (HQ)".to_string(),
                    biome_key: geo.biome_key,
                    is_ocean: geo.is_ocean,
                    river_downstream_cell_id,
                    river_flow_norm_01,
                    deposit_estimates: prev_estimates,
                    watershed_id,
                    region_label,
                    mean_annual_temp_c: planet.cell_mean_annual_temp_c(cid),
                    annual_precip_mm: planet.cell_base_precip_mm_yr(cid),
                    aridity_index_01: planet.cell_aridity_index_01(cid),
                    soil_fertility_01: planet.cell_soil_fertility_01(cid),
                    rock_strata_code: planet.cell_strata_code(cid),
                    drainage_area_km2: planet.cell_drainage_area_km2(cid),
                    strahler_order: planet.cell_strahler_order(cid),
                    is_lake: planet.cell_is_lake(cid),
                    river_channel_downstream_cell_id: planet
                        .cell_river_channel_downstream(cid)
                        .map(CellId::get),
                    river_discharge_m3_s: planet.cell_river_discharge_m3_s(cid),
                    river_velocity_mps: planet.cell_river_velocity_mps(cid),
                    hydro_potential_kw: planet.cell_hydro_potential_kw(cid),
                    geology_province: planet.cell_geology_province(cid).as_str().to_owned(),
                    roughness_m: planet.cell_roughness_m(cid),
                    basin_depth_m: planet.cell_basin_depth_m(cid),
                    fault_instability_01: planet.cell_fault_instability_01(cid),
                    volcanic_hazard_01: planet.cell_volcanic_hazard_01(cid),
                    soil_drainage_01: planet.cell_soil_drainage_01(cid),
                    soil_clay_01: planet.cell_soil_clay_01(cid),
                    marine_upwelling_01: planet.cell_marine_upwelling_01(cid),
                    shelf_nutrient_01: planet.cell_shelf_nutrient_01(cid),
                    lake_kind: planet.cell_lake_kind(cid).as_str().to_owned(),
                    lake_salinity_per_mille: planet.cell_lake_salinity_per_mille(cid),
                    is_estuary: planet.cell_is_estuary(cid),
                    river_navigability: planet.cell_river_navigability(cid).as_str().to_owned(),
                    is_spring_oasis: false,
                    groundwater_m3: 0.0,
                    dam_status: None,
                },
            );
        }

        live_cells
    }

    /// Reads currently archived cell tier (`None` for unobserved cells).
    #[must_use]
    pub fn cell_knowledge(&self, cell_id: u32) -> Option<&KnownCellDto> {
        self.surveyed_cells.get(&cell_id)
    }

    /// Reads accumulated audit discrepancies (`ENG-EPI-004`).
    #[must_use]
    pub fn audit_discrepancies(&self) -> &[AuditDiscrepancyDto] {
        &self.audit_discrepancies
    }

    /// Compiles a strictly air-gapped `DiegeticSnapshot` with default running clock metadata.
    #[allow(clippy::too_many_arguments)]
    pub fn compile_snapshot(
        &mut self,
        tick: Tick,
        paused: bool,
        registry: &dyn ModRegistryPort,
        planet: &dyn PlanetGeometryPort,
        weather: &dyn SurfaceConditionsPort,
        ledger: &dyn PhysicalLedgerPort,
        population: &dyn PopulationPort,
    ) -> DiegeticSnapshot {
        self.compile_snapshot_with_clock(
            tick,
            paused,
            1,
            None,
            registry,
            planet,
            weather,
            ledger,
            population,
        )
    }

    /// Compiles a strictly air-gapped `DiegeticSnapshot` with clock state (M1/M2 compatibility API).
    #[allow(clippy::too_many_arguments)]
    pub fn compile_snapshot_with_clock(
        &mut self,
        tick: Tick,
        paused: bool,
        speed_multiplier: u32,
        auto_pause_reason: Option<String>,
        registry: &dyn ModRegistryPort,
        planet: &dyn PlanetGeometryPort,
        weather: &dyn SurfaceConditionsPort,
        ledger: &dyn PhysicalLedgerPort,
        population: &dyn PopulationPort,
    ) -> DiegeticSnapshot {
        self.compile_snapshot_m4(
            tick,
            paused,
            speed_multiplier,
            auto_pause_reason,
            registry,
            planet,
            weather,
            ledger,
            population,
            None,
            None,
            &SnapshotSubsystemInputs::default(),
        )
    }

    /// Compiles the full M3+M4 air-gapped `DiegeticSnapshot` with neutral M5-M7 plugs (`ENG-EPI-001..007`).
    #[allow(clippy::too_many_arguments)]
    pub fn compile_snapshot_m4(
        &mut self,
        tick: Tick,
        paused: bool,
        speed_multiplier: u32,
        auto_pause_reason: Option<String>,
        registry: &dyn ModRegistryPort,
        planet: &dyn PlanetGeometryPort,
        weather: &dyn SurfaceConditionsPort,
        ledger: &dyn PhysicalLedgerPort,
        population: &dyn PopulationPort,
        logistics: Option<&dyn LogisticsPort>,
        command: Option<&dyn CommandPort>,
        extras: &SnapshotSubsystemInputs,
    ) -> DiegeticSnapshot {
        self.compile_snapshot_m7(
            tick,
            paused,
            speed_multiplier,
            auto_pause_reason,
            registry,
            planet,
            weather,
            ledger,
            population,
            logistics,
            command,
            extras,
            M5ToM7PlugRefs::default(),
        )
    }

    /// Compiles the full M0-M7 + Option B continuous sphere air-gapped `DiegeticSnapshot` strictly through Spine ports (`ENG-EPI-001..007`).
    #[allow(clippy::too_many_arguments)]
    pub fn compile_snapshot_m7(
        &mut self,
        tick: Tick,
        paused: bool,
        speed_multiplier: u32,
        auto_pause_reason: Option<String>,
        registry: &dyn ModRegistryPort,
        planet: &dyn PlanetGeometryPort,
        weather: &dyn SurfaceConditionsPort,
        ledger: &dyn PhysicalLedgerPort,
        population: &dyn PopulationPort,
        logistics: Option<&dyn LogisticsPort>,
        command: Option<&dyn CommandPort>,
        extras: &SnapshotSubsystemInputs,
        plugs: M5ToM7PlugRefs<'_>,
    ) -> DiegeticSnapshot {
        let scenario = registry.scenario_init();
        let hq_cell = ledger.hq_cell();
        let sight_radius = ledger.hq_sight_radius_cells();
        let total_cells = planet.cell_count();

        let live_cells = self.observe_hq_halo(tick, planet, ledger);

        // Refresh live dynamic surface hydrology, aquifer springs, salinity, and dam status in HQ sight (`ENG-GEO-014`, `ENG-WTH-006`).
        for &cid_u32 in &live_cells {
            if let Some(KnownCellDto::Surveyed {
                river_discharge_m3_s,
                hydro_potential_kw,
                lake_salinity_per_mille,
                river_navigability,
                is_spring_oasis,
                groundwater_m3,
                dam_status,
                ..
            }) = self.surveyed_cells.get_mut(&cid_u32)
            {
                let cid = CellId(cid_u32);
                let live_q = weather.cell_live_discharge_m3_s(cid);
                if live_q > 0.0 || planet.cell_river_channel_downstream(cid).is_some() {
                    *river_discharge_m3_s = live_q.max(planet.cell_river_discharge_m3_s(cid));
                }
                let live_sal = weather.cell_salinity_per_mille(cid);
                if live_sal > 0.0 {
                    *lake_salinity_per_mille = live_sal;
                }
                let live_nav = weather.cell_live_navigability(cid);
                if live_nav != sim_spine::RiverNavigabilityKind::None {
                    *river_navigability = live_nav.as_str().to_owned();
                } else {
                    *river_navigability = planet.cell_river_navigability(cid).as_str().to_owned();
                }
                *is_spring_oasis = weather.cell_is_spring_oasis(cid);
                *groundwater_m3 = weather.cell_groundwater_m3(cid);
                let live_dam = weather.dam_at_cell(cid).map(DamStatusDto::from);
                if let Some(ref d) = live_dam {
                    *hydro_potential_kw = d.live_hydro_kw.max(planet.cell_hydro_potential_kw(cid));
                } else {
                    *hydro_potential_kw = planet.cell_hydro_potential_kw(cid);
                }
                *dam_status = live_dam;
            }
        }

        // Strict air-gap: union of sampled globe cells, live HQ sight halo, and previously known cells.
        let mut emitted_cells: BTreeSet<u32> = (0..total_cells.min(64)).collect();
        emitted_cells.extend(&live_cells);
        emitted_cells.extend(self.surveyed_cells.keys().copied());

        let mut known_cells = Vec::with_capacity(emitted_cells.len());
        let mut observed_weather = Vec::new();
        for cid_u32 in emitted_cells {
            if live_cells.contains(&cid_u32) {
                let st = weather.surface(CellId(cid_u32));
                observed_weather.push(ObservedWeatherDto {
                    cell_id: cid_u32,
                    observed_tick: tick.get(),
                    precip_mm_hr: st.precip_mm_hr,
                    mud_index_01: st.mud_index_01,
                    temp_c: st.temp_c,
                    visibility_01: st.visibility_01,
                    flood_stage_01: st.flood_stage_01,
                    snow_depth_m: st.snow_depth_m,
                    ice_thickness_m: st.ice_thickness_m,
                    wind_u_mps: st.wind_u_mps,
                    wind_v_mps: st.wind_v_mps,
                });
            } else if let Some(remote_w) = self.remote_weather_reports.get(&cid_u32) {
                observed_weather.push(remote_w.clone());
            }

            if let Some(known) = self.surveyed_cells.get(&cid_u32) {
                known_cells.push(known.clone());
            } else {
                known_cells.push(KnownCellDto::Unexplored { cell_id: cid_u32 });
            }
        }

        // Known settlements, live vs remote courier-reported commodity ledgers, and population cohorts
        let mut ledger_rows = Vec::new();
        let mut population_rows = Vec::new();
        let hq_sid = ledger.hq_settlement();
        let mut known_settlements = Vec::new();
        let mut surveyed_settlement_cells = BTreeSet::new();
        let mut known_settlement_meta: BTreeMap<u32, (String, u32, bool)> = BTreeMap::new();
        let mut all_settlement_names: BTreeMap<u32, String> = BTreeMap::new();
        for settlement in ledger.settlements() {
            let sid_u32 = settlement.id.get();
            all_settlement_names.insert(sid_u32, settlement.display_name.clone());
            let scell = settlement.cell_id.get();
            let is_live = live_cells.contains(&scell);
            if is_live || self.surveyed_cells.contains_key(&scell) {
                surveyed_settlement_cells.insert(scell);
                known_settlement_meta.insert(
                    sid_u32,
                    (settlement.display_name.clone(), scell, is_live),
                );
                known_settlements.push(KnownSettlementDto {
                    settlement_id: sid_u32,
                    settlement_name: settlement.display_name.clone(),
                    cell_id: scell,
                    is_hq: hq_sid == Some(settlement.id),
                });
            }
            let unrest_label = extras
                .unrest_by_settlement
                .get(&settlement.id.get())
                .cloned()
                .unwrap_or_else(|| format!("{:?}", population.unrest_stage_for_settlement(settlement.id)));

            if live_cells.contains(&scell) {
                for entry in ledger.settlement_inventory_entries(settlement.id) {
                    let (c_key, c_name, c_cat) = registry
                        .commodity_def(entry.commodity)
                        .map(|c| (c.key.clone(), c.display_name.clone(), c.category.clone()))
                        .unwrap_or_default();
                    ledger_rows.push(ReportedLedgerRowDto {
                        settlement_id: settlement.id.get(),
                        settlement_name: settlement.display_name.clone(),
                        commodity_key: c_key,
                        commodity_name: c_name,
                        reported_mass_kg: entry.mass_kg,
                        min_est_kg: entry.mass_kg,
                        max_est_kg: entry.mass_kg,
                        observed_tick: tick.get(),
                        provenance_label: "Direct Observation (HQ)".to_string(),
                        category: c_cat,
                        confidence_01: 1.0,
                        reporter_name: "HQ Quartermaster".to_string(),
                    });
                }
                for &cohort_id in population.settlement_cohorts(settlement.id) {
                    if let Some(cv) = population.cohort_view(cohort_id) {
                        let (v_key, v_name) = registry
                            .vocation_def(cv.vocation)
                            .map(|v| (v.key.clone(), v.display_name.clone()))
                            .unwrap_or_else(|| (format!("v_{}", cv.vocation.0), "Laborer".to_owned()));
                        population_rows.push(PopulationCohortRowDto {
                            cohort_id: cv.id.0,
                            settlement_id: settlement.id.get(),
                            settlement_name: settlement.display_name.clone(),
                            vocation_key: v_key,
                            vocation_name: v_name,
                            reported_headcount: cv.headcount,
                            min_headcount: cv.headcount,
                            max_headcount: cv.headcount,
                            mean_skill_01: cv.mean_skill_01,
                            literacy_01: 0.70,
                            nutrition_01: 0.90,
                            unrest_stage: unrest_label.clone(),
                            observed_tick: tick.get(),
                            confidence_01: 1.0,
                            provenance_label: "Direct Observation (HQ)".to_string(),
                        });
                    }
                }
            } else {
                // Strictly air-gapped remote settlement: emit only the latest courier-reported rows in archive.
                for ((sid, _), row) in &self.remote_ledger_reports {
                    if *sid == settlement.id.get() {
                        let mut aged = row.clone();
                        let age = tick.get().saturating_sub(row.observed_tick);
                        aged.confidence_01 = (1.0 / (1.0 + (age as f32) / 140.0)).clamp(0.18, 0.94);
                        ledger_rows.push(aged);
                    }
                }
                for row in self.remote_pop_reports.values() {
                    if row.settlement_id == settlement.id.get() {
                        population_rows.push(row.clone());
                    }
                }
            }
        }

        // Surveyed road edges: strictly emit built or traffic-worn roads whose both land endpoints are known (`ENG-LOG-007`).
        let mut surveyed_roads = Vec::new();
        let mut seen_edges = BTreeSet::new();
        for info in extras.road_edges.values() {
            if !info.road_built {
                continue;
            }
            if !self.surveyed_cells.contains_key(&info.from_cell_id)
                || !self.surveyed_cells.contains_key(&info.to_cell_id)
            {
                continue;
            }
            let from_ocean = planet.cell_biome_and_strata(CellId(info.from_cell_id)).is_ocean;
            let to_ocean = planet.cell_biome_and_strata(CellId(info.to_cell_id)).is_ocean;
            if from_ocean || to_ocean {
                continue;
            }
            if seen_edges.insert(info.edge_id) {
                surveyed_roads.push(SurveyedRoadEdgeDto {
                    edge_id: info.edge_id,
                    from_cell_id: info.from_cell_id,
                    to_cell_id: info.to_cell_id,
                    infra_key: info.infra_key.clone(),
                    washed_out: info.washed_out,
                    river_edge: info.river_edge,
                    bridge_cap_kg: info.bridge_cap_kg,
                    road_purpose: info.road_purpose.clone(),
                    purpose_label: info.purpose_label.clone(),
                    cumulative_passages: info.cumulative_passages,
                    cumulative_tonnes_passed: info.cumulative_tonnes_passed,
                });
            }
        }

        // Dead-reckoning out-of-sight convoy projections (`ENG-EPI-005`) + HQ transit tracking (`ENG-EPI-003`)
        let mut tracked_convoys = Vec::new();
        let mut hq_in_transit = false;
        let mut hq_dest_cell = None;
        let mut hq_eta_tick = None;
        if let Some(log_port) = logistics {
            for convoy in log_port.all_active_convoys() {
                if convoy.is_hq_move {
                    hq_in_transit = true;
                    hq_dest_cell = Some(convoy.destination_cell.get());
                    hq_eta_tick = Some(convoy.expected_arrival_tick.get());
                }
                let (status, projected_cell, current_unit_xyz, distance_travelled_km, conf, prov) =
                    if live_cells.contains(&convoy.current_cell.get()) {
                        (
                            "confirmed_sighting".to_owned(),
                            convoy.current_cell.get(),
                            convoy.current_unit_xyz,
                            convoy.distance_travelled_km,
                            1.0,
                            format!("Confirmed HQ Sight (Tick {})", tick.get()),
                        )
                    } else if tick.get() <= convoy.expected_arrival_tick.get() {
                        let span = convoy
                            .expected_arrival_tick
                            .get()
                            .saturating_sub(convoy.departure_tick.get())
                            .max(1);
                        let elapsed = tick.get().saturating_sub(convoy.departure_tick.get());
                        let frac = ((elapsed as f64) / (span as f64)).clamp(0.0, 1.0);
                        let max_idx = convoy.planned_route.len().saturating_sub(1);
                        let idx = ((frac * (max_idx as f64)).round() as usize).min(max_idx);
                        let proj_c = convoy
                            .planned_route
                            .get(idx)
                            .map(|c| c.get())
                            .unwrap_or(convoy.origin_cell.get());
                        let dead_reckoned_xyz = slerp_unit_xyz(
                            convoy.origin_unit_xyz,
                            convoy.destination_unit_xyz,
                            frac as f32,
                        );
                        (
                            "en_route_projected".to_owned(),
                            proj_c,
                            dead_reckoned_xyz,
                            (frac as f32) * convoy.total_route_km,
                            0.64,
                            format!(
                                "Dead-Reckoned Estimate (ETA Tick {})",
                                convoy.expected_arrival_tick.get()
                            ),
                        )
                    } else if tick.get() <= convoy.expected_arrival_tick.get() + convoy.grace_ticks {
                        (
                            "delayed_projected".to_owned(),
                            convoy.destination_cell.get(),
                            convoy.destination_unit_xyz,
                            convoy.total_route_km,
                            0.42,
                            format!(
                                "Awaiting Overdue Arrival (ETA was Tick {})",
                                convoy.expected_arrival_tick.get()
                            ),
                        )
                    } else {
                        (
                            "overdue".to_owned(),
                            convoy.destination_cell.get(),
                            convoy.destination_unit_xyz,
                            convoy.total_route_km,
                            0.20,
                            format!(
                                "OVERDUE — No Courier Confirmation Since Tick {}",
                                convoy.departure_tick.get()
                            ),
                        )
                    };

                tracked_convoys.push(TrackedConvoyDto {
                    convoy_id: convoy.convoy_id.0,
                    label: convoy.label.clone(),
                    status,
                    projected_cell_id: projected_cell,
                    channel: format!("{:?}", convoy.channel),
                    origin_cell_id: convoy.origin_cell.get(),
                    destination_cell_id: convoy.destination_cell.get(),
                    route_cells: convoy.planned_route.iter().map(|c| c.get()).collect(),
                    origin_unit_xyz: convoy.origin_unit_xyz,
                    current_unit_xyz,
                    destination_unit_xyz: convoy.destination_unit_xyz,
                    route_waypoints_xyz: convoy.route_waypoints_xyz.clone(),
                    distance_travelled_km,
                    total_route_km: convoy.total_route_km,
                    departure_tick: convoy.departure_tick.get(),
                    expected_arrival_tick: convoy.expected_arrival_tick.get(),
                    confidence_01: conf,
                    provenance_label: prov,
                });
            }
        }

        // Communication latency contours from active HQ cell across known settlements/rings
        let mut communication_contours = Vec::new();
        for ks in &known_settlements {
            let km = planet.great_circle_km(hq_cell, CellId(ks.cell_id));
            let hrs = (km / 14.0).max(1.0);
            communication_contours.push(CommLatencyContourDto {
                cell_id: ks.cell_id,
                estimated_courier_hours: hrs,
            });
        }

        let hq_name = hq_sid
            .and_then(|sid| ledger.settlements().iter().find(|s| s.id == sid))
            .map(|s| s.display_name.clone())
            .unwrap_or_else(|| scenario.hq_settlement_key.clone());

        let (cap_attn, used_attn) = command
            .map(|cmd| {
                let a = cmd.attention_summary(CharacterId(0));
                (a.capacity_attention, a.used_attention)
            })
            .unwrap_or((scenario.governance_base_attention, 0.0));

        let subordinates = population
            .all_characters()
            .iter()
            .map(|ch| {
                let char_mandates: Vec<MandateSummaryDto> = extras
                    .mandates
                    .iter()
                    .filter(|m| m.assignee_id == ch.id.get())
                    .cloned()
                    .collect();
                let home_cell_id = ledger
                    .settlements()
                    .iter()
                    .find(|s| s.id == ch.home_settlement)
                    .map(|s| s.cell_id.get())
                    .unwrap_or(hq_cell.get());
                let loyalty_label = if ch.honesty_vs_sycophancy >= 0.45 {
                    "Candid".to_owned()
                } else if ch.honesty_vs_sycophancy <= -0.25 {
                    "Pliant".to_owned()
                } else {
                    "Guarded".to_owned()
                };
                let alloc = (char_mandates.len() as f32) * scenario.clerical_overhead_per_mandate;
                SubordinateSummaryDto {
                    character_id: ch.id.get(),
                    display_name: ch.display_name.clone(),
                    role_title: ch.role_title.clone(),
                    competence: ch.competence,
                    home_settlement_id: ch.home_settlement.get(),
                    home_cell_id,
                    precision_01: ch.precision_01,
                    loyalty_label,
                    allocated_attention: alloc,
                    assigned_mandates: char_mandates,
                }
            })
            .collect();

        let time_series: Vec<TimeSeriesDto> = self
            .time_series_history
            .iter()
            .map(|(k, pts)| {
                let (label, unit) = match k.as_str() {
                    "grain_stock" => ("Reported Grain Reserve", "kg"),
                    "iron_stock" => ("Reported Iron Ingots", "kg"),
                    "population_total" => ("Colony Headcount", "people"),
                    "attention_load" => ("HQ Attention Load", "pct"),
                    _ => (k.as_str(), "units"),
                };
                TimeSeriesDto {
                    series_key: k.clone(),
                    label: label.to_owned(),
                    unit: unit.to_owned(),
                    points: pts.clone(),
                }
            })
            .collect();

        let mut char_names = BTreeMap::new();
        for ch in population.all_characters() {
            char_names.insert(ch.id.get(), ch.display_name.clone());
        }

        let available_models: Vec<GovernanceModelOptionDto> = registry
            .governance_models()
            .iter()
            .map(|m| GovernanceModelOptionDto {
                model_key: m.key.clone(),
                display_name: m.display_name.clone(),
                base_attention_capacity: m.base_attention_capacity,
                clerical_overhead_per_mandate: m.clerical_overhead_per_mandate,
                hierarchy_hop_latency_hours: m.hierarchy_hop_latency_hours,
            })
            .collect();
        let available_policies: Vec<PolicyLawOptionDto> = registry
            .policy_laws()
            .iter()
            .map(|p| PolicyLawOptionDto {
                policy_key: p.key.clone(),
                display_name: p.display_name.clone(),
                tax_or_tribute_rate_01: p.tax_or_tribute_rate,
                patent_duration_years: p.patent_duration_years,
            })
            .collect();
        let governance_dto = if let Some(gov) = plugs.governance.and_then(|g| g.governance_state())
        {
            GovernanceSnapshotDto {
                active_model_key: gov.active_model_key,
                active_model_name: gov.active_model_name,
                previous_model_key: gov.previous_model_key,
                adaptation_progress_01: gov.adaptation_progress_01,
                active_policy_key: gov.active_policy_key,
                active_policy_name: gov.active_policy_name,
                tax_or_tribute_rate_01: gov.tax_or_tribute_rate_01,
                patent_duration_years: gov.patent_duration_years,
                skimming_leakage_fraction: gov.skimming_leakage_fraction,
                effective_attention_capacity: gov.effective_attention_capacity,
                clerical_overhead_per_mandate: gov.clerical_overhead_per_mandate,
                hierarchy_hop_latency_hours: gov.hierarchy_hop_latency_hours,
                chartered_carriers_total: gov.chartered_carriers_total,
                bureaucratic_capture_events: gov.bureaucratic_capture_events,
                available_models,
                available_policies,
            }
        } else {
            GovernanceSnapshotDto {
                effective_attention_capacity: cap_attn,
                clerical_overhead_per_mandate: scenario.clerical_overhead_per_mandate,
                hierarchy_hop_latency_hours: scenario.hierarchy_hop_latency_hours,
                adaptation_progress_01: 1.0,
                available_models,
                available_policies,
                ..GovernanceSnapshotDto::default()
            }
        };

        let surveyed_edge_endpoints: BTreeMap<u32, (u32, u32)> = surveyed_roads
            .iter()
            .map(|r| (r.edge_id, (r.from_cell_id, r.to_cell_id)))
            .collect();
        let non_state_economy_dto = if let Some(eco) = plugs.non_state_eco {
            let actors = eco
                .non_state_actors()
                .iter()
                .filter_map(|a| {
                    let (s_name, _, is_live) = known_settlement_meta.get(&a.home_settlement.get())?;
                    Some(NonStateActorDto {
                        actor_id: a.actor_id.get(),
                        name: a.name.clone(),
                        kind: a.kind.as_str().to_owned(),
                        home_settlement_id: a.home_settlement.get(),
                        settlement_name: s_name.clone(),
                        working_capital_coins: a.working_capital_coins,
                        held_commodity_key: a
                            .held_commodity
                            .and_then(|cid| registry.commodity_def(cid).map(|c| c.key.clone())),
                        held_mass_kg: a.held_mass_kg,
                        hoarded_mass_kg: a.hoarded_mass_kg,
                        private_carriers_owned: a.private_carriers_owned,
                        chartered_to_state_carriers: a.chartered_to_state_carriers,
                        compliance_01: a.compliance_01,
                        observed_tick: tick.get(),
                        confidence_01: if *is_live { 1.0 } else { 0.78 },
                        provenance_label: if *is_live {
                            "Direct Observation (HQ)".to_owned()
                        } else {
                            "Commercial Guild Dispatch".to_owned()
                        },
                    })
                })
                .collect();
            let informal_markets = eco
                .informal_markets()
                .iter()
                .filter_map(|m| {
                    let (s_name, _, is_live) = known_settlement_meta.get(&m.settlement.get())?;
                    let (c_key, c_name) = registry
                        .commodity_def(m.commodity)
                        .map(|c| (c.key.clone(), c.display_name.clone()))
                        .unwrap_or_default();
                    Some(InformalMarketDto {
                        settlement_id: m.settlement.get(),
                        settlement_name: s_name.clone(),
                        commodity_key: c_key,
                        commodity_name: c_name,
                        clearing_price_index: m.clearing_price_index,
                        black_market_premium_01: m.black_market_premium_01,
                        black_market_leakage_kg: m.black_market_leakage_kg,
                        hoarded_reserve_kg: m.hoarded_reserve_kg,
                        observed_tick: tick.get(),
                        confidence_01: if *is_live { 1.0 } else { 0.75 },
                        provenance_label: if *is_live {
                            "Direct Market Audit (HQ)".to_owned()
                        } else {
                            "Remote Market Report".to_owned()
                        },
                    })
                })
                .collect();
            let congested_edges = eco
                .congested_edges()
                .iter()
                .filter_map(|ce| {
                    let (from_cell_id, to_cell_id) =
                        if let Some(pair) = surveyed_edge_endpoints.get(&ce.edge_id.get()) {
                            *pair
                        } else if let Some(info) = extras.road_edges.get(&ce.edge_id.get())
                            && self.surveyed_cells.contains_key(&info.from_cell_id)
                            && self.surveyed_cells.contains_key(&info.to_cell_id)
                        {
                            (info.from_cell_id, info.to_cell_id)
                        } else {
                            return None;
                        };
                    Some(EdgeCongestionDto {
                        edge_id: ce.edge_id.get(),
                        from_cell_id,
                        to_cell_id,
                        hourly_flow_tonnes: ce.hourly_flow_tonnes,
                        capacity_tonnes: ce.capacity_tonnes,
                        congestion_multiplier: ce.congestion_multiplier,
                    })
                })
                .collect();
            NonStateEconomySnapshotDto {
                actors,
                informal_markets,
                congested_edges,
            }
        } else {
            NonStateEconomySnapshotDto::default()
        };

        let tech_display_by_id: BTreeMap<u16, String> = registry
            .tech_components()
            .iter()
            .map(|c| (c.id.0, c.display_name.clone()))
            .collect();
        let theory_display_by_id: BTreeMap<u16, String> = registry
            .theory_domains()
            .iter()
            .map(|d| (d.id.0, d.display_name.clone()))
            .collect();
        let knowledge_dto = if let Some(kn) = plugs.knowledge {
            let settlements = kn
                .settlement_knowledge_views()
                .iter()
                .filter_map(|sk| {
                    let (s_name, _, is_live) = known_settlement_meta.get(&sk.settlement.get())?;
                    let know_how = sk
                        .know_how_levels
                        .iter()
                        .map(|(cid, key, val)| KnowledgeComponentScoreDto {
                            component_id: cid.0,
                            component_key: key.clone(),
                            display_name: tech_display_by_id
                                .get(&cid.0)
                                .cloned()
                                .unwrap_or_else(|| key.clone()),
                            know_how_01: *val,
                        })
                        .collect();
                    let know_why = sk
                        .know_why_levels
                        .iter()
                        .map(|(did, key, val)| TheoryDomainScoreDto {
                            domain_id: did.0,
                            domain_key: key.clone(),
                            display_name: theory_display_by_id
                                .get(&did.0)
                                .cloned()
                                .unwrap_or_else(|| key.clone()),
                            know_why_01: *val,
                        })
                        .collect();
                    Some(SettlementKnowledgeDto {
                        settlement_id: sk.settlement.get(),
                        settlement_name: s_name.clone(),
                        innovation_mode_key: sk.innovation_mode_key.clone(),
                        bottleneck_pressure_01: sk.bottleneck_pressure_01,
                        leisure_surplus_01: sk.leisure_surplus_01,
                        know_how,
                        know_why,
                        active_patent_holder_actor_id: sk
                            .active_patent_holder
                            .map(|a| a.get()),
                        patent_expiry_tick: sk.patent_expiry_tick.map(Tick::get),
                        regression_losses: sk.regression_losses,
                        observed_tick: tick.get(),
                        confidence_01: if *is_live { 1.0 } else { 0.80 },
                        provenance_label: if *is_live {
                            "Direct Archive Audit (HQ)".to_owned()
                        } else {
                            "Guild Master Courier".to_owned()
                        },
                    })
                })
                .collect();
            let carriers = kn
                .knowledge_carriers()
                .iter()
                .filter_map(|kc| {
                    let (s_name, _, _) = known_settlement_meta.get(&kc.settlement.get())?;
                    Some(KnowledgeCarrierDto {
                        settlement_id: kc.settlement.get(),
                        settlement_name: s_name.clone(),
                        component_key: kc.component_key.clone(),
                        carrier_kind: kc.carrier_kind.as_str().to_owned(),
                        retention_strength_01: kc.retention_strength_01,
                        holder_label: kc.holder_label.clone(),
                    })
                })
                .collect();
            KnowledgeSnapshotDto {
                settlements,
                carriers,
                available_modes: vec![
                    "guild_secrecy".to_owned(),
                    "state_arsenal".to_owned(),
                    "open_scholarly_patronage".to_owned(),
                    "private_patent_exclusivity".to_owned(),
                ],
            }
        } else {
            KnowledgeSnapshotDto::default()
        };

        let available_doctrines: Vec<DoctrineOptionDto> = registry
            .doctrines()
            .iter()
            .map(|d| DoctrineOptionDto {
                doctrine_key: d.key.clone(),
                display_name: d.display_name.clone(),
                initiative_bonus: d.initiative_bonus,
                strict_obedience_weight: d.strict_obedience_weight,
                rain_penalty_mitigation: d.rain_penalty_mitigation,
            })
            .collect();
        let warfare_dto = if let Some(war) = plugs.warfare {
            let theaters = war
                .theaters()
                .iter()
                .map(|t| TheaterDirectiveDto {
                    theater_id: u32::from(t.theater_id.0),
                    commander_id: t.commander.get(),
                    commander_name: char_names
                        .get(&t.commander.get())
                        .cloned()
                        .unwrap_or_else(|| format!("Commander #{}", t.commander.get())),
                    objective_cell_id: t.objective_cell.get(),
                    supply_priority_01: t.supply_priority_01,
                    issued_tick: t.issued_tick.get(),
                    autonomous_withdrawals: t.autonomous_withdrawals,
                })
                .collect();
            let formations = war
                .formations()
                .iter()
                .filter(|f| {
                    live_cells.contains(&f.current_cell.get())
                        || self.surveyed_cells.contains_key(&f.current_cell.get())
                        || known_settlement_meta.contains_key(&f.home_settlement.get())
                })
                .map(|f| {
                    let is_live = live_cells.contains(&f.current_cell.get());
                    MilitaryFormationDto {
                        formation_id: f.formation_id.0,
                        label: f.label.clone(),
                        template_key: f.template_key.clone(),
                        theater_id: u32::from(f.theater_id.0),
                        commander_id: f.commander.get(),
                        commander_name: char_names
                            .get(&f.commander.get())
                            .cloned()
                            .unwrap_or_else(|| format!("Commander #{}", f.commander.get())),
                        home_settlement_id: f.home_settlement.get(),
                        current_cell_id: f.current_cell.get(),
                        current_unit_xyz: f.current_unit_xyz,
                        target_cell_id: f.target_cell.get(),
                        target_unit_xyz: f.target_unit_xyz,
                        personnel_count: f.personnel_count,
                        equipment_mass_kg: f.equipment_mass_kg,
                        munitions_days: f.munitions_days,
                        rations_days: f.rations_days,
                        cohesion_01: f.cohesion_01,
                        doctrine_key: f.doctrine_key.clone(),
                        no_retreat_order: f.no_retreat_order,
                        encircled: f.encircled,
                        insubordinate_withdrawal: f.insubordinate_withdrawal,
                        stale_order_ticks: f.stale_order_ticks,
                        observed_tick: tick.get(),
                        confidence_01: if is_live { 1.0 } else { 0.62 },
                        provenance_label: if is_live {
                            "Direct Staff Observation".to_owned()
                        } else {
                            "Theater Courier Dispatch".to_owned()
                        },
                    }
                })
                .collect();
            WarfareSnapshotDto {
                theaters,
                formations,
                available_doctrines,
            }
        } else {
            WarfareSnapshotDto {
                available_doctrines,
                ..WarfareSnapshotDto::default()
            }
        };

        let succession_dto = if let Some(suc) = plugs.succession {
            let roster = suc
                .roster_characters()
                .iter()
                .filter(|c| {
                    known_settlement_meta.contains_key(&c.settlement.get()) || c.is_ruler
                })
                .map(|c| SuccessionCharacterDto {
                    character_id: c.character_id.get(),
                    name: c.name.clone(),
                    role_title: c.role_title.clone(),
                    age_years: c.age_years,
                    health_01: c.health_01,
                    is_alive: c.is_alive,
                    is_ruler: c.is_ruler,
                    settlement_id: c.settlement.get(),
                    settlement_name: all_settlement_names
                        .get(&c.settlement.get())
                        .cloned()
                        .unwrap_or_else(|| format!("Settlement #{}", c.settlement.get())),
                })
                .collect();
            let pending_transition = suc.pending_transition().map(|tr| SuccessionTransitionDto {
                rule_key: tr.rule_key.clone(),
                rule_display_name: tr.rule_display_name.clone(),
                deceased_ruler_name: tr.deceased_ruler_name.clone(),
                successor_name: tr.successor_name.clone(),
                transition_tick: tr.transition_tick.get(),
                disruption_until_tick: tr.disruption_until_tick.get(),
                inherited_archive_entries: tr.inherited_archive_entries,
                lost_unrecorded_rapport_count: tr.lost_unrecorded_rapport_count,
                awaiting_player_acknowledgement: tr.awaiting_player_acknowledgement,
            });
            SuccessionSnapshotDto {
                roster,
                pending_transition,
                effective_command_multiplier: suc.effective_command_multiplier(tick),
            }
        } else {
            SuccessionSnapshotDto::default()
        };

        let active_alert_count = u32::from(auto_pause_reason.is_some())
            + (extras.open_proposals.len() as u32)
            + (self.archived_dispatches.len().min(99) as u32);

        DiegeticSnapshot {
            schema_version: 2,
            scenario_key: scenario.scenario_key.clone(),
            scenario_name: scenario.display_name.clone(),
            globe_topology: GlobeTopologyDto {
                subdivision: scenario.goldberg_subdivision,
                cell_count: total_cells,
                triangles: planet.icosphere_triangles().to_vec(),
            },
            known_cells,
            hq_presence: HqPresenceDto {
                mode: if hq_in_transit {
                    "InTransit".to_string()
                } else {
                    "Stationary".to_string()
                },
                cell_id: hq_cell.get(),
                settlement_name: hq_name,
                sight_radius_cells: sight_radius,
                in_transit: hq_in_transit,
                destination_cell_id: hq_dest_cell,
                eta_tick: hq_eta_tick,
                communication_contours,
                relocation_previews: extras.relocation_previews.clone(),
            },
            known_settlements,
            surveyed_roads,
            tracked_convoys,
            observed_weather,
            ledger_rows,
            delegation_tree: DelegationTreeDto {
                leader_title: match scenario.archetype {
                    sim_spine::ScenarioArchetypeKind::EmergentDawn => "Elder Coordinator",
                    sim_spine::ScenarioArchetypeKind::ExpeditionColony => "Colony Commander",
                    sim_spine::ScenarioArchetypeKind::GovernorSuccessor => "Provincial Governor",
                }
                .to_string(),
                capacity_attention: cap_attn,
                used_attention: used_attn,
                subordinates,
                clerical_overhead: extras.clerical_overhead,
                overload_ratio: extras.overload_ratio,
                focused_mandate_id: extras.focused_mandate_id,
                focused_attention_share: extras.focused_attention_share,
                active_mandates: extras.mandates.clone(),
            },
            dispatch_inbox: self.archived_dispatches.clone(),
            clock_and_notifications: ClockAndNotificationsDto {
                tick: tick.get(),
                calendar_text: Self::format_calendar_tick(registry, tick),
                paused,
                speed_multiplier: speed_multiplier.max(1),
                active_alert_count,
                auto_pause_reason,
            },
            population_rows,
            facility_rows: extras.facility_rows.clone(),
            time_series,
            open_proposals: extras.open_proposals.clone(),
            notification_rules: extras.notification_rules.clone(),
            audit_discrepancies: self.audit_discrepancies.clone(),
            governance: governance_dto,
            non_state_economy: non_state_economy_dto,
            knowledge: knowledge_dto,
            warfare: warfare_dto,
            succession: succession_dto,
        }
    }
}

fn write_deposit_estimates(out: &mut Vec<u8>, ests: &[DepositEstimateDto]) {
    write_u32_le(out, ests.len() as u32);
    for d in ests {
        write_u16_le(out, d.material_id);
        write_str(out, &d.material_label);
        write_str(out, &d.category_key);
        write_f64_bits(out, d.min_tonnes);
        write_f64_bits(out, d.point_est_tonnes);
        write_f64_bits(out, d.max_tonnes);
        write_f32_bits(out, d.purity_est_01);
        write_f32_bits(out, d.confidence_01);
        write_f32_bits(out, d.surveyor_skill_01);
        write_u8(out, d.instrument_tier);
        write_u64_le(out, d.observed_tick);
        write_str(out, &d.provenance_label);
        write_u8(out, d.accessibility_tier);
        write_u8(out, u8::from(d.is_offshore));
        write_u8(out, u8::from(d.is_alluvial_placer));
    }
}

fn read_deposit_estimates(input: &mut &[u8]) -> Result<Vec<DepositEstimateDto>, String> {
    let n = read_u32_le(input)? as usize;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        out.push(DepositEstimateDto {
            material_id: read_u16_le(input)?,
            material_label: read_str(input)?,
            category_key: read_str(input)?,
            min_tonnes: read_f64_bits(input)?,
            point_est_tonnes: read_f64_bits(input)?,
            max_tonnes: read_f64_bits(input)?,
            purity_est_01: read_f32_bits(input)?,
            confidence_01: read_f32_bits(input)?,
            surveyor_skill_01: read_f32_bits(input)?,
            instrument_tier: read_u8(input)?,
            observed_tick: read_u64_le(input)?,
            provenance_label: read_str(input)?,
            accessibility_tier: read_u8(input)?,
            is_offshore: read_u8(input)? != 0,
            is_alluvial_placer: read_u8(input)? != 0,
        });
    }
    Ok(out)
}

fn write_dam_status(out: &mut Vec<u8>, dam: Option<&DamStatusDto>) {
    match dam {
        Some(d) => {
            write_u8(out, 1);
            write_u32_le(out, d.cell_id);
            write_str(out, &d.rule_curve);
            write_f64_bits(out, d.reservoir_storage_m3);
            write_f64_bits(out, d.reservoir_capacity_m3);
            write_f32_bits(out, d.reservoir_fill_01);
            write_f32_bits(out, d.reservoir_head_m);
            write_f32_bits(out, d.spillway_release_m3_s);
            write_f32_bits(out, d.live_hydro_kw);
            write_f32_bits(out, d.sediment_silt_01);
            write_f32_bits(out, d.condition_01);
            write_u8(out, u8::from(d.breached));
        }
        None => write_u8(out, 0),
    }
}

fn read_dam_status(input: &mut &[u8]) -> Result<Option<DamStatusDto>, String> {
    if read_u8(input)? == 0 {
        return Ok(None);
    }
    Ok(Some(DamStatusDto {
        cell_id: read_u32_le(input)?,
        rule_curve: read_str(input)?,
        reservoir_storage_m3: read_f64_bits(input)?,
        reservoir_capacity_m3: read_f64_bits(input)?,
        reservoir_fill_01: read_f32_bits(input)?,
        reservoir_head_m: read_f32_bits(input)?,
        spillway_release_m3_s: read_f32_bits(input)?,
        live_hydro_kw: read_f32_bits(input)?,
        sediment_silt_01: read_f32_bits(input)?,
        condition_01: read_f32_bits(input)?,
        breached: read_u8(input)? != 0,
    }))
}

struct CellGeoHydroExt {
    geology_province: String,
    roughness_m: f32,
    basin_depth_m: f32,
    fault_instability_01: f32,
    volcanic_hazard_01: f32,
    soil_drainage_01: f32,
    soil_clay_01: f32,
    marine_upwelling_01: f32,
    shelf_nutrient_01: f32,
    lake_kind: String,
    lake_salinity_per_mille: f32,
    is_estuary: bool,
    river_navigability: String,
    is_spring_oasis: bool,
    groundwater_m3: f64,
    dam_status: Option<DamStatusDto>,
}

#[allow(clippy::too_many_arguments)]
fn write_geo_hydro_ext(
    out: &mut Vec<u8>,
    geology_province: &str,
    roughness_m: f32,
    basin_depth_m: f32,
    fault_instability_01: f32,
    volcanic_hazard_01: f32,
    soil_drainage_01: f32,
    soil_clay_01: f32,
    marine_upwelling_01: f32,
    shelf_nutrient_01: f32,
    lake_kind: &str,
    lake_salinity_per_mille: f32,
    is_estuary: bool,
    river_navigability: &str,
    is_spring_oasis: bool,
    groundwater_m3: f64,
    dam_status: Option<&DamStatusDto>,
) {
    write_str(out, geology_province);
    write_f32_bits(out, roughness_m);
    write_f32_bits(out, basin_depth_m);
    write_f32_bits(out, fault_instability_01);
    write_f32_bits(out, volcanic_hazard_01);
    write_f32_bits(out, soil_drainage_01);
    write_f32_bits(out, soil_clay_01);
    write_f32_bits(out, marine_upwelling_01);
    write_f32_bits(out, shelf_nutrient_01);
    write_str(out, lake_kind);
    write_f32_bits(out, lake_salinity_per_mille);
    write_u8(out, u8::from(is_estuary));
    write_str(out, river_navigability);
    write_u8(out, u8::from(is_spring_oasis));
    write_f64_bits(out, groundwater_m3);
    write_dam_status(out, dam_status);
}

fn read_geo_hydro_ext(input: &mut &[u8]) -> Result<CellGeoHydroExt, String> {
    Ok(CellGeoHydroExt {
        geology_province: read_str(input)?,
        roughness_m: read_f32_bits(input)?,
        basin_depth_m: read_f32_bits(input)?,
        fault_instability_01: read_f32_bits(input)?,
        volcanic_hazard_01: read_f32_bits(input)?,
        soil_drainage_01: read_f32_bits(input)?,
        soil_clay_01: read_f32_bits(input)?,
        marine_upwelling_01: read_f32_bits(input)?,
        shelf_nutrient_01: read_f32_bits(input)?,
        lake_kind: read_str(input)?,
        lake_salinity_per_mille: read_f32_bits(input)?,
        is_estuary: read_u8(input)? != 0,
        river_navigability: read_str(input)?,
        is_spring_oasis: read_u8(input)? != 0,
        groundwater_m3: read_f64_bits(input)?,
        dam_status: read_dam_status(input)?,
    })
}

impl CanonicalSaveablePort for PlayerKnowledgeArchive {
    fn cartridge_tag(&self) -> &'static str {
        "player-view"
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        write_u32_le(out, self.surveyed_cells.len() as u32);
        for (&cid, entry) in &self.surveyed_cells {
            write_u32_le(out, cid);
            match entry {
                KnownCellDto::Unexplored { cell_id } => {
                    write_u8(out, 0);
                    write_u32_le(out, *cell_id);
                }
                KnownCellDto::Surveyed {
                    cell_id,
                    observed_tick,
                    is_live_hq_sight,
                    elevation_m,
                    confidence_01,
                    provenance_label,
                    biome_key,
                    is_ocean,
                    river_downstream_cell_id,
                    river_flow_norm_01,
                    deposit_estimates,
                    watershed_id,
                    region_label,
                    mean_annual_temp_c,
                    annual_precip_mm,
                    aridity_index_01,
                    soil_fertility_01,
                    rock_strata_code,
                    drainage_area_km2,
                    strahler_order,
                    is_lake,
                    river_channel_downstream_cell_id,
                    river_discharge_m3_s,
                    river_velocity_mps,
                    hydro_potential_kw,
                    geology_province,
                    roughness_m,
                    basin_depth_m,
                    fault_instability_01,
                    volcanic_hazard_01,
                    soil_drainage_01,
                    soil_clay_01,
                    marine_upwelling_01,
                    shelf_nutrient_01,
                    lake_kind,
                    lake_salinity_per_mille,
                    is_estuary,
                    river_navigability,
                    is_spring_oasis,
                    groundwater_m3,
                    dam_status,
                } => {
                    write_u8(out, 1);
                    write_u32_le(out, *cell_id);
                    write_u64_le(out, *observed_tick);
                    write_u8(out, u8::from(*is_live_hq_sight));
                    write_f32_bits(out, *elevation_m);
                    write_f32_bits(out, *confidence_01);
                    write_str(out, provenance_label);
                    write_str(out, biome_key);
                    write_u8(out, u8::from(*is_ocean));
                    match river_downstream_cell_id {
                        Some(downstream) => {
                            write_u8(out, 1);
                            write_u32_le(out, *downstream);
                        }
                        None => write_u8(out, 0),
                    }
                    write_f32_bits(out, *river_flow_norm_01);
                    write_deposit_estimates(out, deposit_estimates);
                    write_u32_le(out, *watershed_id);
                    write_str(out, region_label);
                    write_f32_bits(out, *mean_annual_temp_c);
                    write_f32_bits(out, *annual_precip_mm);
                    write_f32_bits(out, *aridity_index_01);
                    write_f32_bits(out, *soil_fertility_01);
                    write_u8(out, *rock_strata_code);
                    write_f32_bits(out, *drainage_area_km2);
                    write_u8(out, *strahler_order);
                    write_u8(out, u8::from(*is_lake));
                    match river_channel_downstream_cell_id {
                        Some(ch_ds) => {
                            write_u8(out, 1);
                            write_u32_le(out, *ch_ds);
                        }
                        None => write_u8(out, 0),
                    }
                    write_f32_bits(out, *river_discharge_m3_s);
                    write_f32_bits(out, *river_velocity_mps);
                    write_f32_bits(out, *hydro_potential_kw);
                    write_geo_hydro_ext(
                        out,
                        geology_province,
                        *roughness_m,
                        *basin_depth_m,
                        *fault_instability_01,
                        *volcanic_hazard_01,
                        *soil_drainage_01,
                        *soil_clay_01,
                        *marine_upwelling_01,
                        *shelf_nutrient_01,
                        lake_kind,
                        *lake_salinity_per_mille,
                        *is_estuary,
                        river_navigability,
                        *is_spring_oasis,
                        *groundwater_m3,
                        dam_status.as_ref(),
                    );
                }
                KnownCellDto::Rumored {
                    cell_id,
                    observed_tick,
                    elevation_m,
                    confidence_01,
                    provenance_label,
                    biome_key,
                    is_ocean,
                    watershed_id,
                    region_label,
                } => {
                    write_u8(out, 2);
                    write_u32_le(out, *cell_id);
                    write_u64_le(out, *observed_tick);
                    write_f32_bits(out, *elevation_m);
                    write_f32_bits(out, *confidence_01);
                    write_str(out, provenance_label);
                    write_str(out, biome_key);
                    write_u8(out, u8::from(*is_ocean));
                    write_u32_le(out, *watershed_id);
                    write_str(out, region_label);
                }
                KnownCellDto::Stale {
                    cell_id,
                    observed_tick,
                    last_live_tick,
                    elevation_m,
                    confidence_01,
                    provenance_label,
                    biome_key,
                    is_ocean,
                    river_downstream_cell_id,
                    river_flow_norm_01,
                    deposit_estimates,
                    watershed_id,
                    region_label,
                    mean_annual_temp_c,
                    annual_precip_mm,
                    aridity_index_01,
                    soil_fertility_01,
                    rock_strata_code,
                    drainage_area_km2,
                    strahler_order,
                    is_lake,
                    river_channel_downstream_cell_id,
                    river_discharge_m3_s,
                    river_velocity_mps,
                    hydro_potential_kw,
                    geology_province,
                    roughness_m,
                    basin_depth_m,
                    fault_instability_01,
                    volcanic_hazard_01,
                    soil_drainage_01,
                    soil_clay_01,
                    marine_upwelling_01,
                    shelf_nutrient_01,
                    lake_kind,
                    lake_salinity_per_mille,
                    is_estuary,
                    river_navigability,
                    is_spring_oasis,
                    groundwater_m3,
                    dam_status,
                } => {
                    write_u8(out, 3);
                    write_u32_le(out, *cell_id);
                    write_u64_le(out, *observed_tick);
                    write_u64_le(out, *last_live_tick);
                    write_f32_bits(out, *elevation_m);
                    write_f32_bits(out, *confidence_01);
                    write_str(out, provenance_label);
                    write_str(out, biome_key);
                    write_u8(out, u8::from(*is_ocean));
                    match river_downstream_cell_id {
                        Some(downstream) => {
                            write_u8(out, 1);
                            write_u32_le(out, *downstream);
                        }
                        None => write_u8(out, 0),
                    }
                    write_f32_bits(out, *river_flow_norm_01);
                    write_deposit_estimates(out, deposit_estimates);
                    write_u32_le(out, *watershed_id);
                    write_str(out, region_label);
                    write_f32_bits(out, *mean_annual_temp_c);
                    write_f32_bits(out, *annual_precip_mm);
                    write_f32_bits(out, *aridity_index_01);
                    write_f32_bits(out, *soil_fertility_01);
                    write_u8(out, *rock_strata_code);
                    write_f32_bits(out, *drainage_area_km2);
                    write_u8(out, *strahler_order);
                    write_u8(out, u8::from(*is_lake));
                    match river_channel_downstream_cell_id {
                        Some(ch_ds) => {
                            write_u8(out, 1);
                            write_u32_le(out, *ch_ds);
                        }
                        None => write_u8(out, 0),
                    }
                    write_f32_bits(out, *river_discharge_m3_s);
                    write_f32_bits(out, *river_velocity_mps);
                    write_f32_bits(out, *hydro_potential_kw);
                    write_geo_hydro_ext(
                        out,
                        geology_province,
                        *roughness_m,
                        *basin_depth_m,
                        *fault_instability_01,
                        *volcanic_hazard_01,
                        *soil_drainage_01,
                        *soil_clay_01,
                        *marine_upwelling_01,
                        *shelf_nutrient_01,
                        lake_kind,
                        *lake_salinity_per_mille,
                        *is_estuary,
                        river_navigability,
                        *is_spring_oasis,
                        *groundwater_m3,
                        dam_status.as_ref(),
                    );
                }
            }
        }

        write_u64_le(out, self.next_dispatch_seq);
        write_u32_le(out, self.archived_dispatches.len() as u32);
        for d in &self.archived_dispatches {
            write_u64_le(out, d.dispatch_id);
            write_str(out, &d.title);
            write_u64_le(out, d.sent_tick);
            write_u64_le(out, d.arrived_tick);
            write_str(out, &d.origin_label);
            write_u32_le(out, d.sender_id);
            write_str(out, &d.sender_name);
            write_u32_le(out, d.origin_cell_id);
            write_str(out, &d.event_class_key);
            write_str(out, &d.severity);
            write_f32_bits(out, d.confidence_01);
            write_u32_le(out, d.body_tokens.len() as u32);
            for t in &d.body_tokens {
                write_str(out, &t.text);
                write_u8(out, u8::from(t.entity_kind.is_some()));
                write_str(out, t.entity_kind.as_deref().unwrap_or(""));
                write_u8(out, u8::from(t.entity_id.is_some()));
                write_u64_le(out, t.entity_id.unwrap_or(0));
                write_u8(out, u8::from(t.target_cell_id.is_some()));
                write_u32_le(out, t.target_cell_id.unwrap_or(0));
            }
        }

        write_u32_le(out, self.remote_ledger_reports.len() as u32);
        for ((sid, key), row) in &self.remote_ledger_reports {
            write_u32_le(out, *sid);
            write_str(out, key);
            write_str(out, &row.settlement_name);
            write_str(out, &row.commodity_name);
            write_f64_bits(out, row.reported_mass_kg);
            write_f64_bits(out, row.min_est_kg);
            write_f64_bits(out, row.max_est_kg);
            write_u64_le(out, row.observed_tick);
            write_str(out, &row.provenance_label);
            write_str(out, &row.category);
            write_f32_bits(out, row.confidence_01);
            write_str(out, &row.reporter_name);
        }

        write_u32_le(out, self.time_series_history.len() as u32);
        for (k, pts) in &self.time_series_history {
            write_str(out, k);
            write_u32_le(out, pts.len() as u32);
            for p in pts {
                write_u64_le(out, p.tick);
                write_f64_bits(out, p.value);
                write_f64_bits(out, p.min_bound);
                write_f64_bits(out, p.max_bound);
                write_u8(out, u8::from(p.is_confirmed));
            }
        }

        write_u32_le(out, self.audit_discrepancies.len() as u32);
        for a in &self.audit_discrepancies {
            write_u32_le(out, a.settlement_id);
            write_u32_le(out, a.character_id);
            write_str(out, &a.character_name);
            write_str(out, &a.commodity_key);
            write_f64_bits(out, a.claimed_mass_kg);
            write_f64_bits(out, a.verified_mass_kg);
            write_f64_bits(out, a.discrepancy_kg);
            write_u64_le(out, a.detected_tick);
        }

        write_u32_le(out, self.remote_pop_reports.len() as u32);
        for (&cid, p) in &self.remote_pop_reports {
            write_u32_le(out, cid);
            write_u32_le(out, p.settlement_id);
            write_str(out, &p.settlement_name);
            write_str(out, &p.vocation_key);
            write_str(out, &p.vocation_name);
            write_u32_le(out, p.reported_headcount);
            write_u32_le(out, p.min_headcount);
            write_u32_le(out, p.max_headcount);
            write_f32_bits(out, p.mean_skill_01);
            write_f32_bits(out, p.literacy_01);
            write_f32_bits(out, p.nutrition_01);
            write_str(out, &p.unrest_stage);
            write_u64_le(out, p.observed_tick);
            write_f32_bits(out, p.confidence_01);
            write_str(out, &p.provenance_label);
        }

        write_u32_le(out, self.remote_weather_reports.len() as u32);
        for (&cid, w) in &self.remote_weather_reports {
            write_u32_le(out, cid);
            write_u64_le(out, w.observed_tick);
            write_f32_bits(out, w.precip_mm_hr);
            write_f32_bits(out, w.mud_index_01);
            write_f32_bits(out, w.temp_c);
            write_f32_bits(out, w.visibility_01);
            write_f32_bits(out, w.flood_stage_01);
            write_f32_bits(out, w.snow_depth_m);
            write_f32_bits(out, w.ice_thickness_m);
            write_f32_bits(out, w.wind_u_mps);
            write_f32_bits(out, w.wind_v_mps);
        }
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        let count = read_u32_le(input)? as usize;
        self.surveyed_cells.clear();
        for _ in 0..count {
            let key = read_u32_le(input)?;
            let tag = read_u8(input)?;
            let dto = match tag {
                0 => KnownCellDto::Unexplored {
                    cell_id: read_u32_le(input)?,
                },
                1 => {
                    let cell_id = read_u32_le(input)?;
                    let observed_tick = read_u64_le(input)?;
                    let is_live_hq_sight = read_u8(input)? != 0;
                    let elevation_m = read_f32_bits(input)?;
                    let confidence_01 = read_f32_bits(input)?;
                    let provenance_label = read_str(input)?;
                    let biome_key = read_str(input)?;
                    let is_ocean = read_u8(input)? != 0;
                    let river_downstream_cell_id = if read_u8(input)? != 0 {
                        Some(read_u32_le(input)?)
                    } else {
                        None
                    };
                    let river_flow_norm_01 = read_f32_bits(input)?;
                    let deposit_estimates = read_deposit_estimates(input)?;
                    let watershed_id = read_u32_le(input)?;
                    let region_label = read_str(input)?;
                    let mean_annual_temp_c = read_f32_bits(input)?;
                    let annual_precip_mm = read_f32_bits(input)?;
                    let aridity_index_01 = read_f32_bits(input)?;
                    let soil_fertility_01 = read_f32_bits(input)?;
                    let rock_strata_code = read_u8(input)?;
                    let drainage_area_km2 = read_f32_bits(input)?;
                    let strahler_order = read_u8(input)?;
                    let is_lake = read_u8(input)? != 0;
                    let river_channel_downstream_cell_id = if read_u8(input)? != 0 {
                        Some(read_u32_le(input)?)
                    } else {
                        None
                    };
                    let river_discharge_m3_s = read_f32_bits(input)?;
                    let river_velocity_mps = read_f32_bits(input)?;
                    let hydro_potential_kw = read_f32_bits(input)?;
                    let ext = read_geo_hydro_ext(input)?;
                    KnownCellDto::Surveyed {
                        cell_id,
                        observed_tick,
                        is_live_hq_sight,
                        elevation_m,
                        confidence_01,
                        provenance_label,
                        biome_key,
                        is_ocean,
                        river_downstream_cell_id,
                        river_flow_norm_01,
                        deposit_estimates,
                        watershed_id,
                        region_label,
                        mean_annual_temp_c,
                        annual_precip_mm,
                        aridity_index_01,
                        soil_fertility_01,
                        rock_strata_code,
                        drainage_area_km2,
                        strahler_order,
                        is_lake,
                        river_channel_downstream_cell_id,
                        river_discharge_m3_s,
                        river_velocity_mps,
                        hydro_potential_kw,
                        geology_province: ext.geology_province,
                        roughness_m: ext.roughness_m,
                        basin_depth_m: ext.basin_depth_m,
                        fault_instability_01: ext.fault_instability_01,
                        volcanic_hazard_01: ext.volcanic_hazard_01,
                        soil_drainage_01: ext.soil_drainage_01,
                        soil_clay_01: ext.soil_clay_01,
                        marine_upwelling_01: ext.marine_upwelling_01,
                        shelf_nutrient_01: ext.shelf_nutrient_01,
                        lake_kind: ext.lake_kind,
                        lake_salinity_per_mille: ext.lake_salinity_per_mille,
                        is_estuary: ext.is_estuary,
                        river_navigability: ext.river_navigability,
                        is_spring_oasis: ext.is_spring_oasis,
                        groundwater_m3: ext.groundwater_m3,
                        dam_status: ext.dam_status,
                    }
                }
                2 => KnownCellDto::Rumored {
                    cell_id: read_u32_le(input)?,
                    observed_tick: read_u64_le(input)?,
                    elevation_m: read_f32_bits(input)?,
                    confidence_01: read_f32_bits(input)?,
                    provenance_label: read_str(input)?,
                    biome_key: read_str(input)?,
                    is_ocean: read_u8(input)? != 0,
                    watershed_id: read_u32_le(input)?,
                    region_label: read_str(input)?,
                },
                3 => {
                    let cell_id = read_u32_le(input)?;
                    let observed_tick = read_u64_le(input)?;
                    let last_live_tick = read_u64_le(input)?;
                    let elevation_m = read_f32_bits(input)?;
                    let confidence_01 = read_f32_bits(input)?;
                    let provenance_label = read_str(input)?;
                    let biome_key = read_str(input)?;
                    let is_ocean = read_u8(input)? != 0;
                    let river_downstream_cell_id = if read_u8(input)? != 0 {
                        Some(read_u32_le(input)?)
                    } else {
                        None
                    };
                    let river_flow_norm_01 = read_f32_bits(input)?;
                    let deposit_estimates = read_deposit_estimates(input)?;
                    let watershed_id = read_u32_le(input)?;
                    let region_label = read_str(input)?;
                    let mean_annual_temp_c = read_f32_bits(input)?;
                    let annual_precip_mm = read_f32_bits(input)?;
                    let aridity_index_01 = read_f32_bits(input)?;
                    let soil_fertility_01 = read_f32_bits(input)?;
                    let rock_strata_code = read_u8(input)?;
                    let drainage_area_km2 = read_f32_bits(input)?;
                    let strahler_order = read_u8(input)?;
                    let is_lake = read_u8(input)? != 0;
                    let river_channel_downstream_cell_id = if read_u8(input)? != 0 {
                        Some(read_u32_le(input)?)
                    } else {
                        None
                    };
                    let river_discharge_m3_s = read_f32_bits(input)?;
                    let river_velocity_mps = read_f32_bits(input)?;
                    let hydro_potential_kw = read_f32_bits(input)?;
                    let ext = read_geo_hydro_ext(input)?;
                    KnownCellDto::Stale {
                        cell_id,
                        observed_tick,
                        last_live_tick,
                        elevation_m,
                        confidence_01,
                        provenance_label,
                        biome_key,
                        is_ocean,
                        river_downstream_cell_id,
                        river_flow_norm_01,
                        deposit_estimates,
                        watershed_id,
                        region_label,
                        mean_annual_temp_c,
                        annual_precip_mm,
                        aridity_index_01,
                        soil_fertility_01,
                        rock_strata_code,
                        drainage_area_km2,
                        strahler_order,
                        is_lake,
                        river_channel_downstream_cell_id,
                        river_discharge_m3_s,
                        river_velocity_mps,
                        hydro_potential_kw,
                        geology_province: ext.geology_province,
                        roughness_m: ext.roughness_m,
                        basin_depth_m: ext.basin_depth_m,
                        fault_instability_01: ext.fault_instability_01,
                        volcanic_hazard_01: ext.volcanic_hazard_01,
                        soil_drainage_01: ext.soil_drainage_01,
                        soil_clay_01: ext.soil_clay_01,
                        marine_upwelling_01: ext.marine_upwelling_01,
                        shelf_nutrient_01: ext.shelf_nutrient_01,
                        lake_kind: ext.lake_kind,
                        lake_salinity_per_mille: ext.lake_salinity_per_mille,
                        is_estuary: ext.is_estuary,
                        river_navigability: ext.river_navigability,
                        is_spring_oasis: ext.is_spring_oasis,
                        groundwater_m3: ext.groundwater_m3,
                        dam_status: ext.dam_status,
                    }
                }
                other => return Err(format!("invalid KnownCellDto tag {other}")),
            };
            self.surveyed_cells.insert(key, dto);
        }

        self.next_dispatch_seq = read_u64_le(input)?;
        let n_disp = read_u32_le(input)? as usize;
        self.archived_dispatches.clear();
        for _ in 0..n_disp {
            let dispatch_id = read_u64_le(input)?;
            let title = read_str(input)?;
            let sent_tick = read_u64_le(input)?;
            let arrived_tick = read_u64_le(input)?;
            let origin_label = read_str(input)?;
            let sender_id = read_u32_le(input)?;
            let sender_name = read_str(input)?;
            let origin_cell_id = read_u32_le(input)?;
            let event_class_key = read_str(input)?;
            let severity = read_str(input)?;
            let confidence_01 = read_f32_bits(input)?;
            let n_tok = read_u32_le(input)? as usize;
            let mut body_tokens = Vec::with_capacity(n_tok);
            for _ in 0..n_tok {
                let text = read_str(input)?;
                let has_kind = read_u8(input)? != 0;
                let ek = read_str(input)?;
                let has_eid = read_u8(input)? != 0;
                let eid = read_u64_le(input)?;
                let has_cell = read_u8(input)? != 0;
                let cid = read_u32_le(input)?;
                body_tokens.push(DispatchTokenDto {
                    text,
                    entity_kind: has_kind.then_some(ek),
                    entity_id: has_eid.then_some(eid),
                    target_cell_id: has_cell.then_some(cid),
                });
            }
            self.archived_dispatches.push(DiegeticDispatchDto {
                dispatch_id,
                title,
                sent_tick,
                arrived_tick,
                origin_label,
                sender_id,
                sender_name,
                origin_cell_id,
                event_class_key,
                severity,
                confidence_01,
                body_tokens,
            });
        }

        let n_rl = read_u32_le(input)? as usize;
        self.remote_ledger_reports.clear();
        for _ in 0..n_rl {
            let sid = read_u32_le(input)?;
            let key = read_str(input)?;
            let row = ReportedLedgerRowDto {
                settlement_id: sid,
                settlement_name: read_str(input)?,
                commodity_key: key.clone(),
                commodity_name: read_str(input)?,
                reported_mass_kg: read_f64_bits(input)?,
                min_est_kg: read_f64_bits(input)?,
                max_est_kg: read_f64_bits(input)?,
                observed_tick: read_u64_le(input)?,
                provenance_label: read_str(input)?,
                category: read_str(input)?,
                confidence_01: read_f32_bits(input)?,
                reporter_name: read_str(input)?,
            };
            self.remote_ledger_reports.insert((sid, key), row);
        }

        let n_ts = read_u32_le(input)? as usize;
        self.time_series_history.clear();
        for _ in 0..n_ts {
            let k = read_str(input)?;
            let n_p = read_u32_le(input)? as usize;
            let mut pts = Vec::with_capacity(n_p);
            for _ in 0..n_p {
                pts.push(TimeSeriesPointDto {
                    tick: read_u64_le(input)?,
                    value: read_f64_bits(input)?,
                    min_bound: read_f64_bits(input)?,
                    max_bound: read_f64_bits(input)?,
                    is_confirmed: read_u8(input)? != 0,
                });
            }
            self.time_series_history.insert(k, pts);
        }

        let n_aud = read_u32_le(input)? as usize;
        self.audit_discrepancies.clear();
        for _ in 0..n_aud {
            self.audit_discrepancies.push(AuditDiscrepancyDto {
                settlement_id: read_u32_le(input)?,
                character_id: read_u32_le(input)?,
                character_name: read_str(input)?,
                commodity_key: read_str(input)?,
                claimed_mass_kg: read_f64_bits(input)?,
                verified_mass_kg: read_f64_bits(input)?,
                discrepancy_kg: read_f64_bits(input)?,
                detected_tick: read_u64_le(input)?,
            });
        }

        let n_pop = read_u32_le(input)? as usize;
        self.remote_pop_reports.clear();
        for _ in 0..n_pop {
            let cid = read_u32_le(input)?;
            let row = PopulationCohortRowDto {
                cohort_id: cid,
                settlement_id: read_u32_le(input)?,
                settlement_name: read_str(input)?,
                vocation_key: read_str(input)?,
                vocation_name: read_str(input)?,
                reported_headcount: read_u32_le(input)?,
                min_headcount: read_u32_le(input)?,
                max_headcount: read_u32_le(input)?,
                mean_skill_01: read_f32_bits(input)?,
                literacy_01: read_f32_bits(input)?,
                nutrition_01: read_f32_bits(input)?,
                unrest_stage: read_str(input)?,
                observed_tick: read_u64_le(input)?,
                confidence_01: read_f32_bits(input)?,
                provenance_label: read_str(input)?,
            };
            self.remote_pop_reports.insert(cid, row);
        }

        let n_wr = read_u32_le(input)? as usize;
        self.remote_weather_reports.clear();
        for _ in 0..n_wr {
            let cid = read_u32_le(input)?;
            let obs = ObservedWeatherDto {
                cell_id: cid,
                observed_tick: read_u64_le(input)?,
                precip_mm_hr: read_f32_bits(input)?,
                mud_index_01: read_f32_bits(input)?,
                temp_c: read_f32_bits(input)?,
                visibility_01: read_f32_bits(input)?,
                flood_stage_01: read_f32_bits(input)?,
                snow_depth_m: read_f32_bits(input)?,
                ice_thickness_m: read_f32_bits(input)?,
                wind_u_mps: read_f32_bits(input)?,
                wind_v_mps: read_f32_bits(input)?,
            };
            self.remote_weather_reports.insert(cid, obs);
        }
        Ok(())
    }
}
