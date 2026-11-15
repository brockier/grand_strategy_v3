# Module Design: Physical Carriers & Shared Logistics Routing

**Module ID:** `ENG-LOG-07`  
**Code Location:** `code/engine/crates/sim-logistics/`  
**Milestone & Design Depth:** `M3–M4: Full Detailed Design`  
**Satisfies Requirements:** `ENG-LOG-001`, `ENG-LOG-002`, `ENG-LOG-002.1`, `ENG-LOG-003`, `ENG-LOG-004` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `PlanetGeometryPort` (edge lengths, slopes, river links, great-circle distance)
  - `SurfaceConditionsPort` (hourly `mud_index_01`, `flood_stage_01`, `ice_thickness_m`, `snow_depth_m`, `wind_u_mps`, `wind_v_mps`)
  - `ModRegistryPort` (carrier payload caps, energy burn curves, infrastructure build/traversal specs)
  - `PhysicalLedgerPort` (loading/unloading physical cargo batches and refueling route-local fodder/fuel)
  - `NonStateEconomyPlugPort` (`M5` plug hook returning edge congestion factor and charter market rates)
- **Provided Spine Ports (Outputs):**
  - `LogisticsPort` (`enqueue_shipment`, `enqueue_courier`, `convoy_state`, `arrived_envelopes`)
  - Unified transit updates for cargo, cohorts, HQ staff moves, survey teams, and physical couriers

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Edge Infrastructure & Transit Entity Model

```rust
pub struct EdgeInfraState {
    pub edge_id: EdgeId,
    pub infra_class: InfraClassId,
    pub condition_01: f32,
    pub bridge_intact: bool,
    pub bridge_max_tonnes: f32,
}

pub enum TransitPayload {
    Freight { cargo_stockpile: StockpileId },
    PassengerCohorts { cohort_ids: Vec<CohortId> },
    NamedRetinue { character_ids: Vec<CharacterId>, is_hq_move: bool },
    SurveyExpedition { leader: CharacterId, target_cells: Vec<CellId> },
    PhysicalCourier { envelopes: Vec<CourierEnvelope> },
}

pub struct TransitEntity {
    pub id: ConvoyId,
    pub owner: ActorId,
    pub carrier_class: CarrierClassId,
    pub carrier_count: u32,
    pub origin_unit_xyz: [f32; 3],
    pub current_unit_xyz: [f32; 3],
    pub destination_unit_xyz: [f32; 3],
    pub current_cell: CellId,
    pub route_cells: Vec<CellId>,
    pub route_waypoints_xyz: Vec<[f32; 3]>,
    pub distance_travelled_km: f64,
    pub total_route_km: f64,
    pub current_speed_kmh: f32,
    pub payload: TransitPayload,
    pub supply_stockpile: StockpileId, // Fodder, fuel, spare parts carried onboard
    pub departure_tick: Tick,
    pub scheduled_eta_tick: Tick,
    pub stranded_reason: Option<StrandReason>,
}
```

Every moving entity—from a grain ox-cart to a migrating cohort, relocating player HQ, or mounted dispatch rider—is stored as a `TransitEntity` in one unified `LogisticsArena` carrying continuous 3D unit-sphere coordinates (`current_unit_xyz`) advanced along road splines and string-pulled spherical great-circle segments (`ENG-LOG-002.1`).

---

## 3. Algorithms, Edge-Cost Physics & Anti-Teleportation Invariant

### 3.1 Time-Dependent Edge Traversal Cost (`ENG-LOG-002`)
For any candidate edge `(u, v)` and `CarrierDef`:
1. **Structural Impassability Check:**
   - Reject land carriers crossing deep water unless `bridge_intact && convoy_tonnage <= bridge_max_tonnes` or `ice_thickness_m >= carrier.min_ice_thickness_m`.
   - Reject river barges when `ice_thickness_m >= 0.15` or `flood_stage_01 > carrier.max_safe_flood`.
   - Reject mountain passes when `snow_depth_m > carrier.max_snow_clearance_m`.
