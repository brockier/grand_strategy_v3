//! M7 Plug Cartridge: Operational Warfare, Theater Directives (Churchill),
//! Rigid Operational Battle Plans (Hitler), No-Retreat Stale-Order Encirclement,
//! Subordinate Insubordination, and Hourly Weather/Supply Physics.

#![deny(clippy::disallowed_types)]

use std::collections::BTreeMap;

use sim_spine::codec::{
    read_f32_bits, read_f64_bits, read_str, read_u8, read_u16_le, read_u32_le, read_u64_le,
    write_f32_bits, write_f64_bits, write_str, write_u8, write_u16_le, write_u32_le, write_u64_le,
};
use sim_spine::ids::{CellId, CharacterId, FormationId, SettlementId, TheaterId, Tick};
use sim_spine::plugs::{MilitaryFormationView, OperationalCombatPlugPort, TheaterDirectiveView};
use sim_spine::ports::{
    great_circle_arc_km, slerp_unit_xyz, CanonicalSaveablePort, ModRegistryPort,
    PlanetGeometryPort, SurfaceConditionsPort,
};

#[derive(Clone, Debug)]
struct PendingOperationalDispatch {
    formation_id: FormationId,
    delivery_tick: Tick,
    target_cell: CellId,
    target_unit_xyz: [f32; 3],
    doctrine_key: String,
    no_retreat: bool,
}

/// Concrete M7 operational warfare state enforcing the ban on tactical micro.
#[derive(Clone, Debug, Default)]
pub struct OperationalWarfarePlugin {
    theaters: Vec<TheaterDirectiveView>,
    formations: Vec<MilitaryFormationView>,
    home_unit_xyz: BTreeMap<FormationId, [f32; 3]>,
    home_cell: BTreeMap<FormationId, CellId>,
    commander_traits: BTreeMap<CharacterId, (f32, f32)>, // (initiative_01, loyalty_01)
    hostile_pressure_by_cell: BTreeMap<CellId, f32>,
    pending_orders: Vec<PendingOperationalDispatch>,
    next_formation_id: u32,
    last_tick: Tick,
}

impl OperationalWarfarePlugin {
    /// Seeds operational theaters and an initial garrison/defense formation from `ModRegistryPort`.
    #[must_use]
    pub fn from_registry(
        registry: &dyn ModRegistryPort,
        planet: &dyn PlanetGeometryPort,
        hq_settlement: SettlementId,
        hq_cell: CellId,
        commander: CharacterId,
    ) -> Self {
        let mut plugin = Self {
            next_formation_id: 1,
            ..Self::default()
        };
        let default_doctrine = registry
            .doctrines()
            .first()
            .map(|d| d.key.clone())
            .unwrap_or_else(|| "default_doctrine".to_string());

        plugin.theaters.push(TheaterDirectiveView {
            theater_id: TheaterId(1),
            commander,
            objective_cell: hq_cell,
            supply_priority_01: 0.85,
            issued_tick: Tick(0),
            autonomous_withdrawals: 0,
        });

        if let Some(tpl) = registry.formation_templates().first() {
            let xyz = planet.cell_unit_xyz(hq_cell);
            let fid = FormationId(plugin.next_formation_id);
            plugin.next_formation_id += 1;
            plugin.home_cell.insert(fid, hq_cell);
            plugin.home_unit_xyz.insert(fid, xyz);
            plugin.formations.push(MilitaryFormationView {
                formation_id: fid,
                label: tpl.display_name.clone(),
                template_key: tpl.key.clone(),
                theater_id: TheaterId(1),
                commander,
                home_settlement: hq_settlement,
                current_cell: hq_cell,
                current_unit_xyz: xyz,
                target_cell: hq_cell,
                target_unit_xyz: xyz,
                personnel_count: tpl.personnel_count,
                equipment_mass_kg: f64::from(tpl.personnel_count) * tpl.equipment_kg_per_troop,
                munitions_days: 14.0,
                rations_days: 14.0,
                cohesion_01: 0.95,
                doctrine_key: default_doctrine,
                no_retreat_order: false,
                encircled: false,
                insubordinate_withdrawal: false,
                stale_order_ticks: 0,
            });
        }

        plugin
    }

