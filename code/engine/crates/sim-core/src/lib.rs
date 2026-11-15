//! Pure data-agnostic deterministic simulation kernel executing strictly via `sim-spine` ports.
//! Contains zero hardcoded commodities, processes, carriers, scenarios, or rendering code.
#![deny(clippy::disallowed_types)]

use std::collections::BTreeMap;

use sim_spine::{
    codec::{
        read_f32_bits, read_f64_bits, read_str, read_u16_le, read_u32_le, read_u64_le, read_u8,
        write_f32_bits, write_f64_bits, write_str, write_u16_le, write_u32_le, write_u64_le,
        write_u8,
    },
    ActorId, AttentionLoadView, CanonicalSaveablePort, CellId, CharacterId, CharacterSnapshotView,
    CohortDelta, CohortId, CohortSnapshotView, CommandPort, CommodityId, EdgeId, FacilityId,
    GovernanceFrictionPlugPort, KnowledgeModifierPlugPort, LedgerCommand, LedgerReceipt,
    MandateId, MandateView, ModRegistryPort, NeutralGovernanceFrictionPlug,
    NeutralKnowledgeModifierPlug, NeutralNonStateEconomyPlug, NeutralOperationalCombatPlug,
    NeutralSuccessionPlug, NonStateEconomyPlugPort, OperationalCombatPlugPort, PhysicalLedgerPort,
    PopulationPort, ProcessId, SettlementId, SettlementSummaryView, SpinePlayerCommand,
    StockpileEntryView, StockpileId, SuccessionPlugPort, SurfaceConditionsPort, Tick, TickPhase,
    VocationId,
};

#[derive(Clone, Debug)]
struct InternalFacility {
    id: FacilityId,
    settlement: SettlementId,
    process: ProcessId,
    workers: u32,
}

#[derive(Clone, Debug)]
struct InternalStockLot {
    stockpile: StockpileId,
    settlement: SettlementId,
    commodity: CommodityId,
    owner: ActorId,
    mass_kg: f64,
    purity: f32,
}

/// Deterministic planet + economy + cohort state initialized exclusively from `ModRegistryPort`.
pub struct SimWorld {
    current_tick: Tick,
    paused: bool,
    speed_multiplier: u32,
    world_seed: u64,
    cell_count: u32,
    hq_settlement: Option<SettlementId>,
    hq_cell: CellId,
    hq_sight_radius_cells: u8,
    settlements: Vec<SettlementSummaryView>,
    settlement_stockpile_ids: BTreeMap<SettlementId, Vec<StockpileId>>,
    stock_lots: BTreeMap<(StockpileId, CommodityId, ActorId), InternalStockLot>,
    facilities: Vec<InternalFacility>,
    cohorts: BTreeMap<CohortId, CohortSnapshotView>,
    settlement_cohort_ids: BTreeMap<SettlementId, Vec<CohortId>>,
    characters: Vec<CharacterSnapshotView>,
    active_mandates: Vec<MandateView>,
    non_state_eco: Box<dyn NonStateEconomyPlugPort>,
    governance_friction: Box<dyn GovernanceFrictionPlugPort>,
    knowledge_mod: Box<dyn KnowledgeModifierPlugPort>,
    combat_plug: Box<dyn OperationalCombatPlugPort>,
    succession_plug: Box<dyn SuccessionPlugPort>,
}

