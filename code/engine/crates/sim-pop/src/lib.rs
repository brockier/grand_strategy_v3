//! Modular demographic cohorts, skill apprenticeship, character promotion, and social unrest cartridge.
//!
//! Runs in strictly O(cohorts) per hourly step using `BTreeMap` indices, conserving exact integer
//! headcounts across vital events and promotions, and communicates exclusively via `sim-spine`.

#![deny(clippy::disallowed_types)]

use std::collections::BTreeMap;

use sim_spine::codec::{
    read_f32_bits, read_f64_bits, read_str, read_u8, read_u16_le, read_u32_le, read_u64_le,
    write_f32_bits, write_f64_bits, write_str, write_u8, write_u16_le, write_u32_le, write_u64_le,
};
use sim_spine::ids::{CharacterId, CohortId, SettlementId, Tick, VocationId};
use sim_spine::ports::{
    CanonicalSaveablePort, CharacterSnapshotView, CohortDelta, CohortSnapshotView, ModRegistryPort,
    PopulationPort, ScenarioCharacterSeed, UnrestStage,
};

/// 6-attribute O(cohorts) population unit belonging to one settlement and occupational vocation.
#[derive(Clone, Debug, PartialEq)]
pub struct PopCohort {
    pub id: CohortId,
    pub settlement: SettlementId,
    pub vocation: VocationId,
    pub archetype_idx: u16,
    pub headcount: u32,
    pub dependents: u32,
    pub elders: u32,
    pub literacy_01: f32,
    pub culture_key: String,
    pub health_01: f32,
    pub nutrition_01: f32,
    pub mean_skill_01: f32,
    pub working_active: bool,
    pub fractional_birth_accum: f64,
    pub fractional_death_accum: f64,
}

impl PopCohort {
    /// Converts cohort state into lightweight Spine snapshot view.
    pub fn to_view(&self) -> CohortSnapshotView {
        CohortSnapshotView {
            id: self.id,
            settlement: self.settlement,
            vocation: self.vocation,
            headcount: self.headcount,
            mean_skill_01: self.mean_skill_01,
        }
    }

    /// Approximate working-age headcount available for labor assignment.
    pub fn workable_headcount(&self) -> u32 {
        self.headcount
            .saturating_sub(self.dependents.min(self.headcount / 2))
            .saturating_sub(self.elders.min(self.headcount / 4))
    }
}

/// Complete named character record including extended psychological axes used by `sim-command`.
#[derive(Clone, Debug, PartialEq)]
pub struct CharacterFullRecord {
    pub snapshot: CharacterSnapshotView,
    pub initiative_01: f32,
    pub adaptability_01: f32,
    pub ambition_01: f32,
    pub loyalty_01: f32,
    pub greed_skimming_01: f32,
    pub promoted_from_cohort: Option<CohortId>,
    pub alive: bool,
}

/// Per-settlement welfare, tax grievance, and progressive unrest status (`ENG-POP-005`).
#[derive(Clone, Debug, PartialEq)]
pub struct SettlementWelfareState {
    pub settlement: SettlementId,
    pub grievance_01: f32,
    pub nutrition_deficit_01: f32,
    pub cold_deficit_01: f32,
    pub tax_burden_01: f32,
    pub unrest_stage: UnrestStage,
    pub labor_efficiency_mult: f32,
    pub strike_hours: u32,
    pub out_migrated_headcount: u32,
    pub last_daily_grain_need_kg: f64,
    pub last_daily_water_need_kg: f64,
    pub last_daily_fuel_need_kg: f64,
    pub last_daily_clothing_need_kg: f64,
}

impl SettlementWelfareState {
    fn new(settlement: SettlementId) -> Self {
        Self {
            settlement,
            grievance_01: 0.04,
            nutrition_deficit_01: 0.0,
            cold_deficit_01: 0.0,
            tax_burden_01: 0.10,
            unrest_stage: UnrestStage::Compliant,
            labor_efficiency_mult: 1.0,
            strike_hours: 0,
            out_migrated_headcount: 0,
            last_daily_grain_need_kg: 0.0,
            last_daily_water_need_kg: 0.0,
            last_daily_fuel_need_kg: 0.0,
            last_daily_clothing_need_kg: 0.0,
        }
    }

    fn recompute_stage(&mut self) {
        let g = self.grievance_01.clamp(0.0, 1.0);
        let (stage, mult) = if g < 0.22 {
            (UnrestStage::Compliant, 1.0)
        } else if g < 0.42 {
            (UnrestStage::WorkSlowdown, 0.65)
        } else if g < 0.62 {
            (UnrestStage::LaborStrike, 0.20)
        } else if g < 0.78 {
            (UnrestStage::OutMigration, 0.25)
        } else if g < 0.90 {
            (UnrestStage::Banditry, 0.15)
        } else {
            (UnrestStage::LocalRevolt, 0.05)
        };
        self.unrest_stage = stage;
        self.labor_efficiency_mult = mult;
    }
}

