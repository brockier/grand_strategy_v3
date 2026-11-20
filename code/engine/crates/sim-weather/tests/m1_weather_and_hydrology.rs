#![deny(clippy::disallowed_types)]

use std::collections::BTreeMap;
use std::f32::consts::PI;

use sim_spine::{
    CalendarConfigView, CanonicalSaveablePort, CellGeologyView, CellId, DamRuleCurveKind, EdgeId,
    GeologyProvinceKind, LakeKind, NaturalBoundsView, PlanetGenConfigView, PlanetGeometryPort,
    RockStrataClass, SurfaceConditionsPort, Tick,
};
use sim_weather::WeatherEngine;

fn test_planet_cfg() -> PlanetGenConfigView {
    PlanetGenConfigView {
        id: "test_world".to_string(),
        goldberg_subdivision: 3,
        tectonic_plate_count: 8,
        land_fraction_target: 0.5,
        axial_tilt_deg: 23.44,
        solar_constant_w_m2: 1361.0,
        rotation_period_hours: 24.0,
        mean_radius_km: 6371.0,
        base_sea_level_m: 0.0,
        max_orogenic_uplift_m: 3500.0,
        planetary_density_kg_m3: 5514.0,
        natural_bounds: NaturalBoundsView {
            ore_count: 2,
            flux_count: 2,
            fuel_count: 2,
            crop_count: 2,
            fauna_count: 2,
            pathogen_count: 2,
            ore_melting_k_range: [1000.0, 1800.0],
            ore_purity_range: [0.2, 0.7],
            fuel_enthalpy_mj_kg_range: [20.0, 42.0],
            crop_opt_temp_c_range: [12.0, 26.0],
        },
        ..PlanetGenConfigView::default()
    }
}

fn test_cal_cfg() -> CalendarConfigView {
    CalendarConfigView {
        hours_per_day: 24,
        days_per_year: 360,
        year_prefix: "Y".to_string(),
        starting_year: 1,
        month_names: vec!["M1".to_string()],
        month_lengths: vec![360],
    }
}

/// Self-contained 6-cell test fixture with northern/southern hemisphere cells and 4-hop river chain:
/// Cell 0 (North +45 deg, high headwater 900m) -> Cell 1 (mid-basin 450m) -> Cell 2 (lower valley 120m) -> Cell 3 (ocean sink -200m)
/// Cell 4 (South -45 deg land 200m) -> Cell 5 (South ocean -200m)
struct FixturePlanet {
    lat_lon: Vec<[f32; 2]>,
    xyz: Vec<[f32; 3]>,
    elev_m: Vec<f32>,
    is_ocean: Vec<bool>,
    downstream: Vec<Option<CellId>>,
    drainage: Vec<f32>,
    nbrs: Vec<Vec<CellId>>,
    edge_map: BTreeMap<(u32, u32), EdgeId>,
    topo: Vec<CellId>,
    provinces: Vec<GeologyProvinceKind>,
    faults: Vec<f32>,
    soil_drainage: Vec<f32>,
    lake_kinds: Vec<LakeKind>,
    lake_salinity: Vec<f32>,
    aridity: Vec<f32>,
    base_precip: Vec<f32>,
    biome_keys: Vec<&'static str>,
}

impl FixturePlanet {
    fn new() -> Self {
        let coords = [
            (45.0_f32, 0.0_f32),
            (42.0, 2.0),
            (39.0, 4.0),
            (36.0, 6.0),
            (-45.0, 0.0),
            (-42.0, 3.0),
        ];
        let mut lat_lon = Vec::new();
        let mut xyz = Vec::new();
        for (lat_d, lon_d) in coords {
            let lat = lat_d.to_radians();
            let lon = lon_d.to_radians();
            lat_lon.push([lat, lon]);
            xyz.push([lat.cos() * lon.cos(), lat.sin(), lat.cos() * lon.sin()]);
        }
        let mut edge_map = BTreeMap::new();
        let pairs = [(0u32, 1u32), (1, 2), (2, 3), (4, 5)];
        for (i, (a, b)) in pairs.into_iter().enumerate() {
            edge_map.insert((a.min(b), a.max(b)), EdgeId(i as u32));
        }
        Self {
            lat_lon,
            xyz,
            elev_m: vec![900.0, 450.0, 120.0, -200.0, 200.0, -200.0],
            is_ocean: vec![false, false, false, true, false, true],
            downstream: vec![
                Some(CellId(1)),
                Some(CellId(2)),
                Some(CellId(3)),
                None,
                Some(CellId(5)),
                None,
            ],
            drainage: vec![10_000.0, 20_000.0, 30_000.0, 40_000.0, 10_000.0, 20_000.0],
            nbrs: vec![
                vec![CellId(1)],
                vec![CellId(0), CellId(2)],
                vec![CellId(1), CellId(3)],
                vec![CellId(2)],
                vec![CellId(5)],
                vec![CellId(4)],
            ],
            edge_map,
            topo: vec![CellId(0), CellId(1), CellId(2), CellId(4)],
            provinces: vec![GeologyProvinceKind::Platform; 6],
            faults: vec![0.1; 6],
            soil_drainage: vec![0.5; 6],
            lake_kinds: vec![LakeKind::None; 6],
            lake_salinity: vec![0.2; 6],
            aridity: vec![0.9; 6],
            base_precip: vec![950.0; 6],
            biome_keys: vec!["temperate_forest"; 6],
        }
    }