    /// Registers commander `(initiative_01, loyalty_01)` profile for operational disobedience checks (`ENG-WAR-003.2`).
    pub fn set_commander_profile(
        &mut self,
        commander: CharacterId,
        initiative_01: f32,
        loyalty_01: f32,
    ) {
        self.commander_traits
            .insert(commander, (initiative_01.clamp(0.0, 1.0), loyalty_01.clamp(0.0, 1.0)));
    }

    /// Assigns an operational hostile pincer/encirclement threat intensity on `cell` (`0.0..1.0`).
    pub fn set_hostile_pressure(&mut self, cell: CellId, pressure_01: f32) {
        self.hostile_pressure_by_cell
            .insert(cell, pressure_01.clamp(0.0, 1.5));
    }

    /// Mustering a formation directly from physical population & depot equipment (`ENG-WAR-001.2`).
    #[allow(clippy::too_many_arguments)]
    pub fn muster_formation(
        &mut self,
        registry: &dyn ModRegistryPort,
        planet: &dyn PlanetGeometryPort,
        template_key: &str,
        theater_id: TheaterId,
        commander: CharacterId,
        settlement: SettlementId,
        cell: CellId,
    ) -> Result<FormationId, String> {
        let tpl = registry
            .formation_templates()
            .iter()
            .find(|t| t.key == template_key)
            .ok_or_else(|| format!("Unknown formation template '{template_key}'"))?;
        let doctrine_key = registry
            .doctrines()
            .first()
            .map(|d| d.key.clone())
            .unwrap_or_else(|| "default_doctrine".to_string());
        let fid = FormationId(self.next_formation_id);
        self.next_formation_id += 1;
        let xyz = planet.cell_unit_xyz(cell);
        self.home_cell.insert(fid, cell);
        self.home_unit_xyz.insert(fid, xyz);
        self.formations.push(MilitaryFormationView {
            formation_id: fid,
            label: tpl.display_name.clone(),
            template_key: tpl.key.clone(),
            theater_id,
            commander,
            home_settlement: settlement,
            current_cell: cell,
            current_unit_xyz: xyz,
            target_cell: cell,
            target_unit_xyz: xyz,
            personnel_count: tpl.personnel_count,
            equipment_mass_kg: f64::from(tpl.personnel_count) * tpl.equipment_kg_per_troop,
            munitions_days: 14.0,
            rations_days: 14.0,
            cohesion_01: 0.92,
            doctrine_key,
            no_retreat_order: false,
            encircled: false,
            insubordinate_withdrawal: false,
            stale_order_ticks: 0,
        });
        Ok(fid)
    }

    /// Rejects illegal individual-soldier or hex-micro commands (`ENG-WAR-001.1`).
    pub fn reject_tactical_micro(&self, reason: &str) -> Result<(), String> {
        Err(format!("ERR_TACTICAL_MICRO_FORBIDDEN: {reason}"))
    }