/// Hourly physiological needs summary consumed by `sim-economy::step_subsistence_for_settlement`.
#[derive(Clone, Debug, Default)]
pub struct SettlementHourlyNeeds {
    pub grain_need_kg: f64,
    pub water_need_kg: f64,
    pub fuel_need_below_10c_kg: f64,
    pub clothing_need_kg: f64,
}

/// Outcomes returned from `PopulationRegistry::step_settlement_hourly`.
#[derive(Clone, Debug, Default)]
pub struct PopulationStepOutcome {
    pub births: u32,
    pub deaths: u32,
    pub unrest_stage: UnrestStage,
    pub previous_unrest_stage: UnrestStage,
    pub out_migrants_departed: u32,
}

fn splitmix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn unit_f32_from_hash(seed: u64) -> f32 {
    let bits = splitmix64(seed) >> 40;
    (bits as f32) / 16_777_216.0
}

fn unrest_to_u8(u: UnrestStage) -> u8 {
    u as u8
}

fn u8_to_unrest(v: u8) -> Result<UnrestStage, String> {
    match v {
        0 => Ok(UnrestStage::Compliant),
        1 => Ok(UnrestStage::WorkSlowdown),
        2 => Ok(UnrestStage::LaborStrike),
        3 => Ok(UnrestStage::OutMigration),
        4 => Ok(UnrestStage::Banditry),
        5 => Ok(UnrestStage::LocalRevolt),
        _ => Err(format!("Invalid UnrestStage tag {v}")),
    }
}

/// Deterministic population cohort, skill, promotion, and unrest cartridge.
#[derive(Clone, Debug, Default)]
pub struct PopulationRegistry {
    cohorts: Vec<PopCohort>,
    settlement_cohort_ids: BTreeMap<SettlementId, Vec<CohortId>>,
    welfare: BTreeMap<SettlementId, SettlementWelfareState>,
    characters_full: Vec<CharacterFullRecord>,
    character_views: Vec<CharacterSnapshotView>,
    next_cohort_id: u32,
    next_character_id: u32,
    initial_total_population: u64,
    cumulative_births: u64,
    cumulative_deaths: u64,
    cumulative_out_migrated: u64,
    cumulative_promoted: u64,
}

impl PopulationRegistry {
    /// Creates an empty population registry.
    pub fn new() -> Self {
        Self {
            cohorts: Vec::new(),
            settlement_cohort_ids: BTreeMap::new(),
            welfare: BTreeMap::new(),
            characters_full: Vec::new(),
            character_views: Vec::new(),
            next_cohort_id: 1,
            next_character_id: 1,
            initial_total_population: 0,
            cumulative_births: 0,
            cumulative_deaths: 0,
            cumulative_out_migrated: 0,
            cumulative_promoted: 0,
        }
    }

    /// Registers a settlement and seeds its starting occupational cohorts.
    pub fn seed_settlement_cohorts(
        &mut self,
        settlement: SettlementId,
        cohort_specs: &[(VocationId, u32, usize)],
        registry: &dyn ModRegistryPort,
        culture_key: &str,
        initial_skill_01: f32,
    ) {
        self.welfare
            .entry(settlement)
            .or_insert_with(|| SettlementWelfareState::new(settlement));
        for &(vocation, count, archetype_idx) in cohort_specs {
            self.add_cohort(
                settlement,
                vocation,
                archetype_idx as u16,
                count,
                registry,
                culture_key,
                initial_skill_01,
                true,
            );
        }
    }

    /// Adds a single demographic cohort and returns its allocated `CohortId`.
    #[allow(clippy::too_many_arguments)]
    pub fn add_cohort(
        &mut self,
        settlement: SettlementId,
        vocation: VocationId,
        archetype_idx: u16,
        headcount: u32,
        registry: &dyn ModRegistryPort,
        culture_key: &str,
        mean_skill_01: f32,
        record_as_initial: bool,
    ) -> CohortId {
        let id = CohortId(self.next_cohort_id);
        self.next_cohort_id += 1;

        let arch = registry.cohort_archetype_def(archetype_idx as usize);
        let dep_share = arch.map(|a| a.dependents_share_01).unwrap_or(0.24);
        let eld_share = arch.map(|a| a.elders_share_01).unwrap_or(0.10);
        let literacy = arch.map(|a| a.initial_literacy).unwrap_or(0.65);

        let dependents = ((f64::from(headcount) * f64::from(dep_share)).round() as u32).min(headcount);
        let elders = ((f64::from(headcount) * f64::from(eld_share)).round() as u32)
            .min(headcount.saturating_sub(dependents));

        let cohort = PopCohort {
            id,
            settlement,
            vocation,
            archetype_idx,
            headcount,
            dependents,
            elders,
            literacy_01: literacy.clamp(0.0, 1.0),
            culture_key: culture_key.to_owned(),
            health_01: 0.90,
            nutrition_01: 0.90,
            mean_skill_01: mean_skill_01.clamp(0.0, 1.0),
            working_active: true,
            fractional_birth_accum: 0.0,
            fractional_death_accum: 0.0,
        };
        self.cohorts.push(cohort);
        self.settlement_cohort_ids
            .entry(settlement)
            .or_default()
            .push(id);
        self.welfare
            .entry(settlement)
            .or_insert_with(|| SettlementWelfareState::new(settlement));
        if record_as_initial {
            self.initial_total_population += u64::from(headcount);
        }
        id
    }

