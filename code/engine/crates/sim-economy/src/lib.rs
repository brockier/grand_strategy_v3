#![deny(clippy::disallowed_types)]

//! Physical stock-batch ledger, conservation of mass, tool degradation,
//! data-driven production processes, and weather damage (`ENG-ECO-001`..`004`, `ENG-WTH-004`..`005`).

use std::collections::BTreeMap;

use sim_spine::{
    codec::{
        read_f32_bits, read_f64_bits, read_str, read_u16_le, read_u32_le, read_u64_le, read_u8,
        write_f32_bits, write_f64_bits, write_str, write_u16_le, write_u32_le, write_u64_le,
        write_u8,
    },
    ActorId, CanonicalSaveablePort, CellId, CommodityId, LedgerCommand, LedgerReceipt,
    ModRegistryPort, PhysicalLedgerPort, ProcessId, SettlementId, SettlementSummaryView,
    StockpileEntryView, StockpileId, SubsurfaceDepositTruthPort, SurfaceConditionsPort,
};

/// Cumulative global mass-conservation audit ledger in kilograms (`ENG-ECO-001`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MassConservationLedger {
    pub initial_mass_kg: f64,
    pub extracted_from_earth_kg: f64,
    pub subsistence_consumed_kg: f64,
    pub process_and_carrier_burned_kg: f64,
    pub spoiled_mass_kg: f64,
    pub tool_wear_mass_lost_kg: f64,
    pub skimmed_shrinkage_kg: f64,
    pub in_transit_cargo_kg: f64,
}

impl MassConservationLedger {
    /// Returns absolute mass balance discrepancy across all physical pools in kilograms.
    #[must_use]
    pub fn balance_error_kg(&self, current_stockpile_mass_kg: f64) -> f64 {
        let lhs = self.initial_mass_kg + self.extracted_from_earth_kg;
        let rhs = current_stockpile_mass_kg
            + self.in_transit_cargo_kg
            + self.subsistence_consumed_kg
            + self.process_and_carrier_burned_kg
            + self.spoiled_mass_kg
            + self.tool_wear_mass_lost_kg
            + self.skimmed_shrinkage_kg;
        (lhs - rhs).abs()
    }
}

/// Metadata tracking physical storage shelter protection for a named stockpile (`ENG-WTH-005`).
#[derive(Clone, Debug, PartialEq)]
pub struct StockpileLot {
    pub id: StockpileId,
    pub settlement: SettlementId,
    pub roof_protection_01: f32,
}

/// Physical commodity lot inside a spatial stockpile tagged with legal owner, purity, and wear (`ENG-ECO-001`).
#[derive(Clone, Debug, PartialEq)]
pub struct StockBatch {
    pub stockpile: StockpileId,
    pub settlement: SettlementId,
    pub commodity: CommodityId,
    pub owner: ActorId,
    pub mass_kg: f64,
    pub purity_01: f32,
    pub condition_01: f32,
    pub spoilage_age_hours: u32,
}

/// Active industrial, agricultural, mining, or construction facility owned by any legal actor (`ENG-ECO-004`).
#[derive(Clone, Debug, PartialEq)]
pub struct FacilityInstance {
    pub id: u32,
    pub settlement: SettlementId,
    pub process: ProcessId,
    pub owner: ActorId,
    pub assigned_workers: u32,
    pub tool_condition_01: f32,
    pub halted_reason: Option<String>,
    pub weather_idle_crew_hours: u64,
    pub completed_cycles: u64,
    pub total_output_kg: f64,
    pub last_daily_output_kg: f64,
    pub last_tool_used: Option<CommodityId>,
}

/// Settlement-level spatial economy record.
#[derive(Clone, Debug, PartialEq)]
pub struct SettlementEconomyState {
    pub summary: SettlementSummaryView,
    pub primary_stockpile: StockpileId,
    pub stockpiles: Vec<StockpileId>,
    pub shaft_water_level_01: f32,
    pub flood_halted_mines: u32,
}

/// Daily subsistence fulfillment ratios returned for cohort welfare transitions (`ENG-ECO-003`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SubsistenceSatisfaction {
    pub calorie_ratio_01: f32,
    pub water_ratio_01: f32,
    pub heating_ratio_01: f32,
    pub clothing_ratio_01: f32,
    pub combined_ratio_01: f32,
}

impl Default for SubsistenceSatisfaction {
    fn default() -> Self {
        Self {
            calorie_ratio_01: 1.0,
            water_ratio_01: 1.0,
            heating_ratio_01: 1.0,
            clothing_ratio_01: 1.0,
            combined_ratio_01: 1.0,
        }
    }
}

/// Modular physical goods ledger, production, tool-wear, and weather damage cartridge.
#[derive(Clone, Debug)]
pub struct EconomyLedger {
    settlements: Vec<SettlementEconomyState>,
    settlement_summaries: Vec<SettlementSummaryView>,
    hq_settlement: Option<SettlementId>,
    hq_cell: CellId,
    hq_sight_radius_cells: u8,
    stockpile_lots: Vec<StockpileLot>,
    batches: Vec<StockBatch>,
    facilities: Vec<FacilityInstance>,
    conservation: MassConservationLedger,
    next_facility_id: u32,
}

