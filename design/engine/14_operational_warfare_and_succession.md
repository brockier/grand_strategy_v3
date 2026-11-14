# Module Design: Operational Warfare & Generational Succession (`M7`)

**Module ID:** `ENG-PLUG-14`  
**Code Location:** `crates/sim-plug-warfare/` and `crates/sim-plug-succession/`  
**Milestone & Design Depth:** `M7: Locked Spine Port Contracts & Replaceable Baseline Design`  
**Satisfies Requirements:** `ENG-WAR-001`, `ENG-WAR-002`, `ENG-WAR-003`, `ENG-WAR-004`, `ENG-POP-006` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

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
  - Implements `OperationalCombatPlugPort` and `SuccessionPlugPort` defined in `crates/sim-spine/`
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

## 3. Replaceable Baseline Behavioral Design (`M7` Scope)

When decomposed into `.1`/`.2` `Active` requirements at `M7`, these two plug cartridges govern end-game military and dynastic realism:

1. **Command Delegation Spectrum ("Churchill vs. Hitler", `ENG-WAR-001`, `ENG-WAR-002`, `ENG-WAR-003`):**
   - *Churchill Mode (`TheaterDirective`):* Low Personal Attention cost; field generals inspect `SurfaceConditionsPort` and enemy contact locally with zero HQ lag, shifting crossings or executing fighting withdrawals autonomously according to their competence.
   - *Hitler Mode (`OperationalBattlePlan`):* High Personal Attention cost; enforces player-chosen division objectives and `"No Retreat"` posture via `CourierEnvelope`. When HQ sits hundreds of kilometers behind the front and storm weather delays wires/runners, divisions execute stale yesterday orders into mud or encirclement unless bold commanders trigger insubordination checks.
   - Relocating physical HQ near the front drops courier lag to armies while expanding lag back to core production ministries (`ENG-EPI-003`).
2. **Weather-Coupled Hourly Combat Physics (`ENG-WAR-004`):**
   - Queries `SurfaceConditionsPort` hourly during `TickPhase::HourlyCombatStep`: mud mires heavy artillery and supply wagons; flood surges wash out fords/pontoons and split flanking corps; heavy precipitation degrades exposed black powder and bowstrings; fog/blizzards cut reconnaissance radius and slow battlefield runners.
3. **Mortal Generational Succession (`ENG-POP-006`):**
   - Tracks player character aging, battlefield proximity hazard, illness, and political deposition.
   - Upon death or ouster, invokes externalized `SuccessionRuleId` (dynastic heir, council election, military coup, chartered board appointment) transferring control strictly over `player-view`'s existing `PlayerKnowledgeArchive`, physical ledgers, standing bureaucracies, debts, and entrenched officials—discarding unarchived head knowledge cleanly.

---

## 4. Isolated Cartridge Verification Strategy

1. Prior to `M7`, neutral baseline adapters do nothing (`Ok(())` / `None`).
2. At `M7`, IPC fuzz tests verify tactical unit movement overrides fail validation (`ENG-WAR-001`), storm-front combat simulations quantify mud/ford-washout effects (`ENG-WAR-004`), distant HQ `"No Retreat"` runs reproduce stale-order encirclement risks (`ENG-WAR-003`), and leader mortality tests prove unarchived secret knowledge does not survive succession (`ENG-POP-006`).

---

## 5. Lift-and-Shift Replacement Guide

1. Operational combat resolution (for instance, swapping Lanchester attrition equations for supply-weighted posture clashes) or succession politics can be completely rewritten inside `crates/sim-plug-warfare/` and `crates/sim-plug-succession/`.
2. Zero core economy, weather, or UI shell contracts change as long as `OperationalCombatPlugPort` and `SuccessionPlugPort` remain satisfied.