    /// Registers a named subordinate from scenario configuration data.
    pub fn seed_character_from_scenario(
        &mut self,
        seed: &ScenarioCharacterSeed,
        home_settlement: SettlementId,
    ) -> CharacterId {
        let id = CharacterId(self.next_character_id);
        self.next_character_id += 1;
        let view = CharacterSnapshotView {
            id,
            key: seed.key.clone(),
            display_name: seed.display_name.clone(),
            role_title: seed.role_title.clone(),
            home_settlement,
            competence: seed.competence,
            precision_01: seed.precision_01,
            honesty_vs_sycophancy: seed.honesty_vs_sycophancy,
            caution_vs_alarmism: seed.caution_vs_alarmism,
        };
        self.characters_full.push(CharacterFullRecord {
            snapshot: view.clone(),
            initiative_01: seed.initiative_01,
            adaptability_01: seed.adaptability_01,
            ambition_01: seed.ambition_01,
            loyalty_01: seed.loyalty_01,
            greed_skimming_01: seed.greed_skimming_01,
            promoted_from_cohort: None,
            alive: true,
        });
        self.character_views.push(view);
        id
    }

    /// Promotes a qualified individual from an existing cohort into a named subordinate leader (`ENG-POP-004`).
    /// Decrements cohort integer headcount by 1 cleanly without duplicating people.
    pub fn promote_from_cohort(
        &mut self,
        cohort_id: CohortId,
        key: String,
        display_name: String,
        role_title: String,
        rng_salt: u64,
    ) -> Result<CharacterId, String> {
        let Some(cohort) = self.cohorts.iter_mut().find(|c| c.id == cohort_id) else {
            return Err(format!("Cohort {:?} does not exist", cohort_id));
        };
        if cohort.headcount == 0 {
            return Err(format!("Cohort {:?} has zero headcount to promote", cohort_id));
        }

        let settlement = cohort.settlement;
        let skill = cohort.mean_skill_01;
        let lit = cohort.literacy_01;
        cohort.headcount -= 1;
        // Track promotion separately so conservation audit balances exact headcount across cohorts + leaders
        self.cumulative_promoted += 1;

        let id = CharacterId(self.next_character_id);
        self.next_character_id += 1;

        let h_base = splitmix64(
            rng_salt
                ^ u64::from(cohort_id.0).wrapping_mul(0x517C_C1B7_2722_0A95)
                ^ u64::from(id.0),
        );
        let j1 = (unit_f32_from_hash(h_base) - 0.5) * 0.24;
        let j2 = (unit_f32_from_hash(h_base.wrapping_add(1)) - 0.5) * 0.28;
        let j3 = (unit_f32_from_hash(h_base.wrapping_add(2)) - 0.5) * 0.80;
        let j4 = (unit_f32_from_hash(h_base.wrapping_add(3)) - 0.5) * 0.80;

        let competence = (0.35 + 0.45 * skill + 0.20 * lit + j1).clamp(0.10, 0.98);
        let precision_01 = (0.30 + 0.35 * skill + 0.35 * lit + j2).clamp(0.10, 0.98);
        let honesty_vs_sycophancy = (0.20 + j3).clamp(-0.90, 0.90);
        let caution_vs_alarmism = (0.05 + j4).clamp(-0.90, 0.90);
        let initiative_01 =
            (0.40 + 0.30 * unit_f32_from_hash(h_base.wrapping_add(4))).clamp(0.10, 0.95);
        let adaptability_01 =
            (0.38 + 0.35 * unit_f32_from_hash(h_base.wrapping_add(5))).clamp(0.10, 0.95);
        let ambition_01 =
            (0.35 + 0.40 * unit_f32_from_hash(h_base.wrapping_add(6))).clamp(0.10, 0.95);
        let loyalty_01 =
            (0.45 + 0.35 * unit_f32_from_hash(h_base.wrapping_add(7))).clamp(0.15, 0.98);
        let greed_skimming_01 =
            (0.05 + 0.20 * unit_f32_from_hash(h_base.wrapping_add(8))).clamp(0.0, 0.65);

        let view = CharacterSnapshotView {
            id,
            key,
            display_name,
            role_title,
            home_settlement: settlement,
            competence,
            precision_01,
            honesty_vs_sycophancy,
            caution_vs_alarmism,
        };
        self.characters_full.push(CharacterFullRecord {
            snapshot: view.clone(),
            initiative_01,
            adaptability_01,
            ambition_01,
            loyalty_01,
            greed_skimming_01,
            promoted_from_cohort: Some(cohort_id),
            alive: true,
        });
        self.character_views.push(view);
        Ok(id)
    }