impl EconomyLedger {
    /// Constructs initial physical settlements, stockpiles, and facilities strictly from `ModRegistryPort`.
    #[must_use]
    pub fn from_registry(registry: &dyn ModRegistryPort) -> Self {
        let scenario = registry.scenario_init();
        let mut settlements = Vec::with_capacity(scenario.settlements.len());
        let mut settlement_summaries = Vec::with_capacity(scenario.settlements.len());
        let mut stockpile_lots = Vec::new();
        let mut batches = Vec::new();
        let mut facilities = Vec::new();
        let mut initial_mass_kg = 0.0_f64;
        let mut next_stockpile_id = 0_u32;
        let mut next_facility_id = 1_u32;
        let mut hq_settlement = None;
        let mut hq_cell = CellId(0);

        for (idx, seed) in scenario.settlements.iter().enumerate() {
            let sid = SettlementId(idx as u32);
            let owner = ActorId(1);
            let summary = SettlementSummaryView {
                id: sid,
                key: seed.key.clone(),
                display_name: seed.display_name.clone(),
                cell_id: CellId(seed.cell_index),
                owner,
            };
            if seed.key == scenario.hq_settlement_key {
                hq_settlement = Some(sid);
                hq_cell = summary.cell_id;
            }
            settlement_summaries.push(summary.clone());

            let primary_stockpile = StockpileId(next_stockpile_id);
            next_stockpile_id += 1;
            stockpile_lots.push(StockpileLot {
                id: primary_stockpile,
                settlement: sid,
                roof_protection_01: 0.96,
            });
            let mut sid_stockpiles = vec![primary_stockpile];

            for (s_idx, &(commodity, mass_kg, purity)) in seed.stockpiles.iter().enumerate() {
                if mass_kg <= 0.0 {
                    continue;
                }
                let roof = seed
                    .stockpile_roofs_01
                    .get(s_idx)
                    .copied()
                    .unwrap_or(0.95)
                    .clamp(0.0, 1.0);
                let target_stockpile = if roof >= 0.85 {
                    primary_stockpile
                } else {
                    let dump_id = StockpileId(next_stockpile_id);
                    next_stockpile_id += 1;
                    stockpile_lots.push(StockpileLot {
                        id: dump_id,
                        settlement: sid,
                        roof_protection_01: roof,
                    });
                    sid_stockpiles.push(dump_id);
                    dump_id
                };

                initial_mass_kg += mass_kg;
                Self::merge_batch_into_vec(
                    &mut batches,
                    StockBatch {
                        stockpile: target_stockpile,
                        settlement: sid,
                        commodity,
                        owner,
                        mass_kg,
                        purity_01: purity,
                        condition_01: 1.0,
                        spoilage_age_hours: 0,
                    },
                );
            }

            for (f_idx, &(process, workers)) in seed.initial_facilities.iter().enumerate() {
                let fac_owner = seed
                    .initial_facility_owners
                    .get(f_idx)
                    .copied()
                    .filter(|o| o.get() != 0)
                    .unwrap_or(owner);
                facilities.push(FacilityInstance {
                    id: next_facility_id,
                    settlement: sid,
                    process,
                    owner: fac_owner,
                    assigned_workers: workers,
                    tool_condition_01: 1.0,
                    halted_reason: None,
                    weather_idle_crew_hours: 0,
                    completed_cycles: 0,
                    total_output_kg: 0.0,
                    last_daily_output_kg: 0.0,
                    last_tool_used: None,
                });
                next_facility_id += 1;
            }

            settlements.push(SettlementEconomyState {
                summary,
                primary_stockpile,
                stockpiles: sid_stockpiles,
                shaft_water_level_01: seed.initial_shaft_water_01.clamp(0.0, 1.0),
                flood_halted_mines: 0,
            });
        }

        if hq_settlement.is_none() && !settlement_summaries.is_empty() {
            hq_settlement = Some(settlement_summaries[0].id);
            hq_cell = settlement_summaries[0].cell_id;
        }

        Self {
            settlements,
            settlement_summaries,
            hq_settlement,
            hq_cell,
            hq_sight_radius_cells: scenario.hq_sight_radius_cells.max(1),
            stockpile_lots,
            batches,
            facilities,
            conservation: MassConservationLedger {
                initial_mass_kg,
                ..MassConservationLedger::default()
            },
            next_facility_id,
        }
    }

    fn merge_batch_into_vec(batches: &mut Vec<StockBatch>, incoming: StockBatch) {
        if incoming.mass_kg <= 0.0 {
            return;
        }
        if let Some(existing) = batches.iter_mut().find(|b| {
            b.stockpile == incoming.stockpile
                && b.commodity == incoming.commodity
                && b.owner == incoming.owner
        }) {
            let total = existing.mass_kg + incoming.mass_kg;
            existing.purity_01 = ((f64::from(existing.purity_01) * existing.mass_kg
                + f64::from(incoming.purity_01) * incoming.mass_kg)
                / total) as f32;
            existing.condition_01 = ((f64::from(existing.condition_01) * existing.mass_kg
                + f64::from(incoming.condition_01) * incoming.mass_kg)
                / total) as f32;
            existing.mass_kg = total;
        } else {
            batches.push(incoming);
        }
    }

    /// Creates a new settlement with initial roof-rated stockpiles and facilities cleanly accounting initial mass.
    pub fn establish_settlement(
        &mut self,
        key: String,
        display_name: String,
        cell_id: CellId,
        owner: ActorId,
        initial_stockpiles: &[(CommodityId, f64, f32, f32)],
        initial_facilities: &[(ProcessId, u32, ActorId)],
    ) -> SettlementId {
        let sid = SettlementId(self.settlements.len() as u32);
        let summary = SettlementSummaryView {
            id: sid,
            key,
            display_name,
            cell_id,
            owner,
        };
        let mut next_lot = self.stockpile_lots.len() as u32;
        let primary_stockpile = StockpileId(next_lot);
        next_lot += 1;
        self.stockpile_lots.push(StockpileLot {
            id: primary_stockpile,
            settlement: sid,
            roof_protection_01: 0.96,
        });
        let mut sid_lots = vec![primary_stockpile];

        for &(commodity, mass_kg, purity, roof) in initial_stockpiles {
            if mass_kg <= 0.0 {
                continue;
            }
            let clamped_roof = roof.clamp(0.0, 1.0);
            let target_lot = if clamped_roof >= 0.85 {
                primary_stockpile
            } else {
                let dump = StockpileId(next_lot);
                next_lot += 1;
                self.stockpile_lots.push(StockpileLot {
                    id: dump,
                    settlement: sid,
                    roof_protection_01: clamped_roof,
                });
                sid_lots.push(dump);
                dump
            };
            self.conservation.initial_mass_kg += mass_kg;
            Self::merge_batch_into_vec(
                &mut self.batches,
                StockBatch {
                    stockpile: target_lot,
                    settlement: sid,
                    commodity,
                    owner,
                    mass_kg,
                    purity_01: purity,
                    condition_01: 1.0,
                    spoilage_age_hours: 0,
                },
            );
        }

        for &(process, workers, fac_owner) in initial_facilities {
            self.add_facility(sid, process, fac_owner, workers);
        }

        self.settlement_summaries.push(summary.clone());
        self.settlements.push(SettlementEconomyState {
            summary,
            primary_stockpile,
            stockpiles: sid_lots,
            shaft_water_level_01: 0.0,
            flood_halted_mines: 0,
        });
        sid
    }

