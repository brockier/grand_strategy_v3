//! Seeded procedural catalog generation of world ores, impurities, fluxes, fuels, crops, fauna, and pathogens (`ENG-GEO-005`).

use rand_chacha::ChaCha8Rng;
use rand_core::RngCore;
use sim_spine::{NaturalBoundsView, ProceduralMaterialId};

#[inline]
fn next_unit_f32(rng: &mut ChaCha8Rng) -> f32 {
    let bits = (rng.next_u64() >> 40) as u32;
    (bits as f32) / ((1u32 << 24) as f32)
}

#[inline]
fn sample_range(rng: &mut ChaCha8Rng, bounds: [f32; 2]) -> f32 {
    let lo = bounds[0].min(bounds[1]);
    let hi = bounds[0].max(bounds[1]);
    lo + (hi - lo) * next_unit_f32(rng)
}

/// Procedural metallic ore definition sampled per world seed.
#[derive(Clone, Debug, PartialEq)]
pub struct ProceduralOreDef {
    pub id: ProceduralMaterialId,
    pub key: String,
    pub melting_point_k: f32,
    pub reduction_enthalpy_mj_kg: f32,
    pub base_grade_01: f32,
    pub trace_impurity_01: f32,
    pub max_recoverable_yield_01: f32,
    pub optimal_flux_id: ProceduralMaterialId,
    pub optimal_alloy_tensile_mpa: f32,
    pub brittleness_penalty_without_flux_01: f32,
}

/// Procedural smelting flux neutralizing specific ore impurities.
#[derive(Clone, Debug, PartialEq)]
pub struct ProceduralFluxDef {
    pub id: ProceduralMaterialId,
    pub key: String,
    pub basicity_index: f32,
    pub slag_melting_k: f32,
}

/// Procedural fuel definition honoring combustion thermodynamic ceilings.
#[derive(Clone, Debug, PartialEq)]
pub struct ProceduralFuelDef {
    pub id: ProceduralMaterialId,
    pub key: String,
    pub is_liquid: bool,
    pub calorific_enthalpy_mj_kg: f32,
    pub max_thermal_efficiency_01: f32,
    pub useful_energy_mj_kg: f32,
    pub ash_fouling_01: f32,
}

/// Procedural edible/industrial crop definition.
#[derive(Clone, Debug, PartialEq)]
pub struct ProceduralCropDef {
    pub id: ProceduralMaterialId,
    pub key: String,
    pub optimal_temp_c: f32,
    pub frost_lethal_c: f32,
    pub soil_moisture_pref_01: f32,
    pub yield_kg_per_ha: f32,
}

/// Procedural wild or draft fauna species.
#[derive(Clone, Debug, PartialEq)]
pub struct ProceduralFaunaDef {
    pub id: ProceduralMaterialId,
    pub key: String,
    pub draft_power_watts: f32,
    pub daily_fodder_kg: f32,
    pub wild_hazard_01: f32,
}

/// Procedural endemic pathogen constraining settlement sanitation.
#[derive(Clone, Debug, PartialEq)]
pub struct ProceduralPathogenDef {
    pub id: ProceduralMaterialId,
    pub key: String,
    pub incubation_hours: u16,
    pub base_r0: f32,
    pub case_fatality_01: f32,
}

/// World-seeded catalog of generated materials, crops, fauna, and pathogens (`ENG-GEO-005`).
#[derive(Clone, Debug, PartialEq)]
pub struct ProceduralMaterialCatalog {
    pub ores: Vec<ProceduralOreDef>,
    pub fluxes: Vec<ProceduralFluxDef>,
    pub fuels: Vec<ProceduralFuelDef>,
    pub crops: Vec<ProceduralCropDef>,
    pub fauna: Vec<ProceduralFaunaDef>,
    pub pathogens: Vec<ProceduralPathogenDef>,
}

