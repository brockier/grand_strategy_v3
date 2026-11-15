#![deny(clippy::disallowed_types)]

//! Geodesic planet topology, tectonic relief, priority-flood hydrology,
//! rain-shadow climatology, strata-conditioned subsurface deposits,
//! and bounded procedural natural materials (`ENG-GEO-001` through `ENG-GEO-006`).

pub mod procedural;
pub mod topology;

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use rand_chacha::ChaCha8Rng;
use rand_core::{RngCore, SeedableRng};
use sim_spine::{
    codec::{
        read_f32_bits, read_f64_bits, read_str, read_u16_le, read_u32_le, read_u64_le, read_u8,
        write_f32_bits, write_f64_bits, write_str, write_u16_le, write_u32_le, write_u64_le,
        write_u8,
    },
    CanonicalSaveablePort, CellGeologyView, CellId, EdgeId, HiddenDepositView, NaturalBoundsView,
    PlanetGenConfigView, PlanetGeometryPort, PlateId, ProceduralDepositCategory,
    ProceduralMaterialId, RockStrataClass, SubsurfaceDepositTruthPort,
};

use crate::procedural::{
    ProceduralCropDef, ProceduralFaunaDef, ProceduralFluxDef, ProceduralFuelDef,
    ProceduralMaterialCatalog, ProceduralOreDef, ProceduralPathogenDef,
};
use crate::topology::{cross3, norm3, subdivision_to_frequency, GoldbergTopology};

/// Deterministic float ordering wrapper for priority-flood min-heap.
#[derive(Copy, Clone, Debug, PartialEq)]
struct OrdF32(f32);

impl Eq for OrdF32 {}