    /// Adds a facility belonging to any actor (`ActorId`) at a settlement (`ENG-ECO-004`).
    pub fn add_facility(
        &mut self,
        settlement: SettlementId,
        process: ProcessId,
        owner: ActorId,
        assigned_workers: u32,
    ) -> u32 {
        let fid = self.next_facility_id;
        self.next_facility_id += 1;
        self.facilities.push(FacilityInstance {
            id: fid,
            settlement,
            process,
            owner,
            assigned_workers,
            tool_condition_01: 1.0,
            halted_reason: None,
            weather_idle_crew_hours: 0,
            completed_cycles: 0,
            total_output_kg: 0.0,
            last_daily_output_kg: 0.0,
            last_tool_used: None,
        });
        fid
    }

    /// Updates a settlement's planetary cell index during map placement initialization.
    pub fn relocate_settlement_cell(&mut self, sid: SettlementId, cell_id: CellId) {
        if let Some(st) = self.settlements.iter_mut().find(|s| s.summary.id == sid) {
            st.summary.cell_id = cell_id;
        }
        if let Some(sm) = self
            .settlement_summaries
            .iter_mut()
            .find(|s| s.id == sid)
        {
            sm.cell_id = cell_id;
        }
        if self.hq_settlement == Some(sid) {
            self.hq_cell = cell_id;
        }
    }

    /// Updates the player's physical HQ cell and optional resident settlement upon HQ relocation (`ENG-EPI-003`).
    pub fn set_hq_location(&mut self, cell_id: CellId) {
        self.hq_cell = cell_id;
        self.hq_settlement = self
            .settlement_summaries
            .iter()
            .find(|s| s.cell_id == cell_id)
            .map(|s| s.id);
    }

    /// Sets HQ direct optical observation halo radius in planetary grid rings.
    pub fn set_hq_sight_radius_cells(&mut self, radius: u8) {
        self.hq_sight_radius_cells = radius.max(1);
    }

    /// Scales initial starting stockpile masses uniformly for scenario archetypes while keeping conservation exact.
    pub fn scale_all_stockpiles_for_archetype(&mut self, factor: f64) {
        let clamped = factor.clamp(0.05, 5.0);
        let mut new_total = 0.0_f64;
        for b in &mut self.batches {
            b.mass_kg *= clamped;
            new_total += b.mass_kg;
        }
        self.conservation.initial_mass_kg = new_total;
    }

    /// Returns mass conservation totals.
    #[must_use]
    pub fn conservation_ledger(&self) -> &MassConservationLedger {
        &self.conservation
    }

    /// Returns absolute conservation discrepancy across all pools in kilograms (`ENG-ECO-001`).
    #[must_use]
    pub fn conservation_error_kg(&self) -> f64 {
        self.conservation
            .balance_error_kg(self.total_stockpile_mass_kg())
    }

    /// Returns total physical commodity mass stored across all settlement stockpiles.
    #[must_use]
    pub fn total_stockpile_mass_kg(&self) -> f64 {
        self.batches.iter().map(|b| b.mass_kg).sum()
    }

    /// Returns total commodity mass stored in a settlement's stockpiles.
    #[must_use]
    pub fn settlement_commodity_mass_kg(
        &self,
        sid: SettlementId,
        commodity: CommodityId,
        owner: Option<ActorId>,
    ) -> f64 {
        self.batches
            .iter()
            .filter(|b| {
                b.settlement == sid
                    && b.commodity == commodity
                    && owner.is_none_or(|o| b.owner == o)
            })
            .map(|b| b.mass_kg)
            .sum()
    }

    /// Returns stockpile roof metadata by stockpile ID.
    #[must_use]
    pub fn stockpile_lot(&self, id: StockpileId) -> Option<&StockpileLot> {
        self.stockpile_lots.iter().find(|l| l.id == id)
    }

    /// Returns immutable slice of all active facilities.
    #[must_use]
    pub fn facilities(&self) -> &[FacilityInstance] {
        &self.facilities
    }

    /// Returns settlement underground shaft water accumulation `[0.0, 1.0]`.
    #[must_use]
    pub fn settlement_shaft_water_level(&self, sid: SettlementId) -> f32 {
        self.settlements
            .iter()
            .find(|s| s.summary.id == sid)
            .map_or(0.0, |s| s.shaft_water_level_01)
    }

    /// Sets a settlement's underground shaft water level (used by scenario tests and pumps).
    pub fn set_settlement_shaft_water_level(&mut self, sid: SettlementId, level_01: f32) {
        if let Some(st) = self.settlements.iter_mut().find(|s| s.summary.id == sid) {
            st.shaft_water_level_01 = level_01.clamp(0.0, 1.0);
        }
    }

    /// Adjusts worker headcount on an existing facility (`ENG-CMD-004`).
    pub fn set_facility_workers(&mut self, facility_id: u32, workers: u32) -> bool {
        if let Some(f) = self.facilities.iter_mut().find(|f| f.id == facility_id) {
            f.assigned_workers = workers;
            true
        } else {
            false
        }
    }

    /// Atomically transfers physical mass between two stockpiles with zero loss (`ENG-ECO-001`).
    pub fn transfer_stock_atomic(
        &mut self,
        from: StockpileId,
        to: StockpileId,
        commodity: CommodityId,
        owner: ActorId,
        mass_kg: f64,
    ) -> bool {
        if mass_kg <= 0.0 || !mass_kg.is_finite() {
            return false;
        }
        let Some(src_idx) = self.batches.iter().position(|b| {
            b.stockpile == from && b.commodity == commodity && b.owner == owner && b.mass_kg >= mass_kg
        }) else {
            return false;
        };
        let Some(dst_settlement) = self
            .stockpile_lots
            .iter()
            .find(|l| l.id == to)
            .map(|l| l.settlement)
        else {
            return false;
        };
        let purity = self.batches[src_idx].purity_01;
        let condition = self.batches[src_idx].condition_01;
        let age = self.batches[src_idx].spoilage_age_hours;
        self.batches[src_idx].mass_kg -= mass_kg;
        if self.batches[src_idx].mass_kg <= 1e-9 {
            self.batches.remove(src_idx);
        }
        Self::merge_batch_into_vec(
            &mut self.batches,
            StockBatch {
                stockpile: to,
                settlement: dst_settlement,
                commodity,
                owner,
                mass_kg,
                purity_01: purity,
                condition_01: condition,
                spoilage_age_hours: age,
            },
        );
        true
    }

