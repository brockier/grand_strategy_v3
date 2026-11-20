#![deny(clippy::disallowed_types)]

use std::collections::BTreeSet;
use std::f64::consts::PI;

use sim_planet::topology::norm3;
use sim_planet::GeneratedPlanet;
use sim_spine::{
    CanonicalSaveablePort, CellId, GeologyProvinceKind, LakeKind, NaturalBoundsView,
    PlanetGenConfigView, PlanetGeometryPort, ProceduralDepositCategory, RiverNavigabilityKind,
    RockStrataClass, SubsurfaceDepositTruthPort,
};

fn default_planet_cfg(subdiv: u8) -> PlanetGenConfigView {
    PlanetGenConfigView {
        id: "test_gaia".to_string(),
        goldberg_subdivision: subdiv,
        tectonic_plate_count: 14,
        land_fraction_target: 0.34,
        axial_tilt_deg: 23.44,
        solar_constant_w_m2: 1361.0,
        rotation_period_hours: 24.0,
        mean_radius_km: 6371.0,
        base_sea_level_m: 0.0,
        max_orogenic_uplift_m: 4800.0,
        planetary_density_kg_m3: 5514.0,
        natural_bounds: NaturalBoundsView {
            ore_count: 4,
            flux_count: 3,
            fuel_count: 4,
            crop_count: 5,
            fauna_count: 4,
            pathogen_count: 3,
            ore_melting_k_range: [950.0, 1950.0],
            ore_purity_range: [0.18, 0.72],
            fuel_enthalpy_mj_kg_range: [14.0, 44.0],
            crop_opt_temp_c_range: [10.0, 29.0],
        },
        ..PlanetGenConfigView::default()
    }
}

