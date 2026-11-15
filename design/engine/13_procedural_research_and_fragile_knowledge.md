# Module Design: Non-Bought Research & Fragile Knowledge (`M6`)

**Module ID:** `ENG-PLUG-13`  
**Code Location:** `code/engine/crates/sim-plug-research/`  
**Milestone & Design Depth:** `M6: Full Detailed Design`  
**Satisfies Requirements:** `ENG-RES-001.1`..`ENG-RES-001.2`, `ENG-RES-002.1`..`ENG-RES-002.2`, `ENG-RES-003.1`..`ENG-RES-003.2`, `ENG-RES-004.1`..`ENG-RES-004.2`, `ENG-RES-005.1`..`ENG-RES-005.2` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `ModRegistryPort` (procedural material parameters, component primitives, theoretical domain rules)
  - `PhysicalLedgerPort` (physical prototypes, precision tooling, written archive items, laboratory consumables)
  - `PopulationPort` (surplus-leisure cohorts, practicing vocational cohorts, Named Individual cognitive/curiosity traits)
  - `LogisticsPort` (traveling engineers, smuggled manuals, industrial espionage dispatches)
- **Provided Spine Ports (Outputs):**
  - Implements `KnowledgeModifierPlugPort` defined in `code/engine/crates/sim-spine/`
  - Emits empirical workplace field-note events and patent/discovery dispatches to `player-view`

---

## 2. Locked Spine Port Contract (Wired from `M0` Onward)

```rust
pub trait KnowledgeModifierPlugPort: Send + Sync {
    fn is_process_constructible(&self, settlement: SettlementId, process: ProcessId) -> bool;
    fn process_yield_and_waste_modifier(&self, settlement: SettlementId, process: ProcessId) -> KnowledgeProcessModifier;
    fn on_workplace_bottleneck_observed(&mut self, event: WorkplaceBottleneckEvent);
    fn check_patent_restriction(&self, actor: ActorId, component: TechComponentId) -> PatentGateStatus;
    fn step_weekly_inquiry_and_decay(&mut self, ctx: &mut PlugKnowledgeContext);
}
```

**Design Invariant (`ENG-RES-001`):** `sim-spine` contains **no** `ResearchPoints` currency type, **no** global `TechTreeUnlockSet`, and **no** IPC command capable of purchasing technologies.

---

## 3. Authoritative Behavioral Design (`M6` Scope)

Implemented inside `code/engine/crates/sim-plug-research/` with zero global science points, deterministic `BTreeMap` storage, and full `CanonicalSaveablePort` round-trips:

1. **Two-Track Representation (`ENG-RES-002.1`..`002.2`, `ENG-RES-003.1`..`003.2`):**
   - **Know-How (`TechComponentId`):** Modular working mechanisms and processes (weighted lever hoists, forced-draft tuyeres, check valves, bored pressure cylinders, refractory linings, sprung axles). Active workplace bottlenecks (high mine shaft water, brittle high-slag smelting, muddy axle breaks) advance local trials using real practitioner hours and material batches. Once any prototype exists physically, cross-pollination lowers empirical trial cost by at least `35%` for adjacent components in that domain.
   - **Know-Why (`TheoryDomainId`):** Abstract causal treatises synthesized by leisure scholars/specialists studying assayed procedural materials and functioning prototypes (strictly stalled during acute caloric deficit). Once domain mastery crosses `0.50`, directed R&D speed doubles (`>= 2.0x`) and trial material waste drops by `>= 40%`.
2. **Four Institutional Modes (`ENG-RES-004.1`..`004.2`):**
   - *Workplace Tinkering* -> *State Prizes & War Arsenals* -> *Patent Monopoly* (time-limited exclusivity enforced via `patent_duration_years`, blocking third-party syndicate adoption until expiry or leak) -> *Universities & Research Laboratories* (requiring literacy threshold + capital surplus).
3. **Four-Carrier Fragility & Regression (`ENG-RES-005.1`..`005.2`):**
   Every settlement capability tracks counts across `(practicing_cohorts, living_specialists, workshop_prototypes, written_archives)`. Destroying prototypes alongside cohort/specialist extinction with zero surviving archive copies reverts `process_prerequisites_met` to `false`.

---

## 4. Isolated Cartridge Verification Strategy

1. Prior to `M6`, `BaselineM4KnowledgePlug` permits scenario-configured survival recipes directly via `ModRegistryPort`.
2. At `M6`, isolated tests feed synthetic flooded-shaft bottleneck events, verify zero abstract science points exist, confirm steam-pump creation lowers locomotive component discovery friction, test patent legal blockades on rival actors, and verify siege/plague carrier destruction induces genuine technological loss.

---

## 5. Lift-and-Shift Replacement Guide

1. You can swap the discovery probability engine (for instance, replacing Bayesian experimentation with combinatorial chemical/mechanical synthesis graphs) entirely inside `code/engine/crates/sim-plug-research/`.
2. Zero sibling simulation crates change as long as `KnowledgeModifierPlugPort` semantics remain intact.
