# Module Design: Planetary Geometry, Tectonics, Hydrology & Procedural Science

**Module ID:** `ENG-GEO-04`  
**Code Location:** `code/engine/crates/sim-planet/`  
**Milestone & Design Depth:** `M1–M7: Full Detailed Design`  
**Satisfies Requirements:** `ENG-GEO-001.1`, `ENG-GEO-002`, `ENG-GEO-003`, `ENG-GEO-004`, `ENG-GEO-005`, `ENG-GEO-006`, `ENG-GEO-007` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `ModRegistryPort` (`PlanetGenConfig`, physical bounds and trade-off curves for procedural materials, crops, fauna, and pathogens)
  - 64-bit `world_seed`
- **Provided Spine Ports (Outputs):**
  - `PlanetGeometryPort` (welded subdivided icosphere continuous field, `icosphere_triangles()`, continuous `sample_elevation_at_unit_xyz`, fast `nearest_sample_cell`, sample coordinates, elevations, edge slopes, watershed downstream links, natural watershed regions, surface geological strata, `cell_mean_annual_temp_c`, `cell_aridity_index_01`, `cell_soil_fertility_01`, `cell_strahler_order`, `cell_strata_code`)
  - `SubsurfaceDepositTruthPort` (sealed ground-truth deposit veins, grade distributions, and water-table hazards readable *only* by `sim-economy` extraction and survey mechanics inside `sim-core`—never by `player-view` directly!)
  - `ProceduralMaterialCatalog` (world-generated ores, trace impurities, fluxes, fuels, crops, fauna, and pathogens)

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Welded Subdivided Icosphere Continuous Field (Hex-Free Option B, `m = 24`)

- Subdivision frequency `m` (configured in `mods/core-schemas/planet/generation.toml` as `goldberg_subdivision = 24`, yielding `5,762` sample vertices and `11,520` spherical triangles without any exposed hexagonal dual faces; `m = 8` (`642` vertices) remains valid for compact regression fixtures).
- Stored as flat, cache-aligned Compressed Sparse Row (`CSR`) arrays plus smooth triangulation:
  - `cell_center_unit_xyz: Vec<[f32; 3]>` (unit sphere positions)
  - `cell_lat_lon_rad: Vec<[f32; 2]>`
  - `cell_area_km2: Vec<f32>`
  - `icosphere_triangles: Vec<[u32; 3]>` (outward CCW closed spherical manifold triangles)
  - `neighbor_offsets: Vec<u32>`, `neighbor_cells: Vec<CellId>` (sample graph adjacency sorted counter-clockwise)
  - `edge_endpoints: Vec<(CellId, CellId)>`, `edge_length_km: Vec<f32>`, `edge_slope_rad: Vec<f32>`
  - `watershed_id: Vec<WatershedId>` (natural hydrological drainage basins for hex-free region labels)
  - Continuous biophysical fields: `aridity_index_01: Vec<f32>`, `soil_fertility_01: Vec<f32>`, `strahler_order: Vec<u8>`

### 2.2 Geological & Hydrological Cell Records

```rust
pub struct CellTerrainData {
    pub elevation_m: f32,         // negative = ocean bathymetry, >= 0 = land
    pub plate_id: PlateId,
    pub rock_strata: RockStrataClass, // OceanicBasalt | IgneousOrogenic | VolcanicArc | MetamorphicShield | SedimentaryBasin | AlluvialPlain
    pub surface_soil: SoilClass,
    pub river_downstream: Option<CellId>,
    pub watershed_id: WatershedId,
    pub drainage_area_km2: f32,
    pub strahler_order: u8,
    pub base_climate_precip_mm_yr: f32,
    pub aridity_index_01: f32,
    pub soil_fertility_01: f32,
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
2. **Domain-Warped Anisotropic Ridged Orogeny & Continental Shelves (`ENG-GEO-002`, `ENG-GEO-007`):** Sample `K` plate seeds via geodesic farthest-point sampling, flood-assign plates with noise-perturbed boundaries, assign continental versus oceanic crust + Euler pole rotation vectors, compute convergent boundary stress (`dot(relative_vel, boundary_normal)`), add domain-warped 5-octave ridged multifractal uplift (`(1 - |n(u + w(u))|)^2.2`) to form curved cordilleras, shape two-tier continental-shelf (`-15m .. -180m`) versus abyssal-basin bathymetry, and carve high-latitude glacial fjord/skerry troughs along cold coastal belts (`|lat| > 46 deg`).
3. **Wind Gyres & Orographic Precipitation (`ENG-GEO-004`):** Compute latitudinal Hadley/Ferrel/Polar prevailing wind vectors rotated by Coriolis deflection; march moisture parcels downwind across cell steps, precipitating moisture up windward slopes and leaving realistic rain shadows on leeward slopes.
4. **Priority-Flood Acyclic Watersheds, Valley Incision & Strahler Order (`ENG-GEO-004`, `ENG-GEO-007`):** Execute Barnes priority-flood depression resolution from ocean coast cells inward across land elevation so every land cell receives a strictly non-increasing downhill `river_downstream: Option<CellId>` pointer (guaranteeing an acyclic DAG terminating at sea or endorheic salt basins), accumulate `drainage_area_km2`, compute hierarchical integer `strahler_order` (`1..7`), and incise fluvial valleys monotonically without breaking downhill flow.
5. **Continuous Biophysical Indices & Strata Deposits (`ENG-GEO-003`, `ENG-GEO-007`):** Derive `aridity_index_01` from precipitation versus temperature-driven potential evapotranspiration and `soil_fertility_01` from parent `RockStrataClass`, drainage, moisture, and slope; sample subsurface deposits conditioned on geological strata.
6. **Bounded Procedural Material Roll (`ENG-GEO-005`):** Generate world-specific materials, fluxes, fuels, crops, fauna, and pathogens using rejection-bounded physical curves so mass conservation (`output_metal <= ore_mass * purity`) and thermodynamic efficiency limits (`heat_released <= fuel_enthalpy`) can never be violated.
7. **Circular Geodesic HQ Horizon (`ENG-EPI-002`):** `player-view::observe_hq_halo` traverses `VecDeque` breadth-first rings bounded by true geodesic `great_circle_km` radius (`hq_sight_radius_cells = 4` by default on `m = 24`) so explored horizons form natural circles rather than 6-sided graph spokes.

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
