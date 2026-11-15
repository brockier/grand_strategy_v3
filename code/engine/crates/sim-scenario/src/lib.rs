#![deny(clippy::disallowed_types)]

//! World scenario composition root uniting `sim-clock`, `sim-planet`, `sim-weather`,
//! `sim-core`, and `player-view` behind Spine ports (`ENG-CLK-004`..`006`, `ENG-SCN-004`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use player_view::{DiegeticSnapshot, PlayerKnowledgeArchive};
use rand_core::RngCore;
use sim_clock::{
    build_worker_pool, compute_canonical_digest, execute_parallel_sorted, in_period_bucket,
    load_snapshot_file, phase_rng, save_snapshot_file, ClockRunMode, SaveCodecError,
    SimulationClock, TickTelemetryRing, DAILY_PERIOD,
};
use sim_core::SimWorld;
use sim_planet::GeneratedPlanet;
use sim_spine::{
    ActorId, AutoPauseSignal, CanonicalSaveablePort, CellId, CommandPort, CommodityId,
    LedgerCommand, ModRegistryPort, PhaseTimingSample, PhysicalLedgerPort, PlanetGeometryPort,
    SettlementId, SpinePlayerCommand, StockpileId, SurfaceConditionsPort, Tick, TickPhase,
};
use sim_weather::WeatherEngine;

fn normalize_scenario_path(raw: &Path) -> PathBuf {
    let base = if raw.is_file() {
        raw.parent().unwrap_or(raw)
    } else {
        raw
    };
    if base.exists() {
        return base.to_path_buf();
    }
    let under_mods = Path::new("mods").join(base);
    if under_mods.exists() {
        return under_mods;
    }
    let up_mods = Path::new("../../mods").join(base);
    if up_mods.exists() {
        return up_mods;
    }
    base.to_path_buf()
}

/// Resolves an ordered mod pack stack from optional `--pack` directories and `--scenario` path.
#[must_use]
pub fn resolve_scenario_pack_stack(
    explicit_packs: &[PathBuf],
    scenario_opt: Option<&Path>,
) -> Vec<PathBuf> {
    if !explicit_packs.is_empty() {
        let mut packs = explicit_packs.to_vec();
        if let Some(sc_raw) = scenario_opt {
            let sc = normalize_scenario_path(sc_raw);
            if !packs.iter().any(|p| p == &sc) {
                packs.push(sc);
            }
        }
        return packs;
    }

    let sc_dir = match scenario_opt {
        Some(sc_raw) => normalize_scenario_path(sc_raw),
        None => normalize_scenario_path(Path::new("mods/unexplained-colony")),
    };

    let mut stack = Vec::new();
    if let Some(parent) = sc_dir.parent() {
        let core = parent.join("core-schemas");
        if core != sc_dir && core.join("pack.toml").is_file() {
            stack.push(core);
        }
        let era = parent.join("baseline-era");
        if era != sc_dir && era.join("pack.toml").is_file() {
            stack.push(era);
        }
    }
    stack.push(sc_dir);
    stack
}

/// Complete headless world uniting planetary topology, hourly weather, staggered simulation clock,
/// physical core economy/demographics, and player-view epistemic air-gap archive.
pub struct ScenarioWorld {
    pub clock: SimulationClock,
    pub planet: GeneratedPlanet,
    pub weather: WeatherEngine,
    pub core: SimWorld,
    pub archive: PlayerKnowledgeArchive,
    pub telemetry: TickTelemetryRing,
    profiling_enabled: bool,
    queued_commands: BTreeMap<u64, Vec<SpinePlayerCommand>>,
    worker_pool: Option<rayon::ThreadPool>,
}

impl ScenarioWorld {
    /// Composes and warms up a fresh deterministic world exclusively from external mod registry data.
    #[must_use]
    pub fn from_registry(registry: &dyn ModRegistryPort, world_seed: u64) -> Self {
        let planet_cfg = registry.planet_gen_config();
        let cal_cfg = registry.calendar_config();

        let planet = GeneratedPlanet::generate(world_seed, planet_cfg);
        let mut weather = WeatherEngine::new(&planet, planet_cfg, cal_cfg);
        // Deterministic 48-hour atmospheric spin-up prior to Tick(0).
        for warm in 0..48 {
            weather.step_hourly(Tick(warm));
        }

        let mut core = SimWorld::from_registry(registry, world_seed);
        core.set_planet_cell_count(planet.cell_count());

        // Relocate settlements initially pinned to cell 0 or deep ocean onto the best temperate coastal cell.
        let initial_settlements: Vec<(SettlementId, CellId)> = core
            .settlements()
            .iter()
            .map(|s| (s.id, s.cell_id))
            .collect();
        for (sid, initial_cell) in initial_settlements {
            if initial_cell.get() == 0 || planet.is_ocean_cell(initial_cell) {
                let landing = select_temperate_coastal_landing(&planet);
                core.relocate_settlement_cell(sid, landing);
            }
        }

        let mut archive = PlayerKnowledgeArchive::new();
        let _ = archive.observe_hq_halo(Tick(0), &planet, &core);

        Self {
            clock: SimulationClock::new(world_seed),
            planet,
            weather,
            core,
            archive,
            telemetry: TickTelemetryRing::new(),
            profiling_enabled: false,
            queued_commands: BTreeMap::new(),
            worker_pool: None,
        }
    }