    /// Splits part of a cohort into a newly created destination settlement cohort upon physical migration arrival.
    pub fn migrate_cohort_segment(
        &mut self,
        source_cohort_id: CohortId,
        dest_settlement: SettlementId,
        headcount_moved: u32,
        registry: &dyn ModRegistryPort,
    ) -> Option<CohortId> {
        let idx = self.cohorts.iter().position(|c| c.id == source_cohort_id)?;
        let actual = headcount_moved.min(self.cohorts[idx].headcount);
        if actual == 0 {
            return None;
        }
        self.cohorts[idx].headcount -= actual;
        let vocation = self.cohorts[idx].vocation;
        let arch = self.cohorts[idx].archetype_idx;
        let culture = self.cohorts[idx].culture_key.clone();
        let skill = self.cohorts[idx].mean_skill_01;
        let dest_id = self.add_cohort(
            dest_settlement,
            vocation,
            arch,
            actual,
            registry,
            &culture,
            skill,
            false,
        );
        Some(dest_id)
    }

    /// Reads complete cohort struct.
    pub fn cohort(&self, id: CohortId) -> Option<&PopCohort> {
        self.cohorts.iter().find(|c| c.id == id)
    }

    /// Reads all cohorts in canonical order.
    pub fn all_cohorts(&self) -> &[PopCohort] {
        &self.cohorts
    }

    /// Sets working vs idle flag on a cohort so skill accrual vs idle decay reflects labor allocation.
    pub fn set_cohort_working(&mut self, id: CohortId, working: bool) {
        if let Some(c) = self.cohorts.iter_mut().find(|c| c.id == id) {
            c.working_active = working;
        }
    }

    /// Returns full extended character record.
    pub fn character_full(&self, id: CharacterId) -> Option<&CharacterFullRecord> {
        self.characters_full.iter().find(|c| c.snapshot.id == id)
    }

    /// Returns all extended character records.
    pub fn all_characters_full(&self) -> &[CharacterFullRecord] {
        &self.characters_full
    }

    /// Reads welfare and unrest state for a settlement.
    pub fn settlement_welfare(&self, sid: SettlementId) -> Option<&SettlementWelfareState> {
        self.welfare.get(&sid)
    }

    /// Sets taxation burden level (0..=1) impacting settlement grievance and unrest progression.
    pub fn set_settlement_tax_burden(&mut self, sid: SettlementId, tax_burden_01: f32) {
        let state = self
            .welfare
            .entry(sid)
            .or_insert_with(|| SettlementWelfareState::new(sid));
        state.tax_burden_01 = tax_burden_01.clamp(0.0, 1.0);
    }

    /// Applies an explicit grievance impulse from emergency requisition or welfare relief.
    pub fn adjust_settlement_grievance(&mut self, sid: SettlementId, delta: f32) {
        let state = self
            .welfare
            .entry(sid)
            .or_insert_with(|| SettlementWelfareState::new(sid));
        state.grievance_01 = (state.grievance_01 + delta).clamp(0.0, 1.0);
        state.recompute_stage();
    }

    /// Total living headcount across all active demographic cohorts.
    pub fn total_population(&self) -> u32 {
        self.cohorts.iter().map(|c| c.headcount).sum()
    }

    /// Total living headcount in a specific settlement.
    pub fn settlement_population(&self, sid: SettlementId) -> u32 {
        self.cohorts
            .iter()
            .filter(|c| c.settlement == sid)
            .map(|c| c.headcount)
            .sum()
    }

    /// Net integer population conservation residual (must equal `0` whenever accounting for all births,
    /// deaths, out-migration departures, and officer promotions from initial seeds).
    pub fn conservation_discrepancy(&self) -> i64 {
        let lhs = i64::from(self.total_population())
            + (self.cumulative_deaths as i64)
            + (self.cumulative_out_migrated as i64)
            + (self.cumulative_promoted as i64);
        let rhs = (self.initial_total_population as i64) + (self.cumulative_births as i64);
        lhs - rhs
    }

    /// Exposes current cumulative birth count.
    pub fn cumulative_births(&self) -> u64 {
        self.cumulative_births
    }

    /// Exposes current cumulative death count.
    pub fn cumulative_deaths(&self) -> u64 {
        self.cumulative_deaths
    }