impl ProceduralMaterialCatalog {
    /// Generates bounded procedural natural tables deterministically from `rng` and mod physical bounds.
    #[must_use]
    pub fn generate(rng: &mut ChaCha8Rng, bounds: &NaturalBoundsView) -> Self {
        let mut next_id = 0u16;
        let mut alloc_id = || {
            let id = ProceduralMaterialId(next_id);
            next_id += 1;
            id
        };

        let flux_count = usize::from(bounds.flux_count.max(1));
        let mut fluxes = Vec::with_capacity(flux_count);
        for idx in 0..flux_count {
            let id = alloc_id();
            fluxes.push(ProceduralFluxDef {
                id,
                key: format!("flux_mineral_{idx}"),
                basicity_index: sample_range(rng, [0.35, 1.45]),
                slag_melting_k: sample_range(rng, [1050.0, 1520.0]),
            });
        }

        let ore_count = usize::from(bounds.ore_count.max(1));
        let mut ores = Vec::with_capacity(ore_count);
        for idx in 0..ore_count {
            let id = alloc_id();
            let base_grade_01 = sample_range(rng, bounds.ore_purity_range).clamp(0.05, 0.90);
            let trace_impurity_01 = sample_range(rng, [0.02, (1.0 - base_grade_01).min(0.28)]);
            // Conservation of mass invariant: extractable metal yield <= ore grade minus unavoidable loss.
            let max_recoverable_yield_01 = (base_grade_01 * (1.0 - trace_impurity_01 * 0.4))
                .min(1.0 - trace_impurity_01)
                .clamp(0.01, base_grade_01);
            let flux_choice = fluxes[(rng.next_u64() as usize) % fluxes.len()].id;
            let melting_point_k = sample_range(rng, bounds.ore_melting_k_range).max(600.0);
            let tensile = sample_range(rng, [240.0, 920.0]);
            ores.push(ProceduralOreDef {
                id,
                key: format!("metal_ore_{idx}"),
                melting_point_k,
                reduction_enthalpy_mj_kg: sample_range(rng, [4.5, 16.0]),
                base_grade_01,
                trace_impurity_01,
                max_recoverable_yield_01,
                optimal_flux_id: flux_choice,
                optimal_alloy_tensile_mpa: tensile,
                brittleness_penalty_without_flux_01: sample_range(rng, [0.20, 0.72]),
            });
        }

        let fuel_count = usize::from(bounds.fuel_count.max(1));
        let mut fuels = Vec::with_capacity(fuel_count);
        for idx in 0..fuel_count {
            let id = alloc_id();
            let calorific = sample_range(rng, bounds.fuel_enthalpy_mj_kg_range).max(2.0);
            let eff = sample_range(rng, [0.58, 0.94]);
            fuels.push(ProceduralFuelDef {
                id,
                key: format!("fossil_fuel_{idx}"),
                is_liquid: idx % 2 == 1,
                calorific_enthalpy_mj_kg: calorific,
                max_thermal_efficiency_01: eff,
                // First/Second-law bound: useful energy strictly <= calorific enthalpy.
                useful_energy_mj_kg: calorific * eff,
                ash_fouling_01: sample_range(rng, [0.01, 0.32]),
            });
        }

        let crop_count = usize::from(bounds.crop_count.max(1));
        let mut crops = Vec::with_capacity(crop_count);
        for idx in 0..crop_count {
            let id = alloc_id();
            let opt_t = sample_range(rng, bounds.crop_opt_temp_c_range);
            crops.push(ProceduralCropDef {
                id,
                key: format!("cultivar_{idx}"),
                optimal_temp_c: opt_t,
                frost_lethal_c: (opt_t - sample_range(rng, [14.0, 26.0])).min(2.0),
                soil_moisture_pref_01: sample_range(rng, [0.30, 0.78]),
                yield_kg_per_ha: sample_range(rng, [950.0, 4200.0]),
            });
        }

        let fauna_count = usize::from(bounds.fauna_count.max(1));
        let mut fauna = Vec::with_capacity(fauna_count);
        for idx in 0..fauna_count {
            let id = alloc_id();
            fauna.push(ProceduralFaunaDef {
                id,
                key: format!("fauna_{idx}"),
                draft_power_watts: sample_range(rng, [220.0, 980.0]),
                daily_fodder_kg: sample_range(rng, [4.0, 24.0]),
                wild_hazard_01: sample_range(rng, [0.05, 0.65]),
            });
        }

        let pathogen_count = usize::from(bounds.pathogen_count.max(1));
        let mut pathogens = Vec::with_capacity(pathogen_count);
        for idx in 0..pathogen_count {
            let id = alloc_id();
            pathogens.push(ProceduralPathogenDef {
                id,
                key: format!("pathogen_{idx}"),
                incubation_hours: 12 + ((rng.next_u64() % 120) as u16),
                base_r0: sample_range(rng, [0.8, 3.6]),
                case_fatality_01: sample_range(rng, [0.01, 0.18]),
            });
        }

        Self {
            ores,
            fluxes,
            fuels,
            crops,
            fauna,
            pathogens,
        }
    }
}
