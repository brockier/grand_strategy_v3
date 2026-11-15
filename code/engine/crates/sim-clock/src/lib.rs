//! Deterministic 1-hour simulation clock, calendar projection, 24h/168h staggered bucket
//! scheduler, counter-based `PhaseRng`, thread-independent parallel reductions, LZ4 save/load
//! snapshot codec, and non-state-mutating subsystem profiling telemetry (`ENG-CLK-001`..`006`).
#![deny(clippy::disallowed_types)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::f64::consts::TAU;
use std::fs;
use std::path::Path;

use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;
use rayon::prelude::*;
use sim_spine::codec::{
    read_bytes, read_str, read_u8, read_u16_le, read_u32_le, read_u64_le, write_bytes, write_str,
    write_u8, write_u16_le, write_u32_le, write_u64_le,
};
use sim_spine::{
    AutoPauseSignal, CalendarConfigView, CalendarDateTime, CanonicalSaveablePort,
    ClockControlCommand, EventClassId, PhaseTimingSample, Tick, TickPhase, TickTelemetryPort,
    TickTelemetrySummary,
};
use thiserror::Error;

/// Staggered update period for daily economic/extraction/transport entities (`ENG-CLK-003`).
pub const DAILY_PERIOD: u64 = 24;
/// Staggered update period for weekly demographic/maintenance entities (`ENG-CLK-003`).
pub const WEEKLY_PERIOD: u64 = 168;
/// Staggered update period for monthly census/audit sweeps.
pub const MONTHLY_PERIOD: u64 = 720;

const SAVE_MAGIC: [u8; 4] = *b"NGSV";
const SAVE_FORMAT_VERSION: u32 = 1;
const TELEMETRY_RING_CAPACITY: usize = 256;

/// Clock run state machine controlling pause, single-hour step, speed multiplier, and auto-pause (`ENG-CLK-002`).
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum ClockRunMode {
    Paused,
    StepSingleTick,
    Running { speed_multiplier: u32 },
    AutoPaused { reason_event: EventClassId, tick: Tick },
}

/// Maps discrete tick index to scenario-defined calendar fields and solar/diurnal angles (`ENG-CLK-001`).
#[must_use]
pub fn project_calendar(cal: &CalendarConfigView, tick: Tick) -> CalendarDateTime {
    let hours_per_day = u64::from(cal.hours_per_day.max(1));
    let days_per_year = u64::from(cal.days_per_year.max(1));
    let year_ticks = hours_per_day * days_per_year;

    let total_days = tick.get() / hours_per_day;
    let hour_of_day = (tick.get() % hours_per_day) as u32;
    let years_elapsed = (total_days / days_per_year) as i32;
    let day_of_year = (total_days % days_per_year) as u32;

    let mut remaining_days = day_of_year;
    let mut month_index = 0usize;
    let mut month_name = String::from("Cycle");
    for (idx, (name, &len)) in cal.month_names.iter().zip(cal.month_lengths.iter()).enumerate() {
        if remaining_days < len {
            month_index = idx;
            month_name = name.clone();
            break;
        }
        remaining_days -= len;
    }

    let tick_in_year = tick.get() % year_ticks.max(1);
    let solar_longitude_rad = TAU * (tick_in_year as f64) / (year_ticks.max(1) as f64);
    let diurnal_angle_rad = TAU * f64::from(hour_of_day) / (hours_per_day as f64);

    CalendarDateTime {
        tick,
        year: cal.starting_year + years_elapsed,
        month_index,
        month_name,
        day_of_year,
        day_of_month: remaining_days + 1,
        hour_of_day,
        solar_longitude_rad,
        diurnal_angle_rad,
    }
}

/// Authoritative hourly simulation clock controller implementing pause, single-step, variable rates,
/// auto-pause interruption, and canonical state persistence (`ENG-CLK-001`, `ENG-CLK-002`).
#[derive(Clone, Debug, PartialEq)]
pub struct SimulationClock {
    pub current_tick: Tick,
    pub world_seed: u64,
    pub mode: ClockRunMode,
    pub auto_pause_events: BTreeSet<EventClassId>,
}