    /// Computes total hourly physiological requirements for all cohorts in a settlement.
    pub fn compute_settlement_hourly_needs(
        &self,
        sid: SettlementId,
        ambient_temp_c: f32,
        registry: &dyn ModRegistryPort,
    ) -> SettlementHourlyNeeds {
        let mut needs = SettlementHourlyNeeds::default();
        let cold_active = ambient_temp_c < 10.0;
        let cold_severity = if cold_active {
            ((10.0 - ambient_temp_c) / 15.0).clamp(0.25, 2.0) as f64
        } else {
            0.0
        };
        for c in self.cohorts.iter().filter(|c| c.settlement == sid && c.headcount > 0) {
            let pop = f64::from(c.headcount);
            let arch = registry.cohort_archetype_def(c.archetype_idx as usize);
            let g_day = arch.map(|a| a.daily_grain_need_kg).unwrap_or(0.75);
            let w_day = arch.map(|a| a.daily_water_need_kg).unwrap_or(2.5);
            let f_day = arch.map(|a| a.daily_fuel_need_kg_below_10c).unwrap_or(1.8);
            let cl_day = arch.map(|a| a.daily_clothing_wear_kg).unwrap_or(0.02);

            needs.grain_need_kg += pop * g_day / 24.0;
            needs.water_need_kg += pop * w_day / 24.0;
            if cold_active {
                needs.fuel_need_below_10c_kg += pop * (f_day * cold_severity) / 24.0;
            }
            needs.clothing_need_kg += pop * cl_day / 24.0;
        }
        needs
    }