    /// Enables or disables nanosecond subsystem telemetry collection (`ENG-CLK-006`).
    pub fn set_profiling_enabled(&mut self, enabled: bool) {
        self.profiling_enabled = enabled;
    }

    /// Configures deterministic Rayon worker pool thread count (`ENG-CLK-004`).
    pub fn set_worker_threads(&mut self, threads: usize) -> Result<(), String> {
        if threads == 0 {
            self.worker_pool = None;
            return Ok(());
        }
        let pool = build_worker_pool(threads).map_err(|e| e.to_string())?;
        self.worker_pool = Some(pool);
        Ok(())
    }

    /// Enqueues a timestamped player command executed in `TickPhase::IngestPlayerCommands`.
    pub fn enqueue_command(&mut self, target_tick: Tick, cmd: SpinePlayerCommand) {
        self.queued_commands
            .entry(target_tick.get())
            .or_default()
            .push(cmd);
    }

    /// Steps the complete 8-phase deterministic hourly pipeline forward by 1 tick.
    pub fn step_one_tick(&mut self, registry: &dyn ModRegistryPort) {
        let tick = Tick(self.clock.current_tick.get() + 1);
        self.clock.current_tick = tick;

        let t0 = self.profiling_enabled.then(Instant::now);

        // Phase 0: IngestPlayerCommands
        let cmds = self.queued_commands.remove(&tick.get()).unwrap_or_default();
        let cmd_count = cmds.len() as u32;
        for cmd in cmds {
            apply_clock_command(&mut self.clock, &cmd);
            self.core.ingest_player_command(tick, cmd);
        }
        self.core.sync_clock_tick(
            tick,
            matches!(
                self.clock.mode,
                ClockRunMode::Paused | ClockRunMode::AutoPaused { .. }
            ),
            match self.clock.mode {
                ClockRunMode::Running { speed_multiplier } => speed_multiplier,
                _ => 1,
            },
        );
        let p0_ns = elapsed_ns(t0);

        // Phase 1: HourlyWeatherAndHydrology
        let t1 = self.profiling_enabled.then(Instant::now);
        self.weather.step_hourly(tick);
        let p1_entities = self.planet.cell_count();
        let p1_ns = elapsed_ns(t1);

        // Phase 2: HourlyCombatStep
        let t2 = self.profiling_enabled.then(Instant::now);
        let p2_entities = self.core.step_hourly_combat_phase(tick);
        let p2_ns = elapsed_ns(t2);

        // Phase 3: HourlyCourierAndSignalStep (M2 courier hook)
        let t3 = self.profiling_enabled.then(Instant::now);
        let p3_entities = 0u32;
        let p3_ns = elapsed_ns(t3);

        // Phase 4: DailyBucketLogisticsAndMarkets
        let t4 = self.profiling_enabled.then(Instant::now);
        let p4_entities = self.run_parallel_logistics_bucket(tick);
        let p4_ns = elapsed_ns(t4);

        // Phase 5: DailyBucketExtractionProductionConstruction
        let t5 = self.profiling_enabled.then(Instant::now);
        let p5_entities =
            self.core
                .step_daily_production_with_weather(tick, registry, Some(&self.weather));
        let p5_ns = elapsed_ns(t5);

        // Phase 6: WeeklyMonthlyBucketDemographicsMandatesResearch
        let t6 = self.profiling_enabled.then(Instant::now);
        let p6_entities = self.core.step_weekly_demographics_phase(tick);
        let p6_ns = elapsed_ns(t6);

        // Phase 7: CompileDiegeticPlayerViewAndNotifications
        let t7 = self.profiling_enabled.then(Instant::now);
        let p7_entities = self
            .archive
            .observe_hq_halo(tick, &self.planet, &self.core)
            .len() as u32;
        let p7_ns = elapsed_ns(t7);

        if self.profiling_enabled {
            let phase_ns = [p0_ns, p1_ns, p2_ns, p3_ns, p4_ns, p5_ns, p6_ns, p7_ns];
            let total_ns = phase_ns.iter().sum();
            self.telemetry.record_sample(PhaseTimingSample {
                tick,
                phase_ns,
                total_ns,
                bucket_entities_processed: [
                    cmd_count,
                    p1_entities,
                    p2_entities,
                    p3_entities,
                    p4_entities,
                    p5_entities,
                    p6_entities,
                    p7_entities,
                ],
            });
        }
    }

