# Module Design: Planetary Geometry, Tectonics, Hydrology & Procedural Science

**Module ID:** `ENG-GEO-04`  
**Code Location:** `code/engine/crates/sim-planet/`  
**Milestone & Design Depth:** `M1–M7: Full Detailed Design`  
**Satisfies Requirements:** `ENG-GEO-001.1`, `ENG-GEO-002`, `ENG-GEO-003`, `ENG-GEO-004`, `ENG-GEO-005`, `ENG-GEO-006`, `ENG-GEO-007`, `ENG-GEO-008`, `ENG-GEO-009`, `ENG-GEO-010`, `ENG-GEO-011`, `ENG-GEO-012`, `ENG-GEO-013` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

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
    pub elevation_m: f32,         // negative = submarine bathymetry, >= 0 = land
    pub channel_bed_elev_m: f32,  // strictly downhill incised river bed elevation
    pub water_surface_elev_m: f32,// monotonic water profile (0.0m at ocean shoreline)
    pub roughness_m: f32,         // sub-cell terrain roughness impeding unbuilt travel
    pub plate_id: PlateId,
    pub geology_province: GeologyProvinceKind, // 13 unified land & marine provinces
    pub province_age_myr: f32,
    pub basin_depth_m: f32,
    pub fault_instability_01: f32,
    pub volcanic_hazard_01: f32,
    pub rock_strata: RockStrataClass, // OceanicBasalt | IgneousOrogenic | VolcanicArc | MetamorphicShield | SedimentaryBasin | AlluvialPlain
    pub surface_soil: SoilClass,
    pub soil_drainage_01: f32,
    pub soil_clay_01: f32,
    pub marine_upwelling_01: f32,
    pub shelf_nutrient_01: f32,
    pub river_downstream: Option<CellId>, // General watershed drainage link across all land cells
    pub spill_parent: Option<CellId>,     // Priority-flood outlet across flat depressions
    pub watershed_id: WatershedId,
    pub drainage_area_km2: f32,
    pub strahler_order: u8,
    pub is_lake: bool,
    pub lake_kind: LakeKind,              // None | Exorheic | Endorheic
    pub lake_surface_elev_m: f32,
    pub lake_salinity_per_mille: f32,
    pub is_estuary: bool,
    pub river_navigability: RiverNavigabilityKind, // None | ShallowFord | Rapids | DeepNavigable
    pub river_channel_downstream: Option<CellId>, // Directed acyclic downstream perennial river channel link
    pub river_discharge_m3_s: f32,
    pub river_velocity_mps: f32,
    pub hydro_potential_kw: f32,
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
    pub accessibility_tier: u8,     // 1 (surface/outcrop), 2 (mid-shaft), 3 (deep/seismic)
    pub is_offshore: bool,          // continental_shelf submarine hydrocarbon/mineral bed
    pub is_alluvial_placer: bool,   // downstream river placer transported from upland orogens
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
2. **Two-Generation Tectonic Provinces & Unified Land-Sea Bathymetry (`ENG-GEO-002`, `ENG-GEO-007`, `ENG-GEO-009`, `ENG-GEO-010`):** Sample `K` plate seeds via geodesic farthest-point sampling, flood-assign plates with noise-perturbed boundaries, evaluate both ancient eroded relict sutures (`OldOrogen`, `ShieldCraton`, `Platform`) and modern active margins (`YoungOrogen`, `VolcanicArc`, `ContinentalRift`, `MidOceanRidge`, `OceanicTrench`), calibrate continuous land hypsometry and marine bathymetry (`ContinentalShelf` `-12m..-185m`, `ContinentalSlope` `-185m..-2800m`, `AbyssalPlain` `-3200m..-4600m`) to emergent sea level matching `land_fraction_target`, classify depositional lowlands (`SedimentaryBasin`, `AlluvialPlain`), and compute `basin_depth_m`, `fault_instability_01`, `volcanic_hazard_01`, and `roughness_m`.
3. **Wind Gyres, Orographic Precipitation & Marine Upwelling (`ENG-GEO-004`, `ENG-GEO-013`):** Compute latitudinal Hadley, Ferrel, and Polar prevailing wind vectors rotated by Coriolis deflection; march moisture parcels downwind across cell steps, precipitating moisture up windward slopes and leaving rain shadows on leeward slopes; compute offshore Ekman coastal `marine_upwelling_01` and estuarine/shelf `shelf_nutrient_01`.
4. **Spillway Priority-Flood, Monotonic Thalwegs, Exorheic/Endorheic Lakes, Estuaries & Navigability (`ENG-GEO-004`, `ENG-GEO-007`, `ENG-GEO-008`, `ENG-GEO-012`):** Execute Barnes priority-flood depression resolution from ocean coast cells inward across the spherical manifold, recording explicit `spill_parent` outlets across flat basins so every land sample receives an acyclic downhill pointer. Accumulate climate runoff (`effective_precip - evap`) topologically into `river_discharge_m3_s`, differentiate freshwater `Exorheic` overflow lakes (`salinity 0.1..0.5` per mille) from humid vs. arid terminal `Endorheic` salt basins (`22..165` per mille), clamp monotonic `channel_bed_elev_m` and `water_surface_elev_m` (`bed[u] > bed[ds]`, `surf[u] >= surf[ds] >= 0.0`), tag tidal `is_estuary` mouths and `RiverNavigabilityKind` (`ShallowFord`, `Rapids`, `DeepNavigable`), and prune/route every perennial reach so zero dry-land terminations remain.
5. **Continuous Soil Texture & Data-Driven Province-Affinity Deposits (`ENG-GEO-003`, `ENG-GEO-011`, `ENG-GEO-013`):** Derive `aridity_index_01`, `soil_drainage_01`, `soil_clay_01`, and `soil_fertility_01` from province weathering and climate; spawn contiguous multi-cell veins, `is_offshore` shelf deposits, and downstream `is_alluvial_placer` deposits strictly from externalized `[[deposit_affinities]]` in `mods/core-schemas/planet/geology.toml` with a guaranteed reachable `accessibility_tier == 1` starter ore and fuel deposit near coastal colony landing zones.
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