    fn debit_settlement_commodity(
        &mut self,
        sid: SettlementId,
        commodity: CommodityId,
        owner_filter: Option<ActorId>,
        requested_kg: f64,
    ) -> (f64, f32) {
        if requested_kg <= 0.0 {
            return (0.0, 1.0);
        }
        let mut remaining = requested_kg;
        let mut drawn_total = 0.0_f64;
        let mut weighted_purity = 0.0_f64;

        let mut i = 0;
        while i < self.batches.len() && remaining > 1e-9 {
            let matches = self.batches[i].settlement == sid
                && self.batches[i].commodity == commodity
                && owner_filter.is_none_or(|o| self.batches[i].owner == o)
                && self.batches[i].mass_kg > 0.0;
            if matches {
                let take = remaining.min(self.batches[i].mass_kg);
                weighted_purity += take * f64::from(self.batches[i].purity_01);
                drawn_total += take;
                remaining -= take;
                self.batches[i].mass_kg -= take;
                if self.batches[i].mass_kg <= 1e-9 {
                    self.batches.remove(i);
                    continue;
                }
            }
            i += 1;
        }
        let avg_purity = if drawn_total > 0.0 {
            (weighted_purity / drawn_total) as f32
        } else {
            1.0
        };
        (drawn_total, avg_purity)
    }

    /// Moves physical cargo out of origin settlement stockpiles into `in_transit` mass tracking (`ENG-LOG-004`).
    pub fn debit_cargo_for_transit(
        &mut self,
        sid: SettlementId,
        commodity: CommodityId,
        owner: ActorId,
        requested_kg: f64,
    ) -> Option<(f64, f32)> {
        let avail = self.settlement_commodity_mass_kg(sid, commodity, Some(owner));
        if avail + 1e-6 < requested_kg {
            return None;
        }
        let (drawn, purity) =
            self.debit_settlement_commodity(sid, commodity, Some(owner), requested_kg);
        self.conservation.in_transit_cargo_kg += drawn;
        Some((drawn, purity))
    }

    /// Loads carrier fodder, fuel, or spare parts from a settlement into transit reserve (`ENG-LOG-001`).
    pub fn load_fuel_into_transit(
        &mut self,
        sid: SettlementId,
        commodity: CommodityId,
        max_kg: f64,
    ) -> f64 {
        let (drawn, _) = self.debit_settlement_commodity(sid, commodity, None, max_kg);
        self.conservation.in_transit_cargo_kg += drawn;
        drawn
    }

    /// Accounts fuel or spare parts burned out of an active convoy's onboard transit reserve (`ENG-ECO-001`).
    pub fn record_onboard_transit_burn(&mut self, burned_kg: f64) {
        if burned_kg <= 0.0 {
            return;
        }
        let accounted = burned_kg.min(self.conservation.in_transit_cargo_kg);
        self.conservation.in_transit_cargo_kg -= accounted;
        self.conservation.process_and_carrier_burned_kg += accounted;
    }

    /// Credits physical cargo and unused onboard fuel into a settlement primary stockpile upon arrival (`ENG-LOG-004`).
    pub fn credit_cargo_arrival(
        &mut self,
        sid: SettlementId,
        commodity: CommodityId,
        owner: ActorId,
        mass_kg: f64,
        purity_01: f32,
    ) {
        if mass_kg <= 0.0 {
            return;
        }
        let primary = self
            .settlements
            .iter()
            .find(|s| s.summary.id == sid)
            .map(|s| s.primary_stockpile)
            .unwrap_or(StockpileId(0));
        let shifted = mass_kg.min(self.conservation.in_transit_cargo_kg);
        self.conservation.in_transit_cargo_kg -= shifted;
        Self::merge_batch_into_vec(
            &mut self.batches,
            StockBatch {
                stockpile: primary,
                settlement: sid,
                commodity,
                owner,
                mass_kg,
                purity_01,
                condition_01: 1.0,
                spoilage_age_hours: 0,
            },
        );
    }

    /// Hourly spoilage acceleration on unroofed perishables and underground mine seepage (`ENG-WTH-004`, `ENG-WTH-005`).
    pub fn apply_hourly_weather_and_spoilage(
        &mut self,
        registry: &dyn ModRegistryPort,
        weather: &dyn SurfaceConditionsPort,
    ) {
        let mut cell_by_sid = BTreeMap::new();
        let mut has_mine = BTreeMap::new();
        for fac in &self.facilities {
            if let Some(p) = registry.process_def(fac.process)
                && p.is_underground_mine
            {
                has_mine.insert(fac.settlement, true);
            }
        }

        for st in &mut self.settlements {
            cell_by_sid.insert(st.summary.id, st.summary.cell_id);
            if has_mine.get(&st.summary.id).copied().unwrap_or(false) {
                let surf = weather.surface(st.summary.cell_id);
                if surf.precip_mm_hr > 4.0 || surf.flood_stage_01 > 0.35 {
                    let inflow = surf.precip_mm_hr * 0.006 + (surf.flood_stage_01 - 0.35).max(0.0) * 0.035;
                    st.shaft_water_level_01 = (st.shaft_water_level_01 + inflow).clamp(0.0, 1.0);
                }
            }
        }

        let mut lot_roofs = BTreeMap::new();
        for lot in &self.stockpile_lots {
            lot_roofs.insert(lot.id, lot.roof_protection_01);
        }

        let mut i = 0;
        while i < self.batches.len() {
            let batch_sid = self.batches[i].settlement;
            let batch_cid = self.batches[i].commodity;
            let batch_stock = self.batches[i].stockpile;
            self.batches[i].spoilage_age_hours =
                self.batches[i].spoilage_age_hours.saturating_add(1);

            if let Some(cdef) = registry.commodity_def(batch_cid)
                && cdef.spoilage_rate_per_hour > 0.0
                && let Some(&cell) = cell_by_sid.get(&batch_sid)
            {
                let surf = weather.surface(cell);
                let roof = lot_roofs.get(&batch_stock).copied().unwrap_or(0.95);
                let exposed = f64::from((1.0 - roof).clamp(0.0, 1.0));
                let wet_mult =
                    1.0 + f64::from(surf.precip_mm_hr.max(0.0)) * 0.8 + f64::from(surf.mud_index_01) * 2.5;
                let temp_mult = if surf.temp_c > 12.0 {
                    1.0 + f64::from(surf.temp_c - 12.0) * 0.04
                } else {
                    0.85
                };
                let effective_rate =
                    (cdef.spoilage_rate_per_hour * temp_mult * (0.35 + 4.0 * exposed * wet_mult))
                        .min(0.05);
                let lost = self.batches[i].mass_kg * effective_rate;
                self.batches[i].mass_kg -= lost;
                self.conservation.spoiled_mass_kg += lost;
                if self.batches[i].mass_kg <= 1e-9 {
                    self.batches.remove(i);
                    continue;
                }
            }
            i += 1;
        }
    }