    /// High-level Churchill-mode theater directive allowing autonomous operational discretion (`ENG-WAR-002.1`).
    #[allow(clippy::too_many_arguments)]
    pub fn issue_theater_directive(
        &mut self,
        theater_id: TheaterId,
        commander: CharacterId,
        objective_cell: CellId,
        supply_priority_01: f32,
        tick: Tick,
        planet: &dyn PlanetGeometryPort,
        registry: &dyn ModRegistryPort,
    ) {
        if let Some(th) = self.theaters.iter_mut().find(|t| t.theater_id == theater_id) {
            th.commander = commander;
            th.objective_cell = objective_cell;
            th.supply_priority_01 = supply_priority_01.clamp(0.1, 1.0);
            th.issued_tick = tick;
        } else {
            self.theaters.push(TheaterDirectiveView {
                theater_id,
                commander,
                objective_cell,
                supply_priority_01: supply_priority_01.clamp(0.1, 1.0),
                issued_tick: tick,
                autonomous_withdrawals: 0,
            });
        }
        let elastic_doc = registry
            .doctrines()
            .iter()
            .max_by(|a, b| a.initiative_bonus.total_cmp(&b.initiative_bonus))
            .map(|d| d.key.clone());

        let obj_xyz = planet.cell_unit_xyz(objective_cell);
        for f in &mut self.formations {
            if f.theater_id == theater_id {
                f.commander = commander;
                f.no_retreat_order = false;
                f.target_cell = objective_cell;
                f.target_unit_xyz = obj_xyz;
                if let Some(doc) = &elastic_doc {
                    f.doctrine_key = doc.clone();
                }
            }
        }
    }

    /// Rigid Hitler-mode operational battle plan queued via physical courier delay (`ENG-WAR-002.2`, `ENG-WAR-003.1`).
    #[allow(clippy::too_many_arguments)]
    pub fn issue_operational_battle_plan(
        &mut self,
        formation_id: FormationId,
        target_cell: CellId,
        doctrine_key: String,
        no_retreat: bool,
        tick: Tick,
        courier_delay_hours: u64,
        planet: &dyn PlanetGeometryPort,
    ) -> Result<Tick, String> {
        if !self.formations.iter().any(|f| f.formation_id == formation_id) {
            return Err(format!("Unknown formation {:?}", formation_id));
        }
        let delivery_tick = Tick(tick.0.saturating_add(courier_delay_hours.max(1)));
        self.pending_orders.push(PendingOperationalDispatch {
            formation_id,
            delivery_tick,
            target_cell,
            target_unit_xyz: planet.cell_unit_xyz(target_cell),
            doctrine_key,
            no_retreat,
        });
        Ok(delivery_tick)
    }

