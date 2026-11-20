//! Modular command hierarchy, bounded sovereign attention, mandate decomposition, and escalation.
//!
//! Models Churchill-style strategic mandates vs Hitler-style direct focus, non-linear convex
//! oversight dilution, physical courier gating on order activation, and arbitration proposals.

#![deny(clippy::disallowed_types)]

use std::collections::BTreeMap;

use sim_spine::codec::{
    read_f32_bits, read_str, read_u8, read_u32_le, read_u64_le, write_f32_bits, write_str,
    write_u8, write_u32_le, write_u64_le,
};
use sim_spine::ids::{CellId, CharacterId, MandateId, SettlementId, Tick};
use sim_spine::ports::{
    AttentionLoadView, CanonicalSaveablePort, CharacterSnapshotView, CommandPort, MandateView,
    NotificationBehaviorKind, SpinePlayerCommand,
};

/// Lifecycle state of an issued sovereign or lieutenant mandate.
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
pub enum MandateStatus {
    /// Physical order rider is still traveling across terrain from HQ to assignee.
    InTransitCourier = 0,
    /// Order arrived; subordinate decomposed directive and is actively executing it.
    Active = 1,
    /// Mandate fulfilled or cleanly closed.
    Completed = 2,
    /// Mandate cancelled or superseded.
    Superseded = 3,
}

impl MandateStatus {
    /// Human-readable status token returned over `CommandPort`.
    pub fn as_str(&self) -> &'static str {
        match self {
            MandateStatus::InTransitCourier => "in_transit_courier",
            MandateStatus::Active => "active",
            MandateStatus::Completed => "completed",
            MandateStatus::Superseded => "superseded",
        }
    }
}

fn mandate_status_to_u8(s: MandateStatus) -> u8 {
    s as u8
}

fn u8_to_mandate_status(v: u8) -> Result<MandateStatus, String> {
    match v {
        0 => Ok(MandateStatus::InTransitCourier),
        1 => Ok(MandateStatus::Active),
        2 => Ok(MandateStatus::Completed),
        3 => Ok(MandateStatus::Superseded),
        _ => Err(format!("Invalid MandateStatus tag {v}")),
    }
}

/// Complete internal command mandate with priority, authority tier, decomposition steps, and efficiency.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandMandate {
    pub id: MandateId,
    pub issuer: CharacterId,
    pub assignee: CharacterId,
    pub target_settlement: SettlementId,
    pub target_cell: CellId,
    pub objective_kind: String,
    pub authority_tier: String,
    pub summary: String,
    pub priority_weight: u32,
    pub status: MandateStatus,
    pub issued_tick: Tick,
    pub activated_tick: Option<Tick>,
    pub subtasks: Vec<String>,
    pub realized_efficiency: f32,
    pub progress_01: f32,
}

impl CommandMandate {
    /// Converts internal mandate into immutable public `MandateView`.
    pub fn to_view(&self) -> MandateView {
        MandateView {
            id: self.id,
            issuer: self.issuer,
            assignee: self.assignee,
            target_settlement: self.target_settlement,
            summary: self.summary.clone(),
            status: self.status.as_str().to_owned(),
        }
    }
}

/// Option selectable by the player inside an `ArbitrationProposal` (`ENG-CMD-004`).
#[derive(Clone, Debug, PartialEq)]
pub struct ArbitrationOption {
    pub label: String,
    pub description: String,
    pub attention_cost: f32,
    pub priority_boost: i32,
    pub welfare_relief_delta: f32,
}

/// Resource or command bottleneck escalated by a subordinate lieutenant to HQ.
#[derive(Clone, Debug, PartialEq)]
pub struct ArbitrationProposal {
    pub proposal_id: u64,
    pub settlement: SettlementId,
    pub origin_cell: CellId,
    pub raised_by: CharacterId,
    pub issue_key: String,
    pub title: String,
    pub summary: String,
    pub created_tick: Tick,
    pub deadline_tick: Tick,
    pub options: Vec<ArbitrationOption>,
    pub chosen_option: Option<u32>,
}

/// Resolved effect returned when a player resolves an arbitration proposal.
#[derive(Clone, Debug, PartialEq)]
pub struct AppliedArbitrationEffect {
    pub proposal_id: u64,
    pub settlement: SettlementId,
    pub option_index: u32,
    pub chosen_label: String,
    pub welfare_relief_delta: f32,
    pub priority_boost: i32,
}