    /// Consumes cohort physical grain, water, heating fuel, and clothing from local settlement stocks (`ENG-ECO-003`).
    #[allow(clippy::too_many_arguments)]
    pub fn step_subsistence_for_settlement(
        &mut self,
        sid: SettlementId,
        grain_need_kg: f64,
        water_need_kg: f64,
        fuel_need_below_10c_kg: f64,
        clothing_need_kg: f64,
        ambient_temp_c: f32,
        registry: &dyn ModRegistryPort,
    ) -> SubsistenceSatisfaction {
        let grain_id = registry.commodity_id_by_key("raw_grain");
        let water_id = registry.commodity_id_by_key("potable_water");
        let charcoal_id = registry.commodity_id_by_key("charcoal");
        let timber_id = registry.commodity_id_by_key("timber");
        let clothing_id = registry.commodity_id_by_key("simple_clothing");

        let calorie_ratio_01 = if grain_need_kg > 1e-6 {
            if let Some(gid) = grain_id {
                let (drawn, _) = self.debit_settlement_commodity(sid, gid, None, grain_need_kg);
                self.conservation.subsistence_consumed_kg += drawn;
                (drawn / grain_need_kg) as f32
            } else {
                0.0
            }
        } else {
            1.0
        };

        let water_ratio_01 = if water_need_kg > 1e-6 {
            if let Some(wid) = water_id {
                let (drawn, _) = self.debit_settlement_commodity(sid, wid, None, water_need_kg);
                self.conservation.subsistence_consumed_kg += drawn;
                (drawn / water_need_kg) as f32
            } else {
                1.0
            }
        } else {
            1.0
        };

        let heating_ratio_01 = if ambient_temp_c < 10.0 && fuel_need_below_10c_kg > 1e-6 {
            let need = fuel_need_below_10c_kg;
            let mut burned = 0.0_f64;
            if let Some(cid) = charcoal_id {
                let (d, _) = self.debit_settlement_commodity(sid, cid, None, need);
                burned += d;
            }
            if burned < need
                && let Some(tid) = timber_id
            {
                let rem = (need - burned) * 1.8;
                let (d, _) = self.debit_settlement_commodity(sid, tid, None, rem);
                burned += d / 1.8;
                self.conservation.subsistence_consumed_kg += d - (d / 1.8);
            }
            self.conservation.subsistence_consumed_kg += burned;
            (burned / need).min(1.0) as f32
        } else {
            1.0
        };

        let clothing_ratio_01 = if clothing_need_kg > 1e-6 {
            if let Some(clid) = clothing_id {
                let (drawn, _) = self.debit_settlement_commodity(sid, clid, None, clothing_need_kg);
                self.conservation.subsistence_consumed_kg += drawn;
                (drawn / clothing_need_kg).min(1.0) as f32
            } else {
                1.0
            }
        } else {
            1.0
        };

        let combined_ratio_01 = (calorie_ratio_01 * 0.45
            + water_ratio_01 * 0.30
            + heating_ratio_01 * 0.15
            + clothing_ratio_01 * 0.10)
            .clamp(0.0, 1.0);

        SubsistenceSatisfaction {
            calorie_ratio_01,
            water_ratio_01,
            heating_ratio_01,
            clothing_ratio_01,
            combined_ratio_01,
        }
    }