impl SimWorld {
    /// Initializes a clean simulation world exclusively from external data provided via `ModRegistryPort`.
    #[must_use]
    pub fn from_registry(registry: &dyn ModRegistryPort, world_seed: u64) -> Self {
        let scenario = registry.scenario_init();
        let subdiv = scenario.goldberg_subdivision.max(1) as u32;
        let cell_count = 10 * (1u32 << (2 * subdiv.min(4))) + 2;
        let primary_actor = ActorId(1);

        let mut settlements = Vec::with_capacity(scenario.settlements.len());
        let mut settlement_stockpile_ids: BTreeMap<SettlementId, Vec<StockpileId>> = BTreeMap::new();
        let mut stock_lots: BTreeMap<(StockpileId, CommodityId, ActorId), InternalStockLot> =
            BTreeMap::new();
        let mut facilities = Vec::new();
        let mut cohorts: BTreeMap<CohortId, CohortSnapshotView> = BTreeMap::new();
        let mut settlement_cohort_ids: BTreeMap<SettlementId, Vec<CohortId>> = BTreeMap::new();
        let mut settlement_id_by_key: BTreeMap<String, SettlementId> = BTreeMap::new();

        let mut next_cohort_id = 1u32;
        let mut next_facility_id = 1u32;

        for (idx, seed) in scenario.settlements.iter().enumerate() {
            let sid = SettlementId(idx as u32);
            let stock_id = StockpileId(idx as u32);
            let cell_id = CellId(seed.cell_index % cell_count.max(1));
            settlement_id_by_key.insert(seed.key.clone(), sid);
            settlements.push(SettlementSummaryView {
                id: sid,
                key: seed.key.clone(),
                display_name: seed.display_name.clone(),
                cell_id,
                owner: primary_actor,
            });
            settlement_stockpile_ids.insert(sid, vec![stock_id]);

            for (cid, mass_kg, purity) in &seed.stockpiles {
                let entry = stock_lots
                    .entry((stock_id, *cid, primary_actor))
                    .or_insert(InternalStockLot {
                        stockpile: stock_id,
                        settlement: sid,
                        commodity: *cid,
                        owner: primary_actor,
                        mass_kg: 0.0,
                        purity: *purity,
                    });
                entry.mass_kg += *mass_kg;
            }

            let mut s_cohorts = Vec::new();
            for (vocation, count) in &seed.cohorts {
                let coh_id = CohortId(next_cohort_id);
                next_cohort_id += 1;
                s_cohorts.push(coh_id);
                cohorts.insert(
                    coh_id,
                    CohortSnapshotView {
                        id: coh_id,
                        settlement: sid,
                        vocation: *vocation,
                        headcount: *count,
                        mean_skill_01: 0.35,
                    },
                );
            }
            settlement_cohort_ids.insert(sid, s_cohorts);

            for (proc_id, workers) in &seed.initial_facilities {
                let fid = FacilityId(next_facility_id);
                next_facility_id += 1;
                facilities.push(InternalFacility {
                    id: fid,
                    settlement: sid,
                    process: *proc_id,
                    workers: *workers,
                });
            }
        }

        let hq_settlement = settlement_id_by_key.get(&scenario.hq_settlement_key).copied();
        let hq_cell = hq_settlement
            .and_then(|sid| settlements.get(sid.get() as usize))
            .map(|s| s.cell_id)
            .unwrap_or(CellId(0));

        let mut characters = Vec::with_capacity(scenario.characters.len());
        for (idx, ch) in scenario.characters.iter().enumerate() {
            let home = settlement_id_by_key
                .get(&ch.home_settlement_key)
                .copied()
                .or(hq_settlement)
                .unwrap_or(SettlementId(0));
            characters.push(CharacterSnapshotView {
                id: CharacterId(idx as u32 + 1),
                key: ch.key.clone(),
                display_name: ch.display_name.clone(),
                role_title: ch.role_title.clone(),
                home_settlement: home,
                competence: ch.competence,
                precision_01: ch.precision_01,
                honesty_vs_sycophancy: ch.honesty_vs_sycophancy,
                caution_vs_alarmism: ch.caution_vs_alarmism,
            });
        }

        Self {
            current_tick: Tick(0),
            paused: false,
            speed_multiplier: 1,
            world_seed,
            cell_count,
            hq_settlement,
            hq_cell,
            hq_sight_radius_cells: scenario.hq_sight_radius_cells,
            settlements,
            settlement_stockpile_ids,
            stock_lots,
            facilities,
            cohorts,
            settlement_cohort_ids,
            characters,
            active_mandates: Vec::new(),
            non_state_eco: Box::new(NeutralNonStateEconomyPlug),
            governance_friction: Box::new(NeutralGovernanceFrictionPlug),
            knowledge_mod: Box::new(NeutralKnowledgeModifierPlug),
            combat_plug: Box::new(NeutralOperationalCombatPlug),
            succession_plug: Box::new(NeutralSuccessionPlug),
        }
    }