// @req ENG-GEO-001.1
#[test]
fn geodesic_topology_has_exact_euler_counts_symmetry_and_spherical_area() {
    let cfg = default_planet_cfg(3); // GP(8, 0) -> 10 * 8^2 + 2 = 642 sample nodes, 20 * 8^2 = 1280 triangles
    let planet = GeneratedPlanet::generate(42, &cfg);
    let n = planet.cell_count();
    assert_eq!(n, 642);

    let tris = planet.icosphere_triangles();
    assert_eq!(tris.len(), 1280, "must expose 20 * m^2 welded icosphere triangles");
    // Check Euler characteristic V - E + F == 2 and outward CCW winding on every triangle.
    let v_count = n as i64;
    let e_count = planet.topology.edge_endpoints.len() as i64;
    let f_count = tris.len() as i64;
    assert_eq!(v_count - e_count + f_count, 2, "icosphere Euler invariant V - E + F = 2");
    for &[a, b, c] in tris {
        let pa = planet.cell_unit_xyz(CellId(a));
        let pb = planet.cell_unit_xyz(CellId(b));
        let pc = planet.cell_unit_xyz(CellId(c));
        let ab = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
        let ac = [pc[0] - pa[0], pc[1] - pa[1], pc[2] - pa[2]];
        let nx = ab[1] * ac[2] - ab[2] * ac[1];
        let ny = ab[2] * ac[0] - ab[0] * ac[2];
        let nz = ab[0] * ac[1] - ab[1] * ac[0];
        let cx = pa[0] + pb[0] + pc[0];
        let cy = pa[1] + pb[1] + pc[1];
        let cz = pa[2] + pb[2] + pc[2];
        assert!(nx * cx + ny * cy + nz * cz > 0.0, "triangle winding must be outward CCW");
    }

    let mut deg5 = 0u32;
    let mut deg6 = 0u32;
    let mut area_sum = 0.0_f64;

    for raw in 0..n {
        let cid = CellId(raw);
        let nbrs = planet.cell_neighbors(cid);
        match nbrs.len() {
            5 => deg5 += 1,
            6 => deg6 += 1,
            other => panic!("unexpected vertex degree {other}"),
        }
        for &nb in nbrs {
            assert!(
                planet.cell_neighbors(nb).contains(&cid),
                "neighbor relation must be symmetric"
            );
            let eid = planet
                .cell_edge(cid, nb)
                .expect("adjacent cells must share an EdgeId");
            assert!(planet.edge_length_km(eid) > 0.0);
        }
        area_sum += f64::from(planet.cell_area_km2(cid));
    }

    assert_eq!(deg5, 12);
    assert_eq!(deg6, n - 12);

    let expected_area = 4.0 * PI * f64::from(cfg.mean_radius_km).powi(2);
    let rel_err = (area_sum - expected_area).abs() / expected_area;
    assert!(
        rel_err < 1e-4,
        "spherical dual area sum {area_sum} diverged from {expected_area}"
    );

    // Continuous sphere sampling, barycentric elevation interpolation, and natural watershed assignment.
    let c0 = CellId(10);
    let nb0 = planet.cell_neighbors(c0)[0];
    let p0 = planet.cell_unit_xyz(c0);
    let p1 = planet.cell_unit_xyz(nb0);
    let mid = [p0[0] + p1[0], p0[1] + p1[1], p0[2] + p1[2]];
    let len = (mid[0] * mid[0] + mid[1] * mid[1] + mid[2] * mid[2]).sqrt();
    let u_mid = [mid[0] / len, mid[1] / len, mid[2] / len];
    assert_eq!(planet.nearest_sample_cell(p0), c0);
    let sampled_elev = planet.sample_elevation_at_unit_xyz(u_mid);
    let e0 = planet.cell_elevation_m(c0);
    let e1 = planet.cell_elevation_m(nb0);
    let lo = e0.min(e1) - 1e-2;
    let hi = e0.max(e1) + 1e-2;
    assert!(
        sampled_elev >= lo && sampled_elev <= hi,
        "continuous midpoint elevation {sampled_elev} must interpolate within [{lo}, {hi}]"
    );
    let mut land_watershed_count = 0usize;
    for raw in 0..n {
        let cid = CellId(raw);
        if !planet.is_ocean[raw as usize] && planet.watershed_id(cid).0 > 0 {
            land_watershed_count += 1;
        }
    }
    assert!(land_watershed_count > 0, "land samples must belong to positive natural watersheds");

    // Great-circle distance bounded cleanly by [0, PI * R].
    let antipodal_max = (PI * f64::from(cfg.mean_radius_km)) as f32 + 1.0;
    for raw in (0..n).step_by(29) {
        let a = CellId(raw);
        let b = CellId((raw + 137) % n);
        let d = planet.great_circle_km(a, b);
        assert!(d > 0.0 && d <= antipodal_max);
        assert!((d - planet.great_circle_km(b, a)).abs() < 1e-3);
    }
}

// @req ENG-GEO-002
#[test]
fn tectonic_convergent_boundaries_uplift_statistically_and_hit_land_fraction() {
    let cfg = default_planet_cfg(4); // GP(16, 0) -> 2562 cells for strong sample size
    let planet = GeneratedPlanet::generate(777, &cfg);

    let mut total_area = 0.0_f64;
    let mut land_area = 0.0_f64;
    let mut conv_elev_sum = 0.0_f64;
    let mut conv_count = 0usize;
    let mut int_elev_sum = 0.0_f64;
    let mut int_count = 0usize;

    for raw in 0..planet.cell_count() {
        let idx = raw as usize;
        let area = f64::from(planet.cell_area_km2(CellId(raw)));
        total_area += area;
        if !planet.is_ocean[idx] {
            land_area += area;
        }
        if planet.is_convergent_boundary[idx] {
            conv_elev_sum += f64::from(planet.elevation_m[idx]);
            conv_count += 1;
        } else if !planet.is_divergent_boundary[idx] {
            int_elev_sum += f64::from(planet.elevation_m[idx]);
            int_count += 1;
        }
    }

    let land_frac = (land_area / total_area) as f32;
    assert!(
        (land_frac - cfg.land_fraction_target).abs() < 0.03,
        "land fraction {land_frac} missed target {}",
        cfg.land_fraction_target
    );
    assert!(conv_count > 10 && int_count > 100);
    let mean_conv = conv_elev_sum / (conv_count as f64);
    let mean_int = int_elev_sum / (int_count as f64);
    assert!(
        mean_conv > mean_int + 450.0,
        "convergent plate boundary mean {mean_conv:.1}m should strongly exceed plate interior {mean_int:.1}m"
    );
}