impl PartialOrd for OrdF32 {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrdF32 {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

#[inline]
fn next_unit_f32(rng: &mut ChaCha8Rng) -> f32 {
    let bits = (rng.next_u64() >> 40) as u32;
    (bits as f32) / ((1u32 << 24) as f32)
}

#[inline]
fn split_seed(world_seed: u64, salt: u64) -> ChaCha8Rng {
    let mut mixed = world_seed ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    ChaCha8Rng::seed_from_u64(mixed ^ (mixed >> 31))
}

#[inline]
fn dot3_f64(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn dot3_f32(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// Deterministic 3D value noise on integer lattice for FBM spherical terrain relief.
fn hash_lattice(ix: i32, iy: i32, iz: i32, seed: u64) -> f64 {
    let mut h = seed
        ^ (ix as i64 as u64).wrapping_mul(0x517C_C1B7_2722_0A95)
        ^ (iy as i64 as u64).wrapping_mul(0x85EB_CA6B_C2B2_AE3D)
        ^ (iz as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h = (h ^ (h >> 33)).wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    h = (h ^ (h >> 33)).wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    h ^= h >> 33;
    ((h >> 11) as f64) / ((1u64 << 53) as f64) * 2.0 - 1.0
}

#[inline]
fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

#[inline]
fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

fn value_noise_3d(x: f64, y: f64, z: f64, seed: u64) -> f64 {
    let ix = x.floor() as i32;
    let iy = y.floor() as i32;
    let iz = z.floor() as i32;
    let fx = smoothstep(x - f64::from(ix));
    let fy = smoothstep(y - f64::from(iy));
    let fz = smoothstep(z - f64::from(iz));

    let c000 = hash_lattice(ix, iy, iz, seed);
    let c100 = hash_lattice(ix + 1, iy, iz, seed);
    let c010 = hash_lattice(ix, iy + 1, iz, seed);
    let c110 = hash_lattice(ix + 1, iy + 1, iz, seed);
    let c001 = hash_lattice(ix, iy, iz + 1, seed);
    let c101 = hash_lattice(ix + 1, iy, iz + 1, seed);
    let c011 = hash_lattice(ix, iy + 1, iz + 1, seed);
    let c111 = hash_lattice(ix + 1, iy + 1, iz + 1, seed);

    let x00 = lerp(c000, c100, fx);
    let x10 = lerp(c010, c110, fx);
    let x01 = lerp(c001, c101, fx);
    let x11 = lerp(c011, c111, fx);
    lerp(lerp(x00, x10, fy), lerp(x01, x11, fy), fz)
}

fn fbm_sphere(p: [f32; 3], seed: u64, octaves: u8) -> f64 {
    let mut amp = 1.0;
    let mut freq = 2.2;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for oct in 0..octaves {
        let s = seed.wrapping_add(u64::from(oct).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        sum += amp
            * value_noise_3d(
                f64::from(p[0]) * freq,
                f64::from(p[1]) * freq,
                f64::from(p[2]) * freq,
                s,
            );
        norm += amp;
        amp *= 0.5;
        freq *= 2.05;
    }
    if norm > 0.0 {
        sum / norm
    } else {
        0.0
    }
}

/// Complete planetary body containing closed Goldberg topology, tectonic relief,
/// acyclic watershed drainage, rain-shadow climatology, strata-conditioned deposits,
/// and seeded procedural material catalog.
#[derive(Clone, Debug, PartialEq)]
pub struct GeneratedPlanet {
    pub world_seed: u64,
    pub config: PlanetGenConfigView,
    pub topology: GoldbergTopology,
    pub elevation_m: Vec<f32>,
    pub raw_tectonic_uplift_m: Vec<f32>,
    pub is_ocean: Vec<bool>,
    pub is_convergent_boundary: Vec<bool>,
    pub is_divergent_boundary: Vec<bool>,
    pub plate_id: Vec<PlateId>,
    pub strata: Vec<RockStrataClass>,
    pub biome_keys: Vec<String>,
    pub base_precip_mm_yr: Vec<f32>,
    pub mean_annual_temp_c: Vec<f32>,
    pub prevailing_wind_uv: Vec<[f32; 2]>,
    pub edge_slope_rad: Vec<f32>,
    pub river_downstream: Vec<Option<CellId>>,
    pub watershed_ids: Vec<sim_spine::WatershedId>,
    pub drainage_area_km2: Vec<f32>,
    pub topo_sorted_land_cells: Vec<CellId>,
    pub deposits: Vec<HiddenDepositView>,
    pub cell_deposit_ranges: Vec<(usize, usize)>,
    pub materials: ProceduralMaterialCatalog,
}

impl GeneratedPlanet {
    /// Generates a deterministic spherical planet from `world_seed` and data-driven `PlanetGenConfigView`.
    #[must_use]
    pub fn generate(world_seed: u64, cfg: &PlanetGenConfigView) -> Self {
        let frequency = subdivision_to_frequency(cfg.goldberg_subdivision.max(1));
        let topology = GoldbergTopology::generate(frequency, cfg.mean_radius_km.max(500.0));
        let n = topology.cell_unit_xyz.len();

        let mut mat_rng = split_seed(world_seed, 1);
        let materials = ProceduralMaterialCatalog::generate(&mut mat_rng, &cfg.natural_bounds);

        // 1. Seed tectonic plates using farthest-point geodesic sampling.
        let plate_count = usize::from(cfg.tectonic_plate_count.max(4)).min(n / 2).max(4);
        let mut plate_rng = split_seed(world_seed, 2);
        let mut plate_seeds = Vec::with_capacity(plate_count);
        let first_seed = (plate_rng.next_u64() as usize) % n;
        plate_seeds.push(first_seed);

        let mut min_dot_dist = vec![f32::MAX; n];
        for _ in 1..plate_count {
            let last_pos = topology.cell_unit_xyz[*plate_seeds.last().unwrap_or(&0)];
            let mut best_idx = 0usize;
            let mut best_dist = -1.0_f32;
            for (idx, &pos) in topology.cell_unit_xyz.iter().enumerate() {
                let ang = dot3_f32(pos, last_pos).clamp(-1.0, 1.0).acos();
                if ang < min_dot_dist[idx] {
                    min_dot_dist[idx] = ang;
                }
                if min_dot_dist[idx] > best_dist {
                    best_dist = min_dot_dist[idx];
                    best_idx = idx;
                }
            }
            plate_seeds.push(best_idx);
        }

        // Assign each cell to a plate with noise-warped geodesic Voronoi score for jagged fault lines.
        let mut plate_id = Vec::with_capacity(n);
        for &pos in &topology.cell_unit_xyz {
            let warp_x = fbm_sphere(pos, world_seed ^ 0x1111, 3) as f32 * 0.14;
            let warp_y =
                fbm_sphere([pos[1], pos[2], pos[0]], world_seed ^ 0x2222, 3) as f32 * 0.14;
            let warp_z =
                fbm_sphere([pos[2], pos[0], pos[1]], world_seed ^ 0x3333, 3) as f32 * 0.14;
            let mut best_p = 0usize;
            let mut best_score = -f32::MAX;
            for (p_idx, &seed_cell) in plate_seeds.iter().enumerate() {
                let sp = topology.cell_unit_xyz[seed_cell];
                let d = dot3_f32(pos, sp) + warp_x * sp[0] + warp_y * sp[1] + warp_z * sp[2];
                if d > best_score {
                    best_score = d;
                    best_p = p_idx;
                }
            }
            plate_id.push(PlateId(best_p as u16));
        }

        // Assign each plate an Euler spin vector and continental vs oceanic crustal buoyancy.
        let target_land = cfg.land_fraction_target.clamp(0.05, 0.95);
        let mut plate_omegas = Vec::with_capacity(plate_count);
        let mut plate_base_elev = Vec::with_capacity(plate_count);
        let continental_cutoff = (f32::from(plate_count as u16) * target_land)
            .round()
            .clamp(1.0, (plate_count - 1) as f32) as usize;
        for p_idx in 0..plate_count {
            let ax = f64::from(next_unit_f32(&mut plate_rng) * 2.0 - 1.0);
            let ay = f64::from(next_unit_f32(&mut plate_rng) * 2.0 - 1.0);
            let az = f64::from(next_unit_f32(&mut plate_rng) * 2.0 - 1.0);
            let axis = norm3([ax, ay, az]);
            let omega_rate = f64::from(0.5 + next_unit_f32(&mut plate_rng) * 1.2);
            plate_omegas.push([axis[0] * omega_rate, axis[1] * omega_rate, axis[2] * omega_rate]);
            let is_cont = p_idx < continental_cutoff;
            let crustal = if is_cont {
                550.0 + next_unit_f32(&mut plate_rng) * 550.0
            } else {
                -1850.0 - next_unit_f32(&mut plate_rng) * 900.0
            };
            plate_base_elev.push(crustal);
        }

        // 2. Compute plate boundary kinematics: convergence vs divergence vs transform.
        let mut is_convergent_boundary = vec![false; n];
        let mut is_divergent_boundary = vec![false; n];
        let mut raw_tectonic_uplift_m = vec![0.0_f32; n];
        let mut direct_orogeny_m = vec![0.0_f32; n];
        let max_uplift = cfg.max_orogenic_uplift_m.max(500.0);

        for idx in 0..n {
            let cid = CellId(idx as u32);
            let pa = plate_id[idx];
            let pos_a = [
                f64::from(topology.cell_unit_xyz[idx][0]),
                f64::from(topology.cell_unit_xyz[idx][1]),
                f64::from(topology.cell_unit_xyz[idx][2]),
            ];
            let om_a = plate_omegas[pa.get() as usize];
            let vel_a = cross3(om_a, pos_a);

            let mut max_conv = 0.0_f32;
            let mut min_div = 0.0_f32;
            for &nb in topology.neighbors(cid) {
                let nb_idx = nb.get() as usize;
                let pb = plate_id[nb_idx];
                if pb == pa {
                    continue;
                }
                let pos_b = [
                    f64::from(topology.cell_unit_xyz[nb_idx][0]),
                    f64::from(topology.cell_unit_xyz[nb_idx][1]),
                    f64::from(topology.cell_unit_xyz[nb_idx][2]),
                ];
                let n_ab = norm3([
                    pos_b[0] - pos_a[0],
                    pos_b[1] - pos_a[1],
                    pos_b[2] - pos_a[2],
                ]);
                let om_b = plate_omegas[pb.get() as usize];
                let vel_b = cross3(om_b, pos_b);
                let rel_v = [vel_a[0] - vel_b[0], vel_a[1] - vel_b[1], vel_a[2] - vel_b[2]];
                let conv = dot3_f64(rel_v, n_ab) as f32;
                if conv > max_conv {
                    max_conv = conv;
                }
                if conv < min_div {
                    min_div = conv;
                }
            }

            if max_conv > 0.25 {
                is_convergent_boundary[idx] = true;
                let norm_strength = (max_conv / 2.2).clamp(0.18, 1.0);
                direct_orogeny_m[idx] = max_uplift * norm_strength;
            } else if min_div < -0.25 {
                is_divergent_boundary[idx] = true;
                let norm_div = ((-min_div) / 2.2).clamp(0.15, 1.0);
                direct_orogeny_m[idx] = -0.22 * max_uplift * norm_div;
            }
        }

        // Propagate orogenic belts smoothly onto neighboring cells (2 diffusion passes).
        let mut diffused_uplift = direct_orogeny_m.clone();
        for pass in 0..2 {
            let decay = if pass == 0 { 0.42 } else { 0.28 };
            let prev = diffused_uplift.clone();
            for idx in 0..n {
                let cid = CellId(idx as u32);
                let nbrs = topology.neighbors(cid);
                let mut sum_nbr = 0.0_f32;
                for &nb in nbrs {
                    sum_nbr += prev[nb.get() as usize];
                }
                let mean_nbr = if nbrs.is_empty() {
                    0.0
                } else {
                    sum_nbr / (nbrs.len() as f32)
                };
                diffused_uplift[idx] = prev[idx] * (1.0 - decay) + mean_nbr * decay;
            }
        }
        // Preserve crisp ridge boost right on convergent cells.
        for idx in 0..n {
            if is_convergent_boundary[idx] {
                diffused_uplift[idx] = diffused_uplift[idx].max(direct_orogeny_m[idx] * 0.72);
            }
            raw_tectonic_uplift_m[idx] = diffused_uplift[idx];
        }

        // Combine crustal buoyancy, tectonic uplift, and multi-octave spherical FBM relief.
        let mut raw_elev = Vec::with_capacity(n);
        for (idx, &pos) in topology.cell_unit_xyz.iter().enumerate() {
            let base = plate_base_elev[plate_id[idx].get() as usize];
            let fbm_macro = fbm_sphere(pos, world_seed ^ 0x4444_5555, 4) as f32 * 950.0;
            let fbm_micro = fbm_sphere(
                [pos[0] * 3.1, pos[1] * 3.1, pos[2] * 3.1],
                world_seed ^ 0x6666_7777,
                3,
            ) as f32
                * 260.0;
            raw_elev.push(base + raw_tectonic_uplift_m[idx] + fbm_macro + fbm_micro);
        }

        // Smooth interior noise slightly (2 thermal passes) while preserving convergent uplift ridges.
        for _ in 0..2 {
            let prev = raw_elev.clone();
            for idx in 0..n {
                let cid = CellId(idx as u32);
                let nbrs = topology.neighbors(cid);
                let mean_nbr: f32 =
                    nbrs.iter().map(|nb| prev[nb.get() as usize]).sum::<f32>()
                        / (nbrs.len().max(1) as f32);
                let alpha = if is_convergent_boundary[idx] {
                    0.10
                } else {
                    0.28
                };
                raw_elev[idx] = prev[idx] * (1.0 - alpha) + mean_nbr * alpha;
            }
        }

        // 3. Area-weighted quantile sea-level calibration to hit `cfg.land_fraction_target`.
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| raw_elev[a].total_cmp(&raw_elev[b]));
        let total_area: f64 = topology.cell_area_km2.iter().map(|&a| f64::from(a)).sum();
        let target_ocean_area = total_area * (1.0 - f64::from(target_land));
        let mut accum_ocean = 0.0_f64;
        let mut sea_threshold = raw_elev[order[0]] - 1.0;
        for &cell_idx in &order {
            if accum_ocean >= target_ocean_area {
                break;
            }
            sea_threshold = raw_elev[cell_idx];
            accum_ocean += f64::from(topology.cell_area_km2[cell_idx]);
        }

        let sea_shift = cfg.base_sea_level_m - sea_threshold;
        for e in &mut raw_elev {
            *e += sea_shift;
        }

        let mut is_ocean = Vec::with_capacity(n);
        let mut ocean_count = 0usize;
        for &e in &raw_elev {
            let ocean = e <= cfg.base_sea_level_m;
            if ocean {
                ocean_count += 1;
            }
            is_ocean.push(ocean);
        }

        // 4. Barnes (2014) Priority-Flood depression resolution -> strict cycle-free downhill river DAG (`ENG-GEO-004`).
        let mut elevation_m = raw_elev;
        let mut river_downstream: Vec<Option<CellId>> = vec![None; n];
        let mut visited = vec![false; n];
        let mut heap: BinaryHeap<Reverse<(OrdF32, u32)>> = BinaryHeap::new();

        if ocean_count > 0 {
            for idx in 0..n {
                if is_ocean[idx] {
                    visited[idx] = true;
                    heap.push(Reverse((OrdF32(elevation_m[idx]), idx as u32)));
                }
            }
        } else {
            // Closed-basin world edge case: global lowest cell serves as single endorheic sink.
            let min_idx = order[0];
            visited[min_idx] = true;
            heap.push(Reverse((OrdF32(elevation_m[min_idx]), min_idx as u32)));
        }

        while let Some(Reverse((OrdF32(cur_h), u_u32))) = heap.pop() {
            let u = CellId(u_u32);
            for &nb in topology.neighbors(u) {
                let nb_idx = nb.get() as usize;
                if visited[nb_idx] {
                    continue;
                }
                visited[nb_idx] = true;
                if !is_ocean[nb_idx] {
                    river_downstream[nb_idx] = Some(u);
                    if elevation_m[nb_idx] < cur_h + 0.05 {
                        elevation_m[nb_idx] = cur_h + 0.05;
                    }
                }
                heap.push(Reverse((OrdF32(elevation_m[nb_idx]), nb.get())));
            }
        }

        // Build deterministic Kahn topological sort from headwaters (in_deg == 0) down to ocean outlets.
        let mut in_degree = vec![0u32; n];
        for idx in 0..n {
            if !is_ocean[idx]
                && let Some(ds) = river_downstream[idx]
                && !is_ocean[ds.get() as usize]
            {
                in_degree[ds.get() as usize] += 1;
            }
        }
        let mut ready = BTreeSet::new();
        for idx in 0..n {
            if !is_ocean[idx] && in_degree[idx] == 0 {
                ready.insert(idx as u32);
            }
        }
        let mut topo_sorted_land_cells = Vec::with_capacity(n - ocean_count);
        while let Some(u) = ready.pop_first() {
            topo_sorted_land_cells.push(CellId(u));
            if let Some(ds) = river_downstream[u as usize] {
                let ds_idx = ds.get() as usize;
                if !is_ocean[ds_idx] {
                    in_degree[ds_idx] -= 1;
                    if in_degree[ds_idx] == 0 {
                        ready.insert(ds.get());
                    }
                }
            }
        }

        // Accumulate basin drainage areas downstream cleanly along topological DAG.
        let mut drainage_area_km2 = topology.cell_area_km2.clone();
        for &cid in &topo_sorted_land_cells {
            let idx = cid.get() as usize;
            if let Some(ds) = river_downstream[idx] {
                drainage_area_km2[ds.get() as usize] += drainage_area_km2[idx];
            }
        }

        // Edge slope angle along each undirected edge (non-negative slope magnitude).
        let mut edge_slope_rad = Vec::with_capacity(topology.edge_endpoints.len());
        for (e_idx, &(a, b)) in topology.edge_endpoints.iter().enumerate() {
            let dh = (elevation_m[a.get() as usize] - elevation_m[b.get() as usize]).abs();
            let run_m = (topology.edge_length_km[e_idx] * 1000.0).max(1.0);
            edge_slope_rad.push((dh / run_m).atan());
        }

        // 5. Latitudinal zonal wind belts + 24-sweep orographic rain-shadow moisture transport (`ENG-GEO-004`).
        let mut mean_annual_temp_c = Vec::with_capacity(n);
        let mut prevailing_wind_uv = Vec::with_capacity(n);
        for idx in 0..n {
            let [lat, _lon] = topology.cell_lat_lon_rad[idx];
            let abs_lat = f64::from(lat).abs();
            let solar_scale = f64::from(cfg.solar_constant_w_m2 / 1361.0).clamp(0.4, 2.2);
            let sea_temp = (31.0 * abs_lat.cos() - 3.5) * solar_scale;
            let lapse_c = if is_ocean[idx] {
                0.0
            } else {
                (elevation_m[idx] - cfg.base_sea_level_m).max(0.0) * 0.0065
            };
            mean_annual_temp_c.push(sea_temp as f32 - lapse_c);

            // Hadley/Ferrel/Polar belts: Trades (<23 deg) East->West (<0), Westerlies (23..67) West->East (>0), Polar (<0).
            let u_zonal = -(4.0 * abs_lat).cos() * 9.5;
            let v_merid = (2.0 * abs_lat).sin() * f64::from(lat).signum() * 3.0;
            prevailing_wind_uv.push([u_zonal as f32, v_merid as f32]);
        }

        // Precompute upwind weights per cell `(nbr_idx, weight)` based on wind alignment.
        let mut upwind_weights: Vec<Vec<(usize, f32)>> = vec![Vec::new(); n];
        for idx in 0..n {
            let cid = CellId(idx as u32);
            let pos = topology.cell_unit_xyz[idx];
            let (east, north) = topology::tangent_basis([
                f64::from(pos[0]),
                f64::from(pos[1]),
                f64::from(pos[2]),
            ]);
            let [wu, wv] = prevailing_wind_uv[idx];
            let wind_3d = norm3([
                east[0] * f64::from(wu) + north[0] * f64::from(wv),
                east[1] * f64::from(wu) + north[1] * f64::from(wv),
                east[2] * f64::from(wu) + north[2] * f64::from(wv),
            ]);
            let mut weights = Vec::new();
            let mut w_sum = 0.0_f32;
            for &nb in topology.neighbors(cid) {
                let nb_idx = nb.get() as usize;
                let nb_pos = topology.cell_unit_xyz[nb_idx];
                // Vector from neighbor -> cell; positive dot with wind_3d means neighbor sits upwind of cell.
                let dir = norm3([
                    f64::from(pos[0] - nb_pos[0]),
                    f64::from(pos[1] - nb_pos[1]),
                    f64::from(pos[2] - nb_pos[2]),
                ]);
                let align = dot3_f64(dir, wind_3d) as f32;
                if align > 0.05 {
                    weights.push((nb_idx, align));
                    w_sum += align;
                }
            }
            if w_sum > 0.0 {
                for (_, w) in &mut weights {
                    *w /= w_sum;
                }
            }
            upwind_weights[idx] = weights;
        }

        let mut moisture = Vec::with_capacity(n);
        for &temp in &mean_annual_temp_c {
            let t_k = (f64::from(temp) + 273.15).max(210.0);
            let sat = (10.0 + (t_k - 250.0) * 0.45).clamp(2.0, 55.0) as f32;
            moisture.push(sat);
        }
        let mut precip_accum = vec![0.0_f32; n];
        let mut next_moisture = moisture.clone();
        for _sweep in 0..24 {
            for idx in 0..n {
                let weights = &upwind_weights[idx];
                let (q_in, upwind_elev) = if weights.is_empty() {
                    (moisture[idx], elevation_m[idx])
                } else {
                    let mut q = 0.0_f32;
                    let mut ue = 0.0_f32;
                    for &(nb_idx, w) in weights {
                        q += moisture[nb_idx] * w;
                        ue += elevation_m[nb_idx] * w;
                    }
                    (q, ue)
                };

                if is_ocean[idx] {
                    let t_c = mean_annual_temp_c[idx];
                    let evap_cap = (14.0 + (t_c + 10.0).max(0.0) * 0.72).clamp(4.0, 52.0);
                    let rain = q_in * 0.14;
                    precip_accum[idx] += rain * 38.0;
                    next_moisture[idx] = (q_in * 0.40 + evap_cap * 0.60).clamp(2.0, 55.0);
                } else {
                    let elev_delta_m = (elevation_m[idx] - upwind_elev).max(0.0);
                    let orographic_lift = (elev_delta_m / 650.0).clamp(0.0, 2.2);
                    let is_leeward = elevation_m[idx] + 60.0 < upwind_elev;
                    let base_frac = if is_leeward { 0.055 } else { 0.14 };
                    let condense_frac = (base_frac + 0.34 * orographic_lift).clamp(0.04, 0.86);
                    let rain = q_in * condense_frac;
                    precip_accum[idx] += rain * 46.0;
                    let residual = (q_in - rain * 0.85).max(0.4);
                    // Small vegetative recycling in warm moist lowlands.
                    let recycle = if !is_leeward && mean_annual_temp_c[idx] > 4.0 {
                        0.45
                    } else {
                        0.10
                    };
                    next_moisture[idx] = (residual + recycle).clamp(0.4, 48.0);
                }
            }
            moisture.copy_from_slice(&next_moisture);
        }
        let base_precip_mm_yr: Vec<f32> = precip_accum
            .into_iter()
            .map(|p| (p / 24.0 * 14.0).clamp(35.0, 4600.0))
            .collect();

        // 6. Rock strata classes, biome strings, and strata-conditioned subsurface deposits (`ENG-GEO-003`).
        let mut strata = Vec::with_capacity(n);
        let mut biome_keys = Vec::with_capacity(n);
        for idx in 0..n {
            let elev_above = elevation_m[idx] - cfg.base_sea_level_m;
            let [lat, _lon] = topology.cell_lat_lon_rad[idx];
            let abs_lat_deg = lat.abs().to_degrees();
            let temp = mean_annual_temp_c[idx];
            let precip = base_precip_mm_yr[idx];

            let st = if is_ocean[idx] {
                RockStrataClass::OceanicBasalt
            } else if is_convergent_boundary[idx] && elev_above > 750.0 {
                RockStrataClass::IgneousOrogenic
            } else if is_convergent_boundary[idx] || is_divergent_boundary[idx] {
                RockStrataClass::VolcanicArc
            } else if elev_above > 520.0 {
                RockStrataClass::MetamorphicShield
            } else if drainage_area_km2[idx] > topology.cell_area_km2[idx] * 3.5 {
                RockStrataClass::AlluvialPlain
            } else {
                RockStrataClass::SedimentaryBasin
            };
            strata.push(st);

            let biome = if is_ocean[idx] {
                if elev_above > -250.0 {
                    "shallow_sea"
                } else {
                    "deep_ocean"
                }
            } else if abs_lat_deg > 68.0 || elev_above > 3000.0 {
                "alpine_glacier"
            } else if temp < 1.5 {
                "tundra"
            } else if precip < 240.0 {
                "arid_desert"
            } else if temp < 9.5 && precip >= 320.0 {
                "boreal_taiga"
            } else if temp > 21.0 && precip >= 1350.0 {
                "tropical_rainforest"
            } else if temp > 17.0 && precip < 880.0 {
                "savanna_steppe"
            } else if precip >= 620.0 {
                "temperate_forest"
            } else {
                "temperate_grassland"
            };
            biome_keys.push(biome.to_string());
        }

        let (deposits, cell_deposit_ranges) =
            seed_subsurface_deposits(world_seed, &strata, &materials, &topology.cell_area_km2);
        let watershed_ids = compute_watershed_ids(&river_downstream, &is_ocean);

        Self {
            world_seed,
            config: cfg.clone(),
            topology,
            elevation_m,
            raw_tectonic_uplift_m,
            is_ocean,
            is_convergent_boundary,
            is_divergent_boundary,
            plate_id,
            strata,
            biome_keys,
            base_precip_mm_yr,
            mean_annual_temp_c,
            prevailing_wind_uv,
            edge_slope_rad,
            river_downstream,
            watershed_ids,
            drainage_area_km2,
            topo_sorted_land_cells,
            deposits,
            cell_deposit_ranges,
            materials,
        }
    }

    /// Computes canonical Blake3 digest of geometry, elevations, drainage, deposits, and catalog (`ENG-GEO-006`).
    #[must_use]
    pub fn blake3_digest(&self) -> [u8; 32] {
        let mut buf = Vec::with_capacity(self.elevation_m.len() * 24 + 4096);
        self.write_canonical_state(&mut buf);
        *blake3::hash(&buf).as_bytes()
    }

    /// Returns geological strata class for `cell`.
    #[must_use]
    pub fn strata_class(&self, cell: CellId) -> RockStrataClass {
        self.strata
            .get(cell.get() as usize)
            .copied()
            .unwrap_or(RockStrataClass::OceanicBasalt)
    }

    /// Returns whether `cell` is below planetary sea level.
    #[must_use]
    pub fn is_ocean_cell(&self, cell: CellId) -> bool {
        self.is_ocean.get(cell.get() as usize).copied().unwrap_or(true)
    }

    /// Returns mean annual climatic baseline temperature in Celsius for `cell`.
    #[must_use]
    pub fn cell_mean_temp_c(&self, cell: CellId) -> f32 {
        self.mean_annual_temp_c
            .get(cell.get() as usize)
            .copied()
            .unwrap_or(10.0)
    }

    /// Returns planetary prevailing `[u_mps, v_mps]` surface wind belt vector for `cell`.
    #[must_use]
    pub fn cell_prevailing_wind_uv(&self, cell: CellId) -> [f32; 2] {
        self.prevailing_wind_uv
            .get(cell.get() as usize)
            .copied()
            .unwrap_or([0.0, 0.0])
    }
}

fn seed_subsurface_deposits(
    world_seed: u64,
    strata: &[RockStrataClass],
    materials: &ProceduralMaterialCatalog,
    cell_area_km2: &[f32],
) -> (Vec<HiddenDepositView>, Vec<(usize, usize)>) {
    let mut rng = split_seed(world_seed, 3);
    let n = strata.len();
    let mut deposits = Vec::new();
    let mut ranges = Vec::with_capacity(n);

    let solid_fuels: Vec<&ProceduralFuelDef> =
        materials.fuels.iter().filter(|f| !f.is_liquid).collect();
    let liquid_fuels: Vec<&ProceduralFuelDef> =
        materials.fuels.iter().filter(|f| f.is_liquid).collect();

    for (idx, &st) in strata.iter().enumerate() {
        let start = deposits.len();
        let area_factor = (f64::from(cell_area_km2[idx]) / 50000.0).clamp(0.3, 4.0);
        let cid = CellId(idx as u32);
        let roll = next_unit_f32(&mut rng);

        match st {
            RockStrataClass::IgneousOrogenic
            | RockStrataClass::VolcanicArc
            | RockStrataClass::MetamorphicShield => {
                // Orogenic / shield / volcanic belts strongly host metallic ore seams.
                if roll < 0.62 && !materials.ores.is_empty() {
                    let ore = &materials.ores[(rng.next_u64() as usize) % materials.ores.len()];
                    deposits.push(HiddenDepositView {
                        cell: cid,
                        material_id: ore.id,
                        category: ProceduralDepositCategory::MetallicOre,
                        total_mass_tonnes: 180_000.0 * area_factor
                            * f64::from(0.5 + next_unit_f32(&mut rng)),
                        average_purity_01: (ore.base_grade_01 * (0.85 + 0.3 * next_unit_f32(&mut rng)))
                            .clamp(0.08, 0.92),
                        overburden_depth_m: 15.0 + next_unit_f32(&mut rng) * 180.0,
                        aquifer_flood_risk_01: 0.12 + next_unit_f32(&mut rng) * 0.45,
                    });
                }
                if next_unit_f32(&mut rng) < 0.24 && !materials.fluxes.is_empty() {
                    let flux =
                        &materials.fluxes[(rng.next_u64() as usize) % materials.fluxes.len()];
                    deposits.push(HiddenDepositView {
                        cell: cid,
                        material_id: flux.id,
                        category: ProceduralDepositCategory::SmeltingFlux,
                        total_mass_tonnes: 220_000.0 * area_factor
                            * f64::from(0.5 + next_unit_f32(&mut rng)),
                        average_purity_01: 0.55 + next_unit_f32(&mut rng) * 0.38,
                        overburden_depth_m: 10.0 + next_unit_f32(&mut rng) * 90.0,
                        aquifer_flood_risk_01: 0.10 + next_unit_f32(&mut rng) * 0.30,
                    });
                }
            }
            RockStrataClass::SedimentaryBasin => {
                // Sedimentary basins host coal/oil beds, carbonate/silicate fluxes, and trace bog iron.
                if roll < 0.38 && !solid_fuels.is_empty() {
                    let fuel = solid_fuels[(rng.next_u64() as usize) % solid_fuels.len()];
                    deposits.push(HiddenDepositView {
                        cell: cid,
                        material_id: fuel.id,
                        category: ProceduralDepositCategory::SolidFuel,
                        total_mass_tonnes: 420_000.0 * area_factor
                            * f64::from(0.6 + next_unit_f32(&mut rng)),
                        average_purity_01: 0.62 + next_unit_f32(&mut rng) * 0.32,
                        overburden_depth_m: 25.0 + next_unit_f32(&mut rng) * 240.0,
                        aquifer_flood_risk_01: 0.28 + next_unit_f32(&mut rng) * 0.55,
                    });
                } else if roll < 0.62 && !liquid_fuels.is_empty() {
                    let fuel = liquid_fuels[(rng.next_u64() as usize) % liquid_fuels.len()];
                    deposits.push(HiddenDepositView {
                        cell: cid,
                        material_id: fuel.id,
                        category: ProceduralDepositCategory::LiquidFuel,
                        total_mass_tonnes: 260_000.0 * area_factor
                            * f64::from(0.5 + next_unit_f32(&mut rng)),
                        average_purity_01: 0.68 + next_unit_f32(&mut rng) * 0.28,
                        overburden_depth_m: 120.0 + next_unit_f32(&mut rng) * 900.0,
                        aquifer_flood_risk_01: 0.35 + next_unit_f32(&mut rng) * 0.50,
                    });
                }
                if next_unit_f32(&mut rng) < 0.30 && !materials.fluxes.is_empty() {
                    let flux =
                        &materials.fluxes[(rng.next_u64() as usize) % materials.fluxes.len()];
                    deposits.push(HiddenDepositView {
                        cell: cid,
                        material_id: flux.id,
                        category: ProceduralDepositCategory::SmeltingFlux,
                        total_mass_tonnes: 340_000.0 * area_factor,
                        average_purity_01: 0.60 + next_unit_f32(&mut rng) * 0.35,
                        overburden_depth_m: 8.0 + next_unit_f32(&mut rng) * 75.0,
                        aquifer_flood_risk_01: 0.20 + next_unit_f32(&mut rng) * 0.40,
                    });
                }
                if next_unit_f32(&mut rng) < 0.08 && !materials.ores.is_empty() {
                    let ore = &materials.ores[(rng.next_u64() as usize) % materials.ores.len()];
                    deposits.push(HiddenDepositView {
                        cell: cid,
                        material_id: ore.id,
                        category: ProceduralDepositCategory::MetallicOre,
                        total_mass_tonnes: 65_000.0 * area_factor,
                        average_purity_01: (ore.base_grade_01 * 0.72).clamp(0.06, 0.70),
                        overburden_depth_m: 5.0 + next_unit_f32(&mut rng) * 60.0,
                        aquifer_flood_risk_01: 0.45 + next_unit_f32(&mut rng) * 0.45,
                    });
                }
            }
            RockStrataClass::AlluvialPlain => {
                if roll < 0.22 && !solid_fuels.is_empty() {
                    let fuel = solid_fuels[(rng.next_u64() as usize) % solid_fuels.len()];
                    deposits.push(HiddenDepositView {
                        cell: cid,
                        material_id: fuel.id,
                        category: ProceduralDepositCategory::SolidFuel,
                        total_mass_tonnes: 140_000.0 * area_factor,
                        average_purity_01: 0.50 + next_unit_f32(&mut rng) * 0.30,
                        overburden_depth_m: 6.0 + next_unit_f32(&mut rng) * 45.0,
                        aquifer_flood_risk_01: 0.55 + next_unit_f32(&mut rng) * 0.40,
                    });
                }
                if next_unit_f32(&mut rng) < 0.14 && !materials.ores.is_empty() {
                    let ore = &materials.ores[(rng.next_u64() as usize) % materials.ores.len()];
                    deposits.push(HiddenDepositView {
                        cell: cid,
                        material_id: ore.id,
                        category: ProceduralDepositCategory::MetallicOre,
                        total_mass_tonnes: 55_000.0 * area_factor,
                        average_purity_01: (ore.base_grade_01 * 0.65).clamp(0.05, 0.68),
                        overburden_depth_m: 4.0 + next_unit_f32(&mut rng) * 28.0,
                        aquifer_flood_risk_01: 0.60 + next_unit_f32(&mut rng) * 0.35,
                    });
                }
            }
            RockStrataClass::OceanicBasalt => {
                if roll < 0.08 && !liquid_fuels.is_empty() {
                    let fuel = liquid_fuels[(rng.next_u64() as usize) % liquid_fuels.len()];
                    deposits.push(HiddenDepositView {
                        cell: cid,
                        material_id: fuel.id,
                        category: ProceduralDepositCategory::LiquidFuel,
                        total_mass_tonnes: 180_000.0 * area_factor,
                        average_purity_01: 0.62 + next_unit_f32(&mut rng) * 0.30,
                        overburden_depth_m: 350.0 + next_unit_f32(&mut rng) * 900.0,
                        aquifer_flood_risk_01: 0.90,
                    });
                }
            }
        }
        ranges.push((start, deposits.len()));
    }

    (deposits, ranges)
}

impl PlanetGeometryPort for GeneratedPlanet {
    fn cell_count(&self) -> u32 {
        self.topology.cell_unit_xyz.len() as u32
    }

    fn cell_neighbors(&self, cell: CellId) -> &[CellId] {
        self.topology.neighbors(cell)
    }

    fn cell_edge(&self, from: CellId, to: CellId) -> Option<EdgeId> {
        self.topology.find_edge(from, to)
    }

    fn cell_elevation_m(&self, cell: CellId) -> f32 {
        self.elevation_m
            .get(cell.get() as usize)
            .copied()
            .unwrap_or(self.config.base_sea_level_m)
    }

    fn cell_slope_rad(&self, edge: EdgeId) -> f32 {
        self.edge_slope_rad
            .get(edge.get() as usize)
            .copied()
            .unwrap_or(0.0)
    }

    fn cell_biome_and_strata(&self, cell: CellId) -> CellGeologyView {
        let idx = cell.get() as usize;
        if idx >= self.elevation_m.len() {
            return CellGeologyView {
                cell_id: cell,
                elevation_m: self.config.base_sea_level_m,
                strata_label: RockStrataClass::OceanicBasalt.as_str().to_string(),
                biome_key: "deep_ocean".to_string(),
                is_ocean: true,
            };
        }
        CellGeologyView {
            cell_id: cell,
            elevation_m: self.elevation_m[idx],
            strata_label: self.strata[idx].as_str().to_string(),
            biome_key: self.biome_keys[idx].clone(),
            is_ocean: self.is_ocean[idx],
        }
    }

    fn watershed_downstream(&self, cell: CellId) -> Option<CellId> {
        self.river_downstream
            .get(cell.get() as usize)
            .copied()
            .flatten()
    }

    fn great_circle_km(&self, a: CellId, b: CellId) -> f32 {
        self.topology.great_circle_km(a, b)
    }

    fn cell_unit_xyz(&self, cell: CellId) -> [f32; 3] {
        self.topology
            .cell_unit_xyz
            .get(cell.get() as usize)
            .copied()
            .unwrap_or([0.0, 1.0, 0.0])
    }

    fn cell_lat_lon_rad(&self, cell: CellId) -> [f32; 2] {
        self.topology
            .cell_lat_lon_rad
            .get(cell.get() as usize)
            .copied()
            .unwrap_or([0.0, 0.0])
    }

    fn cell_area_km2(&self, cell: CellId) -> f32 {
        self.topology
            .cell_area_km2
            .get(cell.get() as usize)
            .copied()
            .unwrap_or(0.0)
    }

    fn cell_drainage_area_km2(&self, cell: CellId) -> f32 {
        self.drainage_area_km2
            .get(cell.get() as usize)
            .copied()
            .unwrap_or(0.0)
    }

    fn cell_base_precip_mm_yr(&self, cell: CellId) -> f32 {
        self.base_precip_mm_yr
            .get(cell.get() as usize)
            .copied()
            .unwrap_or(0.0)
    }

    fn edge_length_km(&self, edge: EdgeId) -> f32 {
        self.topology
            .edge_length_km
            .get(edge.get() as usize)
            .copied()
            .unwrap_or(0.0)
    }

    fn topo_sorted_land_cells(&self) -> &[CellId] {
        &self.topo_sorted_land_cells
    }

    fn nearest_sample_cell(&self, unit_xyz: [f32; 3]) -> CellId {
        self.topology.nearest_sample_cell(unit_xyz)
    }

    fn sample_elevation_at_unit_xyz(&self, unit_xyz: [f32; 3]) -> f32 {
        self.topology
            .sample_field_at_unit_xyz(unit_xyz, &self.elevation_m)
            .unwrap_or(self.config.base_sea_level_m)
    }

    fn icosphere_triangles(&self) -> &[[u32; 3]] {
        &self.topology.icosphere_triangles
    }

    fn watershed_id(&self, cell: CellId) -> sim_spine::WatershedId {
        self.watershed_ids
            .get(cell.get() as usize)
            .copied()
            .unwrap_or(sim_spine::WatershedId(0))
    }

    fn region_label(&self, cell: CellId) -> String {
        let wid = self.watershed_id(cell);
        if wid.0 == 0 {
            "Pelagic Waters".to_string()
        } else {
            format!("Watershed #{}", wid.0)
        }
    }

    fn radius_km(&self) -> f32 {
        self.topology.radius_km
    }
}

impl SubsurfaceDepositTruthPort for GeneratedPlanet {
    fn deposits_in_cell(&self, cell: CellId) -> &[HiddenDepositView] {
        let idx = cell.get() as usize;
        if let Some(&(s, e)) = self.cell_deposit_ranges.get(idx) {
            &self.deposits[s..e]
        } else {
            &[]
        }
    }

    fn all_deposits(&self) -> &[HiddenDepositView] {
        &self.deposits
    }
}

impl CanonicalSaveablePort for GeneratedPlanet {
    fn cartridge_tag(&self) -> &'static str {
        "planet"
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        write_u64_le(out, self.world_seed);
        write_planet_gen_config(out, &self.config);
        write_u32_le(out, self.topology.frequency);
        write_f32_bits(out, self.topology.radius_km);
        write_u32_le(out, self.elevation_m.len() as u32);
        for idx in 0..self.elevation_m.len() {
            write_f32_bits(out, self.elevation_m[idx]);
            write_f32_bits(out, self.raw_tectonic_uplift_m[idx]);
            write_u8(out, u8::from(self.is_ocean[idx]));
            write_u8(out, u8::from(self.is_convergent_boundary[idx]));
            write_u8(out, u8::from(self.is_divergent_boundary[idx]));
            write_u16_le(out, self.plate_id[idx].get());
            write_u8(out, self.strata[idx] as u8);
            write_str(out, &self.biome_keys[idx]);
            write_f32_bits(out, self.base_precip_mm_yr[idx]);
            write_f32_bits(out, self.mean_annual_temp_c[idx]);
            write_f32_bits(out, self.prevailing_wind_uv[idx][0]);
            write_f32_bits(out, self.prevailing_wind_uv[idx][1]);
            match self.river_downstream[idx] {
                None => write_u32_le(out, u32::MAX),
                Some(ds) => write_u32_le(out, ds.get()),
            }
            write_f32_bits(out, self.drainage_area_km2[idx]);
        }
        write_u32_le(out, self.edge_slope_rad.len() as u32);
        for &s in &self.edge_slope_rad {
            write_f32_bits(out, s);
        }
        write_u32_le(out, self.topo_sorted_land_cells.len() as u32);
        for &c in &self.topo_sorted_land_cells {
            write_u32_le(out, c.get());
        }
        write_u32_le(out, self.deposits.len() as u32);
        for dep in &self.deposits {
            write_u32_le(out, dep.cell.get());
            write_u16_le(out, dep.material_id.0);
            write_u8(out, dep.category as u8);
            write_f64_bits(out, dep.total_mass_tonnes);
            write_f32_bits(out, dep.average_purity_01);
            write_f32_bits(out, dep.overburden_depth_m);
            write_f32_bits(out, dep.aquifer_flood_risk_01);
        }
        write_material_catalog(out, &self.materials);
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        let world_seed = read_u64_le(input)?;
        let cfg = read_planet_gen_config(input)?;
        let freq = read_u32_le(input)?;
        let radius_km = read_f32_bits(input)?;
        let n = read_u32_le(input)? as usize;

        let topology = GoldbergTopology::generate(freq, radius_km);
        if topology.cell_unit_xyz.len() != n {
            return Err("planet cell count does not match Goldberg topology".to_string());
        }

        let mut elevation_m = Vec::with_capacity(n);
        let mut raw_tectonic_uplift_m = Vec::with_capacity(n);
        let mut is_ocean = Vec::with_capacity(n);
        let mut is_convergent_boundary = Vec::with_capacity(n);
        let mut is_divergent_boundary = Vec::with_capacity(n);
        let mut plate_id = Vec::with_capacity(n);
        let mut strata = Vec::with_capacity(n);
        let mut biome_keys = Vec::with_capacity(n);
        let mut base_precip_mm_yr = Vec::with_capacity(n);
        let mut mean_annual_temp_c = Vec::with_capacity(n);
        let mut prevailing_wind_uv = Vec::with_capacity(n);
        let mut river_downstream = Vec::with_capacity(n);
        let mut drainage_area_km2 = Vec::with_capacity(n);

        for _ in 0..n {
            elevation_m.push(read_f32_bits(input)?);
            raw_tectonic_uplift_m.push(read_f32_bits(input)?);
            is_ocean.push(read_u8(input)? != 0);
            is_convergent_boundary.push(read_u8(input)? != 0);
            is_divergent_boundary.push(read_u8(input)? != 0);
            plate_id.push(PlateId(read_u16_le(input)?));
            strata.push(match read_u8(input)? {
                0 => RockStrataClass::OceanicBasalt,
                1 => RockStrataClass::IgneousOrogenic,
                2 => RockStrataClass::VolcanicArc,
                3 => RockStrataClass::MetamorphicShield,
                4 => RockStrataClass::SedimentaryBasin,
                5 => RockStrataClass::AlluvialPlain,
                other => return Err(format!("invalid strata discriminant {other}")),
            });
            biome_keys.push(read_str(input)?);
            base_precip_mm_yr.push(read_f32_bits(input)?);
            mean_annual_temp_c.push(read_f32_bits(input)?);
            prevailing_wind_uv.push([read_f32_bits(input)?, read_f32_bits(input)?]);
            let ds_raw = read_u32_le(input)?;
            river_downstream.push(if ds_raw == u32::MAX {
                None
            } else {
                Some(CellId(ds_raw))
            });
            drainage_area_km2.push(read_f32_bits(input)?);
        }

        let edge_count = read_u32_le(input)? as usize;
        let mut edge_slope_rad = Vec::with_capacity(edge_count);
        for _ in 0..edge_count {
            edge_slope_rad.push(read_f32_bits(input)?);
        }

        let topo_count = read_u32_le(input)? as usize;
        let mut topo_sorted_land_cells = Vec::with_capacity(topo_count);
        for _ in 0..topo_count {
            topo_sorted_land_cells.push(CellId(read_u32_le(input)?));
        }

        let dep_count = read_u32_le(input)? as usize;
        let mut deposits = Vec::with_capacity(dep_count);
        let mut per_cell_bins: BTreeMap<u32, Vec<HiddenDepositView>> = BTreeMap::new();
        for _ in 0..dep_count {
            let cell = CellId(read_u32_le(input)?);
            let material_id = ProceduralMaterialId(read_u16_le(input)?);
            let category = match read_u8(input)? {
                0 => ProceduralDepositCategory::MetallicOre,
                1 => ProceduralDepositCategory::SmeltingFlux,
                2 => ProceduralDepositCategory::SolidFuel,
                3 => ProceduralDepositCategory::LiquidFuel,
                other => return Err(format!("invalid deposit category {other}")),
            };
            let dep = HiddenDepositView {
                cell,
                material_id,
                category,
                total_mass_tonnes: read_f64_bits(input)?,
                average_purity_01: read_f32_bits(input)?,
                overburden_depth_m: read_f32_bits(input)?,
                aquifer_flood_risk_01: read_f32_bits(input)?,
            };
            per_cell_bins.entry(cell.get()).or_default().push(dep);
        }
        let mut cell_deposit_ranges = Vec::with_capacity(n);
        for idx in 0..n {
            let s = deposits.len();
            if let Some(list) = per_cell_bins.get(&(idx as u32)) {
                deposits.extend(list.iter().cloned());
            }
            cell_deposit_ranges.push((s, deposits.len()));
        }

        let materials = read_material_catalog(input)?;
        let watershed_ids = compute_watershed_ids(&river_downstream, &is_ocean);

        *self = Self {
            world_seed,
            config: cfg,
            topology,
            elevation_m,
            raw_tectonic_uplift_m,
            is_ocean,
            is_convergent_boundary,
            is_divergent_boundary,
            plate_id,
            strata,
            biome_keys,
            base_precip_mm_yr,
            mean_annual_temp_c,
            prevailing_wind_uv,
            edge_slope_rad,
            river_downstream,
            watershed_ids,
            drainage_area_km2,
            topo_sorted_land_cells,
            deposits,
            cell_deposit_ranges,
            materials,
        };
        Ok(())
    }
}

fn compute_watershed_ids(
    river_downstream: &[Option<CellId>],
    is_ocean: &[bool],
) -> Vec<sim_spine::WatershedId> {
    let n = river_downstream.len();
    let mut outlet_ranks: BTreeMap<u32, sim_spine::WatershedId> = BTreeMap::new();
    let mut next_wid = 1u32;
    let mut out = vec![sim_spine::WatershedId(0); n];

    for idx in 0..n {
        if is_ocean[idx] {
            continue;
        }
        let mut cur = idx;
        for _ in 0..n {
            let Some(ds) = river_downstream[cur] else {
                break;
            };
            let ds_idx = ds.get() as usize;
            if ds_idx >= n || is_ocean[ds_idx] || ds_idx == cur {
                break;
            }
            cur = ds_idx;
        }
        let wid = *outlet_ranks.entry(cur as u32).or_insert_with(|| {
            let id = sim_spine::WatershedId(next_wid);
            next_wid += 1;
            id
        });
        out[idx] = wid;
    }
    out
}

fn write_planet_gen_config(out: &mut Vec<u8>, cfg: &PlanetGenConfigView) {
    write_str(out, &cfg.id);
    write_u8(out, cfg.goldberg_subdivision);
    write_u16_le(out, cfg.tectonic_plate_count);
    write_f32_bits(out, cfg.land_fraction_target);
    write_f32_bits(out, cfg.axial_tilt_deg);
    write_f32_bits(out, cfg.solar_constant_w_m2);
    write_f32_bits(out, cfg.rotation_period_hours);
    write_f32_bits(out, cfg.mean_radius_km);
    write_f32_bits(out, cfg.base_sea_level_m);
    write_f32_bits(out, cfg.max_orogenic_uplift_m);
    write_f32_bits(out, cfg.planetary_density_kg_m3);
    write_u16_le(out, cfg.natural_bounds.ore_count);
    write_u16_le(out, cfg.natural_bounds.flux_count);
    write_u16_le(out, cfg.natural_bounds.fuel_count);
    write_u16_le(out, cfg.natural_bounds.crop_count);
    write_u16_le(out, cfg.natural_bounds.fauna_count);
    write_u16_le(out, cfg.natural_bounds.pathogen_count);
    for v in cfg.natural_bounds.ore_melting_k_range {
        write_f32_bits(out, v);
    }
    for v in cfg.natural_bounds.ore_purity_range {
        write_f32_bits(out, v);
    }
    for v in cfg.natural_bounds.fuel_enthalpy_mj_kg_range {
        write_f32_bits(out, v);
    }
    for v in cfg.natural_bounds.crop_opt_temp_c_range {
        write_f32_bits(out, v);
    }
}

fn read_planet_gen_config(input: &mut &[u8]) -> Result<PlanetGenConfigView, String> {
    Ok(PlanetGenConfigView {
        id: read_str(input)?,
        goldberg_subdivision: read_u8(input)?,
        tectonic_plate_count: read_u16_le(input)?,
        land_fraction_target: read_f32_bits(input)?,
        axial_tilt_deg: read_f32_bits(input)?,
        solar_constant_w_m2: read_f32_bits(input)?,
        rotation_period_hours: read_f32_bits(input)?,
        mean_radius_km: read_f32_bits(input)?,
        base_sea_level_m: read_f32_bits(input)?,
        max_orogenic_uplift_m: read_f32_bits(input)?,
        planetary_density_kg_m3: read_f32_bits(input)?,
        natural_bounds: NaturalBoundsView {
            ore_count: read_u16_le(input)?,
            flux_count: read_u16_le(input)?,
            fuel_count: read_u16_le(input)?,
            crop_count: read_u16_le(input)?,
            fauna_count: read_u16_le(input)?,
            pathogen_count: read_u16_le(input)?,
            ore_melting_k_range: [read_f32_bits(input)?, read_f32_bits(input)?],
            ore_purity_range: [read_f32_bits(input)?, read_f32_bits(input)?],
            fuel_enthalpy_mj_kg_range: [read_f32_bits(input)?, read_f32_bits(input)?],
            crop_opt_temp_c_range: [read_f32_bits(input)?, read_f32_bits(input)?],
        },
    })
}

fn write_material_catalog(out: &mut Vec<u8>, cat: &ProceduralMaterialCatalog) {
    write_u32_le(out, cat.fluxes.len() as u32);
    for f in &cat.fluxes {
        write_u16_le(out, f.id.0);
        write_str(out, &f.key);
        write_f32_bits(out, f.basicity_index);
        write_f32_bits(out, f.slag_melting_k);
    }
    write_u32_le(out, cat.ores.len() as u32);
    for o in &cat.ores {
        write_u16_le(out, o.id.0);
        write_str(out, &o.key);
        write_f32_bits(out, o.melting_point_k);
        write_f32_bits(out, o.reduction_enthalpy_mj_kg);
        write_f32_bits(out, o.base_grade_01);
        write_f32_bits(out, o.trace_impurity_01);
        write_f32_bits(out, o.max_recoverable_yield_01);
        write_u16_le(out, o.optimal_flux_id.0);
        write_f32_bits(out, o.optimal_alloy_tensile_mpa);
        write_f32_bits(out, o.brittleness_penalty_without_flux_01);
    }
    write_u32_le(out, cat.fuels.len() as u32);
    for f in &cat.fuels {
        write_u16_le(out, f.id.0);
        write_str(out, &f.key);
        write_u8(out, u8::from(f.is_liquid));
        write_f32_bits(out, f.calorific_enthalpy_mj_kg);
        write_f32_bits(out, f.max_thermal_efficiency_01);
        write_f32_bits(out, f.useful_energy_mj_kg);
        write_f32_bits(out, f.ash_fouling_01);
    }
    write_u32_le(out, cat.crops.len() as u32);
    for c in &cat.crops {
        write_u16_le(out, c.id.0);
        write_str(out, &c.key);
        write_f32_bits(out, c.optimal_temp_c);
        write_f32_bits(out, c.frost_lethal_c);
        write_f32_bits(out, c.soil_moisture_pref_01);
        write_f32_bits(out, c.yield_kg_per_ha);
    }
    write_u32_le(out, cat.fauna.len() as u32);
    for fa in &cat.fauna {
        write_u16_le(out, fa.id.0);
        write_str(out, &fa.key);
        write_f32_bits(out, fa.draft_power_watts);
        write_f32_bits(out, fa.daily_fodder_kg);
        write_f32_bits(out, fa.wild_hazard_01);
    }
    write_u32_le(out, cat.pathogens.len() as u32);
    for p in &cat.pathogens {
        write_u16_le(out, p.id.0);
        write_str(out, &p.key);
        write_u16_le(out, p.incubation_hours);
        write_f32_bits(out, p.base_r0);
        write_f32_bits(out, p.case_fatality_01);
    }
}

fn read_material_catalog(input: &mut &[u8]) -> Result<ProceduralMaterialCatalog, String> {
    let n_flux = read_u32_le(input)? as usize;
    let mut fluxes = Vec::with_capacity(n_flux);
    for _ in 0..n_flux {
        fluxes.push(ProceduralFluxDef {
            id: ProceduralMaterialId(read_u16_le(input)?),
            key: read_str(input)?,
            basicity_index: read_f32_bits(input)?,
            slag_melting_k: read_f32_bits(input)?,
        });
    }
    let n_ore = read_u32_le(input)? as usize;
    let mut ores = Vec::with_capacity(n_ore);
    for _ in 0..n_ore {
        ores.push(ProceduralOreDef {
            id: ProceduralMaterialId(read_u16_le(input)?),
            key: read_str(input)?,
            melting_point_k: read_f32_bits(input)?,
            reduction_enthalpy_mj_kg: read_f32_bits(input)?,
            base_grade_01: read_f32_bits(input)?,
            trace_impurity_01: read_f32_bits(input)?,
            max_recoverable_yield_01: read_f32_bits(input)?,
            optimal_flux_id: ProceduralMaterialId(read_u16_le(input)?),
            optimal_alloy_tensile_mpa: read_f32_bits(input)?,
            brittleness_penalty_without_flux_01: read_f32_bits(input)?,
        });
    }
    let n_fuel = read_u32_le(input)? as usize;
    let mut fuels = Vec::with_capacity(n_fuel);
    for _ in 0..n_fuel {
        fuels.push(ProceduralFuelDef {
            id: ProceduralMaterialId(read_u16_le(input)?),
            key: read_str(input)?,
            is_liquid: read_u8(input)? != 0,
            calorific_enthalpy_mj_kg: read_f32_bits(input)?,
            max_thermal_efficiency_01: read_f32_bits(input)?,
            useful_energy_mj_kg: read_f32_bits(input)?,
            ash_fouling_01: read_f32_bits(input)?,
        });
    }
    let n_crop = read_u32_le(input)? as usize;
    let mut crops = Vec::with_capacity(n_crop);
    for _ in 0..n_crop {
        crops.push(ProceduralCropDef {
            id: ProceduralMaterialId(read_u16_le(input)?),
            key: read_str(input)?,
            optimal_temp_c: read_f32_bits(input)?,
            frost_lethal_c: read_f32_bits(input)?,
            soil_moisture_pref_01: read_f32_bits(input)?,
            yield_kg_per_ha: read_f32_bits(input)?,
        });
    }
    let n_fauna = read_u32_le(input)? as usize;
    let mut fauna = Vec::with_capacity(n_fauna);
    for _ in 0..n_fauna {
        fauna.push(ProceduralFaunaDef {
            id: ProceduralMaterialId(read_u16_le(input)?),
            key: read_str(input)?,
            draft_power_watts: read_f32_bits(input)?,
            daily_fodder_kg: read_f32_bits(input)?,
            wild_hazard_01: read_f32_bits(input)?,
        });
    }
    let n_pathogen = read_u32_le(input)? as usize;
    let mut pathogens = Vec::with_capacity(n_pathogen);
    for _ in 0..n_pathogen {
        pathogens.push(ProceduralPathogenDef {
            id: ProceduralMaterialId(read_u16_le(input)?),
            key: read_str(input)?,
            incubation_hours: read_u16_le(input)?,
            base_r0: read_f32_bits(input)?,
            case_fatality_01: read_f32_bits(input)?,
        });
    }
    Ok(ProceduralMaterialCatalog {
        ores,
        fluxes,
        fuels,
        crops,
        fauna,
        pathogens,
    })
}
