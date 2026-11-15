//! M5 Plug Cartridge: Autonomous Non-State Economy, Governance Transition & Bureaucratic Friction.
//! Communicates strictly through `sim-spine` ports with zero sibling crate coupling.

#![deny(clippy::disallowed_types)]

use std::collections::BTreeMap;

use sim_spine::codec::{
    read_f32_bits, read_f64_bits, read_str, read_u8, read_u16_le, read_u32_le, read_u64_le,
    write_f32_bits, write_f64_bits, write_str, write_u8, write_u16_le, write_u32_le, write_u64_le,
};
use sim_spine::ids::{ActorId, CharacterId, CommodityId, EdgeId, SettlementId, Tick};
use sim_spine::plugs::{
    ArbitrageOrderRequest, EdgeCongestionView, GovernanceFrictionPlugPort, GovernanceStateView,
    InformalMarketView, NonStateActorKind, NonStateActorView, NonStateEconomyPlugPort,
};
use sim_spine::ports::{
    CanonicalSaveablePort, LogisticsPort, ModRegistryPort, PhysicalLedgerPort,
    PlanetGeometryPort,
};

#[derive(Clone, Debug)]
struct PolicyState {
    key: String,
    display_name: String,
    tax_or_tribute_rate: f32,
    patent_duration_years: u32,
}

/// Unified M5 plug state implementing `NonStateEconomyPlugPort` and `GovernanceFrictionPlugPort`.
#[derive(Clone, Debug)]
pub struct PrivateEconomyPlugin {
    active_model_key: String,
    active_model_name: String,
    previous_model_key: Option<String>,
    base_attention_capacity: f32,
    clerical_overhead_per_mandate: f32,
    hierarchy_hop_latency_hours: u32,
    adaptation_progress_01: f32,
    active_policy: PolicyState,
    skimming_leakage_fraction: f32,
    bureaucratic_capture_events: u64,
    chartered_carriers_total: u32,
    actors: Vec<NonStateActorView>,
    markets: Vec<InformalMarketView>,
    congestion_by_edge: BTreeMap<EdgeId, EdgeCongestionView>,
    congested_list: Vec<EdgeCongestionView>,
    pending_arbitrage: Vec<ArbitrageOrderRequest>,
    last_stepped_tick: Tick,
}

impl Default for PrivateEconomyPlugin {
    fn default() -> Self {
        Self {
            active_model_key: String::new(),
            active_model_name: String::new(),
            previous_model_key: None,
            base_attention_capacity: 100.0,
            clerical_overhead_per_mandate: 4.0,
            hierarchy_hop_latency_hours: 2,
            adaptation_progress_01: 1.0,
            active_policy: PolicyState {
                key: String::new(),
                display_name: String::new(),
                tax_or_tribute_rate: 0.0,
                patent_duration_years: 0,
            },
            skimming_leakage_fraction: 0.0,
            bureaucratic_capture_events: 0,
            chartered_carriers_total: 0,
            actors: Vec::new(),
            markets: Vec::new(),
            congestion_by_edge: BTreeMap::new(),
            congested_list: Vec::new(),
            pending_arbitrage: Vec::new(),
            last_stepped_tick: Tick(0),
        }
    }
}

fn kind_to_u8(k: NonStateActorKind) -> u8 {
    k as u8
}

fn u8_to_kind(v: u8) -> Result<NonStateActorKind, String> {
    match v {
        0 => Ok(NonStateActorKind::MerchantHouse),
        1 => Ok(NonStateActorKind::CraftGuild),
        2 => Ok(NonStateActorKind::ClanEstate),
        3 => Ok(NonStateActorKind::CharteredCompany),
        4 => Ok(NonStateActorKind::ReligiousFoundation),
        5 => Ok(NonStateActorKind::IndependentSmallholders),
        _ => Err(format!("Invalid NonStateActorKind tag {v}")),
    }
}