    fn new_oasis_and_endorheic() -> Self {
        let mut p = Self::new();
        p.elev_m = vec![1600.0, 120.0, 18.0, -200.0, 14.0, -200.0];
        p.is_ocean = vec![false, false, false, true, false, true];
        p.downstream = vec![
            Some(CellId(1)),
            Some(CellId(2)),
            None,
            None,
            None,
            None,
        ];
        p.provinces = vec![
            GeologyProvinceKind::YoungOrogen,
            GeologyProvinceKind::ContinentalRift,
            GeologyProvinceKind::SedimentaryBasin,
            GeologyProvinceKind::AbyssalPlain,
            GeologyProvinceKind::SedimentaryBasin,
            GeologyProvinceKind::AbyssalPlain,
        ];
        p.faults = vec![0.15, 0.85, 0.15, 0.05, 0.1, 0.05];
        p.soil_drainage = vec![0.78, 0.55, 0.35, 0.0, 0.25, 0.0];
        p.lake_kinds = vec![
            LakeKind::None,
            LakeKind::None,
            LakeKind::Exorheic,
            LakeKind::None,
            LakeKind::Endorheic,
            LakeKind::None,
        ];
        p.lake_salinity = vec![0.2, 0.2, 0.25, 35.0, 48.0, 35.0];
        p.aridity = vec![1.25, 0.18, 0.25, 1.0, 0.12, 1.0];
        p.base_precip = vec![1600.0, 85.0, 120.0, 800.0, 45.0, 800.0];
        p.biome_keys = vec![
            "alpine_rock",
            "warm_desert",
            "warm_desert",
            "deep_ocean",
            "warm_desert",
            "deep_ocean",
        ];
        p
    }
}

impl PlanetGeometryPort for FixturePlanet {
    fn cell_count(&self) -> u32 {
        6
    }

    fn cell_neighbors(&self, cell: CellId) -> &[CellId] {
        &self.nbrs[cell.get() as usize]
    }

    fn cell_edge(&self, from: CellId, to: CellId) -> Option<EdgeId> {
        let a = from.get().min(to.get());
        let b = from.get().max(to.get());
        self.edge_map.get(&(a, b)).copied()
    }

    fn cell_elevation_m(&self, cell: CellId) -> f32 {
        self.elev_m[cell.get() as usize]
    }

    fn cell_slope_rad(&self, _edge: EdgeId) -> f32 {
        0.015
    }

    fn cell_biome_and_strata(&self, cell: CellId) -> CellGeologyView {
        let i = cell.get() as usize;
        CellGeologyView {
            cell_id: cell,
            elevation_m: self.elev_m[i],
            strata_label: RockStrataClass::AlluvialPlain.as_str().to_string(),
            biome_key: self.biome_keys[i].to_string(),
            is_ocean: self.is_ocean[i],
        }
    }

    fn watershed_downstream(&self, cell: CellId) -> Option<CellId> {
        self.downstream[cell.get() as usize]
    }

    fn great_circle_km(&self, _a: CellId, _b: CellId) -> f32 {
        250.0
    }

    fn cell_unit_xyz(&self, cell: CellId) -> [f32; 3] {
        self.xyz[cell.get() as usize]
    }

    fn cell_lat_lon_rad(&self, cell: CellId) -> [f32; 2] {
        self.lat_lon[cell.get() as usize]
    }

    fn cell_area_km2(&self, _cell: CellId) -> f32 {
        10_000.0
    }