impl SimulationClock {
    /// Creates a new simulation clock at `Tick(0)` in `Running { speed_multiplier: 1 }` mode.
    #[must_use]
    pub fn new(world_seed: u64) -> Self {
        Self {
            current_tick: Tick(0),
            world_seed,
            mode: ClockRunMode::Running {
                speed_multiplier: 1,
            },
            auto_pause_events: BTreeSet::new(),
        }
    }

    /// Configures whether raising `event_id` automatically suspends the simulation clock.
    pub fn set_event_auto_pause(&mut self, event_id: EventClassId, enabled: bool) {
        if enabled {
            self.auto_pause_events.insert(event_id);
        } else {
            self.auto_pause_events.remove(&event_id);
        }
    }

    /// Applies a validated clock control command without corrupting ticks.
    pub fn apply_control_command(&mut self, cmd: ClockControlCommand) {
        match cmd {
            ClockControlCommand::Pause => {
                self.mode = ClockRunMode::Paused;
            }
            ClockControlCommand::Resume => {
                self.mode = ClockRunMode::Running {
                    speed_multiplier: 1,
                };
            }
            ClockControlCommand::StepSingleHour => {
                self.mode = ClockRunMode::StepSingleTick;
            }
            ClockControlCommand::SetSpeedMultiplier(multiplier) => {
                self.mode = ClockRunMode::Running {
                    speed_multiplier: multiplier.max(1),
                };
            }
        }
    }

    /// Advances zero or more ticks depending on the active `ClockRunMode` and `requested_base_ticks`.
    /// Stops immediately on the exact tick where `exec_one` returns `Some(AutoPauseSignal)`.
    pub fn advance_frame(
        &mut self,
        requested_base_ticks: u64,
        mut exec_one: impl FnMut(Tick) -> Option<AutoPauseSignal>,
    ) -> u64 {
        let budget = match self.mode {
            ClockRunMode::Paused | ClockRunMode::AutoPaused { .. } => return 0,
            ClockRunMode::StepSingleTick => 1,
            ClockRunMode::Running { speed_multiplier } => {
                requested_base_ticks.saturating_mul(u64::from(speed_multiplier.max(1)))
            }
        };

        let mut executed = 0u64;
        for _ in 0..budget {
            let next_tick = Tick(self.current_tick.get() + 1);
            self.current_tick = next_tick;
            executed += 1;

            if let Some(signal) = exec_one(next_tick) {
                self.mode = ClockRunMode::AutoPaused {
                    reason_event: signal.reason_event,
                    tick: signal.tick,
                };
                return executed;
            }
        }

        if matches!(self.mode, ClockRunMode::StepSingleTick) {
            self.mode = ClockRunMode::Paused;
        }
        executed
    }
}

impl CanonicalSaveablePort for SimulationClock {
    fn cartridge_tag(&self) -> &'static str {
        "sim-clock"
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        write_u64_le(out, self.current_tick.get());
        write_u64_le(out, self.world_seed);
        match self.mode {
            ClockRunMode::Paused => write_u8(out, 0),
            ClockRunMode::StepSingleTick => write_u8(out, 1),
            ClockRunMode::Running { speed_multiplier } => {
                write_u8(out, 2);
                write_u32_le(out, speed_multiplier);
            }
            ClockRunMode::AutoPaused { reason_event, tick } => {
                write_u8(out, 3);
                write_u16_le(out, reason_event.get());
                write_u64_le(out, tick.get());
            }
        }
        write_u32_le(out, self.auto_pause_events.len() as u32);
        for ev in &self.auto_pause_events {
            write_u16_le(out, ev.get());
        }
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        self.current_tick = Tick(read_u64_le(input)?);
        self.world_seed = read_u64_le(input)?;
        self.mode = match read_u8(input)? {
            0 => ClockRunMode::Paused,
            1 => ClockRunMode::StepSingleTick,
            2 => ClockRunMode::Running {
                speed_multiplier: read_u32_le(input)?,
            },
            3 => ClockRunMode::AutoPaused {
                reason_event: EventClassId(read_u16_le(input)?),
                tick: Tick(read_u64_le(input)?),
            },
            tag => return Err(format!("Invalid ClockRunMode discriminant: {tag}")),
        };
        let rule_count = read_u32_le(input)? as usize;
        self.auto_pause_events.clear();
        for _ in 0..rule_count {
            self.auto_pause_events.insert(EventClassId(read_u16_le(input)?));
        }
        Ok(())
    }
}

