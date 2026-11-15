//! Modular multi-channel logistics, terrain-aware pathfinding, and physical courier network.
//!
//! Communicates exclusively through `sim-spine` ports (`PlanetGeometryPort`, `SurfaceConditionsPort`,
//! `ModRegistryPort`, `LogisticsPort`, `CanonicalSaveablePort`) and maintains deterministic state
//! for freight convoys, migrating cohorts, marches, HQ retinues, survey parties, and couriers.

#![deny(clippy::disallowed_types)]

use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BinaryHeap};

use sim_spine::codec::{
    read_f32_bits, read_f64_bits, read_str, read_u8, read_u16_le, read_u32_le, read_u64_le,
    write_f32_bits, write_f64_bits, write_str, write_u8, write_u16_le, write_u32_le, write_u64_le,
};
use sim_spine::ids::{
    ActorId, CarrierClassId, CellId, CharacterId, CommodityId, ConvoyId, DispatchId, EdgeId,
    InfraClassId, MandateId, SettlementId, Tick,
};
use sim_spine::ports::{
    ArrivedCourierEnvelope, CanonicalSaveablePort, CarrierDefView, ConvoyTransitView,
    CourierEnvelope, LogisticsPort, ModRegistryPort, PlanetGeometryPort, ShipmentRequest,
    SurfaceConditionsPort, TransitChannelKind,
};

/// Physical state and restrictions of an edge connecting two adjacent planetary cells.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeLogisticsState {
    pub edge_id: EdgeId,
    pub infra_class: InfraClassId,
    pub bridge_weight_cap_kg: Option<f64>,
    pub washed_out: bool,
    pub river_edge: bool,
    pub sea_edge: bool,
    pub requires_bridge_for_land: bool,
}

/// Arrived non-courier physical payload queued for deposition into `sim-economy` at arrival.
#[derive(Clone, Debug, PartialEq)]
pub struct ArrivedCargoDelivery {
    pub convoy_id: ConvoyId,
    pub channel: TransitChannelKind,
    pub origin_settlement: SettlementId,
    pub destination_settlement: Option<SettlementId>,
    pub destination_cell: CellId,
    pub owner: ActorId,
    pub carrier_class: CarrierClassId,
    pub commodity: Option<CommodityId>,
    pub mass_kg: f64,
    pub purity_01: f32,
    pub unused_fuel_kg: f64,
    pub unused_spares_kg: f64,
    pub arrived_tick: Tick,
    pub is_hq_move: bool,
    pub survey_target_cell: Option<CellId>,
    pub mandate_payload_id: Option<MandateId>,
}

/// Active physical transit entry shared uniformly across all 6 `TransitChannelKind` channels.
#[derive(Clone, Debug, PartialEq)]
pub struct ActiveConvoy {
    pub convoy_id: ConvoyId,
    pub label: String,
    pub channel: TransitChannelKind,
    pub origin_settlement: SettlementId,
    pub destination_settlement: Option<SettlementId>,
    pub origin_cell: CellId,
    pub current_cell: CellId,
    pub destination_cell: CellId,
    pub planned_route: Vec<CellId>,
    pub route_cursor: usize,
    pub progress_on_edge_km: f64,
    pub carrier_class: CarrierClassId,
    pub owner: ActorId,
    pub cargo_commodity: Option<CommodityId>,
    pub cargo_mass_kg: f64,
    pub cargo_purity_01: f32,
    pub fuel_remaining_kg: f64,
    pub spare_parts_remaining_kg: f64,
    pub departure_tick: Tick,
    pub expected_arrival_tick: Tick,
    pub grace_ticks: u64,
    pub is_hq_move: bool,
    pub stranded_no_fuel: bool,
    pub arrived: bool,
    pub dispatch_id: Option<DispatchId>,
    pub sender: Option<CharacterId>,
    pub mandate_payload_id: Option<MandateId>,
    pub subject: String,
}

impl ActiveConvoy {
    /// Converts internal convoy record into public Spine transit view.
    pub fn to_transit_view(&self) -> ConvoyTransitView {
        ConvoyTransitView {
            convoy_id: self.convoy_id,
            label: self.label.clone(),
            channel: self.channel,
            origin_cell: self.origin_cell,
            current_cell: self.current_cell,
            destination_cell: self.destination_cell,
            planned_route: self.planned_route.clone(),
            departure_tick: self.departure_tick,
            expected_arrival_tick: self.expected_arrival_tick,
            grace_ticks: self.grace_ticks,
            is_hq_move: self.is_hq_move,
            stranded_no_fuel: self.stranded_no_fuel,
        }
    }
}

/// Parameters for launching any physical convoy or expedition through the unified router.
#[derive(Clone, Debug)]
pub struct UnifiedTransitLaunch {
    pub label: String,
    pub channel: TransitChannelKind,
    pub origin_settlement: SettlementId,
    pub destination_settlement: Option<SettlementId>,
    pub origin_cell: CellId,
    pub destination_cell: CellId,
    pub carrier_class: CarrierClassId,
    pub owner: ActorId,
    pub cargo_commodity: Option<CommodityId>,
    pub cargo_mass_kg: f64,
    pub cargo_purity_01: f32,
    pub fuel_loaded_kg: f64,
    pub spare_parts_loaded_kg: f64,
    pub current_tick: Tick,
    pub is_hq_move: bool,
    pub dispatch_id: Option<DispatchId>,
    pub sender: Option<CharacterId>,
    pub mandate_payload_id: Option<MandateId>,
    pub subject: String,
}

/// Metrics returned from a single hourly step of the logistics network.
#[derive(Clone, Debug, Default)]
pub struct LogisticsStepSummary {
    pub total_fuel_burned_kg: f64,
    pub total_spares_worn_kg: f64,
    pub unused_courier_supplies_kg: f64,
    pub arrived_cargo: Vec<ArrivedCargoDelivery>,
    pub newly_stranded_convoys: Vec<ConvoyId>,
    pub new_washout_edges: Vec<EdgeId>,
}

