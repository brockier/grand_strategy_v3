//! M6 Plug Cartridge: Empirical Know-How Emergence, Theoretical Know-Why,
//! Institutional Innovation Modes, Physical Carriers, and Fragile Regression.
//! Strictly free of science points, tech trees, and target-picking shortcuts.

#![deny(clippy::disallowed_types)]

use std::collections::BTreeMap;

use sim_spine::codec::{
    read_f32_bits, read_str, read_u8, read_u16_le, read_u32_le, read_u64_le, write_f32_bits,
    write_str, write_u8, write_u16_le, write_u32_le, write_u64_le,
};
use sim_spine::ids::{ActorId, ProcessId, SettlementId, TechComponentId, TheoryDomainId, Tick};
use sim_spine::plugs::{
    KnowledgeCarrierKind, KnowledgeCarrierRecordView, KnowledgeModifierPlugPort,
    SettlementKnowledgeView,
};
use sim_spine::ports::{CanonicalSaveablePort, ModRegistryPort};

const VALID_MODES: [&str; 4] = [
    "guild_secrecy",
    "state_arsenal",
    "open_scholarly_patronage",
    "private_patent_exclusivity",
];

#[derive(Clone, Debug)]
struct ComponentMeta {
    id: TechComponentId,
    key: String,
    domain_key: String,
    difficulty: f32,
}

#[derive(Clone, Debug)]
struct DomainMeta {
    id: TheoryDomainId,
    key: String,
    abstraction_depth: u8,
}

#[derive(Clone, Debug)]
struct SettlementKnowledgeInternal {
    settlement: SettlementId,
    innovation_mode_key: String,
    bottleneck_pressure_01: f32,
    leisure_surplus_01: f32,
    know_how: BTreeMap<TechComponentId, f32>,
    know_why: BTreeMap<TheoryDomainId, f32>,
    active_patent_holder: Option<ActorId>,
    patent_expiry_tick: Option<Tick>,
    regression_losses: u32,
}

/// Concrete M6 plug implementing emergent, carrier-bound, regressive knowledge evolution.
#[derive(Clone, Debug, Default)]
pub struct KnowledgeEvolutionPlugin {
    components: Vec<ComponentMeta>,
    domains: Vec<DomainMeta>,
    settlements: BTreeMap<SettlementId, SettlementKnowledgeInternal>,
    views_cache: Vec<SettlementKnowledgeView>,
    carriers: Vec<KnowledgeCarrierRecordView>,
    gated_process_requirements: BTreeMap<ProcessId, (TechComponentId, f32)>,
    patent_duration_hours: u64,
    last_tick: Tick,
}

fn carrier_to_u8(k: KnowledgeCarrierKind) -> u8 {
    k as u8
}

fn u8_to_carrier(v: u8) -> Result<KnowledgeCarrierKind, String> {
    match v {
        0 => Ok(KnowledgeCarrierKind::PractitionerCohort),
        1 => Ok(KnowledgeCarrierKind::MasterIndividual),
        2 => Ok(KnowledgeCarrierKind::PhysicalTreatise),
        3 => Ok(KnowledgeCarrierKind::WorkingPrototypeArtifact),
        _ => Err(format!("Invalid KnowledgeCarrierKind tag {v}")),
    }
}

