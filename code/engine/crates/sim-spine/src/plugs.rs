//! Pre-wired M5-M7 extension plug traits with neutral M0-M4 baseline adapters.

use crate::ids::{CellId, CharacterId, EdgeId, ProcessId, SettlementId, Tick};

/// Pre-wired M5 non-state economy, private actor clearing, and shared road congestion plug.
pub trait NonStateEconomyPlugPort: Send + Sync {
    fn edge_congestion_multiplier(&self, _edge: EdgeId) -> f32 {
        1.0
    }

    fn step_private_actors(&mut self, _tick: Tick) {}
}

/// Pre-wired M5 hierarchical bureaucratic friction and Goodhart distortion plug.
pub trait GovernanceFrictionPlugPort: Send + Sync {
    fn oversight_efficiency_multiplier(&self, _settlement: SettlementId, _overseer: CharacterId) -> f32 {
        1.0
    }
}

/// Pre-wired M6 empirical/theoretical four-carrier knowledge prerequisite plug.
pub trait KnowledgeModifierPlugPort: Send + Sync {
    fn process_prerequisites_met(&self, _settlement: SettlementId, _process: ProcessId) -> bool {
        true
    }
}

/// Pre-wired M7 operational warfare plug ("Churchill" directive vs. "Hitler" plan spectrum).
pub trait OperationalCombatPlugPort: Send + Sync {
    fn step_hourly_combat(&mut self, _tick: Tick) {}
}

/// Pre-wired M7 mortal leader succession and legacy handover plug.
pub trait SuccessionPlugPort: Send + Sync {
    fn check_succession_transition(&mut self, _tick: Tick, _hq_cell: CellId) -> bool {
        false
    }
}

/// Neutral baseline implementation of `NonStateEconomyPlugPort` used in M0-M4.
#[derive(Default, Clone, Copy, Debug)]
pub struct NeutralNonStateEconomyPlug;
impl NonStateEconomyPlugPort for NeutralNonStateEconomyPlug {}

/// Neutral baseline implementation of `GovernanceFrictionPlugPort` used in M0-M4.
#[derive(Default, Clone, Copy, Debug)]
pub struct NeutralGovernanceFrictionPlug;
impl GovernanceFrictionPlugPort for NeutralGovernanceFrictionPlug {}

/// Neutral baseline implementation of `KnowledgeModifierPlugPort` used in M0-M4.
#[derive(Default, Clone, Copy, Debug)]
pub struct NeutralKnowledgeModifierPlug;
impl KnowledgeModifierPlugPort for NeutralKnowledgeModifierPlug {}

/// Neutral baseline implementation of `OperationalCombatPlugPort` used in M0-M4.
#[derive(Default, Clone, Copy, Debug)]
pub struct NeutralOperationalCombatPlug;
impl OperationalCombatPlugPort for NeutralOperationalCombatPlug {}

/// Neutral baseline implementation of `SuccessionPlugPort` used in M0-M4.
#[derive(Default, Clone, Copy, Debug)]
pub struct NeutralSuccessionPlug;
impl SuccessionPlugPort for NeutralSuccessionPlug {}