    fn run_parallel_logistics_bucket(&mut self, tick: Tick) -> u32 {
        #[derive(Clone)]
        struct SettlementWork {
            id: SettlementId,
            first_lot: Option<(StockpileId, CommodityId, ActorId)>,
        }

        let world_seed = self.clock.world_seed;
        let items: Vec<SettlementWork> = self
            .core
            .settlements()
            .iter()
            .map(|s| {
                let first_lot = self
                    .core
                    .settlement_inventory_entries(s.id)
                    .into_iter()
                    .find(|e| e.mass_kg > 1.0)
                    .map(|e| (e.stockpile, e.commodity, e.owner));
                SettlementWork { id: s.id, first_lot }
            })
            .collect();

        let map_fn = |w: &SettlementWork| -> Vec<(u64, u32, LedgerCommand)> {
            let sid_u64 = u64::from(w.id.get());
            if !in_period_bucket(sid_u64, tick, DAILY_PERIOD) {
                return Vec::new();
            }
            let Some((stockpile, commodity, owner)) = w.first_lot else {
                return Vec::new();
            };
            let mut rng = phase_rng(
                world_seed,
                tick,
                TickPhase::DailyBucketLogisticsAndMarkets,
                sid_u64,
            );
            let micro_salvage_kg = ((rng.next_u64() % 5) as f64) * 0.001;
            if micro_salvage_kg <= 0.0 {
                return Vec::new();
            }
            vec![(
                sid_u64,
                0,
                LedgerCommand {
                    stockpile,
                    commodity,
                    owner,
                    delta_kg: micro_salvage_kg,
                },
            )]
        };

        let cmds = if self.worker_pool.is_some() {
            execute_parallel_sorted(self.worker_pool.as_ref(), &items, map_fn)
        } else {
            let mut tagged: Vec<(u64, u32, LedgerCommand)> =
                items.iter().flat_map(map_fn).collect();
            tagged.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            tagged.into_iter().map(|(_, _, c)| c).collect()
        };

        if !cmds.is_empty() {
            let _ = self.core.apply_ledger_commands(&cmds);
        }
        self.core.step_daily_markets_phase(tick)
    }

    /// Executes `count` consecutive ticks via `step_one_tick`.
    pub fn step_ticks(&mut self, registry: &dyn ModRegistryPort, count: u64) {
        for _ in 0..count {
            self.step_one_tick(registry);
        }
    }

    /// Advances zero or more ticks through `SimulationClock::advance_frame` honoring Pause/AutoPause/Speed multipliers.
    pub fn advance_with_clock(
        &mut self,
        registry: &dyn ModRegistryPort,
        requested_base_ticks: u64,
        mut check_auto_pause: impl FnMut(&Self, Tick) -> Option<AutoPauseSignal>,
    ) -> u64 {
        let mut clock = self.clock.clone();
        let executed = clock.advance_frame(requested_base_ticks, |next_tick| {
            self.clock.current_tick = Tick(next_tick.get() - 1);
            self.step_one_tick(registry);
            check_auto_pause(self, next_tick)
        });
        self.clock = clock;
        executed
    }

    /// Computes canonical Blake3 world state digest across clock and all four simulation cartridges (`ENG-CLK-004`, `ENG-CLK-005`).
    #[must_use]
    pub fn state_digest_bytes(&self) -> [u8; 32] {
        compute_canonical_digest(
            &self.clock,
            &[&self.planet, &self.weather, &self.core, &self.archive],
        )
    }

    /// Returns lowercase hex encoding of `state_digest_bytes`.
    #[must_use]
    pub fn state_digest_hex(&self) -> String {
        blake3::Hash::from_bytes(self.state_digest_bytes())
            .to_hex()
            .to_string()
    }

    /// Saves full world state cleanly into a compressed binary snapshot with header mod hash (`ENG-CLK-005`).
    pub fn save_to_path(
        &self,
        path: &Path,
        registry: &dyn ModRegistryPort,
    ) -> Result<(), SaveCodecError> {
        save_snapshot_file(
            path,
            registry.bundle_blake3_bytes(),
            &self.clock,
            &[&self.planet, &self.weather, &self.core, &self.archive],
        )
    }