/// Queued physical courier dispatch required to carry a new order from HQ (`ENG-CMD-003`).
#[derive(Clone, Debug, PartialEq)]
pub struct PendingOrderCourier {
    pub mandate_id: MandateId,
    pub assignee: CharacterId,
    pub destination_cell: CellId,
    pub destination_settlement: SettlementId,
    pub subject: String,
    pub issued_tick: Tick,
}

/// Pending player command queued via `CommandPort` for ScenarioWorld orchestration.
#[derive(Clone, Debug, PartialEq)]
pub struct CommandWorldAction {
    pub tick: Tick,
    pub command: SpinePlayerCommand,
}

/// Deterministic command hierarchy, attention budget, delegation, and arbitration cartridge.
#[derive(Clone, Debug)]
pub struct CommandHierarchy {
    sovereign_id: CharacterId,
    base_attention_capacity: f32,
    clerical_overhead_per_mandate: f32,
    hierarchy_hop_latency_hours: u32,
    focused_mandate: Option<MandateId>,
    focused_attention_share: f32,
    mandates: Vec<CommandMandate>,
    cached_views_by_owner: BTreeMap<CharacterId, Vec<MandateView>>,
    proposals: Vec<ArbitrationProposal>,
    applied_resolutions: Vec<AppliedArbitrationEffect>,
    pending_order_couriers: Vec<PendingOrderCourier>,
    pending_world_actions: Vec<CommandWorldAction>,
    notification_rules: BTreeMap<String, NotificationBehaviorKind>,
    assignee_settlement_lookup: BTreeMap<CharacterId, (SettlementId, CellId)>,
    next_mandate_id: u64,
    next_proposal_id: u64,
    used_attention_by_owner: BTreeMap<CharacterId, f32>,
    blind_spot_penalty_by_settlement: BTreeMap<SettlementId, f32>,
}

impl Default for CommandHierarchy {
    fn default() -> Self {
        Self {
            sovereign_id: CharacterId(0),
            base_attention_capacity: 100.0,
            clerical_overhead_per_mandate: 8.5,
            hierarchy_hop_latency_hours: 4,
            focused_mandate: None,
            focused_attention_share: 0.0,
            mandates: Vec::new(),
            cached_views_by_owner: BTreeMap::new(),
            proposals: Vec::new(),
            applied_resolutions: Vec::new(),
            pending_order_couriers: Vec::new(),
            pending_world_actions: Vec::new(),
            notification_rules: BTreeMap::new(),
            assignee_settlement_lookup: BTreeMap::new(),
            next_mandate_id: 1,
            next_proposal_id: 1,
            used_attention_by_owner: BTreeMap::new(),
            blind_spot_penalty_by_settlement: BTreeMap::new(),
        }
    }
}

impl CommandHierarchy {
    /// Initializes bounded attention parameters and default notification matrix from scenario/mod config.
    pub fn with_config(
        base_attention_capacity: f32,
        clerical_overhead_per_mandate: f32,
        hierarchy_hop_latency_hours: u32,
        default_rules: BTreeMap<String, NotificationBehaviorKind>,
    ) -> Self {
        let mut h = Self {
            base_attention_capacity: base_attention_capacity.max(20.0),
            clerical_overhead_per_mandate: clerical_overhead_per_mandate.max(1.0),
            hierarchy_hop_latency_hours,
            notification_rules: default_rules,
            ..Self::default()
        };
        h.recompute_attention_and_efficiencies(&[]);
        h
    }

    /// Registers or refreshes subordinate settlement + cell destinations for order routing.
    pub fn sync_character_locations(
        &mut self,
        chars: &[CharacterSnapshotView],
        settlement_cells: &BTreeMap<SettlementId, CellId>,
    ) {
        self.assignee_settlement_lookup.clear();
        for ch in chars {
            let cell = settlement_cells
                .get(&ch.home_settlement)
                .copied()
                .unwrap_or(CellId(0));
            self.assignee_settlement_lookup
                .insert(ch.id, (ch.home_settlement, cell));
        }
    }