    /// Steps courier order delivery, Churchill/Hitler doctrine reaction, mud/rain/flood physics,
    /// supply consumption, encirclement collapse, and subordinate insubordination (`ENG-WAR-001..004`).
    pub fn step_with_world_context(
        &mut self,
        tick: Tick,
        planet: &dyn PlanetGeometryPort,
        weather: &dyn SurfaceConditionsPort,
        registry: &dyn ModRegistryPort,
        supply_line_available: bool,
    ) {
        self.last_tick = tick;

        // 1. Deliver queued courier battle orders whose transit elapsed (`ENG-WAR-003.1`).
        let mut remaining_orders = Vec::with_capacity(self.pending_orders.len());
        for ord in self.pending_orders.drain(..) {
            if tick.0 >= ord.delivery_tick.0 {
                if let Some(f) = self
                    .formations
                    .iter_mut()
                    .find(|f| f.formation_id == ord.formation_id)
                {
                    f.target_cell = ord.target_cell;
                    f.target_unit_xyz = ord.target_unit_xyz;
                    f.doctrine_key = ord.doctrine_key;
                    f.no_retreat_order = ord.no_retreat;
                    f.insubordinate_withdrawal = false;
                }
            } else {
                remaining_orders.push(ord);
            }
        }
        self.pending_orders = remaining_orders;

        let radius_km = planet.radius_km();

        // 2. Step each active formation continuously on the spherical globe.
        for f in &mut self.formations {
            let surf = weather.surface(f.current_cell);
            let doc_mitigation = registry
                .doctrines()
                .iter()
                .find(|d| d.key == f.doctrine_key)
                .map(|d| d.rain_penalty_mitigation)
                .unwrap_or(0.0);

            // Hourly mud/flood/rain combat & mobility attenuation (`ENG-WAR-004.1`).
            let mud_pen = (surf.mud_index_01 * (1.0 - doc_mitigation * 0.5)).clamp(0.0, 0.95);
            let flood_pen = if surf.flood_stage_01 >= 0.4 {
                0.45
            } else {
                0.0
            };
            let rain_pen = (surf.precip_mm_hr / 25.0 * (1.0 - doc_mitigation)).clamp(0.0, 0.55);
            let weather_mobility = (1.0 - 0.65 * mud_pen - flood_pen * 0.5 - rain_pen * 0.4).clamp(0.12, 1.0);

            let pressure = self
                .hostile_pressure_by_cell
                .get(&f.current_cell)
                .copied()
                .unwrap_or(0.0)
                .max(
                    self.hostile_pressure_by_cell
                        .get(&f.target_cell)
                        .copied()
                        .unwrap_or(0.0),
                );

            let home_xyz = self
                .home_unit_xyz
                .get(&f.formation_id)
                .copied()
                .unwrap_or(f.current_unit_xyz);
            let home_cid = self
                .home_cell
                .get(&f.formation_id)
                .copied()
                .unwrap_or(f.current_cell);

            // Evaluate Churchill autonomous retreat vs Hitler No-Retreat encirclement vs Insubordination.
            let (init_01, loyal_01) = self
                .commander_traits
                .get(&f.commander)
                .copied()
                .unwrap_or((0.55, 0.60));

            if pressure >= 0.60 {
                if !f.no_retreat_order {
                    // Theater Directive mode: commander autonomously breaks contact & withdraws (`ENG-WAR-002.1`).
                    if f.target_cell != home_cid {
                        f.target_cell = home_cid;
                        f.target_unit_xyz = home_xyz;
                        if let Some(th) = self.theaters.iter_mut().find(|t| t.theater_id == f.theater_id) {
                            th.autonomous_withdrawals += 1;
                        }
                    }
                    f.encircled = false;
                } else if init_01 > loyal_01 + 0.15 && f.personnel_count > 0 {
                    // High-initiative commander defies suicidal No-Retreat order (`ENG-WAR-003.2`).
                    f.no_retreat_order = false;
                    f.insubordinate_withdrawal = true;
                    f.encircled = false;
                    f.target_cell = home_cid;
                    f.target_unit_xyz = home_xyz;
                } else {
                    // Strictly bound by stale "No Retreat" order -> pocket encirclement (`ENG-WAR-003.1`).
                    f.encircled = true;
                    f.stale_order_ticks += 1;
                }
            } else {
                f.encircled = false;
            }

            // Continuous spherical marching (`ENG-GEO-001.1` / `ENG-WAR-001.2`).
            let rem_km = great_circle_arc_km(f.current_unit_xyz, f.target_unit_xyz, radius_km);
            if rem_km > 0.25 && f.cohesion_01 > 0.08 && !f.encircled {
                let speed_kmh = 4.2 * weather_mobility;
                let step_t = (speed_kmh / rem_km).clamp(0.0, 1.0);
                f.current_unit_xyz = slerp_unit_xyz(f.current_unit_xyz, f.target_unit_xyz, step_t);
                f.current_cell = planet.nearest_sample_cell(f.current_unit_xyz);
            }

            // Physical supply consumption vs logistics replenishment (`ENG-WAR-004.2`).
            let ammo_burn = if pressure > 0.1 { 0.08 } else { 0.01 };
            let ration_burn = 1.0 / 24.0;
            f.munitions_days = (f.munitions_days - ammo_burn).max(0.0);
            f.rations_days = (f.rations_days - ration_burn).max(0.0);

            if supply_line_available && !f.encircled {
                let priority = self
                    .theaters
                    .iter()
                    .find(|t| t.theater_id == f.theater_id)
                    .map(|t| t.supply_priority_01)
                    .unwrap_or(0.6);
                f.munitions_days = (f.munitions_days + 0.10 * priority).min(18.0);
                f.rations_days = (f.rations_days + 0.06 * priority).min(18.0);
            }

            if f.encircled || f.munitions_days <= 0.0 || f.rations_days <= 0.0 {
                let attrition_frac = if f.encircled { 0.018 } else { 0.008 };
                let loss = ((f.personnel_count as f32) * attrition_frac).ceil() as u32;
                f.personnel_count = f.personnel_count.saturating_sub(loss.min(f.personnel_count));
                f.cohesion_01 = (f.cohesion_01 - if f.encircled { 0.025 } else { 0.010 }).max(0.0);
            } else {
                // Weather still stresses cohesion slightly during wet mud marches (`ENG-WAR-004.1`).
                let weather_drain = 0.004 * mud_pen;
                f.cohesion_01 = (f.cohesion_01 + 0.003 - weather_drain).clamp(0.05, 1.0);
            }
        }
    }
}