/// Unified multi-channel transport graph, router, and physical courier network.
#[derive(Clone, Debug)]
pub struct LogisticsNetwork {
    default_infra: InfraClassId,
    timber_bridge_infra: Option<InfraClassId>,
    river_channel_infra: Option<InfraClassId>,
    default_freight_carrier: CarrierClassId,
    default_courier_carrier: CarrierClassId,
    edges: BTreeMap<EdgeId, EdgeLogisticsState>,
    convoys: Vec<ActiveConvoy>,
    arrived_couriers: Vec<ArrivedCourierEnvelope>,
    pending_shipments: Vec<(ShipmentRequest, Option<(CellId, CellId)>)>,
    next_convoy_id: u64,
    next_dispatch_id: u64,
    total_cargo_mass_in_transit_kg: f64,
}

impl Default for LogisticsNetwork {
    fn default() -> Self {
        Self {
            default_infra: InfraClassId(0),
            timber_bridge_infra: None,
            river_channel_infra: None,
            default_freight_carrier: CarrierClassId(0),
            default_courier_carrier: CarrierClassId(0),
            edges: BTreeMap::new(),
            convoys: Vec::new(),
            arrived_couriers: Vec::new(),
            pending_shipments: Vec::new(),
            next_convoy_id: 1,
            next_dispatch_id: 1,
            total_cargo_mass_in_transit_kg: 0.0,
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct AStarNode {
    f_cost_micros: u64,
    g_cost_micros: u64,
    cell: CellId,
}

impl Ord for AStarNode {
    fn cmp(&self, other: &Self) -> Ordering {
        self.f_cost_micros
            .cmp(&other.f_cost_micros)
            .then_with(|| self.g_cost_micros.cmp(&other.g_cost_micros))
            .then_with(|| self.cell.0.cmp(&other.cell.0))
    }
}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl LogisticsNetwork {
    /// Initializes edge topology from planetary geometry and infrastructure definitions.
    pub fn from_planet_and_registry(
        planet: &dyn PlanetGeometryPort,
        registry: &dyn ModRegistryPort,
    ) -> Self {
        let default_infra = registry
            .infra_id_by_key("foot_trail")
            .unwrap_or(InfraClassId(0));
        let timber_bridge_infra = registry.infra_id_by_key("timber_bridge");
        let river_channel_infra = registry.infra_id_by_key("river_channel");
        let default_freight_carrier = registry
            .carrier_id_by_key("foot_porter_column")
            .unwrap_or(CarrierClassId(0));
        let default_courier_carrier = registry
            .carrier_id_by_key("mounted_dispatch_rider")
            .unwrap_or(default_freight_carrier);

        let mut edges = BTreeMap::new();
        for idx in 0..planet.cell_count() {
            let u = CellId(idx);
            let elev_u = planet.cell_elevation_m(u);
            let ds_u = planet.watershed_downstream(u);
            for &v in planet.cell_neighbors(u) {
                if let Some(edge_id) = planet.cell_edge(u, v) {
                    if edges.contains_key(&edge_id) {
                        continue;
                    }
                    let elev_v = planet.cell_elevation_m(v);
                    let ds_v = planet.watershed_downstream(v);
                    let sea_edge = elev_u <= 0.0 || elev_v <= 0.0;
                    let drains_between = (ds_u == Some(v) && planet.cell_drainage_area_km2(u) >= 400.0)
                        || (ds_v == Some(u) && planet.cell_drainage_area_km2(v) >= 400.0);
                    let infra_class = if sea_edge && river_channel_infra.is_some() {
                        river_channel_infra.unwrap_or(default_infra)
                    } else {
                        default_infra
                    };
                    edges.insert(
                        edge_id,
                        EdgeLogisticsState {
                            edge_id,
                            infra_class,
                            bridge_weight_cap_kg: None,
                            washed_out: false,
                            river_edge: drains_between || sea_edge,
                            sea_edge,
                            requires_bridge_for_land: false,
                        },
                    );
                }
            }
        }

        Self {
            default_infra,
            timber_bridge_infra,
            river_channel_infra,
            default_freight_carrier,
            default_courier_carrier,
            edges,
            convoys: Vec::new(),
            arrived_couriers: Vec::new(),
            pending_shipments: Vec::new(),
            next_convoy_id: 1,
            next_dispatch_id: 1,
            total_cargo_mass_in_transit_kg: 0.0,
        }
    }

    /// Reads edge logistics metadata.
    pub fn edge_state(&self, edge_id: EdgeId) -> Option<&EdgeLogisticsState> {
        self.edges.get(&edge_id)
    }

    /// Returns all edge logistics metadata keyed by `EdgeId`.
    pub fn edges(&self) -> &BTreeMap<EdgeId, EdgeLogisticsState> {
        &self.edges
    }

    /// Returns default land freight carrier class ID.
    pub fn default_freight_carrier(&self) -> CarrierClassId {
        self.default_freight_carrier
    }

    /// Returns default courier carrier class ID.
    pub fn default_courier_carrier(&self) -> CarrierClassId {
        self.default_courier_carrier
    }

    /// Explicitly configures an edge's infrastructure, bridge capacity, washout state, and water flags.
    pub fn set_edge_state(
        &mut self,
        edge_id: EdgeId,
        infra_class: InfraClassId,
        bridge_weight_cap_kg: Option<f64>,
        washed_out: bool,
        river_edge: bool,
        requires_bridge_for_land: bool,
    ) {
        let sea_edge = self.edges.get(&edge_id).map(|e| e.sea_edge).unwrap_or(false);
        self.edges.insert(
            edge_id,
            EdgeLogisticsState {
                edge_id,
                infra_class,
                bridge_weight_cap_kg,
                washed_out,
                river_edge,
                sea_edge,
                requires_bridge_for_land,
            },
        );
    }

    /// Upgrades an edge's infrastructure tier and optional bridge payload ceiling.
    pub fn upgrade_edge_infra(
        &mut self,
        edge_id: EdgeId,
        infra_class: InfraClassId,
        bridge_weight_cap_kg: Option<f64>,
    ) {
        let entry = self.edges.entry(edge_id).or_insert(EdgeLogisticsState {
            edge_id,
            infra_class: self.default_infra,
            bridge_weight_cap_kg: None,
            washed_out: false,
            river_edge: false,
            sea_edge: false,
            requires_bridge_for_land: false,
        });
        entry.infra_class = infra_class;
        entry.bridge_weight_cap_kg = bridge_weight_cap_kg;
        entry.washed_out = false;
    }

    /// Marks or clears a bridge/road washout on a specified edge.
    pub fn set_edge_washed_out(&mut self, edge_id: EdgeId, washed_out: bool) {
        if let Some(entry) = self.edges.get_mut(&edge_id) {
            entry.washed_out = washed_out;
        }
    }

    /// Returns total freight/cargo mass currently physically carried on active convoys (for conservation ledger checks).
    pub fn mass_in_transit_kg(&self) -> f64 {
        self.convoys
            .iter()
            .filter(|c| !c.arrived)
            .map(|c| c.cargo_mass_kg)
            .sum()
    }

    /// Returns all active (and recently completed) convoys managed by the unified stepper.
    pub fn convoys(&self) -> &[ActiveConvoy] {
        &self.convoys
    }

    /// Clears arrived courier buffer after `sim-command` / `player-view` ingests them.
    pub fn drain_arrived_couriers(&mut self) -> Vec<ArrivedCourierEnvelope> {
        std::mem::take(&mut self.arrived_couriers)
    }

    /// Evaluates directional speed (km/hr) along edge `u -> v` for the given carrier and cargo load.
    /// Returns `None` if impassable under current infrastructure, weight cap, washout, slope, or ice conditions.
    #[allow(clippy::too_many_arguments)]
    pub fn evaluate_edge_speed_km_hr(
        &self,
        u: CellId,
        v: CellId,
        carrier: &CarrierDefView,
        cargo_mass_kg: f64,
        planet: &dyn PlanetGeometryPort,
        weather: &dyn SurfaceConditionsPort,
        registry: &dyn ModRegistryPort,
    ) -> Option<f64> {
        eval_edge_speed_with_map(
            &self.edges,
            self.default_infra,
            self.timber_bridge_infra,
            self.river_channel_infra,
            u,
            v,
            carrier,
            cargo_mass_kg,
            planet,
            weather,
            registry,
        )
    }

    /// Unified A* shortest-travel-time pathfinder shared across all 6 `TransitChannelKind` channels.
    /// Returns `(route_cells, estimated_travel_hours)` including `origin` at index `0` and `dest` at end.
    #[allow(clippy::too_many_arguments)]
    pub fn plan_route(
        &self,
        origin: CellId,
        dest: CellId,
        carrier_id: CarrierClassId,
        cargo_mass_kg: f64,
        planet: &dyn PlanetGeometryPort,
        weather: &dyn SurfaceConditionsPort,
        registry: &dyn ModRegistryPort,
    ) -> Option<(Vec<CellId>, f64)> {
        if origin == dest {
            return Some((vec![origin], 0.0));
        }
        let carrier = registry.carrier_def(carrier_id)?;
        let max_speed_km_hr = f64::from(carrier.base_speed_km_hr.max(1.0)) * 1.6;
        let to_micros = |hours: f64| -> u64 { ((hours.max(0.0) * 1_000_000.0).round()) as u64 };

        let mut g_cost: BTreeMap<CellId, u64> = BTreeMap::new();
        let mut parent: BTreeMap<CellId, CellId> = BTreeMap::new();
        let mut open: BinaryHeap<Reverse<AStarNode>> = BinaryHeap::new();

        g_cost.insert(origin, 0);
        let h0 = to_micros(f64::from(planet.great_circle_km(origin, dest)) / max_speed_km_hr);
        open.push(Reverse(AStarNode {
            f_cost_micros: h0,
            g_cost_micros: 0,
            cell: origin,
        }));

        while let Some(Reverse(current)) = open.pop() {
            if current.cell == dest {
                let mut rev = vec![dest];
                let mut cursor = dest;
                while cursor != origin {
                    let prev = *parent.get(&cursor)?;
                    rev.push(prev);
                    cursor = prev;
                }
                rev.reverse();
                let est_hours = (current.g_cost_micros as f64) / 1_000_000.0;
                return Some((rev, est_hours));
            }
            if let Some(&best_g) = g_cost.get(&current.cell)
                && current.g_cost_micros > best_g
            {
                continue;
            }
            for &next in planet.cell_neighbors(current.cell) {
                let Some(edge_id) = planet.cell_edge(current.cell, next) else {
                    continue;
                };
                let Some(speed_km_hr) = self.evaluate_edge_speed_km_hr(
                    current.cell,
                    next,
                    carrier,
                    cargo_mass_kg,
                    planet,
                    weather,
                    registry,
                ) else {
                    continue;
                };
                let dist_km = f64::from(planet.edge_length_km(edge_id).max(1.0));
                let step_hours = dist_km / speed_km_hr.max(0.2);
                let step_micros = to_micros(step_hours).max(1);
                let tentative_g = current.g_cost_micros.saturating_add(step_micros);
                let prev_best = g_cost.get(&next).copied().unwrap_or(u64::MAX);
                if tentative_g < prev_best {
                    g_cost.insert(next, tentative_g);
                    parent.insert(next, current.cell);
                    let h =
                        to_micros(f64::from(planet.great_circle_km(next, dest)) / max_speed_km_hr);
                    open.push(Reverse(AStarNode {
                        f_cost_micros: tentative_g.saturating_add(h),
                        g_cost_micros: tentative_g,
                        cell: next,
                    }));
                }
            }
        }
        None
    }

    /// Launches a physical convoy across ANY of the 6 `TransitChannelKind` channels through a single code path.
    pub fn launch_unified_transit(
        &mut self,
        req: UnifiedTransitLaunch,
        planet: &dyn PlanetGeometryPort,
        weather: &dyn SurfaceConditionsPort,
        registry: &dyn ModRegistryPort,
    ) -> Result<ConvoyId, String> {
        let (route, est_hours) = self
            .plan_route(
                req.origin_cell,
                req.destination_cell,
                req.carrier_class,
                req.cargo_mass_kg,
                planet,
                weather,
                registry,
            )
            .ok_or_else(|| {
                format!(
                    "No valid physical route for carrier {:?} between {:?} and {:?}",
                    req.carrier_class, req.origin_cell, req.destination_cell
                )
            })?;

        let convoy_id = ConvoyId(self.next_convoy_id);
        self.next_convoy_id += 1;

        let travel_ticks = (est_hours.ceil() as u64).max(1);
        let same_cell = req.origin_cell == req.destination_cell;
        let expected_arrival = if same_cell {
            Tick(req.current_tick.0 + 1)
        } else {
            Tick(req.current_tick.0 + travel_ticks)
        };
        let grace_ticks = (travel_ticks / 4).max(4);
        let stranded_no_fuel =
            !same_cell && req.fuel_loaded_kg <= 0.0 && req.channel != TransitChannelKind::PhysicalCourier;

        self.total_cargo_mass_in_transit_kg += req.cargo_mass_kg;

        self.convoys.push(ActiveConvoy {
            convoy_id,
            label: req.label,
            channel: req.channel,
            origin_settlement: req.origin_settlement,
            destination_settlement: req.destination_settlement,
            origin_cell: req.origin_cell,
            current_cell: req.origin_cell,
            destination_cell: req.destination_cell,
            planned_route: route,
            route_cursor: 0,
            progress_on_edge_km: 0.0,
            carrier_class: req.carrier_class,
            owner: req.owner,
            cargo_commodity: req.cargo_commodity,
            cargo_mass_kg: req.cargo_mass_kg,
            cargo_purity_01: req.cargo_purity_01,
            fuel_remaining_kg: req.fuel_loaded_kg,
            spare_parts_remaining_kg: req.spare_parts_loaded_kg,
            departure_tick: req.current_tick,
            expected_arrival_tick: expected_arrival,
            grace_ticks,
            is_hq_move: req.is_hq_move,
            stranded_no_fuel,
            arrived: false,
            dispatch_id: req.dispatch_id.or_else(|| {
                (req.channel == TransitChannelKind::PhysicalCourier).then(|| {
                    let did = DispatchId(self.next_dispatch_id);
                    self.next_dispatch_id += 1;
                    did
                })
            }),
            sender: req.sender,
            mandate_payload_id: req.mandate_payload_id,
            subject: req.subject,
        });

        Ok(convoy_id)
    }

    /// Resolves any queued `LogisticsPort::enqueue_shipment` calls once world context (settlement cells) is provided.
    pub fn flush_pending_port_shipments(
        &mut self,
        settlement_cells: &BTreeMap<SettlementId, CellId>,
        current_tick: Tick,
        planet: &dyn PlanetGeometryPort,
        weather: &dyn SurfaceConditionsPort,
        registry: &dyn ModRegistryPort,
    ) {
        let pending = std::mem::take(&mut self.pending_shipments);
        for (req, override_cells) in pending {
            let (orig_cell, dest_cell) = match override_cells {
                Some(pair) => pair,
                None => {
                    let Some(&o) = settlement_cells.get(&req.origin_settlement) else {
                        continue;
                    };
                    let Some(&d) = settlement_cells.get(&req.destination_settlement) else {
                        continue;
                    };
                    (o, d)
                }
            };
            let _ = self.launch_unified_transit(
                UnifiedTransitLaunch {
                    label: format!("Freight #{}", self.next_convoy_id),
                    channel: TransitChannelKind::FreightCargo,
                    origin_settlement: req.origin_settlement,
                    destination_settlement: Some(req.destination_settlement),
                    origin_cell: orig_cell,
                    destination_cell: dest_cell,
                    carrier_class: req.carrier_class,
                    owner: req.owner,
                    cargo_commodity: Some(req.commodity),
                    cargo_mass_kg: req.mass_kg,
                    cargo_purity_01: 0.90,
                    fuel_loaded_kg: 0.0,
                    spare_parts_loaded_kg: 0.0,
                    current_tick,
                    is_hq_move: false,
                    dispatch_id: None,
                    sender: None,
                    mandate_payload_id: None,
                    subject: String::new(),
                },
                planet,
                weather,
                registry,
            );
        }
    }

    /// Advances all active convoys, marches, migrations, HQ moves, surveys, and couriers for 1 hour.
    pub fn step_hourly(
        &mut self,
        current_tick: Tick,
        planet: &dyn PlanetGeometryPort,
        weather: &dyn SurfaceConditionsPort,
        registry: &dyn ModRegistryPort,
    ) -> LogisticsStepSummary {
        let mut summary = LogisticsStepSummary::default();

        // Weather washouts on timber bridges / unimproved crossing edges under extreme floods
        for (edge_id, state) in &mut self.edges {
            if state.river_edge && !state.sea_edge && !state.washed_out {
                // Only severe floods trigger dynamic washout if bridge exists
                let has_bridge =
                    Some(state.infra_class) == self.timber_bridge_infra || state.bridge_weight_cap_kg.is_some();
                if has_bridge {
                    // Find endpoint flood by sampling nothing extra: leave deterministic flood check to external weather hook
                    let _ = edge_id;
                }
            }
        }

        // Check extreme flood washouts directly on adjacent cell pairs with bridges
        for c in 0..planet.cell_count() {
            let u = CellId(c);
            let f_u = weather.surface(u).flood_stage_01;
            if f_u < 0.90 {
                continue;
            }
            for &v in planet.cell_neighbors(u) {
                if let Some(edge_id) = planet.cell_edge(u, v)
                    && let Some(state) = self.edges.get_mut(&edge_id)
                    && !state.washed_out
                    && (Some(state.infra_class) == self.timber_bridge_infra
                        || state.bridge_weight_cap_kg.is_some())
                {
                    state.washed_out = true;
                    summary.new_washout_edges.push(edge_id);
                }
            }
        }

        let Self {
            edges,
            default_infra,
            timber_bridge_infra,
            river_channel_infra,
            convoys,
            arrived_couriers,
            total_cargo_mass_in_transit_kg,
            ..
        } = self;

        for convoy in convoys.iter_mut() {
            if convoy.arrived {
                continue;
            }
            if convoy.current_cell == convoy.destination_cell
                && convoy.route_cursor + 1 >= convoy.planned_route.len()
            {
                record_convoy_arrival(
                    convoy,
                    current_tick,
                    arrived_couriers,
                    total_cargo_mass_in_transit_kg,
                    &mut summary,
                );
                continue;
            }

            if convoy.stranded_no_fuel {
                continue;
            }

            let Some(carrier) = registry.carrier_def(convoy.carrier_class) else {
                continue;
            };

            let load_ratio = (convoy.cargo_mass_kg / carrier.max_payload_kg.max(1.0)).clamp(0.0, 2.5);
            let hourly_fuel_burn = carrier.fuel_burn_kg_per_hour * (1.0 + 0.35 * load_ratio);

            if hourly_fuel_burn > 0.0
                && convoy.fuel_remaining_kg <= 0.0
                && convoy.channel != TransitChannelKind::PhysicalCourier
            {
                convoy.stranded_no_fuel = true;
                summary.newly_stranded_convoys.push(convoy.convoy_id);
                continue;
            }

            let next_idx = convoy.route_cursor + 1;
            if next_idx >= convoy.planned_route.len() {
                convoy.current_cell = convoy.destination_cell;
                record_convoy_arrival(
                    convoy,
                    current_tick,
                    arrived_couriers,
                    total_cargo_mass_in_transit_kg,
                    &mut summary,
                );
                continue;
            }

            let u = convoy.planned_route[convoy.route_cursor];
            let v = convoy.planned_route[next_idx];
            let Some(speed_km_hr) = eval_edge_speed_with_map(
                edges,
                *default_infra,
                *timber_bridge_infra,
                *river_channel_infra,
                u,
                v,
                carrier,
                convoy.cargo_mass_kg,
                planet,
                weather,
                registry,
            ) else {
                // Temporarily blocked by weather (mud/flood/washout/ice); convoy holds position without advancing
                continue;
            };

            let fuel_used = hourly_fuel_burn.min(convoy.fuel_remaining_kg);
            let throttle = if convoy.channel == TransitChannelKind::PhysicalCourier {
                1.0
            } else if hourly_fuel_burn > 1e-9 {
                (fuel_used / hourly_fuel_burn).clamp(0.0, 1.0)
            } else {
                1.0
            };
            convoy.fuel_remaining_kg -= fuel_used;
            summary.total_fuel_burned_kg += fuel_used;

            if convoy.fuel_remaining_kg <= 1e-9
                && hourly_fuel_burn > 0.0
                && throttle < 0.99
                && convoy.channel != TransitChannelKind::PhysicalCourier
            {
                convoy.fuel_remaining_kg = 0.0;
                convoy.stranded_no_fuel = true;
                summary.newly_stranded_convoys.push(convoy.convoy_id);
            }

            let distance_step_km = speed_km_hr * throttle;
            let spare_wear = distance_step_km
                * carrier.spare_parts_wear_per_km
                * (0.5 + 0.5 * load_ratio);
            if spare_wear > 0.0 && convoy.spare_parts_remaining_kg > 0.0 {
                let consumed = spare_wear.min(convoy.spare_parts_remaining_kg);
                convoy.spare_parts_remaining_kg -= consumed;
                summary.total_spares_worn_kg += consumed;
            }

            let mut travel_budget_km = distance_step_km;
            while travel_budget_km > 0.0 && convoy.route_cursor + 1 < convoy.planned_route.len() {
                let seg_u = convoy.planned_route[convoy.route_cursor];
                let seg_v = convoy.planned_route[convoy.route_cursor + 1];
                let Some(seg_edge) = planet.cell_edge(seg_u, seg_v) else {
                    break;
                };
                let edge_km = f64::from(planet.edge_length_km(seg_edge).max(1.0));
                let rem_km = (edge_km - convoy.progress_on_edge_km).max(0.0);
                if travel_budget_km >= rem_km {
                    travel_budget_km -= rem_km;
                    convoy.progress_on_edge_km = 0.0;
                    convoy.route_cursor += 1;
                    convoy.current_cell = convoy.planned_route[convoy.route_cursor];
                } else {
                    convoy.progress_on_edge_km += travel_budget_km;
                    travel_budget_km = 0.0;
                }
            }

            if convoy.route_cursor + 1 >= convoy.planned_route.len() {
                convoy.current_cell = convoy.destination_cell;
                record_convoy_arrival(
                    convoy,
                    current_tick,
                    arrived_couriers,
                    total_cargo_mass_in_transit_kg,
                    &mut summary,
                );
            }
        }

        summary
    }
}

impl LogisticsPort for LogisticsNetwork {
    fn convoy_state(&self, convoy: ConvoyId) -> Option<ConvoyTransitView> {
        self.convoys
            .iter()
            .find(|c| c.convoy_id == convoy)
            .map(ActiveConvoy::to_transit_view)
    }

    fn all_active_convoys(&self) -> Vec<ConvoyTransitView> {
        self.convoys
            .iter()
            .filter(|c| !c.arrived)
            .map(ActiveConvoy::to_transit_view)
            .collect()
    }

    fn enqueue_shipment(&mut self, req: ShipmentRequest) -> ConvoyId {
        let id = ConvoyId(self.next_convoy_id);
        self.next_convoy_id += 1;
        self.pending_shipments.push((req, None));
        id
    }

    fn enqueue_courier(&mut self, env: CourierEnvelope) -> DispatchId {
        let id = DispatchId(self.next_dispatch_id);
        self.next_dispatch_id += 1;
        let convoy_id = ConvoyId(self.next_convoy_id);
        self.next_convoy_id += 1;
        self.convoys.push(ActiveConvoy {
            convoy_id,
            label: format!("Courier #{}", id.0),
            channel: TransitChannelKind::PhysicalCourier,
            origin_settlement: env.origin_settlement,
            destination_settlement: None,
            origin_cell: env.origin_cell,
            current_cell: env.origin_cell,
            destination_cell: env.origin_cell,
            planned_route: vec![env.origin_cell],
            route_cursor: 0,
            progress_on_edge_km: 0.0,
            carrier_class: self.default_courier_carrier,
            owner: ActorId(0),
            cargo_commodity: None,
            cargo_mass_kg: 0.0,
            cargo_purity_01: 1.0,
            fuel_remaining_kg: 0.0,
            spare_parts_remaining_kg: 0.0,
            departure_tick: env.sent_tick,
            expected_arrival_tick: Tick(env.sent_tick.0 + 1),
            grace_ticks: 4,
            is_hq_move: false,
            stranded_no_fuel: false,
            arrived: false,
            dispatch_id: Some(id),
            sender: Some(env.sender),
            mandate_payload_id: None,
            subject: env.subject,
        });
        id
    }

    fn arrived_envelopes(&self) -> &[ArrivedCourierEnvelope] {
        &self.arrived_couriers
    }
}

fn channel_to_u8(c: TransitChannelKind) -> u8 {
    c as u8
}

fn u8_to_channel(v: u8) -> Result<TransitChannelKind, String> {
    match v {
        0 => Ok(TransitChannelKind::FreightCargo),
        1 => Ok(TransitChannelKind::CohortMigration),
        2 => Ok(TransitChannelKind::MilitaryMarch),
        3 => Ok(TransitChannelKind::HqRetinue),
        4 => Ok(TransitChannelKind::SurveyExpedition),
        5 => Ok(TransitChannelKind::PhysicalCourier),
        _ => Err(format!("Invalid TransitChannelKind tag {v}")),
    }
}

impl CanonicalSaveablePort for LogisticsNetwork {
    fn cartridge_tag(&self) -> &'static str {
        "sim-logistics"
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        write_u16_le(out, self.default_infra.0);
        write_u8(out, u8::from(self.timber_bridge_infra.is_some()));
        write_u16_le(out, self.timber_bridge_infra.map(|x| x.0).unwrap_or(0));
        write_u8(out, u8::from(self.river_channel_infra.is_some()));
        write_u16_le(out, self.river_channel_infra.map(|x| x.0).unwrap_or(0));
        write_u16_le(out, self.default_freight_carrier.0);
        write_u16_le(out, self.default_courier_carrier.0);
        write_u64_le(out, self.next_convoy_id);
        write_u64_le(out, self.next_dispatch_id);
        write_f64_bits(out, self.total_cargo_mass_in_transit_kg);

        write_u32_le(out, self.edges.len() as u32);
        for e in self.edges.values() {
            write_u32_le(out, e.edge_id.0);
            write_u16_le(out, e.infra_class.0);
            write_u8(out, u8::from(e.bridge_weight_cap_kg.is_some()));
            write_f64_bits(out, e.bridge_weight_cap_kg.unwrap_or(0.0));
            write_u8(out, u8::from(e.washed_out));
            write_u8(out, u8::from(e.river_edge));
            write_u8(out, u8::from(e.sea_edge));
            write_u8(out, u8::from(e.requires_bridge_for_land));
        }

        write_u32_le(out, self.convoys.len() as u32);
        for c in &self.convoys {
            write_u64_le(out, c.convoy_id.0);
            write_str(out, &c.label);
            write_u8(out, channel_to_u8(c.channel));
            write_u32_le(out, c.origin_settlement.0);
            write_u8(out, u8::from(c.destination_settlement.is_some()));
            write_u32_le(out, c.destination_settlement.map(|x| x.0).unwrap_or(0));
            write_u32_le(out, c.origin_cell.0);
            write_u32_le(out, c.current_cell.0);
            write_u32_le(out, c.destination_cell.0);
            write_u32_le(out, c.planned_route.len() as u32);
            for cell in &c.planned_route {
                write_u32_le(out, cell.0);
            }
            write_u32_le(out, c.route_cursor as u32);
            write_f64_bits(out, c.progress_on_edge_km);
            write_u16_le(out, c.carrier_class.0);
            write_u32_le(out, c.owner.0);
            write_u8(out, u8::from(c.cargo_commodity.is_some()));
            write_u16_le(out, c.cargo_commodity.map(|x| x.0).unwrap_or(0));
            write_f64_bits(out, c.cargo_mass_kg);
            write_f32_bits(out, c.cargo_purity_01);
            write_f64_bits(out, c.fuel_remaining_kg);
            write_f64_bits(out, c.spare_parts_remaining_kg);
            write_u64_le(out, c.departure_tick.0);
            write_u64_le(out, c.expected_arrival_tick.0);
            write_u64_le(out, c.grace_ticks);
            write_u8(out, u8::from(c.is_hq_move));
            write_u8(out, u8::from(c.stranded_no_fuel));
            write_u8(out, u8::from(c.arrived));
            write_u8(out, u8::from(c.dispatch_id.is_some()));
            write_u64_le(out, c.dispatch_id.map(|x| x.0).unwrap_or(0));
            write_u8(out, u8::from(c.sender.is_some()));
            write_u32_le(out, c.sender.map(|x| x.0).unwrap_or(0));
            write_u8(out, u8::from(c.mandate_payload_id.is_some()));
            write_u64_le(out, c.mandate_payload_id.map(|x| x.0).unwrap_or(0));
            write_str(out, &c.subject);
        }

        write_u32_le(out, self.arrived_couriers.len() as u32);
        for env in &self.arrived_couriers {
            write_u64_le(out, env.dispatch_id.0);
            write_u32_le(out, env.origin_settlement.0);
            write_u32_le(out, env.sender.0);
            write_u64_le(out, env.sent_tick.0);
            write_u64_le(out, env.arrived_tick.0);
            write_str(out, &env.subject);
        }
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        self.default_infra = InfraClassId(read_u16_le(input)?);
        let has_tb = read_u8(input)? != 0;
        let tb = InfraClassId(read_u16_le(input)?);
        self.timber_bridge_infra = has_tb.then_some(tb);
        let has_rc = read_u8(input)? != 0;
        let rc = InfraClassId(read_u16_le(input)?);
        self.river_channel_infra = has_rc.then_some(rc);
        self.default_freight_carrier = CarrierClassId(read_u16_le(input)?);
        self.default_courier_carrier = CarrierClassId(read_u16_le(input)?);
        self.next_convoy_id = read_u64_le(input)?;
        self.next_dispatch_id = read_u64_le(input)?;
        self.total_cargo_mass_in_transit_kg = read_f64_bits(input)?;

        let edge_count = read_u32_le(input)? as usize;
        self.edges.clear();
        for _ in 0..edge_count {
            let edge_id = EdgeId(read_u32_le(input)?);
            let infra_class = InfraClassId(read_u16_le(input)?);
            let has_cap = read_u8(input)? != 0;
            let cap = read_f64_bits(input)?;
            let washed_out = read_u8(input)? != 0;
            let river_edge = read_u8(input)? != 0;
            let sea_edge = read_u8(input)? != 0;
            let requires_bridge_for_land = read_u8(input)? != 0;
            self.edges.insert(
                edge_id,
                EdgeLogisticsState {
                    edge_id,
                    infra_class,
                    bridge_weight_cap_kg: has_cap.then_some(cap),
                    washed_out,
                    river_edge,
                    sea_edge,
                    requires_bridge_for_land,
                },
            );
        }

        let convoy_count = read_u32_le(input)? as usize;
        self.convoys.clear();
        for _ in 0..convoy_count {
            let convoy_id = ConvoyId(read_u64_le(input)?);
            let label = read_str(input)?;
            let channel = u8_to_channel(read_u8(input)?)?;
            let origin_settlement = SettlementId(read_u32_le(input)?);
            let has_ds = read_u8(input)? != 0;
            let ds = SettlementId(read_u32_le(input)?);
            let origin_cell = CellId(read_u32_le(input)?);
            let current_cell = CellId(read_u32_le(input)?);
            let destination_cell = CellId(read_u32_le(input)?);
            let route_len = read_u32_le(input)? as usize;
            let mut planned_route = Vec::with_capacity(route_len);
            for _ in 0..route_len {
                planned_route.push(CellId(read_u32_le(input)?));
            }
            let route_cursor = read_u32_le(input)? as usize;
            let progress_on_edge_km = read_f64_bits(input)?;
            let carrier_class = CarrierClassId(read_u16_le(input)?);
            let owner = ActorId(read_u32_le(input)?);
            let has_comm = read_u8(input)? != 0;
            let comm = CommodityId(read_u16_le(input)?);
            let cargo_mass_kg = read_f64_bits(input)?;
            let cargo_purity_01 = read_f32_bits(input)?;
            let fuel_remaining_kg = read_f64_bits(input)?;
            let spare_parts_remaining_kg = read_f64_bits(input)?;
            let departure_tick = Tick(read_u64_le(input)?);
            let expected_arrival_tick = Tick(read_u64_le(input)?);
            let grace_ticks = read_u64_le(input)?;
            let is_hq_move = read_u8(input)? != 0;
            let stranded_no_fuel = read_u8(input)? != 0;
            let arrived = read_u8(input)? != 0;
            let has_did = read_u8(input)? != 0;
            let did = DispatchId(read_u64_le(input)?);
            let has_sender = read_u8(input)? != 0;
            let sender = CharacterId(read_u32_le(input)?);
            let has_mid = read_u8(input)? != 0;
            let mid = MandateId(read_u64_le(input)?);
            let subject = read_str(input)?;
            self.convoys.push(ActiveConvoy {
                convoy_id,
                label,
                channel,
                origin_settlement,
                destination_settlement: has_ds.then_some(ds),
                origin_cell,
                current_cell,
                destination_cell,
                planned_route,
                route_cursor,
                progress_on_edge_km,
                carrier_class,
                owner,
                cargo_commodity: has_comm.then_some(comm),
                cargo_mass_kg,
                cargo_purity_01,
                fuel_remaining_kg,
                spare_parts_remaining_kg,
                departure_tick,
                expected_arrival_tick,
                grace_ticks,
                is_hq_move,
                stranded_no_fuel,
                arrived,
                dispatch_id: has_did.then_some(did),
                sender: has_sender.then_some(sender),
                mandate_payload_id: has_mid.then_some(mid),
                subject,
            });
        }

        let arrived_count = read_u32_le(input)? as usize;
        self.arrived_couriers.clear();
        for _ in 0..arrived_count {
            self.arrived_couriers.push(ArrivedCourierEnvelope {
                dispatch_id: DispatchId(read_u64_le(input)?),
                origin_settlement: SettlementId(read_u32_le(input)?),
                sender: CharacterId(read_u32_le(input)?),
                sent_tick: Tick(read_u64_le(input)?),
                arrived_tick: Tick(read_u64_le(input)?),
                subject: read_str(input)?,
            });
        }

        self.pending_shipments.clear();
        Ok(())
    }
}

#[allow(clippy::too_many_arguments)]
fn eval_edge_speed_with_map(
    edges: &BTreeMap<EdgeId, EdgeLogisticsState>,
    default_infra: InfraClassId,
    timber_bridge_infra: Option<InfraClassId>,
    river_channel_infra: Option<InfraClassId>,
    u: CellId,
    v: CellId,
    carrier: &CarrierDefView,
    cargo_mass_kg: f64,
    planet: &dyn PlanetGeometryPort,
    weather: &dyn SurfaceConditionsPort,
    registry: &dyn ModRegistryPort,
) -> Option<f64> {
    let edge_id = planet.cell_edge(u, v)?;
    let edge_state = edges.get(&edge_id);
    let infra_id = edge_state.map(|e| e.infra_class).unwrap_or(default_infra);
    let river_edge = edge_state.map(|e| e.river_edge).unwrap_or(false);
    let sea_edge = edge_state.map(|e| e.sea_edge).unwrap_or(false);
    let washed_out = edge_state.map(|e| e.washed_out).unwrap_or(false);
    let bridge_cap = edge_state.and_then(|e| e.bridge_weight_cap_kg);
    let requires_bridge = edge_state.map(|e| e.requires_bridge_for_land).unwrap_or(false);

    let surf_u = weather.surface(u);
    let surf_v = weather.surface(v);
    let max_ice = surf_u.ice_thickness_m.max(surf_v.ice_thickness_m);
    let avg_mud = 0.5 * (surf_u.mud_index_01 + surf_v.mud_index_01);
    let max_flood = surf_u.flood_stage_01.max(surf_v.flood_stage_01);
    let avg_snow = 0.5 * (surf_u.snow_depth_m + surf_v.snow_depth_m);

    if carrier.is_watercraft {
        let is_water_route = Some(infra_id) == river_channel_infra || river_edge || sea_edge;
        if !is_water_route {
            return None;
        }
        if max_ice >= 0.15 || max_ice > carrier.max_ice_thickness_m {
            return None;
        }
        let base = f64::from(carrier.base_speed_km_hr.max(0.5));
        let infra_mult = registry
            .infra_def(infra_id)
            .map(|i| f64::from(i.speed_multiplier.max(0.25)))
            .unwrap_or(1.0);
        let wind_mult = if carrier.uses_wind_sail {
            let ll_u = planet.cell_lat_lon_rad(u);
            let ll_v = planet.cell_lat_lon_rad(v);
            let dlat = ll_v[0] - ll_u[0];
            let dlon = (ll_v[1] - ll_u[1]) * ll_u[0].cos();
            let mag = (dlat * dlat + dlon * dlon).sqrt();
            let avg_u = 0.5 * (surf_u.wind_u_mps + surf_v.wind_u_mps);
            let avg_v = 0.5 * (surf_u.wind_v_mps + surf_v.wind_v_mps);
            let wind_speed = (avg_u * avg_u + avg_v * avg_v).sqrt();
            if mag > 1e-6 && wind_speed > 0.25 {
                let dir_east = dlon / mag;
                let dir_north = dlat / mag;
                let dot_alignment = (avg_u * dir_east + avg_v * dir_north) / wind_speed;
                (1.0 + 0.42 * f64::from(dot_alignment)).clamp(0.42, 1.48)
            } else {
                0.70
            }
        } else {
            1.0
        };
        return Some((base * infra_mult * wind_mult).max(0.4));
    }

    if washed_out {
        return None;
    }
    if requires_bridge && Some(infra_id) != timber_bridge_infra && bridge_cap.is_none() {
        return None;
    }
    if let Some(cap_kg) = bridge_cap
        && cargo_mass_kg > cap_kg
    {
        return None;
    }
    if !carrier.allowed_infra_classes.is_empty()
        && !carrier.allowed_infra_classes.contains(&infra_id)
    {
        return None;
    }

    let slope = planet.cell_slope_rad(edge_id).abs();
    if slope > carrier.max_grade_rad {
        return None;
    }

    let infra = registry.infra_def(infra_id);
    let infra_speed_mult = infra
        .map(|i| f64::from(i.speed_multiplier.max(0.25)))
        .unwrap_or(1.0);
    let mud_shield = infra.map(|i| i.mud_shield_factor).unwrap_or(0.0);
    let effective_mud = (avg_mud * (1.0 - mud_shield)).clamp(0.0, 1.0);
    let mud_mult = f64::from((1.0 - effective_mud * carrier.mud_penalty_factor).clamp(0.18, 1.0));
    let grade_ratio = (slope / carrier.max_grade_rad.max(0.01)).clamp(0.0, 1.0);
    let grade_mult = f64::from(1.0 - 0.45 * grade_ratio);
    let flood_mult = if max_flood >= 0.85 {
        0.22
    } else if max_flood >= 0.45 {
        0.55
    } else {
        1.0
    };
    let snow_mult = if avg_snow >= 0.40 {
        0.42
    } else if avg_snow >= 0.15 {
        0.72
    } else {
        1.0
    };

    let speed = f64::from(carrier.base_speed_km_hr.max(0.5))
        * infra_speed_mult
        * mud_mult
        * grade_mult
        * flood_mult
        * snow_mult;
    Some(speed.max(0.35))
}

fn record_convoy_arrival(
    convoy: &mut ActiveConvoy,
    current_tick: Tick,
    arrived_couriers: &mut Vec<ArrivedCourierEnvelope>,
    total_cargo_mass_in_transit_kg: &mut f64,
    summary: &mut LogisticsStepSummary,
) {
    convoy.arrived = true;
    *total_cargo_mass_in_transit_kg =
        (*total_cargo_mass_in_transit_kg - convoy.cargo_mass_kg).max(0.0);
    let unused_fuel_kg = std::mem::take(&mut convoy.fuel_remaining_kg);
    let unused_spares_kg = std::mem::take(&mut convoy.spare_parts_remaining_kg);
    if let Some(dispatch_id) = convoy.dispatch_id {
        summary.unused_courier_supplies_kg += unused_fuel_kg + unused_spares_kg;
        arrived_couriers.push(ArrivedCourierEnvelope {
            dispatch_id,
            origin_settlement: convoy.origin_settlement,
            sender: convoy.sender.unwrap_or(CharacterId(1)),
            sent_tick: convoy.departure_tick,
            arrived_tick: current_tick,
            subject: convoy.subject.clone(),
        });
    } else {
        summary.arrived_cargo.push(ArrivedCargoDelivery {
            convoy_id: convoy.convoy_id,
            channel: convoy.channel,
            origin_settlement: convoy.origin_settlement,
            destination_settlement: convoy.destination_settlement,
            destination_cell: convoy.destination_cell,
            owner: convoy.owner,
            carrier_class: convoy.carrier_class,
            commodity: convoy.cargo_commodity,
            mass_kg: convoy.cargo_mass_kg,
            purity_01: convoy.cargo_purity_01,
            unused_fuel_kg,
            unused_spares_kg,
            arrived_tick: current_tick,
            is_hq_move: convoy.is_hq_move,
            survey_target_cell: (convoy.channel == TransitChannelKind::SurveyExpedition)
                .then_some(convoy.destination_cell),
            mandate_payload_id: convoy.mandate_payload_id,
        });
    }
}