    /// Synchronizes planetary cell count from `sim-planet` into core validation bounds.
    pub fn set_planet_cell_count(&mut self, cell_count: u32) {
        self.cell_count = cell_count.max(1);
    }

    /// Relocates a settlement to `new_cell` and keeps `hq_cell` synchronized when applicable.
    pub fn relocate_settlement_cell(&mut self, settlement_id: SettlementId, new_cell: CellId) {
        for s in &mut self.settlements {
            if s.id == settlement_id {
                s.cell_id = new_cell;
            }
        }
        if self.hq_settlement == Some(settlement_id) {
            self.hq_cell = new_cell;
        }
    }

    /// Synchronizes clock tick and pause state into `SimWorld`.
    pub fn sync_clock_tick(&mut self, tick: Tick, paused: bool, speed_multiplier: u32) {
        self.current_tick = tick;
        self.paused = paused;
        self.speed_multiplier = speed_multiplier.max(1);
    }

    /// Runs Phase 3 hourly combat update.
    pub fn step_hourly_combat_phase(&mut self, tick: Tick) -> u32 {
        self.combat_plug.step_hourly_combat(tick);
        0
    }

    /// Runs Phase 5 daily private-actor/market update.
    pub fn step_daily_markets_phase(&mut self, tick: Tick) -> u32 {
        self.non_state_eco.step_private_actors(tick);
        self.settlements.len() as u32
    }

    /// Runs Phase 7 weekly/monthly demographics & succession check.
    pub fn step_weekly_demographics_phase(&mut self, tick: Tick) -> u32 {
        if tick.get().is_multiple_of(168) {
            let _ = self
                .succession_plug
                .check_succession_transition(tick, self.hq_cell);
            return self.cohorts.len() as u32;
        }
        0
    }

    /// Total headcount across all cohorts.
    #[must_use]
    pub fn total_population(&self) -> u64 {
        self.cohorts.values().map(|c| u64::from(c.headcount)).sum()
    }

    /// Total material mass stored across all stockpiles (`kg`).
    #[must_use]
    pub fn total_stockpile_mass_kg(&self) -> f64 {
        self.stock_lots.values().map(|l| l.mass_kg).sum()
    }

    /// Number of initialized production/extraction facilities.
    #[must_use]
    pub fn facility_count(&self) -> usize {
        self.facilities.len()
    }

    /// Number of active demographic cohorts.
    #[must_use]
    pub fn cohort_count(&self) -> usize {
        self.cohorts.len()
    }

    /// Returns current discrete simulation hour tick.
    #[must_use]
    pub fn current_tick(&self) -> Tick {
        self.current_tick
    }