    /// Returns sovereign commander identifier (`CharacterId(0)`).
    pub fn sovereign_id(&self) -> CharacterId {
        self.sovereign_id
    }

    /// Returns configured base attention capacity.
    pub fn base_attention_capacity(&self) -> f32 {
        self.base_attention_capacity
    }

    /// Returns current personal zoom mandate ID and share `[0.0..=0.85]`.
    pub fn personal_focus_state(&self) -> (Option<MandateId>, f32) {
        (self.focused_mandate, self.focused_attention_share)
    }

    /// Reads current notification rule map keyed by `event_class_key`.
    pub fn notification_rules(&self) -> &BTreeMap<String, NotificationBehaviorKind> {
        &self.notification_rules
    }

    /// Reads all command mandates.
    pub fn all_mandates(&self) -> &[CommandMandate] {
        &self.mandates
    }

    /// Reads all arbitration proposals (open and resolved).
    pub fn all_proposals(&self) -> &[ArbitrationProposal] {
        &self.proposals
    }

    /// Returns currently unresolved arbitration proposals.
    pub fn open_proposals(&self) -> Vec<&ArbitrationProposal> {
        self.proposals
            .iter()
            .filter(|p| p.chosen_option.is_none())
            .collect()
    }

    /// Drains newly queued physical courier dispatches to carry issued orders.
    pub fn drain_pending_order_couriers(&mut self) -> Vec<PendingOrderCourier> {
        std::mem::take(&mut self.pending_order_couriers)
    }

    /// Drains world-level commands (HQ moves, tax/notification updates) for ScenarioWorld orchestration.
    pub fn drain_world_actions(&mut self) -> Vec<CommandWorldAction> {
        std::mem::take(&mut self.pending_world_actions)
    }

    /// Drains effects applied by resolved arbitration choices so ScenarioWorld applies welfare/priority shifts.
    pub fn drain_applied_resolutions(&mut self) -> Vec<AppliedArbitrationEffect> {
        std::mem::take(&mut self.applied_resolutions)
    }

    /// Activates an `InTransitCourier` mandate ONLY when the courier physically delivers the order packet.
    pub fn mark_order_delivered(&mut self, mandate_id: MandateId, arrived_tick: Tick) -> bool {
        let Some(m) = self.mandates.iter_mut().find(|m| m.id == mandate_id) else {
            return false;
        };
        if m.status == MandateStatus::InTransitCourier {
            m.status = MandateStatus::Active;
            m.activated_tick = Some(arrived_tick);
            if m.subtasks.is_empty() {
                m.subtasks = Self::decompose_objective(&m.objective_kind, &m.authority_tier);
            }
            self.rebuild_view_cache();
            return true;
        }
        false
    }

    /// Updates notification escalation matrix behavior cleanly.
    pub fn set_notification_rule(&mut self, event_key: String, behavior: NotificationBehaviorKind) {
        self.notification_rules.insert(event_key, behavior);
    }

    /// Updates effective governance capacity, clerical overhead, and hop latency parameters (`ENG-CMD-006`).
    pub fn set_institutional_parameters(
        &mut self,
        base_attention_capacity: f32,
        clerical_overhead_per_mandate: f32,
        hierarchy_hop_latency_hours: u32,
    ) {
        self.base_attention_capacity = base_attention_capacity.max(20.0);
        self.clerical_overhead_per_mandate = clerical_overhead_per_mandate.max(1.0);
        self.hierarchy_hop_latency_hours = hierarchy_hop_latency_hours;
    }

    /// Computes total clerical maintenance overhead strictly as a function of active mandates.
    pub fn clerical_overhead_total(&self) -> f32 {
        let active_count = self
            .mandates
            .iter()
            .filter(|m| matches!(m.status, MandateStatus::Active | MandateStatus::InTransitCourier))
            .count();
        (active_count as f32) * self.clerical_overhead_per_mandate
    }