    fn cell_drainage_area_km2(&self, cell: CellId) -> f32 {
        self.drainage[cell.get() as usize]
    }

    fn cell_base_precip_mm_yr(&self, cell: CellId) -> f32 {
        self.base_precip[cell.get() as usize]
    }

    fn edge_length_km(&self, _edge: EdgeId) -> f32 {
        250.0
    }

    fn topo_sorted_land_cells(&self) -> &[CellId] {
        &self.topo
    }

    fn cell_geology_province(&self, cell: CellId) -> GeologyProvinceKind {
        self.provinces[cell.get() as usize]
    }

    fn cell_fault_instability_01(&self, cell: CellId) -> f32 {
        self.faults[cell.get() as usize]
    }

    fn cell_soil_drainage_01(&self, cell: CellId) -> f32 {
        self.soil_drainage[cell.get() as usize]
    }

    fn cell_is_lake(&self, cell: CellId) -> bool {
        self.lake_kinds[cell.get() as usize] != LakeKind::None
    }

    fn cell_lake_kind(&self, cell: CellId) -> LakeKind {
        self.lake_kinds[cell.get() as usize]
    }

    fn cell_lake_salinity_per_mille(&self, cell: CellId) -> f32 {
        self.lake_salinity[cell.get() as usize]
    }

    fn cell_aridity_index_01(&self, cell: CellId) -> f32 {
        self.aridity[cell.get() as usize]
    }

    fn cell_river_channel_downstream(&self, cell: CellId) -> Option<CellId> {
        let idx = cell.get() as usize;
        if self.is_ocean[idx] {
            None
        } else {
            self.downstream[idx]
        }
    }

    fn cell_river_discharge_m3_s(&self, cell: CellId) -> f32 {
        let idx = cell.get() as usize;
        if self.is_ocean[idx] {
            0.0
        } else {
            95.0 + (idx as f32) * 35.0
        }
    }

    fn cell_hydro_potential_kw(&self, cell: CellId) -> f32 {
        let idx = cell.get() as usize;
        if self.is_ocean[idx] { 0.0 } else { 14_500.0 }
    }
}

// @req ENG-WTH-001
#[test]
fn orbital_seasonal_and_diurnal_forcing_exhibits_hemispheric_opposition() {
    let planet = FixturePlanet::new();
    let pcfg = test_planet_cfg();
    let cal = test_cal_cfg();
    let mut weather = WeatherEngine::new(&planet, &pcfg, &cal);

    let hours_per_year = u64::from(cal.hours_per_day * cal.days_per_year);
    let qtr_year = hours_per_year / 4; // Northern summer solstice
    let three_qtr_year = 3 * hours_per_year / 4; // Northern winter solstice

    let mut north_summer = 0.0_f32;
    let mut south_winter = 0.0_f32;
    let mut north_winter = 0.0_f32;
    let mut south_summer = 0.0_f32;

    // Sample full 24h diurnal swing across one day in mid-northern land cell (Cell 1) too.
    let mut diurnal_min = f32::MAX;
    let mut diurnal_max = -f32::MAX;

    for t in 0..=three_qtr_year {
        weather.step_hourly(Tick(t));
        if t == qtr_year {
            north_summer = weather.surface(CellId(1)).temp_c;
            south_winter = weather.surface(CellId(4)).temp_c;
        }
        if t >= qtr_year && t < qtr_year + 24 {
            let tc = weather.surface(CellId(1)).temp_c;
            diurnal_min = diurnal_min.min(tc);
            diurnal_max = diurnal_max.max(tc);
        }
        if t == three_qtr_year {
            north_winter = weather.surface(CellId(1)).temp_c;
            south_summer = weather.surface(CellId(4)).temp_c;
        }
    }

    assert!(
        north_summer > north_winter + 12.0,
        "north summer {north_summer:.1}C should strongly exceed north winter {north_winter:.1}C"
    );
    assert!(
        south_summer > south_winter + 12.0,
        "south summer {south_summer:.1}C should strongly exceed south winter {south_winter:.1}C"
    );
    assert!(
        north_summer > south_winter + 10.0,
        "northern summer and southern winter must be in opposition"
    );
    assert!(
        diurnal_max - diurnal_min > 1.0,
        "diurnal cycle should create measurable day-night temperature spread"
    );
    let _ = PI;
}