    /// Returns whether simulation clock is paused.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Steps the deterministic 8-phase hourly tick pipeline by one hour.
    pub fn step_one_tick(&mut self, registry: &dyn ModRegistryPort) {
        let tick = Tick(self.current_tick.get() + 1);
        self.current_tick = tick;

        let phases = [
            TickPhase::IngestPlayerCommands,
            TickPhase::HourlyWeatherAndHydrology,
            TickPhase::HourlyCombatStep,
            TickPhase::HourlyCourierAndSignalStep,
            TickPhase::DailyBucketLogisticsAndMarkets,
            TickPhase::DailyBucketExtractionProductionConstruction,
            TickPhase::WeeklyMonthlyBucketDemographicsMandatesResearch,
            TickPhase::CompileDiegeticPlayerViewAndNotifications,
        ];

        for phase in phases {
            match phase {
                TickPhase::IngestPlayerCommands => {}
                TickPhase::HourlyWeatherAndHydrology => {}
                TickPhase::HourlyCombatStep => {
                    self.combat_plug.step_hourly_combat(tick);
                }
                TickPhase::HourlyCourierAndSignalStep => {}
                TickPhase::DailyBucketLogisticsAndMarkets => {
                    self.non_state_eco.step_private_actors(tick);
                }
                TickPhase::DailyBucketExtractionProductionConstruction => {
                    let _ = self.step_daily_production_with_weather(tick, registry, None);
                }
                TickPhase::WeeklyMonthlyBucketDemographicsMandatesResearch => {
                    if tick.get().is_multiple_of(168) {
                        let _ = self.succession_plug.check_succession_transition(tick, self.hq_cell);
                    }
                }
                TickPhase::CompileDiegeticPlayerViewAndNotifications => {}
            }
        }
    }

    /// Steps `count` hourly ticks cleanly without panics.
    pub fn step_ticks(&mut self, registry: &dyn ModRegistryPort, count: u64) {
        for _ in 0..count {
            self.step_one_tick(registry);
        }
    }

    fn apply_hourly_spoilage(&mut self, registry: &dyn ModRegistryPort) {
        for lot in self.stock_lots.values_mut() {
            if let Some(def) = registry.commodity_def(lot.commodity)
                && def.spoilage_rate_per_hour > 0.0 && lot.mass_kg > 0.0 {
                    lot.mass_kg = (lot.mass_kg * (1.0 - def.spoilage_rate_per_hour)).max(0.0);
                }
        }
    }

    /// Executes staggered daily production & extraction bucket (with optional surface mud halt checks) plus hourly spoilage, returning the count of due facilities evaluated.
    pub fn step_daily_production_with_weather(
        &mut self,
        tick: Tick,
        registry: &dyn ModRegistryPort,
        weather: Option<&dyn SurfaceConditionsPort>,
    ) -> u32 {
        let primary_actor = ActorId(1);
        let mut evaluated = 0u32;
        for fac_idx in 0..self.facilities.len() {
            let fac_id = self.facilities[fac_idx].id;
            if u64::from(fac_id.get()) % 24 != tick.get() % 24 {
                continue;
            }
            evaluated += 1;
            let process_id = self.facilities[fac_idx].process;
            let settlement_id = self.facilities[fac_idx].settlement;
            let workers = self.facilities[fac_idx].workers;
            if !self
                .knowledge_mod
                .process_prerequisites_met(settlement_id, process_id)
            {
                continue;
            }
            let Some(proc_def) = registry.process_def(process_id) else {
                continue;
            };
            if let Some(w_port) = weather
                && proc_def.weather_halt_mud_threshold > 0.0
            {
                let settlement_cell = self
                    .settlements
                    .iter()
                    .find(|s| s.id == settlement_id)
                    .map(|s| s.cell_id)
                    .unwrap_or(self.hq_cell);
                let surf = w_port.surface(settlement_cell);
                if surf.mud_index_01 >= proc_def.weather_halt_mud_threshold {
                    continue;
                }
            }
            let Some(stock_id) = self
                .settlement_stockpile_ids
                .get(&settlement_id)
                .and_then(|v| v.first().copied())
            else {
                continue;
            };

            let labor_ratio = (f64::from(workers) * 8.0 / proc_def.workforce_hours_required.max(1.0))
                .clamp(0.0, 4.0);
            if labor_ratio <= 0.0 {
                continue;
            }

            let mut max_affordable_ratio = labor_ratio;
            for inp in &proc_def.inputs {
                let avail = self
                    .stock_lots
                    .get(&(stock_id, inp.commodity, primary_actor))
                    .map(|l| l.mass_kg)
                    .unwrap_or(0.0);
                if inp.amount_kg > 0.0 {
                    max_affordable_ratio = max_affordable_ratio.min(avail / inp.amount_kg);
                }
            }
            if max_affordable_ratio <= 1e-6 {
                continue;
            }

            for inp in &proc_def.inputs {
                if let Some(lot) = self.stock_lots.get_mut(&(stock_id, inp.commodity, primary_actor)) {
                    lot.mass_kg = (lot.mass_kg - inp.amount_kg * max_affordable_ratio).max(0.0);
                }
            }

            if let Some(tool_id) = proc_def.tool_commodity
                && let Some(tool_def) = registry.commodity_def(tool_id)
                    && let Some(tool_lot) =
                        self.stock_lots.get_mut(&(stock_id, tool_id, primary_actor))
                    {
                        let wear = tool_def.tool_wear_per_hour * f64::from(proc_def.cycle_hours);
                        tool_lot.mass_kg = (tool_lot.mass_kg - wear).max(0.0);
                    }

            for out in &proc_def.outputs {
                let entry = self
                    .stock_lots
                    .entry((stock_id, out.commodity, primary_actor))
                    .or_insert(InternalStockLot {
                        stockpile: stock_id,
                        settlement: settlement_id,
                        commodity: out.commodity,
                        owner: primary_actor,
                        mass_kg: 0.0,
                        purity: 0.9,
                    });
                entry.mass_kg += out.amount_kg * max_affordable_ratio;
            }
        }
        self.apply_hourly_spoilage(registry);
        evaluated
    }