impl KnowledgeEvolutionPlugin {
    /// Constructs M6 knowledge state strictly from externalized `ModRegistryPort` catalogs.
    #[must_use]
    pub fn from_registry(registry: &dyn ModRegistryPort, settlement_ids: &[SettlementId]) -> Self {
        let components: Vec<ComponentMeta> = registry
            .tech_components()
            .iter()
            .map(|c| ComponentMeta {
                id: c.id,
                key: c.key.clone(),
                domain_key: c.domain_key.clone(),
                difficulty: c.empirical_difficulty.max(0.10),
            })
            .collect();

        let domains: Vec<DomainMeta> = registry
            .theory_domains()
            .iter()
            .map(|d| DomainMeta {
                id: d.id,
                key: d.key.clone(),
                abstraction_depth: d.abstraction_depth.max(1),
            })
            .collect();

        // Bind complex industrial/mechanical processes dynamically to loaded TechComponents
        // without hardcoding any mod process key or string literal (`ENG-MOD-001`, `ENG-RES-005`).
        let mut gated_process_requirements = BTreeMap::new();
        if !components.is_empty() {
            let mut adv_comp_idx = 0usize;
            for pidx in 0..registry.process_count() {
                let pid = ProcessId(pidx as u16);
                if let Some(pdef) = registry.process_def(pid)
                    && (pdef.workforce_hours_required >= 28.0 || pdef.tool_wear_intensity >= 0.0005)
                {
                    let comp = &components[adv_comp_idx % components.len()];
                    let req_level = if comp.difficulty >= 0.45 { 0.40 } else { 0.28 };
                    gated_process_requirements.insert(pid, (comp.id, req_level));
                    adv_comp_idx += 1;
                }
            }
        }

        let cal = registry.calendar_config();
        let hours_per_year = u64::from(cal.hours_per_day.max(1)) * u64::from(cal.days_per_year.max(1));
        let default_patent_years = registry
            .policy_laws()
            .iter()
            .map(|l| u64::from(l.patent_duration_years))
            .max()
            .unwrap_or(5)
            .max(1);

        let mut settlements = BTreeMap::new();
        let mut carriers = Vec::new();
        for &sid in settlement_ids {
            let mut kh = BTreeMap::new();
            for c in &components {
                // Early practical primitives start partially known by founding practitioners;
                // high-difficulty primitives begin unperfected (`0.08`).
                let init_kh = if c.difficulty < 0.40 { 0.42 } else { 0.08 };
                kh.insert(c.id, init_kh);
                if init_kh >= 0.30 {
                    carriers.push(KnowledgeCarrierRecordView {
                        settlement: sid,
                        component: c.id,
                        component_key: c.key.clone(),
                        carrier_kind: KnowledgeCarrierKind::PractitionerCohort,
                        retention_strength_01: init_kh,
                        holder_label: format!("Artisan Guild Bench #{}", sid.0),
                    });
                    carriers.push(KnowledgeCarrierRecordView {
                        settlement: sid,
                        component: c.id,
                        component_key: c.key.clone(),
                        carrier_kind: KnowledgeCarrierKind::PhysicalTreatise,
                        retention_strength_01: 0.65,
                        holder_label: format!("Field Notebook #{}", sid.0),
                    });
                }
            }
            let mut kw = BTreeMap::new();
            for d in &domains {
                kw.insert(d.id, 0.05);
            }
            settlements.insert(
                sid,
                SettlementKnowledgeInternal {
                    settlement: sid,
                    innovation_mode_key: VALID_MODES[0].to_string(),
                    bottleneck_pressure_01: 0.0,
                    leisure_surplus_01: 0.0,
                    know_how: kh,
                    know_why: kw,
                    active_patent_holder: None,
                    patent_expiry_tick: None,
                    regression_losses: 0,
                },
            );
        }

        let mut plugin = Self {
            components,
            domains,
            settlements,
            views_cache: Vec::new(),
            carriers,
            gated_process_requirements,
            patent_duration_hours: hours_per_year * default_patent_years,
            last_tick: Tick(0),
        };
        plugin.rebuild_views_cache();
        plugin
    }

    /// Configures a settlement's institutional innovation posture (`ENG-RES-003.1`).
    /// Never accepts tech targets or science-point purchases (`ENG-RES-001.1`).
    pub fn set_innovation_mode(
        &mut self,
        settlement: SettlementId,
        mode_key: &str,
        tick: Tick,
        patent_holder_hint: Option<ActorId>,
    ) -> Result<(), String> {
        if !VALID_MODES.contains(&mode_key) {
            return Err(format!(
                "ERR_BOUGHT_TECH_FORBIDDEN: invalid innovation mode '{mode_key}'"
            ));
        }
        let st = self
            .settlements
            .get_mut(&settlement)
            .ok_or_else(|| format!("Unknown settlement {:?}", settlement))?;
        st.innovation_mode_key = mode_key.to_string();
        if mode_key == "private_patent_exclusivity" {
            st.active_patent_holder = Some(patent_holder_hint.unwrap_or(ActorId(100)));
            st.patent_expiry_tick = Some(Tick(tick.0.saturating_add(self.patent_duration_hours)));
        } else {
            st.active_patent_holder = None;
            st.patent_expiry_tick = None;
        }
        self.rebuild_views_cache();
        Ok(())
    }