// @req ENG-GEO-003
#[test]
fn subsurface_deposits_follow_strata_affinities_and_remain_sealed_from_geometry_port() {
    let cfg = default_planet_cfg(4);
    let planet = GeneratedPlanet::generate(2026, &cfg);

    let mut ore_cells = 0usize;
    let mut ore_in_igneous_shield = 0usize;
    let mut igneous_shield_cells = 0usize;
    let mut sed_alluvial_cells = 0usize;
    let mut ore_in_sed_alluvial = 0usize;
    let mut seen_categories = BTreeSet::new();

    for raw in 0..planet.cell_count() {
        let cid = CellId(raw);
        let strata = planet.strata_class(cid);
        let deps = planet.deposits_in_cell(cid);
        let has_ore = deps
            .iter()
            .any(|d| d.category == ProceduralDepositCategory::MetallicOre);
        for d in deps {
            seen_categories.insert(d.category);
            assert!(d.total_mass_tonnes > 0.0);
            assert!(d.average_purity_01 > 0.0 && d.average_purity_01 <= 1.0);
        }
        if has_ore {
            ore_cells += 1;
        }
        match strata {
            RockStrataClass::IgneousOrogenic
            | RockStrataClass::VolcanicArc
            | RockStrataClass::MetamorphicShield => {
                igneous_shield_cells += 1;
                if has_ore {
                    ore_in_igneous_shield += 1;
                }
            }
            RockStrataClass::SedimentaryBasin | RockStrataClass::AlluvialPlain => {
                sed_alluvial_cells += 1;
                if has_ore {
                    ore_in_sed_alluvial += 1;
                }
            }
            RockStrataClass::OceanicBasalt => {}
        }

        // Verify PlanetGeometryPort view returns only surface fields.
        let geo = planet.cell_biome_and_strata(cid);
        assert_eq!(geo.cell_id, cid);
        assert!(!geo.strata_label.is_empty());
        assert!(!geo.biome_key.is_empty());
    }

    assert!(ore_cells > 20);
    assert_eq!(
        seen_categories.len(),
        4,
        "all 4 subsurface deposit categories should generate globally"
    );
    let ign_rate = (ore_in_igneous_shield as f64) / (igneous_shield_cells.max(1) as f64);
    let sed_rate = (ore_in_sed_alluvial as f64) / (sed_alluvial_cells.max(1) as f64);
    assert!(
        ign_rate > sed_rate * 2.0,
        "metallic ore probability in igneous/shield strata ({ign_rate:.3}) should strongly exceed sedimentary/alluvial ({sed_rate:.3})"
    );
}