    /// Computes a deterministic Blake3 digest of the full physical world state.
    #[must_use]
    pub fn state_digest_hex(&self) -> String {
        let mut buf = Vec::with_capacity(4096);
        self.write_canonical_state(&mut buf);
        blake3::hash(&buf).to_hex().to_string()
    }

    /// Reads current vocational skill multiplier if present (placeholder for M3/M4 hooks).
    #[must_use]
    pub fn governance_factor(&self, settlement: SettlementId, leader: CharacterId) -> f32 {
        self.governance_friction
            .oversight_efficiency_multiplier(settlement, leader)
    }

    /// Reads edge congestion multiplier for routing layers cleanly.
    #[must_use]
    pub fn edge_congestion(&self, edge: EdgeId) -> f32 {
        self.non_state_eco.edge_congestion_multiplier(edge)
    }

    /// Helper used only in inspection: total cohort headcount for a given vocation.
    #[must_use]
    pub fn total_population_for_vocation(&self, vocation: VocationId) -> u32 {
        self.cohorts
            .values()
            .filter(|c| c.vocation == vocation)
            .map(|c| c.headcount)
            .sum()
    }
}

impl CanonicalSaveablePort for SimWorld {
    fn cartridge_tag(&self) -> &'static str {
        "sim-core"
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        write_u64_le(out, self.current_tick.get());
        write_u8(out, u8::from(self.paused));
        write_u32_le(out, self.speed_multiplier);
        write_u64_le(out, self.world_seed);
        write_u32_le(out, self.cell_count);
        match self.hq_settlement {
            None => {
                write_u8(out, 0);
                write_u32_le(out, 0);
            }
            Some(sid) => {
                write_u8(out, 1);
                write_u32_le(out, sid.get());
            }
        }
        write_u32_le(out, self.hq_cell.get());
        write_u8(out, self.hq_sight_radius_cells);

        write_u32_le(out, self.settlements.len() as u32);
        for s in &self.settlements {
            write_u32_le(out, s.id.get());
            write_str(out, &s.key);
            write_str(out, &s.display_name);
            write_u32_le(out, s.cell_id.get());
            write_u32_le(out, s.owner.get());
        }

        write_u32_le(out, self.settlement_stockpile_ids.len() as u32);
        for (sid, stocks) in &self.settlement_stockpile_ids {
            write_u32_le(out, sid.get());
            write_u32_le(out, stocks.len() as u32);
            for st in stocks {
                write_u32_le(out, st.get());
            }
        }

