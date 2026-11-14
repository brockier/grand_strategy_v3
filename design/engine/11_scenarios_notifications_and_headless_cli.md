# Module Design: Scenarios, Notification Router & Headless CLI

**Module ID:** `ENG-SCN-11`  
**Code Location:** `crates/sim-scenario/` and `crates/sim-cli/`  
**Milestone & Design Depth:** `M0, M1, M3, M4: Full Detailed Design`  
**Satisfies Requirements:** `ENG-SCN-001`, `ENG-SCN-002`, `ENG-SCN-003`, `ENG-SCN-004` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `ModRegistryPort` (loaded scenarios, initial entity templates, default notification rules, calendar config)
  - All Spine module composition interfaces wired inside the composition root (`sim-scenario`)
  - CLI invocation flags (`--scenario`, `--seed`, `--ticks`, `--threads`, `--profile`)
- **Provided Spine Ports (Outputs):**
  - Initialized world state across all registered cartridges (`ScenarioBuilder::build_world`)
  - Filtered per-event `NotificationDeliveryAction` (`LogOnly`, `BannerAlert`, `DispatchPopup`, `DispatchPopupAndAutoPause`) + `AutoPauseSignal`
  - Headless `sim-cli` executable emitting deterministic JSON run summaries and profiling metrics

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Scenario Archetypes & "The Unexplained Colony" Structure (`ENG-SCN-001`, `ENG-SCN-002`)

```rust
pub enum ScenarioArchetype {
    EmergentDawn,
    ExpeditionColony,
    GovernorSuccessor,
}

pub struct ScenarioInitSpec {
    pub archetype: ScenarioArchetype,
    pub planet_seed_override: Option<u64>,
    pub starting_settlements: Vec<StartingSettlementSpec>,
    pub starting_cohorts: Vec<StartingCohortSpec>,
    pub starting_characters: Vec<StartingCharacterSpec>,
    pub starting_stockpiles: Vec<StartingStockSpec>, // Including initial finite tool durability < 1.0 or finite wear life
    pub initial_hq_settlement: String,
    pub initial_known_radius_cells: u8,
}
```

For Scenario Pack 1 (`mods/unexplained-colony/`):
- `archetype = ExpeditionColony`, `initial_known_radius_cells = 1`, zero pre-revealed distant continent data.
- One coastal/river settlement, founding cohorts (general laborers, miners/quarriers, woodcutters/farmers, smiths/wrights, clerks), 3 to 5 named subordinates with distinct epistemic traits (for example, one candid methodical engineer, one sycophantic quartermaster, one bold scout), and finite high-grade starting steel tools carrying real wear depreciation.

### 2.2 Per-Event Notification Matrix State (`ENG-SCN-003`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotificationDeliveryMode {
    LogOnly,
    BannerAlert,
    DispatchPopup,
    DispatchPopupAndAutoPause,
}

pub struct NotificationConfigTable {
    pub rules_by_event_class: BTreeMap<EventClassId, NotificationDeliveryMode>,
}
```

---

## 3. Algorithms, Composition Boot & CLI Workflows

### 3.1 Deterministic Scenario Composition (`ENG-SCN-001`, `ENG-SCN-002`)
1. Load layered `CompiledModBundle` via `sim-data`.
2. Instantiate `sim-planet` from `seed` (searching seeded landing candidates matching scenario coastal/climate predicates when `planet_seed_override` is unset).
3. Spin up baseline `sim-weather` over warm-up ticks (so seasonal snowpack, river discharge, and soil moisture begin in realistic equilibrium rather than zeroed synthetic state).
4. Create initial `ActorId`s, settlements, `StockpileId`s, `DemographicCohort`s, `NamedIndividual`s, and starting `PlayerKnowledgeArchive` covering strictly `initial_known_radius_cells`.
5. Pre-wire all active cartridges (`M0`–`M4` cores plus active or baseline `M5`–`M7` plug adapters) into `SimulationWorld`.

### 3.2 Notification Evaluation & Auto-Pause Routing (`ENG-SCN-003`)
During `TickPhase::CompileDiegeticPlayerViewAndNotifications`:
1. Collect all `DiegeticEvent`s that physically reached HQ on the current tick (arrived courier dispatches, overdue expedition threshold crossings, HQ local shortages, battle reports).
2. Look up `rules_by_event_class.get(&event.class_id)` defaulting safely to `LogOnly`.
3. Tag each emitted `PlayerNotification` with its resolved `NotificationDeliveryMode`; if **any** event resolves to `DispatchPopupAndAutoPause`, emit `AutoPauseSignal(event.class_id, tick)` immediately to `sim-clock`.

### 3.3 `sim-cli` Headless Command Suite (`ENG-SCN-004`)
1. `sim-cli run --scenario <path> --ticks <N> --seed <u64> [--threads <k>] [--json-out <file>]`: executes headless at maximum native speed, prints ticks/sec throughput, final Blake3 hash, survivor counts, stockpile sums, and exits non-zero on invariant failures.
2. `sim-cli verify-determinism --scenario <path> --ticks <N>`: runs `1`-thread vs `4`-thread and save/reload mid-run comparisons automatically.
3. `sim-cli export-schemas --out <dir>`: invokes `sim-data` JSON Schema exporter.

---

## 4. Isolated Cartridge Verification Strategy

1. **Three-Archetype Boot (`@req ENG-SCN-001`):** Fixture tests boot `EmergentDawn`, `ExpeditionColony`, and `GovernorSuccessor` mod directories through one unified loader path.
2. **Unexplained Colony End-to-End (`@req ENG-SCN-002`):** Runs `mods/unexplained-colony/` via `sim-cli` for `8,760` ticks and verifies non-HQ cells begin `Unexplored`, starting tools wear measurably, and zero NaNs or panics occur.
3. **Notification Auto-Pause Switch (`@req ENG-SCN-003`):** Toggles `overdue_expedition` between `LogOnly` and `DispatchPopupAndAutoPause` and confirms clock pauses on the exact overdue arrival tick.
4. **Headless Throughput Benchmark (`@req ENG-SCN-004`):** Runs multi-decade benchmark via `sim-cli` without linking Electron or graphics libraries and asserts deterministic JSON telemetry output.

---

## 5. Lift-and-Shift Replacement Guide

1. `crates/sim-scenario/` is the *only* engine composition root that wires cartridges together. Adding a completely new scenario archetype or swapping a cartridge implementation (for instance, plugging in a custom `M5` economy crate) touches only the dependency binding table inside `ScenarioBuilder`.
2. Zero simulation physics live inside `sim-scenario` or `sim-cli`; `requirements/engine_requirements.md` remains untouched.