// @req ENG-GEO-004
#[test]
fn watersheds_are_strictly_acyclic_monotone_and_windward_slopes_beat_rain_shadows() {
    let cfg = default_planet_cfg(4);
    let planet = GeneratedPlanet::generate(9911, &cfg);
    let n = planet.cell_count();
    let max_hops = n as usize + 1;

    // Verify acyclic downhill drainage and ocean termination for every land cell.
    let mut land_count = 0usize;
    for raw in 0..n {
        let cid = CellId(raw);
        if planet.is_ocean_cell(cid) {
            assert_eq!(planet.watershed_downstream(cid), None);
            continue;
        }
        land_count += 1;
        let mut cur = cid;
        let mut hops = 0usize;
        while let Some(next) = planet.watershed_downstream(cur) {
            hops += 1;
            assert!(hops <= max_hops, "cycle detected in watershed DAG");
            assert!(
                planet.cell_elevation_m(next) < planet.cell_elevation_m(cur),
                "watershed downstream elevation must strictly decrease"
            );
            cur = next;
        }
        assert!(
            planet.is_ocean_cell(cur),
            "every land watershed path must terminate at an ocean sink"
        );
    }

    assert_eq!(planet.topo_sorted_land_cells().len(), land_count);

    // Check headwaters-before-downstream invariant on topo_sorted_land_cells.
    let mut topo_pos = vec![usize::MAX; n as usize];
    for (pos, &cid) in planet.topo_sorted_land_cells().iter().enumerate() {
        topo_pos[cid.get() as usize] = pos;
    }
    for &cid in planet.topo_sorted_land_cells() {
        if let Some(ds) = planet.watershed_downstream(cid)
            && !planet.is_ocean_cell(ds)
        {
            assert!(
                topo_pos[cid.get() as usize] < topo_pos[ds.get() as usize],
                "upstream cell must appear before downstream cell in topo sort"
            );
        }
    }

    // Windward orographic uplift precipitation vs leeward descending rain shadow.
    let mut windward_precip = 0.0_f64;
    let mut windward_n = 0usize;
    let mut leeward_precip = 0.0_f64;
    let mut leeward_n = 0usize;

    for raw in 0..n {
        let cid = CellId(raw);
        if planet.is_ocean_cell(cid) {
            continue;
        }
        let pos = planet.cell_unit_xyz(cid);
        let (east, north) = sim_planet::topology::tangent_basis([
            f64::from(pos[0]),
            f64::from(pos[1]),
            f64::from(pos[2]),
        ]);
        let [wu, wv] = planet.cell_prevailing_wind_uv(cid);
        let wind_3d = norm3([
            east[0] * f64::from(wu) + north[0] * f64::from(wv),
            east[1] * f64::from(wu) + north[1] * f64::from(wv),
            east[2] * f64::from(wu) + north[2] * f64::from(wv),
        ]);
        let mut best_upwind = None;
        let mut best_align = -1.0_f64;
        for &nb in planet.cell_neighbors(cid) {
            let np = planet.cell_unit_xyz(nb);
            let dir = norm3([
                f64::from(pos[0] - np[0]),
                f64::from(pos[1] - np[1]),
                f64::from(pos[2] - np[2]),
            ]);
            let align = dir[0] * wind_3d[0] + dir[1] * wind_3d[1] + dir[2] * wind_3d[2];
            if align > best_align {
                best_align = align;
                best_upwind = Some(nb);
            }
        }
        if let Some(up_cell) = best_upwind {
            let dh = planet.cell_elevation_m(cid) - planet.cell_elevation_m(up_cell);
            let p = f64::from(planet.cell_base_precip_mm_yr(cid));
            if dh > 180.0 {
                windward_precip += p;
                windward_n += 1;
            } else if dh < -180.0 {
                leeward_precip += p;
                leeward_n += 1;
            }
        }
    }

    assert!(windward_n > 20 && leeward_n > 20);
    let mean_w = windward_precip / (windward_n as f64);
    let mean_l = leeward_precip / (leeward_n as f64);
    assert!(
        mean_w > mean_l * 1.15,
        "windward mean precip {mean_w:.1} mm/yr must exceed leeward rain shadow {mean_l:.1} mm/yr"
    );
}

