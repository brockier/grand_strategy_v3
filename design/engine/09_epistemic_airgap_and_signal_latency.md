# Module Design: Epistemic Air-Gap & Signal Latency (`crates/player-view`)

**Module ID:** `ENG-EPI-09`  
**Code Location:** `crates/player-view/`  
**Milestone & Design Depth:** `M3–M4: Full Detailed Design`  
**Satisfies Requirements:** `ENG-EPI-001`, `ENG-EPI-002`, `ENG-EPI-003`, `ENG-EPI-004`, `ENG-EPI-005`, `ENG-EPI-006`, `ENG-EPI-007` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `PlanetGeometryPort`, `SurfaceConditionsPort`, `PhysicalLedgerPort`, `LogisticsPort`, `PopulationPort`, `CommandPort` (read-only access strictly scoped inside Rust host boundary; *never* forwarded raw)
  - `ArrivedCourierEnvelope` queue delivered upon courier arrival at HQ (`TickPhase::HourlyCourierAndSignalStep`)
  - Outgoing `RelocateHqCommand` state
- **Provided Spine Ports (Outputs):**
  - `DiegeticSnapshot` (the **sole** top-level IPC DTO serializable to `apps/ui-shell`)
  - Arrived `DiegeticDispatch` inbox and `ArbitrationProposal` items
  - `AutoPauseSignal` triggers routed through `NotificationRouter`

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Physical HQ Presence State

```rust
pub enum HqPresenceState {
    Stationary { cell_id: CellId, sight_radius_cells: u8 },
    InTransit { convoy_id: ConvoyId, current_cell: CellId, origin: CellId, dest: CellId, eta_tick: Tick },
}
```

### 2.2 Persistent Cartographic & Ledger Knowledge Archive

`PlayerKnowledgeArchive` stores only what the player's administration has personally witnessed or received via physical dispatch—including inherited inaccuracies:

```rust
pub enum CellKnowledgeTier {
    Unexplored,
    Rumored { sketch_biome_hint: Option<String>, source_note: String },
    Surveyed {
        observed_tick: Tick,
        surveyor_id: Option<CharacterId>,
        elevation_estimate_m: f32,
        elevation_error_m: f32,
        biome_id: u16,
        known_rivers: Vec<CellId>,
        deposit_estimates: Vec<SurveyDepositEstimate>,
    },
}

pub struct SurveyDepositEstimate {
    pub material_label: String,
    pub min_tonnes_est: f64,
    pub point_tonnes_est: f64,
    pub max_tonnes_est: f64,
    pub confidence_01: f32,
    pub observed_tick: Tick,
    pub surveyor_id: CharacterId,
}
```

### 2.3 Ghosted Dead-Reckoning Schedule Tracker (`ENG-EPI-007`)

```rust
pub enum ProjectedMarkerStatus {
    ConfirmedAtHqSight,
    GhostedProjected,
    OverdueAlert { overdue_since_tick: Tick },
}

pub struct TrackedExpeditionOrConvoy {
    pub convoy_id: ConvoyId,
    pub last_confirmed_cell: CellId,
    pub last_confirmed_tick: Tick,
    pub planned_route: Vec<CellId>,
    pub expected_departure_tick: Tick,
    pub expected_arrival_tick: Tick,
    pub overdue_grace_ticks: u64,
    pub projected_cell: CellId,
    pub status: ProjectedMarkerStatus,
}
```

---

## 3. Algorithms, Air-Gap Rules & Epistemic Filtering

### 3.1 Compile-Time Air-Gap Enforcement (`ENG-EPI-001`)
1. `sim-core`, `sim-planet`, `sim-economy`, `sim-weather`, `sim-pop`, and `sim-logistics` internal types deliberately **do not implement** frontend IPC serialization traits (`IpcDtoSerialize`).
2. `crates/player-view` builds `DiegeticSnapshot` solely from `PlayerKnowledgeArchive` plus the `HqPresenceState` live sight radius. Unexplored cells omit elevation, rock strata, and weather inside serialized IPC snapshots (`CellViewState::Unexplored` carries zero terrain payload).

### 3.2 Immediate HQ Sight vs. Distant Latency (`ENG-EPI-002`, `ENG-EPI-003`, `ENG-EPI-004`)
1. On every tick `Phase 8`, BFS within `sight_radius_cells` around `HqPresenceState::current_cell()` writes live un-perturbed observations (`observed_tick = now`, `Direct Observation (HQ)`) for local terrain, weather, stockpiles, cohorts, and visible convoys.
2. Outside HQ sight radius, `PlayerKnowledgeArchive` performs **zero automatic polling** of remote truth. Updates occur strictly when:
   - A `PhysicalCourier` `TransitEntity` reaches `HqPresenceState::current_cell()` carrying an `ArrivedCourierEnvelope` (survey report, settlement ledger snapshot, bottleneck proposal, or convoy sighting), OR
   - An electrical telegraph/radio link exists and completes transmission across wired/line nodes.