impl PrivateEconomyPlugin {
    /// Seeds M5 governance model, fiscal policy, non-state actors, and markets strictly from `ModRegistryPort`.
    #[must_use]
    pub fn from_registry(registry: &dyn ModRegistryPort, settlement_ids: &[SettlementId]) -> Self {
        let scenario = registry.scenario_init();
        let gov = registry
            .governance_models()
            .iter()
            .find(|g| {
                (g.base_attention_capacity - scenario.governance_base_attention).abs() < 1e-3
                    && (g.clerical_overhead_per_mandate - scenario.clerical_overhead_per_mandate)
                        .abs()
                        < 1e-3
                    && g.hierarchy_hop_latency_hours == scenario.hierarchy_hop_latency_hours
            })
            .or_else(|| registry.governance_models().first());
        let (model_key, model_name, base_att, overhead, hop_lat) = match gov {
            Some(g) => (
                g.key.clone(),
                g.display_name.clone(),
                g.base_attention_capacity,
                g.clerical_overhead_per_mandate,
                g.hierarchy_hop_latency_hours,
            ),
            None => (
                "default_council".to_string(),
                "Default Council".to_string(),
                scenario.governance_base_attention,
                scenario.clerical_overhead_per_mandate,
                scenario.hierarchy_hop_latency_hours,
            ),
        };

        let policy = registry.policy_laws().first();
        let active_policy = match policy {
            Some(p) => PolicyState {
                key: p.key.clone(),
                display_name: p.display_name.clone(),
                tax_or_tribute_rate: p.tax_or_tribute_rate,
                patent_duration_years: p.patent_duration_years,
            },
            None => PolicyState {
                key: "baseline".to_string(),
                display_name: "Baseline Charter".to_string(),
                tax_or_tribute_rate: 0.05,
                patent_duration_years: 0,
            },
        };

        let mut primary_comms = Vec::new();
        for idx in 0..registry.commodity_count() {
            let cid = CommodityId(idx as u16);
            if registry.commodity_def(cid).is_some() {
                primary_comms.push(cid);
            }
        }
        let kinds = [
            NonStateActorKind::MerchantHouse,
            NonStateActorKind::CraftGuild,
            NonStateActorKind::ClanEstate,
            NonStateActorKind::CharteredCompany,
            NonStateActorKind::ReligiousFoundation,
            NonStateActorKind::IndependentSmallholders,
        ];

        let mut actors = Vec::new();
        let mut next_actor_id = 100u32;
        for &sid in settlement_ids {
            for (k_idx, &kind) in kinds.iter().enumerate() {
                let held_commodity = primary_comms
                    .get((sid.0 as usize + k_idx) % primary_comms.len().max(1))
                    .copied();
                let owned_carriers = match kind {
                    NonStateActorKind::MerchantHouse | NonStateActorKind::CharteredCompany => 4,
                    NonStateActorKind::ClanEstate | NonStateActorKind::CraftGuild => 2,
                    _ => 1,
                };
                let label = format!("{} (Sector {})", kind.as_str(), sid.0);
                actors.push(NonStateActorView {
                    actor_id: ActorId(next_actor_id),
                    name: label,
                    kind,
                    home_settlement: sid,
                    working_capital_coins: 1200.0 + f64::from(owned_carriers) * 250.0,
                    held_commodity,
                    held_mass_kg: 450.0,
                    hoarded_mass_kg: 0.0,
                    private_carriers_owned: owned_carriers,
                    chartered_to_state_carriers: 0,
                    compliance_01: 0.92,
                });
                next_actor_id += 1;
            }
        }

        let mut markets = Vec::new();
        for &sid in settlement_ids {
            for &cid in primary_comms.iter().take(4) {
                markets.push(InformalMarketView {
                    settlement: sid,
                    commodity: cid,
                    clearing_price_index: 1.0,
                    black_market_premium_01: 0.0,
                    black_market_leakage_kg: 0.0,
                    hoarded_reserve_kg: 0.0,
                });
            }
        }

        Self {
            active_model_key: model_key,
            active_model_name: model_name,
            previous_model_key: None,
            base_attention_capacity: base_att,
            clerical_overhead_per_mandate: overhead,
            hierarchy_hop_latency_hours: hop_lat,
            adaptation_progress_01: 1.0,
            active_policy,
            skimming_leakage_fraction: 0.0,
            bureaucratic_capture_events: 0,
            chartered_carriers_total: 0,
            actors,
            markets,
            congestion_by_edge: BTreeMap::new(),
            congested_list: Vec::new(),
            pending_arbitrage: Vec::new(),
            last_stepped_tick: Tick(0),
        }
    }