    /// Computes non-linear convex oversight efficiency for a given settlement or commander mandate.
    pub fn settlement_oversight_multiplier(&self, sid: SettlementId) -> f32 {
        let matching: Vec<&CommandMandate> = self
            .mandates
            .iter()
            .filter(|m| m.target_settlement == sid && m.status == MandateStatus::Active)
            .collect();
        if matching.is_empty() {
            // Normal baseline autonomy without overstretched direct micromanagement
            return 1.0 - self.blind_spot_penalty_by_settlement.get(&sid).copied().unwrap_or(0.0);
        }
        let avg: f32 =
            matching.iter().map(|m| m.realized_efficiency).sum::<f32>() / (matching.len() as f32);
        avg.clamp(0.15, 1.30)
    }

    /// Raises an arbitration escalation proposal (mine flood, strike, or grain shortage) with `>= 2` options.
    #[allow(clippy::too_many_arguments)]
    pub fn escalate_bottleneck(
        &mut self,
        settlement: SettlementId,
        origin_cell: CellId,
        raised_by: CharacterId,
        issue_key: String,
        title: String,
        summary: String,
        current_tick: Tick,
        options: Vec<ArbitrationOption>,
    ) -> Option<u64> {
        // Avoid duplicating an already open identical issue in the same settlement
        if self
            .proposals
            .iter()
            .any(|p| p.settlement == settlement && p.issue_key == issue_key && p.chosen_option.is_none())
        {
            return None;
        }
        if options.len() < 2 {
            return None;
        }
        let proposal_id = self.next_proposal_id;
        self.next_proposal_id += 1;
        self.proposals.push(ArbitrationProposal {
            proposal_id,
            settlement,
            origin_cell,
            raised_by,
            issue_key,
            title,
            summary,
            created_tick: current_tick,
            deadline_tick: Tick(current_tick.0 + 48),
            options,
            chosen_option: None,
        });
        Some(proposal_id)
    }