        write_u32_le(out, self.stock_lots.len() as u32);
        for lot in self.stock_lots.values() {
            write_u32_le(out, lot.stockpile.get());
            write_u32_le(out, lot.settlement.get());
            write_u16_le(out, lot.commodity.get());
            write_u32_le(out, lot.owner.get());
            write_f64_bits(out, lot.mass_kg);
            write_f32_bits(out, lot.purity);
        }

        write_u32_le(out, self.facilities.len() as u32);
        for f in &self.facilities {
            write_u32_le(out, f.id.get());
            write_u32_le(out, f.settlement.get());
            write_u16_le(out, f.process.get());
            write_u32_le(out, f.workers);
        }

        write_u32_le(out, self.cohorts.len() as u32);
        for coh in self.cohorts.values() {
            write_u32_le(out, coh.id.get());
            write_u32_le(out, coh.settlement.get());
            write_u16_le(out, coh.vocation.get());
            write_u32_le(out, coh.headcount);
            write_f32_bits(out, coh.mean_skill_01);
        }

        write_u32_le(out, self.settlement_cohort_ids.len() as u32);
        for (sid, ids) in &self.settlement_cohort_ids {
            write_u32_le(out, sid.get());
            write_u32_le(out, ids.len() as u32);
            for cid in ids {
                write_u32_le(out, cid.get());
            }
        }

        write_u32_le(out, self.characters.len() as u32);
        for ch in &self.characters {
            write_u32_le(out, ch.id.get());
            write_str(out, &ch.key);
            write_str(out, &ch.display_name);
            write_str(out, &ch.role_title);
            write_u32_le(out, ch.home_settlement.get());
            write_f32_bits(out, ch.competence);
            write_f32_bits(out, ch.precision_01);
            write_f32_bits(out, ch.honesty_vs_sycophancy);
            write_f32_bits(out, ch.caution_vs_alarmism);
        }

        write_u32_le(out, self.active_mandates.len() as u32);
        for m in &self.active_mandates {
            write_u64_le(out, m.id.get());
            write_u32_le(out, m.issuer.get());
            write_u32_le(out, m.assignee.get());
            write_u32_le(out, m.target_settlement.get());
            write_str(out, &m.summary);
            write_str(out, &m.status);
        }
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        self.current_tick = Tick(read_u64_le(input)?);
        self.paused = read_u8(input)? != 0;
        self.speed_multiplier = read_u32_le(input)?;
        self.world_seed = read_u64_le(input)?;
        self.cell_count = read_u32_le(input)?;
        let has_hq = read_u8(input)? != 0;
        let hq_raw = read_u32_le(input)?;
        self.hq_settlement = if has_hq {
            Some(SettlementId(hq_raw))
        } else {
            None
        };
        self.hq_cell = CellId(read_u32_le(input)?);
        self.hq_sight_radius_cells = read_u8(input)?;

        let n_set = read_u32_le(input)? as usize;
        self.settlements.clear();
        self.settlements.reserve(n_set);
        for _ in 0..n_set {
            self.settlements.push(SettlementSummaryView {
                id: SettlementId(read_u32_le(input)?),
                key: read_str(input)?,
                display_name: read_str(input)?,
                cell_id: CellId(read_u32_le(input)?),
                owner: ActorId(read_u32_le(input)?),
            });
        }

        let n_ss = read_u32_le(input)? as usize;
        self.settlement_stockpile_ids.clear();
        for _ in 0..n_ss {
            let sid = SettlementId(read_u32_le(input)?);
            let cnt = read_u32_le(input)? as usize;
            let mut stocks = Vec::with_capacity(cnt);
            for _ in 0..cnt {
                stocks.push(StockpileId(read_u32_le(input)?));
            }
            self.settlement_stockpile_ids.insert(sid, stocks);
        }