    /// Switches institutional governance model (`ENG-CMD-006.1`, `ENG-CMD-006.2`), incurring temporary adaptation drag.
    pub fn switch_governance_model(
        &mut self,
        model_key: &str,
        registry: &dyn ModRegistryPort,
    ) -> Result<(), String> {
        let model = registry
            .governance_models()
            .iter()
            .find(|g| g.key == model_key)
            .ok_or_else(|| format!("Unknown governance model key '{model_key}'"))?;
        if self.active_model_key == model.key {
            return Ok(());
        }
        self.previous_model_key = Some(self.active_model_key.clone());
        self.active_model_key = model.key.clone();
        self.active_model_name = model.display_name.clone();
        self.base_attention_capacity = model.base_attention_capacity;
        self.clerical_overhead_per_mandate = model.clerical_overhead_per_mandate;
        self.hierarchy_hop_latency_hours = model.hierarchy_hop_latency_hours;
        // Switching institutions creates a transition dip across ministries until stabilization.
        self.adaptation_progress_01 = 0.20;
        Ok(())
    }

    /// Enacts a fiscal policy law or tax override (`ENG-ECO-007`).
    pub fn enact_policy(
        &mut self,
        policy_key: &str,
        rate_override_01: Option<f32>,
        registry: &dyn ModRegistryPort,
    ) -> Result<(), String> {
        let law = registry
            .policy_laws()
            .iter()
            .find(|p| p.key == policy_key)
            .ok_or_else(|| format!("Unknown policy law key '{policy_key}'"))?;
        let rate = rate_override_01
            .unwrap_or(law.tax_or_tribute_rate)
            .clamp(0.0, 0.90);
        self.active_policy = PolicyState {
            key: law.key.clone(),
            display_name: law.display_name.clone(),
            tax_or_tribute_rate: rate,
            patent_duration_years: law.patent_duration_years,
        };
        Ok(())
    }

    /// Requisitions private freight carriers at `target_settlement` for state logistics (`ENG-LOG-006`).
    pub fn charter_private_carriers(&mut self, target_settlement: SettlementId, count: u32) -> u32 {
        let mut remaining = count;
        let mut chartered_now = 0u32;
        for actor in &mut self.actors {
            if remaining == 0 {
                break;
            }
            if actor.home_settlement != target_settlement || actor.private_carriers_owned == 0 {
                continue;
            }
            let take = remaining.min(actor.private_carriers_owned);
            actor.private_carriers_owned -= take;
            actor.chartered_to_state_carriers += take;
            actor.working_capital_coins += f64::from(take) * 45.0;
            remaining -= take;
            chartered_now += take;
        }
        self.chartered_carriers_total += chartered_now;
        chartered_now
    }

    /// Returns the total private carriers still available for autonomous merchant arbitrage.
    #[must_use]
    pub fn available_private_carriers(&self) -> u32 {
        self.actors.iter().map(|a| a.private_carriers_owned).sum()
    }

