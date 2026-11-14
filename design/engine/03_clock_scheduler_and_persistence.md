# Module Design: Simulation Clock, Multi-Rate Scheduler & Persistence

**Module ID:** `ENG-CLK-03`  
**Code Location:** `crates/sim-clock/`  
**Milestone & Design Depth:** `M1: Full Detailed Design`  
**Satisfies Requirements:** `ENG-CLK-001`, `ENG-CLK-002`, `ENG-CLK-003`, `ENG-CLK-004`, `ENG-CLK-005`, `ENG-CLK-006` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `ModRegistryPort::calendar_config()`
  - `ClockControlCommand` (`Pause`, `Resume`, `StepSingleHour`, `SetSpeedMultiplier`)
  - `AutoPauseSignal` emitted during `TickPhase::CompileDiegeticPlayerViewAndNotifications`
  - Registered `TickPhaseHandler` callbacks from active engine cartridges
- **Provided Spine Ports (Outputs):**
  - `Tick` clock stream and calendar projection (`CalendarDateTime`)
  - Counter-based deterministic per-entity `PhaseRng` streams (`ChaCha8Rng`)
  - Versioned save/load archive serializer (`SaveSnapshotCodec`) and Blake3 world digest
  - `TickTelemetryPort` per-subsystem microsecond profiling ring buffer

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Clock & Calendar State Machine

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClockRunMode {
    Paused,
    StepSingleTick,
    Running { target_ticks_per_sec: u32 },
    AutoPaused { reason_event: EventClassId, tick: Tick },
}

pub struct SimulationClock {
    pub current_tick: Tick,
    pub world_seed: u64,
    pub mode: ClockRunMode,
    pub accumulator_ticks: f64,
}
```

`CalendarConfigView` (loaded from `sim-data`) maps `current_tick` cleanly to `(year, month_or_season, day, hour_of_day, solar_longitude_rad)` using `hours_per_day`, `days_per_year`, and season tables without hardcoded Gregorian assumptions unless specified by scenario data. Default baseline is `24` hours/day and `365` days/year (`8,760` ticks/year).

### 2.2 Staggered Bucket Partitioning & Counter-Based RNG

- **Daily Bucket Rule (`period = 24`):** Entity `id` updates strictly when `(id as u64 % 24) == (tick.0 % 24)`.
- **Weekly Bucket Rule (`period = 168`):** Entity `id` updates strictly when `(id as u64 % 168) == (tick.0 % 168)`.
- **Monthly / Audit Rule (`period = 720`):** Entity `id` updates strictly when `(id as u64 % 720) == (tick.0 % 720)`.
- **Thread-Count Independent RNG (`PhaseRng`):**
  Every randomized step derives an isolated stateless `ChaCha8Rng` seeded deterministically via SipHash/Blake3 mixing of `(world_seed, tick.0, phase_id as u8, entity_id_u64)`. Because RNG streams depend solely on entity identity and tick rather than shared mutable RNG cursor order, parallel `rayon::par_iter()` loops produce **bit-identical results across `1`, `2`, `4`, `8`, or `N` CPU threads**.

### 2.3 Versioned Save Archive Format & Telemetry Buffer

- Archive layout: `MagicBytes("NGSV") + FormatVersion(u32) + ModBundleHash([u8; 32]) + PreSaveStateDigest([u8; 32]) + ZstdCompressedCanonicalPayload` (serialized with fixed-endian `bincode` over sorted `BTreeMap`/`Vec` arrays—zero `HashMap` iteration order!).
- `TickTelemetryRing`: fixed `256`-tick circular buffer recording `[u64; 8]` nanosecond durations across all 8 `TickPhase` stages plus bucket entity counts.

---

## 3. Algorithms & Execution Flow

### 3.1 Single Hourly Tick Execution Loop
1. Inspect `ClockRunMode`: if `Paused` or `AutoPaused`, emit zero ticks and leave all cartridge arenas untouched.
2. Execute `TickPhase::IngestPlayerCommands`..`TickPhase::CompileDiegeticPlayerViewAndNotifications` in exact sequential phase order.
3. Inside daily/weekly phases (`Phases 5, 6, 7`), filter entities deterministically to the active hourly bucket (`bucket = tick % period`), process parallelizable per-entity read/compute stages via `rayon`, sort emitted `Command`/`Delta` vectors strictly by `(source_entity_id, sub_seq)` before applying sequential reductions to `PhysicalLedgerPort` or `PopulationPort`.
4. Advance `current_tick += 1`.
5. If `ClockRunMode == StepSingleTick`, immediately transition to `Paused`. If `CompileDiegeticPlayerViewAndNotifications` raised an `AutoPauseSignal`, immediately transition to `AutoPaused { reason_event, tick }`, halting any remaining ticks queued inside the current frame burst.

### 3.2 Deterministic Save & Reload Verification Protocol (`ENG-CLK-005`)
1. Saving can occur strictly *between* completed ticks (never mid-phase).
2. `SaveSnapshotCodec` collects serialized state snapshots from every registered cartridge in Spine declaration order alongside `SimulationClock`, `ModBundleHash`, and `Blake3` canonical state digest.
3. On reload, `sim-clock` checks `FormatVersion` and `ModBundleHash`, restores exact state across every cartridge, and verifies the post-load Blake3 digest equals `PreSaveStateDigest` before allowing tick `T + 1`.

---

## 4. Isolated Cartridge Verification Strategy

1. **Calendar & Hour Wrap (`@req ENG-CLK-001`):** Unit test steps `8,760` ticks and verifies exact wrap from `(Year 0, Day 364, Hour 23)` -> `(Year 1, Day 0, Hour 0)`.
2. **Pause, Step & Auto-Pause (`@req ENG-CLK-002`):** Mock cartridges assert zero mutations over 100 frame updates while `Paused`, exact `tick + 1` on `StepSingleTick`, and immediate halt on tick `k` when `AutoPauseSignal` triggers mid-burst.
3. **Bucket Coverage & Flat Variance (`@req ENG-CLK-003`):** Property test over `10,000` synthetic entities across `24` and `168` ticks asserts every entity runs exactly once per period and bucket size stays within `[floor(N/P), ceil(N/P)]`.
4. **Thread-Count Bit Parity (`@req ENG-CLK-004`):** Runs identical `2,160`-tick synthetic workload using `rayon::ThreadPoolBuilder` with `1`, `2`, `4`, and `8` threads and asserts identical Blake3 state hashes.
5. **Save/Load Determinism (`@req ENG-CLK-005`):** Runs continuous `0..1500` versus save at `500` + reload + `500..1500` and asserts equal tick-`1500` Blake3 digest.
6. **Profiler Telemetry (`@req ENG-CLK-006`):** Verifies `TickTelemetryPort` exports structured per-phase nanoseconds without altering any simulation state hash.

---

## 5. Lift-and-Shift Replacement Guide

1. To swap thread execution frameworks (for instance, replacing `rayon` with `tokio` worker pools or SIMD lane batching) or compression codecs (`zstd` -> `lz4`), modify only `crates/sim-clock/`.
2. Any replacement clock MUST preserve: `1 tick = 1 hour`, strict 8-phase sequential barrier ordering, deterministic bucket predicates, and `(world_seed, tick, phase, entity_id)` RNG derivation.
3. Zero sibling cartridges require modification.
