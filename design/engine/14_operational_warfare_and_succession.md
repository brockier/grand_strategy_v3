# Module Design: Operational Warfare & Generational Succession (`M7`)

**Module ID:** `ENG-PLUG-14`  
**Code Location:** `code/engine/crates/sim-plug-warfare/` and `code/engine/crates/sim-plug-succession/`  
**Milestone & Design Depth:** `M7: Full Detailed Design`  
**Satisfies Requirements:** `ENG-WAR-001.1`, `ENG-WAR-002.1`..`ENG-WAR-002.2`, `ENG-WAR-003.1`..`ENG-WAR-003.2`, `ENG-WAR-004.1`..`ENG-WAR-004.2`, `ENG-POP-006.1`..`ENG-POP-006.2` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `PlanetGeometryPort` (terrain elevation, river crossings, edge slopes)
  - `SurfaceConditionsPort` (hourly `mud_index_01`, `flood_stage_01`, `precip_mm_hr`, `visibility_01`, `ice_thickness_m`)
  - `PhysicalLedgerPort` (ammunition, rations, fodder, weapons, artillery carriage condition)
  - `LogisticsPort` (supply convoys, wounded evacuation, courier dispatch latency between HQ and front)
  - `PopulationPort` (recruited combat cohorts, commander traits, player character aging/mortality)
  - `CommandPort` (Attention budget consumed by military delegation vs. domestic ministries)
- **Provided Spine Ports (Outputs):**
  - Implements `OperationalCombatPlugPort` and `SuccessionPlugPort` defined in `code/engine/crates/sim-spine/`
  - Emits delayed combat reports, encirclement alerts, and succession transition events into `player-view`

---

## 2. Locked Spine Port Contracts (Wired from `M0` Onward)

```rust
pub enum MilitaryCommandOrder {
    // Low Personal Attention: assigns strategic theater goals, commander, and logistics tonnage
    TheaterDirective { theater: TheaterId, commander: CharacterId, posture: StrategicPosture, supply_priority: u8 },
    // High Personal Attention: dictates division objectives, phase lines, and rigid non-retreat doctrines
    OperationalBattlePlan { formation: FormationId, target_cells: Vec<CellId>, no_retreat: bool, phase_tick: Tick },
}

pub trait OperationalCombatPlugPort: Send + Sync {
    // Deliberately rejects single-tick hex micromanagement commands (ENG-WAR-001)
    fn enqueue_military_order(&mut self, arrived_order: MilitaryCommandOrder) -> Result<(), WarfareCommandError>;
    fn step_hourly_combat(&mut self, ctx: &mut PlugCombatContext);
}

pub trait SuccessionPlugPort: Send + Sync {
    fn evaluate_leader_mortality_and_succession(&mut self, ctx: &mut PlugSuccessionContext) -> Option<SuccessionTransition>;
}
```

---

## 3. Authoritative Behavioral Design (`M7` Scope)

Implemented across `code/engine/crates/sim-plug-warfare/` and `code/engine/crates/sim-plug-succession/` on continuous spherical terrain coordinates (`unit_xyz: [f32; 3]`):

1. **Command Delegation Spectrum ("Churchill vs. Hitler", `ENG-WAR-001.1`, `ENG-WAR-002.1`..`002.2`, `ENG-WAR-003.1`..`003.2`):**
   - *Strict Micro Ban:* Direct unit step-by-step tactical overrides fail command validation immediately with `ERR_TACTICAL_MICRO_FORBIDDEN`.
   - *Churchill Mode (`TheaterDirective`):* Low Personal Attention cost; field generals read local `SurfaceConditionsPort` and enemy contact continuously without HQ lag, autonomously sidestepping flooded crossings, muddy defiles, or enveloping flanks.
   - *Hitler Mode (`OperationalBattlePlan`):* High Personal Attention cost; prescribes explicit route coordinates, phase lines, and optional `no_retreat: bool`. When distant HQ courier lag delays relief against flank threats, obedient commanders hold into encirclement casualties whereas high-initiative/low-obedience officers break orders (`insubordination`) to save their corps and dispatch delayed explanations.
2. **Weather-Coupled Hourly Combat Physics (`ENG-WAR-004.1`..`004.2`):**
   - Every hourly combat tick couples `mud_index_01` (bogging heavy artillery/supply), `flood_stage_01` (blocking unbridged river crossings and splitting wings), `precip_mm_hr` (fouling propellants/bowstrings), and `visibility_01` (degrading recon and runner speed).
3. **Mortal Generational Succession (`ENG-POP-006.1`..`006.2`):**
   - Ages the mortal player ruler and monitors health/hazard triggers under external `SuccessionRuleRecord` definitions. Upon demise, pauses via `leader_succession`, selects the heir/appointee/magistrate, preserves 100% of `PlayerKnowledgeArchive` physical maps, ledgers, institutions, and debts, purges unrecorded personal head observations, and applies `transition_disruption_hours`.

---

## 4. Isolated Cartridge Verification Strategy

1. Prior to `M7`, neutral baseline adapters do nothing (`Ok(())` / `None`).
2. At `M7`, IPC fuzz tests verify tactical unit movement overrides fail validation (`ENG-WAR-001`), storm-front combat simulations quantify mud/ford-washout effects (`ENG-WAR-004`), distant HQ `"No Retreat"` runs reproduce stale-order encirclement risks (`ENG-WAR-003`), and leader mortality tests prove unarchived secret knowledge does not survive succession (`ENG-POP-006`).

---

## 5. Lift-and-Shift Replacement Guide

1. Operational combat resolution (for instance, swapping Lanchester attrition equations for supply-weighted posture clashes) or succession politics can be completely rewritten inside `code/engine/crates/sim-plug-warfare/` and `code/engine/crates/sim-plug-succession/`.
2. Zero core economy, weather, or UI shell contracts change as long as `OperationalCombatPlugPort` and `SuccessionPlugPort` remain satisfied.