    /// Executes daily production/extraction/construction processes across settlements
    /// honoring external recipes, tool wear, mine flooding, outdoor weather halts, crop stress,
    /// and actor ownership (`ENG-ECO-002..004`, `ENG-WTH-004..005`).
    #[allow(clippy::too_many_arguments)]
    pub fn step_facilities_for_settlement(
        &mut self,
        sid: SettlementId,
        registry: &dyn ModRegistryPort,
        weather: &dyn SurfaceConditionsPort,
        labor_efficiency_mult: f32,
        attention_efficiency_by_facility: &BTreeMap<u32, f32>,
        skimming_fraction_by_facility: &BTreeMap<u32, f32>,
        hidden_deposits: Option<&dyn SubsurfaceDepositTruthPort>,
    ) -> u32 {
        let Some(st_idx) = self.settlements.iter().position(|s| s.summary.id == sid) else {
            return 0;
        };
        let cell_id = self.settlements[st_idx].summary.cell_id;
        let primary_stock = self.settlements[st_idx].primary_stockpile;
        let surf = weather.surface(cell_id);

        let deposit_grade_bonus = hidden_deposits
            .map(|deps| {
                let list = deps.deposits_in_cell(cell_id);
                list.iter()
                    .map(|d| d.average_purity_01)
                    .fold(0.0_f32, f32::max)
            })
            .unwrap_or(0.5);

        let mut processed_count = 0_u32;
        self.settlements[st_idx].flood_halted_mines = 0;

        let fac_indices: Vec<usize> = self
            .facilities
            .iter()
            .enumerate()
            .filter_map(|(idx, f)| (f.settlement == sid).then_some(idx))
            .collect();

        for fac_idx in fac_indices {
            processed_count += 1;
            let proc_id = self.facilities[fac_idx].process;
            let owner = self.facilities[fac_idx].owner;
            let workers = self.facilities[fac_idx].assigned_workers;
            let fac_id = self.facilities[fac_idx].id;

            let Some(pdef) = registry.process_def(proc_id) else {
                continue;
            };
            self.facilities[fac_idx].last_daily_output_kg = 0.0;
            self.facilities[fac_idx].halted_reason = None;

            if workers == 0 {
                self.facilities[fac_idx].halted_reason = Some("no_workers".to_string());
                continue;
            }

            // Weather construction halt (`ENG-WTH-004`)
            if pdef.is_outdoor_construction
                && (surf.precip_mm_hr > 8.0
                    || surf.mud_index_01 >= pdef.weather_halt_mud_threshold
                    || surf.temp_c < pdef.min_temp_c)
            {
                self.facilities[fac_idx].halted_reason = Some("weather_halt".to_string());
                self.facilities[fac_idx].weather_idle_crew_hours += u64::from(workers) * 24;
                continue;
            }

            // Underground mine flooding halt (`ENG-WTH-004`)
            if pdef.is_underground_mine && self.settlements[st_idx].shaft_water_level_01 >= 0.60 {
                self.facilities[fac_idx].halted_reason = Some("mine_flooded".to_string());
                self.settlements[st_idx].flood_halted_mines += 1;
                continue;
            }

            // General heavy mud halt
            if !pdef.is_outdoor_construction
                && !pdef.is_mine_drainage
                && surf.mud_index_01 >= pdef.weather_halt_mud_threshold
            {
                self.facilities[fac_idx].halted_reason = Some("mud_halt".to_string());
                self.facilities[fac_idx].weather_idle_crew_hours += u64::from(workers) * 24;
                continue;
            }

            // Crop temperature & moisture stress (`ENG-WTH-005`)
            let climate_yield_factor = if pdef.is_crop_cultivation {
                if surf.temp_c < pdef.min_temp_c || surf.temp_c > pdef.max_temp_c {
                    self.facilities[fac_idx].halted_reason =
                        Some("crop_climate_failure".to_string());
                    continue;
                }
                let m_diff = (surf.mud_index_01 - pdef.optimal_moisture_01).abs();
                (1.0 - m_diff * 1.15).clamp(0.15, 1.0)
            } else {
                1.0
            };

            // Tool availability, hardness scaling, fallback, and wear (`ENG-ECO-002`)
            let mut active_tool_used = None;
            let mut tool_eff_mult = 1.0_f64;
            if let Some(primary_tool) = pdef.tool_commodity {
                let primary_avail =
                    self.settlement_commodity_mass_kg(sid, primary_tool, Some(owner));
                let chosen = if primary_avail >= 0.25 {
                    Some(primary_tool)
                } else if let Some(fb) = pdef.fallback_tool_commodity {
                    let fb_avail = self.settlement_commodity_mass_kg(sid, fb, Some(owner));
                    if fb_avail >= 0.25 {
                        Some(fb)
                    } else {
                        None
                    }
                } else {
                    None
                };

                let Some(tool_id) = chosen else {
                    self.facilities[fac_idx].halted_reason = Some("tool_exhausted".to_string());
                    self.facilities[fac_idx].tool_condition_01 = 0.0;
                    continue;
                };
                active_tool_used = Some(tool_id);
                if let Some(tdef) = registry.commodity_def(tool_id) {
                    tool_eff_mult = 0.65 + 0.35 * f64::from(tdef.tool_hardness_rating.clamp(0.2, 1.0));
                    let wear_kg = f64::from(workers)
                        * 24.0
                        * tdef.tool_wear_per_hour
                        * pdef.tool_wear_intensity;
                    let (worn, _) =
                        self.debit_settlement_commodity(sid, tool_id, Some(owner), wear_kg);
                    self.conservation.tool_wear_mass_lost_kg += worn;
                    let remaining_tool =
                        self.settlement_commodity_mass_kg(sid, tool_id, Some(owner));
                    self.facilities[fac_idx].tool_condition_01 =
                        (remaining_tool as f32 / 100.0).clamp(0.05, 1.0);
                }
            }
            self.facilities[fac_idx].last_tool_used = active_tool_used;

            let attn_mult = attention_efficiency_by_facility
                .get(&fac_id)
                .copied()
                .unwrap_or(1.0);
            let raw_scale = f64::from(workers)
                * (12.0 / pdef.workforce_hours_required.max(1.0))
                * f64::from(labor_efficiency_mult.clamp(0.05, 1.5))
                * f64::from(attn_mult.clamp(0.15, 1.25))
                * tool_eff_mult
                * f64::from(climate_yield_factor);

            let mut max_input_scale = raw_scale;
            for inp in &pdef.inputs {
                if inp.amount_kg <= 0.0 {
                    continue;
                }
                let avail = self.settlement_commodity_mass_kg(sid, inp.commodity, Some(owner));
                max_input_scale = max_input_scale.min(avail / inp.amount_kg);
            }
            if max_input_scale <= 1e-4 {
                self.facilities[fac_idx].halted_reason = Some("inputs_shortage".to_string());
                continue;
            }

            let mut total_in_kg = 0.0_f64;
            for inp in &pdef.inputs {
                let req = inp.amount_kg * max_input_scale;
                let (drawn, _) = self.debit_settlement_commodity(sid, inp.commodity, Some(owner), req);
                total_in_kg += drawn;
            }

            let skim_frac = f64::from(
                skimming_fraction_by_facility
                    .get(&fac_id)
                    .copied()
                    .unwrap_or(0.0)
                    .clamp(0.0, 0.45),
            );

            let mut total_out_gross_kg = 0.0_f64;
            let mut credited_out_kg = 0.0_f64;
            let purity_out = if pdef.is_underground_mine {
                (0.70 + deposit_grade_bonus * 0.28).clamp(0.5, 0.99)
            } else {
                0.92
            };

            for out in &pdef.outputs {
                let gross = out.amount_kg * max_input_scale;
                total_out_gross_kg += gross;
                let skimmed = gross * skim_frac;
                let net_credited = gross - skimmed;
                credited_out_kg += net_credited;
                self.conservation.skimmed_shrinkage_kg += skimmed;

                Self::merge_batch_into_vec(
                    &mut self.batches,
                    StockBatch {
                        stockpile: primary_stock,
                        settlement: sid,
                        commodity: out.commodity,
                        owner,
                        mass_kg: net_credited,
                        purity_01: purity_out,
                        condition_01: 1.0,
                        spoilage_age_hours: 0,
                    },
                );
            }

            // Exact mass-conservation accounting between physical inputs, outputs, slag, and fuel/ash (`ENG-ECO-001`).
            if total_out_gross_kg >= total_in_kg {
                self.conservation.extracted_from_earth_kg += total_out_gross_kg - total_in_kg;
            } else {
                let diff = total_in_kg - total_out_gross_kg;
                let slag_kg = if let Some(waste_id) = pdef.waste_commodity {
                    let s = diff * 0.55;
                    Self::merge_batch_into_vec(
                        &mut self.batches,
                        StockBatch {
                            stockpile: primary_stock,
                            settlement: sid,
                            commodity: waste_id,
                            owner,
                            mass_kg: s,
                            purity_01: 0.25,
                            condition_01: 1.0,
                            spoilage_age_hours: 0,
                        },
                    );
                    s
                } else {
                    0.0
                };
                self.conservation.process_and_carrier_burned_kg += diff - slag_kg;
            }

            if pdef.is_mine_drainage {
                let drained = (0.42 * (max_input_scale / raw_scale.max(1e-6))) as f32;
                self.settlements[st_idx].shaft_water_level_01 =
                    (self.settlements[st_idx].shaft_water_level_01 - drained).max(0.0);
            }

            self.facilities[fac_idx].completed_cycles += 1;
            self.facilities[fac_idx].last_daily_output_kg = credited_out_kg;
            self.facilities[fac_idx].total_output_kg += credited_out_kg;
        }

        processed_count
    }
}