2. **Effective Speed & Continuous Arc Equation (`ENG-LOG-002`, `ENG-LOG-002.1`):**
   `speed_kmh = base_speed * slope_penalty(heading_slope, carrier) * mud_penalty(mud_index_01) * congestion_penalty * wind_sailing_factor(bearing, wind_vec)`
   where `congestion_penalty` queries `NonStateEconomyPlugPort`. Each step integrates `step_km = speed_kmh * dt_hours`, advances `distance_travelled_km` along road/river splines or any-angle string-pulled great-circle arcs, updates `current_unit_xyz` via `slerp`, and syncs `current_cell = planet.nearest_sample_cell(current_unit_xyz)`.
3. **Breakdown & Hazard Rolls:** Deterministic `PhaseRng` evaluates axle/wheel or hull failure probability scaled by `mud_index_01`, road wear, and excess weight, consuming carried spare parts or stranding the convoy.

### 3.2 Route Fodder & Fuel Consumption (`ENG-LOG-001`, `ENG-LOG-004`)
1. Every movement step computes energy burn `E = (carrier_tare_kg + cargo_mass_kg) * step_km * terrain_resistance`.
2. Energy converts directly into fodder, coal, oil, or crew ration withdrawals from `supply_stockpile` via `PhysicalLedgerPort`.
3. When entering an intermediate depot/settlement cell, convoys optionally top up fuel/fodder; if `supply_stockpile` runs dry mid-route, the convoy sets `stranded_reason = OutOfFuelOrFodder`, stalling movement until rescued or resupplied.
4. **Zero Teleportation (`ENG-LOG-004`):** Cargo only enters destination `StockpileId` on the exact tick `current_cell == destination_cell` via explicit `LedgerCommand::Transfer { from: cargo_stockpile, to: dest_stockpile }`.

### 3.3 Unified Multi-Rate Stepping (`ENG-LOG-003`)
- **Hourly (`Phase 4: HourlyCourierAndSignalStep`):** Advances high-speed `PhysicalCourier`, `NamedRetinue` (including HQ moves), and urgent emergency dispatches every single hour so dispatch latency resolves accurately down to the hour.
- **Daily Sharded (`Phase 5: DailyBucketLogisticsAndMarkets`):** Advances bulk freight convoys, survey expeditions, and cohort migrations deterministically (`convoy_id % 24 == tick % 24`) across 24-hour movement steps, queuing `ArrivedCourierEnvelope` records whenever couriers reach HQ or target officials.

---

## 4. Isolated Cartridge Verification Strategy

1. **Desert Fodder Exhaustion (`@req ENG-LOG-001`):** Dispatches an ox-cart across barren cells without forage; verifies fodder depletion matches payload weight equation and strands the cart mid-route.
2. **Weather & Bridge Obstruction (`@req ENG-LOG-002`):** Tests slow uphill mud speeds, halted barges on frozen river edges, and blocked movement across washed-out flood bridges.
3. **Unified Channel Parity (`@req ENG-LOG-003`):** Washes out a single bridge on a mock graph and asserts simultaneous delay across cargo carts, HQ relocation travel, and couriers.
4. **Anti-Teleportation Property Test (`@req ENG-LOG-004`):** Severs graph paths between Camp A and Smelter B; asserts zero mass ever crosses from A's stockpiles to B's stockpiles across `1,000` ticks.

---

## 5. Lift-and-Shift Replacement Guide

1. You can swap A* for Contraction Hierarchies, flow-network continuous routing, or multi-hop relay scheduling completely inside `code/engine/crates/sim-logistics/`.
2. The replacement cartridge must honor `LogisticsPort`, consume `PlanetGeometryPort` + `SurfaceConditionsPort`, preserve mass via `PhysicalLedgerPort`, and emit `ArrivedCourierEnvelope`s strictly upon physical edge completion.