    /// Evaluates governance transition friction, edge congestion, informal price discovery,
    /// hoarding, black-market leakage, and inter-settlement arbitrage every tick (`ENG-CMD-006`, `ENG-ECO-005..007`, `ENG-LOG-005`).
    #[allow(clippy::too_many_arguments)]
    pub fn step_with_world_context(
        &mut self,
        tick: Tick,
        ledger: &dyn PhysicalLedgerPort,
        logistics: &dyn LogisticsPort,
        planet: &dyn PlanetGeometryPort,
        registry: &dyn ModRegistryPort,
        char_skimming_samples: &[(CharacterId, f32)],
        overseer_settlement: SettlementId,
        default_infra_throughput_tph: f32,
    ) {
        self.last_stepped_tick = tick;

        // 1. Institutional adaptation recovery over ~120 hours (`ENG-CMD-006.2`).
        if self.adaptation_progress_01 < 1.0 {
            self.adaptation_progress_01 = (self.adaptation_progress_01 + 1.0 / 120.0).min(1.0);
            if self.adaptation_progress_01 >= 0.999 {
                self.previous_model_key = None;
            }
        }

        // 2. Bureaucratic skimming & capture scaling (`ENG-CMD-006.3`).
        let avg_greed = if char_skimming_samples.is_empty() {
            0.12
        } else {
            let sum: f32 = char_skimming_samples.iter().map(|&(_, g)| g).sum();
            sum / (char_skimming_samples.len() as f32)
        };
        let latency_factor = (self.hierarchy_hop_latency_hours as f32 / 8.0).clamp(0.05, 1.5);
        let transition_gap = 1.0 - self.adaptation_progress_01;
        let oversight = self.oversight_efficiency_multiplier(overseer_settlement, CharacterId(1));
        self.skimming_leakage_fraction =
            (avg_greed * (0.25 + self.active_policy.tax_or_tribute_rate + latency_factor * 0.4 + transition_gap * 0.5)
                / oversight.max(0.30))
            .clamp(0.0, 0.45);
        if self.skimming_leakage_fraction >= 0.14 && tick.0.is_multiple_of(24) {
            self.bureaucratic_capture_events += 1;
        }

        // 3. Edge congestion vs throughput capacity (`ENG-LOG-005.1`, `ENG-LOG-005.2`).
        let mut flow_by_edge_kg: BTreeMap<EdgeId, f64> = BTreeMap::new();
        for convoy in logistics.all_active_convoys() {
            let footprint_kg = match convoy.channel {
                sim_spine::TransitChannelKind::PhysicalCourier
                | sim_spine::TransitChannelKind::SurveyExpedition => 50.0,
                _ => convoy.cargo_mass_kg.max(150.0) + 180.0,
            };
            let mut active_edge = None;
            for w in convoy.planned_route.windows(2) {
                if w[0] == convoy.current_cell
                    && let Some(eid) = planet.cell_edge(w[0], w[1])
                {
                    active_edge = Some(eid);
                    break;
                }
            }
            if active_edge.is_none() {
                active_edge = planet.cell_edge(convoy.current_cell, convoy.destination_cell);
            }
            if let Some(eid) = active_edge {
                *flow_by_edge_kg.entry(eid).or_insert(0.0) += footprint_kg;
            }
        }
        self.congestion_by_edge.clear();
        let cap_t = default_infra_throughput_tph.max(0.5);
        for (eid, kg) in flow_by_edge_kg {
            let flow_t = (kg / 1000.0) as f32;
            let mult = if flow_t > cap_t {
                (1.0 + 0.65 * ((flow_t / cap_t) - 1.0)).clamp(1.0, 3.8)
            } else {
                1.0
            };
            self.congestion_by_edge.insert(
                eid,
                EdgeCongestionView {
                    edge_id: eid,
                    hourly_flow_tonnes: flow_t,
                    capacity_tonnes: cap_t,
                    congestion_multiplier: mult,
                },
            );
        }
        self.congested_list = self.congestion_by_edge.values().cloned().collect();

        // 4. Clearing price discovery, survival hoarding, and black-market leakage (`ENG-ECO-005`, `ENG-ECO-006`).
        let coercive_pressure = (self.active_policy.tax_or_tribute_rate * 1.4
            + if self.active_policy.key.contains("rationing") { 0.35 } else { 0.0 })
        .clamp(0.0, 1.0);

        for mkt in &mut self.markets {
            let public_kg: f64 = ledger
                .settlement_stockpiles(mkt.settlement)
                .iter()
                .map(|&sp| ledger.stockpile_mass_kg(sp, mkt.commodity, None))
                .sum();
            let scarcity = (1.0 - (public_kg / 2000.0)).clamp(0.0, 1.0) as f32;
            mkt.clearing_price_index =
                (0.75 + 2.4 * scarcity + 0.8 * self.active_policy.tax_or_tribute_rate)
                    .clamp(0.45, 5.5);

            // Hoarding & black-market premium increase when coercion OR acute scarcity pushes compliance down.
            let target_hoard_ratio = if coercive_pressure >= 0.30 || scarcity >= 0.65 {
                (0.15 + 0.45 * coercive_pressure + 0.30 * scarcity).clamp(0.0, 0.75)
            } else {
                0.02
            };
            let mut held_sum = 0.0;
            let mut hoarded_sum = 0.0;
            for actor in &mut self.actors {
                if actor.home_settlement == mkt.settlement && actor.held_commodity == Some(mkt.commodity) {
                    let total_owned = actor.held_mass_kg + actor.hoarded_mass_kg;
                    let desired_hoard = total_owned * f64::from(target_hoard_ratio);
                    let delta = (desired_hoard - actor.hoarded_mass_kg) * 0.25;
                    actor.hoarded_mass_kg = (actor.hoarded_mass_kg + delta).clamp(0.0, total_owned);
                    actor.held_mass_kg = (total_owned - actor.hoarded_mass_kg).max(0.0);
                    actor.compliance_01 = (1.0 - coercive_pressure * 0.65).clamp(0.25, 0.98);
                    held_sum += actor.held_mass_kg;
                    hoarded_sum += actor.hoarded_mass_kg;
                }
            }
            mkt.hoarded_reserve_kg = hoarded_sum;
            mkt.black_market_premium_01 =
                (coercive_pressure * 0.55 + scarcity * 0.40).clamp(0.0, 1.5);
            mkt.black_market_leakage_kg = if mkt.black_market_premium_01 >= 0.18 {
                (hoarded_sum * 0.08 + held_sum * 0.02).min(hoarded_sum.max(12.0))
            } else {
                0.0
            };
        }

        // 5. Private inter-settlement arbitrage requests on daily cadence (`ENG-ECO-005.3`, `ENG-LOG-006`).
        if tick.0.is_multiple_of(24) && registry.commodity_count() > 0 {
            let mut by_comm: BTreeMap<CommodityId, Vec<&InformalMarketView>> = BTreeMap::new();
            for mkt in &self.markets {
                by_comm.entry(mkt.commodity).or_default().push(mkt);
            }
            for (cid, list) in by_comm {
                if list.len() < 2 {
                    continue;
                }
                let mut sorted = list;
                sorted.sort_by(|a, b| a.clearing_price_index.total_cmp(&b.clearing_price_index));
                let low = sorted[0];
                let high = sorted[sorted.len() - 1];
                let margin = high.clearing_price_index - low.clearing_price_index;
                if margin > 0.12 + self.active_policy.tax_or_tribute_rate
                    && let Some(merchant) = self.actors.iter_mut().find(|a| {
                        a.home_settlement == low.settlement
                            && a.private_carriers_owned > 0
                            && a.held_mass_kg > 20.0
                    })
                {
                    let cargo = (f64::from(merchant.private_carriers_owned) * 40.0)
                        .min(merchant.held_mass_kg * 0.35);
                    if cargo >= 10.0 {
                        merchant.working_capital_coins += cargo * f64::from(margin) * 0.4;
                        self.pending_arbitrage.push(ArbitrageOrderRequest {
                            origin_settlement: low.settlement,
                            destination_settlement: high.settlement,
                            commodity: cid,
                            mass_kg: cargo,
                            owner: merchant.actor_id,
                            expected_margin_index: margin,
                        });
                    }
                }
            }
        }
    }
}