/// Returns true iff `slot_or_dense_id` belongs to the active staggered hourly bucket (`ENG-CLK-003`).
#[must_use]
pub const fn in_period_bucket(slot_or_dense_id: u64, tick: Tick, period: u64) -> bool {
    if period == 0 {
        return true;
    }
    (slot_or_dense_id % period) == (tick.get() % period)
}

/// Assigns deterministic compact dense slots to arbitrary entity IDs so sparse IDs
/// distribute across buckets within `[floor(N / P), ceil(N / P)]` (`ENG-CLK-003`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct StaggeredBucketRegistry {
    slot_by_entity: BTreeMap<u64, u64>,
    next_slot: u64,
}

impl StaggeredBucketRegistry {
    /// Creates an empty deterministic staggered scheduler registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers an entity identifier and returns its compact deterministic bucket slot.
    pub fn register_entity(&mut self, entity_id: u64) -> u64 {
        if let Some(&slot) = self.slot_by_entity.get(&entity_id) {
            return slot;
        }
        let slot = self.next_slot;
        self.next_slot += 1;
        self.slot_by_entity.insert(entity_id, slot);
        slot
    }

    /// Returns true iff `entity_id` executes on `tick` under interval `period`.
    #[must_use]
    pub fn is_due(&self, entity_id: u64, tick: Tick, period: u64) -> bool {
        self.slot_by_entity
            .get(&entity_id)
            .is_some_and(|&slot| in_period_bucket(slot, tick, period))
    }

    /// Returns total registered entity count.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slot_by_entity.len()
    }

    /// Returns true when no entities are registered.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slot_by_entity.is_empty()
    }
}

/// Derives an isolated stateless counter-based `ChaCha8Rng` keyed solely by
/// `(world_seed, tick, phase, entity_id)` so parallel thread count never shifts RNG state (`ENG-CLK-004`).
#[must_use]
pub fn phase_rng(world_seed: u64, tick: Tick, phase: TickPhase, entity_id: u64) -> ChaCha8Rng {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"NG_PHASE_RNG_V1");
    hasher.update(&world_seed.to_le_bytes());
    hasher.update(&tick.get().to_le_bytes());
    hasher.update(&[phase as u8]);
    hasher.update(&entity_id.to_le_bytes());
    let digest: [u8; 32] = hasher.finalize().into();
    ChaCha8Rng::from_seed(digest)
}

/// Builds a configured `rayon::ThreadPool` for deterministic fixed-thread or native-thread runs.
pub fn build_worker_pool(num_threads: usize) -> Result<rayon::ThreadPool, rayon::ThreadPoolBuildError> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads.max(1))
        .build()
}

/// Executes `map_fn` over `items` in parallel (using `pool` when supplied) and sorts emitted
/// `(source_entity_id, sub_seq, payload)` tuples deterministically before reduction (`ENG-CLK-004`).
pub fn execute_parallel_sorted<T, D, F>(
    pool: Option<&rayon::ThreadPool>,
    items: &[T],
    map_fn: F,
) -> Vec<D>
where
    T: Sync,
    D: Send,
    F: Fn(&T) -> Vec<(u64, u32, D)> + Sync,
{
    let run = || {
        items
            .par_iter()
            .flat_map(&map_fn)
            .collect::<Vec<(u64, u32, D)>>()
    };

    let mut tagged = match pool {
        Some(p) => p.install(run),
        None => run(),
    };
    tagged.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
    tagged.into_iter().map(|(_, _, d)| d).collect()
}