    /// Recomputes bounded attention load, non-linear convex oversight dilution, and blind-spot penalties.
    pub fn recompute_attention_and_efficiencies(&mut self, chars: &[CharacterSnapshotView]) {
        let active_indices: Vec<usize> = self
            .mandates
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                matches!(
                    m.status,
                    MandateStatus::Active | MandateStatus::InTransitCourier
                )
            })
            .map(|(idx, _)| idx)
            .collect();

        let active_n = active_indices.len();
        let zoom_active = self.focused_mandate.is_some()
            && active_indices
                .iter()
                .any(|&i| Some(self.mandates[i].id) == self.focused_mandate);
        let zoom_share = if zoom_active {
            self.focused_attention_share.clamp(0.0, 0.85)
        } else {
            0.0
        };

        let mut total_used = (active_n as f32) * self.clerical_overhead_per_mandate;
        for &idx in &active_indices {
            let m = &self.mandates[idx];
            let micromanage_weight = if m.authority_tier == "no_retreat_operational"
                || m.authority_tier == "operational_micro"
            {
                14.0
            } else {
                6.5
            };
            let priority_cost = (m.priority_weight as f32).clamp(1.0, 10.0) * 1.4;
            total_used += micromanage_weight + priority_cost;
        }
        // Account for direct sovereign zoom focus allocation
        total_used += zoom_share * self.base_attention_capacity * 0.45;

        self.used_attention_by_owner
            .insert(self.sovereign_id, total_used);

        // Non-linear convex overload dilution (`ENG-CMD-002`):
        // As active mandate count / used load grows beyond capacity, efficiency falls quadratically.
        let load_ratio = total_used / self.base_attention_capacity.max(1.0);
        let excess = (load_ratio - 1.0).max(0.0);
        let overload_convex = 1.0 / (1.0 + 2.2 * excess * excess);
        let span_excess = (active_n as f32 - 2.0).max(0.0);
        let span_convex = 1.0 / (1.0 + 0.09 * span_excess * span_excess);

        // Available unzoomed attention pool shrinks when Personal Attention zooms onto 1 mandate (`ENG-CMD-005`)
        let non_focused_pool_mult = (1.0 - zoom_share * 0.72).clamp(0.22, 1.0);

        self.blind_spot_penalty_by_settlement.clear();
        for &idx in &active_indices {
            let assignee_id = self.mandates[idx].assignee;
            let subordinate = chars.iter().find(|c| c.id == assignee_id);
            let sub_comp = subordinate.map(|c| c.competence).unwrap_or(0.60);
            let sub_bonus = 0.75 + 0.45 * sub_comp;

            let is_focused = Some(self.mandates[idx].id) == self.focused_mandate;
            let focus_mult = if is_focused {
                1.0 + 0.38 * zoom_share
            } else if zoom_active {
                non_focused_pool_mult
            } else {
                1.0
            };

            let realized = (overload_convex * span_convex * sub_bonus * focus_mult).clamp(0.12, 1.35);
            self.mandates[idx].realized_efficiency = realized;

            if zoom_active && !is_focused {
                let penalty = (1.0 - non_focused_pool_mult) * 0.45;
                let entry = self
                    .blind_spot_penalty_by_settlement
                    .entry(self.mandates[idx].target_settlement)
                    .or_insert(0.0);
                *entry = (*entry).max(penalty);
            }
        }

        self.rebuild_view_cache();
    }

    /// Advances active mandates' progress and recomputes attention efficiencies for 1 hour.
    pub fn step_hourly(&mut self, _tick: Tick, chars: &[CharacterSnapshotView]) {
        for m in &mut self.mandates {
            if m.status == MandateStatus::Active {
                m.progress_01 = (m.progress_01 + 0.008 * m.realized_efficiency).clamp(0.0, 1.0);
            }
        }
        self.recompute_attention_and_efficiencies(chars);
    }

    fn decompose_objective(objective_kind: &str, authority_tier: &str) -> Vec<String> {
        match objective_kind {
            "stabilize_food" => vec![
                "Prioritize seed_field_cultivation labor allocation".to_owned(),
                "Protect granary roofs & draw cistern rations".to_owned(),
            ],
            "repair_mine" => vec![
                "Operate mine sump pumps to drop aquifer seepage".to_owned(),
                "Maintain hand tools & smelt iron reinforcement".to_owned(),
            ],
            "fortify_logistics" => vec![
                "Grade drained roadbeds & repair river bridges".to_owned(),
                "Stage pack fodder reserves at waystations".to_owned(),
            ],
            "geological_survey" => vec![
                "Dispatch scout party to exposed seam strata".to_owned(),
                "Assay sample core and dispatch report courier".to_owned(),
            ],
            _ => vec![
                format!("Coordinate subordinate crews for {objective_kind}"),
                format!("Audit stocks under {authority_tier} doctrine"),
            ],
        }
    }

    fn rebuild_view_cache(&mut self) {
        self.cached_views_by_owner.clear();
        for m in &self.mandates {
            let v = m.to_view();
            self.cached_views_by_owner
                .entry(m.issuer)
                .or_default()
                .push(v.clone());
            if m.assignee != m.issuer {
                self.cached_views_by_owner
                    .entry(m.assignee)
                    .or_default()
                    .push(v);
            }
        }
    }

    /// Processes a player command directly, queueing physical couriers for new mandates (`ENG-CMD-003`).
    pub fn apply_player_command(
        &mut self,
        tick: Tick,
        cmd: SpinePlayerCommand,
        chars: &[CharacterSnapshotView],
        settlement_cells: &BTreeMap<SettlementId, CellId>,
    ) {
        self.sync_character_locations(chars, settlement_cells);
        match cmd {
            SpinePlayerCommand::IssueMandate {
                assignee,
                objective_kind,
                target_cell,
                priority_weight,
                authority_tier,
            } => {
                let id = MandateId(self.next_mandate_id);
                self.next_mandate_id += 1;
                let assignee_loc = self
                    .assignee_settlement_lookup
                    .get(&assignee)
                    .copied()
                    .or_else(|| settlement_cells.iter().next().map(|(&s, &c)| (s, c)))
                    .unwrap_or((SettlementId(1), target_cell));
                let target_settlement = settlement_cells
                    .iter()
                    .find(|&(_, &c)| c == target_cell)
                    .map(|(&s, _)| s)
                    .unwrap_or(assignee_loc.0);
                let subtasks = Self::decompose_objective(&objective_kind, &authority_tier);
                let summary = format!(
                    "{} -> {:?} ({})",
                    objective_kind, target_settlement, authority_tier
                );
                self.mandates.push(CommandMandate {
                    id,
                    issuer: self.sovereign_id,
                    assignee,
                    target_settlement,
                    target_cell,
                    objective_kind: objective_kind.clone(),
                    authority_tier: authority_tier.clone(),
                    summary: summary.clone(),
                    priority_weight: priority_weight.clamp(1, 10),
                    status: MandateStatus::InTransitCourier,
                    issued_tick: tick,
                    activated_tick: None,
                    subtasks,
                    realized_efficiency: 1.0,
                    progress_01: 0.0,
                });
                self.pending_order_couriers.push(PendingOrderCourier {
                    mandate_id: id,
                    assignee,
                    destination_cell: assignee_loc.1,
                    destination_settlement: assignee_loc.0,
                    subject: format!("directive:{}:{}", id.0, objective_kind),
                    issued_tick: tick,
                });
                self.recompute_attention_and_efficiencies(chars);
            }
            SpinePlayerCommand::ResolveArbitrationProposal {
                proposal_id,
                chosen_option_index,
            } => {
                if let Some(prop) = self
                    .proposals
                    .iter_mut()
                    .find(|p| p.proposal_id == proposal_id && p.chosen_option.is_none())
                {
                    let idx = (chosen_option_index as usize).min(prop.options.len().saturating_sub(1));
                    prop.chosen_option = Some(idx as u32);
                    let chosen = prop.options[idx].clone();
                    self.applied_resolutions.push(AppliedArbitrationEffect {
                        proposal_id,
                        settlement: prop.settlement,
                        option_index: idx as u32,
                        chosen_label: chosen.label,
                        welfare_relief_delta: chosen.welfare_relief_delta,
                        priority_boost: chosen.priority_boost,
                    });
                    if chosen.priority_boost != 0 {
                        for m in &mut self.mandates {
                            if m.target_settlement == prop.settlement
                                && m.status == MandateStatus::Active
                            {
                                let next_w = (m.priority_weight as i32 + chosen.priority_boost)
                                    .clamp(1, 10) as u32;
                                m.priority_weight = next_w;
                            }
                        }
                    }
                    self.recompute_attention_and_efficiencies(chars);
                }
            }
            SpinePlayerCommand::SetPersonalAttentionZoom {
                target_mandate_id,
                share_01,
            } => {
                self.focused_mandate = target_mandate_id;
                self.focused_attention_share = if target_mandate_id.is_some() {
                    share_01.clamp(0.0, 0.85)
                } else {
                    0.0
                };
                self.recompute_attention_and_efficiencies(chars);
            }
            SpinePlayerCommand::UpdateNotificationRule {
                event_class_key,
                behavior,
            } => {
                self.notification_rules.insert(event_class_key, behavior);
            }
            other @ (SpinePlayerCommand::RelocateHq { .. }
            | SpinePlayerCommand::SetPaused(_)
            | SpinePlayerCommand::StepSingleTick
            | SpinePlayerCommand::SetSpeedMultiplier(_)
            | SpinePlayerCommand::AcknowledgeNotification(_)
            | SpinePlayerCommand::StubPing { .. }
            | SpinePlayerCommand::SwitchGovernanceModel { .. }
            | SpinePlayerCommand::EnactEconomicPolicy { .. }
            | SpinePlayerCommand::CharterPrivateCarriers { .. }
            | SpinePlayerCommand::SetInnovationMode { .. }
            | SpinePlayerCommand::IssueTheaterDirective { .. }
            | SpinePlayerCommand::IssueOperationalBattlePlan { .. }
            | SpinePlayerCommand::AcknowledgeSuccession
            | SpinePlayerCommand::ConstructDam { .. }
            | SpinePlayerCommand::SetDamRuleCurve { .. }) => {
                self.pending_world_actions
                    .push(CommandWorldAction { tick, command: other });
            }
        }
    }
}