impl NonStateEconomyPlugPort for PrivateEconomyPlugin {
    fn edge_congestion_multiplier(&self, edge: EdgeId) -> f32 {
        self.congestion_by_edge
            .get(&edge)
            .map(|c| c.congestion_multiplier)
            .unwrap_or(1.0)
    }

    fn step_private_actors(&mut self, tick: Tick) {
        self.last_stepped_tick = tick;
    }

    fn non_state_actors(&self) -> &[NonStateActorView] {
        &self.actors
    }

    fn informal_markets(&self) -> &[InformalMarketView] {
        &self.markets
    }

    fn congested_edges(&self) -> &[EdgeCongestionView] {
        &self.congested_list
    }

    fn drain_arbitrage_requests(&mut self) -> Vec<ArbitrageOrderRequest> {
        std::mem::take(&mut self.pending_arbitrage)
    }

    fn total_chartered_carriers(&self) -> u32 {
        self.chartered_carriers_total
    }
}

impl GovernanceFrictionPlugPort for PrivateEconomyPlugin {
    fn oversight_efficiency_multiplier(
        &self,
        _settlement: SettlementId,
        _overseer: CharacterId,
    ) -> f32 {
        let transition_mult = 0.55 + 0.45 * self.adaptation_progress_01;
        let latency_penalty = 1.0 / (1.0 + 0.04 * (self.hierarchy_hop_latency_hours as f32));
        (transition_mult * latency_penalty).clamp(0.35, 1.15)
    }