    /// Rejects any command attempting to spend currency/points to buy or select a target tech (`ENG-RES-001.1`).
    pub fn reject_bought_tech_request(&self) -> Result<(), String> {
        Err("ERR_BOUGHT_TECH_FORBIDDEN: science-point pools, tech trees, and bought tech targets are structurally forbidden".to_string())
    }

    /// Steps empirical Know-How emergence, cross-pollination, Know-Why leisure inquiry,
    /// patent expiration, and carrier upkeep across settlements (`ENG-RES-001..005`).
    pub fn step_settlement_knowledge(
        &mut self,
        tick: Tick,
        settlement: SettlementId,
        active_production_hours: f32,
        bottleneck_pressure_01: f32,
        leisure_surplus_01: f32,
    ) {
        self.last_tick = tick;
        let Some(st) = self.settlements.get_mut(&settlement) else {
            return;
        };
        st.bottleneck_pressure_01 = bottleneck_pressure_01.clamp(0.0, 1.0);
        st.leisure_surplus_01 = leisure_surplus_01.clamp(0.0, 1.0);

        if let Some(exp) = st.patent_expiry_tick
            && tick.0 >= exp.0
        {
            st.active_patent_holder = None;
            st.patent_expiry_tick = None;
        }

        // Cross-pollination boost when complementary components are already practiced locally (`ENG-RES-002.1`).
        let practiced_count = st.know_how.values().filter(|&&v| v >= 0.25).count();
        let cross_poll_mult = if practiced_count >= 2 {
            1.0 + 0.40 * ((practiced_count - 1).min(4) as f32)
        } else {
            1.0
        };

        // Know-Why advances ONLY under genuine specialist leisure surplus (`ENG-RES-002.2`).
        if st.leisure_surplus_01 > 0.12 {
            for dom in &self.domains {
                let rate = 0.0025 * (st.leisure_surplus_01 - 0.10)
                    / f32::from(dom.abstraction_depth)
                    * match st.innovation_mode_key.as_str() {
                        "open_scholarly_patronage" => 1.75,
                        "state_arsenal" => 1.15,
                        _ => 0.85,
                    };
                let entry = st.know_why.entry(dom.id).or_insert(0.0);
                *entry = (*entry + rate).clamp(0.0, 1.0);
                if *entry >= 0.25
                    && let Some(dom_comp) = self.components.iter().find(|c| c.domain_key == dom.key)
                {
                    let has_treatise = self.carriers.iter().any(|rec| {
                        rec.settlement == settlement
                            && rec.component == dom_comp.id
                            && rec.carrier_kind == KnowledgeCarrierKind::PhysicalTreatise
                    });
                    if !has_treatise {
                        self.carriers.push(KnowledgeCarrierRecordView {
                            settlement,
                            component: dom_comp.id,
                            component_key: dom_comp.key.clone(),
                            carrier_kind: KnowledgeCarrierKind::PhysicalTreatise,
                            retention_strength_01: *entry,
                            holder_label: format!("Scholarly Treatise ({})", dom.key),
                        });
                    }
                }
            }
        }

        // Empirical Know-How advances strictly from active production work + bottleneck pressure (`ENG-RES-001.2`).
        if active_production_hours > 0.0 {
            let mode_rate = match st.innovation_mode_key.as_str() {
                "state_arsenal" => 1.35,
                "private_patent_exclusivity" => 1.25,
                "open_scholarly_patronage" => 1.15,
                _ => 1.0,
            };
            let stress_mult = 1.0 + 2.4 * st.bottleneck_pressure_01;
            let work_factor = (active_production_hours / 40.0).clamp(0.05, 3.0);

            for comp in &self.components {
                let dom_know_why = self
                    .domains
                    .iter()
                    .find(|d| d.key == comp.domain_key)
                    .and_then(|d| st.know_why.get(&d.id).copied())
                    .unwrap_or(0.0);
                let why_above_base = ((dom_know_why - 0.05).max(0.0) / 0.95).clamp(0.0, 1.0);
                let ceiling = (0.72 + 0.28 * why_above_base).clamp(0.72, 1.0);
                let theory_accel = 1.0 + 1.2 * why_above_base;
                let cur = st.know_how.entry(comp.id).or_insert(0.0);
                let delta =
                    (0.0018 * work_factor * stress_mult * cross_poll_mult * mode_rate * theory_accel)
                        / comp.difficulty.max(0.1);
                if *cur < ceiling {
                    *cur = (*cur + delta).min(ceiling);
                }

                // Spawn physical treatise / prototype carriers once mastery matures.
                if *cur >= 0.45 {
                    let has_proto = self.carriers.iter().any(|rec| {
                        rec.settlement == settlement
                            && rec.component == comp.id
                            && rec.carrier_kind == KnowledgeCarrierKind::WorkingPrototypeArtifact
                    });
                    if !has_proto {
                        self.carriers.push(KnowledgeCarrierRecordView {
                            settlement,
                            component: comp.id,
                            component_key: comp.key.clone(),
                            carrier_kind: KnowledgeCarrierKind::WorkingPrototypeArtifact,
                            retention_strength_01: *cur,
                            holder_label: format!("Workshop Prototype ({})", comp.key),
                        });
                    }
                }
            }
        }
        self.rebuild_views_cache();
    }

