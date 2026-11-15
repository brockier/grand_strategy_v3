#![deny(clippy::disallowed_types)]

//! World scenario composition root uniting `sim-clock`, `sim-planet`, `sim-weather`,
//! `sim-economy`, `sim-logistics`, `sim-pop`, `sim-command`, `sim-core`, and `player-view`
//! exclusively behind `sim-spine` ports (`ENG-CLK-004`..`006`, `ENG-SCN-001`..`004`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use player_view::{
    estimate_deposit_interval, perturb_reported_quantity, ArbitrationOptionDto,
    ArbitrationProposalDto, DepositEstimateDto, DiegeticDispatchDto, DiegeticSnapshot,
    DispatchTokenDto, EdgeViewInput, FacilityRowDto, HqRelocationPreviewDto, MandateSummaryDto,
    NotificationRuleDto, ObservedWeatherDto, PlayerKnowledgeArchive, PopulationCohortRowDto,
    ReportedLedgerRowDto, SnapshotSubsystemInputs, TimeSeriesPointDto,
};
use sim_clock::{
    build_worker_pool, compute_canonical_digest, execute_parallel_sorted, in_period_bucket,
    load_snapshot_file, save_snapshot_file, ClockRunMode, SaveCodecError, SimulationClock,
    TickTelemetryRing, DAILY_PERIOD,
};
use sim_command::CommandHierarchy;
use sim_core::SimWorld;
use sim_economy::EconomyLedger;
use sim_logistics::{LogisticsNetwork, UnifiedTransitLaunch};
use sim_planet::GeneratedPlanet;
use sim_pop::PopulationRegistry;
use sim_spine::{
    AutoPauseSignal, CanonicalSaveablePort, CellId, CharacterId, CommandPort, MandateId,
    ModRegistryPort, NotificationBehaviorKind, PhaseTimingSample, PhysicalLedgerPort,
    PlanetGeometryPort, PopulationPort, ScenarioArchetypeKind, SettlementId, SpinePlayerCommand,
    SubsurfaceDepositTruthPort, SurfaceConditionsPort, Tick, TransitChannelKind, UnrestStage,
    VocationId,
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

/// Complete deterministic world uniting physical simulation cartridges and `player-view` archive.
pub struct ScenarioWorld {
    pub clock: SimulationClock,
    pub planet: GeneratedPlanet,
    pub weather: WeatherEngine,
    pub economy: EconomyLedger,
    pub logistics: LogisticsNetwork,
    pub pop: PopulationRegistry,
    pub command: CommandHierarchy,
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
        Self::from_registry_with_archetype(registry, world_seed, None)
    }

    /// Composes a fresh deterministic world overriding the starting scenario archetype (`ENG-SCN-001`).
    #[must_use]
    pub fn from_registry_with_archetype(
        registry: &dyn ModRegistryPort,
        world_seed: u64,
        override_archetype: Option<ScenarioArchetypeKind>,
    ) -> Self {
        let scenario = registry.scenario_init();
        let archetype = override_archetype.unwrap_or(scenario.archetype);
        let planet_cfg = registry.planet_gen_config();
        let cal_cfg = registry.calendar_config();

        let planet = GeneratedPlanet::generate(world_seed, planet_cfg);
        let mut weather = WeatherEngine::new(&planet, planet_cfg, cal_cfg);
        // Deterministic 48-hour atmospheric spin-up prior to Tick(0).
        for warm in 0..48 {
            weather.step_hourly(Tick(warm));
        }

        let mut economy = EconomyLedger::from_registry(registry);
        let mut core = SimWorld::from_registry(registry, world_seed);
        core.set_planet_cell_count(planet.cell_count());

        let initial_settlements: Vec<(SettlementId, CellId)> = economy
            .settlements()
            .iter()
            .map(|s| (s.id, s.cell_id))
            .collect();
        for (sid, initial_cell) in initial_settlements {
            if initial_cell.get() == 0 || planet.is_ocean_cell(initial_cell) {
                let landing = select_temperate_coastal_landing(&planet);
                economy.relocate_settlement_cell(sid, landing);
                core.relocate_settlement_cell(sid, landing);
            }
        }

        let mut logistics = LogisticsNetwork::from_planet_and_registry(&planet, registry);

        let initial_skill = match archetype {
            ScenarioArchetypeKind::EmergentDawn => 0.22,
            ScenarioArchetypeKind::ExpeditionColony => 0.58,
            ScenarioArchetypeKind::GovernorSuccessor => 0.68,
        };
        let mut pop = PopulationRegistry::new();
        let mut settlement_by_key = BTreeMap::new();
        for (idx, seed) in scenario.settlements.iter().enumerate() {
            let sid = SettlementId(idx as u32);
            settlement_by_key.insert(seed.key.clone(), sid);
            let mut specs = Vec::with_capacity(seed.cohorts.len());
            for (c_idx, &(voc, cnt)) in seed.cohorts.iter().enumerate() {
                let arch_idx = seed.cohort_archetype_indices.get(c_idx).copied().unwrap_or(0);
                specs.push((voc, cnt, arch_idx));
            }
            if specs.is_empty() && seed.initial_population > 0 {
                specs.push((VocationId(0), seed.initial_population, 0));
            }
            pop.seed_settlement_cohorts(sid, &specs, registry, "landing_colonists", initial_skill);
        }

        for ch in &scenario.characters {
            let home_sid = settlement_by_key
                .get(&ch.home_settlement_key)
                .copied()
                .unwrap_or(SettlementId(0));
            pop.seed_character_from_scenario(ch, home_sid);
        }

        let mut default_rules = BTreeMap::new();
        for (_, key, _, behavior) in registry.notification_classes() {
            default_rules.insert(key, behavior);
        }

        let (attn_cap, overhead) = match archetype {
            ScenarioArchetypeKind::EmergentDawn => (
                (scenario.governance_base_attention * 0.78).max(60.0),
                scenario.clerical_overhead_per_mandate,
            ),
            ScenarioArchetypeKind::ExpeditionColony => (
                scenario.governance_base_attention,
                scenario.clerical_overhead_per_mandate,
            ),
            ScenarioArchetypeKind::GovernorSuccessor => (
                scenario.governance_base_attention * 1.15,
                scenario.clerical_overhead_per_mandate,
            ),
        };
        let mut command = CommandHierarchy::with_config(
            attn_cap,
            overhead,
            scenario.hierarchy_hop_latency_hours,
            default_rules,
        );
        let settlement_cells = Self::settlement_cell_map(&economy);
        command.sync_character_locations(pop.all_characters(), &settlement_cells);

        let mut archive = PlayerKnowledgeArchive::new();
        match archetype {
            ScenarioArchetypeKind::EmergentDawn => {
                economy.scale_all_stockpiles_for_archetype(0.35);
                economy.set_hq_sight_radius_cells(1);
            }
            ScenarioArchetypeKind::ExpeditionColony => {
                // Single coastal landing perimeter with surrounding wilderness unexplored (`ENG-SCN-002`).
            }
            ScenarioArchetypeKind::GovernorSuccessor => {
                economy.set_hq_sight_radius_cells(2);
                let hq = economy.hq_cell();
                // Pre-seed regional road infrastructure & stale surveys across radius 3 (`ENG-SCN-001`).
                let metalled = registry.infra_id_by_key("metalled_road");
                let mut ring = BTreeMap::new();
                ring.insert(hq, 0u8);
                let mut frontier = vec![hq];
                while let Some(cur) = frontier.pop() {
                    let dist = ring[&cur];
                    if dist >= 3 {
                        continue;
                    }
                    for &nb in planet.cell_neighbors(cur) {
                        if let Some(eid) = planet.cell_edge(cur, nb)
                            && let Some(m_id) = metalled
                            && !planet.is_ocean_cell(cur)
                            && !planet.is_ocean_cell(nb)
                        {
                            logistics.upgrade_edge_infra(eid, m_id, None);
                        }
                        if let std::collections::btree_map::Entry::Vacant(e) = ring.entry(nb) {
                            e.insert(dist + 1);
                            frontier.push(nb);
                        }
                    }
                }
                for (cid, dist) in ring {
                    if dist >= 2 && !planet.is_ocean_cell(cid) {
                        archive.seed_historical_stale_cell(
                            cid.get(),
                            Tick(0),
                            &planet,
                            "Provincial Charter Survey (Stale)".to_owned(),
                        );
                    }
                }
            }
        }

        let _ = archive.observe_hq_halo(Tick(0), &planet, &economy);
        Self::record_daily_time_series(Tick(0), registry, &economy, &pop, &command, &mut archive);

        Self {
            clock: SimulationClock::new(world_seed),
            planet,
            weather,
            economy,
            logistics,
            pop,
            command,
            core,
            archive,
            telemetry: TickTelemetryRing::new(),
            profiling_enabled: false,
            queued_commands: BTreeMap::new(),
            worker_pool: None,
        }
    }

    fn settlement_cell_map(economy: &EconomyLedger) -> BTreeMap<SettlementId, CellId> {
        economy
            .settlements()
            .iter()
            .map(|s| (s.id, s.cell_id))
            .collect()
    }

    fn trigger_event_notification(
        clock: &mut SimulationClock,
        registry: &dyn ModRegistryPort,
        command: &CommandHierarchy,
        tick: Tick,
        event_class_key: &str,
    ) -> bool {
        let Some(behavior) = command.notification_rules().get(event_class_key).copied() else {
            return false;
        };
        if behavior == NotificationBehaviorKind::DispatchPopupAndAutoPause
            && let Some(eid) = registry.event_class_id(event_class_key)
        {
            clock.mode = ClockRunMode::AutoPaused {
                reason_event: eid,
                tick,
            };
            return true;
        }
        false
    }

    fn record_daily_time_series(
        tick: Tick,
        registry: &dyn ModRegistryPort,
        economy: &EconomyLedger,
        pop: &PopulationRegistry,
        command: &CommandHierarchy,
        archive: &mut PlayerKnowledgeArchive,
    ) {
        let hq_sid = economy.hq_settlement().unwrap_or(SettlementId(0));
        let grain_val = registry
            .commodity_id_by_key("raw_grain")
            .map(|cid| economy.settlement_commodity_mass_kg(hq_sid, cid, None))
            .unwrap_or(0.0);
        let iron_val = registry
            .commodity_id_by_key("wrought_iron")
            .map(|cid| economy.settlement_commodity_mass_kg(hq_sid, cid, None))
            .unwrap_or(0.0);
        let total_pop = f64::from(pop.total_population());
        let attn = command.attention_summary(CharacterId(0));
        let attn_pct = if attn.capacity_attention > 0.0 {
            f64::from((attn.used_attention / attn.capacity_attention) * 100.0)
        } else {
            0.0
        };

        archive.record_time_series_sample(
            "grain_stock",
            TimeSeriesPointDto {
                tick: tick.get(),
                value: grain_val,
                min_bound: grain_val * 0.96,
                max_bound: grain_val * 1.04,
                is_confirmed: true,
            },
        );
        archive.record_time_series_sample(
            "iron_stock",
            TimeSeriesPointDto {
                tick: tick.get(),
                value: iron_val,
                min_bound: iron_val * 0.95,
                max_bound: iron_val * 1.05,
                is_confirmed: true,
            },
        );
        archive.record_time_series_sample(
            "population_total",
            TimeSeriesPointDto {
                tick: tick.get(),
                value: total_pop,
                min_bound: (total_pop - 2.0).max(0.0),
                max_bound: total_pop + 2.0,
                is_confirmed: true,
            },
        );
        archive.record_time_series_sample(
            "attention_load",
            TimeSeriesPointDto {
                tick: tick.get(),
                value: attn_pct,
                min_bound: attn_pct,
                max_bound: attn_pct,
                is_confirmed: true,
            },
        );
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
        let ready_keys: Vec<u64> = self
            .queued_commands
            .range(..=tick.get())
            .map(|(&k, _)| k)
            .collect();
        let mut cmds = Vec::new();
        for k in ready_keys {
            if let Some(mut v) = self.queued_commands.remove(&k) {
                cmds.append(&mut v);
            }
        }
        let cmd_count = cmds.len() as u32;
        let settlement_cells = Self::settlement_cell_map(&self.economy);
        let grain_id = registry.commodity_id_by_key("raw_grain");

        for cmd in cmds {
            apply_clock_command(&mut self.clock, &cmd);
            self.core.ingest_player_command(tick, cmd.clone());
            self.command.apply_player_command(
                tick,
                cmd,
                self.pop.all_characters(),
                &settlement_cells,
            );
        }

        // Launch HQ physical relocation retinue when requested via `RelocateHq` (`ENG-EPI-003`)
        for world_act in self.command.drain_world_actions() {
            if let SpinePlayerCommand::RelocateHq { destination_cell } = world_act.command {
                let cur_hq_cell = self.economy.hq_cell();
                if cur_hq_cell == destination_cell {
                    self.economy.set_hq_location(destination_cell);
                } else {
                    let origin_sid = self.economy.hq_settlement().unwrap_or(SettlementId(0));
                    let carrier_id = self.logistics.default_freight_carrier();
                    let est_hours = self
                        .logistics
                        .plan_route(
                            cur_hq_cell,
                            destination_cell,
                            carrier_id,
                            0.0,
                            &self.planet,
                            &self.weather,
                            registry,
                        )
                        .map(|(_, h)| h)
                        .unwrap_or(240.0);
                    let burn_hr = registry
                        .carrier_def(carrier_id)
                        .map(|c| c.fuel_burn_kg_per_hour)
                        .unwrap_or(0.6);
                    let required_fodder = (est_hours * burn_hr * 1.75).max(45.0);
                    let fodder_kg = grain_id
                        .map(|cid| {
                            self.economy
                                .load_fuel_into_transit(origin_sid, cid, required_fodder)
                        })
                        .unwrap_or(0.0);
                    let _ = self.logistics.launch_unified_transit(
                        UnifiedTransitLaunch {
                            label: format!("HQ Retinue -> Cell {}", destination_cell.get()),
                            channel: TransitChannelKind::HqRetinue,
                            origin_settlement: origin_sid,
                            destination_settlement: self
                                .economy
                                .settlements()
                                .iter()
                                .find(|s| s.cell_id == destination_cell)
                                .map(|s| s.id),
                            origin_cell: cur_hq_cell,
                            destination_cell,
                            carrier_class: carrier_id,
                            owner: sim_spine::ActorId(1),
                            cargo_commodity: None,
                            cargo_mass_kg: 0.0,
                            cargo_purity_01: 1.0,
                            fuel_loaded_kg: fodder_kg,
                            spare_parts_loaded_kg: 0.0,
                            current_tick: tick,
                            is_hq_move: true,
                            dispatch_id: None,
                            sender: None,
                            mandate_payload_id: None,
                            subject: String::new(),
                        },
                        &self.planet,
                        &self.weather,
                        registry,
                    );
                }
            }
        }

        // Queue physical courier riders for every newly issued command mandate (`ENG-CMD-003`)
        for order in self.command.drain_pending_order_couriers() {
            let origin_sid = self.economy.hq_settlement().unwrap_or(SettlementId(0));
            let origin_cell = self.economy.hq_cell();
            let courier_carrier = self.logistics.default_courier_carrier();
            let fodder_kg = grain_id
                .map(|cid| self.economy.load_fuel_into_transit(origin_sid, cid, 4.0))
                .unwrap_or(0.0);
            let _ = self.logistics.launch_unified_transit(
                UnifiedTransitLaunch {
                    label: format!("Order Courier M#{}", order.mandate_id.0),
                    channel: TransitChannelKind::PhysicalCourier,
                    origin_settlement: origin_sid,
                    destination_settlement: Some(order.destination_settlement),
                    origin_cell,
                    destination_cell: order.destination_cell,
                    carrier_class: courier_carrier,
                    owner: sim_spine::ActorId(1),
                    cargo_commodity: None,
                    cargo_mass_kg: 0.0,
                    cargo_purity_01: 1.0,
                    fuel_loaded_kg: fodder_kg,
                    spare_parts_loaded_kg: 0.0,
                    current_tick: tick,
                    is_hq_move: false,
                    dispatch_id: None,
                    sender: Some(order.assignee),
                    mandate_payload_id: Some(order.mandate_id),
                    subject: order.subject,
                },
                &self.planet,
                &self.weather,
                registry,
            );
        }

        // Apply consequences of resolved arbitration choices cleanly across population welfare
        for resolved in self.command.drain_applied_resolutions() {
            if resolved.welfare_relief_delta.abs() > 1e-6 {
                self.pop
                    .adjust_settlement_grievance(resolved.settlement, -resolved.welfare_relief_delta);
            }
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
        self.economy
            .apply_hourly_weather_and_spoilage(registry, &self.weather);
        let p1_entities = self.planet.cell_count();
        let p1_ns = elapsed_ns(t1);

        // Phase 2: HourlyCombatStep
        let t2 = self.profiling_enabled.then(Instant::now);
        let p2_entities = self.core.step_hourly_combat_phase(tick);
        let p2_ns = elapsed_ns(t2);

        // Phase 3: HourlyCourierAndSignalStep
        let t3 = self.profiling_enabled.then(Instant::now);
        let log_summary =
            self.logistics
                .step_hourly(tick, &self.planet, &self.weather, registry);
        self.economy.record_onboard_transit_burn(
            log_summary.total_fuel_burned_kg
                + log_summary.total_spares_worn_kg
                + log_summary.unused_courier_supplies_kg,
        );

        let mut p3_entities = 0u32;
        for delivery in log_summary.arrived_cargo {
            p3_entities += 1;
            let carrier_fuel_comm = registry
                .carrier_def(delivery.carrier_class)
                .map(|c| c.fodder_or_fuel_commodity);
            let carrier_spares_comm = registry
                .carrier_def(delivery.carrier_class)
                .and_then(|c| c.spare_parts_commodity);

            if let Some(dest_sid) = delivery.destination_settlement {
                if let Some(comm) = delivery.commodity
                    && delivery.mass_kg > 0.0
                {
                    self.economy.credit_cargo_arrival(
                        dest_sid,
                        comm,
                        delivery.owner,
                        delivery.mass_kg,
                        delivery.purity_01,
                    );
                }
                if delivery.unused_fuel_kg > 0.0
                    && let Some(fc) = carrier_fuel_comm
                {
                    self.economy.credit_cargo_arrival(
                        dest_sid,
                        fc,
                        delivery.owner,
                        delivery.unused_fuel_kg,
                        0.95,
                    );
                }
                if delivery.unused_spares_kg > 0.0
                    && let Some(sc) = carrier_spares_comm
                {
                    self.economy.credit_cargo_arrival(
                        dest_sid,
                        sc,
                        delivery.owner,
                        delivery.unused_spares_kg,
                        0.95,
                    );
                }
            } else {
                // Wilderness stop (survey site or wilderness camp HQ): unused fuel/spares are consumed/carried cleanly
                self.economy.record_onboard_transit_burn(
                    delivery.mass_kg + delivery.unused_fuel_kg + delivery.unused_spares_kg,
                );
            }

            if delivery.is_hq_move {
                self.economy.set_hq_location(delivery.destination_cell);
                let _ = self.archive.observe_hq_halo(tick, &self.planet, &self.economy);
            }

            if let Some(survey_cell) = delivery.survey_target_cell {
                let char_view = self
                    .pop
                    .all_characters()
                    .first()
                    .cloned()
                    .unwrap_or_else(|| sim_spine::CharacterSnapshotView {
                        id: CharacterId(1),
                        key: "scout".to_owned(),
                        display_name: "Field Surveyor".to_owned(),
                        role_title: "Warden".to_owned(),
                        home_settlement: delivery.origin_settlement,
                        competence: 0.72,
                        precision_01: 0.80,
                        honesty_vs_sycophancy: 0.65,
                        caution_vs_alarmism: 0.2,
                    });
                let skill_pct = (char_view.precision_01 * 100.0).round() as u32;
                let hq_cell = self.economy.hq_cell();
                let courier_carrier = self.logistics.default_courier_carrier();
                let _ = self.logistics.launch_unified_transit(
                    UnifiedTransitLaunch {
                        label: format!("Survey Assay Cell {}", survey_cell.get()),
                        channel: TransitChannelKind::PhysicalCourier,
                        origin_settlement: delivery.origin_settlement,
                        destination_settlement: self.economy.hq_settlement(),
                        origin_cell: survey_cell,
                        destination_cell: hq_cell,
                        carrier_class: courier_carrier,
                        owner: sim_spine::ActorId(1),
                        cargo_commodity: None,
                        cargo_mass_kg: 0.0,
                        cargo_purity_01: 1.0,
                        fuel_loaded_kg: 0.0,
                        spare_parts_loaded_kg: 0.0,
                        current_tick: tick,
                        is_hq_move: false,
                        dispatch_id: None,
                        sender: Some(char_view.id),
                        mandate_payload_id: None,
                        subject: format!(
                            "survey_report:{}:{}:{}",
                            survey_cell.get(),
                            char_view.id.0,
                            skill_pct
                        ),
                    },
                    &self.planet,
                    &self.weather,
                    registry,
                );
            }
        }

        // Deliver physical courier envelopes that reached their destination cell
        for env in self.logistics.drain_arrived_couriers() {
            p3_entities += 1;
            if let Some(rest) = env.subject.strip_prefix("directive:") {
                let mut parts = rest.splitn(2, ':');
                if let Some(id_str) = parts.next()
                    && let Ok(mid_num) = id_str.parse::<u64>()
                {
                    let mid = MandateId(mid_num);
                    if self.command.mark_order_delivered(mid, tick) {
                        let m_opt = self
                            .command
                            .all_mandates()
                            .iter()
                            .find(|m| m.id == mid)
                            .cloned();
                        if let Some(m) = m_opt
                            && m.objective_kind == "geological_survey"
                        {
                            let start_cell = self
                                .economy
                                .settlements()
                                .iter()
                                .find(|s| s.id == m.target_settlement)
                                .map(|s| s.cell_id)
                                .unwrap_or_else(|| self.economy.hq_cell());
                            let survey_carrier = self.logistics.default_freight_carrier();
                            let est_hours = self
                                .logistics
                                .plan_route(
                                    start_cell,
                                    m.target_cell,
                                    survey_carrier,
                                    0.0,
                                    &self.planet,
                                    &self.weather,
                                    registry,
                                )
                                .map(|(_, h)| h)
                                .unwrap_or(240.0);
                            let burn_hr = registry
                                .carrier_def(survey_carrier)
                                .map(|c| c.fuel_burn_kg_per_hour)
                                .unwrap_or(0.6);
                            let required_fodder = (est_hours * burn_hr * 1.75).max(24.0);
                            let fodder = grain_id
                                .map(|cid| {
                                    self.economy
                                        .load_fuel_into_transit(m.target_settlement, cid, required_fodder)
                                })
                                .unwrap_or(0.0);
                            let _ = self.logistics.launch_unified_transit(
                                UnifiedTransitLaunch {
                                    label: format!("Survey Party -> Cell {}", m.target_cell.get()),
                                    channel: TransitChannelKind::SurveyExpedition,
                                    origin_settlement: m.target_settlement,
                                    destination_settlement: None,
                                    origin_cell: start_cell,
                                    destination_cell: m.target_cell,
                                    carrier_class: survey_carrier,
                                    owner: sim_spine::ActorId(1),
                                    cargo_commodity: None,
                                    cargo_mass_kg: 0.0,
                                    cargo_purity_01: 1.0,
                                    fuel_loaded_kg: fodder,
                                    spare_parts_loaded_kg: 0.0,
                                    current_tick: tick,
                                    is_hq_move: false,
                                    dispatch_id: None,
                                    sender: Some(m.assignee),
                                    mandate_payload_id: Some(mid),
                                    subject: String::new(),
                                },
                                &self.planet,
                                &self.weather,
                                registry,
                            );
                        }
                    }
                }
            } else if let Some(rest) = env.subject.strip_prefix("survey_report:") {
                let parts: Vec<&str> = rest.split(':').collect();
                if parts.len() >= 3
                    && let (Ok(cell_u32), Ok(char_u32), Ok(skill_pct)) = (
                        parts[0].parse::<u32>(),
                        parts[1].parse::<u32>(),
                        parts[2].parse::<u32>(),
                    )
                {
                    let cid = CellId(cell_u32);
                    let skill = (skill_pct as f32 / 100.0).clamp(0.1, 1.0);
                    let deposits = self.planet.deposits_in_cell(cid);
                    let mut ests = Vec::with_capacity(deposits.len());
                    for d in deposits {
                        let (min_t, pt_t, max_t, pur_e, conf) = estimate_deposit_interval(
                            self.clock.world_seed,
                            tick.get(),
                            d.material_id.0,
                            d.total_mass_tonnes,
                            d.average_purity_01,
                            skill,
                            1,
                        );
                        ests.push(DepositEstimateDto {
                            material_id: d.material_id.0,
                            material_label: format!("{:?} Seam #{}", d.category, d.material_id.0),
                            category_key: format!("{:?}", d.category),
                            min_tonnes: min_t,
                            point_est_tonnes: pt_t,
                            max_tonnes: max_t,
                            purity_est_01: pur_e,
                            confidence_01: conf,
                            surveyor_skill_01: skill,
                            instrument_tier: 1,
                            observed_tick: tick.get(),
                            provenance_label: format!(
                                "Assay Report (Tick {}, Skill {:.0}%)",
                                tick.get(),
                                skill * 100.0
                            ),
                        });
                    }
                    let char_name = self
                        .pop
                        .character_view(CharacterId(char_u32))
                        .map(|c| c.display_name)
                        .unwrap_or_else(|| "Survey Warden".to_owned());
                    self.archive.ingest_survey_report(
                        cell_u32,
                        tick,
                        &self.planet,
                        ests.clone(),
                        format!("Surveyed by {char_name} (Tick {})", tick.get()),
                    );
                    self.archive.record_arrived_dispatch(DiegeticDispatchDto {
                        dispatch_id: env.dispatch_id.0,
                        title: format!("Geological Assay: Sector Cell #{cell_u32}"),
                        sent_tick: env.sent_tick.get(),
                        arrived_tick: tick.get(),
                        origin_label: format!("Sector Cell #{cell_u32}"),
                        sender_id: char_u32,
                        sender_name: char_name.clone(),
                        origin_cell_id: cell_u32,
                        event_class_key: "mandate_issued_log".to_owned(),
                        severity: "DispatchPopup".to_owned(),
                        confidence_01: ests.first().map(|e| e.confidence_01).unwrap_or(0.84),
                        body_tokens: vec![
                            DispatchTokenDto {
                                text: char_name,
                                entity_kind: Some("character".to_owned()),
                                entity_id: Some(u64::from(char_u32)),
                                target_cell_id: Some(cell_u32),
                            },
                            DispatchTokenDto {
                                text: " reports strata survey at ".to_owned(),
                                entity_kind: None,
                                entity_id: None,
                                target_cell_id: None,
                            },
                            DispatchTokenDto {
                                text: format!("Cell #{cell_u32}"),
                                entity_kind: Some("cell".to_owned()),
                                entity_id: Some(u64::from(cell_u32)),
                                target_cell_id: Some(cell_u32),
                            },
                        ],
                    });
                }
            } else if let Some(rest) = env.subject.strip_prefix("status_report:") {
                let parts: Vec<&str> = rest.split(':').collect();
                if parts.len() >= 2
                    && let (Ok(sid_u32), Ok(char_u32)) =
                        (parts[0].parse::<u32>(), parts[1].parse::<u32>())
                {
                    let sid = SettlementId(sid_u32);
                    if let Some(s_sum) = self
                        .economy
                        .settlements()
                        .iter()
                        .find(|s| s.id == sid)
                        .cloned()
                    {
                        let reporter_full = self.pop.character_full(CharacterId(char_u32)).cloned();
                        let reporter_view = self
                            .pop
                            .character_view(CharacterId(char_u32))
                            .unwrap_or_else(|| sim_spine::CharacterSnapshotView {
                                id: CharacterId(char_u32),
                                key: "clerk".to_owned(),
                                display_name: "District Steward".to_owned(),
                                role_title: "Clerk".to_owned(),
                                home_settlement: sid,
                                competence: 0.65,
                                precision_01: 0.72,
                                honesty_vs_sycophancy: 0.4,
                                caution_vs_alarmism: 0.0,
                            });
                        let skimming = reporter_full
                            .as_ref()
                            .map(|f| f.greed_skimming_01)
                            .unwrap_or(0.0);
                        for entry in self.economy.settlement_inventory_entries(sid) {
                            let (rep, min_e, max_e, prov) = perturb_reported_quantity(
                                self.clock.world_seed,
                                tick.get(),
                                sid_u32,
                                entry.commodity.0,
                                entry.mass_kg,
                                &reporter_view,
                                skimming,
                            );
                            let (c_key, c_name, c_cat) = registry
                                .commodity_def(entry.commodity)
                                .map(|c| (c.key.clone(), c.display_name.clone(), c.category.clone()))
                                .unwrap_or_default();
                            self.archive.record_remote_ledger_report(ReportedLedgerRowDto {
                                settlement_id: sid_u32,
                                settlement_name: s_sum.display_name.clone(),
                                commodity_key: c_key,
                                commodity_name: c_name,
                                reported_mass_kg: rep,
                                min_est_kg: min_e,
                                max_est_kg: max_e,
                                observed_tick: env.sent_tick.get(),
                                provenance_label: prov,
                                category: c_cat,
                                confidence_01: reporter_view.precision_01,
                                reporter_name: reporter_view.display_name.clone(),
                            });
                        }
                        let st = self.weather.surface(s_sum.cell_id);
                        self.archive.record_remote_weather_report(ObservedWeatherDto {
                            cell_id: s_sum.cell_id.get(),
                            observed_tick: env.sent_tick.get(),
                            precip_mm_hr: st.precip_mm_hr,
                            mud_index_01: st.mud_index_01,
                            temp_c: st.temp_c,
                            visibility_01: st.visibility_01,
                            flood_stage_01: st.flood_stage_01,
                            snow_depth_m: st.snow_depth_m,
                            ice_thickness_m: st.ice_thickness_m,
                            wind_u_mps: st.wind_u_mps,
                            wind_v_mps: st.wind_v_mps,
                        });
                        for &cid in self.pop.settlement_cohorts(sid) {
                            if let Some(cv) = self.pop.cohort_view(cid) {
                                let (vk, vn) = registry
                                    .vocation_def(cv.vocation)
                                    .map(|v| (v.key.clone(), v.display_name.clone()))
                                    .unwrap_or_default();
                                self.archive.record_remote_population_report(
                                    PopulationCohortRowDto {
                                        cohort_id: cv.id.0,
                                        settlement_id: sid_u32,
                                        settlement_name: s_sum.display_name.clone(),
                                        vocation_key: vk,
                                        vocation_name: vn,
                                        reported_headcount: cv.headcount,
                                        min_headcount: cv.headcount.saturating_sub(3),
                                        max_headcount: cv.headcount + 3,
                                        mean_skill_01: cv.mean_skill_01,
                                        literacy_01: 0.70,
                                        nutrition_01: 0.85,
                                        unrest_stage: format!(
                                            "{:?}",
                                            self.pop.unrest_stage_for_settlement(sid)
                                        ),
                                        observed_tick: env.sent_tick.get(),
                                        confidence_01: reporter_view.precision_01,
                                        provenance_label: format!(
                                            "Courier from {}",
                                            reporter_view.display_name
                                        ),
                                    },
                                );
                            }
                        }
                        self.archive.record_arrived_dispatch(DiegeticDispatchDto {
                            dispatch_id: env.dispatch_id.0,
                            title: format!("Situation Report: {}", s_sum.display_name),
                            sent_tick: env.sent_tick.get(),
                            arrived_tick: tick.get(),
                            origin_label: s_sum.display_name.clone(),
                            sender_id: char_u32,
                            sender_name: reporter_view.display_name.clone(),
                            origin_cell_id: s_sum.cell_id.get(),
                            event_class_key: "mandate_issued_log".to_owned(),
                            severity: "LogOnly".to_owned(),
                            confidence_01: reporter_view.precision_01,
                            body_tokens: vec![
                                DispatchTokenDto {
                                    text: reporter_view.display_name,
                                    entity_kind: Some("character".to_owned()),
                                    entity_id: Some(u64::from(char_u32)),
                                    target_cell_id: Some(s_sum.cell_id.get()),
                                },
                                DispatchTokenDto {
                                    text: " submitted periodic ledgers for ".to_owned(),
                                    entity_kind: None,
                                    entity_id: None,
                                    target_cell_id: None,
                                },
                                DispatchTokenDto {
                                    text: s_sum.display_name,
                                    entity_kind: Some("settlement".to_owned()),
                                    entity_id: Some(u64::from(sid_u32)),
                                    target_cell_id: Some(s_sum.cell_id.get()),
                                },
                            ],
                        });
                    }
                }
            }
        }
        let p3_ns = elapsed_ns(t3);

        // Phase 4: DailyBucketLogisticsAndMarkets (flush Spine shipments & dispatch remote status couriers)
        let t4 = self.profiling_enabled.then(Instant::now);
        let s_cells = Self::settlement_cell_map(&self.economy);
        self.logistics
            .flush_pending_port_shipments(&s_cells, tick, &self.planet, &self.weather, registry);
        let p4_entities = self.run_parallel_logistics_and_dispatches(tick, registry);
        let p4_ns = elapsed_ns(t4);

        // Phase 5: DailyBucketExtractionProductionConstruction (hourly subsistence + daily facility cycle)
        let t5 = self.profiling_enabled.then(Instant::now);
        let mut p5_entities = 0u32;
        let settlement_list: Vec<(SettlementId, CellId, String)> = self
            .economy
            .settlements()
            .iter()
            .map(|s| (s.id, s.cell_id, s.display_name.clone()))
            .collect();
        let is_daily_tick = tick.get().is_multiple_of(24);
        let mut subsistence_by_sid = BTreeMap::new();
        for &(sid, cell_id, _) in &settlement_list {
            let ambient_c = self.weather.surface(cell_id).temp_c;
            let needs = self
                .pop
                .compute_settlement_hourly_needs(sid, ambient_c, registry);
            let sat = self.economy.step_subsistence_for_settlement(
                sid,
                needs.grain_need_kg,
                needs.water_need_kg,
                needs.fuel_need_below_10c_kg,
                needs.clothing_need_kg,
                ambient_c,
                registry,
            );
            subsistence_by_sid.insert(sid, (sat, ambient_c));
        }

        if is_daily_tick {
            let mut attn_map = BTreeMap::new();
            let mut skim_map = BTreeMap::new();
            for fac in self.economy.facilities() {
                let over_mult = self.command.settlement_oversight_multiplier(fac.settlement);
                attn_map.insert(fac.id, over_mult);
                let skim = self
                    .pop
                    .all_characters_full()
                    .iter()
                    .find(|c| c.snapshot.home_settlement == fac.settlement)
                    .map(|c| c.greed_skimming_01 * (1.0 - c.loyalty_01 * 0.5))
                    .unwrap_or(0.0);
                skim_map.insert(fac.id, skim);
            }
            for &(sid, _, _) in &settlement_list {
                let labor_mult = self.pop.labor_efficiency_multiplier(sid);
                p5_entities += self.economy.step_facilities_for_settlement(
                    sid,
                    registry,
                    &self.weather,
                    labor_mult,
                    &attn_map,
                    &skim_map,
                    Some(&self.planet),
                );
            }
        }
        let p5_ns = elapsed_ns(t5);

        // Phase 6: WeeklyMonthlyBucketDemographicsMandatesResearch
        let t6 = self.profiling_enabled.then(Instant::now);
        let mut p6_entities = 0u32;
        for (sid, cell_id, s_name) in &settlement_list {
            let (sat, ambient_c) = subsistence_by_sid[sid];
            let pop_out = self.pop.step_settlement_hourly(
                *sid,
                tick,
                sat.calorie_ratio_01,
                sat.water_ratio_01,
                sat.heating_ratio_01,
                sat.clothing_ratio_01,
                ambient_c,
                registry,
            );
            p6_entities += 1;

            // Check unrest escalation or mine flooding bottlenecks (`ENG-CMD-004`, `ENG-SCN-003`)
            let shaft_water = self.economy.settlement_shaft_water_level(*sid);
            let char_id = self
                .pop
                .all_characters()
                .iter()
                .find(|c| c.home_settlement == *sid)
                .map(|c| c.id)
                .unwrap_or(CharacterId(1));
            if shaft_water >= 0.55 {
                let _ = self.command.escalate_bottleneck(
                    *sid,
                    *cell_id,
                    char_id,
                    "mine_flooding_halt".to_owned(),
                    format!("Mine Shaft Flooding at {s_name}"),
                    "Groundwater seepage reached >= 55%; choose sump reinforcement vs emergency pumping."
                        .to_owned(),
                    tick,
                    vec![
                        sim_command::ArbitrationOption {
                            label: "Divert timber to sump shoring".to_owned(),
                            description: "Raise drainage priority weight and calm workers."
                                .to_owned(),
                            attention_cost: 6.0,
                            priority_boost: 2,
                            welfare_relief_delta: 0.08,
                        },
                        sim_command::ArbitrationOption {
                            label: "Order double pump shifts".to_owned(),
                            description: "Push extraction crews hard with zero relief."
                                .to_owned(),
                            attention_cost: 4.0,
                            priority_boost: 1,
                            welfare_relief_delta: -0.05,
                        },
                    ],
                );
                let _ = Self::trigger_event_notification(
                    &mut self.clock,
                    registry,
                    &self.command,
                    tick,
                    "mine_flooding_halt",
                );
            }
            if pop_out.previous_unrest_stage < UnrestStage::LaborStrike
                && pop_out.unrest_stage >= UnrestStage::LaborStrike
            {
                let _ = self.command.escalate_bottleneck(
                    *sid,
                    *cell_id,
                    char_id,
                    "unrest_escalation".to_owned(),
                    format!("Labor Strike Erupting at {s_name}"),
                    "Subsistence or taxation grievance crossed strike threshold.".to_owned(),
                    tick,
                    vec![
                        sim_command::ArbitrationOption {
                            label: "Issue emergency grain & relief".to_owned(),
                            description: "Grant concessions reducing settlement grievance."
                                .to_owned(),
                            attention_cost: 8.0,
                            priority_boost: 0,
                            welfare_relief_delta: 0.28,
                        },
                        sim_command::ArbitrationOption {
                            label: "Hold strict rationing quotas".to_owned(),
                            description: "Maintain output quotas without concessions.".to_owned(),
                            attention_cost: 4.0,
                            priority_boost: 1,
                            welfare_relief_delta: 0.0,
                        },
                    ],
                );
                let _ = Self::trigger_event_notification(
                    &mut self.clock,
                    registry,
                    &self.command,
                    tick,
                    "unrest_escalation",
                );
            }
        }
        self.command.step_hourly(tick, self.pop.all_characters());
        if is_daily_tick {
            Self::record_daily_time_series(
                tick,
                registry,
                &self.economy,
                &self.pop,
                &self.command,
                &mut self.archive,
            );
        }
        let p6_ns = elapsed_ns(t6);

        // Phase 7: CompileDiegeticPlayerViewAndNotifications
        let t7 = self.profiling_enabled.then(Instant::now);
        let live_cells = self.archive.observe_hq_halo(tick, &self.planet, &self.economy);
        for &(sid, cid, _) in &settlement_list {
            if live_cells.contains(&cid.get()) {
                let reporter = self
                    .pop
                    .all_characters()
                    .iter()
                    .find(|c| c.home_settlement == sid)
                    .cloned();
                if let Some(rep) = reporter {
                    for entry in self.economy.settlement_inventory_entries(sid) {
                        if let Some(cdef) = registry.commodity_def(entry.commodity) {
                            self.archive.audit_settlement_against_direct_sight(
                                tick,
                                sid.get(),
                                rep.id.0,
                                &rep.display_name,
                                &cdef.key,
                                entry.mass_kg,
                            );
                        }
                    }
                }
            }
        }
        let p7_entities = live_cells.len() as u32;
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

    fn run_parallel_logistics_and_dispatches(
        &mut self,
        tick: Tick,
        registry: &dyn ModRegistryPort,
    ) -> u32 {
        #[derive(Clone)]
        struct RemoteCourierPlan {
            sid: SettlementId,
            cell: CellId,
            char_id: CharacterId,
        }

        let hq_cell = self.economy.hq_cell();
        let items: Vec<RemoteCourierPlan> = self
            .economy
            .settlements()
            .iter()
            .filter(|s| s.cell_id != hq_cell)
            .map(|s| {
                let char_id = self
                    .pop
                    .all_characters()
                    .iter()
                    .find(|c| c.home_settlement == s.id)
                    .map(|c| c.id)
                    .unwrap_or(CharacterId(1));
                RemoteCourierPlan {
                    sid: s.id,
                    cell: s.cell_id,
                    char_id,
                }
            })
            .collect();

        let map_fn = |w: &RemoteCourierPlan| -> Vec<(u64, u32, RemoteCourierPlan)> {
            let sid_u64 = u64::from(w.sid.get());
            if !in_period_bucket(sid_u64, tick, DAILY_PERIOD) {
                return Vec::new();
            }
            vec![(sid_u64, 0, w.clone())]
        };

        let plans = if self.worker_pool.is_some() {
            execute_parallel_sorted(self.worker_pool.as_ref(), &items, map_fn)
        } else {
            let mut tagged: Vec<(u64, u32, RemoteCourierPlan)> =
                items.iter().flat_map(map_fn).collect();
            tagged.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)));
            tagged.into_iter().map(|(_, _, p)| p).collect()
        };

        let mut count = 0u32;
        let grain_id = registry.commodity_id_by_key("raw_grain");
        let courier_carrier = self.logistics.default_courier_carrier();
        for plan in plans {
            let fodder = grain_id
                .map(|cid| self.economy.load_fuel_into_transit(plan.sid, cid, 3.0))
                .unwrap_or(0.0);
            if self
                .logistics
                .launch_unified_transit(
                    UnifiedTransitLaunch {
                        label: format!("Daily Ledger Rider S#{}", plan.sid.get()),
                        channel: TransitChannelKind::PhysicalCourier,
                        origin_settlement: plan.sid,
                        destination_settlement: self.economy.hq_settlement(),
                        origin_cell: plan.cell,
                        destination_cell: hq_cell,
                        carrier_class: courier_carrier,
                        owner: sim_spine::ActorId(1),
                        cargo_commodity: None,
                        cargo_mass_kg: 0.0,
                        cargo_purity_01: 1.0,
                        fuel_loaded_kg: fodder,
                        spare_parts_loaded_kg: 0.0,
                        current_tick: tick,
                        is_hq_move: false,
                        dispatch_id: None,
                        sender: Some(plan.char_id),
                        mandate_payload_id: None,
                        subject: format!("status_report:{}:{}", plan.sid.get(), plan.char_id.0),
                    },
                    &self.planet,
                    &self.weather,
                    registry,
                )
                .is_ok()
            {
                count += 1;
            }
        }
        count
    }

    /// Executes `count` consecutive ticks via `step_one_tick`.
    pub fn step_ticks(&mut self, registry: &dyn ModRegistryPort, count: u64) {
        for _ in 0..count {
            self.step_one_tick(registry);
        }
    }

    /// Steps up to `max_ticks` forward, halting immediately if the clock hits `Paused` or `AutoPaused` (`ENG-SCN-003`).
    pub fn step_ticks_respecting_clock(
        &mut self,
        registry: &dyn ModRegistryPort,
        max_ticks: u64,
    ) -> u64 {
        let mut ran = 0u64;
        for _ in 0..max_ticks {
            if matches!(
                self.clock.mode,
                ClockRunMode::Paused | ClockRunMode::AutoPaused { .. }
            ) {
                break;
            }
            self.step_one_tick(registry);
            ran += 1;
            if matches!(
                self.clock.mode,
                ClockRunMode::Paused | ClockRunMode::AutoPaused { .. }
            ) {
                break;
            }
        }
        ran
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
        if matches!(
            self.clock.mode,
            ClockRunMode::AutoPaused { .. } | ClockRunMode::Paused
        ) {
            // Preserve internal AutoPaused signal raised inside step_one_tick
        } else {
            self.clock = clock;
        }
        executed
    }

    /// Computes canonical Blake3 world state digest across clock and all eight simulation cartridges (`ENG-CLK-004`, `ENG-CLK-005`).
    #[must_use]
    pub fn state_digest_bytes(&self) -> [u8; 32] {
        compute_canonical_digest(
            &self.clock,
            &[
                &self.planet,
                &self.weather,
                &self.economy,
                &self.logistics,
                &self.pop,
                &self.command,
                &self.core,
                &self.archive,
            ],
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
            &[
                &self.planet,
                &self.weather,
                &self.economy,
                &self.logistics,
                &self.pop,
                &self.command,
                &self.core,
                &self.archive,
            ],
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
                &mut self.economy,
                &mut self.logistics,
                &mut self.pop,
                &mut self.command,
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
                let label = registry
                    .notification_classes()
                    .into_iter()
                    .find(|(eid, _, _, _)| *eid == reason_event)
                    .map(|(_, key, name, _)| format!("{name} ({key})"))
                    .unwrap_or_else(|| format!("EventClass #{}", reason_event.get()));
                (true, 1, Some(label))
            }
        };

        let attn = self.command.attention_summary(CharacterId(0));
        let (focused_id, focused_share) = self.command.personal_focus_state();
        let mandates: Vec<MandateSummaryDto> = self
            .command
            .all_mandates()
            .iter()
            .map(|m| MandateSummaryDto {
                mandate_id: m.id.0,
                assignee_id: m.assignee.0,
                target_settlement_id: m.target_settlement.get(),
                target_cell_id: m.target_cell.get(),
                objective_kind: m.objective_kind.clone(),
                authority_tier: m.authority_tier.clone(),
                summary: m.summary.clone(),
                priority_weight: m.priority_weight,
                status: m.status.as_str().to_owned(),
                realized_efficiency: m.realized_efficiency,
                progress_01: m.progress_01,
                subtasks: m.subtasks.clone(),
            })
            .collect();

        let open_proposals: Vec<ArbitrationProposalDto> = self
            .command
            .open_proposals()
            .into_iter()
            .map(|p| ArbitrationProposalDto {
                proposal_id: p.proposal_id,
                settlement_id: p.settlement.get(),
                origin_cell_id: p.origin_cell.get(),
                raised_by_id: p.raised_by.0,
                raised_by_name: self
                    .pop
                    .character_view(p.raised_by)
                    .map(|c| c.display_name)
                    .unwrap_or_else(|| "Subordinate".to_owned()),
                title: p.title.clone(),
                summary: p.summary.clone(),
                created_tick: p.created_tick.get(),
                deadline_tick: p.deadline_tick.get(),
                options: p
                    .options
                    .iter()
                    .enumerate()
                    .map(|(i, o)| ArbitrationOptionDto {
                        index: i as u32,
                        label: o.label.clone(),
                        description: o.description.clone(),
                        attention_cost: o.attention_cost,
                    })
                    .collect(),
            })
            .collect();

        let notification_rules: Vec<NotificationRuleDto> = registry
            .notification_classes()
            .into_iter()
            .map(|(_, key, display_name, def_beh)| {
                let cur = self
                    .command
                    .notification_rules()
                    .get(&key)
                    .copied()
                    .unwrap_or(def_beh);
                NotificationRuleDto {
                    event_class_key: key,
                    display_name,
                    behavior: format!("{cur:?}"),
                }
            })
            .collect();

        let mut facility_rows = Vec::new();
        for f in self.economy.facilities() {
            if let Some(s_sum) = self
                .economy
                .settlements()
                .iter()
                .find(|s| s.id == f.settlement)
            {
                let (pk, pn) = registry
                    .process_def(f.process)
                    .map(|p| (p.key.clone(), p.display_name.clone()))
                    .unwrap_or_default();
                facility_rows.push(FacilityRowDto {
                    facility_id: f.id,
                    settlement_id: f.settlement.get(),
                    settlement_name: s_sum.display_name.clone(),
                    process_key: pk,
                    process_name: pn,
                    assigned_workers: f.assigned_workers,
                    tool_condition_01: f.tool_condition_01,
                    weather_halted: f.halted_reason.is_some(),
                    shaft_water_01: self.economy.settlement_shaft_water_level(f.settlement),
                    observed_tick: self.clock.current_tick.get(),
                    provenance_label: "Reported Operations".to_owned(),
                });
            }
        }

        let mut road_edges = BTreeMap::new();
        for (&eid, st) in self.logistics.edges() {
            if let Some(infra) = registry.infra_def(st.infra_class) {
                road_edges.insert(
                    eid.get(),
                    EdgeViewInput {
                        edge_id: eid.get(),
                        from_cell_id: 0,
                        to_cell_id: 0,
                        infra_key: infra.key.clone(),
                        washed_out: st.washed_out,
                        river_edge: st.river_edge,
                        bridge_cap_kg: st.bridge_weight_cap_kg,
                    },
                );
            }
        }

        let mut unrest_by_settlement = BTreeMap::new();
        for s in self.economy.settlements() {
            unrest_by_settlement.insert(
                s.id.get(),
                format!("{:?}", self.pop.unrest_stage_for_settlement(s.id)),
            );
        }

        let hq_cell = self.economy.hq_cell();
        let mut relocation_previews = Vec::new();
        let haven_cell = self
            .economy
            .settlements()
            .first()
            .map(|s| s.cell_id)
            .unwrap_or(hq_cell);
        for &nb in self.planet.cell_neighbors(hq_cell).iter().take(6) {
            if self.planet.is_ocean_cell(nb) {
                continue;
            }
            let dist_km = self.planet.great_circle_km(hq_cell, nb);
            let h_dist = self.planet.great_circle_km(nb, haven_cell);
            relocation_previews.push(HqRelocationPreviewDto {
                target_cell_id: nb.get(),
                label: format!("Forward Post Cell #{}", nb.get()),
                travel_hours: (dist_km / 3.4).max(1.0),
                projected_haven_latency_hours: (h_dist / 14.0).max(1.0),
            });
        }

        let extras = SnapshotSubsystemInputs {
            world_seed: self.clock.world_seed,
            clerical_overhead: self.command.clerical_overhead_total(),
            overload_ratio: if attn.capacity_attention > 0.0 {
                attn.used_attention / attn.capacity_attention
            } else {
                0.0
            },
            focused_mandate_id: focused_id.map(|m| m.0),
            focused_attention_share: focused_share,
            mandates,
            open_proposals,
            notification_rules,
            facility_rows,
            road_edges,
            unrest_by_settlement,
            relocation_previews,
        };

        self.archive.compile_snapshot_m4(
            self.clock.current_tick,
            paused,
            speed_multiplier,
            auto_pause_reason,
            registry,
            &self.planet,
            &self.weather,
            &self.economy,
            &self.pop,
            Some(&self.logistics),
            Some(&self.command),
            &extras,
        )
    }

    /// Validates world health invariants across weather, economy conservation, and population conservation.
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
        let mass = self.economy.total_stockpile_mass_kg();
        if !mass.is_finite() || mass < 0.0 {
            return Err(format!("invalid total stockpile mass {mass}"));
        }
        let econ_err = self.economy.conservation_error_kg();
        if !econ_err.is_finite() || econ_err > 1e-4 {
            return Err(format!("economy mass conservation error {econ_err} kg"));
        }
        let pop_err = self.pop.conservation_discrepancy();
        if pop_err != 0 {
            return Err(format!(
                "population integer conservation discrepancy {pop_err}"
            ));
        }
        if self.pop.total_population() == 0 {
            return Err("colony population collapsed to zero".to_owned());
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
        SpinePlayerCommand::AcknowledgeNotification(_) => {
            if matches!(clock.mode, ClockRunMode::AutoPaused { .. }) {
                clock.mode = ClockRunMode::Paused;
            }
        }
        SpinePlayerCommand::StepSingleTick
        | SpinePlayerCommand::StubPing { .. }
        | SpinePlayerCommand::IssueMandate { .. }
        | SpinePlayerCommand::ResolveArbitrationProposal { .. }
        | SpinePlayerCommand::SetPersonalAttentionZoom { .. }
        | SpinePlayerCommand::RelocateHq { .. }
        | SpinePlayerCommand::UpdateNotificationRule { .. } => {}
    }
}

#[inline]
fn elapsed_ns(started: Option<Instant>) -> u64 {
    started.map_or(0, |t| t.elapsed().as_nanos() as u64)
}

const _CANONICAL_CLOCK_CHECK: Option<&dyn CanonicalSaveablePort> = None;