    /// Advances all cohorts in a settlement in O(cohorts) time given realized subsistence satisfaction.
    #[allow(clippy::too_many_arguments)]
    pub fn step_settlement_hourly(
        &mut self,
        sid: SettlementId,
        _tick: Tick,
        caloric_ratio_01: f32,
        water_ratio_01: f32,
        heating_ratio_01: f32,
        clothing_ratio_01: f32,
        ambient_temp_c: f32,
        registry: &dyn ModRegistryPort,
    ) -> PopulationStepOutcome {
        let mut outcome = PopulationStepOutcome::default();
        let nutrition_factor = (0.65 * caloric_ratio_01 + 0.35 * water_ratio_01).clamp(0.0, 1.2);
        let cold_exposure = if ambient_temp_c < 10.0 {
            ((1.0 - heating_ratio_01) * 0.70 + (1.0 - clothing_ratio_01) * 0.30).clamp(0.0, 1.0)
        } else {
            (1.0 - clothing_ratio_01) * 0.15
        };

        let mut total_headcount = 0u32;

        // O(cohorts) single linear pass across settlement cohorts
        for cohort in &mut self.cohorts {
            if cohort.settlement != sid || cohort.headcount == 0 {
                continue;
            }
            total_headcount += cohort.headcount;
            let arch = registry.cohort_archetype_def(cohort.archetype_idx as usize);
            let voc = registry.vocation_def(cohort.vocation);

            cohort.nutrition_01 = (cohort.nutrition_01 * 0.85 + nutrition_factor * 0.15).clamp(0.0, 1.0);
            let target_health = (0.70 * cohort.nutrition_01 + 0.30 * (1.0 - cold_exposure)).clamp(0.05, 1.0);
            cohort.health_01 = (cohort.health_01 * 0.90 + target_health * 0.10).clamp(0.05, 1.0);

            // Skill accrual & apprenticeship vs idle/elder-attrition decay (`ENG-POP-003`)
            let base_gain_hr = voc.map(|v| v.base_skill_gain_per_day).unwrap_or(0.003) / 24.0;
            let base_decay_hr = voc.map(|v| v.skill_decay_per_day).unwrap_or(0.0005) / 24.0;
            if cohort.working_active {
                let apprentice_bonus = 1.0 + 0.35 * cohort.literacy_01 + 0.25 * cohort.mean_skill_01;
                cohort.mean_skill_01 =
                    (cohort.mean_skill_01 + base_gain_hr * apprentice_bonus).clamp(0.0, 1.0);
            } else {
                cohort.mean_skill_01 = (cohort.mean_skill_01 - base_decay_hr * 2.5).clamp(0.0, 1.0);
            }

            // Annual rates converted deterministically to hourly continuous expectancy -> integer discrete events
            let base_fert = f64::from(arch.map(|a| a.base_fertility_annual).unwrap_or(0.032));
            let base_mort = f64::from(arch.map(|a| a.base_mortality_annual).unwrap_or(0.015));

            let fert_mult = f64::from((cohort.nutrition_01 * cohort.health_01).clamp(0.15, 1.25));
            let famine_stress = if caloric_ratio_01 < 0.60 {
                f64::from((0.60 - caloric_ratio_01) * 0.65)
            } else {
                0.0
            };
            let thirst_stress = if water_ratio_01 < 0.60 {
                f64::from((0.60 - water_ratio_01) * 0.85)
            } else {
                0.0
            };
            let exposure_stress = f64::from(cold_exposure) * 0.35;

            let hourly_births =
                f64::from(cohort.headcount) * (base_fert * fert_mult) / 8760.0;
            let hourly_deaths = f64::from(cohort.headcount)
                * ((base_mort / f64::from(cohort.health_01.max(0.25)))
                    + famine_stress
                    + thirst_stress
                    + exposure_stress)
                / 8760.0;

            cohort.fractional_birth_accum += hourly_births;
            cohort.fractional_death_accum += hourly_deaths;

            if cohort.fractional_birth_accum >= 1.0 {
                let births = cohort.fractional_birth_accum.floor() as u32;
                cohort.fractional_birth_accum -= f64::from(births);
                cohort.headcount += births;
                cohort.dependents += births;
                // New young dependents dilute cohort average skill slightly
                if cohort.headcount > 0 {
                    let prev = (cohort.headcount - births) as f32;
                    cohort.mean_skill_01 = (cohort.mean_skill_01 * prev) / (cohort.headcount as f32);
                }
                outcome.births += births;
                self.cumulative_births += u64::from(births);
            }

            if cohort.fractional_death_accum >= 1.0 && cohort.headcount > 0 {
                let raw_deaths = cohort.fractional_death_accum.floor() as u32;
                let deaths = raw_deaths.min(cohort.headcount);
                cohort.fractional_death_accum -= f64::from(deaths);

                // Subtract elders first (representing experienced elder loss & retirement attrition)
                let elder_deaths = deaths.min(cohort.elders);
                cohort.elders -= elder_deaths;
                let rem = deaths - elder_deaths;
                let dep_deaths = rem.min(cohort.dependents);
                cohort.dependents -= dep_deaths;
                cohort.headcount -= deaths;

                if elder_deaths > 0 && cohort.headcount > 0 {
                    // Losing elders pulls down cohort institutional experience slightly
                    cohort.mean_skill_01 =
                        (cohort.mean_skill_01 * (1.0 - 0.12 * (elder_deaths as f32 / cohort.headcount as f32)))
                            .clamp(0.0, 1.0);
                }

                outcome.deaths += deaths;
                self.cumulative_deaths += u64::from(deaths);
            }
        }

        let welfare = self
            .welfare
            .entry(sid)
            .or_insert_with(|| SettlementWelfareState::new(sid));
        outcome.previous_unrest_stage = welfare.unrest_stage;
        welfare.nutrition_deficit_01 = (1.0 - caloric_ratio_01.min(1.0)).max(0.0);
        welfare.cold_deficit_01 = cold_exposure;

        let tax_pressure = (welfare.tax_burden_01 - 0.25).max(0.0) * 0.045;
        let misery_rise = welfare.nutrition_deficit_01 * 0.018
            + (1.0 - water_ratio_01.min(1.0)) * 0.025
            + cold_exposure * 0.016
            + tax_pressure;

        if misery_rise > 0.0005 {
            welfare.grievance_01 = (welfare.grievance_01 + misery_rise).clamp(0.0, 1.0);
        } else if caloric_ratio_01 >= 0.95 && water_ratio_01 >= 0.95 && welfare.tax_burden_01 <= 0.30
        {
            welfare.grievance_01 = (welfare.grievance_01 - 0.004).clamp(0.0, 1.0);
        }

        welfare.recompute_stage();
        if welfare.unrest_stage == UnrestStage::LaborStrike {
            welfare.strike_hours += 1;
        }
        if matches!(
            welfare.unrest_stage,
            UnrestStage::OutMigration | UnrestStage::Banditry | UnrestStage::LocalRevolt
        ) && total_headcount > 20
        {
            // Periodic small desertion/out-migration attrition during acute stage 3+ unrest
            if welfare.strike_hours.is_multiple_of(24) {
                let mut lost = 0u32;
                for cohort in &mut self.cohorts {
                    if cohort.settlement == sid && cohort.headcount > 5 {
                        cohort.headcount -= 1;
                        if cohort.dependents > cohort.headcount {
                            cohort.dependents = cohort.headcount;
                        }
                        if cohort.elders > cohort.headcount.saturating_sub(cohort.dependents) {
                            cohort.elders = cohort.headcount.saturating_sub(cohort.dependents);
                        }
                        lost += 1;
                        self.cumulative_out_migrated += 1;
                        welfare.out_migrated_headcount += 1;
                        break;
                    }
                }
                outcome.out_migrants_departed = lost;
            }
        }
        outcome.unrest_stage = welfare.unrest_stage;
        outcome
    }

