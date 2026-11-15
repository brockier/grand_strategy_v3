//! Exact closed Goldberg polyhedron `GP(m, 0)` geodesic topology (`ENG-GEO-001`).

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;

use sim_spine::{CellId, EdgeId};

/// Maps scenario subdivision depth cleanly into icosahedral frequency `m`.
/// `subdiv <= 6` maps to `2^subdiv` (for instance `3 -> m=8 -> 642 cells`); `subdiv >= 8` is used directly.
#[must_use]
pub fn subdivision_to_frequency(goldberg_subdivision: u8) -> u32 {
    if goldberg_subdivision >= 8 {
        u32::from(goldberg_subdivision)
    } else {
        1u32 << u32::from(goldberg_subdivision.min(6))
    }
}

#[inline]
fn dot3(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[inline]
fn sub3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

#[inline]
fn add3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

#[inline]
fn scale3(a: [f64; 3], s: f64) -> [f64; 3] {
    [a[0] * s, a[1] * s, a[2] * s]
}

#[inline]
pub fn cross3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

#[inline]
pub fn norm3(v: [f64; 3]) -> [f64; 3] {
    let len = dot3(v, v).sqrt().max(1e-15);
    [v[0] / len, v[1] / len, v[2] / len]
}

/// Computes local `(east, north)` orthonormal tangent basis vectors for a unit-sphere normal.
#[must_use]
pub fn tangent_basis(p: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let pole = if p[1].abs() < 0.99 {
        [0.0, 1.0, 0.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let east = norm3(cross3(pole, p));
    let north = norm3(cross3(p, east));
    (east, north)
}

/// Spherical triangle area on unit sphere (Steradian excess using Oosterom & Strackee formula).
fn unit_spherical_triangle_area(a: [f64; 3], b: [f64; 3], c: [f64; 3]) -> f64 {
    let det = dot3(a, cross3(b, c)).abs();
    let denom = 1.0 + dot3(a, b) + dot3(b, c) + dot3(c, a);
    2.0 * det.atan2(denom.max(1e-12))
}

fn canonical_icosahedron() -> (Vec<[f64; 3]>, Vec<[usize; 3]>) {
    let phi = (1.0 + 5.0_f64.sqrt()) * 0.5;
    let raw_verts = [
        [-1.0, phi, 0.0],
        [1.0, phi, 0.0],
        [-1.0, -phi, 0.0],
        [1.0, -phi, 0.0],
        [0.0, -1.0, phi],
        [0.0, 1.0, phi],
        [0.0, -1.0, -phi],
        [0.0, 1.0, -phi],
        [phi, 0.0, -1.0],
        [phi, 0.0, 1.0],
        [-phi, 0.0, -1.0],
        [-phi, 0.0, 1.0],
    ];
    let verts: Vec<[f64; 3]> = raw_verts.into_iter().map(norm3).collect();

    let raw_faces = [
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];

    let faces = raw_faces
        .into_iter()
        .map(|[a, b, c]| {
            let centroid = add3(verts[a], add3(verts[b], verts[c]));
            let normal = cross3(sub3(verts[b], verts[a]), sub3(verts[c], verts[a]));
            if dot3(normal, centroid) < 0.0 {
                [a, c, b]
            } else {
                [a, b, c]
            }
        })
        .collect();

    (verts, faces)
}

/// Closed geodesic Goldberg polyhedron CSR graph and spherical metrics (`ENG-GEO-001`).
#[derive(Clone, Debug, PartialEq)]
pub struct GoldbergTopology {
    pub frequency: u32,
    pub radius_km: f32,
    pub cell_unit_xyz: Vec<[f32; 3]>,
    pub cell_lat_lon_rad: Vec<[f32; 2]>,
    pub cell_area_km2: Vec<f32>,
    pub neighbor_offsets: Vec<u32>,
    pub neighbor_cells: Vec<CellId>,
    pub edge_endpoints: Vec<(CellId, CellId)>,
    pub edge_length_km: Vec<f32>,
    pub edge_map: BTreeMap<(u32, u32), EdgeId>,
}

impl GoldbergTopology {
    /// Generates a closed Goldberg polyhedron `GP(m, 0)` on a sphere of `radius_km`.
    #[must_use]
    pub fn generate(frequency: u32, radius_km: f32) -> Self {
        let m = frequency.max(1);
        let (ico_v, ico_f) = canonical_icosahedron();
        let expected_cells = (10 * m * m + 2) as usize;

        let mut positions: Vec<[f64; 3]> = ico_v.clone();
        let mut edge_vert_map: BTreeMap<(u32, u32, u32), u32> = BTreeMap::new();
        let mut raw_neighbors: Vec<BTreeSet<u32>> = vec![BTreeSet::new(); expected_cells];

        fn edge_vertex(
            a: usize,
            b: usize,
            step: u32,
            m: u32,
            ico_v: &[[f64; 3]],
            positions: &mut Vec<[f64; 3]>,
            edge_vert_map: &mut BTreeMap<(u32, u32, u32), u32>,
        ) -> u32 {
            let (lo, hi, t) = if a < b {
                (a as u32, b as u32, step)
            } else {
                (b as u32, a as u32, m - step)
            };
            if let Some(&id) = edge_vert_map.get(&(lo, hi, t)) {
                return id;
            }
            let alpha = f64::from(t) / f64::from(m);
            let p = norm3(add3(
                scale3(ico_v[lo as usize], 1.0 - alpha),
                scale3(ico_v[hi as usize], alpha),
            ));
            let id = positions.len() as u32;
            positions.push(p);
            edge_vert_map.insert((lo, hi, t), id);
            id
        }

        let mut grid: Vec<Vec<u32>> = Vec::with_capacity((m + 1) as usize);
        for &[v_a, v_b, v_c] in &ico_f {
            grid.clear();
            for row in 0..=m {
                let mut row_ids = Vec::with_capacity((m - row + 1) as usize);
                for col in 0..=(m - row) {
                    let i = m - row - col;
                    let j = row;
                    let k = col;
                    let vid = if i == m {
                        v_a as u32
                    } else if j == m {
                        v_b as u32
                    } else if k == m {
                        v_c as u32
                    } else if k == 0 {
                        edge_vertex(v_a, v_b, j, m, &ico_v, &mut positions, &mut edge_vert_map)
                    } else if j == 0 {
                        edge_vertex(v_a, v_c, k, m, &ico_v, &mut positions, &mut edge_vert_map)
                    } else if i == 0 {
                        edge_vertex(v_b, v_c, k, m, &ico_v, &mut positions, &mut edge_vert_map)
                    } else {
                        let inv = 1.0 / f64::from(m);
                        let p = norm3(add3(
                            scale3(ico_v[v_a], f64::from(i) * inv),
                            add3(
                                scale3(ico_v[v_b], f64::from(j) * inv),
                                scale3(ico_v[v_c], f64::from(k) * inv),
                            ),
                        ));
                        let id = positions.len() as u32;
                        positions.push(p);
                        id
                    };
                    row_ids.push(vid);
                }
                grid.push(row_ids);
            }

            let mut link_tri = |a: u32, b: u32, c: u32| {
                raw_neighbors[a as usize].insert(b);
                raw_neighbors[a as usize].insert(c);
                raw_neighbors[b as usize].insert(a);
                raw_neighbors[b as usize].insert(c);
                raw_neighbors[c as usize].insert(a);
                raw_neighbors[c as usize].insert(b);
            };

            for r in 0..(m as usize) {
                let cur = &grid[r];
                let nxt = &grid[r + 1];
                for c in 0..nxt.len() {
                    link_tri(cur[c], nxt[c], cur[c + 1]);
                    if c + 1 < nxt.len() {
                        link_tri(cur[c + 1], nxt[c], nxt[c + 1]);
                    }
                }
            }
        }

        let n_cells = positions.len();
        assert_eq!(n_cells, expected_cells, "Goldberg vertex count invariant");

        let mut cell_unit_xyz = Vec::with_capacity(n_cells);
        let mut cell_lat_lon_rad = Vec::with_capacity(n_cells);
        let mut cell_area_km2 = Vec::with_capacity(n_cells);
        let mut neighbor_offsets = Vec::with_capacity(n_cells + 1);
        let mut neighbor_cells = Vec::with_capacity(n_cells * 6);
        let mut edge_endpoints = Vec::with_capacity(30 * (m * m) as usize);
        let mut edge_length_km = Vec::with_capacity(30 * (m * m) as usize);
        let mut edge_map = BTreeMap::new();

        let r_km = f64::from(radius_km.max(1.0));
        let r2 = r_km * r_km;

        for idx in 0..n_cells {
            let p = positions[idx];
            cell_unit_xyz.push([p[0] as f32, p[1] as f32, p[2] as f32]);
            let lat = p[1].clamp(-1.0, 1.0).asin();
            let lon = p[2].atan2(p[0]);
            cell_lat_lon_rad.push([lat as f32, lon as f32]);

            // Counter-clockwise neighbor ordering around outward normal.
            let (east, north) = tangent_basis(p);
            let mut sorted_nbrs: Vec<(f64, u32)> = raw_neighbors[idx]
                .iter()
                .map(|&nb| {
                    let diff = sub3(positions[nb as usize], p);
                    let angle = dot3(diff, north).atan2(dot3(diff, east));
                    (angle, nb)
                })
                .collect();
            sorted_nbrs.sort_by(|a, b| a.0.total_cmp(&b.0));

            let deg = sorted_nbrs.len();
            neighbor_offsets.push(neighbor_cells.len() as u32);
            for &(_, nb) in &sorted_nbrs {
                neighbor_cells.push(CellId(nb));
                let lo = (idx as u32).min(nb);
                let hi = (idx as u32).max(nb);
                edge_map.entry((lo, hi)).or_insert_with(|| {
                    let eid = EdgeId(edge_endpoints.len() as u32);
                    let ang = dot3(positions[lo as usize], positions[hi as usize])
                        .clamp(-1.0, 1.0)
                        .acos();
                    edge_endpoints.push((CellId(lo), CellId(hi)));
                    edge_length_km.push((ang * r_km) as f32);
                    eid
                });
            }

            // Dual Voronoi cell spherical polygon area from centroids of adjacent triangles.
            let mut dual_corners = Vec::with_capacity(deg);
            for k in 0..deg {
                let nb_a = sorted_nbrs[k].1 as usize;
                let nb_b = sorted_nbrs[(k + 1) % deg].1 as usize;
                let centroid = norm3(add3(p, add3(positions[nb_a], positions[nb_b])));
                dual_corners.push(centroid);
            }
            let mut unit_area = 0.0;
            for k in 0..deg {
                unit_area += unit_spherical_triangle_area(
                    p,
                    dual_corners[k],
                    dual_corners[(k + 1) % deg],
                );
            }
            cell_area_km2.push((unit_area * r2) as f32);
        }
        neighbor_offsets.push(neighbor_cells.len() as u32);

        // Normalize dual area tiny rounding drift within exact sphere surface area `4 * PI * R^2`.
        let target_area = 4.0 * PI * r2;
        let current_area: f64 = cell_area_km2.iter().map(|&a| f64::from(a)).sum();
        if current_area > 0.0 {
            let corr = target_area / current_area;
            for area in &mut cell_area_km2 {
                *area = (f64::from(*area) * corr) as f32;
            }
        }

        Self {
            frequency: m,
            radius_km,
            cell_unit_xyz,
            cell_lat_lon_rad,
            cell_area_km2,
            neighbor_offsets,
            neighbor_cells,
            edge_endpoints,
            edge_length_km,
            edge_map,
        }
    }

    /// Returns slice of neighbor `CellId`s in CCW order around `cell`.
    #[must_use]
    pub fn neighbors(&self, cell: CellId) -> &[CellId] {
        let idx = cell.get() as usize;
        if idx + 1 >= self.neighbor_offsets.len() {
            return &[];
        }
        let s = self.neighbor_offsets[idx] as usize;
        let e = self.neighbor_offsets[idx + 1] as usize;
        &self.neighbor_cells[s..e]
    }

    /// Looks up undirected edge identifier connecting `a` and `b`.
    #[must_use]
    pub fn find_edge(&self, a: CellId, b: CellId) -> Option<EdgeId> {
        let lo = a.get().min(b.get());
        let hi = a.get().max(b.get());
        self.edge_map.get(&(lo, hi)).copied()
    }

    /// Computes exact great-circle surface arc distance in kilometers between cells `a` and `b`.
    #[must_use]
    pub fn great_circle_km(&self, a: CellId, b: CellId) -> f32 {
        let pa = self.cell_unit_xyz[a.get() as usize];
        let pb = self.cell_unit_xyz[b.get() as usize];
        let dot = (f64::from(pa[0]) * f64::from(pb[0])
            + f64::from(pa[1]) * f64::from(pb[1])
            + f64::from(pa[2]) * f64::from(pb[2]))
        .clamp(-1.0, 1.0);
        (dot.acos() * f64::from(self.radius_km)) as f32
    }
}