3. **HQ Relocation (`ENG-EPI-003`):** Ordering `RelocateHq(dest)` spawns a `NamedRetinue` convoy in `sim-logistics` (`is_hq_move = true`). Couriers already inbound toward the old HQ cell must re-route from the old HQ relay to the traveling retinue, producing authentic dispatch lag during relocation.

### 3.3 Subordinate Epistemic Trait Perturbation (`ENG-EPI-005`, `ENG-EPI-006`)
When an outgoing report or ledger snapshot is generated at a distant settlement/expedition on send tick `T_send` by `CharacterId`:
1. **Precision (`precision_01`):** Adds zero-mean Gaussian/triangular noise proportional to `(1.0 - precision_01)` to stockpile, population, and tonnage estimates.
2. **Honesty vs. Sycophancy (`honesty_vs_sycophancy`):** If `honesty < 0.0` (sycophantic) and an active shortfall/delay exists (`actual < target`), clamps or inflates reported numbers upward toward `target` and suppresses non-critical hazard notes until crisis severity forces escalation.
3. **Caution vs. Alarmism (`caution_vs_alarmism`):** Shifts threat/shortfall margin estimates downward (complacent) or upward (alarmist).
4. **Survey Confidence (`ENG-EPI-005`):** Novice surveyors with basic hammers generate wide `[min_tonnes_est .. max_tonnes_est]` bounds; experienced geologists with core drills narrow intervals significantly without exposing exact `SubsurfaceDepositTruthPort` tonnage.

### 3.4 Dead-Reckoning Schedule Projection (`ENG-EPI-007`)
1. For each tracked out-of-sight convoy/expedition, interpolate `projected_cell` monotonically along `planned_route` between `(expected_departure_tick, expected_arrival_tick)`.
2. Even if mud/flooding secretly halted the convoy inside `sim-logistics`, `projected_cell` advances ghosts smoothly on schedule until `current_tick > expected_arrival_tick + overdue_grace_ticks`, flipping status to `OverdueAlert` and raising an event.
3. Whenever a traveler, patrol, or courier passes within sight of the real convoy and reports back to HQ, `projected_cell` snaps cleanly to the confirmed report coordinate and recalculates ETA.

---

## 4. Isolated Cartridge Verification Strategy

1. **Air-Gap Serialization Proof (`@req ENG-EPI-001`):** Inspects JSON output of `DiegeticSnapshot` on tick `0` for "The Unexplained Colony" and asserts zero elevation, mineral, or remote stock fields exist for non-HQ cells.
2. **HQ Zero-Latency Boundary (`@req ENG-EPI-002`):** Mutates HQ cell vs. remote cell stockpiles in mock ports; verifies immediate update at HQ and zero change at remote settlement until courier arrival.
3. **HQ Relocation Travel (`@req ENG-EPI-003`):** Relocates HQ along a 3-cell road and asserts sight halo shifts hourly with travel and dispatch travel time drops for nearby outposts.
4. **Courier Interception & Weather Delay (`@req ENG-EPI-004`):** Floods river bridge along courier path and asserts proportional arrival delay in `player-view`.
5. **Survey Precision Intervals (`@req ENG-EPI-005`):** Compares mock novice vs. expert survey outputs asserting interval bracketing around true mass without revealing exact tonnage.
6. **Epistemic Trait Bias (`@req ENG-EPI-006`):** Evaluates identical mining shortfall under sycophantic vs. honest overseer traits and asserts deterministic bias divergence.
7. **Dead-Reckoning Ghost & Overdue (`@req ENG-EPI-007`):** Strands a convoy secretly at cell `1` and asserts `GhostedProjected` advances along schedule until `ETA + grace` flips it to `OverdueAlert`.

---

## 5. Lift-and-Shift Replacement Guide

1. `crates/player-view/` is already isolated behind the IPC boundary; changing report compression formats, rumor spread models, or dead-reckoning heuristics never alters ground-truth simulation physics.
2. Any replacement epistemic filter MUST maintain the `DiegeticSnapshot` IPC contract consumed by `apps/ui-shell` and enforce the compile-time seal against raw `sim-core` types.