    /// Loads a compressed binary snapshot into all world cartridges and verifies mod compatibility & digest (`ENG-CLK-005`).
    pub fn load_from_path(
        &mut self,
        path: &Path,
        registry: &dyn ModRegistryPort,
    ) -> Result<[u8; 32], SaveCodecError> {
        load_snapshot_file(
            path,
            registry.bundle_blake3_bytes(),
            &mut self.clock,
            &mut [
                &mut self.planet,
                &mut self.weather,
                &mut self.core,
                &mut self.archive,
            ],
        )
    }

    /// Compiles an IPC-safe air-gapped `DiegeticSnapshot` from the live world state.
    #[must_use]
    pub fn compile_diegetic_snapshot(&mut self, registry: &dyn ModRegistryPort) -> DiegeticSnapshot {
        let (paused, speed_multiplier, auto_pause_reason) = match self.clock.mode {
            ClockRunMode::Paused | ClockRunMode::StepSingleTick => (true, 1, None),
            ClockRunMode::Running { speed_multiplier } => (false, speed_multiplier, None),
            ClockRunMode::AutoPaused { reason_event, .. } => {
                (true, 1, Some(format!("EventClass #{}", reason_event.get())))
            }
        };
        self.archive.compile_snapshot_with_clock(
            self.clock.current_tick,
            paused,
            speed_multiplier,
            auto_pause_reason,
            registry,
            &self.planet,
            &self.weather,
            &self.core,
            &self.core,
        )
    }

    /// Validates world health invariants (finite values, water conservation, non-negative inventories).
    pub fn verify_invariants(&self) -> Result<(), String> {
        for raw in 0..self.planet.cell_count() {
            let s = self.weather.surface(CellId(raw));
            if !s.temp_c.is_finite()
                || !s.precip_mm_hr.is_finite()
                || !s.mud_index_01.is_finite()
                || !s.flood_stage_01.is_finite()
                || !s.visibility_01.is_finite()
            {
                return Err(format!("non-finite weather surface state at cell {raw}"));
            }
        }
        let err = self.weather.water_mass_balance_error_m3();
        let pool = self.weather.total_land_water_m3().max(1.0);
        if err / pool >= 1e-6 {
            return Err(format!(
                "land water mass balance drifted: err={err}, pool={pool}"
            ));
        }
        let mass = self.core.total_stockpile_mass_kg();
        if !mass.is_finite() || mass < 0.0 {
            return Err(format!("invalid total stockpile mass {mass}"));
        }
        Ok(())
    }
}

fn select_temperate_coastal_landing(planet: &GeneratedPlanet) -> CellId {
    let mut best_cell = CellId(0);
    let mut best_score = -f64::MAX;
    for raw in 0..planet.cell_count() {
        let cid = CellId(raw);
        if planet.is_ocean_cell(cid) {
            continue;
        }
        let is_coastal = planet
            .cell_neighbors(cid)
            .iter()
            .any(|&nb| planet.is_ocean_cell(nb));
        let temp = planet.cell_mean_temp_c(cid);
        let elev = planet.cell_elevation_m(cid);
        let drain = planet.cell_drainage_area_km2(cid);

        let mut score = 0.0_f64;
        if is_coastal {
            score += 500.0;
        }
        // Prefer habitable temperate zone centered around 14 C.
        score += (35.0 - f64::from((temp - 14.0).abs())) * 10.0;
        if elev <= 650.0 {
            score += 120.0;
        }
        score += f64::from(drain.log10().max(0.0)) * 25.0;

        if score > best_score {
            best_score = score;
            best_cell = cid;
        }
    }
    best_cell
}

fn apply_clock_command(clock: &mut SimulationClock, cmd: &SpinePlayerCommand) {
    match *cmd {
        SpinePlayerCommand::SetPaused(true) => {
            clock.mode = ClockRunMode::Paused;
        }
        SpinePlayerCommand::SetPaused(false) => {
            clock.mode = ClockRunMode::Running {
                speed_multiplier: 1,
            };
        }
        SpinePlayerCommand::SetSpeedMultiplier(m) => {
            clock.mode = ClockRunMode::Running {
                speed_multiplier: m.max(1),
            };
        }
        SpinePlayerCommand::StepSingleTick
        | SpinePlayerCommand::AcknowledgeNotification(_)
        | SpinePlayerCommand::StubPing { .. } => {}
    }
}

#[inline]
fn elapsed_ns(started: Option<Instant>) -> u64 {
    started.map_or(0, |t| t.elapsed().as_nanos() as u64)
}

const _CANONICAL_CLOCK_CHECK: Option<&dyn CanonicalSaveablePort> = None;
