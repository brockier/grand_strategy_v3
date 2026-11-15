//! Strict Epistemic Air-Gap & IPC DTO Boundary (`player-view`).
//! The **only** simulation crate allowed to emit IPC JSON payloads to `code/ui`.
#![deny(clippy::disallowed_types)]

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use sim_spine::{
    codec::{
        read_f32_bits, read_f64_bits, read_str, read_u16_le, read_u32_le, read_u64_le, read_u8,
        write_f32_bits, write_f64_bits, write_str, write_u16_le, write_u32_le, write_u64_le,
        write_u8,
    },
    CanonicalSaveablePort, CellId, CharacterId, CharacterSnapshotView, CommandPort, LogisticsPort,
    MandateId, ModRegistryPort, NotificationBehaviorKind, PhysicalLedgerPort, PlanetGeometryPort,
    PopulationPort, SpinePlayerCommand, SurfaceConditionsPort, Tick,
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
        let mut frontier = vec![(hq_cell, 0u8)];
        live_cells.insert(hq_cell.get());
        while let Some((cid, dist)) = frontier.pop() {
            if dist < sight_radius {
                for &nbr in planet.cell_neighbors(cid) {
                    if live_cells.insert(nbr.get()) {
                        frontier.push((nbr, dist + 1));
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
                        },
                    );
                }
                KnownCellDto::Unexplored { .. } | KnownCellDto::Rumored { .. } => {}
            }
        }

        for &cid_u32 in &live_cells {
            let prev_estimates = self
                .surveyed_cells
                .get(&cid_u32)
                .map(|c| c.deposit_estimates().to_vec())
                .unwrap_or_default();
            let elev = planet.cell_elevation_m(CellId(cid_u32));
            let geo = planet.cell_biome_and_strata(CellId(cid_u32));
            let river_downstream_cell_id =
                planet.watershed_downstream(CellId(cid_u32)).map(CellId::get);
            let river_flow_norm_01 = if geo.is_ocean {
                0.0
            } else {
                (planet.cell_drainage_area_km2(CellId(cid_u32)) / 2500.0).clamp(0.0, 1.0)
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

    /// Compiles the full M3+M4 air-gapped `DiegeticSnapshot` reading exclusively through Spine ports (`ENG-EPI-001..007`).
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
        let scenario = registry.scenario_init();
        let hq_cell = ledger.hq_cell();
        let sight_radius = ledger.hq_sight_radius_cells();
        let total_cells = planet.cell_count();

        let live_cells = self.observe_hq_halo(tick, planet, ledger);

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
        for settlement in ledger.settlements() {
            let scell = settlement.cell_id.get();
            if live_cells.contains(&scell) || self.surveyed_cells.contains_key(&scell) {
                surveyed_settlement_cells.insert(scell);
                known_settlements.push(KnownSettlementDto {
                    settlement_id: settlement.id.get(),
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

        // Surveyed road edges around surveyed land cells
        let mut surveyed_roads = Vec::new();
        let mut seen_edges = BTreeSet::new();
        for &s_cell in &surveyed_settlement_cells {
            for &nbr in planet.cell_neighbors(CellId(s_cell)) {
                let nbr_u32 = nbr.get();
                if !self.surveyed_cells.contains_key(&nbr_u32) {
                    continue;
                }
                let nbr_geo = planet.cell_biome_and_strata(nbr);
                if nbr_geo.is_ocean {
                    continue;
                }
                if let Some(eid) = planet.cell_edge(CellId(s_cell), nbr)
                    && seen_edges.insert(eid.get())
                {
                    if let Some(info) = extras.road_edges.get(&eid.get()) {
                        surveyed_roads.push(SurveyedRoadEdgeDto {
                            edge_id: eid.get(),
                            from_cell_id: s_cell,
                            to_cell_id: nbr_u32,
                            infra_key: info.infra_key.clone(),
                            washed_out: info.washed_out,
                            river_edge: info.river_edge,
                            bridge_cap_kg: info.bridge_cap_kg,
                        });
                    } else {
                        surveyed_roads.push(SurveyedRoadEdgeDto {
                            edge_id: eid.get(),
                            from_cell_id: s_cell,
                            to_cell_id: nbr_u32,
                            infra_key: "foot_trail".to_string(),
                            washed_out: false,
                            river_edge: false,
                            bridge_cap_kg: None,
                        });
                    }
                }
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
                let (status, projected_cell, conf, prov) =
                    if live_cells.contains(&convoy.current_cell.get()) {
                        (
                            "confirmed_sighting".to_owned(),
                            convoy.current_cell.get(),
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
                        (
                            "en_route_projected".to_owned(),
                            proj_c,
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
        });
    }
    Ok(out)
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
                }
                KnownCellDto::Rumored {
                    cell_id,
                    observed_tick,
                    elevation_m,
                    confidence_01,
                    provenance_label,
                    biome_key,
                    is_ocean,
                } => {
                    write_u8(out, 2);
                    write_u32_le(out, *cell_id);
                    write_u64_le(out, *observed_tick);
                    write_f32_bits(out, *elevation_m);
                    write_f32_bits(out, *confidence_01);
                    write_str(out, provenance_label);
                    write_str(out, biome_key);
                    write_u8(out, u8::from(*is_ocean));
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
                1 => KnownCellDto::Surveyed {
                    cell_id: read_u32_le(input)?,
                    observed_tick: read_u64_le(input)?,
                    is_live_hq_sight: read_u8(input)? != 0,
                    elevation_m: read_f32_bits(input)?,
                    confidence_01: read_f32_bits(input)?,
                    provenance_label: read_str(input)?,
                    biome_key: read_str(input)?,
                    is_ocean: read_u8(input)? != 0,
                    river_downstream_cell_id: if read_u8(input)? != 0 {
                        Some(read_u32_le(input)?)
                    } else {
                        None
                    },
                    river_flow_norm_01: read_f32_bits(input)?,
                    deposit_estimates: read_deposit_estimates(input)?,
                },
                2 => KnownCellDto::Rumored {
                    cell_id: read_u32_le(input)?,
                    observed_tick: read_u64_le(input)?,
                    elevation_m: read_f32_bits(input)?,
                    confidence_01: read_f32_bits(input)?,
                    provenance_label: read_str(input)?,
                    biome_key: read_str(input)?,
                    is_ocean: read_u8(input)? != 0,
                },
                3 => KnownCellDto::Stale {
                    cell_id: read_u32_le(input)?,
                    observed_tick: read_u64_le(input)?,
                    last_live_tick: read_u64_le(input)?,
                    elevation_m: read_f32_bits(input)?,
                    confidence_01: read_f32_bits(input)?,
                    provenance_label: read_str(input)?,
                    biome_key: read_str(input)?,
                    is_ocean: read_u8(input)? != 0,
                    river_downstream_cell_id: if read_u8(input)? != 0 {
                        Some(read_u32_le(input)?)
                    } else {
                        None
                    },
                    river_flow_norm_01: read_f32_bits(input)?,
                    deposit_estimates: read_deposit_estimates(input)?,
                },
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
