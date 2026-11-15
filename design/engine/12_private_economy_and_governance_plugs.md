# Module Design: Non-State Economy, Multi-Medium Accounting & Governance Plugs (`M5`)

**Module ID:** `ENG-PLUG-12`  
**Code Location:** `code/engine/crates/sim-plug-private-eco/`  
**Milestone & Design Depth:** `M5: Full Detailed Design`  
**Satisfies Requirements:** `ENG-ECO-005.1`..`ENG-ECO-005.4`, `ENG-ECO-006.1`..`ENG-ECO-006.2`, `ENG-ECO-007.1`..`ENG-ECO-007.2`, `ENG-LOG-005.1`..`ENG-LOG-005.2`, `ENG-LOG-006.1`..`ENG-LOG-006.2`, `ENG-CMD-006.1`..`ENG-CMD-006.4`, `ENG-CMD-007.1`..`ENG-CMD-007.2` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

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

## 3. Authoritative Behavioral Design (`M5` Scope)

Implemented inside `code/engine/crates/sim-plug-private-eco/` with `#![deny(clippy::disallowed_types)]`, deterministic `BTreeMap` arenas, and `CanonicalSaveablePort` state serialization:

1. **Four Concrete Non-State Actor Engines (`ENG-ECO-005.1`..`005.4`):**
   - *Households & Smallholders:* Hold subsistence caloric, fuel, and tool reserves first; route marginal surplus to local spot exchange.
   - *Landed Estates & Clans:* Extract in-kind grain/corvée shares on arable tracts; hoard local grain buffers during shortages and under-report taxable reserves until audited.
   - *Merchant Houses & Hauling Guilds:* Maintain lagged regional market price estimates; detect spatial price spreads exceeding haul cost and dispatch private arbitrage caravans via `LogisticsPort`.
   - *Syndicates, Craft Guilds & Chartered Companies:* Pool capital to submit concession bids or finance autonomous deep mines, smelters, and transport buildouts when expected margin exceeds physical/political risk.
2. **Multi-Medium Clearing (`ENG-ECO-006.1`..`006.2`):** Single unified accounting engine clearing bilateral physical barter ratios, in-kind tribute/corvée obligations, metallic coinage, merchant promissory credit, and state command quotas without requiring currency from tick `0`.
3. **Cross-Era Policy Levers (`ENG-ECO-007.1`..`007.2`):** `tribute_corvee`, `market_purchase_orders` (mobilizing private production at zero Attention cost with profiteering elasticity), `chartered_concession` (trading capital savings for syndicate oligarch share), and `direct_state_enterprise`.
4. **Shared Congestion & Freight Crowding-Out (`ENG-LOG-005.1`..`005.2`, `ENG-LOG-006.1`..`006.2`):** Aggregates state Quartermaster axle load and private caravan tonnage per corridor (`edge_congestion_multiplier <= 1.0`), accumulates corridor wear (`wear_01`), scales regional carrier charter rates non-linearly as idle private wagons shrink, and throttles civilian staple freight when the state impresses/charters carriers.
5. **Four Bureaucratic Frictions & Five Governance Trade-Offs (`ENG-CMD-006.1`..`006.4`, `ENG-CMD-007.1`..`007.2`):** Tracks hop latency/filtering (`hop_count * hierarchy_hop_latency_hours`), Goodhart uninspected tonnage purity dilution (`inspect/audit` resets), clerical grain/literate-labor overhead, reform inertia transition countdowns, and all 5 externalized `GovernanceModelRecord` profiles (`patrimonial_rule`, `devolved_fiefs`, `state_bureaucracy`, `chartered_guilds`, `open_markets_patents`).

---

## 4. Isolated Cartridge Verification Strategy

1. Prior to `M5`, `sim-spine` tests assert `BaselineM4Plug` returns `1.0` congestion and zero non-state arbitrage so `M3`–`M4` behavior stays crisp and predictable.
2. At `M5`, pluggable test suites bind mock `PhysicalLedgerPort` and `LogisticsPort` fixtures to verify price-arbitrage caravans, military wagon impressment starving civilian freight, Goodhart brittle-ingot gaming under uninspected quotas, and distinct friction trade-offs across all five governance models.

---

## 5. Lift-and-Shift Replacement Guide

1. You can swap a pure microeconomic general-equilibrium solver for an agent-based merchant heuristic or completely redesign how feudal estates collect rents simply by substituting the crate implementing `NonStateEconomyPlugPort` and `GovernanceFrictionPlugPort`.
2. Zero modifications to `sim-economy`, `sim-logistics`, `sim-command`, or `requirements/` are required because all interactions cross the locked Spine plug traits above.
