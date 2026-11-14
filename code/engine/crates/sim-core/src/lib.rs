//! Pure data-agnostic deterministic simulation kernel executing strictly via `sim-spine` ports.
//! Contains zero hardcoded commodities, processes, carriers, scenarios, or rendering code.
#![deny(clippy::disallowed_types)]

use std::collections::BTreeMap;

use sim_spine::{
    ActorId, AttentionLoadView, CellGeologyView, CellId, CharacterId, CharacterSnapshotView,
    CohortDelta, CohortId, CohortSnapshotView, CommandPort, CommodityId, EdgeId, FacilityId,
    GovernanceFrictionPlugPort, KnowledgeModifierPlugPort, LedgerCommand, LedgerReceipt,
    MandateView, ModRegistryPort, NeutralGovernanceFrictionPlug, NeutralKnowledgeModifierPlug,
    NeutralNonStateEconomyPlug, NeutralOperationalCombatPlug, NeutralSuccessionPlug,
    NonStateEconomyPlugPort, OperationalCombatPlugPort, PhysicalLedgerPort, PlanetGeometryPort,
    PopulationPort, ProcessId, SettlementId, SettlementSummaryView, SpinePlayerCommand,
    StockpileEntryView, StockpileId, SuccessionPlugPort, SurfaceConditionSnapshot,
    SurfaceConditionsPort, Tick, TickPhase, VocationId,
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
    neighbors_scratch: Vec<CellId>,
    empty_neighbors: Vec<CellId>,
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

        let neighbors_scratch = if cell_count > 1 {
            vec![CellId(1), CellId((2).min(cell_count - 1))]
        } else {
            Vec::new()
        };

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
            neighbors_scratch,
            empty_neighbors: Vec::new(),
            non_state_eco: Box::new(NeutralNonStateEconomyPlug),
            governance_friction: Box::new(NeutralGovernanceFrictionPlug),
            knowledge_mod: Box::new(NeutralKnowledgeModifierPlug),
            combat_plug: Box::new(NeutralOperationalCombatPlug),
            succession_plug: Box::new(NeutralSuccessionPlug),
        }
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
                    self.run_daily_facility_bucket(tick, registry);
                    self.apply_hourly_spoilage(registry);
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

    fn run_daily_facility_bucket(&mut self, tick: Tick, registry: &dyn ModRegistryPort) {
        let primary_actor = ActorId(1);
        for fac_idx in 0..self.facilities.len() {
            let fac_id = self.facilities[fac_idx].id;
            if u64::from(fac_id.get()) % 24 != tick.get() % 24 {
                continue;
            }
            let process_id = self.facilities[fac_idx].process;
            let settlement_id = self.facilities[fac_idx].settlement;
            let workers = self.facilities[fac_idx].workers;
            if !self.knowledge_mod.process_prerequisites_met(settlement_id, process_id) {
                continue;
            }
            let Some(proc_def) = registry.process_def(process_id) else {
                continue;
            };
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
    }

    /// Computes a deterministic Blake3 digest of the full physical world state.
    #[must_use]
    pub fn state_digest_hex(&self) -> String {
        let mut hasher = blake3::Hasher::new();
        hasher.update(&self.current_tick.get().to_le_bytes());
        hasher.update(&self.world_seed.to_le_bytes());
        for ((stock, cid, owner), lot) in &self.stock_lots {
            hasher.update(&stock.get().to_le_bytes());
            hasher.update(&cid.get().to_le_bytes());
            hasher.update(&owner.get().to_le_bytes());
            hasher.update(&lot.mass_kg.to_bits().to_le_bytes());
        }
        hasher.finalize().to_hex().to_string()
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

impl PlanetGeometryPort for SimWorld {
    fn cell_count(&self) -> u32 {
        self.cell_count
    }

    fn cell_neighbors(&self, cell: CellId) -> &[CellId] {
        if cell.get() == 0 {
            &self.neighbors_scratch
        } else {
            &self.empty_neighbors
        }
    }

    fn cell_edge(&self, _from: CellId, _to: CellId) -> Option<EdgeId> {
        Some(EdgeId(0))
    }

    fn cell_elevation_m(&self, _cell: CellId) -> f32 {
        120.0
    }

    fn cell_slope_rad(&self, _edge: EdgeId) -> f32 {
        0.05
    }

    fn cell_biome_and_strata(&self, cell: CellId) -> CellGeologyView {
        CellGeologyView {
            cell_id: cell,
            elevation_m: 120.0,
            strata_label: String::new(),
            biome_key: String::new(),
            is_ocean: false,
        }
    }

    fn watershed_downstream(&self, _cell: CellId) -> Option<CellId> {
        None
    }

    fn great_circle_km(&self, a: CellId, b: CellId) -> f32 {
        if a == b {
            0.0
        } else {
            42.0
        }
    }
}

impl SurfaceConditionsPort for SimWorld {
    fn surface(&self, _cell: CellId) -> SurfaceConditionSnapshot {
        SurfaceConditionSnapshot::default()
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