    /// Propagates partial practitioner/treatise carriers from `from` to `to` upon courier or migrant arrival (`ENG-RES-004.2`).
    pub fn diffuse_knowledge_carrier(
        &mut self,
        from: SettlementId,
        to: SettlementId,
        transfer_factor_01: f32,
    ) {
        let Some(src) = self.settlements.get(&from).cloned() else {
            return;
        };
        let Some(dst) = self.settlements.get_mut(&to) else {
            return;
        };
        let mode_barrier = match src.innovation_mode_key.as_str() {
            "guild_secrecy" => 0.30,
            "open_scholarly_patronage" => 1.0,
            _ => 0.65,
        };
        let mult = (transfer_factor_01 * mode_barrier).clamp(0.05, 0.85);
        for (cid, &src_kh) in &src.know_how {
            if src_kh >= 0.25 {
                let dst_kh = dst.know_how.entry(*cid).or_insert(0.0);
                if src_kh * mult > *dst_kh {
                    *dst_kh = (src_kh * mult).clamp(0.0, 1.0);
                    let comp_key = self
                        .components
                        .iter()
                        .find(|c| c.id == *cid)
                        .map(|c| c.key.clone())
                        .unwrap_or_default();
                    let has_master = self.carriers.iter().any(|rec| {
                        rec.settlement == to
                            && rec.component == *cid
                            && rec.carrier_kind == KnowledgeCarrierKind::MasterIndividual
                    });
                    if !has_master {
                        self.carriers.push(KnowledgeCarrierRecordView {
                            settlement: to,
                            component: *cid,
                            component_key: comp_key,
                            carrier_kind: KnowledgeCarrierKind::MasterIndividual,
                            retention_strength_01: *dst_kh,
                            holder_label: format!("Travelling Master from Sector {}", from.0),
                        });
                    }
                }
            }
        }
        self.rebuild_views_cache();
    }

    /// Simulates loss/death of practitioner carriers and destruction/spoilage of unmaintained treatises & prototypes (`ENG-RES-005`).
    pub fn apply_carrier_loss(
        &mut self,
        settlement: SettlementId,
        lose_living_practitioners: bool,
        lose_treatises_and_prototypes: bool,
    ) {
        if lose_living_practitioners {
            self.carriers.retain(|rec| {
                !(rec.settlement == settlement
                    && matches!(
                        rec.carrier_kind,
                        KnowledgeCarrierKind::PractitionerCohort
                            | KnowledgeCarrierKind::MasterIndividual
                    ))
            });
        }
        if lose_treatises_and_prototypes {
            self.carriers.retain(|rec| {
                !(rec.settlement == settlement
                    && matches!(
                        rec.carrier_kind,
                        KnowledgeCarrierKind::PhysicalTreatise
                            | KnowledgeCarrierKind::WorkingPrototypeArtifact
                    ))
            });
        }

        if let Some(st) = self.settlements.get_mut(&settlement) {
            for (&cid, kh) in &mut st.know_how {
                let surviving_strength: f32 = self
                    .carriers
                    .iter()
                    .filter(|r| r.settlement == settlement && r.component == cid)
                    .map(|r| r.retention_strength_01)
                    .fold(0.0, f32::max);
                if surviving_strength <= 0.05 && *kh >= 0.25 {
                    // Complete regression when all carriers are destroyed or dispersed.
                    *kh = (*kh * 0.20).min(0.12);
                    st.regression_losses += 1;
                } else if lose_living_practitioners {
                    // Partial retention ONLY via treatises/prototypes allows slow rebuilding (`0.25..0.32`).
                    *kh = (*kh * 0.60).min(surviving_strength.max(0.22));
                }
            }
        }
        self.rebuild_views_cache();
    }

