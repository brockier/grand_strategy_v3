#![deny(clippy::disallowed_types)]

//! Hourly double-buffered spherical atmosphere advection, orbital insolation,
//! precipitation/snow/mud, and conservative topological watershed flood routing
//! (`ENG-WTH-001` through `ENG-WTH-003`).

use std::collections::BTreeMap;
use std::f64::consts::PI;

use sim_spine::{
    codec::{
        read_f32_bits, read_f64_bits, read_str, read_u32_le, read_u64_le, read_u8, write_f32_bits,
        write_f64_bits, write_str, write_u32_le, write_u64_le, write_u8,
    },
    CalendarConfigView, CanonicalSaveablePort, CellId, DamRuleCurveKind, DamStatusView,
    FluidDefView, HydrologyRulesView, LakeKind, PlanetGenConfigView, PlanetGeometryPort,
    RiverNavigabilityKind, SurfaceConditionSnapshot, SurfaceConditionsPort, Tick,
};

#[inline]
fn dot3_f64(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
fn norm3(v: [f64; 3]) -> [f64; 3] {
    let len = dot3_f64(v, v).sqrt().max(1e-15);
    [v[0] / len, v[1] / len, v[2] / len]
}

fn tangent_basis(p: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let pole = if p[1].abs() < 0.99 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let east = norm3(cross3(pole, p));
    let north = norm3(cross3(p, east));
    (east, north)
}

/// Internal dam and reservoir facility operational state (`ENG-GEO-014`).
#[derive(Clone, Debug, PartialEq)]
struct DamFacilityState {
    cell_idx: u32,
    rule_curve: DamRuleCurveKind,
    reservoir_storage_m3: f64,
    reservoir_capacity_m3: f64,
    reservoir_head_m: f32,
    spillway_release_m3_s: f32,
    live_hydro_kw: f32,
    sediment_silt_01: f32,
    condition_01: f32,
    breached: bool,
    impounded_upstream_cells: Vec<CellId>,
}

impl DamFacilityState {
    fn to_view(&self) -> DamStatusView {
        let fill_01 = if self.reservoir_capacity_m3 > 1.0 {
            (self.reservoir_storage_m3 / self.reservoir_capacity_m3).clamp(0.0, 1.0) as f32
        } else {
            0.0
        };
        DamStatusView {
            cell_id: CellId(self.cell_idx),
            rule_curve: self.rule_curve,
            reservoir_storage_m3: self.reservoir_storage_m3,
            reservoir_capacity_m3: self.reservoir_capacity_m3,
            reservoir_fill_01: fill_01,
            reservoir_head_m: self.reservoir_head_m,
            spillway_release_m3_s: self.spillway_release_m3_s,
            live_hydro_kw: self.live_hydro_kw,
            sediment_silt_01: self.sediment_silt_01,
            condition_01: self.condition_01,
            breached: self.breached,
            impounded_upstream_cells: self.impounded_upstream_cells.clone(),
        }
    }
}

/// Hourly planetary weather and surface hydrology cartridge (`ENG-WTH-001`..`006`, `ENG-GEO-014`).
#[derive(Clone, Debug, PartialEq)]
pub struct WeatherEngine {
    cell_count: usize,
    hours_per_year: u32,
    axial_tilt_rad: f64,
    solar_flux_scale: f64,
    rotation_period_hours: f64,
    base_sea_level_m: f32,
    fluid: FluidDefView,
    rules: HydrologyRulesView,

    // Precomputed static per-cell spherical/hydrologic geometry (read-only after `new`).
    sin_lat: Vec<f32>,
    cos_lat: Vec<f32>,
    lon_rad: Vec<f32>,
    elev_above_sea_m: Vec<f32>,
    is_ocean: Vec<bool>,
    area_m2: Vec<f64>,
    soil_cap_m3: Vec<f64>,
    gw_cap_m3: Vec<f64>,
    soil_drainage_01: Vec<f32>,
    fault_instability_01: Vec<f32>,
    lake_kind: Vec<LakeKind>,
    base_lake_salinity: Vec<f32>,
    base_lake_vol_m3: Vec<f64>,
    has_perennial_channel: Vec<bool>,
    base_channel_discharge_m3_s: Vec<f32>,
    base_channel_slope_rad: Vec<f32>,
    channel_bankfull_m3: Vec<f64>,
    runoff_k: Vec<f32>,
    downstream_idx: Vec<Option<usize>>,
    topo_order: Vec<usize>,
    land_adj_pairs: Vec<(usize, usize)>,
    upwind_offsets: Vec<u32>,
    upwind_nbrs: Vec<u32>,
    upwind_weights: Vec<f32>,
    base_wind_u: Vec<f32>,
    base_wind_v: Vec<f32>,
    annual_precip_norm: Vec<f32>,

    // Mutable SoA weather & hydrology state.
    air_temp_c: Vec<f32>,
    vapor_kg_m2: Vec<f32>,
    wind_u_mps: Vec<f32>,
    wind_v_mps: Vec<f32>,
    cloud_01: Vec<f32>,
    precip_mm_hr: Vec<f32>,
    mud_index_01: Vec<f32>,
    flood_stage_01: Vec<f32>,
    river_ice_m: Vec<f32>,
    visibility_01: Vec<f32>,
    is_spring_oasis: Vec<bool>,
    live_salinity_per_mille: Vec<f32>,
    live_discharge_m3_s: Vec<f32>,
    live_navigability: Vec<RiverNavigabilityKind>,

    // Exact cubic-meter water conservation pools on land cells.
    surface_water_m3: Vec<f64>,
    soil_water_m3: Vec<f64>,
    snow_water_m3: Vec<f64>,
    groundwater_m3: Vec<f64>,
    dams: BTreeMap<u32, DamFacilityState>,

    // Reusable double buffers avoiding allocations during hourly ticks.
    scratch_temp_c: Vec<f32>,
    scratch_vapor_kg_m2: Vec<f32>,
    scratch_gw_head: Vec<f32>,

    // Conservation ledger in cubic meters across land territory.
    initial_land_water_m3: f64,
    cumulative_precip_m3: f64,
    cumulative_storm_injected_m3: f64,
    cumulative_evap_m3: f64,
    cumulative_ocean_outflow_m3: f64,
}

impl WeatherEngine {
    /// Initializes precomputed upwind spherical transport weights and baseline hydrological pools.
    #[must_use]
    pub fn new(
        planet: &dyn PlanetGeometryPort,
        planet_cfg: &PlanetGenConfigView,
        cal_cfg: &CalendarConfigView,
    ) -> Self {
        let n = planet.cell_count() as usize;
        let hours_per_year = cal_cfg
            .hours_per_day
            .max(1)
            .saturating_mul(cal_cfg.days_per_year.max(1));
        let axial_tilt_rad = f64::from(planet_cfg.axial_tilt_deg).to_radians();
        let solar_flux_scale = f64::from(planet_cfg.solar_constant_w_m2 / 1361.0).clamp(0.4, 2.5);
        let rotation_period_hours = f64::from(planet_cfg.rotation_period_hours.max(1.0));
        let base_sea_level_m = planet_cfg.base_sea_level_m;
        let rules = planet_cfg.hydrology_rules.clone();
        let fluid = planet_cfg
            .fluids
            .iter()
            .find(|f| f.id == rules.active_fluid_id)
            .cloned()
            .unwrap_or_default();

        let mut sin_lat = Vec::with_capacity(n);
        let mut cos_lat = Vec::with_capacity(n);
        let mut lon_rad = Vec::with_capacity(n);
        let mut elev_above_sea_m = Vec::with_capacity(n);
        let mut is_ocean = Vec::with_capacity(n);
        let mut area_m2 = Vec::with_capacity(n);
        let mut soil_cap_m3 = Vec::with_capacity(n);
        let mut gw_cap_m3 = Vec::with_capacity(n);
        let mut soil_drainage_01 = Vec::with_capacity(n);
        let mut fault_instability_01 = Vec::with_capacity(n);
        let mut lake_kind = Vec::with_capacity(n);
        let mut base_lake_salinity = Vec::with_capacity(n);
        let mut base_lake_vol_m3 = Vec::with_capacity(n);
        let mut has_perennial_channel = Vec::with_capacity(n);
        let mut base_channel_discharge_m3_s = Vec::with_capacity(n);
        let mut base_channel_slope_rad = Vec::with_capacity(n);
        let mut channel_bankfull_m3 = Vec::with_capacity(n);
        let mut runoff_k = Vec::with_capacity(n);
        let mut downstream_idx = Vec::with_capacity(n);
        let mut base_wind_u = Vec::with_capacity(n);
        let mut base_wind_v = Vec::with_capacity(n);
        let mut annual_precip_norm = Vec::with_capacity(n);
        let mut land_adj_pairs = Vec::new();

        for raw in 0..planet.cell_count() {
            let cid = CellId(raw);
            let idx = raw as usize;
            let [lat, lon] = planet.cell_lat_lon_rad(cid);
            sin_lat.push(lat.sin());
            cos_lat.push(lat.cos().max(0.0));
            lon_rad.push(lon);

            let geo = planet.cell_biome_and_strata(cid);
            is_ocean.push(geo.is_ocean);
            let elev_above = if geo.is_ocean {
                0.0
            } else {
                (geo.elevation_m - base_sea_level_m).max(0.0)
            };
            elev_above_sea_m.push(elev_above);

            let cell_m2 = f64::from(planet.cell_area_km2(cid).max(1.0)) * 1_000_000.0;
            area_m2.push(cell_m2);

            let drain_km2 = planet.cell_drainage_area_km2(cid).max(planet.cell_area_km2(cid));
            soil_cap_m3.push(cell_m2 * 0.35);
            gw_cap_m3.push(cell_m2 * f64::from(rules.aquifer_capacity_mm.max(40.0)) * 0.001);
            soil_drainage_01.push(planet.cell_soil_drainage_01(cid).clamp(0.05, 1.0));
            fault_instability_01.push(planet.cell_fault_instability_01(cid).clamp(0.0, 1.0));
            let lk = planet.cell_lake_kind(cid);
            lake_kind.push(lk);
            base_lake_salinity.push(planet.cell_lake_salinity_per_mille(cid));
            let bankfull = f64::from(drain_km2) * 18_000.0;
            base_lake_vol_m3.push((bankfull * 0.25).max(1.0));
            channel_bankfull_m3.push(bankfull);
            has_perennial_channel.push(planet.cell_river_channel_downstream(cid).is_some());
            base_channel_discharge_m3_s.push(planet.cell_river_discharge_m3_s(cid));

            let ds = planet.watershed_downstream(cid).map(|d| d.get() as usize);
            downstream_idx.push(ds);
            let slope = ds
                .and_then(|d_idx| planet.cell_edge(cid, CellId(d_idx as u32)))
                .map_or(0.002, |eid| planet.cell_slope_rad(eid));
            base_channel_slope_rad.push(slope);
            runoff_k.push((0.18 + slope * 8.0).clamp(0.14, 0.55));

            let abs_lat = f64::from(lat).abs();
            let u0 = (-(4.0 * abs_lat).cos() * 9.5) as f32;
            let v0 = ((2.0 * abs_lat).sin() * f64::from(lat).signum() * 3.0) as f32;
            base_wind_u.push(u0);
            base_wind_v.push(v0);

            let p_norm = (planet.cell_base_precip_mm_yr(cid) / 950.0).clamp(0.18, 2.4);
            annual_precip_norm.push(p_norm);

            for &nb in planet.cell_neighbors(cid) {
                let nb_idx = nb.get() as usize;
                if idx < nb_idx && !geo.is_ocean && !planet.cell_biome_and_strata(nb).is_ocean {
                    land_adj_pairs.push((idx, nb_idx));
                }
            }
        }

        let topo_order: Vec<usize> = planet
            .topo_sorted_land_cells()
            .iter()
            .map(|c| c.get() as usize)
            .collect();

        // Precompute normalized upwind neighbor advection weights once from prevailing sphere geometry.
        let mut upwind_offsets = Vec::with_capacity(n + 1);
        let mut upwind_nbrs = Vec::with_capacity(n * 3);
        let mut upwind_weights = Vec::with_capacity(n * 3);
        for raw in 0..planet.cell_count() {
            let cid = CellId(raw);
            let idx = raw as usize;
            upwind_offsets.push(upwind_nbrs.len() as u32);
            let pos = planet.cell_unit_xyz(cid);
            let (east, north) = tangent_basis([f64::from(pos[0]), f64::from(pos[1]), f64::from(pos[2])]);
            let wind_3d = norm3([
                east[0] * f64::from(base_wind_u[idx]) + north[0] * f64::from(base_wind_v[idx]),
                east[1] * f64::from(base_wind_u[idx]) + north[1] * f64::from(base_wind_v[idx]),
                east[2] * f64::from(base_wind_u[idx]) + north[2] * f64::from(base_wind_v[idx]),
            ]);

            let mut local: Vec<(u32, f32)> = Vec::new();
            let mut sum_w = 0.0_f32;
            for &nb in planet.cell_neighbors(cid) {
                let np = planet.cell_unit_xyz(nb);
                let dir = norm3([
                    f64::from(pos[0] - np[0]),
                    f64::from(pos[1] - np[1]),
                    f64::from(pos[2] - np[2]),
                ]);
                let align = dot3_f64(dir, wind_3d) as f32;
                if align > 0.05 {
                    local.push((nb.get(), align));
                    sum_w += align;
                }
            }
            if sum_w > 0.0 {
                for (nb_u32, w) in local {
                    upwind_nbrs.push(nb_u32);
                    upwind_weights.push(w / sum_w);
                }
            }
        }
        upwind_offsets.push(upwind_nbrs.len() as u32);

        // Initial baseline fields matched to orbital vernal equinox equilibrium.
        let mut air_temp_c = Vec::with_capacity(n);
        let mut vapor_kg_m2 = Vec::with_capacity(n);
        let mut cloud_01 = Vec::with_capacity(n);
        let precip_mm_hr = vec![0.0_f32; n];
        let mut mud_index_01 = Vec::with_capacity(n);
        let flood_stage_01 = vec![0.0_f32; n];
        let mut river_ice_m = Vec::with_capacity(n);
        let visibility_01 = vec![0.95_f32; n];

        let mut surface_water_m3 = Vec::with_capacity(n);
        let mut soil_water_m3 = Vec::with_capacity(n);
        let mut snow_water_m3 = Vec::with_capacity(n);
        let mut groundwater_m3 = Vec::with_capacity(n);
        let is_spring_oasis = vec![false; n];
        let live_salinity_per_mille = base_lake_salinity.clone();
        let live_discharge_m3_s = base_channel_discharge_m3_s.clone();
        let mut live_navigability = Vec::with_capacity(n);
        let mut initial_land_water_m3 = 0.0_f64;

        for idx in 0..n {
            live_navigability.push(planet.cell_river_navigability(CellId(idx as u32)));
            let lapse = elev_above_sea_m[idx] * 0.0065;
            let eq_t = ((30.5 * f64::from(cos_lat[idx]) - 3.5) * solar_flux_scale) as f32 - lapse;
            air_temp_c.push(eq_t);
            let q = (8.0 + (eq_t + 15.0).max(0.0) * 0.52) * annual_precip_norm[idx].clamp(0.4, 1.5);
            vapor_kg_m2.push(q.clamp(2.0, 50.0));
            cloud_01.push((annual_precip_norm[idx] * 0.35).clamp(0.05, 0.75));
            river_ice_m.push(if eq_t < fluid.freeze_point_c - 3.0 {
                0.25
            } else {
                0.0
            });

            if is_ocean[idx] {
                mud_index_01.push(0.0);
                surface_water_m3.push(0.0);
                soil_water_m3.push(0.0);
                snow_water_m3.push(0.0);
                groundwater_m3.push(0.0);
            } else {
                let init_soil_frac = (0.25 + annual_precip_norm[idx] * 0.20).clamp(0.15, 0.72);
                let soil_vol = soil_cap_m3[idx] * f64::from(init_soil_frac);
                let surf_vol = if lake_kind[idx] != LakeKind::None {
                    base_lake_vol_m3[idx]
                } else {
                    channel_bankfull_m3[idx] * 0.06
                };
                let snow_vol = if eq_t < fluid.freeze_point_c - 2.0 {
                    area_m2[idx] * 0.04
                } else {
                    0.0
                };
                let gw_frac = (0.35 + 0.28 * soil_drainage_01[idx]).clamp(0.20, 0.78);
                let gw_vol = gw_cap_m3[idx] * f64::from(gw_frac);
                mud_index_01.push((init_soil_frac * 0.45).clamp(0.05, 0.45));
                surface_water_m3.push(surf_vol);
                soil_water_m3.push(soil_vol);
                snow_water_m3.push(snow_vol);
                groundwater_m3.push(gw_vol);
                initial_land_water_m3 += surf_vol + soil_vol + snow_vol + gw_vol;
            }
        }

        let scratch_gw_head = vec![0.0_f32; n];

        Self {
            cell_count: n,
            hours_per_year,
            axial_tilt_rad,
            solar_flux_scale,
            rotation_period_hours,
            base_sea_level_m,
            fluid,
            rules,
            sin_lat,
            cos_lat,
            lon_rad,
            elev_above_sea_m,
            is_ocean,
            area_m2,
            soil_cap_m3,
            gw_cap_m3,
            soil_drainage_01,
            fault_instability_01,
            lake_kind,
            base_lake_salinity,
            base_lake_vol_m3,
            has_perennial_channel,
            base_channel_discharge_m3_s,
            base_channel_slope_rad,
            channel_bankfull_m3,
            runoff_k,
            downstream_idx,
            topo_order,
            land_adj_pairs,
            upwind_offsets,
            upwind_nbrs,
            upwind_weights,
            base_wind_u: base_wind_u.clone(),
            base_wind_v: base_wind_v.clone(),
            annual_precip_norm,
            air_temp_c: air_temp_c.clone(),
            vapor_kg_m2: vapor_kg_m2.clone(),
            wind_u_mps: base_wind_u,
            wind_v_mps: base_wind_v,
            cloud_01,
            precip_mm_hr,
            mud_index_01,
            flood_stage_01,
            river_ice_m,
            visibility_01,
            is_spring_oasis,
            live_salinity_per_mille,
            live_discharge_m3_s,
            live_navigability,
            surface_water_m3,
            soil_water_m3,
            snow_water_m3,
            groundwater_m3,
            dams: BTreeMap::new(),
            scratch_temp_c: air_temp_c,
            scratch_vapor_kg_m2: vapor_kg_m2,
            scratch_gw_head,
            initial_land_water_m3,
            cumulative_precip_m3: 0.0,
            cumulative_storm_injected_m3: 0.0,
            cumulative_evap_m3: 0.0,
            cumulative_ocean_outflow_m3: 0.0,
        }
    }

    /// Injects an instant convective storm volume (`rain_mm` uniformly across `cell`) for storm hydrology testing (`ENG-WTH-002`, `ENG-WTH-003`).
    pub fn inject_storm_rainfall(&mut self, cell: CellId, rain_mm: f32) {
        let idx = cell.get() as usize;
        if idx >= self.cell_count || self.is_ocean[idx] || rain_mm <= 0.0 {
            return;
        }
        let d_vol = f64::from(rain_mm) * 0.001 * self.area_m2[idx];
        self.surface_water_m3[idx] += d_vol;
        self.cumulative_storm_injected_m3 += d_vol;
        self.precip_mm_hr[idx] += rain_mm;
        let boost = (rain_mm / 30.0).clamp(0.0, 0.95);
        self.mud_index_01[idx] = (self.mud_index_01[idx] + boost).clamp(0.0, 1.0);
        self.flood_stage_01[idx] =
            (self.surface_water_m3[idx] / self.channel_bankfull_m3[idx]).clamp(0.0, 1.0) as f32;
        self.visibility_01[idx] = (self.visibility_01[idx] - rain_mm * 0.015).clamp(0.10, 1.0);
    }

    /// Commissions a river dam and impounds an upstream reservoir on a perennial river reach (`ENG-GEO-014`).
    pub fn construct_dam(
        &mut self,
        cell: CellId,
        rule_curve: DamRuleCurveKind,
    ) -> Result<DamStatusView, String> {
        let idx = cell.get() as usize;
        if idx >= self.cell_count {
            return Err(format!("Cell #{} out of range", cell.get()));
        }
        if self.is_ocean[idx] {
            return Err(format!("Cannot construct a dam in ocean Cell #{}", cell.get()));
        }
        let has_flow = self.has_perennial_channel[idx]
            || (self.downstream_idx[idx].is_some()
                && self.live_discharge_m3_s[idx] >= self.rules.dam_min_discharge_m3_s);
        if !has_flow {
            return Err(format!(
                "Cell #{} lacks sufficient perennial river flow for reservoir impoundment",
                cell.get()
            ));
        }
        if let Some(existing) = self.dams.get(&cell.get()) {
            return Ok(existing.to_view());
        }
        let cap = (self.channel_bankfull_m3[idx] * 2.8).max(5.0e6);
        let init_storage = (self.surface_water_m3[idx] * 0.45).min(cap * 0.5);
        self.surface_water_m3[idx] -= init_storage;
        let mut upstream = Vec::new();
        for (u_idx, ds_opt) in self.downstream_idx.iter().enumerate() {
            if ds_opt == &Some(idx) && !self.is_ocean[u_idx] {
                upstream.push(CellId(u_idx as u32));
            }
        }
        let q_init = self.base_channel_discharge_m3_s[idx].max(self.rules.dam_min_discharge_m3_s);
        let head_m = self.rules.dam_min_relief_m.max(18.0);
        let hydro_kw =
            (self.rules.dam_turbine_efficiency_01 * (self.fluid.density_kg_m3 / 1000.0) * 9.81 * q_init * head_m * 0.35)
                .max(0.0);
        let dam = DamFacilityState {
            cell_idx: cell.get(),
            rule_curve,
            reservoir_storage_m3: init_storage,
            reservoir_capacity_m3: cap,
            reservoir_head_m: head_m,
            spillway_release_m3_s: q_init * 0.85,
            live_hydro_kw: hydro_kw,
            sediment_silt_01: 0.0,
            condition_01: 1.0,
            breached: false,
            impounded_upstream_cells: upstream,
        };
        let view = dam.to_view();
        self.dams.insert(cell.get(), dam);
        Ok(view)
    }

    /// Updates the operational reservoir rule curve mode of an existing dam (`ENG-GEO-014`).
    pub fn set_dam_rule_curve(
        &mut self,
        cell: CellId,
        rule_curve: DamRuleCurveKind,
    ) -> Result<DamStatusView, String> {
        let Some(dam) = self.dams.get_mut(&cell.get()) else {
            return Err(format!("No dam exists at Cell #{}", cell.get()));
        };
        dam.rule_curve = rule_curve;
        Ok(dam.to_view())
    }

    /// Returns current total liquid, soil, groundwater, reservoir, and snow water volume stored on all land cells (`m^3`).
    #[must_use]
    pub fn total_land_water_m3(&self) -> f64 {
        let mut total = 0.0_f64;
        for idx in 0..self.cell_count {
            if !self.is_ocean[idx] {
                total += self.surface_water_m3[idx]
                    + self.soil_water_m3[idx]
                    + self.snow_water_m3[idx]
                    + self.groundwater_m3[idx];
            }
        }
        for dam in self.dams.values() {
            total += dam.reservoir_storage_m3;
        }
        total
    }

    /// Returns relative water mass balance error across all land cells since initialization (`0.0` = exact).
    #[must_use]
    pub fn water_mass_balance_error_m3(&self) -> f64 {
        let accounted = self.total_land_water_m3()
            + self.cumulative_evap_m3
            + self.cumulative_ocean_outflow_m3;
        let supplied = self.initial_land_water_m3
            + self.cumulative_precip_m3
            + self.cumulative_storm_injected_m3;
        (accounted - supplied).abs()
    }

    /// Advances atmospheric advection, solar thermodynamics, groundwater aquifers, oases, dam impoundment, mud, and topological flood routing by 1 hour.
    pub fn step_hourly(&mut self, tick: Tick) {
        let orbital_phase =
            2.0 * PI * (tick.get() as f64) / f64::from(self.hours_per_year.max(1));
        let decl = self.axial_tilt_rad * orbital_phase.sin();
        let sin_dec = decl.sin() as f32;
        let cos_dec = decl.cos() as f32;
        let diurnal_base =
            2.0 * PI * (tick.get() as f64) / self.rotation_period_hours.max(1.0);
        let solar_scale_f32 = self.solar_flux_scale as f32;
        let freeze_c = self.fluid.freeze_point_c;
        let evap_scale = self.rules.evapotranspiration_scale;

        // 1. Double-buffered upwind advection into reusable scratch arrays (zero heap allocations).
        let advect_q = (0.22 * ((self.cell_count as f32) / 642.0).sqrt()).clamp(0.22, 0.56);
        for idx in 0..self.cell_count {
            let start = self.upwind_offsets[idx] as usize;
            let end = self.upwind_offsets[idx + 1] as usize;
            if start < end {
                let mut t_in = 0.0_f32;
                let mut q_in = 0.0_f32;
                for k in start..end {
                    let nb = self.upwind_nbrs[k] as usize;
                    let w = self.upwind_weights[k];
                    t_in += self.air_temp_c[nb] * w;
                    q_in += self.vapor_kg_m2[nb] * w;
                }
                self.scratch_temp_c[idx] = self.air_temp_c[idx] * 0.86 + t_in * 0.14;
                self.scratch_vapor_kg_m2[idx] =
                    self.vapor_kg_m2[idx] * (1.0 - advect_q) + q_in * advect_q;
            } else {
                self.scratch_temp_c[idx] = self.air_temp_c[idx];
                self.scratch_vapor_kg_m2[idx] = self.vapor_kg_m2[idx];
            }
        }
        self.air_temp_c.copy_from_slice(&self.scratch_temp_c);
        self.vapor_kg_m2
            .copy_from_slice(&self.scratch_vapor_kg_m2);

        // 2. Diurnal/seasonal solar forcing, condensation, snow/melt, soil infiltration, percolation & evaporation.
        for idx in 0..self.cell_count {
            let slat = self.sin_lat[idx];
            let clat = self.cos_lat[idx];
            let diurnal = (diurnal_base + f64::from(self.lon_rad[idx])).cos() as f32;
            let cos_zenith =
                (slat * sin_dec + clat * cos_dec * diurnal.max(-0.25)).clamp(-0.15, 1.0);

            let lapse = self.elev_above_sea_m[idx] * 0.0065;
            let seasonal_term = slat * sin_dec * 1.15 + clat * cos_dec * 0.56;
            let eq_temp =
                (-12.0 + 54.0 * seasonal_term + 18.0 * cos_zenith) * solar_scale_f32 - lapse;

            let inertia = if self.is_ocean[idx] { 0.015 } else { 0.085 };
            let t_now = self.air_temp_c[idx] * (1.0 - inertia) + eq_temp * inertia;
            self.air_temp_c[idx] = t_now;

            let front_wave =
                (orbital_phase * 14.0 + f64::from(self.lon_rad[idx]) * 3.0).sin() as f32;
            self.wind_u_mps[idx] = self.base_wind_u[idx] * (1.0 + 0.18 * front_wave);
            self.wind_v_mps[idx] = self.base_wind_v[idx] + front_wave * 2.2;

            let sat_q = 4.8 + (t_now + 22.0).max(0.0) * 0.65;

            if self.is_ocean[idx] {
                let target_q = sat_q * 0.94;
                self.vapor_kg_m2[idx] =
                    (self.vapor_kg_m2[idx] * 0.85 + target_q * 0.15).clamp(1.0, 55.0);
                let rain = ((self.vapor_kg_m2[idx] - sat_q * 0.88).max(0.0) * 0.45)
                    * (0.7 + 0.3 * self.annual_precip_norm[idx]);
                self.precip_mm_hr[idx] = rain;
                self.cloud_01[idx] = (self.vapor_kg_m2[idx] / sat_q.max(1.0)).clamp(0.15, 0.98);
                self.visibility_01[idx] =
                    (1.0 - rain * 0.03 - self.cloud_01[idx] * 0.12).clamp(0.25, 1.0);
            } else {
                let wet_bias = (self.annual_precip_norm[idx] - 1.0).clamp(-0.75, 1.35);
                let trigger =
                    sat_q * (0.92 - 0.08 * wet_bias - 0.16 * front_wave).clamp(0.68, 1.05);
                let excess = (self.vapor_kg_m2[idx] - trigger).max(0.0);
                let rain_mm = (excess * 0.18 * self.annual_precip_norm[idx]).clamp(0.0, 22.0);
                self.vapor_kg_m2[idx] = (self.vapor_kg_m2[idx] - rain_mm * 0.45).clamp(0.8, 52.0);
                self.precip_mm_hr[idx] = rain_mm;
                self.cloud_01[idx] = (self.vapor_kg_m2[idx] / sat_q.max(1.0)).clamp(0.05, 0.98);

                let rain_m3 = f64::from(rain_mm) * 0.001 * self.area_m2[idx];
                self.cumulative_precip_m3 += rain_m3;
                if t_now <= freeze_c - 0.5 {
                    self.snow_water_m3[idx] += rain_m3;
                } else {
                    self.surface_water_m3[idx] += rain_m3;
                }

                // Temperature-index snowmelt into surface water.
                if t_now > freeze_c + 0.5 && self.snow_water_m3[idx] > 0.0 {
                    let melt_depth = f64::from(((t_now - (freeze_c + 0.5)) * 0.00045).min(0.025));
                    let melt_m3 = (melt_depth * self.area_m2[idx]).min(self.snow_water_m3[idx]);
                    self.snow_water_m3[idx] -= melt_m3;
                    self.surface_water_m3[idx] += melt_m3;
                }

                // Infiltration into root soil capacity & percolation into groundwater aquifer (`ENG-WTH-006`).
                let soil_headroom = (self.soil_cap_m3[idx] - self.soil_water_m3[idx]).max(0.0);
                let infil_rate = f64::from(self.rules.infiltration_rate_01.clamp(0.02, 0.85)) * 0.28;
                let infil_m3 = (self.surface_water_m3[idx] * infil_rate).min(soil_headroom);
                self.surface_water_m3[idx] -= infil_m3;
                self.soil_water_m3[idx] += infil_m3;

                let gw_headroom = (self.gw_cap_m3[idx] - self.groundwater_m3[idx]).max(0.0);
                let perc_m3 = (self.soil_water_m3[idx] * 0.012 * f64::from(self.soil_drainage_01[idx]))
                    .min(gw_headroom);
                self.soil_water_m3[idx] -= perc_m3;
                self.groundwater_m3[idx] += perc_m3;

                let sm = (self.soil_water_m3[idx] / self.soil_cap_m3[idx].max(1.0)).clamp(0.0, 1.0);
                let pot_evap_m3 =
                    f64::from((t_now + 6.0).max(0.0) * 0.000018 * evap_scale) * self.area_m2[idx];
                let evap_soil = (pot_evap_m3 * 0.78 * sm).min(self.soil_water_m3[idx]);
                self.soil_water_m3[idx] -= evap_soil;
                let evap_surf = (pot_evap_m3 * 0.22).min(self.surface_water_m3[idx]);
                self.surface_water_m3[idx] -= evap_surf;
                self.cumulative_evap_m3 += evap_soil + evap_surf;

                let evap_kg_m2 =
                    (((evap_soil + evap_surf) / self.area_m2[idx].max(1.0)) * 1000.0) as f32;
                let evap_cap = sat_q * 0.74;
                let vapor_with_evap = self.vapor_kg_m2[idx]
                    + evap_kg_m2.min((evap_cap - self.vapor_kg_m2[idx]).max(0.0));
                self.vapor_kg_m2[idx] = vapor_with_evap.clamp(0.8, 52.0);
            }
        }

        // 2b. Conservative antisymmetric lateral groundwater Darcy diffusion & emergent springs/oases (`ENG-WTH-006`).
        for idx in 0..self.cell_count {
            if self.is_ocean[idx] {
                self.scratch_gw_head[idx] = 0.0;
            } else {
                let gw_m = (self.groundwater_m3[idx] / self.area_m2[idx].max(1.0)) * 22.0;
                self.scratch_gw_head[idx] = self.elev_above_sea_m[idx] + gw_m as f32;
            }
        }
        let gw_k = f64::from(self.rules.aquifer_diffusivity.clamp(0.0, 1.0));
        for &(u, v) in &self.land_adj_pairs {
            let dh = f64::from(self.scratch_gw_head[u] - self.scratch_gw_head[v]);
            if dh > 0.0 {
                let q = ((dh / 1000.0).min(0.45) * 0.04 * gw_k * self.groundwater_m3[u])
                    .min(self.groundwater_m3[u] * 0.05);
                self.groundwater_m3[u] -= q;
                self.groundwater_m3[v] += q;
            } else if dh < 0.0 {
                let q = (((-dh) / 1000.0).min(0.45) * 0.04 * gw_k * self.groundwater_m3[v])
                    .min(self.groundwater_m3[v] * 0.05);
                self.groundwater_m3[v] -= q;
                self.groundwater_m3[u] += q;
            }
        }
        let spring_frac = f64::from(self.rules.spring_emergence_threshold_01.clamp(0.5, 0.99));
        for idx in 0..self.cell_count {
            if self.is_ocean[idx] {
                continue;
            }
            let cap = self.gw_cap_m3[idx].max(1.0);
            let fault_boost = if self.fault_instability_01[idx] >= 0.45 {
                0.72
            } else {
                1.0
            };
            let thresh = cap * spring_frac * fault_boost;
            if self.groundwater_m3[idx] > thresh {
                let exfil = ((self.groundwater_m3[idx] - thresh) * 0.16)
                    .min(self.groundwater_m3[idx]);
                self.groundwater_m3[idx] -= exfil;
                let to_soil = (exfil * 0.55).min((self.soil_cap_m3[idx] - self.soil_water_m3[idx]).max(0.0));
                self.soil_water_m3[idx] += to_soil;
                self.surface_water_m3[idx] += exfil - to_soil;
                self.is_spring_oasis[idx] = exfil > 10.0;
            } else {
                self.is_spring_oasis[idx] = false;
            }
        }

        // 3. Conservative upstream-to-downstream flood routing, dam reservoir rule curves & lake salinity (`ENG-WTH-003`, `006`, `GEO-014`).
        for &idx in &self.topo_order {
            let frozen_block = self.air_temp_c[idx] < freeze_c - 2.0;
            let mut hourly_outflow_m3 = 0.0_f64;

            if let Some(dam) = self.dams.get_mut(&(idx as u32)) {
                if !dam.breached {
                    let eff_cap = (dam.reservoir_capacity_m3
                        * f64::from(1.0 - dam.sediment_silt_01 * 0.45))
                    .max(1.0);
                    let inflow = self.surface_water_m3[idx];
                    self.surface_water_m3[idx] = 0.0;
                    dam.reservoir_storage_m3 += inflow;

                    let target_ratio = match dam.rule_curve {
                        DamRuleCurveKind::BalancedPower => 0.85,
                        DamRuleCurveKind::FloodControl => 0.62,
                        DamRuleCurveKind::IrrigationSupply => 0.78,
                    };
                    let target_vol = eff_cap * target_ratio;
                    let fill_now = (dam.reservoir_storage_m3 / eff_cap).clamp(0.0, 1.5);
                    if fill_now >= 0.99 && self.flood_stage_01[idx] >= self.rules.dam_breach_flood_threshold_01 {
                        // Extreme overtopping breach surge: conserve mass completely back to surface channel!
                        dam.breached = true;
                        dam.live_hydro_kw = 0.0;
                        dam.condition_01 = 0.05;
                        self.surface_water_m3[idx] += dam.reservoir_storage_m3;
                        dam.reservoir_storage_m3 = 0.0;
                    } else {
                        let base_release = if dam.reservoir_storage_m3 > target_vol {
                            (dam.reservoir_storage_m3 - target_vol) * 0.45
                                + dam.reservoir_storage_m3 * f64::from(self.runoff_k[idx]) * 0.35
                        } else if !frozen_block {
                            (dam.reservoir_storage_m3 * 0.04)
                                .min(dam.reservoir_storage_m3)
                        } else {
                            0.0
                        };
                        let release = base_release.min(dam.reservoir_storage_m3);
                        dam.reservoir_storage_m3 -= release;
                        hourly_outflow_m3 = release;
                        if let Some(ds) = self.downstream_idx[idx] {
                            if self.is_ocean[ds] {
                                self.cumulative_ocean_outflow_m3 += release;
                            } else {
                                self.surface_water_m3[ds] += release;
                                if dam.rule_curve == DamRuleCurveKind::IrrigationSupply {
                                    let headroom = (self.soil_cap_m3[ds] - self.soil_water_m3[ds]).max(0.0);
                                    let irrig = (dam.reservoir_storage_m3 * 0.015).min(headroom);
                                    dam.reservoir_storage_m3 -= irrig;
                                    self.soil_water_m3[ds] += irrig;
                                }
                            }
                        } else {
                            self.surface_water_m3[idx] += release;
                        }
                        let fill_after = (dam.reservoir_storage_m3 / eff_cap).clamp(0.0, 1.0) as f32;
                        dam.reservoir_head_m = self.rules.dam_min_relief_m.max(12.0) * (0.45 + 0.75 * fill_after);
                        let q_rel = (release / 3600.0) as f32;
                        dam.spillway_release_m3_s = q_rel;
                        let flow_for_power = q_rel.max(self.base_channel_discharge_m3_s[idx] * fill_after * 0.65);
                        dam.live_hydro_kw = (self.rules.dam_turbine_efficiency_01
                            * (self.fluid.density_kg_m3 / 1000.0)
                            * 9.81
                            * flow_for_power
                            * dam.reservoir_head_m)
                            .max(0.0);
                        dam.sediment_silt_01 =
                            (dam.sediment_silt_01 + self.rules.dam_siltation_rate_per_day / 24.0)
                                .min(0.85);
                    }
                }
            }

            if hourly_outflow_m3 == 0.0 && !frozen_block {
                let avail = self.surface_water_m3[idx];
                let lake_damp = if self.lake_kind[idx] == LakeKind::Exorheic {
                    0.55
                } else {
                    1.0
                };
                let outflow = avail * f64::from(self.runoff_k[idx]) * lake_damp;
                if let Some(ds) = self.downstream_idx[idx] {
                    self.surface_water_m3[idx] = avail - outflow;
                    if self.is_ocean[ds] {
                        self.cumulative_ocean_outflow_m3 += outflow;
                    } else {
                        self.surface_water_m3[ds] += outflow;
                    }
                    hourly_outflow_m3 = outflow;
                }
            }

            let t_now = self.air_temp_c[idx];
            if t_now < freeze_c - 1.0 {
                self.river_ice_m[idx] =
                    (self.river_ice_m[idx] + ((freeze_c - 1.0) - t_now) * 0.002).min(2.5);
            } else if t_now > freeze_c + 1.0 && self.river_ice_m[idx] > 0.0 {
                self.river_ice_m[idx] =
                    (self.river_ice_m[idx] - (t_now - (freeze_c + 1.0)) * 0.004).max(0.0);
            }

            let bank = self.channel_bankfull_m3[idx].max(1.0);
            let flood = (self.surface_water_m3[idx] / bank).clamp(0.0, 1.0) as f32;
            self.flood_stage_01[idx] = flood;

            if self.has_perennial_channel[idx] {
                let hourly_q = (hourly_outflow_m3 / 3600.0) as f32;
                let storm_scale = (0.85 + flood * 0.85).clamp(0.25, 2.5);
                let live_q =
                    (self.base_channel_discharge_m3_s[idx] * storm_scale * 0.65 + hourly_q * 0.35)
                        .max(0.0);
                self.live_discharge_m3_s[idx] = live_q;
                let local_slope = self.base_channel_slope_rad[idx] * 8.0;
                let area_scale = ((self.area_m2[idx] as f32) * 1e-6 / 1800.0).clamp(1.0, 55.0);
                let reach_q = live_q / area_scale;
                let dam_bypass = self
                    .dams
                    .get(&(idx as u32))
                    .is_some_and(|d| !d.breached);
                self.live_navigability[idx] =
                    if !dam_bypass && local_slope >= self.rules.rapids_min_slope_rad {
                        RiverNavigabilityKind::Rapids
                    } else if reach_q >= self.rules.deep_navigable_min_q_m3_s {
                        RiverNavigabilityKind::DeepNavigable
                    } else if reach_q >= 2.0 {
                        RiverNavigabilityKind::ShallowFord
                    } else {
                        RiverNavigabilityKind::None
                    };
            }

            if self.lake_kind[idx] == LakeKind::Endorheic {
                let ratio = (self.base_lake_vol_m3[idx] / self.surface_water_m3[idx].max(1.0))
                    .clamp(0.35, 4.5) as f32;
                self.live_salinity_per_mille[idx] =
                    (self.base_lake_salinity[idx] * ratio).clamp(5.0, 320.0);
            }

            let sm = (self.soil_water_m3[idx] / self.soil_cap_m3[idx].max(1.0)).clamp(0.0, 1.0) as f32;
            let raw_mud =
                (0.58 * sm + 0.34 * flood + 0.25 * (self.precip_mm_hr[idx] / 12.0).min(1.0))
                    .clamp(0.0, 1.0);
            let freeze_damp = if t_now < freeze_c - 1.5 { 0.35 } else { 1.0 };
            self.mud_index_01[idx] =
                (self.mud_index_01[idx] * 0.85 + raw_mud * freeze_damp * 0.15).clamp(0.0, 1.0);

            self.visibility_01[idx] =
                (1.0 - (self.precip_mm_hr[idx] / 22.0).clamp(0.0, 0.70) - self.cloud_01[idx] * 0.12)
                    .clamp(0.10, 1.0);
        }
    }

    /// Computes canonical Blake3 digest of all physical weather & water pools.
    #[must_use]
    pub fn blake3_digest(&self) -> [u8; 32] {
        let mut buf = Vec::with_capacity(self.cell_count * 64 + 256);
        self.write_canonical_state(&mut buf);
        *blake3::hash(&buf).as_bytes()
    }
}

impl SurfaceConditionsPort for WeatherEngine {
    fn surface(&self, cell: CellId) -> SurfaceConditionSnapshot {
        let idx = cell.get() as usize;
        if idx >= self.cell_count {
            return SurfaceConditionSnapshot::default();
        }
        let snow_depth_m =
            ((self.snow_water_m3[idx] / self.area_m2[idx].max(1.0)) * 8.5) as f32;
        SurfaceConditionSnapshot {
            precip_mm_hr: self.precip_mm_hr[idx],
            mud_index_01: self.mud_index_01[idx],
            flood_stage_01: self.flood_stage_01[idx],
            snow_depth_m,
            ice_thickness_m: self.river_ice_m[idx],
            temp_c: self.air_temp_c[idx],
            wind_u_mps: self.wind_u_mps[idx],
            wind_v_mps: self.wind_v_mps[idx],
            visibility_01: self.visibility_01[idx],
        }
    }

    fn cell_groundwater_m3(&self, cell: CellId) -> f64 {
        self.groundwater_m3
            .get(cell.get() as usize)
            .copied()
            .unwrap_or(0.0)
    }

    fn cell_is_spring_oasis(&self, cell: CellId) -> bool {
        self.is_spring_oasis
            .get(cell.get() as usize)
            .copied()
            .unwrap_or(false)
    }

    fn cell_salinity_per_mille(&self, cell: CellId) -> f32 {
        self.live_salinity_per_mille
            .get(cell.get() as usize)
            .copied()
            .unwrap_or(0.2)
    }

    fn cell_live_discharge_m3_s(&self, cell: CellId) -> f32 {
        self.live_discharge_m3_s
            .get(cell.get() as usize)
            .copied()
            .unwrap_or(0.0)
    }

    fn cell_live_navigability(&self, cell: CellId) -> RiverNavigabilityKind {
        self.live_navigability
            .get(cell.get() as usize)
            .copied()
            .unwrap_or(RiverNavigabilityKind::None)
    }

    fn active_dams(&self) -> Vec<DamStatusView> {
        self.dams.values().map(DamFacilityState::to_view).collect()
    }

    fn dam_at_cell(&self, cell: CellId) -> Option<DamStatusView> {
        self.dams.get(&cell.get()).map(DamFacilityState::to_view)
    }
}

impl CanonicalSaveablePort for WeatherEngine {
    fn cartridge_tag(&self) -> &'static str {
        "weather"
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        write_u32_le(out, self.cell_count as u32);
        write_u32_le(out, self.hours_per_year);
        write_f64_bits(out, self.axial_tilt_rad);
        write_f64_bits(out, self.solar_flux_scale);
        write_f64_bits(out, self.rotation_period_hours);
        write_f32_bits(out, self.base_sea_level_m);
        write_str(out, &self.fluid.id);
        write_str(out, &self.fluid.display_name);
        write_f32_bits(out, self.fluid.density_kg_m3);
        write_f32_bits(out, self.fluid.freeze_point_c);
        write_f32_bits(out, self.fluid.boil_point_c);
        write_f32_bits(out, self.fluid.latent_heat_evap_kj_kg);
        write_f32_bits(out, self.fluid.specific_heat_j_kg_k);
        write_str(out, &self.rules.active_fluid_id);
        write_f32_bits(out, self.rules.infiltration_rate_01);
        write_f32_bits(out, self.rules.evapotranspiration_scale);
        write_f32_bits(out, self.rules.aquifer_diffusivity);
        write_f32_bits(out, self.rules.aquifer_capacity_mm);
        write_f32_bits(out, self.rules.spring_emergence_threshold_01);
        write_f32_bits(out, self.rules.estuary_max_elev_m);
        write_f32_bits(out, self.rules.shallow_ford_max_q_m3_s);
        write_f32_bits(out, self.rules.deep_navigable_min_q_m3_s);
        write_f32_bits(out, self.rules.rapids_min_slope_rad);
        write_f32_bits(out, self.rules.dam_min_discharge_m3_s);
        write_f32_bits(out, self.rules.dam_min_relief_m);
        write_f32_bits(out, self.rules.dam_turbine_efficiency_01);
        write_f32_bits(out, self.rules.dam_siltation_rate_per_day);
        write_f32_bits(out, self.rules.dam_breach_flood_threshold_01);
        write_f64_bits(out, self.initial_land_water_m3);
        write_f64_bits(out, self.cumulative_precip_m3);
        write_f64_bits(out, self.cumulative_storm_injected_m3);
        write_f64_bits(out, self.cumulative_evap_m3);
        write_f64_bits(out, self.cumulative_ocean_outflow_m3);

        for idx in 0..self.cell_count {
            write_f32_bits(out, self.sin_lat[idx]);
            write_f32_bits(out, self.cos_lat[idx]);
            write_f32_bits(out, self.lon_rad[idx]);
            write_f32_bits(out, self.elev_above_sea_m[idx]);
            write_u8(out, u8::from(self.is_ocean[idx]));
            write_f64_bits(out, self.area_m2[idx]);
            write_f64_bits(out, self.soil_cap_m3[idx]);
            write_f64_bits(out, self.gw_cap_m3[idx]);
            write_f32_bits(out, self.soil_drainage_01[idx]);
            write_f32_bits(out, self.fault_instability_01[idx]);
            write_u8(out, self.lake_kind[idx].code());
            write_f32_bits(out, self.base_lake_salinity[idx]);
            write_f64_bits(out, self.base_lake_vol_m3[idx]);
            write_u8(out, u8::from(self.has_perennial_channel[idx]));
            write_f32_bits(out, self.base_channel_discharge_m3_s[idx]);
            write_f32_bits(out, self.base_channel_slope_rad[idx]);
            write_f64_bits(out, self.channel_bankfull_m3[idx]);
            write_f32_bits(out, self.runoff_k[idx]);
            match self.downstream_idx[idx] {
                None => write_u32_le(out, u32::MAX),
                Some(ds) => write_u32_le(out, ds as u32),
            }
            write_f32_bits(out, self.base_wind_u[idx]);
            write_f32_bits(out, self.base_wind_v[idx]);
            write_f32_bits(out, self.annual_precip_norm[idx]);

            write_f32_bits(out, self.air_temp_c[idx]);
            write_f32_bits(out, self.vapor_kg_m2[idx]);
            write_f32_bits(out, self.wind_u_mps[idx]);
            write_f32_bits(out, self.wind_v_mps[idx]);
            write_f32_bits(out, self.cloud_01[idx]);
            write_f32_bits(out, self.precip_mm_hr[idx]);
            write_f32_bits(out, self.mud_index_01[idx]);
            write_f32_bits(out, self.flood_stage_01[idx]);
            write_f32_bits(out, self.river_ice_m[idx]);
            write_f32_bits(out, self.visibility_01[idx]);
            write_u8(out, u8::from(self.is_spring_oasis[idx]));
            write_f32_bits(out, self.live_salinity_per_mille[idx]);
            write_f32_bits(out, self.live_discharge_m3_s[idx]);
            write_u8(out, self.live_navigability[idx].code());
            write_f64_bits(out, self.surface_water_m3[idx]);
            write_f64_bits(out, self.soil_water_m3[idx]);
            write_f64_bits(out, self.snow_water_m3[idx]);
            write_f64_bits(out, self.groundwater_m3[idx]);
        }

        write_u32_le(out, self.topo_order.len() as u32);
        for &c in &self.topo_order {
            write_u32_le(out, c as u32);
        }
        write_u32_le(out, self.land_adj_pairs.len() as u32);
        for &(u, v) in &self.land_adj_pairs {
            write_u32_le(out, u as u32);
            write_u32_le(out, v as u32);
        }
        write_u32_le(out, self.upwind_offsets.len() as u32);
        for &off in &self.upwind_offsets {
            write_u32_le(out, off);
        }
        write_u32_le(out, self.upwind_nbrs.len() as u32);
        for k in 0..self.upwind_nbrs.len() {
            write_u32_le(out, self.upwind_nbrs[k]);
            write_f32_bits(out, self.upwind_weights[k]);
        }
        write_u32_le(out, self.dams.len() as u32);
        for dam in self.dams.values() {
            write_u32_le(out, dam.cell_idx);
            write_u8(out, dam.rule_curve.code());
            write_f64_bits(out, dam.reservoir_storage_m3);
            write_f64_bits(out, dam.reservoir_capacity_m3);
            write_f32_bits(out, dam.reservoir_head_m);
            write_f32_bits(out, dam.spillway_release_m3_s);
            write_f32_bits(out, dam.live_hydro_kw);
            write_f32_bits(out, dam.sediment_silt_01);
            write_f32_bits(out, dam.condition_01);
            write_u8(out, u8::from(dam.breached));
            write_u32_le(out, dam.impounded_upstream_cells.len() as u32);
            for &uc in &dam.impounded_upstream_cells {
                write_u32_le(out, uc.get());
            }
        }
        let _ = write_u64_le;
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        let cell_count = read_u32_le(input)? as usize;
        let hours_per_year = read_u32_le(input)?;
        let axial_tilt_rad = read_f64_bits(input)?;
        let solar_flux_scale = read_f64_bits(input)?;
        let rotation_period_hours = read_f64_bits(input)?;
        let base_sea_level_m = read_f32_bits(input)?;
        let fluid = FluidDefView {
            id: read_str(input)?,
            display_name: read_str(input)?,
            density_kg_m3: read_f32_bits(input)?,
            freeze_point_c: read_f32_bits(input)?,
            boil_point_c: read_f32_bits(input)?,
            latent_heat_evap_kj_kg: read_f32_bits(input)?,
            specific_heat_j_kg_k: read_f32_bits(input)?,
        };
        let rules = HydrologyRulesView {
            active_fluid_id: read_str(input)?,
            infiltration_rate_01: read_f32_bits(input)?,
            evapotranspiration_scale: read_f32_bits(input)?,
            aquifer_diffusivity: read_f32_bits(input)?,
            aquifer_capacity_mm: read_f32_bits(input)?,
            spring_emergence_threshold_01: read_f32_bits(input)?,
            estuary_max_elev_m: read_f32_bits(input)?,
            shallow_ford_max_q_m3_s: read_f32_bits(input)?,
            deep_navigable_min_q_m3_s: read_f32_bits(input)?,
            rapids_min_slope_rad: read_f32_bits(input)?,
            dam_min_discharge_m3_s: read_f32_bits(input)?,
            dam_min_relief_m: read_f32_bits(input)?,
            dam_turbine_efficiency_01: read_f32_bits(input)?,
            dam_siltation_rate_per_day: read_f32_bits(input)?,
            dam_breach_flood_threshold_01: read_f32_bits(input)?,
        };
        let initial_land_water_m3 = read_f64_bits(input)?;
        let cumulative_precip_m3 = read_f64_bits(input)?;
        let cumulative_storm_injected_m3 = read_f64_bits(input)?;
        let cumulative_evap_m3 = read_f64_bits(input)?;
        let cumulative_ocean_outflow_m3 = read_f64_bits(input)?;

        let mut sin_lat = Vec::with_capacity(cell_count);
        let mut cos_lat = Vec::with_capacity(cell_count);
        let mut lon_rad = Vec::with_capacity(cell_count);
        let mut elev_above_sea_m = Vec::with_capacity(cell_count);
        let mut is_ocean = Vec::with_capacity(cell_count);
        let mut area_m2 = Vec::with_capacity(cell_count);
        let mut soil_cap_m3 = Vec::with_capacity(cell_count);
        let mut gw_cap_m3 = Vec::with_capacity(cell_count);
        let mut soil_drainage_01 = Vec::with_capacity(cell_count);
        let mut fault_instability_01 = Vec::with_capacity(cell_count);
        let mut lake_kind = Vec::with_capacity(cell_count);
        let mut base_lake_salinity = Vec::with_capacity(cell_count);
        let mut base_lake_vol_m3 = Vec::with_capacity(cell_count);
        let mut has_perennial_channel = Vec::with_capacity(cell_count);
        let mut base_channel_discharge_m3_s = Vec::with_capacity(cell_count);
        let mut base_channel_slope_rad = Vec::with_capacity(cell_count);
        let mut channel_bankfull_m3 = Vec::with_capacity(cell_count);
        let mut runoff_k = Vec::with_capacity(cell_count);
        let mut downstream_idx = Vec::with_capacity(cell_count);
        let mut base_wind_u = Vec::with_capacity(cell_count);
        let mut base_wind_v = Vec::with_capacity(cell_count);
        let mut annual_precip_norm = Vec::with_capacity(cell_count);

        let mut air_temp_c = Vec::with_capacity(cell_count);
        let mut vapor_kg_m2 = Vec::with_capacity(cell_count);
        let mut wind_u_mps = Vec::with_capacity(cell_count);
        let mut wind_v_mps = Vec::with_capacity(cell_count);
        let mut cloud_01 = Vec::with_capacity(cell_count);
        let mut precip_mm_hr = Vec::with_capacity(cell_count);
        let mut mud_index_01 = Vec::with_capacity(cell_count);
        let mut flood_stage_01 = Vec::with_capacity(cell_count);
        let mut river_ice_m = Vec::with_capacity(cell_count);
        let mut visibility_01 = Vec::with_capacity(cell_count);
        let mut is_spring_oasis = Vec::with_capacity(cell_count);
        let mut live_salinity_per_mille = Vec::with_capacity(cell_count);
        let mut live_discharge_m3_s = Vec::with_capacity(cell_count);
        let mut live_navigability = Vec::with_capacity(cell_count);
        let mut surface_water_m3 = Vec::with_capacity(cell_count);
        let mut soil_water_m3 = Vec::with_capacity(cell_count);
        let mut snow_water_m3 = Vec::with_capacity(cell_count);
        let mut groundwater_m3 = Vec::with_capacity(cell_count);

        for _ in 0..cell_count {
            sin_lat.push(read_f32_bits(input)?);
            cos_lat.push(read_f32_bits(input)?);
            lon_rad.push(read_f32_bits(input)?);
            elev_above_sea_m.push(read_f32_bits(input)?);
            is_ocean.push(read_u8(input)? != 0);
            area_m2.push(read_f64_bits(input)?);
            soil_cap_m3.push(read_f64_bits(input)?);
            gw_cap_m3.push(read_f64_bits(input)?);
            soil_drainage_01.push(read_f32_bits(input)?);
            fault_instability_01.push(read_f32_bits(input)?);
            lake_kind.push(LakeKind::from_code(read_u8(input)?));
            base_lake_salinity.push(read_f32_bits(input)?);
            base_lake_vol_m3.push(read_f64_bits(input)?);
            has_perennial_channel.push(read_u8(input)? != 0);
            base_channel_discharge_m3_s.push(read_f32_bits(input)?);
            base_channel_slope_rad.push(read_f32_bits(input)?);
            channel_bankfull_m3.push(read_f64_bits(input)?);
            runoff_k.push(read_f32_bits(input)?);
            let ds_raw = read_u32_le(input)?;
            downstream_idx.push(if ds_raw == u32::MAX {
                None
            } else {
                Some(ds_raw as usize)
            });
            base_wind_u.push(read_f32_bits(input)?);
            base_wind_v.push(read_f32_bits(input)?);
            annual_precip_norm.push(read_f32_bits(input)?);

            air_temp_c.push(read_f32_bits(input)?);
            vapor_kg_m2.push(read_f32_bits(input)?);
            wind_u_mps.push(read_f32_bits(input)?);
            wind_v_mps.push(read_f32_bits(input)?);
            cloud_01.push(read_f32_bits(input)?);
            precip_mm_hr.push(read_f32_bits(input)?);
            mud_index_01.push(read_f32_bits(input)?);
            flood_stage_01.push(read_f32_bits(input)?);
            river_ice_m.push(read_f32_bits(input)?);
            visibility_01.push(read_f32_bits(input)?);
            is_spring_oasis.push(read_u8(input)? != 0);
            live_salinity_per_mille.push(read_f32_bits(input)?);
            live_discharge_m3_s.push(read_f32_bits(input)?);
            live_navigability.push(RiverNavigabilityKind::from_code(read_u8(input)?));
            surface_water_m3.push(read_f64_bits(input)?);
            soil_water_m3.push(read_f64_bits(input)?);
            snow_water_m3.push(read_f64_bits(input)?);
            groundwater_m3.push(read_f64_bits(input)?);
        }

        let topo_len = read_u32_le(input)? as usize;
        let mut topo_order = Vec::with_capacity(topo_len);
        for _ in 0..topo_len {
            topo_order.push(read_u32_le(input)? as usize);
        }
        let pair_len = read_u32_le(input)? as usize;
        let mut land_adj_pairs = Vec::with_capacity(pair_len);
        for _ in 0..pair_len {
            land_adj_pairs.push((read_u32_le(input)? as usize, read_u32_le(input)? as usize));
        }
        let off_len = read_u32_le(input)? as usize;
        let mut upwind_offsets = Vec::with_capacity(off_len);
        for _ in 0..off_len {
            upwind_offsets.push(read_u32_le(input)?);
        }
        let nbr_len = read_u32_le(input)? as usize;
        let mut upwind_nbrs = Vec::with_capacity(nbr_len);
        let mut upwind_weights = Vec::with_capacity(nbr_len);
        for _ in 0..nbr_len {
            upwind_nbrs.push(read_u32_le(input)?);
            upwind_weights.push(read_f32_bits(input)?);
        }
        let dam_count = read_u32_le(input)? as usize;
        let mut dams = BTreeMap::new();
        for _ in 0..dam_count {
            let cell_idx = read_u32_le(input)?;
            let rule_curve = DamRuleCurveKind::from_code(read_u8(input)?);
            let reservoir_storage_m3 = read_f64_bits(input)?;
            let reservoir_capacity_m3 = read_f64_bits(input)?;
            let reservoir_head_m = read_f32_bits(input)?;
            let spillway_release_m3_s = read_f32_bits(input)?;
            let live_hydro_kw = read_f32_bits(input)?;
            let sediment_silt_01 = read_f32_bits(input)?;
            let condition_01 = read_f32_bits(input)?;
            let breached = read_u8(input)? != 0;
            let uc_len = read_u32_le(input)? as usize;
            let mut impounded_upstream_cells = Vec::with_capacity(uc_len);
            for _ in 0..uc_len {
                impounded_upstream_cells.push(CellId(read_u32_le(input)?));
            }
            dams.insert(
                cell_idx,
                DamFacilityState {
                    cell_idx,
                    rule_curve,
                    reservoir_storage_m3,
                    reservoir_capacity_m3,
                    reservoir_head_m,
                    spillway_release_m3_s,
                    live_hydro_kw,
                    sediment_silt_01,
                    condition_01,
                    breached,
                    impounded_upstream_cells,
                },
            );
        }
        let _ = read_u64_le;

        *self = Self {
            cell_count,
            hours_per_year,
            axial_tilt_rad,
            solar_flux_scale,
            rotation_period_hours,
            base_sea_level_m,
            fluid,
            rules,
            sin_lat,
            cos_lat,
            lon_rad,
            elev_above_sea_m,
            is_ocean,
            area_m2,
            soil_cap_m3,
            gw_cap_m3,
            soil_drainage_01,
            fault_instability_01,
            lake_kind,
            base_lake_salinity,
            base_lake_vol_m3,
            has_perennial_channel,
            base_channel_discharge_m3_s,
            base_channel_slope_rad,
            channel_bankfull_m3,
            runoff_k,
            downstream_idx,
            topo_order,
            land_adj_pairs,
            upwind_offsets,
            upwind_nbrs,
            upwind_weights,
            base_wind_u,
            base_wind_v,
            annual_precip_norm,
            air_temp_c: air_temp_c.clone(),
            vapor_kg_m2: vapor_kg_m2.clone(),
            wind_u_mps,
            wind_v_mps,
            cloud_01,
            precip_mm_hr,
            mud_index_01,
            flood_stage_01,
            river_ice_m,
            visibility_01,
            is_spring_oasis,
            live_salinity_per_mille,
            live_discharge_m3_s,
            live_navigability,
            surface_water_m3,
            soil_water_m3,
            snow_water_m3,
            groundwater_m3,
            dams,
            scratch_temp_c: air_temp_c,
            scratch_vapor_kg_m2: vapor_kg_m2,
            scratch_gw_head: vec![0.0; cell_count],
            initial_land_water_m3,
            cumulative_precip_m3,
            cumulative_storm_injected_m3,
            cumulative_evap_m3,
            cumulative_ocean_outflow_m3,
        };
        Ok(())
    }
}