/// Typed save/load error returned cleanly without panicking when archive headers or hashes fail (`ENG-CLK-005`).
#[derive(Clone, Debug, Eq, PartialEq, Error)]
pub enum SaveCodecError {
    #[error("Invalid save archive magic bytes (expected 'NGSV')")]
    InvalidMagic,
    #[error("Unsupported save format version {0} (expected {SAVE_FORMAT_VERSION})")]
    UnsupportedFormatVersion(u32),
    #[error("Mod bundle hash mismatch between save archive and active workspace mods")]
    ModBundleHashMismatch,
    #[error("Corrupted save archive: recomputed digest failed verification")]
    DigestMismatch,
    #[error("Save decompression failure: {0}")]
    Compression(String),
    #[error("Save stream decode failure: {0}")]
    Decode(String),
    #[error("Save file I/O failure: {0}")]
    Io(String),
}

fn encode_canonical_payload(
    clock: &SimulationClock,
    cartridges: &[&dyn CanonicalSaveablePort],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(4096);
    let mut clock_buf = Vec::with_capacity(64);
    clock.write_canonical_state(&mut clock_buf);
    write_str(&mut out, clock.cartridge_tag());
    write_bytes(&mut out, &clock_buf);

    write_u32_le(&mut out, cartridges.len() as u32);
    for c in cartridges {
        let mut c_buf = Vec::with_capacity(512);
        c.write_canonical_state(&mut c_buf);
        write_str(&mut out, c.cartridge_tag());
        write_bytes(&mut out, &c_buf);
    }
    out
}

/// Computes the canonical Blake3 world state digest across clock and all registered cartridges (`ENG-CLK-004`, `ENG-CLK-005`).
#[must_use]
pub fn compute_canonical_digest(
    clock: &SimulationClock,
    cartridges: &[&dyn CanonicalSaveablePort],
) -> [u8; 32] {
    let payload = encode_canonical_payload(clock, cartridges);
    blake3::hash(&payload).into()
}

/// Serializes `clock` and all `cartridges` into a compressed versioned save archive with header digest (`ENG-CLK-005`).
#[must_use]
pub fn save_snapshot_bytes(
    mod_bundle_hash: [u8; 32],
    clock: &SimulationClock,
    cartridges: &[&dyn CanonicalSaveablePort],
) -> Vec<u8> {
    let payload = encode_canonical_payload(clock, cartridges);
    let pre_save_digest: [u8; 32] = blake3::hash(&payload).into();
    let compressed = lz4_flex::compress_prepend_size(&payload);

    let mut archive = Vec::with_capacity(4 + 4 + 32 + 32 + compressed.len());
    archive.extend_from_slice(&SAVE_MAGIC);
    archive.extend_from_slice(&SAVE_FORMAT_VERSION.to_le_bytes());
    archive.extend_from_slice(&mod_bundle_hash);
    archive.extend_from_slice(&pre_save_digest);
    archive.extend_from_slice(&compressed);
    archive
}