    /// Reads current labor multiplier resulting from settlement unrest stage.
    pub fn labor_efficiency_multiplier(&self, sid: SettlementId) -> f32 {
        self.welfare
            .get(&sid)
            .map(|w| w.labor_efficiency_mult)
            .unwrap_or(1.0)
    }
}

impl PopulationPort for PopulationRegistry {
    fn settlement_cohorts(&self, settlement: SettlementId) -> &[CohortId] {
        self.settlement_cohort_ids
            .get(&settlement)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn cohort_view(&self, id: CohortId) -> Option<CohortSnapshotView> {
        self.cohorts.iter().find(|c| c.id == id).map(PopCohort::to_view)
    }

    fn character_view(&self, id: CharacterId) -> Option<CharacterSnapshotView> {
        self.character_views.iter().find(|c| c.id == id).cloned()
    }

    fn all_characters(&self) -> &[CharacterSnapshotView] {
        &self.character_views
    }

    fn apply_labor_and_welfare(&mut self, updates: &[CohortDelta]) {
        for upd in updates {
            if let Some(c) = self.cohorts.iter_mut().find(|c| c.id == upd.cohort) {
                if upd.delta_headcount >= 0 {
                    c.headcount = c.headcount.saturating_add(upd.delta_headcount as u32);
                    self.initial_total_population += upd.delta_headcount as u64;
                } else {
                    let dec = upd.delta_headcount.unsigned_abs().min(c.headcount);
                    c.headcount -= dec;
                    self.cumulative_out_migrated += u64::from(dec);
                }
                c.mean_skill_01 = (c.mean_skill_01 + upd.delta_skill).clamp(0.0, 1.0);
            }
        }
    }

    fn unrest_stage_for_settlement(&self, settlement: SettlementId) -> UnrestStage {
        self.welfare
            .get(&settlement)
            .map(|w| w.unrest_stage)
            .unwrap_or(UnrestStage::Compliant)
    }
}

impl CanonicalSaveablePort for PopulationRegistry {
    fn cartridge_tag(&self) -> &'static str {
        "sim-pop"
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        write_u32_le(out, self.next_cohort_id);
        write_u32_le(out, self.next_character_id);
        write_u64_le(out, self.initial_total_population);
        write_u64_le(out, self.cumulative_births);
        write_u64_le(out, self.cumulative_deaths);
        write_u64_le(out, self.cumulative_out_migrated);
        write_u64_le(out, self.cumulative_promoted);

        write_u32_le(out, self.cohorts.len() as u32);
        for c in &self.cohorts {
            write_u32_le(out, c.id.0);
            write_u32_le(out, c.settlement.0);
            write_u16_le(out, c.vocation.0);
            write_u16_le(out, c.archetype_idx);
            write_u32_le(out, c.headcount);
            write_u32_le(out, c.dependents);
            write_u32_le(out, c.elders);
            write_f32_bits(out, c.literacy_01);
            write_str(out, &c.culture_key);
            write_f32_bits(out, c.health_01);
            write_f32_bits(out, c.nutrition_01);
            write_f32_bits(out, c.mean_skill_01);
            write_u8(out, u8::from(c.working_active));
            write_f64_bits(out, c.fractional_birth_accum);
            write_f64_bits(out, c.fractional_death_accum);
        }

        write_u32_le(out, self.welfare.len() as u32);
        for w in self.welfare.values() {
            write_u32_le(out, w.settlement.0);
            write_f32_bits(out, w.grievance_01);
            write_f32_bits(out, w.nutrition_deficit_01);
            write_f32_bits(out, w.cold_deficit_01);
            write_f32_bits(out, w.tax_burden_01);
            write_u8(out, unrest_to_u8(w.unrest_stage));
            write_f32_bits(out, w.labor_efficiency_mult);
            write_u32_le(out, w.strike_hours);
            write_u32_le(out, w.out_migrated_headcount);
        }

        write_u32_le(out, self.characters_full.len() as u32);
        for ch in &self.characters_full {
            write_u32_le(out, ch.snapshot.id.0);
            write_str(out, &ch.snapshot.key);
            write_str(out, &ch.snapshot.display_name);
            write_str(out, &ch.snapshot.role_title);
            write_u32_le(out, ch.snapshot.home_settlement.0);
            write_f32_bits(out, ch.snapshot.competence);
            write_f32_bits(out, ch.snapshot.precision_01);
            write_f32_bits(out, ch.snapshot.honesty_vs_sycophancy);
            write_f32_bits(out, ch.snapshot.caution_vs_alarmism);
            write_f32_bits(out, ch.initiative_01);
            write_f32_bits(out, ch.adaptability_01);
            write_f32_bits(out, ch.ambition_01);
            write_f32_bits(out, ch.loyalty_01);
            write_f32_bits(out, ch.greed_skimming_01);
            write_u8(out, u8::from(ch.promoted_from_cohort.is_some()));
            write_u32_le(out, ch.promoted_from_cohort.map(|x| x.0).unwrap_or(0));
            write_u8(out, u8::from(ch.alive));
        }
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        self.next_cohort_id = read_u32_le(input)?;
        self.next_character_id = read_u32_le(input)?;
        self.initial_total_population = read_u64_le(input)?;
        self.cumulative_births = read_u64_le(input)?;
        self.cumulative_deaths = read_u64_le(input)?;
        self.cumulative_out_migrated = read_u64_le(input)?;
        self.cumulative_promoted = read_u64_le(input)?;