// @req ENG-GEO-005
#[test]
fn procedural_material_catalog_enforces_physical_bounds_and_mass_energy_laws() {
    let cfg = default_planet_cfg(3);
    let p_a = GeneratedPlanet::generate(100, &cfg);
    let p_b = GeneratedPlanet::generate(200, &cfg);

    assert_ne!(p_a.materials.ores[0].key, "");
    assert_ne!(p_a.materials.ores, p_b.materials.ores);
    assert_eq!(p_a.materials.ores.len(), usize::from(cfg.natural_bounds.ore_count));
    assert_eq!(p_a.materials.fluxes.len(), usize::from(cfg.natural_bounds.flux_count));
    assert_eq!(p_a.materials.fuels.len(), usize::from(cfg.natural_bounds.fuel_count));
    assert_eq!(p_a.materials.crops.len(), usize::from(cfg.natural_bounds.crop_count));
    assert_eq!(p_a.materials.fauna.len(), usize::from(cfg.natural_bounds.fauna_count));
    assert_eq!(
        p_a.materials.pathogens.len(),
        usize::from(cfg.natural_bounds.pathogen_count)
    );

    for ore in &p_a.materials.ores {
        assert!(ore.melting_point_k >= cfg.natural_bounds.ore_melting_k_range[0]);
        assert!(ore.melting_point_k <= cfg.natural_bounds.ore_melting_k_range[1]);
        assert!(ore.base_grade_01 >= cfg.natural_bounds.ore_purity_range[0]);
        assert!(ore.base_grade_01 <= cfg.natural_bounds.ore_purity_range[1]);
        assert!(ore.max_recoverable_yield_01 <= ore.base_grade_01);
        assert!(ore.max_recoverable_yield_01 <= 1.0 - ore.trace_impurity_01 + 1e-6);
    }

    for fuel in &p_a.materials.fuels {
        assert!(fuel.calorific_enthalpy_mj_kg >= cfg.natural_bounds.fuel_enthalpy_mj_kg_range[0]);
        assert!(fuel.calorific_enthalpy_mj_kg <= cfg.natural_bounds.fuel_enthalpy_mj_kg_range[1]);
        assert!(fuel.useful_energy_mj_kg <= fuel.calorific_enthalpy_mj_kg);
    }
}

// @req ENG-GEO-006
#[test]
fn planet_generation_and_canonical_codec_are_bit_identical_for_same_seed() {
    let cfg = default_planet_cfg(3);
    let p1 = GeneratedPlanet::generate(54321, &cfg);
    let p2 = GeneratedPlanet::generate(54321, &cfg);
    let p_diff = GeneratedPlanet::generate(54322, &cfg);

    assert_eq!(p1.blake3_digest(), p2.blake3_digest());
    assert_ne!(p1.blake3_digest(), p_diff.blake3_digest());
    assert_eq!(p1.elevation_m, p2.elevation_m);
    assert_eq!(p1.deposits, p2.deposits);

    // Canonical binary save/load round-trip preserves Blake3 digest cleanly.
    let mut encoded = Vec::new();
    p1.write_canonical_state(&mut encoded);
    let mut restored = GeneratedPlanet::generate(1, &cfg);
    let mut slice = encoded.as_slice();
    restored
        .read_canonical_state(&mut slice)
        .expect("canonical decode must succeed");
    assert!(slice.is_empty());
    assert_eq!(restored.blake3_digest(), p1.blake3_digest());
}

// @req ENG-GEO-007
#[test]
fn high_res_planet_generates_continental_shelves_strahler_rivers_and_biophysical_indices() {
    let cfg = default_planet_cfg(24);
    let planet = GeneratedPlanet::generate(42, &cfg);
    assert_eq!(planet.cell_count(), 5762);
    assert_eq!(planet.icosphere_triangles().len(), 11520);

    let mut ocean_cells = 0usize;
    let mut shelf_cells = 0usize;
    let mut abyssal_cells = 0usize;
    let mut max_strahler = 0u8;

    for raw in 0..planet.cell_count() {
        let cid = CellId(raw);
        let elev = planet.cell_elevation_m(cid);
        let aridity = planet.cell_aridity_index_01(cid);
        let fertility = planet.cell_soil_fertility_01(cid);
        assert!(aridity.is_finite() && (0.0..=1.0).contains(&aridity));
        assert!(fertility.is_finite() && (0.0..=1.0).contains(&fertility));
        if planet.is_ocean_cell(cid) {
            ocean_cells += 1;
            if (-200.0..=0.0).contains(&elev) {
                shelf_cells += 1;
            } else if elev < -2000.0 {
                abyssal_cells += 1;
            }
            assert_eq!(planet.cell_strahler_order(cid), 0);
        } else {
            let ord = planet.cell_strahler_order(cid);
            assert!(ord >= 1);
            max_strahler = max_strahler.max(ord);
        }
    }

    assert!(ocean_cells > 1000);
    assert!(
        shelf_cells * 20 >= ocean_cells,
        "continental shelf cells ({shelf_cells}) must represent at least 5% of ocean cells ({ocean_cells})"
    );
    assert!(abyssal_cells > ocean_cells / 4);
    assert!(
        max_strahler >= 3,
        "m=24 planet must develop hierarchical river basins with Strahler >= 3 (got {max_strahler})"
    );

    // Verify canonical save round-trip preserves new biophysical fields cleanly at m=24.
    let mut buf = Vec::new();
    planet.write_canonical_state(&mut buf);
    let mut decoded = GeneratedPlanet::generate(7, &default_planet_cfg(3));
    let mut slice = buf.as_slice();
    decoded.read_canonical_state(&mut slice).expect("m=24 decode");
    assert!(slice.is_empty());
    assert_eq!(decoded.blake3_digest(), planet.blake3_digest());
}