impl PhysicalLedgerPort for EconomyLedger {
    fn settlements(&self) -> &[SettlementSummaryView] {
        &self.settlement_summaries
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
        self.batches
            .iter()
            .filter(|b| {
                b.stockpile == stock
                    && b.commodity == commodity
                    && owner.is_none_or(|o| b.owner == o)
            })
            .map(|b| b.mass_kg)
            .sum()
    }

    fn settlement_stockpiles(&self, settlement: SettlementId) -> &[StockpileId] {
        self.settlements
            .iter()
            .find(|s| s.summary.id == settlement)
            .map_or(&[], |s| s.stockpiles.as_slice())
    }

    fn settlement_inventory_entries(&self, settlement: SettlementId) -> Vec<StockpileEntryView> {
        self.batches
            .iter()
            .filter(|b| b.settlement == settlement)
            .map(|b| StockpileEntryView {
                stockpile: b.stockpile,
                settlement: b.settlement,
                commodity: b.commodity,
                owner: b.owner,
                mass_kg: b.mass_kg,
                average_purity: b.purity_01,
            })
            .collect()
    }

    fn apply_ledger_commands(&mut self, cmds: &[LedgerCommand]) -> Vec<LedgerReceipt> {
        let mut receipts = Vec::with_capacity(cmds.len());
        for cmd in cmds {
            if !cmd.delta_kg.is_finite() {
                receipts.push(LedgerReceipt {
                    applied: false,
                    resulting_mass_kg: 0.0,
                });
                continue;
            }
            let Some(lot_settlement) = self
                .stockpile_lots
                .iter()
                .find(|l| l.id == cmd.stockpile)
                .map(|l| l.settlement)
            else {
                receipts.push(LedgerReceipt {
                    applied: false,
                    resulting_mass_kg: 0.0,
                });
                continue;
            };
            if cmd.delta_kg < 0.0 {
                let req = -cmd.delta_kg;
                let current =
                    self.stockpile_mass_kg(cmd.stockpile, cmd.commodity, Some(cmd.owner));
                if current + 1e-9 < req {
                    receipts.push(LedgerReceipt {
                        applied: false,
                        resulting_mass_kg: current,
                    });
                    continue;
                }
                let mut remaining = req;
                let mut i = 0;
                while i < self.batches.len() && remaining > 1e-9 {
                    if self.batches[i].stockpile == cmd.stockpile
                        && self.batches[i].commodity == cmd.commodity
                        && self.batches[i].owner == cmd.owner
                    {
                        let take = remaining.min(self.batches[i].mass_kg);
                        self.batches[i].mass_kg -= take;
                        remaining -= take;
                        self.conservation.subsistence_consumed_kg += take;
                        if self.batches[i].mass_kg <= 1e-9 {
                            self.batches.remove(i);
                            continue;
                        }
                    }
                    i += 1;
                }
                let after = self.stockpile_mass_kg(cmd.stockpile, cmd.commodity, Some(cmd.owner));
                receipts.push(LedgerReceipt {
                    applied: true,
                    resulting_mass_kg: after,
                });
            } else {
                self.conservation.extracted_from_earth_kg += cmd.delta_kg;
                Self::merge_batch_into_vec(
                    &mut self.batches,
                    StockBatch {
                        stockpile: cmd.stockpile,
                        settlement: lot_settlement,
                        commodity: cmd.commodity,
                        owner: cmd.owner,
                        mass_kg: cmd.delta_kg,
                        purity_01: 0.90,
                        condition_01: 1.0,
                        spoilage_age_hours: 0,
                    },
                );
                let after = self.stockpile_mass_kg(cmd.stockpile, cmd.commodity, Some(cmd.owner));
                receipts.push(LedgerReceipt {
                    applied: true,
                    resulting_mass_kg: after,
                });
            }
        }
        receipts
    }
}

impl CanonicalSaveablePort for EconomyLedger {
    fn cartridge_tag(&self) -> &'static str {
        "sim-economy"
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        match self.hq_settlement {
            Some(sid) => {
                write_u8(out, 1);
                write_u32_le(out, sid.get());
            }
            None => write_u8(out, 0),
        }
        write_u32_le(out, self.hq_cell.get());
        write_u8(out, self.hq_sight_radius_cells);
        write_u32_le(out, self.next_facility_id);

        write_f64_bits(out, self.conservation.initial_mass_kg);
        write_f64_bits(out, self.conservation.extracted_from_earth_kg);
        write_f64_bits(out, self.conservation.subsistence_consumed_kg);
        write_f64_bits(out, self.conservation.process_and_carrier_burned_kg);
        write_f64_bits(out, self.conservation.spoiled_mass_kg);
        write_f64_bits(out, self.conservation.tool_wear_mass_lost_kg);
        write_f64_bits(out, self.conservation.skimmed_shrinkage_kg);
        write_f64_bits(out, self.conservation.in_transit_cargo_kg);