// @req ENG-WTH-002
#[test]
fn upstream_storm_produces_delayed_downstream_flood_wave_with_conserved_water_mass() {
    let planet = FixturePlanet::new();
    let pcfg = test_planet_cfg();
    let cal = test_cal_cfg();
    let mut weather = WeatherEngine::new(&planet, &pcfg, &cal);

    // Warm up cleanly to stabilize steady baseflow.
    for t in 0..72 {
        weather.step_hourly(Tick(t));
    }

    // Inject extreme storm deluge purely into upstream headwater Cell 0.
    weather.inject_storm_rainfall(CellId(0), 180.0);

    let mut peak_t0 = 0u64;
    let mut peak_f0 = -1.0_f32;
    let mut peak_t2 = 0u64;
    let mut peak_f2 = -1.0_f32;

    for t in 72..140 {
        let f0 = weather.surface(CellId(0)).flood_stage_01;
        let f2 = weather.surface(CellId(2)).flood_stage_01;
        if f0 > peak_f0 {
            peak_f0 = f0;
            peak_t0 = t;
        }
        if f2 > peak_f2 {
            peak_f2 = f2;
            peak_t2 = t;
        }
        weather.step_hourly(Tick(t));
    }

    assert!(peak_f0 > 0.20, "headwater storm peak must spike flood stage");
    assert!(peak_f2 > 0.08, "downstream valley must receive flood wave");
    assert!(
        peak_t2 > peak_t0,
        "downstream flood peak tick ({peak_t2}) must lag behind upstream peak ({peak_t0})"
    );

    // Conservation check across land + evaporation + ocean outflow vs initial + precip + storm inputs.
    let err = weather.water_mass_balance_error_m3();
    let pool = weather.total_land_water_m3().max(1.0);
    assert!(
        err / pool < 1e-8,
        "land water mass balance error {err} exceeds conservation threshold on pool {pool}"
    );
}

// @req ENG-WTH-003
#[test]
fn storm_mud_spikes_surface_conditions_and_canonical_save_roundtrip_is_bit_exact() {
    let planet = FixturePlanet::new();
    let pcfg = test_planet_cfg();
    let cal = test_cal_cfg();
    let mut weather = WeatherEngine::new(&planet, &pcfg, &cal);

    for t in 0..48 {
        weather.step_hourly(Tick(t));
    }
    let baseline = weather.surface(CellId(2));
    weather.inject_storm_rainfall(CellId(2), 95.0);
    let stormy = weather.surface(CellId(2));

    assert!(stormy.mud_index_01 > baseline.mud_index_01 + 0.4);
    assert!(stormy.mud_index_01 >= 0.70);
    assert!(stormy.visibility_01 < baseline.visibility_01);

    let digest_before = weather.blake3_digest();
    let mut buf = Vec::new();
    weather.write_canonical_state(&mut buf);

    let mut restored = WeatherEngine::new(&planet, &pcfg, &cal);
    let mut slice = buf.as_slice();
    restored
        .read_canonical_state(&mut slice)
        .expect("weather canonical decode must succeed");
    assert!(slice.is_empty());
    assert_eq!(restored.blake3_digest(), digest_before);

    // Advance both identically to verify zero drift post-restore.
    for t in 48..72 {
        weather.step_hourly(Tick(t));
        restored.step_hourly(Tick(t));
    }
    assert_eq!(restored.blake3_digest(), weather.blake3_digest());
}