// @req ENG-GEO-008
#[test]
fn perennial_rivers_conserve_volumetric_flow_form_lakes_and_terminate_in_river_lake_or_sea() {
    let cfg = default_planet_cfg(24);
    let mut observed_lakes = 0usize;
    let mut observed_perennial = 0usize;
    let mut observed_hydro_sites = 0usize;

    for seed in [42_u64, 7_u64, 1337_u64] {
        let planet = GeneratedPlanet::generate(seed, &cfg);
        let n = planet.cell_count();
        for idx in 0..n {
            let cid = CellId(idx);
            let q = planet.cell_river_discharge_m3_s(cid);
            assert!(q.is_finite() && q >= 0.0);

            if planet.is_ocean_cell(cid) {
                assert!(!planet.cell_is_lake(cid));
                assert!(planet.cell_river_channel_downstream(cid).is_none());
                assert_eq!(q, 0.0);
                continue;
            }

            if let Some(ds) = planet.cell_river_channel_downstream(cid) {
                observed_perennial += 1;
                let vel = planet.cell_river_velocity_mps(cid);
                let kw = planet.cell_hydro_potential_kw(cid);
                assert!(vel > 0.0 && vel.is_finite(), "perennial channel must have positive flow speed");
                assert!(kw >= 0.0 && kw.is_finite(), "hydro potential must be finite non-negative");
                if kw > 1.0 {
                    observed_hydro_sites += 1;
                }
                assert!(q >= 18.0, "perennial channel must maintain physical discharge");

                // Every perennial river edge must terminate in ocean, lake, or another perennial river confluence.
                let terminates_validly = planet.is_ocean_cell(ds)
                    || planet.cell_is_lake(ds)
                    || planet.cell_river_channel_downstream(ds).is_some();
                assert!(
                    terminates_validly,
                    "perennial river cell {idx} -> {} terminated mid-land without sea, lake, or confluence",
                    ds.get()
                );

                // Volumetric discharge accumulates strictly downstream along land channels.
                if !planet.is_ocean_cell(ds) {
                    let q_ds = planet.cell_river_discharge_m3_s(ds);
                    assert!(
                        q_ds + 1e-3 >= q,
                        "downstream discharge must conserve and accumulate flow ({q_ds} < {q})"
                    );
                }
            }
            if planet.cell_is_lake(cid) {
                observed_lakes += 1;
            }
        }
    }

    assert!(observed_perennial > 200, "expected substantial perennial channels across 3 seeds");
    assert!(observed_hydro_sites > 100, "expected positive hydroelectric reach drops");
    assert!(observed_lakes >= 1, "expected inland lakes in interior basins across m=24 planets");
}