impl CommandPort for CommandHierarchy {
    fn attention_summary(&self, who: CharacterId) -> AttentionLoadView {
        let active_mandate_count = self
            .mandates
            .iter()
            .filter(|m| {
                (m.issuer == who || m.assignee == who)
                    && matches!(
                        m.status,
                        MandateStatus::Active | MandateStatus::InTransitCourier
                    )
            })
            .count();
        let used = self.used_attention_by_owner.get(&who).copied().unwrap_or(0.0);
        AttentionLoadView {
            owner: who,
            used_attention: used,
            capacity_attention: self.base_attention_capacity,
            active_mandate_count,
        }
    }

    fn active_mandates_for(&self, who: CharacterId) -> &[MandateView] {
        self.cached_views_by_owner
            .get(&who)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn ingest_player_command(&mut self, tick: Tick, cmd: SpinePlayerCommand) {
        self.apply_player_command(tick, cmd, &[], &BTreeMap::new());
    }
}

fn behavior_to_u8(b: NotificationBehaviorKind) -> u8 {
    b as u8
}

fn u8_to_behavior(v: u8) -> Result<NotificationBehaviorKind, String> {
    match v {
        0 => Ok(NotificationBehaviorKind::LogOnly),
        1 => Ok(NotificationBehaviorKind::BannerAlert),
        2 => Ok(NotificationBehaviorKind::DispatchPopup),
        3 => Ok(NotificationBehaviorKind::DispatchPopupAndAutoPause),
        _ => Err(format!("Invalid NotificationBehaviorKind tag {v}")),
    }
}

impl CanonicalSaveablePort for CommandHierarchy {
    fn cartridge_tag(&self) -> &'static str {
        "sim-command"
    }