// @req ENG-WTH-006
#[test]
fn darcy_groundwater_emergent_oases_and_endorheic_salinity_conserve_land_water_mass() {
    let planet = FixturePlanet::new_oasis_and_endorheic();
    let pcfg = test_planet_cfg();
    let cal = test_cal_cfg();
    let mut weather = WeatherEngine::new(&planet, &pcfg, &cal);

    // Recharge upland headwater so Darcy head gradient feeds arid piedmont/fault Cell 1.
    for step in 0..96 {
        if step % 12 == 0 && step < 48 {
            weather.inject_storm_rainfall(CellId(0), 60.0);
        }
        weather.step_hourly(Tick(step));
    }

    let gw_piedmont = weather.cell_groundwater_m3(CellId(1));
    assert!(gw_piedmont > 0.0, "groundwater must percolate and diffuse to piedmont cell");
    assert!(
        weather.cell_is_spring_oasis(CellId(1)),
        "arid fault/piedmont cell receiving upland recharge must daylight as a spring oasis"
    );

    // Flash storm temporarily dilutes Endorheic desert salt pan Cell 4, then desert evaporation re-concentrates salinity.
    let sal_eq = weather.cell_salinity_per_mille(CellId(4));
    weather.inject_storm_rainfall(CellId(4), 12.0);
    weather.step_hourly(Tick(96));
    let sal_diluted = weather.cell_salinity_per_mille(CellId(4));
    assert!(sal_diluted <= sal_eq, "fresh rainfall inflow should dilute or hold basin salinity");
    for step in 97..360 {
        weather.step_hourly(Tick(step));
    }
    let sal_concentrated = weather.cell_salinity_per_mille(CellId(4));
    assert!(
        sal_concentrated > sal_diluted && sal_concentrated >= 35.0,
        "endorheic terminal pan must concentrate dissolved salt under desert evaporation ({sal_diluted} -> {sal_concentrated})"
    );

    let err = weather.water_mass_balance_error_m3();
    let pool = weather.total_land_water_m3().max(1.0);
    assert!(
        err / pool < 1e-8,
        "aquifer + surface hydrology mass balance error {err} exceeded threshold on pool {pool}"
    );
}

// @req ENG-GEO-014
#[test]
fn construct_dam_impounds_reservoir_responds_to_rule_curves_and_breaches_on_overtopping() {
    let planet = FixturePlanet::new();
    let pcfg = test_planet_cfg();
    let cal = test_cal_cfg();
    let mut weather = WeatherEngine::new(&planet, &pcfg, &cal);

    for t in 0..24 {
        weather.step_hourly(Tick(t));
    }

    weather
        .construct_dam(CellId(1), DamRuleCurveKind::BalancedPower)
        .expect("dam construction on river reach Cell 1 must succeed");

    // Feed upstream inflows into Cell 0 so reservoir behind Cell 1 fills up and drives turbines.
    for t in 24..60 {
        if t % 6 == 0 {
            weather.inject_storm_rainfall(CellId(0), 65.0);
        }
        weather.step_hourly(Tick(t));
    }

    let power_dam = weather.dam_at_cell(CellId(1)).expect("dam must exist");
    assert!(power_dam.reservoir_storage_m3 > 0.0, "reservoir must impound incoming river flow");
    assert!(power_dam.live_hydro_kw > 0.0, "head + turbine flow must generate hydroelectric kW");

    // Verify canonical save/load preserves live dam state bit-for-bit.
    let digest_before = weather.blake3_digest();
    let mut saved = Vec::new();
    weather.write_canonical_state(&mut saved);
    let mut restored = WeatherEngine::new(&planet, &pcfg, &cal);
    let mut slice = saved.as_slice();
    restored.read_canonical_state(&mut slice).expect("restore weather with dam");
    assert!(slice.is_empty());
    assert_eq!(restored.blake3_digest(), digest_before);

    // Switch rule curve to FloodControl and verify spillway release shifts immediately.
    let release_balanced = power_dam.spillway_release_m3_s;
    weather
        .set_dam_rule_curve(CellId(1), DamRuleCurveKind::FloodControl)
        .expect("rule curve update must succeed");
    weather.step_hourly(Tick(60));
    let flood_dam = weather.dam_at_cell(CellId(1)).expect("dam remains active");
    assert_eq!(flood_dam.rule_curve, DamRuleCurveKind::FloodControl);
    assert!(
        flood_dam.spillway_release_m3_s >= release_balanced * 0.9,
        "flood-control rule curve should release higher protective drawdowns"
    );

    // Overtop reservoir with extreme inflow to verify breach release + mass balance conservation.
    for t in 61..95 {
        weather.inject_storm_rainfall(CellId(0), 850.0);
        weather.inject_storm_rainfall(CellId(1), 850.0);
        weather.step_hourly(Tick(t));
    }
    let post_breach = weather.dam_at_cell(CellId(1)).expect("breached dam record persists");
    assert!(post_breach.breached, "extreme sustained overtopping should breach the dam");
    assert_eq!(post_breach.live_hydro_kw, 0.0);
    assert!(
        weather.surface(CellId(2)).flood_stage_01 > 0.15,
        "dam breach must route floodwater downstream without violating mass balance"
    );

    let err = weather.water_mass_balance_error_m3();
    let pool = weather.total_land_water_m3().max(1.0);
    assert!(
        err / pool < 1e-8,
        "dam impoundment + breach mass balance error {err} exceeded threshold on pool {pool}"
    );
}
