#![deny(clippy::disallowed_types)]

use std::collections::BTreeMap;
use std::f32::consts::PI;

use sim_spine::{
    CalendarConfigView, CanonicalSaveablePort, CellGeologyView, CellId, EdgeId, NaturalBoundsView,
    PlanetGenConfigView, PlanetGeometryPort, RockStrataClass, SurfaceConditionsPort, Tick,
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
        }
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
            biome_key: "temperate_forest".to_string(),
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

    fn cell_base_precip_mm_yr(&self, _cell: CellId) -> f32 {
        950.0
    }

    fn edge_length_km(&self, _edge: EdgeId) -> f32 {
        250.0
    }

    fn topo_sorted_land_cells(&self) -> &[CellId] {
        &self.topo
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