        let n_lots = read_u32_le(input)? as usize;
        self.stock_lots.clear();
        for _ in 0..n_lots {
            let lot = InternalStockLot {
                stockpile: StockpileId(read_u32_le(input)?),
                settlement: SettlementId(read_u32_le(input)?),
                commodity: CommodityId(read_u16_le(input)?),
                owner: ActorId(read_u32_le(input)?),
                mass_kg: read_f64_bits(input)?,
                purity: read_f32_bits(input)?,
            };
            self.stock_lots
                .insert((lot.stockpile, lot.commodity, lot.owner), lot);
        }

        let n_fac = read_u32_le(input)? as usize;
        self.facilities.clear();
        self.facilities.reserve(n_fac);
        for _ in 0..n_fac {
            self.facilities.push(InternalFacility {
                id: FacilityId(read_u32_le(input)?),
                settlement: SettlementId(read_u32_le(input)?),
                process: ProcessId(read_u16_le(input)?),
                workers: read_u32_le(input)?,
            });
        }

        let n_coh = read_u32_le(input)? as usize;
        self.cohorts.clear();
        for _ in 0..n_coh {
            let coh = CohortSnapshotView {
                id: CohortId(read_u32_le(input)?),
                settlement: SettlementId(read_u32_le(input)?),
                vocation: VocationId(read_u16_le(input)?),
                headcount: read_u32_le(input)?,
                mean_skill_01: read_f32_bits(input)?,
            };
            self.cohorts.insert(coh.id, coh);
        }

        let n_sc = read_u32_le(input)? as usize;
        self.settlement_cohort_ids.clear();
        for _ in 0..n_sc {
            let sid = SettlementId(read_u32_le(input)?);
            let cnt = read_u32_le(input)? as usize;
            let mut ids = Vec::with_capacity(cnt);
            for _ in 0..cnt {
                ids.push(CohortId(read_u32_le(input)?));
            }
            self.settlement_cohort_ids.insert(sid, ids);
        }

        let n_ch = read_u32_le(input)? as usize;
        self.characters.clear();
        self.characters.reserve(n_ch);
        for _ in 0..n_ch {
            self.characters.push(CharacterSnapshotView {
                id: CharacterId(read_u32_le(input)?),
                key: read_str(input)?,
                display_name: read_str(input)?,
                role_title: read_str(input)?,
                home_settlement: SettlementId(read_u32_le(input)?),
                competence: read_f32_bits(input)?,
                precision_01: read_f32_bits(input)?,
                honesty_vs_sycophancy: read_f32_bits(input)?,
                caution_vs_alarmism: read_f32_bits(input)?,
            });
        }

        let n_man = read_u32_le(input)? as usize;
        self.active_mandates.clear();
        self.active_mandates.reserve(n_man);
        for _ in 0..n_man {
            self.active_mandates.push(MandateView {
                id: MandateId(read_u64_le(input)?),
                issuer: CharacterId(read_u32_le(input)?),
                assignee: CharacterId(read_u32_le(input)?),
                target_settlement: SettlementId(read_u32_le(input)?),
                summary: read_str(input)?,
                status: read_str(input)?,
            });
        }
        Ok(())
    }
}

impl PhysicalLedgerPort for SimWorld {
    fn settlements(&self) -> &[SettlementSummaryView] {
        &self.settlements
    }

    fn hq_settlement(&self) -> Option<SettlementId> {
        self.hq_settlement
    }

    fn hq_cell(&self) -> CellId {
        self.hq_cell
    }

    fn hq_sight_radius_cells(&self) -> u8 {
        self.hq_sight_radius_cells
    }

    fn stockpile_mass_kg(
        &self,
        stock: StockpileId,
        commodity: CommodityId,
        owner: Option<ActorId>,
    ) -> f64 {
        self.stock_lots
            .iter()
            .filter(|((s, c, o), _)| {
                *s == stock && *c == commodity && owner.is_none_or(|req| *o == req)
            })
            .map(|(_, lot)| lot.mass_kg)
            .sum()
    }