    /// Returns a settlement's know-how level for a given `TechComponentId`.
    #[must_use]
    pub fn settlement_know_how(&self, settlement: SettlementId, comp: TechComponentId) -> f32 {
        self.settlements
            .get(&settlement)
            .and_then(|s| s.know_how.get(&comp).copied())
            .unwrap_or(0.0)
    }

    /// Returns the mapped `(TechComponentId, min_threshold)` gating a given process if any.
    #[must_use]
    pub fn process_gate(&self, process: ProcessId) -> Option<(TechComponentId, f32)> {
        self.gated_process_requirements.get(&process).copied()
    }

    fn rebuild_views_cache(&mut self) {
        self.views_cache = self
            .settlements
            .values()
            .map(|s| SettlementKnowledgeView {
                settlement: s.settlement,
                innovation_mode_key: s.innovation_mode_key.clone(),
                bottleneck_pressure_01: s.bottleneck_pressure_01,
                leisure_surplus_01: s.leisure_surplus_01,
                know_how_levels: self
                    .components
                    .iter()
                    .map(|c| {
                        (
                            c.id,
                            c.key.clone(),
                            s.know_how.get(&c.id).copied().unwrap_or(0.0),
                        )
                    })
                    .collect(),
                know_why_levels: self
                    .domains
                    .iter()
                    .map(|d| {
                        (
                            d.id,
                            d.key.clone(),
                            s.know_why.get(&d.id).copied().unwrap_or(0.0),
                        )
                    })
                    .collect(),
                active_patent_holder: s.active_patent_holder,
                patent_expiry_tick: s.patent_expiry_tick,
                regression_losses: s.regression_losses,
            })
            .collect();
    }
}

impl KnowledgeModifierPlugPort for KnowledgeEvolutionPlugin {
    fn process_prerequisites_met(&self, settlement: SettlementId, process: ProcessId) -> bool {
        let Some(&(comp_id, req_level)) = self.gated_process_requirements.get(&process) else {
            return true;
        };
        let current = self.settlement_know_how(settlement, comp_id);
        let has_carrier = self
            .carriers
            .iter()
            .any(|c| c.settlement == settlement && c.component == comp_id);
        current >= req_level && has_carrier
    }

    fn settlement_knowledge_views(&self) -> &[SettlementKnowledgeView] {
        &self.views_cache
    }

    fn knowledge_carriers(&self) -> &[KnowledgeCarrierRecordView] {
        &self.carriers
    }

    fn actor_patent_permitted(
        &self,
        settlement: SettlementId,
        _component: TechComponentId,
        actor: ActorId,
    ) -> bool {
        let Some(st) = self.settlements.get(&settlement) else {
            return true;
        };
        if let (Some(holder), Some(exp)) = (st.active_patent_holder, st.patent_expiry_tick)
            && self.last_tick.0 < exp.0
            && actor != holder
        {
            return false;
        }
        true
    }
}

impl CanonicalSaveablePort for KnowledgeEvolutionPlugin {
    fn cartridge_tag(&self) -> &'static str {
        "sim-plug-research"
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        write_u64_le(out, self.patent_duration_hours);
        write_u64_le(out, self.last_tick.0);

        write_u32_le(out, self.components.len() as u32);
        for c in &self.components {
            write_u16_le(out, c.id.0);
            write_str(out, &c.key);
            write_str(out, &c.domain_key);
            write_f32_bits(out, c.difficulty);
        }

        write_u32_le(out, self.domains.len() as u32);
        for d in &self.domains {
            write_u16_le(out, d.id.0);
            write_str(out, &d.key);
            write_u8(out, d.abstraction_depth);
        }

