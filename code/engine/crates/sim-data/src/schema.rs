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
pub struct NaturalWorldBoundsRecord {
    #[serde(default = "default_ore_count")]
    pub ore_count: u16,
    #[serde(default = "default_flux_count")]
    pub flux_count: u16,
    #[serde(default = "default_fuel_count")]
    pub fuel_count: u16,
    #[serde(default = "default_crop_count")]
    pub crop_count: u16,
    #[serde(default = "default_fauna_count")]
    pub fauna_count: u16,
    #[serde(default = "default_pathogen_count")]
    pub pathogen_count: u16,
    #[serde(default = "default_ore_melting_k_range")]
    pub ore_melting_k_range: [f32; 2],
    #[serde(default = "default_ore_purity_range")]
    pub ore_purity_range: [f32; 2],
    #[serde(default = "default_fuel_enthalpy_range")]
    pub fuel_enthalpy_mj_kg_range: [f32; 2],
    #[serde(default = "default_crop_temp_range")]
    pub crop_opt_temp_c_range: [f32; 2],
}

fn default_ore_count() -> u16 {
    6
}
fn default_flux_count() -> u16 {
    3
}
fn default_fuel_count() -> u16 {
    4
}
fn default_crop_count() -> u16 {
    5
}
fn default_fauna_count() -> u16 {
    4
}
fn default_pathogen_count() -> u16 {
    3
}
fn default_ore_melting_k_range() -> [f32; 2] {
    [1150.0, 1850.0]
}
fn default_ore_purity_range() -> [f32; 2] {
    [0.18, 0.72]
}
fn default_fuel_enthalpy_range() -> [f32; 2] {
    [14.0, 44.0]
}
fn default_crop_temp_range() -> [f32; 2] {
    [8.0, 30.0]
}

impl Default for NaturalWorldBoundsRecord {
    fn default() -> Self {
        Self {
            ore_count: default_ore_count(),
            flux_count: default_flux_count(),
            fuel_count: default_fuel_count(),
            crop_count: default_crop_count(),
            fauna_count: default_fauna_count(),
            pathogen_count: default_pathogen_count(),
            ore_melting_k_range: default_ore_melting_k_range(),
            ore_purity_range: default_ore_purity_range(),
            fuel_enthalpy_mj_kg_range: default_fuel_enthalpy_range(),
            crop_opt_temp_c_range: default_crop_temp_range(),
        }
    }
}

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
    #[serde(default)]
    pub natural_bounds: NaturalWorldBoundsRecord,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlanetGenDomain {
    pub planet_generation: Option<PlanetGenerationRecord>,
}

fn default_tool_hardness() -> f32 {
    0.75
}
fn default_process_min_temp() -> f32 {
    -40.0
}
fn default_process_max_temp() -> f32 {
    50.0
}
fn default_process_opt_moisture() -> f32 {
    0.45
}
fn default_tool_wear_intensity() -> f64 {
    1.0
}
fn default_max_ice_thickness() -> f32 {
    0.15
}
fn default_daily_water_kg() -> f64 {
    2.0
}
fn default_daily_fuel_kg() -> f64 {
    1.2
}
fn default_daily_clothing_kg() -> f64 {
    0.005
}
fn default_dependents_share() -> f32 {
    0.20
}
fn default_elders_share() -> f32 {
    0.12
}
fn default_roof_protection() -> f32 {
    0.95
}
fn default_half_f32() -> f32 {
    0.50
}
fn default_low_drive_f32() -> f32 {
    0.25
}
fn default_high_loyalty_f32() -> f32 {
    0.70
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
    #[serde(default)]
    pub caloric_mcal_per_kg: f64,
    #[serde(default)]
    pub potable_liters_per_kg: f64,
    #[serde(default)]
    pub heating_mj_per_kg: f64,
    #[serde(default)]
    pub is_clothing: bool,
    #[serde(default = "default_tool_hardness")]
    pub tool_hardness_rating: f32,
    #[serde(default)]
    pub is_slag_or_waste: bool,
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
    #[serde(default)]
    pub fallback_tool_commodity: Option<String>,
    #[serde(default)]
    pub waste_commodity: Option<String>,
    pub weather_halt_mud_threshold: f32,
    #[serde(default)]
    pub is_outdoor_construction: bool,
    #[serde(default)]
    pub is_underground_mine: bool,
    #[serde(default)]
    pub is_crop_cultivation: bool,
    #[serde(default)]
    pub is_mine_drainage: bool,
    #[serde(default = "default_process_min_temp")]
    pub min_temp_c: f32,
    #[serde(default = "default_process_max_temp")]
    pub max_temp_c: f32,
    #[serde(default = "default_process_opt_moisture")]
    pub optimal_moisture_01: f32,
    #[serde(default = "default_tool_wear_intensity")]
    pub tool_wear_intensity: f64,
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
    #[serde(default)]
    pub allowed_infra_classes: Vec<String>,
    #[serde(default)]
    pub is_watercraft: bool,
    #[serde(default)]
    pub uses_wind_sail: bool,
    #[serde(default = "default_max_ice_thickness")]
    pub max_ice_thickness_m: f32,
    #[serde(default)]
    pub spare_parts_commodity: Option<String>,
    #[serde(default)]
    pub spare_parts_wear_per_km: f64,
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
    #[serde(default = "default_daily_water_kg")]
    pub daily_water_need_kg: f64,
    #[serde(default = "default_daily_fuel_kg")]
    pub daily_fuel_need_kg_below_10c: f64,
    #[serde(default = "default_daily_clothing_kg")]
    pub daily_clothing_wear_kg: f64,
    #[serde(default = "default_dependents_share")]
    pub dependents_share_01: f32,
    #[serde(default = "default_elders_share")]
    pub elders_share_01: f32,
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
    #[serde(default = "default_roof_protection")]
    pub roof_protection_01: f32,
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
    #[serde(default)]
    pub owner_actor_id: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StartingSettlementRecord {
    pub id: String,
    pub display_name: String,
    pub cell_index: u32,
    pub initial_population: u32,
    #[serde(default)]
    pub initial_shaft_water_01: f32,
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
    #[serde(default = "default_half_f32")]
    pub initiative_01: f32,
    #[serde(default = "default_half_f32")]
    pub adaptability_01: f32,
    #[serde(default = "default_low_drive_f32")]
    pub ambition_01: f32,
    #[serde(default = "default_high_loyalty_f32")]
    pub loyalty_01: f32,
    #[serde(default = "default_low_drive_f32")]
    pub greed_skimming_01: f32,
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
