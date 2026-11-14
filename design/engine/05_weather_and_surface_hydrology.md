# Module Design: Weather & Surface Hydrology

**Module ID:** `ENG-WTH-05`  
**Code Location:** `crates/sim-weather/`  
**Milestone & Design Depth:** `M1–M3: Full Detailed Design`  
**Satisfies Requirements:** `ENG-WTH-001`, `ENG-WTH-002`, `ENG-WTH-003`, `ENG-WTH-004`, `ENG-WTH-005` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `PlanetGeometryPort` (cell coordinates, elevation, neighbors, watershed downstream links, prevailing climate baseline)
  - `ModRegistryPort::calendar_config()` (axial tilt, orbital length, tick hour)
  - `TickPhase::HourlyWeatherAndHydrology` execution trigger
- **Provided Spine Ports (Outputs):**
  - `SurfaceConditionsPort` (`precip_mm_hr`, `mud_index_01`, `flood_stage_01`, `snow_depth_m`, `ice_thickness_m`, `temp_c`, `wind_u_mps`, `wind_v_mps`, `visibility_01`)
  - `WeatherHazardEvents` (`ShaftWaterInflow`, `ConstructionWeatherHalt`, `CropThermalOrMoistureStress`, `DampSpoilageMultiplier`) consumed by `sim-economy` and `sim-plug-warfare`

---

## 2. Encapsulated Internal State & Data Layout

`WeatherArena` stores flat Structure-of-Arrays (`SoA`) buffers indexed by `CellId.0` so hourly full-globe loops stay L1/L2 cache-friendly and parallelizable:

```rust
pub struct WeatherSoA {
    pub temp_c: Vec<f32>,
    pub atm_moisture_kg_m2: Vec<f32>,
    pub precip_mm_hr: Vec<f32>,
    pub soil_saturation_01: Vec<f32>,
    pub mud_index_01: Vec<f32>,
    pub snow_depth_m: Vec<f32>,
    pub river_storage_m3: Vec<f32>,
    pub river_discharge_m3s: Vec<f32>,
    pub flood_stage_01: Vec<f32>,
    pub ice_thickness_m: Vec<f32>,
    pub wind_u_mps: Vec<f32>,
    pub wind_v_mps: Vec<f32>,
    pub visibility_01: Vec<f32>,
    pub topo_order_cells: Vec<CellId>, // Upstream-to-downstream DAG sort order
}
```

Double-buffered swap vectors (`atm_moisture_next`, `wind_u_next`, `wind_v_next`) ensure parallel advection never depends on cell visit order or thread scheduling.

---

## 3. Algorithms, Physics Equations & Hazard Rules

### 3.1 Orbital Solar Insolation & Diurnal Cycle (`ENG-WTH-002`)
1. Compute orbital solar declination angle `decl = axial_tilt_rad * sin(2 * PI * (year_progress))`.
2. Compute hourly solar altitude cosine `cos_zenith` per cell from `(latitude, longitude, hour_angle, decl)`.
3. Update `temp_c[c]` toward radiative-convective equilibrium (`base_temp(lat, elev) + diurnal_swing * cos_zenith + oceanic_inertia_damping`), producing genuinely inverted summer/winter warmth between `+45` degrees North and `-45` degrees South simultaneously.

### 3.2 Advection, Orographic Condensation & Snowpack (`ENG-WTH-001`, `ENG-WTH-003`)
1. **Evaporation:** Over ocean and warm moist land cells, add surface moisture proportional to Clausius-Clapeyron saturation deficit and wind speed.
2. **Upwind Finite-Volume Advection:** Transport `atm_moisture_kg_m2` across neighbor edges weighted by positive `dot(wind_vec, edge_normal)`.
3. **Condensation & Fronts:** When advection lifts moist air up positive elevation slopes (`delta_elev > 0`) or moisture exceeds `saturation_capacity(temp_c)`, release `precip_mm_hr`:
   - If `temp_c <= 0.5`: deposit as `snow_depth_m` (`10:1` snow-to-water ratio).
   - If `temp_c > 0.5`: add rainfall to `soil_saturation_01` and trigger snowmelt proportional to `(temp_c - 0.5)`.
4. **Mud Formation:** `mud_index_01` rises non-linearly once `soil_saturation_01 > soil_field_capacity` and drains slowly with evaporation/infiltration.

### 3.3 Topological Watershed Flood Routing (`ENG-WTH-003`)
1. Traverse `topo_order_cells` strictly from headwater ridges down to river mouths (`river_downstream` DAG).
2. Surface runoff (`excess_rain + snowmelt`) enters `river_storage_m3[c]`, releasing `river_discharge_m3s[c]` into `river_downstream[c]` via linear-reservoir travel lag (`k_reach = length_km / flow_speed`), raising `flood_stage_01` when discharge exceeds channel bankfull capacity.
3. Extended sub-zero `temp_c` thickens `ice_thickness_m` across river channels (halting barges once `ice_thickness_m >= 0.15` while enabling foot/light-cart ice crossings once `>= 0.35`).

### 3.4 Downstream System Hazard Coupling (`ENG-WTH-004`, `ENG-WTH-005`)
1. **Mine & Pit Flooding:** Subsurface inflow rate equals `base_aquifer_inflow + k_rain * soil_saturation_01 + k_flood * flood_stage_01`; un-pumped mines halt extraction once shaft water crosses safety threshold.
2. **Outdoor Construction Halts:** Earthworks, lime-mortar masonry, and timber framing pause during hours where `precip_mm_hr >= halt_rain_threshold` or `temp_c <= freeze_mortar_threshold`.
3. **Crop & Spoilage Modifiers:** Frost below crop tolerance triggers biomass loss; excess `mud_index_01` / `precip_mm_hr` multiplies spoilage rates for unroofed stockpiles.

---

## 4. Isolated Cartridge Verification Strategy

1. **Variable Bounds (`@req ENG-WTH-001`):** `8,760`-tick full-globe runs verify finite values within `[0.0, 1.0]` for saturation, mud, flood stage, and visibility across all cells.
2. **Hemispheric Inversion (`@req ENG-WTH-002`):** Compares mean daily temperatures at `+45` degrees and `-45` degrees latitude across ticks `0..8760` confirming 180-degree orbital phase inversion.
3. **Downstream Flood Propagation (`@req ENG-WTH-003`):** Injects synthetic storm impulse at headwaters on a flat mock watershed and asserts mass conservation plus delayed flood-peak arrival downstream.
4. **Mine & Construction Halts (`@req ENG-WTH-004`):** Verifies `SurfaceConditionsPort` flags stop mock extraction and masonry steps under heavy rain/frost while dry/pumped fixtures proceed.
5. **Crop & Spoilage Stress (`@req ENG-WTH-005`):** Confirms unroofed grain degrades faster during sustained storm ticks than dry-stored grain.

---

## 5. Lift-and-Shift Replacement Guide

1. `crates/sim-weather/` can be replaced outright with a shallow-water atmospheric baroclinic solver or a simplified Markov regional weather generator without touching any downstream crate.
2. Any replacement MUST implement `SurfaceConditionsPort` with bounded per-cell hourly values and advance deterministically during `TickPhase::HourlyWeatherAndHydrology`.