    fn write_canonical_state(&self, out: &mut Vec<u8>) {
        write_u32_le(out, self.sovereign_id.0);
        write_f32_bits(out, self.base_attention_capacity);
        write_f32_bits(out, self.clerical_overhead_per_mandate);
        write_u32_le(out, self.hierarchy_hop_latency_hours);
        write_u8(out, u8::from(self.focused_mandate.is_some()));
        write_u64_le(out, self.focused_mandate.map(|x| x.0).unwrap_or(0));
        write_f32_bits(out, self.focused_attention_share);
        write_u64_le(out, self.next_mandate_id);
        write_u64_le(out, self.next_proposal_id);

        write_u32_le(out, self.mandates.len() as u32);
        for m in &self.mandates {
            write_u64_le(out, m.id.0);
            write_u32_le(out, m.issuer.0);
            write_u32_le(out, m.assignee.0);
            write_u32_le(out, m.target_settlement.0);
            write_u32_le(out, m.target_cell.0);
            write_str(out, &m.objective_kind);
            write_str(out, &m.authority_tier);
            write_str(out, &m.summary);
            write_u32_le(out, m.priority_weight);
            write_u8(out, mandate_status_to_u8(m.status));
            write_u64_le(out, m.issued_tick.0);
            write_u8(out, u8::from(m.activated_tick.is_some()));
            write_u64_le(out, m.activated_tick.map(|t| t.0).unwrap_or(0));
            write_u32_le(out, m.subtasks.len() as u32);
            for st in &m.subtasks {
                write_str(out, st);
            }
            write_f32_bits(out, m.realized_efficiency);
            write_f32_bits(out, m.progress_01);
        }

        write_u32_le(out, self.proposals.len() as u32);
        for p in &self.proposals {
            write_u64_le(out, p.proposal_id);
            write_u32_le(out, p.settlement.0);
            write_u32_le(out, p.origin_cell.0);
            write_u32_le(out, p.raised_by.0);
            write_str(out, &p.issue_key);
            write_str(out, &p.title);
            write_str(out, &p.summary);
            write_u64_le(out, p.created_tick.0);
            write_u64_le(out, p.deadline_tick.0);
            write_u32_le(out, p.options.len() as u32);
            for o in &p.options {
                write_str(out, &o.label);
                write_str(out, &o.description);
                write_f32_bits(out, o.attention_cost);
                write_u32_le(out, o.priority_boost as u32);
                write_f32_bits(out, o.welfare_relief_delta);
            }
            write_u8(out, u8::from(p.chosen_option.is_some()));
            write_u32_le(out, p.chosen_option.unwrap_or(0));
        }

        write_u32_le(out, self.notification_rules.len() as u32);
        for (k, &v) in &self.notification_rules {
            write_str(out, k);
            write_u8(out, behavior_to_u8(v));
        }

        write_u32_le(out, self.used_attention_by_owner.len() as u32);
        for (&owner, &val) in &self.used_attention_by_owner {
            write_u32_le(out, owner.0);
            write_f32_bits(out, val);
        }
    }