impl OperationalCombatPlugPort for OperationalWarfarePlugin {
    fn step_hourly_combat(&mut self, tick: Tick) {
        self.last_tick = tick;
    }

    fn theaters(&self) -> &[TheaterDirectiveView] {
        &self.theaters
    }

    fn formations(&self) -> &[MilitaryFormationView] {
        &self.formations
    }
}

impl CanonicalSaveablePort for OperationalWarfarePlugin {
    fn cartridge_tag(&self) -> &'static str {
        "sim-plug-warfare"
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        write_u32_le(out, self.next_formation_id);
        write_u64_le(out, self.last_tick.0);

        write_u32_le(out, self.theaters.len() as u32);
        for t in &self.theaters {
            write_u16_le(out, t.theater_id.0);
            write_u32_le(out, t.commander.0);
            write_u32_le(out, t.objective_cell.0);
            write_f32_bits(out, t.supply_priority_01);
            write_u64_le(out, t.issued_tick.0);
            write_u32_le(out, t.autonomous_withdrawals);
        }

        write_u32_le(out, self.formations.len() as u32);
        for f in &self.formations {
            write_u32_le(out, f.formation_id.0);
            write_str(out, &f.label);
            write_str(out, &f.template_key);
            write_u16_le(out, f.theater_id.0);
            write_u32_le(out, f.commander.0);
            write_u32_le(out, f.home_settlement.0);
            write_u32_le(out, f.current_cell.0);
            for &v in &f.current_unit_xyz {
                write_f32_bits(out, v);
            }
            write_u32_le(out, f.target_cell.0);
            for &v in &f.target_unit_xyz {
                write_f32_bits(out, v);
            }
            write_u32_le(out, f.personnel_count);
            write_f64_bits(out, f.equipment_mass_kg);
            write_f32_bits(out, f.munitions_days);
            write_f32_bits(out, f.rations_days);
            write_f32_bits(out, f.cohesion_01);
            write_str(out, &f.doctrine_key);
            write_u8(out, u8::from(f.no_retreat_order));
            write_u8(out, u8::from(f.encircled));
            write_u8(out, u8::from(f.insubordinate_withdrawal));
            write_u64_le(out, f.stale_order_ticks);
        }

        write_u32_le(out, self.home_cell.len() as u32);
        for (fid, cid) in &self.home_cell {
            write_u32_le(out, fid.0);
            write_u32_le(out, cid.0);
            let xyz = self.home_unit_xyz.get(fid).copied().unwrap_or([0.0, 1.0, 0.0]);
            for v in xyz {
                write_f32_bits(out, v);
            }
        }

        write_u32_le(out, self.commander_traits.len() as u32);
        for (cid, &(init_01, loyal_01)) in &self.commander_traits {
            write_u32_le(out, cid.0);
            write_f32_bits(out, init_01);
            write_f32_bits(out, loyal_01);
        }

        write_u32_le(out, self.hostile_pressure_by_cell.len() as u32);
        for (cid, &p) in &self.hostile_pressure_by_cell {
            write_u32_le(out, cid.0);
            write_f32_bits(out, p);
        }

