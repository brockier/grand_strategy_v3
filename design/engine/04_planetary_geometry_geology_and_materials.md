# Module Design: Planetary Geometry, Tectonics, Hydrology & Procedural Science

**Module ID:** `ENG-GEO-04`  
**Code Location:** `code/engine/crates/sim-planet/`  
**Milestone & Design Depth:** `M1: Full Detailed Design`  
**Satisfies Requirements:** `ENG-GEO-001.1`, `ENG-GEO-002`, `ENG-GEO-003`, `ENG-GEO-004`, `ENG-GEO-005`, `ENG-GEO-006` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `ModRegistryPort` (`PlanetGenConfig`, physical bounds and trade-off curves for procedural materials, crops, fauna, and pathogens)
  - 64-bit `world_seed`
- **Provided Spine Ports (Outputs):**
  - `PlanetGeometryPort` (welded subdivided icosphere continuous field, `icosphere_triangles()`, continuous `sample_elevation_at_unit_xyz`, fast `nearest_sample_cell`, sample coordinates, elevations, edge slopes, watershed downstream links, natural watershed regions, surface geological strata)
  - `SubsurfaceDepositTruthPort` (sealed ground-truth deposit veins, grade distributions, and water-table hazards readable *only* by `sim-economy` extraction and survey mechanics inside `sim-core`—never by `player-view` directly!)
  - `ProceduralMaterialCatalog` (world-generated ores, trace impurities, fluxes, fuels, crops, fauna, and pathogens)

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Welded Subdivided Icosphere Continuous Field (Hex-Free Option B)

- Subdivision frequency `m` (configurable in `planet/generation.toml`; for instance `m = 8` yields `642` sample vertices and `1,280` spherical triangles; `m = 32` yields `10,242` sample vertices and `20,480` triangles without any exposed hexagonal dual faces).
- Stored as flat, cache-aligned Compressed Sparse Row (`CSR`) arrays plus smooth triangulation:
  - `cell_center_unit_xyz: Vec<[f32; 3]>` (unit sphere positions)
  - `cell_lat_lon_rad: Vec<[f32; 2]>`
  - `cell_area_km2: Vec<f32>`
  - `icosphere_triangles: Vec<[u32; 3]>` (outward CCW closed spherical manifold triangles)
  - `neighbor_offsets: Vec<u32>`, `neighbor_cells: Vec<CellId>` (sample graph adjacency sorted counter-clockwise)
  - `edge_endpoints: Vec<(CellId, CellId)>`, `edge_length_km: Vec<f32>`, `edge_slope_rad: Vec<f32>`
  - `watershed_id: Vec<WatershedId>` (natural hydrological drainage basins for hex-free region labels)

### 2.2 Geological & Hydrological Cell Records

```rust
pub struct CellTerrainData {
    pub elevation_m: f32,         // negative = ocean bathymetry, >= 0 = land
    pub plate_id: PlateId,
    pub rock_strata: RockStrataClass, // IgneousOrogenic | MetamorphicShield | SedimentaryBasin | VolcanicArc | AlluvialPlain
    pub surface_soil: SoilClass,
    pub river_downstream: Option<CellId>,
    pub watershed_id: WatershedId,
    pub drainage_area_km2: f32,
    pub base_climate_precip_mm_yr: f32,
}

pub struct HiddenSubsurfaceDeposit {
    pub cell: CellId,
    pub material_id: ProceduralMaterialId,
    pub total_mass_tonnes: f64,
    pub average_purity_01: f32,
    pub overburden_depth_m: f32,
    pub aquifer_flood_risk_01: f32,
}
```

### 2.3 World-Specific Procedural Natural Tables (`ProceduralMaterialCatalog`)

Each world seed rolls concrete entries within physical parameter ranges defined by `ModRegistryPort`:
- **Metallic Ores & Impurities:** Melting point, reduction enthalpy, trace brittle-inducing impurities (sulfur/phosphorus analogs), required neutralizing `FluxId`, achievable tensile strength vs. hardness curve.
- **Fuels:** Calorific energy (`MJ/kg`), sulfur/ash fouling rate, combustion temperature ceiling.
- **Crops & Flora:** Growing degree days, minimum frost lethality temperature, soil moisture preference, caloric and fiber yield per hectare.
- **Fauna & Pathogens:** Draft traction energy, fodder intake, reproductive curve; pathogen incubation hours, R0 transmission weight by temperature/crowding, and mortality hazard.

---

## 3. Algorithms & World Generation Pipeline