        write_u32_le(out, self.settlements.len() as u32);
        for s in self.settlements.values() {
            write_u32_le(out, s.settlement.0);
            write_str(out, &s.innovation_mode_key);
            write_f32_bits(out, s.bottleneck_pressure_01);
            write_f32_bits(out, s.leisure_surplus_01);
            write_u8(out, u8::from(s.active_patent_holder.is_some()));
            write_u32_le(out, s.active_patent_holder.map(|a| a.0).unwrap_or(0));
            write_u8(out, u8::from(s.patent_expiry_tick.is_some()));
            write_u64_le(out, s.patent_expiry_tick.map(|t| t.0).unwrap_or(0));
            write_u32_le(out, s.regression_losses);

            write_u32_le(out, s.know_how.len() as u32);
            for (k, v) in &s.know_how {
                write_u16_le(out, k.0);
                write_f32_bits(out, *v);
            }
            write_u32_le(out, s.know_why.len() as u32);
            for (k, v) in &s.know_why {
                write_u16_le(out, k.0);
                write_f32_bits(out, *v);
            }
        }

        write_u32_le(out, self.carriers.len() as u32);
        for c in &self.carriers {
            write_u32_le(out, c.settlement.0);
            write_u16_le(out, c.component.0);
            write_str(out, &c.component_key);
            write_u8(out, carrier_to_u8(c.carrier_kind));
            write_f32_bits(out, c.retention_strength_01);
            write_str(out, &c.holder_label);
        }

        write_u32_le(out, self.gated_process_requirements.len() as u32);
        for (pid, (cid, req)) in &self.gated_process_requirements {
            write_u16_le(out, pid.0);
            write_u16_le(out, cid.0);
            write_f32_bits(out, *req);
        }
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        self.patent_duration_hours = read_u64_le(input)?;
        self.last_tick = Tick(read_u64_le(input)?);

        let clen = read_u32_le(input)? as usize;
        self.components.clear();
        for _ in 0..clen {
            self.components.push(ComponentMeta {
                id: TechComponentId(read_u16_le(input)?),
                key: read_str(input)?,
                domain_key: read_str(input)?,
                difficulty: read_f32_bits(input)?,
            });
        }

        let dlen = read_u32_le(input)? as usize;
        self.domains.clear();
        for _ in 0..dlen {
            self.domains.push(DomainMeta {
                id: TheoryDomainId(read_u16_le(input)?),
                key: read_str(input)?,
                abstraction_depth: read_u8(input)?,
            });
        }

        let slen = read_u32_le(input)? as usize;
        self.settlements.clear();
        for _ in 0..slen {
            let sid = SettlementId(read_u32_le(input)?);
            let innovation_mode_key = read_str(input)?;
            let bottleneck_pressure_01 = read_f32_bits(input)?;
            let leisure_surplus_01 = read_f32_bits(input)?;
            let has_pat = read_u8(input)? != 0;
            let holder = ActorId(read_u32_le(input)?);
            let has_exp = read_u8(input)? != 0;
            let exp = Tick(read_u64_le(input)?);
            let regression_losses = read_u32_le(input)?;
            let kh_len = read_u32_le(input)? as usize;
            let mut know_how = BTreeMap::new();
            for _ in 0..kh_len {
                know_how.insert(TechComponentId(read_u16_le(input)?), read_f32_bits(input)?);
            }
            let kw_len = read_u32_le(input)? as usize;
            let mut know_why = BTreeMap::new();
            for _ in 0..kw_len {
                know_why.insert(TheoryDomainId(read_u16_le(input)?), read_f32_bits(input)?);
            }
            self.settlements.insert(
                sid,
                SettlementKnowledgeInternal {
                    settlement: sid,
                    innovation_mode_key,
                    bottleneck_pressure_01,
                    leisure_surplus_01,
                    know_how,
                    know_why,
                    active_patent_holder: has_pat.then_some(holder),
                    patent_expiry_tick: has_exp.then_some(exp),
                    regression_losses,
                },
            );
        }

        let rlen = read_u32_le(input)? as usize;
        self.carriers.clear();
        for _ in 0..rlen {
            self.carriers.push(KnowledgeCarrierRecordView {
                settlement: SettlementId(read_u32_le(input)?),
                component: TechComponentId(read_u16_le(input)?),
                component_key: read_str(input)?,
                carrier_kind: u8_to_carrier(read_u8(input)?)?,
                retention_strength_01: read_f32_bits(input)?,
                holder_label: read_str(input)?,
            });
        }

        let glen = read_u32_le(input)? as usize;
        self.gated_process_requirements.clear();
        for _ in 0..glen {
            self.gated_process_requirements.insert(
                ProcessId(read_u16_le(input)?),
                (TechComponentId(read_u16_le(input)?), read_f32_bits(input)?),
            );
        }
        self.rebuild_views_cache();
        Ok(())
    }
}