// @req ENG-GEO-009
#[test]
fn geomorphology_roughness_basins_and_tectonic_hazards_vary_across_relief() {
    let cfg = default_planet_cfg(12);
    let planet = GeneratedPlanet::generate(42, &cfg);
    let n = planet.cell_count();
    let mut min_rough = f32::MAX;
    let mut max_rough = 0.0_f32;
    let mut basin_cells = 0usize;
    let mut active_fault_cells = 0usize;
    let mut active_volc_cells = 0usize;

    for idx in 0..n {
        let cid = CellId(idx);
        let r = planet.cell_roughness_m(cid);
        let bd = planet.cell_basin_depth_m(cid);
        let fi = planet.cell_fault_instability_01(cid);
        let vh = planet.cell_volcanic_hazard_01(cid);

        assert!(r.is_finite() && r >= 0.0);
        assert!(bd.is_finite() && bd >= 0.0);
        assert!((0.0..=1.0).contains(&fi));
        assert!((0.0..=1.0).contains(&vh));

        min_rough = min_rough.min(r);
        max_rough = max_rough.max(r);
        if bd > 0.5 {
            basin_cells += 1;
        }
        if fi > 0.35 {
            active_fault_cells += 1;
        }
        if vh > 0.35 {
            active_volc_cells += 1;
        }
    }

    assert!(max_rough > min_rough * 4.0, "roughness should distinguish plains/shelves from mountain chains");
    assert!(basin_cells >= 1, "depression resolution should record filled basin depths");
    assert!(active_fault_cells >= 20, "convergent/rift belts should exhibit elevated seismic risk");
    assert!(active_volc_cells >= 10, "volcanic arcs/ridges should exhibit volcanism hazard");
}

// @req ENG-GEO-010
#[test]
fn continental_and_marine_provinces_classify_all_land_and_sea_tectonic_regimes() {
    let cfg = default_planet_cfg(16);
    let mut land_provs = BTreeSet::new();
    let mut marine_provs = BTreeSet::new();

    for seed in [42_u64, 7_u64] {
        let planet = GeneratedPlanet::generate(seed, &cfg);
        for idx in 0..planet.cell_count() {
            let cid = CellId(idx);
            let prov = planet.cell_geology_province(cid);
            if planet.is_ocean_cell(cid) {
                assert!(
                    prov.is_marine(),
                    "ocean cell {idx} must carry marine geology province, got {prov:?}"
                );
                marine_provs.insert(prov);
            } else {
                assert!(
                    !prov.is_marine(),
                    "land cell {idx} must carry continental geology province, got {prov:?}"
                );
                land_provs.insert(prov);
            }
        }
    }

    assert!(
        land_provs.len() >= 6,
        "expected diverse continental provinces (craton, old/young orogen, arc, basin, platform, alluvial), got {land_provs:?}"
    );
    assert!(
        marine_provs.len() >= 4,
        "expected diverse submarine bathymetry provinces (shelf, slope, ridge, trench, abyss), got {marine_provs:?}"
    );
    assert!(marine_provs.contains(&GeologyProvinceKind::ContinentalShelf));
    assert!(marine_provs.contains(&GeologyProvinceKind::AbyssalPlain));
}