    fn read_canonical_state(&mut self, input: &mut &[u8]) -> Result<(), String> {
        self.sovereign_id = CharacterId(read_u32_le(input)?);
        self.base_attention_capacity = read_f32_bits(input)?;
        self.clerical_overhead_per_mandate = read_f32_bits(input)?;
        self.hierarchy_hop_latency_hours = read_u32_le(input)?;
        let has_focus = read_u8(input)? != 0;
        let focus = MandateId(read_u64_le(input)?);
        self.focused_mandate = has_focus.then_some(focus);
        self.focused_attention_share = read_f32_bits(input)?;
        self.next_mandate_id = read_u64_le(input)?;
        self.next_proposal_id = read_u64_le(input)?;

        let n_m = read_u32_le(input)? as usize;
        self.mandates.clear();
        for _ in 0..n_m {
            let id = MandateId(read_u64_le(input)?);
            let issuer = CharacterId(read_u32_le(input)?);
            let assignee = CharacterId(read_u32_le(input)?);
            let target_settlement = SettlementId(read_u32_le(input)?);
            let target_cell = CellId(read_u32_le(input)?);
            let objective_kind = read_str(input)?;
            let authority_tier = read_str(input)?;
            let summary = read_str(input)?;
            let priority_weight = read_u32_le(input)?;
            let status = u8_to_mandate_status(read_u8(input)?)?;
            let issued_tick = Tick(read_u64_le(input)?);
            let has_act = read_u8(input)? != 0;
            let act_tick = Tick(read_u64_le(input)?);
            let n_sub = read_u32_le(input)? as usize;
            let mut subtasks = Vec::with_capacity(n_sub);
            for _ in 0..n_sub {
                subtasks.push(read_str(input)?);
            }
            let realized_efficiency = read_f32_bits(input)?;
            let progress_01 = read_f32_bits(input)?;
            self.mandates.push(CommandMandate {
                id,
                issuer,
                assignee,
                target_settlement,
                target_cell,
                objective_kind,
                authority_tier,
                summary,
                priority_weight,
                status,
                issued_tick,
                activated_tick: has_act.then_some(act_tick),
                subtasks,
                realized_efficiency,
                progress_01,
            });
        }

        let n_p = read_u32_le(input)? as usize;
        self.proposals.clear();
        for _ in 0..n_p {
            let proposal_id = read_u64_le(input)?;
            let settlement = SettlementId(read_u32_le(input)?);
            let origin_cell = CellId(read_u32_le(input)?);
            let raised_by = CharacterId(read_u32_le(input)?);
            let issue_key = read_str(input)?;
            let title = read_str(input)?;
            let summary = read_str(input)?;
            let created_tick = Tick(read_u64_le(input)?);
            let deadline_tick = Tick(read_u64_le(input)?);
            let n_opt = read_u32_le(input)? as usize;
            let mut options = Vec::with_capacity(n_opt);
            for _ in 0..n_opt {
                options.push(ArbitrationOption {
                    label: read_str(input)?,
                    description: read_str(input)?,
                    attention_cost: read_f32_bits(input)?,
                    priority_boost: read_u32_le(input)? as i32,
                    welfare_relief_delta: read_f32_bits(input)?,
                });
            }
            let has_choice = read_u8(input)? != 0;
            let choice = read_u32_le(input)?;
            self.proposals.push(ArbitrationProposal {
                proposal_id,
                settlement,
                origin_cell,
                raised_by,
                issue_key,
                title,
                summary,
                created_tick,
                deadline_tick,
                options,
                chosen_option: has_choice.then_some(choice),
            });
        }

        let n_r = read_u32_le(input)? as usize;
        self.notification_rules.clear();
        for _ in 0..n_r {
            let k = read_str(input)?;
            let v = u8_to_behavior(read_u8(input)?)?;
            self.notification_rules.insert(k, v);
        }

        let n_u = read_u32_le(input)? as usize;
        self.used_attention_by_owner.clear();
        for _ in 0..n_u {
            let owner = CharacterId(read_u32_le(input)?);
            let val = read_f32_bits(input)?;
            self.used_attention_by_owner.insert(owner, val);
        }

        self.pending_order_couriers.clear();
        self.pending_world_actions.clear();
        self.applied_resolutions.clear();
        self.rebuild_view_cache();
        Ok(())
    }
}
