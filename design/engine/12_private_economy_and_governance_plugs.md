# Module Design: Non-State Economy, Multi-Medium Accounting & Governance Plugs (`M5`)

**Module ID:** `ENG-PLUG-12`  
**Code Location:** `code/engine/crates/sim-plug-private-eco/`  
**Milestone & Design Depth:** `M5: Locked Spine Port Contracts & Replaceable Baseline Design`  
**Satisfies Requirements:** `ENG-ECO-005`, `ENG-ECO-006`, `ENG-ECO-007`, `ENG-LOG-005`, `ENG-LOG-006`, `ENG-CMD-006`, `ENG-CMD-007` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `ModRegistryPort` (governance models, economic policy laws, institutional rules)
  - `PhysicalLedgerPort` (actor inventories, facility ownership, regional scarcity ratios)
  - `LogisticsPort` (carrier fleet utilization, edge axle load counts)
  - `PopulationPort` (household cohorts, noble/clan/merchant Named Individuals)
- **Provided Spine Ports (Outputs):**
  - Implements `NonStateEconomyPlugPort` and `GovernanceFrictionPlugPort` defined in `code/engine/crates/sim-spine/`

---

## 2. Locked Spine Port Contracts (Wired from `M0` Onward)

Because these traits live in `code/engine/crates/sim-spine/`, `sim-economy`, `sim-logistics`, and `sim-command` already call them from `M3`/`M4` onward (via a neutral no-op `BaselineM4Plug` until `M5` activates):

```rust
pub trait NonStateEconomyPlugPort: Send + Sync {
    fn edge_congestion_multiplier(&self, edge: EdgeId, active_axle_load: f32) -> f32;
    fn regional_charter_rate(&self, settlement: SettlementId, carrier: CarrierClassId) -> CharterQuote;
    fn step_daily_non_state_actors(&mut self, ctx: &mut PlugEconomyContext);
    fn apply_policy_lever(&mut self, policy: EconomicPolicyCommand);
}

pub trait GovernanceFrictionPlugPort: Send + Sync {
    fn clerical_capacity_bonus(&self, holder: CharacterId) -> f32;
    fn hop_signal_distortion(&self, sender: CharacterId, recipient: CharacterId, payload: &mut ReportValues);
    fn goodhart_quota_quality_penalty(&self, mandate: MandateId, inspection_freq_01: f32) -> f32;
    fn clerical_overhead_consumption(&self, settlement: SettlementId) -> Vec<CohortRationDrain>;
    fn institutional_reform_resistance(&self, model: GovernanceModelId, proposed_change: PolicyLawId) -> f32;
}
```

---

## 3. Replaceable Baseline Behavioral Design (`M5` Scope)

When decomposed into `.1`/`.2` `Active` requirements at `M5`, the baseline cartridge implements:

1. **Four Concrete Non-State Actor Engines (`ENG-ECO-005`):**
   - *Households & Smallholders:* Hold personal reserves first; route marginal surplus to local spot exchange.
   - *Landed Estates & Clans:* Extract in-kind grain/corvée shares on arable tracts; hoard local grain buffers during shortages and resist outside assessors.
   - *Merchant Houses & Hauling Guilds:* Maintain private messenger sightings; detect spatial price differentials across settlements and dispatch private `TransitEntity` caravans using `LogisticsPort`.
   - *Syndicates, Craft Guilds & Chartered Companies:* Pool capital to submit concession bids or open deep mines, smelters, canals, and railways autonomously when expected return exceeds perceived physical/political risk.
2. **Multi-Medium Clearing (`ENG-ECO-006`):** Single unified ledger balance abstraction supporting pure physical barter ratios, in-kind tribute/corvée obligations, stamped metallic coinage, merchant promissory credit, and state command quotas.
3. **Cross-Era Policy Levers (`ENG-ECO-007`):** Tribute/corvée, market purchase contracts/subsidies/tariffs, chartered concessions/monopolies, and direct state enterprises.
4. **Shared Congestion & Freight Crowding-Out (`ENG-LOG-005`, `ENG-LOG-006`):** Combines state Quartermaster axle load and private caravan axle load per `EdgeId` to emit non-linear congestion multipliers and raise freight charter rates whenever state requisitioning shrinks civilian carrier supply.
5. **Four Bureaucratic Frictions & Five Governance Trade-Offs (`ENG-CMD-006`, `ENG-CMD-007`):** Implements Direct Personal Rule, Devolved Fiefs/Tax Farming, Standardized State Bureaucracy, Guilds/Chartered Monopolies, and Open Markets/Contracts/Patents, ensuring every reform shifts friction profiles rather than removing friction.

---

## 4. Isolated Cartridge Verification Strategy

1. Prior to `M5`, `sim-spine` tests assert `BaselineM4Plug` returns `1.0` congestion and zero non-state arbitrage so `M3`–`M4` behavior stays crisp and predictable.
2. At `M5`, pluggable test suites bind mock `PhysicalLedgerPort` and `LogisticsPort` fixtures to verify price-arbitrage caravans, military wagon impressment starving civilian freight, Goodhart brittle-ingot gaming under uninspected quotas, and distinct friction trade-offs across all five governance models.

---

## 5. Lift-and-Shift Replacement Guide

1. You can swap a pure microeconomic general-equilibrium solver for an agent-based merchant heuristic or completely redesign how feudal estates collect rents simply by substituting the crate implementing `NonStateEconomyPlugPort` and `GovernanceFrictionPlugPort`.
2. Zero modifications to `sim-economy`, `sim-logistics`, `sim-command`, or `requirements/` are required because all interactions cross the locked Spine plug traits above.