    fn governance_state(&self) -> Option<GovernanceStateView> {
        Some(GovernanceStateView {
            active_model_key: self.active_model_key.clone(),
            active_model_name: self.active_model_name.clone(),
            previous_model_key: self.previous_model_key.clone(),
            adaptation_progress_01: self.adaptation_progress_01,
            active_policy_key: self.active_policy.key.clone(),
            active_policy_name: self.active_policy.display_name.clone(),
            tax_or_tribute_rate_01: self.active_policy.tax_or_tribute_rate,
            patent_duration_years: self.active_policy.patent_duration_years,
            skimming_leakage_fraction: self.skimming_leakage_fraction,
            effective_attention_capacity: self.base_attention_capacity
                * (0.65 + 0.35 * self.adaptation_progress_01),
            clerical_overhead_per_mandate: self.clerical_overhead_per_mandate,
            hierarchy_hop_latency_hours: self.hierarchy_hop_latency_hours,
            chartered_carriers_total: self.chartered_carriers_total,
            bureaucratic_capture_events: self.bureaucratic_capture_events,
        })
    }
}

impl CanonicalSaveablePort for PrivateEconomyPlugin {
    fn cartridge_tag(&self) -> &'static str {
        "sim-plug-private-eco"
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        write_str(out, &self.active_model_key);
        write_str(out, &self.active_model_name);
        write_u8(out, u8::from(self.previous_model_key.is_some()));
        write_str(
            out,
            self.previous_model_key.as_deref().unwrap_or_default(),
        );
        write_f32_bits(out, self.base_attention_capacity);
        write_f32_bits(out, self.clerical_overhead_per_mandate);
        write_u32_le(out, self.hierarchy_hop_latency_hours);
        write_f32_bits(out, self.adaptation_progress_01);
        write_str(out, &self.active_policy.key);
        write_str(out, &self.active_policy.display_name);
        write_f32_bits(out, self.active_policy.tax_or_tribute_rate);
        write_u32_le(out, self.active_policy.patent_duration_years);
        write_f32_bits(out, self.skimming_leakage_fraction);
        write_u64_le(out, self.bureaucratic_capture_events);
        write_u32_le(out, self.chartered_carriers_total);
        write_u64_le(out, self.last_stepped_tick.0);

        write_u32_le(out, self.actors.len() as u32);
        for a in &self.actors {
            write_u32_le(out, a.actor_id.0);
            write_str(out, &a.name);
            write_u8(out, kind_to_u8(a.kind));
            write_u32_le(out, a.home_settlement.0);
            write_f64_bits(out, a.working_capital_coins);
            write_u8(out, u8::from(a.held_commodity.is_some()));
            write_u16_le(out, a.held_commodity.map(|c| c.0).unwrap_or(0));
            write_f64_bits(out, a.held_mass_kg);
            write_f64_bits(out, a.hoarded_mass_kg);
            write_u32_le(out, a.private_carriers_owned);
            write_u32_le(out, a.chartered_to_state_carriers);
            write_f32_bits(out, a.compliance_01);
        }