/// Deserializes and validates a versioned save archive back into `clock` and `cartridges`,
/// enforcing format version, mod hash compatibility, and exact post-load Blake3 digest equality (`ENG-CLK-005`).
pub fn load_snapshot_bytes(
    expected_mod_bundle_hash: [u8; 32],
    archive: &[u8],
    clock: &mut SimulationClock,
    cartridges: &mut [&mut dyn CanonicalSaveablePort],
) -> Result<[u8; 32], SaveCodecError> {
    const HEADER_LEN: usize = 4 + 4 + 32 + 32;
    if archive.len() < HEADER_LEN {
        return Err(SaveCodecError::Decode("Archive shorter than 72-byte header".to_string()));
    }
    if archive[0..4] != SAVE_MAGIC {
        return Err(SaveCodecError::InvalidMagic);
    }
    let version = u32::from_le_bytes([archive[4], archive[5], archive[6], archive[7]]);
    if version != SAVE_FORMAT_VERSION {
        return Err(SaveCodecError::UnsupportedFormatVersion(version));
    }
    let mut stored_mod_hash = [0u8; 32];
    stored_mod_hash.copy_from_slice(&archive[8..40]);
    if stored_mod_hash != expected_mod_bundle_hash {
        return Err(SaveCodecError::ModBundleHashMismatch);
    }
    let mut pre_save_digest = [0u8; 32];
    pre_save_digest.copy_from_slice(&archive[40..72]);

    let decompressed = lz4_flex::decompress_size_prepended(&archive[HEADER_LEN..])
        .map_err(|e| SaveCodecError::Compression(e.to_string()))?;

    let raw_digest: [u8; 32] = blake3::hash(&decompressed).into();
    if raw_digest != pre_save_digest {
        return Err(SaveCodecError::DigestMismatch);
    }

    let mut cursor: &[u8] = &decompressed;
    let clock_tag = read_str(&mut cursor).map_err(SaveCodecError::Decode)?;
    if clock_tag != clock.cartridge_tag() {
        return Err(SaveCodecError::Decode(format!(
            "Expected '{}' tag, got '{clock_tag}'",
            clock.cartridge_tag()
        )));
    }
    let clock_bytes = read_bytes(&mut cursor).map_err(SaveCodecError::Decode)?;
    let mut clock_cur: &[u8] = &clock_bytes;
    clock
        .read_canonical_state(&mut clock_cur)
        .map_err(SaveCodecError::Decode)?;

    let count = read_u32_le(&mut cursor).map_err(SaveCodecError::Decode)? as usize;
    if count != cartridges.len() {
        return Err(SaveCodecError::Decode(format!(
            "Cartridge count mismatch: archive has {count}, runtime provided {}",
            cartridges.len()
        )));
    }
    for cartridge in cartridges.iter_mut() {
        let tag = read_str(&mut cursor).map_err(SaveCodecError::Decode)?;
        if tag != cartridge.cartridge_tag() {
            return Err(SaveCodecError::Decode(format!(
                "Cartridge tag mismatch: expected '{}', got '{tag}'",
                cartridge.cartridge_tag()
            )));
        }
        let c_bytes = read_bytes(&mut cursor).map_err(SaveCodecError::Decode)?;
        let mut c_cur: &[u8] = &c_bytes;
        cartridge
            .read_canonical_state(&mut c_cur)
            .map_err(SaveCodecError::Decode)?;
    }

    let refs: Vec<&dyn CanonicalSaveablePort> = cartridges.iter().map(|c| &**c).collect();
    let recomputed = compute_canonical_digest(clock, &refs);
    if recomputed != pre_save_digest {
        return Err(SaveCodecError::DigestMismatch);
    }
    Ok(recomputed)
}

/// Writes a compressed save archive cleanly to disk (`ENG-CLK-005`).
pub fn save_snapshot_file(
    path: &Path,
    mod_bundle_hash: [u8; 32],
    clock: &SimulationClock,
    cartridges: &[&dyn CanonicalSaveablePort],
) -> Result<(), SaveCodecError> {
    let bytes = save_snapshot_bytes(mod_bundle_hash, clock, cartridges);
    fs::write(path, bytes).map_err(|e| SaveCodecError::Io(e.to_string()))
}

/// Reads and restores a compressed save archive from disk (`ENG-CLK-005`).
pub fn load_snapshot_file(
    path: &Path,
    expected_mod_bundle_hash: [u8; 32],
    clock: &mut SimulationClock,
    cartridges: &mut [&mut dyn CanonicalSaveablePort],
) -> Result<[u8; 32], SaveCodecError> {
    let bytes = fs::read(path).map_err(|e| SaveCodecError::Io(e.to_string()))?;
    load_snapshot_bytes(expected_mod_bundle_hash, &bytes, clock, cartridges)
}