        write_u32_le(out, self.settlements.len() as u32);
        for st in &self.settlements {
            write_u32_le(out, st.summary.id.get());
            write_str(out, &st.summary.key);
            write_str(out, &st.summary.display_name);
            write_u32_le(out, st.summary.cell_id.get());
            write_u32_le(out, st.summary.owner.get());
            write_u32_le(out, st.primary_stockpile.get());
            write_f32_bits(out, st.shaft_water_level_01);
            write_u32_le(out, st.flood_halted_mines);
            write_u32_le(out, st.stockpiles.len() as u32);
            for s in &st.stockpiles {
                write_u32_le(out, s.get());
            }
        }

        write_u32_le(out, self.stockpile_lots.len() as u32);
        for lot in &self.stockpile_lots {
            write_u32_le(out, lot.id.get());
            write_u32_le(out, lot.settlement.get());
            write_f32_bits(out, lot.roof_protection_01);
        }

        write_u32_le(out, self.batches.len() as u32);
        for b in &self.batches {
            write_u32_le(out, b.stockpile.get());
            write_u32_le(out, b.settlement.get());
            write_u16_le(out, b.commodity.get());
            write_u32_le(out, b.owner.get());
            write_f64_bits(out, b.mass_kg);
            write_f32_bits(out, b.purity_01);
            write_f32_bits(out, b.condition_01);
            write_u32_le(out, b.spoilage_age_hours);
        }

        write_u32_le(out, self.facilities.len() as u32);
        for f in &self.facilities {
            write_u32_le(out, f.id);
            write_u32_le(out, f.settlement.get());
            write_u16_le(out, f.process.get());
            write_u32_le(out, f.owner.get());
            write_u32_le(out, f.assigned_workers);
            write_f32_bits(out, f.tool_condition_01);
            match &f.halted_reason {
                Some(r) => {
                    write_u8(out, 1);
                    write_str(out, r);
                }
                None => write_u8(out, 0),
            }
            write_u64_le(out, f.weather_idle_crew_hours);
            write_u64_le(out, f.completed_cycles);
            write_f64_bits(out, f.total_output_kg);
            write_f64_bits(out, f.last_daily_output_kg);
            match f.last_tool_used {
                Some(t) => {
                    write_u8(out, 1);
                    write_u16_le(out, t.get());
                }
                None => write_u8(out, 0),
            }
        }
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        self.hq_settlement = if read_u8(input)? != 0 {
            Some(SettlementId(read_u32_le(input)?))
        } else {
            None
        };
        self.hq_cell = CellId(read_u32_le(input)?);
        self.hq_sight_radius_cells = read_u8(input)?;
        self.next_facility_id = read_u32_le(input)?;

        self.conservation = MassConservationLedger {
            initial_mass_kg: read_f64_bits(input)?,
            extracted_from_earth_kg: read_f64_bits(input)?,
            subsistence_consumed_kg: read_f64_bits(input)?,
            process_and_carrier_burned_kg: read_f64_bits(input)?,
            spoiled_mass_kg: read_f64_bits(input)?,
            tool_wear_mass_lost_kg: read_f64_bits(input)?,
            skimmed_shrinkage_kg: read_f64_bits(input)?,
            in_transit_cargo_kg: read_f64_bits(input)?,
        };

        let scount = read_u32_le(input)? as usize;
        self.settlements.clear();
        self.settlement_summaries.clear();
        for _ in 0..scount {
            let id = SettlementId(read_u32_le(input)?);
            let key = read_str(input)?;
            let display_name = read_str(input)?;
            let cell_id = CellId(read_u32_le(input)?);
            let owner = ActorId(read_u32_le(input)?);
            let summary = SettlementSummaryView {
                id,
                key,
                display_name,
                cell_id,
                owner,
            };
            let primary_stockpile = StockpileId(read_u32_le(input)?);
            let shaft_water_level_01 = read_f32_bits(input)?;
            let flood_halted_mines = read_u32_le(input)?;
            let lcount = read_u32_le(input)? as usize;
            let mut stockpiles = Vec::with_capacity(lcount);
            for _ in 0..lcount {
                stockpiles.push(StockpileId(read_u32_le(input)?));
            }
            self.settlement_summaries.push(summary.clone());
            self.settlements.push(SettlementEconomyState {
                summary,
                primary_stockpile,
                stockpiles,
                shaft_water_level_01,
                flood_halted_mines,
            });
        }

        let lot_count = read_u32_le(input)? as usize;
        self.stockpile_lots.clear();
        for _ in 0..lot_count {
            self.stockpile_lots.push(StockpileLot {
                id: StockpileId(read_u32_le(input)?),
                settlement: SettlementId(read_u32_le(input)?),
                roof_protection_01: read_f32_bits(input)?,
            });
        }

        let bcount = read_u32_le(input)? as usize;
        self.batches.clear();
        for _ in 0..bcount {
            self.batches.push(StockBatch {
                stockpile: StockpileId(read_u32_le(input)?),
                settlement: SettlementId(read_u32_le(input)?),
                commodity: CommodityId(read_u16_le(input)?),
                owner: ActorId(read_u32_le(input)?),
                mass_kg: read_f64_bits(input)?,
                purity_01: read_f32_bits(input)?,
                condition_01: read_f32_bits(input)?,
                spoilage_age_hours: read_u32_le(input)?,
            });
        }

        let fcount = read_u32_le(input)? as usize;
        self.facilities.clear();
        for _ in 0..fcount {
            let id = read_u32_le(input)?;
            let settlement = SettlementId(read_u32_le(input)?);
            let process = ProcessId(read_u16_le(input)?);
            let owner = ActorId(read_u32_le(input)?);
            let assigned_workers = read_u32_le(input)?;
            let tool_condition_01 = read_f32_bits(input)?;
            let halted_reason = if read_u8(input)? != 0 {
                Some(read_str(input)?)
            } else {
                None
            };
            let weather_idle_crew_hours = read_u64_le(input)?;
            let completed_cycles = read_u64_le(input)?;
            let total_output_kg = read_f64_bits(input)?;
            let last_daily_output_kg = read_f64_bits(input)?;
            let last_tool_used = if read_u8(input)? != 0 {
                Some(CommodityId(read_u16_le(input)?))
            } else {
                None
            };
            self.facilities.push(FacilityInstance {
                id,
                settlement,
                process,
                owner,
                assigned_workers,
                tool_condition_01,
                halted_reason,
                weather_idle_crew_hours,
                completed_cycles,
                total_output_kg,
                last_daily_output_kg,
                last_tool_used,
            });
        }
        Ok(())
    }
}