// @req ENG-GEO-012
#[test]
fn incised_channel_bed_and_water_surface_descend_monotonically_with_lakes_estuaries_and_navigability() {
    let cfg = default_planet_cfg(24);
    let mut exorheic_count = 0usize;
    let mut endorheic_count = 0usize;
    let mut estuary_count = 0usize;
    let mut deep_nav_count = 0usize;
    let mut ford_or_rapids = 0usize;

    for seed in [42_u64, 7_u64, 1337_u64] {
        let planet = GeneratedPlanet::generate(seed, &cfg);
        for idx in 0..planet.cell_count() {
            let cid = CellId(idx);
            let lk = planet.cell_lake_kind(cid);
            let sal = planet.cell_lake_salinity_per_mille(cid);
            match lk {
                LakeKind::Exorheic => {
                    exorheic_count += 1;
                    assert!(planet.cell_is_lake(cid));
                    assert!(sal < 5.0, "exorheic overflow lake should stay fresh, got {sal}");
                }
                LakeKind::Endorheic => {
                    endorheic_count += 1;
                    assert!(planet.cell_is_lake(cid));
                    assert!(sal > 10.0, "endorheic terminal lake should concentrate salts, got {sal}");
                }
                LakeKind::None => assert!(!planet.cell_is_lake(cid)),
            }
            if planet.cell_is_estuary(cid) {
                estuary_count += 1;
                let ds = planet.cell_river_channel_downstream(cid);
                assert!(
                    ds.is_some_and(|d| planet.is_ocean_cell(d)),
                    "estuary tag only valid on river mouth entering ocean"
                );
            }
            match planet.cell_river_navigability(cid) {
                RiverNavigabilityKind::DeepNavigable => deep_nav_count += 1,
                RiverNavigabilityKind::ShallowFord | RiverNavigabilityKind::Rapids => {
                    ford_or_rapids += 1;
                }
                RiverNavigabilityKind::None => {}
            }

            if let Some(ds) = planet.cell_river_channel_downstream(cid) {
                let bed_u = planet.cell_channel_bed_elev_m(cid);
                let surf_u = planet.cell_water_surface_elev_m(cid);
                let surf_v = if planet.is_ocean_cell(ds) {
                    0.0
                } else {
                    planet.cell_water_surface_elev_m(ds)
                };
                assert!(
                    surf_u + 1e-3 >= surf_v,
                    "seed {seed} river cell {idx} water surface {surf_u} < downstream {} ({surf_v})",
                    ds.get()
                );
                if !planet.is_ocean_cell(ds) {
                    let bed_v = planet.cell_channel_bed_elev_m(ds);
                    assert!(
                        bed_u + 1e-3 >= bed_v,
                        "seed {seed} channel bed {idx} ({bed_u}) rises downstream to {} ({bed_v})",
                        ds.get()
                    );
                }
            }
        }
    }

    assert!(estuary_count >= 10, "expected tidal estuaries at perennial ocean mouths");
    assert!(deep_nav_count >= 20, "expected deep-navigable trunk river reaches");
    assert!(ford_or_rapids >= 50, "expected shallow ford and rapids headwater reaches");
    assert!(
        exorheic_count + endorheic_count >= 1,
        "expected exorheic or endorheic lake basins"
    );
}

// @req ENG-GEO-013
#[test]
fn soil_drainage_clay_and_marine_upwelling_nutrients_persist_across_canonical_save_load() {
    let cfg = default_planet_cfg(10);
    let planet = GeneratedPlanet::generate(99, &cfg);
    let mut high_shelf_nutrient = 0usize;
    let mut max_upwelling = 0.0_f32;
    let mut max_clay = 0.0_f32;
    let mut min_drain = 1.0_f32;

    for idx in 0..planet.cell_count() {
        let cid = CellId(idx);
        let drain = planet.cell_soil_drainage_01(cid);
        let clay = planet.cell_soil_clay_01(cid);
        let upw = planet.cell_marine_upwelling_01(cid);
        let nut = planet.cell_shelf_nutrient_01(cid);
        assert!((0.0..=1.0).contains(&drain));
        assert!((0.0..=1.0).contains(&clay));
        assert!((0.0..=1.0).contains(&upw));
        assert!((0.0..=1.0).contains(&nut));
        if planet.is_ocean_cell(cid) {
            max_upwelling = max_upwelling.max(upw);
            if nut > 0.25 {
                high_shelf_nutrient += 1;
            }
        } else {
            max_clay = max_clay.max(clay);
            min_drain = min_drain.min(drain);
        }
    }

    assert!(high_shelf_nutrient >= 5, "expected nutrient-enriched coastal shelves");
    assert!(max_upwelling > 0.15, "expected offshore wind-driven upwelling zones");
    assert!(max_clay > 0.15 && min_drain < 0.85, "expected differentiated soil drainage and clay");

    let mut bytes = Vec::new();
    planet.write_canonical_state(&mut bytes);
    let mut restored = GeneratedPlanet::generate(99, &cfg);
    let mut slice = bytes.as_slice();
    restored.read_canonical_state(&mut slice).expect("planet read_canonical_state");
    assert!(slice.is_empty());
    assert_eq!(restored.blake3_digest(), planet.blake3_digest());
}