    fn settlement_stockpiles(&self, settlement: SettlementId) -> &[StockpileId] {
        self.settlement_stockpile_ids
            .get(&settlement)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn settlement_inventory_entries(&self, settlement: SettlementId) -> Vec<StockpileEntryView> {
        self.stock_lots
            .values()
            .filter(|lot| lot.settlement == settlement)
            .map(|lot| StockpileEntryView {
                stockpile: lot.stockpile,
                settlement: lot.settlement,
                commodity: lot.commodity,
                owner: lot.owner,
                mass_kg: lot.mass_kg,
                average_purity: lot.purity,
            })
            .collect()
    }

    fn apply_ledger_commands(&mut self, cmds: &[LedgerCommand]) -> Vec<LedgerReceipt> {
        let mut out = Vec::with_capacity(cmds.len());
        for cmd in cmds {
            let sid = SettlementId(cmd.stockpile.get());
            let entry = self
                .stock_lots
                .entry((cmd.stockpile, cmd.commodity, cmd.owner))
                .or_insert(InternalStockLot {
                    stockpile: cmd.stockpile,
                    settlement: sid,
                    commodity: cmd.commodity,
                    owner: cmd.owner,
                    mass_kg: 0.0,
                    purity: 1.0,
                });
            let next_mass = entry.mass_kg + cmd.delta_kg;
            if next_mass >= 0.0 {
                entry.mass_kg = next_mass;
                out.push(LedgerReceipt {
                    applied: true,
                    resulting_mass_kg: entry.mass_kg,
                });
            } else {
                out.push(LedgerReceipt {
                    applied: false,
                    resulting_mass_kg: entry.mass_kg,
                });
            }
        }
        out
    }
}

impl PopulationPort for SimWorld {
    fn settlement_cohorts(&self, settlement: SettlementId) -> &[CohortId] {
        self.settlement_cohort_ids
            .get(&settlement)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn cohort_view(&self, id: CohortId) -> Option<CohortSnapshotView> {
        self.cohorts.get(&id).cloned()
    }

    fn character_view(&self, id: CharacterId) -> Option<CharacterSnapshotView> {
        self.characters.iter().find(|c| c.id == id).cloned()
    }

    fn all_characters(&self) -> &[CharacterSnapshotView] {
        &self.characters
    }

    fn apply_labor_and_welfare(&mut self, updates: &[CohortDelta]) {
        for upd in updates {
            if let Some(coh) = self.cohorts.get_mut(&upd.cohort) {
                let next_head = (coh.headcount as i64 + i64::from(upd.delta_headcount)).max(0) as u32;
                coh.headcount = next_head;
                coh.mean_skill_01 = (coh.mean_skill_01 + upd.delta_skill).clamp(0.0, 1.0);
            }
        }
    }
}

impl CommandPort for SimWorld {
    fn attention_summary(&self, who: CharacterId) -> AttentionLoadView {
        AttentionLoadView {
            owner: who,
            used_attention: 0.0,
            capacity_attention: 100.0,
            active_mandate_count: self.active_mandates.len(),
        }
    }

    fn active_mandates_for(&self, who: CharacterId) -> &[MandateView] {
        let _ = who;
        &self.active_mandates
    }

    fn ingest_player_command(&mut self, _tick: Tick, cmd: SpinePlayerCommand) {
        match cmd {
            SpinePlayerCommand::SetPaused(p) => self.paused = p,
            SpinePlayerCommand::StepSingleTick => {}
            SpinePlayerCommand::SetSpeedMultiplier(m) => self.speed_multiplier = m.max(1),
            SpinePlayerCommand::AcknowledgeNotification(_) => {}
            SpinePlayerCommand::StubPing { .. } => {}
        }
    }
}
