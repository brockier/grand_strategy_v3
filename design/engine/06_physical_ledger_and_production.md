# Module Design: Physical Goods Ledger, Tool Wear & Production

**Module ID:** `ENG-ECO-06`  
**Code Location:** `code/engine/crates/sim-economy/`  
**Milestone & Design Depth:** `M3: Full Detailed Design`  
**Satisfies Requirements:** `ENG-ECO-001`, `ENG-ECO-002`, `ENG-ECO-003`, `ENG-ECO-004` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `ModRegistryPort` (commodity masses/spoilage curves, process recipes, equipment wear parameters)
  - `SurfaceConditionsPort` (weather moisture, freeze/storm construction halts, mine water inflow)
  - `PopulationPort` (available cohort labor hours, vocational proficiency, cohort caloric/fuel consumption requirements)
  - `SubsurfaceDepositTruthPort` (true remaining vein tonnage, purity, overburden, and water inflow for open mine shafts)
  - `NonStateEconomyPlugPort` (`M5` plug hook) and `KnowledgeModifierPlugPort` (`M6` plug hook)
- **Provided Spine Ports (Outputs):**
  - `PhysicalLedgerPort` (spatial inventories, batch queries, atomic double-entry `LedgerCommand` execution)
  - Facility production telemetry & tool wear/failure events

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Single Mass-Conserving Physical Stockpile Ledger

```rust
pub struct StockpileRecord {
    pub id: StockpileId,
    pub cell_id: CellId,
    pub settlement_id: Option<SettlementId>,
    pub owner: ActorId,
    pub roof_protection_01: f32, // 0.0 open yard, 1.0 sealed dry granary/warehouse
    pub capacity_m3: f64,
}

pub struct StockBatch {
    pub batch_id: BatchId,
    pub stockpile_id: StockpileId,
    pub owner: ActorId,
    pub commodity_id: CommodityId,
    pub mass_kg: f64,
    pub purity_01: f32,     // 0.0..=1.0 material purity/grade
    pub condition_01: f32,  // Remaining durability or freshness (1.0 new -> 0.0 broken/rotten)
    pub created_tick: Tick,
}
```

### 2.2 Ownership-Agnostic Facility & Equipment Wear State

```rust
pub struct FacilityInstance {
    pub id: FacilityId,
    pub cell_id: CellId,
    pub settlement_id: SettlementId,
    pub owner: ActorId,              // Never hardcoded Public vs. Private!
    pub process_id: ProcessId,
    pub input_stockpile: StockpileId,
    pub output_stockpile: StockpileId,
    pub assigned_cohort: CohortId,
    pub shaft_water_level_01: f32,   // Drainage state for extraction sites
    pub construction_progress_01: f32,
    pub active: bool,
}
```

No struct contains `is_public_only` or `is_private_only` flags (`ENG-ECO-004`). Facility permissions depend strictly on externalized policy checks queried via `ActorId`.

---

## 3. Algorithms, Mass Balance & Process Execution

### 3.1 Atomic Double-Entry Mass Transfers (`ENG-ECO-001`)
1. Every movement, recipe conversion, spoilage event, or consumption enters as a `LedgerCommand` (`Consume`, `Produce`, `Transfer { from_stock, to_stock, mass_kg }`, `TransferOwnership`).
2. Before committing any `LedgerCommand`:
   - Verify `mass_kg >= 0.0` and available batch mass `>= requested_mass_kg`.
   - Split partial batches cleanly while preserving weighted-average `purity_01` and `condition_01`.
   - When executing `ProcessStep`, assert conservation within floating-point epsilon (`1e-6 kg`):
     `sum(consumed_input_masses) == sum(primary_outputs) + sum(slag_or_ash_mass) + sum(exhaust_gas_mass)`.

### 3.2 Physical Equipment Wear & Starting Tool Clock (`ENG-ECO-002`)
1. Every batch run consumes `wear = base_wear * rock_or_material_hardness * worker_inexperience_penalty` against active tool batches inside `input_stockpile`.
2. Tool efficiency scales as `efficiency(condition_01)` (`condition_01 > 0.25` keeps near-nominal output; below `0.25` efficiency plunges; at `0.0` tool converts into scrap/broken material and halts precision work).
3. In **"The Unexplained Colony"**, starting high-grade steel tools degrade realistically over initial extraction/construction months, forcing local smelting and smithing prior to tool exhaustion.

### 3.3 Daily Sharded Production, Subsistence & Spoilage (`ENG-ECO-003`)
Executed during `TickPhase::DailyBucketExtractionProductionConstruction` for `facility_id % 24 == tick % 24`:
1. **Weather & Drainage Gate:** Update `shaft_water_level_01` from `SurfaceConditionsPort` plus any working pump processes; halt flooded mines or weather-blocked outdoor construction.
2. **Knowledge Gate:** Query `KnowledgeModifierPlugPort::is_process_constructible(settlement, process_id)` (returns mod-enabled default in `M3`–`M5`).
3. **Input & Tool Reservation:** Verify all physical inputs, fuel, water, and usable tools exist locally in `input_stockpile`.
4. **Labor & Proficiency Yield:** Query `PopulationPort::cohort_view(assigned_cohort)` for available worker-hours, stamina, vocational experience, and `CommandPort` oversight dilution modifier.
5. **Deposit Drawdown (Extraction):** Read `SubsurfaceDepositTruthPort` remaining tonnage and purity; draw true extracted ore mass and return ore batch stamped with actual purity and rock waste fraction.
6. **Spoilage & Subsistence:** Subtract daily household caloric rations, potable water, heating fuel, and clothing wear per cohort; apply temperature/humidity spoilage (`spoil_rate * (1.0 + precip_exposure * (1.0 - roof_protection_01))`) moving decayed organic mass to waste/compost.

---

## 4. Isolated Cartridge Verification Strategy

1. **Mass Conservation Property Tests (`@req ENG-ECO-001`):** QuickCheck/proptest fuzzing `10,000` random transfers, splits, and recipes verifies strict mass equality and zero negative stocks.
2. **Tool Degradation Clock (`@req ENG-ECO-002`):** Runs an unmaintained mine using finite starting tools until `condition_01 == 0.0`, asserting smooth efficiency drop and clean work halt without forging replacement tools.
3. **Subsistence & External Recipe Execution (`@req ENG-ECO-003`):** Feeds synthetic alien recipes and verifies starvation/cold flags trigger when rations run dry.
4. **Ownership Parity (`@req ENG-ECO-004`):** Executes identical iron-smelting facility recipes under a `State` `ActorId` and a `PrivateSyndicate` `ActorId` confirming zero branching differences in physical mechanics.

---

## 5. Lift-and-Shift Replacement Guide

1. To replace batch-mass accounting with continuous thermodynamic reactor simulation or serialized unit-level tools, keep `PhysicalLedgerPort` and `StockpileId`/`BatchId` contracts untouched inside `sim-spine`.
2. Zero changes to `sim-logistics`, `sim-pop`, `player-view`, or `requirements/engine_requirements.md` are needed.