        write_u32_le(out, self.pending_orders.len() as u32);
        for o in &self.pending_orders {
            write_u32_le(out, o.formation_id.0);
            write_u64_le(out, o.delivery_tick.0);
            write_u32_le(out, o.target_cell.0);
            for &v in &o.target_unit_xyz {
                write_f32_bits(out, v);
            }
            write_str(out, &o.doctrine_key);
            write_u8(out, u8::from(o.no_retreat));
        }
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        self.next_formation_id = read_u32_le(input)?;
        self.last_tick = Tick(read_u64_le(input)?);

        let tlen = read_u32_le(input)? as usize;
        self.theaters.clear();
        for _ in 0..tlen {
            self.theaters.push(TheaterDirectiveView {
                theater_id: TheaterId(read_u16_le(input)?),
                commander: CharacterId(read_u32_le(input)?),
                objective_cell: CellId(read_u32_le(input)?),
                supply_priority_01: read_f32_bits(input)?,
                issued_tick: Tick(read_u64_le(input)?),
                autonomous_withdrawals: read_u32_le(input)?,
            });
        }

        let flen = read_u32_le(input)? as usize;
        self.formations.clear();
        for _ in 0..flen {
            self.formations.push(MilitaryFormationView {
                formation_id: FormationId(read_u32_le(input)?),
                label: read_str(input)?,
                template_key: read_str(input)?,
                theater_id: TheaterId(read_u16_le(input)?),
                commander: CharacterId(read_u32_le(input)?),
                home_settlement: SettlementId(read_u32_le(input)?),
                current_cell: CellId(read_u32_le(input)?),
                current_unit_xyz: [read_f32_bits(input)?, read_f32_bits(input)?, read_f32_bits(input)?],
                target_cell: CellId(read_u32_le(input)?),
                target_unit_xyz: [read_f32_bits(input)?, read_f32_bits(input)?, read_f32_bits(input)?],
                personnel_count: read_u32_le(input)?,
                equipment_mass_kg: read_f64_bits(input)?,
                munitions_days: read_f32_bits(input)?,
                rations_days: read_f32_bits(input)?,
                cohesion_01: read_f32_bits(input)?,
                doctrine_key: read_str(input)?,
                no_retreat_order: read_u8(input)? != 0,
                encircled: read_u8(input)? != 0,
                insubordinate_withdrawal: read_u8(input)? != 0,
                stale_order_ticks: read_u64_le(input)?,
            });
        }

        let hlen = read_u32_le(input)? as usize;
        self.home_cell.clear();
        self.home_unit_xyz.clear();
        for _ in 0..hlen {
            let fid = FormationId(read_u32_le(input)?);
            let cid = CellId(read_u32_le(input)?);
            let xyz = [read_f32_bits(input)?, read_f32_bits(input)?, read_f32_bits(input)?];
            self.home_cell.insert(fid, cid);
            self.home_unit_xyz.insert(fid, xyz);
        }

        let trlen = read_u32_le(input)? as usize;
        self.commander_traits.clear();
        for _ in 0..trlen {
            self.commander_traits.insert(
                CharacterId(read_u32_le(input)?),
                (read_f32_bits(input)?, read_f32_bits(input)?),
            );
        }

        let plen = read_u32_le(input)? as usize;
        self.hostile_pressure_by_cell.clear();
        for _ in 0..plen {
            self.hostile_pressure_by_cell
                .insert(CellId(read_u32_le(input)?), read_f32_bits(input)?);
        }

        let olen = read_u32_le(input)? as usize;
        self.pending_orders.clear();
        for _ in 0..olen {
            self.pending_orders.push(PendingOperationalDispatch {
                formation_id: FormationId(read_u32_le(input)?),
                delivery_tick: Tick(read_u64_le(input)?),
                target_cell: CellId(read_u32_le(input)?),
                target_unit_xyz: [read_f32_bits(input)?, read_f32_bits(input)?, read_f32_bits(input)?],
                doctrine_key: read_str(input)?,
                no_retreat: read_u8(input)? != 0,
            });
        }
        Ok(())
    }
}
