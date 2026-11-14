# Module Design: Command Hierarchy, Finite Attention & Cascading Mandates

**Module ID:** `ENG-CMD-10`  
**Code Location:** `crates/sim-command/`  
**Milestone & Design Depth:** `M4: Full Detailed Design`  
**Satisfies Requirements:** `ENG-CMD-001`, `ENG-CMD-002`, `ENG-CMD-003`, `ENG-CMD-004`, `ENG-CMD-005` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `PopulationPort` (`CharacterId` competence, drives, epistemic traits, cohort labor supply)
  - `PhysicalLedgerPort` (observed stockpile deficits and input shortages)
  - `LogisticsPort` (`arrived_envelopes`, `enqueue_courier` for outbound orders and escalated proposals)
  - `GovernanceFrictionPlugPort` (`M5` plug hook for multi-tier bureaucracy frictions and governance model modifiers)
- **Provided Spine Ports (Outputs):**
  - `CommandPort` (`attention_summary`, `active_mandates_for`, `ingest_player_command`)
  - Operational sub-task dispatches to `sim-economy`, `sim-logistics`, and `sim-pop`
  - Escalated `ArbitrationProposal` envelopes sent to superior authorities (player HQ)

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Attention Budget & Direct Supervision State

```rust
pub struct AttentionAccount {
    pub holder: CharacterId,
    pub base_capacity: f32,           // Modulated by org competence and clerical tooling
    pub direct_reports: Vec<CharacterId>,
    pub active_mandates: Vec<MandateId>,
    pub zoom_in_locks: Vec<MandateId>, // High-attention personal supervision slices
    pub current_load: f32,
    pub oversight_dilution_01: f32,   // 0.0 within capacity -> rises convexly above capacity
}
```

### 2.2 Strategic Mandates & Escalated Arbitration Proposals

```rust
pub enum MandateObjective {
    IncreaseCommodityOutput { commodity: CommodityId, target_kg_per_day: f64, priority: u8 },
    ConnectCorridor { from_cell: CellId, to_cell: CellId, target_infra: InfraClassId },
    SurveyRegion { center_cell: CellId, radius_cells: u8, mineral_focus: Option<String> },
    MaintainSettlementSubsistence { settlement: SettlementId },
}

pub struct StrategicMandate {
    pub id: MandateId,
    pub issuer: CharacterId,
    pub assignee: CharacterId,
    pub objective: MandateObjective,
    pub parent_mandate: Option<MandateId>,
    pub active_sub_mandates: Vec<MandateId>,
    pub zoom_supervised: bool,
    pub status: MandateStatus, // InTransitToAssignee | Decomposing | Executing | BlockedEscalated | Complete
}

pub struct ArbitrationProposal {
    pub id: ProposalId,
    pub originating_mandate: MandateId,
    pub author: CharacterId,
    pub bottleneck_kind: BottleneckKind, // ShaftFlooding | RoadCongestion | ToolExhaustion | CharterConflict
    pub options: Vec<ProposalOption>,    // >= 2 distinct resource/policy/R&D options
    pub created_tick: Tick,
    pub resolved_option: Option<u8>,
}
```

---

## 3. Algorithms, Oversight Dilution & Pluggable Mandate Decomposition

### 3.1 Attention Capacity & Non-Linear Oversight Dilution (`ENG-CMD-001`, `ENG-CMD-002`, `ENG-CMD-005`)
1. `capacity = base_attention + k_org * character.competence.logistics + governance_plug.clerical_capacity_bonus(holder)`.
2. `current_load = sum(mandate_weight) + direct_report_overhead + sum(zoom_in_extra_weight)`.
   - Toggling `ZoomInSupervision` onto Mandate A moves `Mandate A` direct supervision dilution to `0.0` while adding high weight (`+1.5` slots) to `holder.current_load`, immediately elevating dilution on unattended Ministries B and C (`ENG-CMD-005`).
3. **Convex Dilution Curve:**
   `overload_ratio = max(0.0, (current_load - capacity) / capacity)`
   `oversight_dilution_01 = 1.0 - 1.0 / (1.0 + overload_ratio.powf(1.8))`
4. Downstream behavioral multipliers derived from `oversight_dilution_01`:
   - Labor efficiency slack penalty (`1.0 - 0.35 * dilution`)
   - Unreported skimming/shrinkage probability scaled by `(dilution * assignee.drives.greed_corruption_01)`
   - Construction schedule slippage and heightened workplace accident risk

### 3.2 Cascading Mandate Decomposition (`ENG-CMD-003`)
Executed via an interchangeable `MandateDecomposer` trait during `Phase 7` (using **strictly** the assignee's own local epistemic knowledge—never omniscient planet deposits!):
1. On arrival of a `CourierEnvelope` carrying `StrategicMandate`, the assignee evaluates gaps against `objective`.
2. Example (`IncreaseCommodityOutput { metal }`):
   - Inspects known deposits: if no viable surveyed ore seam exists, spawns a child `SurveyRegion` mandate (`SurveyExpedition` queued via `LogisticsPort`).
   - If ore exists but fuel or smelter capacity is short, spawns sub-mandates or labor reassignments for timber/charcoal burning, shaft expansion, and ore hauling (`ShipmentRequest`).
   - If the assignee also exceeds their own Attention Capacity, sub-mandates inherit execution delays and goal drift (`assignee` prioritizes low-effort tasks over critical bottlenecks).

### 3.3 Bottleneck Escalation Beyond Delegated Authority (`ENG-CMD-004`)
1. When a task encounters a structural blocker (`flooded mine needing scarce timber or pumping experiment`, `road corridor saturated between grain and ore`, or `budget exhaustion`), the subordinate packages an `ArbitrationProposal` containing 2 to 4 concrete policy/resource options and dispatches it via `LogisticsPort::enqueue_courier` back to the superior (`issuer` or player HQ).
2. Resolving the proposal at HQ emits a new `CourierEnvelope` back to the subordinate implementing the chosen option upon arrival.

---

## 4. Isolated Cartridge Verification Strategy

1. **Finite Attention & Zoom-In Trade-Off (`@req ENG-CMD-001`, `@req ENG-CMD-005`):** Assigns 6 mandates to a capacity-5 player; toggles `ZoomInSupervision` on Mandate 1 and asserts Mandate 1 dilution equals `0.0` while Mandates 2–6 experience increased overload dilution.
2. **Non-Linear Dilution Scaling (`@req ENG-CMD-002`):** Sweeps direct reports from `4` to `12` in controlled tests and verifies monotonic super-linear drop in delivered output per worker-hour plus elevated skimming loss.
3. **Autonomous Decomposition (`@req ENG-CMD-003`):** Issues a metal-output mandate without building placements; asserts automatic creation of geological survey expeditions and fuel/smelter sub-tasks using strictly local known surveys.
4. **Proposal Escalation (`@req ENG-CMD-004`):** Triggers deep mine flooding on an active shaft and asserts delivery of an `ArbitrationProposal` envelope with at least two selectable resolution choices.

---

## 5. Lift-and-Shift Replacement Guide

1. The subordinate planner (`MandateDecomposer`) sits cleanly behind a trait inside `crates/sim-command/`, allowing full replacement (for example, swapping utility-scoring AI for GOAP or HTN planners) without altering `sim-economy`, `sim-logistics`, or `player-view`.
2. Any replacement command module MUST preserve `CommandPort`, respect courier travel latency via `LogisticsPort`, and never peek at `SubsurfaceDepositTruthPort` directly during subordinate planning.