        write_u32_le(out, self.markets.len() as u32);
        for m in &self.markets {
            write_u32_le(out, m.settlement.0);
            write_u16_le(out, m.commodity.0);
            write_f32_bits(out, m.clearing_price_index);
            write_f32_bits(out, m.black_market_premium_01);
            write_f64_bits(out, m.black_market_leakage_kg);
            write_f64_bits(out, m.hoarded_reserve_kg);
        }

        write_u32_le(out, self.congested_list.len() as u32);
        for c in &self.congested_list {
            write_u32_le(out, c.edge_id.0);
            write_f32_bits(out, c.hourly_flow_tonnes);
            write_f32_bits(out, c.capacity_tonnes);
            write_f32_bits(out, c.congestion_multiplier);
        }
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        self.active_model_key = read_str(input)?;
        self.active_model_name = read_str(input)?;
        let has_prev = read_u8(input)? != 0;
        let prev = read_str(input)?;
        self.previous_model_key = has_prev.then_some(prev);
        self.base_attention_capacity = read_f32_bits(input)?;
        self.clerical_overhead_per_mandate = read_f32_bits(input)?;
        self.hierarchy_hop_latency_hours = read_u32_le(input)?;
        self.adaptation_progress_01 = read_f32_bits(input)?;
        self.active_policy = PolicyState {
            key: read_str(input)?,
            display_name: read_str(input)?,
            tax_or_tribute_rate: read_f32_bits(input)?,
            patent_duration_years: read_u32_le(input)?,
        };
        self.skimming_leakage_fraction = read_f32_bits(input)?;
        self.bureaucratic_capture_events = read_u64_le(input)?;
        self.chartered_carriers_total = read_u32_le(input)?;
        self.last_stepped_tick = Tick(read_u64_le(input)?);

        let a_len = read_u32_le(input)? as usize;
        self.actors.clear();
        for _ in 0..a_len {
            let actor_id = ActorId(read_u32_le(input)?);
            let name = read_str(input)?;
            let kind = u8_to_kind(read_u8(input)?)?;
            let home_settlement = SettlementId(read_u32_le(input)?);
            let working_capital_coins = read_f64_bits(input)?;
            let has_comm = read_u8(input)? != 0;
            let comm = CommodityId(read_u16_le(input)?);
            let held_mass_kg = read_f64_bits(input)?;
            let hoarded_mass_kg = read_f64_bits(input)?;
            let private_carriers_owned = read_u32_le(input)?;
            let chartered_to_state_carriers = read_u32_le(input)?;
            let compliance_01 = read_f32_bits(input)?;
            self.actors.push(NonStateActorView {
                actor_id,
                name,
                kind,
                home_settlement,
                working_capital_coins,
                held_commodity: has_comm.then_some(comm),
                held_mass_kg,
                hoarded_mass_kg,
                private_carriers_owned,
                chartered_to_state_carriers,
                compliance_01,
            });
        }

        let m_len = read_u32_le(input)? as usize;
        self.markets.clear();
        for _ in 0..m_len {
            self.markets.push(InformalMarketView {
                settlement: SettlementId(read_u32_le(input)?),
                commodity: CommodityId(read_u16_le(input)?),
                clearing_price_index: read_f32_bits(input)?,
                black_market_premium_01: read_f32_bits(input)?,
                black_market_leakage_kg: read_f64_bits(input)?,
                hoarded_reserve_kg: read_f64_bits(input)?,
            });
        }

        let c_len = read_u32_le(input)? as usize;
        self.congestion_by_edge.clear();
        self.congested_list.clear();
        for _ in 0..c_len {
            let view = EdgeCongestionView {
                edge_id: EdgeId(read_u32_le(input)?),
                hourly_flow_tonnes: read_f32_bits(input)?,
                capacity_tonnes: read_f32_bits(input)?,
                congestion_multiplier: read_f32_bits(input)?,
            };
            self.congestion_by_edge.insert(view.edge_id, view.clone());
            self.congested_list.push(view);
        }
        self.pending_arbitrage.clear();
        Ok(())
    }
}