        let n_cohorts = read_u32_le(input)? as usize;
        self.cohorts.clear();
        self.settlement_cohort_ids.clear();
        for _ in 0..n_cohorts {
            let id = CohortId(read_u32_le(input)?);
            let settlement = SettlementId(read_u32_le(input)?);
            let vocation = VocationId(read_u16_le(input)?);
            let archetype_idx = read_u16_le(input)?;
            let headcount = read_u32_le(input)?;
            let dependents = read_u32_le(input)?;
            let elders = read_u32_le(input)?;
            let literacy_01 = read_f32_bits(input)?;
            let culture_key = read_str(input)?;
            let health_01 = read_f32_bits(input)?;
            let nutrition_01 = read_f32_bits(input)?;
            let mean_skill_01 = read_f32_bits(input)?;
            let working_active = read_u8(input)? != 0;
            let fractional_birth_accum = read_f64_bits(input)?;
            let fractional_death_accum = read_f64_bits(input)?;
            self.settlement_cohort_ids
                .entry(settlement)
                .or_default()
                .push(id);
            self.cohorts.push(PopCohort {
                id,
                settlement,
                vocation,
                archetype_idx,
                headcount,
                dependents,
                elders,
                literacy_01,
                culture_key,
                health_01,
                nutrition_01,
                mean_skill_01,
                working_active,
                fractional_birth_accum,
                fractional_death_accum,
            });
        }

        let n_welfare = read_u32_le(input)? as usize;
        self.welfare.clear();
        for _ in 0..n_welfare {
            let settlement = SettlementId(read_u32_le(input)?);
            let grievance_01 = read_f32_bits(input)?;
            let nutrition_deficit_01 = read_f32_bits(input)?;
            let cold_deficit_01 = read_f32_bits(input)?;
            let tax_burden_01 = read_f32_bits(input)?;
            let unrest_stage = u8_to_unrest(read_u8(input)?)?;
            let labor_efficiency_mult = read_f32_bits(input)?;
            let strike_hours = read_u32_le(input)?;
            let out_migrated_headcount = read_u32_le(input)?;
            self.welfare.insert(
                settlement,
                SettlementWelfareState {
                    settlement,
                    grievance_01,
                    nutrition_deficit_01,
                    cold_deficit_01,
                    tax_burden_01,
                    unrest_stage,
                    labor_efficiency_mult,
                    strike_hours,
                    out_migrated_headcount,
                    last_daily_grain_need_kg: 0.0,
                    last_daily_water_need_kg: 0.0,
                    last_daily_fuel_need_kg: 0.0,
                    last_daily_clothing_need_kg: 0.0,
                },
            );
        }

        let n_chars = read_u32_le(input)? as usize;
        self.characters_full.clear();
        self.character_views.clear();
        for _ in 0..n_chars {
            let id = CharacterId(read_u32_le(input)?);
            let key = read_str(input)?;
            let display_name = read_str(input)?;
            let role_title = read_str(input)?;
            let home_settlement = SettlementId(read_u32_le(input)?);
            let competence = read_f32_bits(input)?;
            let precision_01 = read_f32_bits(input)?;
            let honesty_vs_sycophancy = read_f32_bits(input)?;
            let caution_vs_alarmism = read_f32_bits(input)?;
            let initiative_01 = read_f32_bits(input)?;
            let adaptability_01 = read_f32_bits(input)?;
            let ambition_01 = read_f32_bits(input)?;
            let loyalty_01 = read_f32_bits(input)?;
            let greed_skimming_01 = read_f32_bits(input)?;
            let has_prom = read_u8(input)? != 0;
            let prom = CohortId(read_u32_le(input)?);
            let alive = read_u8(input)? != 0;

            let view = CharacterSnapshotView {
                id,
                key,
                display_name,
                role_title,
                home_settlement,
                competence,
                precision_01,
                honesty_vs_sycophancy,
                caution_vs_alarmism,
            };
            self.character_views.push(view.clone());
            self.characters_full.push(CharacterFullRecord {
                snapshot: view,
                initiative_01,
                adaptability_01,
                ambition_01,
                loyalty_01,
                greed_skimming_01,
                promoted_from_cohort: has_prom.then_some(prom),
                alive,
            });
        }

        Ok(())
    }
}
