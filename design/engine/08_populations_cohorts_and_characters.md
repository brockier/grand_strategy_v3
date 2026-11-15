# Module Design: Two-Tier Populations, Cohorts & Named Individuals

**Module ID:** `ENG-POP-08`  
**Code Location:** `code/engine/crates/sim-pop/`  
**Milestone & Design Depth:** `M3–M4: Full Detailed Design`  
**Satisfies Requirements:** `ENG-POP-001`, `ENG-POP-002`, `ENG-POP-003`, `ENG-POP-004`, `ENG-POP-005` in [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Module Boundary Header

- **Consumed Spine Ports (Inputs):**
  - `ModRegistryPort` (vocations, age brackets, demographic curves, trait tables)
  - `PhysicalLedgerPort` (household reserves of food, fuel, clothing, personal tools)
  - `SurfaceConditionsPort` (ambient cold exposure, epidemic weather factors)
  - `SuccessionPlugPort` (`M7` hook for mortal player death/succession trigger)
- **Provided Spine Ports (Outputs):**
  - `PopulationPort` (`settlement_cohorts`, `cohort_view`, `character_view`, `apply_labor_and_welfare`)
  - `UnrestDomainEvent`s (`Slowdown`, `StrikeStarted`, `OutMigration`, `Banditry`, `Revolt`)

---

## 2. Encapsulated Internal State & Data Layout

### 2.1 Tier 1: Demographic Cohort Struct (`O(cohorts)` Constant-Time Scaling)

```rust
pub struct DemographicCohort {
    pub id: CohortId,
    pub settlement_id: SettlementId,
    pub owner_actor: ActorId,
    pub primary_vocation: VocationId,
    // Dimension 1: Age Structure (exact integer headcounts conserved across transitions)
    pub dependents_count: u32,
    pub prime_workers_count: u32,
    pub elders_count: u32,
    // Dimension 2: Education & Literacy
    pub literacy_01: f32,
    // Dimension 3: Practical Domain Experience (0.0..=1.0 per active VocationId)
    pub vocation_experience: BTreeMap<VocationId, f32>,
    // Dimension 4: Health, Nutrition & Stamina
    pub nutrition_01: f32,
    pub stamina_01: f32,
    pub disease_burden_01: f32,
    // Dimension 5: Material Wealth & Reserves (stored physically in household_stockpile)
    pub household_stockpile: StockpileId,
    pub shelter_quality_01: f32,
    // Dimension 6: Cohesion & Political Disposition
    pub trust_in_leadership_01: f32,
    pub grievance_01: f32,
    pub solidarity_01: f32,
    pub unrest_state: CohortUnrestState,
}
```

### 2.2 Tier 2: Named Individual Struct

```rust
pub struct NamedIndividual {
    pub id: CharacterId,
    pub name: String,
    pub origin_cohort: Option<CohortId>,
    pub current_cell: CellId,
    pub age_years: f32,
    pub alive: bool,
    pub health_01: f32,
    pub portrait_tags: PortraitTagSet,
    // Four Multi-Axis Trait Vectors
    pub competence: CompetenceTraits,    // engineering, logistics, fieldcraft_survey, theory, military_cmd
    pub cognitive: CognitiveTraits,      // curiosity, methodicalness, unconventionality
    pub epistemic: EpistemicTraits,      // honesty_vs_sycophancy (-1..1), precision_01, caution_vs_alarmism (-1..1)
    pub drives: PersonalDriveTraits,     // loyalty_01, ambition_01, greed_corruption_01, peer_affinities
}
```

---

## 3. Algorithms, Vital Transitions & Unrest Dynamics

### 3.1 Weekly Sharded Cohort Vital Cycle (`ENG-POP-002`)
Executed during `TickPhase::WeeklyMonthlyBucketDemographicsMandatesResearch` (`cohort_id % 168 == tick % 168`):
1. **Aging & Vital Headcount Conservation:** Advance fractional age transition accumulators (`dependents -> prime_workers -> elders`), compute deterministic rounded births (scaled by `nutrition_01`, `disease_burden_01`, and `prime_workers_count`), and apply age-bucket mortality (`elders` highest, spiked by starvation, freezing without fuel/shelter, or pathogens). Assert `pop_after == pop_before + births - deaths`.
2. **Experience Accrual & Generational Skill Extinction (`ENG-POP-003`):**
   - Active logged work hours increment `vocation_experience[v]` asymptotically toward `1.0`, boosted by elder master ratio (`apprenticeship_bonus = elders_count / max(1, prime_workers_count)`).
   - Zero work hours during the weekly window decay experience via `idle_decay_rate`.
   - When `elder_deaths` remove un-apprenticed specialists faster than apprentices replace them, dilute cohort experience proportionally to `(surviving_masters / prior_masters)`—causing authentic generational skill loss if elder masters die during famine or plague!
3. **Health, Nutrition & Stamina (`ENG-POP-005`):** Compute weekly caloric intake ratio, heating fuel adequacy vs. `SurfaceConditionsPort::temp_c`, and fatigue recovery.

### 3.2 Social Cohesion & Unrest State Machine (`ENG-POP-005`)
```text
Compliant --(grievance > 0.35)--> Slowdown --(grievance > 0.55)--> Strike / OutMigration
                                                                        |
                                                                (grievance > 0.80 & low trust)
                                                                        v
                                                                 Banditry / Revolt
```
- `Slowdown` cuts effective labor efficiency proportionally (`1.0 - 0.5 * slowdown_severity`).
- `Strike` suspends facility labor output until grievances fall or concessions/coercion resolve it.
- `OutMigration` spawns a `PassengerCohorts` `TransitEntity` via `LogisticsPort` migrating families toward better-provisioned settlements.

### 3.3 Deterministic Promotion of Named Individuals (`ENG-POP-004`)
When a role or leadership vacuum requests a new specialist:
1. Sample candidate priors from the target cohort's `literacy_01` and `vocation_experience` (`high-literacy` boosts `precision_01` and `methodicalness`).
2. Roll `epistemic` and `drives` traits from external trait probability distributions using `PhaseRng` so each promoted overseer carries authentic virtues and flaws (`honesty_vs_sycophancy` ranging from candid whistleblower to self-serving flatterer).

---

## 4. Isolated Cartridge Verification Strategy

1. **Constant-Time Population Scaling (`@req ENG-POP-001`):** Micro-benchmark compares step runtime between `200`-person and `2,000,000`-person cohorts confirming identical `O(cohorts)` CPU time.
2. **Demographic Mass Conservation (`@req ENG-POP-002`):** Runs 20-year simulations (`175,200` ticks) asserting exact integer headcount accounting across all transitions.
3. **Apprenticeship & Elder Extinction Decay (`@req ENG-POP-003`):** Kills elder smelter cohorts or halts smelting for 5 years in a test fixture and measures drop in `vocation_experience` and downstream output purity.
4. **Character Promotion Bounds (`@req ENG-POP-004`):** Verifies promoted leaders inherit cohort literacy priors and bounded trait ranges.
5. **Productivity & Unrest Response (`@req ENG-POP-005`):** Tests cold/starvation shocks and verifies progressive transition into `Slowdown`, `Strike`, and `OutMigration`.

---

## 5. Lift-and-Shift Replacement Guide

1. To change demographic granularity (for example, adding single-year age pyramids or cultural/linguistic identity splits inside cohorts), keep `PopulationPort` and `CohortId`/`CharacterId` signatures unchanged inside `code/engine/crates/sim-pop/`.
2. Zero changes to `sim-economy`, `sim-logistics`, or `requirements/` are required.