All stages run deterministically from `ChaCha8Rng::seed_from_u64(world_seed)` using fixed IEEE-754 order without `fast-math`:

1. **Welded Spherical Manifold Construction (`ENG-GEO-001.1`):** Generate subdivided unit icosahedron triangles at frequency `m` with `10 * m^2 + 2` vertices and `20 * m^2` outward CCW triangles, sort sample neighbors counter-clockwise, verify Euler characteristic `V - E + F == 2`, and expose inverse-angular / barycentric terrain interpolation `sample_elevation_at_unit_xyz(u)` alongside `O(m)` hill-climbing `nearest_sample_cell(u)`.
2. **Tectonic Plates & Relief (`ENG-GEO-002`):** Sample `K` plate seeds via geodesic farthest-point sampling, flood-assign plates with noise-perturbed boundaries, assign continental vs. oceanic crust + Euler pole rotation vectors. Compute convergent boundary stress (`dot(relative_vel, boundary_normal)`) to raise orogenic mountain belts, rifts, and volcanic arcs, followed by multi-pass thermal/fluvial erosion smoothing passes while preserving continuous passes.
3. **Wind Gyres & Orographic Precipitation (`ENG-GEO-004`):** Compute latitudinal Hadley/Ferrel/Polar prevailing wind vectors rotated by Coriolis deflection; march moisture parcels downwind across cell steps, precipitating moisture up windward slopes and leaving realistic rain shadows on leeward slopes.
4. **Priority-Flood Acyclic Watersheds (`ENG-GEO-004`):** Execute Barnes priority-flood depression resolution from ocean coast cells inward across land elevation so every land cell receives a strictly non-increasing downhill `river_downstream: Option<CellId>` pointer (guaranteeing an acyclic DAG terminating at sea or endorheic salt basins), then accumulate `drainage_area_km2`.
5. **Strata-Correlated Subsurface Deposit Placement (`ENG-GEO-003`):** Sample ore veins conditioned on `RockStrataClass` and boundary uplift (`IgneousOrogenic` favors metallic sulfides/oxides; `SedimentaryBasin` favors coal/hydrocarbon/limestone beds) with hidden variance in grade, overburden depth, and aquifer risk.
6. **Bounded Procedural Material Roll (`ENG-GEO-005`):** Generate world-specific materials, fluxes, fuels, crops, fauna, and pathogens using rejection-bounded physical curves so mass conservation (`output_metal <= ore_mass * purity`) and thermodynamic efficiency limits (`heat_released <= fuel_enthalpy`) can never be violated.

---

## 4. Isolated Cartridge Verification Strategy

1. **Spherical Manifold Invariants (`@req ENG-GEO-001.1`):** Tests assert `20 * m^2` triangles, `10 * m^2 + 2` vertices, symmetric neighbor adjacency, zero self-loops, exact closed sphere area sum, continuous interpolated elevation, and watershed region naming.
2. **Tectonic Relief Statistics (`@req ENG-GEO-002`):** Across 20 random seeds, tests assert convergent plate-boundary cells have statistically higher mean elevation than interior craton cells and land ratio hits configured targets.
3. **Deposit-Strata Correlation (`@req ENG-GEO-003`):** Chi-square / correlation test confirms higher metallic deposit probability in `IgneousOrogenic` vs. `AlluvialPlain` strata while verifying hidden deposit fields remain outside `PlanetGeometryPort` public surface traits.
4. **Watershed Acyclicity & Rain Shadows (`@req ENG-GEO-004`):** Graph traversal verifies zero cycles in `river_downstream`, monotonic downhill elevation along every river edge, and higher windward vs. leeward precipitation on major ranges.
5. **Procedural Material Physical Bounds (`@req ENG-GEO-005`):** Property tests verify every generated ore/fuel/crop obeys conservation of mass, positive melting bounds, and varied optimal flux recipes across distinct seeds.
6. **Seed Determinism (`@req ENG-GEO-006`):** Generates identical planets twice per seed across two threads and verifies identical Blake3 digests.

---

## 5. Lift-and-Shift Replacement Guide

1. Any geodesic mesh generator (for instance, hexagonal cubed-sphere or rotated icosahedral-hexagonal grids) or tectonic solver can replace `code/engine/crates/sim-planet/` as long as it constructs a closed neighbor CSR graph implementing `PlanetGeometryPort` and preserves deterministic seeded generation.
2. No downstream module assumes flat 2D `(x, y)` grid coordinates; all distance, neighbor, and slope math queries `PlanetGeometryPort`.