/// Non-state-mutating circular profiling buffer tracking all 8 `TickPhase` nanosecond timings (`ENG-CLK-006`).
#[derive(Clone, Debug, Default)]
pub struct TickTelemetryRing {
    samples: VecDeque<PhaseTimingSample>,
    cumulative_phase_ns: [u64; 8],
    cumulative_entities: [u64; 8],
    total_ticks_profiled: u64,
}

impl TickTelemetryRing {
    /// Creates an empty profiling ring buffer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends one completed tick timing sample without touching simulation state.
    pub fn record_sample(&mut self, sample: PhaseTimingSample) {
        self.total_ticks_profiled += 1;
        for (idx, (&ns, &count)) in sample
            .phase_ns
            .iter()
            .zip(sample.bucket_entities_processed.iter())
            .enumerate()
        {
            self.cumulative_phase_ns[idx] += ns;
            self.cumulative_entities[idx] += u64::from(count);
        }
        if self.samples.len() == TELEMETRY_RING_CAPACITY {
            self.samples.pop_front();
        }
        self.samples.push_back(sample);
    }

    /// Formats structured JSON profiling telemetry suitable for `sim-cli --profile` (`ENG-CLK-006`).
    #[must_use]
    pub fn to_json_value(&self) -> serde_json::Value {
        let mut by_phase = serde_json::Map::new();
        for phase in TickPhase::ordered() {
            let idx = phase.index();
            let total_ns = self.cumulative_phase_ns[idx];
            let avg_us = if self.total_ticks_profiled > 0 {
                (total_ns as f64) / (self.total_ticks_profiled as f64 * 1000.0)
            } else {
                0.0
            };
            by_phase.insert(
                phase.label().to_string(),
                serde_json::json!({
                    "totalMs": (total_ns as f64) / 1_000_000.0,
                    "avgUsPerTick": avg_us,
                    "entitiesProcessed": self.cumulative_entities[idx],
                }),
            );
        }

        serde_json::json!({
            "ticksProfiled": self.total_ticks_profiled,
            "ringSampleCount": self.samples.len(),
            "subsystems": by_phase,
            "weatherMs": (self.cumulative_phase_ns[TickPhase::HourlyWeatherAndHydrology.index()] as f64) / 1_000_000.0,
            "transportMs": (self.cumulative_phase_ns[TickPhase::HourlyCourierAndSignalStep.index()] as f64) / 1_000_000.0,
            "productionMs": (self.cumulative_phase_ns[TickPhase::DailyBucketExtractionProductionConstruction.index()] as f64) / 1_000_000.0,
            "populationMs": (self.cumulative_phase_ns[TickPhase::WeeklyMonthlyBucketDemographicsMandatesResearch.index()] as f64) / 1_000_000.0,
            "playerViewMs": (self.cumulative_phase_ns[TickPhase::CompileDiegeticPlayerViewAndNotifications.index()] as f64) / 1_000_000.0,
        })
    }

    /// Formats human-readable profiling summary covering weather, transport, production, population,
    /// and `player-view` projection (`ENG-CLK-006`).
    #[must_use]
    pub fn to_human_readable_report(&self) -> String {
        let mut out = format!(
            "Tick Profiling Summary (ticks_profiled={}, ring_samples={}):\n",
            self.total_ticks_profiled,
            self.samples.len()
        );
        for phase in TickPhase::ordered() {
            let idx = phase.index();
            let ms = (self.cumulative_phase_ns[idx] as f64) / 1_000_000.0;
            out.push_str(&format!(
                "  - {:<24} {:>10.3} ms  (entities: {})\n",
                phase.label(),
                ms,
                self.cumulative_entities[idx]
            ));
        }
        out
    }
}

impl TickTelemetryPort for TickTelemetryRing {
    fn telemetry_summary(&self) -> TickTelemetrySummary {
        TickTelemetrySummary {
            sample_count: self.samples.len(),
            total_ticks_profiled: self.total_ticks_profiled,
            cumulative_phase_ns: self.cumulative_phase_ns,
            latest_sample: self.samples.back().cloned(),
        }
    }
}
