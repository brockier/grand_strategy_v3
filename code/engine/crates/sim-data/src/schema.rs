//! Strongly-typed external mod schemas across all eleven data-driven domains.
//! Every schema struct derives `JsonSchema` (`schemars` Draft 2020-12) and `#[serde(deny_unknown_fields)]`.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Five deterministic layer tiers in load order priority.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug, Deserialize, Serialize, JsonSchema)]
pub enum ModLayerKind {
    CoreSchemas = 0,
    EraPack = 1,
    ScenarioPack = 2,
    UserMod = 3,
    UiSkin = 4,
}

/// Pack header manifest (`pack.toml`).
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackManifest {
    pub schema_version: u32,
    pub pack_id: String,
    pub display_name: String,
    pub layer: ModLayerKind,
    #[serde(default)]
    pub load_priority: i32,
}

// --- Domain 1: Planetary Generation & Natural Distributions ---

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanetGenerationRecord {
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
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanetGenDomain {
    pub planet_generation: Option<PlanetGenerationRecord>,
}

// --- Domain 2: Commodities, Materials & Knowledge Primitives ---

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommodityRecord {
    pub id: String,
    pub display_name: String,
    pub category: String,
    pub mass_per_unit_kg: f64,
    pub volume_per_unit_m3: f64,
    pub spoilage_rate_per_hour: f64,
    #[serde(default)]
    pub energy_mj_per_kg: f64,
    #[serde(default)]
    pub structural_strength: f64,
    #[serde(default)]
    pub is_tool: bool,
    #[serde(default)]
    pub tool_wear_per_hour: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TechComponentRecord {
    pub id: String,
    pub display_name: String,
    pub domain: String,
    pub empirical_difficulty: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TheoryDomainRecord {
    pub id: String,
    pub display_name: String,
    pub abstraction_depth: u8,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CommoditiesAndKnowledgeDomain {
    #[serde(default)]
    pub commodities: Vec<CommodityRecord>,
    #[serde(default)]
    pub tech_components: Vec<TechComponentRecord>,
    #[serde(default)]
    pub theory_domains: Vec<TheoryDomainRecord>,
}

// --- Domain 3: Industrial, Agricultural & Construction Processes ---

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProcessQuantityRecord {
    pub commodity: String,
    pub amount_kg: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProcessRecord {
    pub id: String,
    pub display_name: String,
    pub required_vocation: String,
    pub cycle_hours: u32,
    pub workforce_hours_required: f64,
    #[serde(default)]
    pub tool_commodity: Option<String>,
    pub weather_halt_mud_threshold: f32,
    #[serde(default)]
    pub inputs: Vec<ProcessQuantityRecord>,
    #[serde(default)]
    pub outputs: Vec<ProcessQuantityRecord>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProcessesDomain {
    #[serde(default)]
    pub processes: Vec<ProcessRecord>,
}

// --- Domain 4: Carriers & Infrastructure ---

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CarrierRecord {
    pub id: String,
    pub display_name: String,
    pub max_payload_kg: f64,
    pub max_payload_m3: f64,
    pub base_speed_km_hr: f32,
    pub max_grade_rad: f32,
    pub mud_penalty_factor: f32,
    pub crew_required: u32,
    pub fodder_or_fuel_commodity: String,
    pub fuel_burn_kg_per_hour: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InfraClassRecord {
    pub id: String,
    pub display_name: String,
    pub throughput_tonnes_per_hour: f32,
    pub speed_multiplier: f32,
    pub mud_shield_factor: f32,
    pub maintenance_timber_kg_per_week: f64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LogisticsDomain {
    #[serde(default)]
    pub carriers: Vec<CarrierRecord>,
    #[serde(default)]
    pub infra_classes: Vec<InfraClassRecord>,
}

// --- Domain 5: Population Cohorts & Demographics ---

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CohortArchetypeRecord {
    pub id: String,
    pub display_name: String,
    pub age_min_years: u16,
    pub age_max_years: u16,
    pub daily_grain_need_kg: f64,
    pub base_fertility_annual: f32,
    pub base_mortality_annual: f32,
    pub initial_literacy: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VocationRecord {
    pub id: String,
    pub display_name: String,
    pub base_skill_gain_per_day: f32,
    pub skill_decay_per_day: f32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PopulationDomain {
    #[serde(default)]
    pub cohort_archetypes: Vec<CohortArchetypeRecord>,
    #[serde(default)]
    pub vocations: Vec<VocationRecord>,
}

// --- Domain 6: Named Individuals & Succession ---

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CharacterTraitRecord {
    pub id: String,
    pub display_name: String,
    pub competence_delta: f32,
    pub precision_delta: f32,
    pub honesty_delta: f32,
    pub caution_delta: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SuccessionRuleRecord {
    pub id: String,
    pub display_name: String,
    pub continuity_fraction: f32,
    pub transition_disruption_hours: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CharactersDomain {
    #[serde(default)]
    pub traits: Vec<CharacterTraitRecord>,
    #[serde(default)]
    pub succession_rules: Vec<SuccessionRuleRecord>,
}

// --- Domain 7: Governance, Institutions & Policy Laws ---

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GovernanceModelRecord {
    pub id: String,
    pub display_name: String,
    pub base_attention_capacity: f32,
    pub clerical_overhead_per_mandate: f32,
    pub hierarchy_hop_latency_hours: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PolicyLawRecord {
    pub id: String,
    pub display_name: String,
    pub tax_or_tribute_rate: f32,
    pub patent_duration_years: u32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GovernanceDomain {
    #[serde(default)]
    pub governance_models: Vec<GovernanceModelRecord>,
    #[serde(default)]
    pub policy_laws: Vec<PolicyLawRecord>,
}

// --- Domain 8: Military Formations & Doctrines ---

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FormationTemplateRecord {
    pub id: String,
    pub display_name: String,
    pub personnel_count: u32,
    pub required_vocation: String,
    pub equipment_commodity: String,
    pub equipment_kg_per_troop: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DoctrineRecord {
    pub id: String,
    pub display_name: String,
    pub initiative_bonus: f32,
    pub strict_obedience_weight: f32,
    pub rain_penalty_mitigation: f32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WarfareDomain {
    #[serde(default)]
    pub formation_templates: Vec<FormationTemplateRecord>,
    #[serde(default)]
    pub doctrines: Vec<DoctrineRecord>,
}

// --- Domain 9: Scenarios & Calendars ---

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CalendarRecord {
    pub id: String,
    pub hours_per_day: u32,
    pub days_per_year: u32,
    pub year_prefix: String,
    pub starting_year: i32,
    pub month_names: Vec<String>,
    pub month_lengths: Vec<u32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScenarioHeaderRecord {
    pub id: String,
    pub display_name: String,
    pub starting_profile: String,
    pub governance_model: String,
    pub succession_rule: String,
    pub active_policy: String,
    pub hq_settlement_id: String,
    pub hq_sight_radius_cells: u8,
    pub unexplored_surrounding_cells: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartingStockpileRecord {
    pub commodity: String,
    pub mass_kg: f64,
    pub purity: f32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartingCohortRecord {
    pub archetype: String,
    pub vocation: String,
    pub count: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartingFacilityRecord {
    pub process: String,
    pub workers: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartingSettlementRecord {
    pub id: String,
    pub display_name: String,
    pub cell_index: u32,
    pub initial_population: u32,
    #[serde(default)]
    pub stockpiles: Vec<StartingStockpileRecord>,
    #[serde(default)]
    pub cohorts: Vec<StartingCohortRecord>,
    #[serde(default)]
    pub initial_facilities: Vec<StartingFacilityRecord>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartingCharacterRecord {
    pub id: String,
    pub display_name: String,
    pub role_title: String,
    pub home_settlement: String,
    pub competence: f32,
    pub precision_01: f32,
    pub honesty_vs_sycophancy: f32,
    pub caution_vs_alarmism: f32,
    #[serde(default)]
    pub traits: Vec<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ScenarioAndCalendarDomain {
    pub calendar: Option<CalendarRecord>,
    pub scenario: Option<ScenarioHeaderRecord>,
    #[serde(default)]
    pub starting_settlements: Vec<StartingSettlementRecord>,
    #[serde(default)]
    pub starting_characters: Vec<StartingCharacterRecord>,
}

// --- Domain 10: Notifications & Localization ---

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EventClassRecord {
    pub id: String,
    pub display_name: String,
    pub default_tier: String,
    pub template_key: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LocalizationStringRecord {
    pub id: String,
    pub text: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NotificationsAndLocDomain {
    #[serde(default)]
    pub event_classes: Vec<EventClassRecord>,
    #[serde(default)]
    pub strings: Vec<LocalizationStringRecord>,
}

// --- Domain 11: UI & Globe Skin Manifest Reference ---

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkinReferenceRecord {
    pub id: String,
    pub default_skin_id: String,
    pub skins_root_rel: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SkinRefDomain {
    pub skin_reference: Option<SkinReferenceRecord>,
}

/// Complete merged external mod data model across all eleven domains.
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MergedModDocument {
    pub planet: PlanetGenDomain,
    pub commodities_and_knowledge: CommoditiesAndKnowledgeDomain,
    pub processes: ProcessesDomain,
    pub logistics: LogisticsDomain,
    pub population: PopulationDomain,
    pub characters: CharactersDomain,
    pub governance: GovernanceDomain,
    pub warfare: WarfareDomain,
    pub scenario_and_calendar: ScenarioAndCalendarDomain,
    pub notifications_and_loc: NotificationsAndLocDomain,
    pub skin_ref: SkinRefDomain,
}
