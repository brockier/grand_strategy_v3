# Engine & Epistemic Simulation Requirements Specification

**Document ID:** `ENG-REQ-SPEC`  
**Scope:** Rust Simulation Workspace (`sim-data`, `sim-core`, `player-view`, `sim-cli`) and Asset Provenance Auditing  
**Companion Architecture:** [architecture.md](file:///home/roger/Projects/ng/requirements/architecture.md)  
**Companion UI Specification:** [ui_requirements.md](file:///home/roger/Projects/ng/requirements/ui_requirements.md)  

---

## 1. Traceability, Lifecycle & Automated Test Binding Specification

### 1.1 Requirement Identifier Grammar

1. Top-level engine requirements use the immutable identifier format `ENG-<DOMAIN>-<NNN>` where `<DOMAIN>` is a three-letter uppercase subsystem code (`LIC`, `MOD`, `CLK`, `GEO`, `WTH`, `EPI`, `CMD`, `POP`, `ECO`, `LOG`, `RES`, `WAR`, `SCN`) and `<NNN>` is a zero-padded three-digit integer starting at `001`.
2. When an existing requirement is decomposed into finer testable behaviors, child requirements append dot-delimited sequence numbers (`ENG-<DOMAIN>-<NNN>.<M>`, supporting multi-level suffixes including `.1`, `.2`, and `.1.1`).
3. Requirement identifiers are permanently immutable: an identifier SHALL NEVER be renumbered or reassigned to a different requirement after retirement or decomposition.

### 1.2 Lifecycle Statuses

Every requirement block carries one of four normative lifecycle statuses:

1. `Active` — Normative, implementable leaf requirement enforced by automated tests at or after its target `Milestone` (`M0` through `M7` in the current specification baseline).
2. `Draft` — High-level system-contract requirement reserved for decomposition into `Active` dot-suffixed leaf requirements prior to implementing its target milestone.
3. `Superseded` — Former requirement split into child identifiers or replaced by revised identifiers explicitly listed in `Lineage: Superseded-By: <ID-list>`. Automated tests MUST migrate from superseded identifiers to their replacements.
4. `Removed` — Permanently retired requirement moved to or cross-referenced inside Section 15 (`Retired & Superseded Requirements Registry`) with `Lineage: Removed-Reason: <rationale>`. Automated tests referencing any `Removed` identifier fail traceability validation.

### 1.3 Uniform Requirement Schema & Automated Test Binding

1. Every requirement block in this document follows the exact seven-field schema (`ID`, `Status`, `Milestone`, `Verification Layer`, `Lineage`, `Requirement`, `Acceptance Criteria`) under a level-3 Markdown heading (`### <ID>: <Short Imperative Title>`).
2. Automated Rust unit, property, schema, determinism, CLI, and integration tests bind to requirement identifiers using `// @req ENG-<DOMAIN>-<NNN>` annotations placed directly above `#[test]` functions (or an equivalent `#[req("ENG-...")]` attribute macro), audited automatically by the `check-reqs` workspace verifier (`ENG-SCN-005`).

---

## 2. Licensing, Steam Compliance & Provenance (`ENG-LIC`)

### ENG-LIC-001: Permissive Royalty-Free Rust Dependency Allowlist
- **ID:** `ENG-LIC-001`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `License-Audit`
- **Lineage:** `Origin`
- **Requirement:** All Rust crates in the workspace dependency graph, including transitive dependencies, SHALL carry only permissive, royalty-free SPDX licenses on the project allowlist (`MIT`, `Apache-2.0`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `0BSD`, `Unicode-3.0`, `CC0-1.0`) and SHALL NOT depend on `GPL`, `AGPL`, static `LGPL`, `SSPL`, `CC-BY-NC`, or royalty-bearing crates.
- **Acceptance Criteria:** Executing `cargo deny check licenses` against the workspace `Cargo.lock` exits with status code `0` with zero unapproved, copyleft, or missing-license crates.

### ENG-LIC-002: Optional Feature-Gated Steamworks Adapter Boundary
- **ID:** `ENG-LIC-002`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `Build-Isolation`
- **Lineage:** `Origin`
- **Requirement:** Core crates (`sim-data`, `sim-core`, `player-view`, `sim-cli`) SHALL compile, link, and pass 100 percent of automated tests with zero proprietary Steamworks SDK headers or binaries present; any Steamworks integration SHALL be isolated behind an optional Cargo feature flag and adapter crate.
- **Acceptance Criteria:** Running `cargo test --workspace` in a clean environment lacking Steamworks SDK files succeeds cleanly without feature flags, and static dependency checks confirm `sim-core`, `sim-data`, and `player-view` contain zero references to Steamworks symbols.

### ENG-LIC-003: AI Asset Commercial Rights & Steam Pre-Generated Disclosure Provenance Validation
- **ID:** `ENG-LIC-003`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `Asset-Manifest-Audit`
- **Lineage:** `Origin`
- **Requirement:** The asset validation pipeline SHALL reject any shipped visual or audio asset whose provenance manifest omits generator model name and version, commercial license tier, `commercial_rights_verified = true`, human post-processing notes, or Steam Pre-Generated AI Content category, and SHALL emit a consolidated disclosure report.
- **Acceptance Criteria:** Automated manifest audit tests pass on valid commercial-verified manifests, reject fixture manifests marked non-commercial or missing fields with non-zero exit codes, and generate a deterministic `steam_ai_disclosure.json` summary covering every shipped asset.

### ENG-LIC-004: Automated Third-Party License Notice Aggregation
- **ID:** `ENG-LIC-004`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `Build-Packaging`
- **Lineage:** `Origin`
- **Requirement:** Release packaging tooling SHALL collect exact SPDX license identifiers, copyright lines, and full license texts across every compiled Rust dependency into a single distributable `THIRD_PARTY_NOTICES.txt` artifact.
- **Acceptance Criteria:** Running the license-bundle script produces a non-empty `THIRD_PARTY_NOTICES.txt` whose covered crate set matches 100 percent of non-workspace entries in `cargo metadata --format-version 1`.

---

## 3. Data-Driven Modding & Schema Pipeline (`ENG-MOD`)

### ENG-MOD-001: Zero Hardcoded Game Content in Compiled Rust Crates
- **ID:** `ENG-MOD-001`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `Static-Analysis & Integration`
- **Lineage:** `Origin`
- **Requirement:** Compiled Rust crates SHALL encode only invariant simulation physics, spatial graph routing, demographic transition math, finite-attention rules, epistemic filtering, and schema validation, and SHALL NOT embed hardcoded commodities, crops, technologies, facilities, carrier definitions, traits, doctrines, or starting scenarios.
- **Acceptance Criteria:** A fixture mod pack replacing every standard scenario, commodity, carrier, and facility identifier loads into `sim-core` and simulates 720 ticks without errors, panic, or fallback lookup into hardcoded engine defaults.

### ENG-MOD-002: Automated JSON Schema Generation From Rust Data Types
- **ID:** `ENG-MOD-002`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `Schema-Validation`
- **Lineage:** `Origin`
- **Requirement:** `sim-data` SHALL derive canonical JSON Schema specifications directly from its `serde` data-model definitions via `schemars` and export versioned schema files for mod authors and IDE validation.
- **Acceptance Criteria:** Running the schema-export command writes valid Draft 2020-12 JSON Schema documents for all registered mod domains, and committed schemas match freshly generated output byte-for-byte.

### ENG-MOD-003: Line-Accurate Diagnostic Errors on Malformed Mod Files
- **ID:** `ENG-MOD-003`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `Schema-Validation`
- **Lineage:** `Origin`
- **Requirement:** When loading invalid `TOML` or `JSON` mod definitions, `sim-data` SHALL return structured diagnostic errors reporting exact file path, line and column numbers, field path, and expected type or value constraints rather than panicking.
- **Acceptance Criteria:** Negative validation tests feeding syntax errors, unknown fields, out-of-range numbers, and type mismatches assert zero panics and exact matching file path and line numbers in returned diagnostics.

### ENG-MOD-004: Deterministic Five-Layer Mod Load & Override Resolution
- **ID:** `ENG-MOD-004`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** `sim-data` SHALL load and merge definitions in deterministic five-layer priority order (`core-schemas`, followed by `era-pack`, followed by `scenario-pack`, followed by ordered `user-mods`, followed by `ui-skin`), resolving record ID overrides and patch operations identically across repeated runs.
- **Acceptance Criteria:** Unit tests permuting filesystem directory read order confirm identical merged `ModBundle` serialization hashes and verify later layers deterministically override earlier layers by record ID.

### ENG-MOD-005: Cross-Domain Referential Integrity Validation
- **ID:** `ENG-MOD-005`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Schema-Validation`
- **Lineage:** `Origin`
- **Requirement:** Before initializing tick `0`, `sim-data` SHALL validate all cross-file foreign key references across commodities, processes, carriers, infrastructure classes, vocations, character traits, notification categories, and scenario entities, rejecting dangling references.
- **Acceptance Criteria:** Automated integrity tests assert that any process referencing a missing input commodity, scenario referencing an undefined vocation, or carrier referencing an unknown fuel class is rejected prior to world construction with an actionable reference error.

### ENG-MOD-006: Complete Coverage of All Eleven Externalized Data Domains
- **ID:** `ENG-MOD-006`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Integration`
- **Lineage:** `Origin`
- **Requirement:** `sim-data` SHALL load and bind all eleven externalized data domains specified in the architecture (planetary generation distributions, commodities and knowledge primitives, industrial/agricultural/construction processes, carriers and infrastructure, demographic cohorts, named individual traits, governance and policy laws, military formations and doctrines, scenarios and calendars, notifications and localization templates, and UI/globe skin manifests) exclusively from external files.
- **Acceptance Criteria:** End-to-end load tests verify that an empty workspace plus a complete external mod bundle populates all eleven registries and fails cleanly if any required domain manifest is omitted.

---

## 4. Simulation Clock, Determinism & Persistence (`ENG-CLK`)

### ENG-CLK-001: One-Hour Base Simulation Tick Resolution
- **ID:** `ENG-CLK-001`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** The simulation kernel SHALL advance time in discrete base steps equal to one simulated hour per tick, translating tick indices into scenario-defined calendar years, seasons, months, days, and local solar hours (`8,760` ticks per standard 365-day year).
- **Acceptance Criteria:** Stepping `sim-core` by `8,760` ticks under a 365-day calendar advances the simulated calendar by exactly one year, zero days, and zero hours with deterministic diurnal solar angle wraparound.

### ENG-CLK-002: Clock State Machine Supporting Pause, Step & Variable Rate
- **ID:** `ENG-CLK-002`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** The clock controller SHALL support paused state, deterministic single-tick advance (`+1 hour`) while paused, continuous speed multipliers, and automatic pause triggers triggered by configured event alerts without dropping or duplicating ticks.
- **Acceptance Criteria:** State-machine tests verify zero state mutation when paused over elapsed real time, exact single-tick increment on step commands, and immediate tick suspension on the exact tick raising an auto-pause notification.

### ENG-CLK-003: Deterministic Staggered Multi-Rate Scheduling Across 24 Hourly Buckets
- **ID:** `ENG-CLK-003`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Origin`
- **Requirement:** `sim-core` SHALL execute hourly systems (weather advection, active combat, courier propagation, HQ locality inspection) on every tick while partitioning daily systems across 24 deterministic hourly buckets and weekly/monthly systems across corresponding multi-day hourly buckets by stable entity or region identifiers.
- **Acceptance Criteria:** Multi-day property tests verify every daily entity executes exactly once per 24 ticks and every weekly entity executes exactly once per 168 ticks, with per-tick execution count variance bounded within `ceil(N / 24)` plus one.

### ENG-CLK-004: Bit-Identical Multi-Threaded Determinism Independent of Thread Count
- **ID:** `ENG-CLK-004`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `Determinism-Replay`
- **Lineage:** `Origin`
- **Requirement:** Given identical mod files, world seed, and timestamped command stream, `sim-core` SHALL produce bit-identical state hashes across runs executed with 1 worker thread versus `N` `rayon` worker threads.
- **Acceptance Criteria:** Replay tests executing `2,160` ticks (90 days) with randomized player commands under thread counts `1`, `2`, `4`, and `8` produce identical Blake3 state digests on tick `2,160`.

### ENG-CLK-005: Lossless Deterministic Save & Load Round-Trip
- **ID:** `ENG-CLK-005`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `Determinism-Replay`
- **Lineage:** `Origin`
- **Requirement:** Saving full simulation state plus RNG and scheduler cursors at any tick `T` and reloading into a fresh engine instance SHALL advance to tick `T + K` with a state hash bit-identical to an uninterrupted run to tick `T + K`.
- **Acceptance Criteria:** Running `0 -> 500`, serializing snapshot to disk, loading in a new process, and advancing `500 -> 1,500` yields the exact same tick-1,500 state hash as running `0 -> 1,500` continuously, and validates schema compatibility markers.

### ENG-CLK-006: Per-Subsystem Tick-Duration Profiling Telemetry
- **ID:** `ENG-CLK-006`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `CLI-Integration`
- **Lineage:** `Origin`
- **Requirement:** `sim-core` SHALL optionally record per-tick and per-subsystem execution wall-clock durations and bucket population counts for diagnostic inspection via `sim-cli`.
- **Acceptance Criteria:** Running `sim-cli` with profiling enabled outputs JSON and human-readable timing breakdowns covering weather, transport, production, population, and `player-view` projection without changing simulation hashes.

---

## 5. Geodesic Planet, Geology, Hydrology & Procedural Science (`ENG-GEO`)

### ENG-GEO-001: Closed Goldberg Polyhedron Geodesic Sphere Topology
- **ID:** `ENG-GEO-001`
- **Status:** `Superseded`
- **Milestone:** `M1`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Superseded-By: ENG-GEO-001.1`
- **Requirement:** Planetary geography SHALL be tessellated as a closed geodesic Goldberg polyhedron composed exclusively of hexagonal cells and exactly 12 pentagonal cells, enforcing symmetric neighbor adjacency, continuous great-circle distance metrics, and zero polar distortion singularities.
- **Acceptance Criteria:** Superseded by continuous hex-free spherical field architecture (`ENG-GEO-001.1`) while preserving underlying sample-graph Euler and area conservation invariants.

### ENG-GEO-001.1: Continuous Spherical Terrain Field & Natural Watershed Regions Without Hexagonal Tiling
- **ID:** `ENG-GEO-001.1`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Supersedes: ENG-GEO-001`
- **Requirement:** Planetary geography SHALL be represented on a closed unit sphere underpinned by a welded subdivided icosahedral sample manifold (`10 * m^2 + 2` sample nodes, `20 * m^2` spherical triangles, Euler characteristic `2`, zero exposed hexagonal tiles), providing continuous barycentric/inverse-angular terrain interpolation `sample_elevation_at_unit_xyz`, fast `nearest_sample_cell` lookup, symmetric sample adjacency, and deterministic natural watershed region groupings.
- **Acceptance Criteria:** Topology invariants verify `triangle_count == 20 * m^2`, Euler `V - E + F == 2`, symmetric sample adjacency, surface area within `0.01%` of `4 * pi * R^2`, smooth continuous elevation sampling across triangle interiors, and natural watershed region labels without hexagonal tile lock.

### ENG-GEO-002: Tectonic Plate Simulation Generating Elevation & Basins
- **ID:** `ENG-GEO-002`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Origin`
- **Requirement:** World generation SHALL simulate tectonic plates, convergent uplift, divergent rifting, volcanic arcs, and sedimentary basins to produce realistic elevation relief, ocean trenches, continental shelves, and continuous mountain passes.
- **Acceptance Criteria:** Generated planets across 20 random seeds satisfy configurable land-fraction targets, exhibit statistically higher mean elevation along convergent plate boundaries than plate interiors, and contain connected continental landmasses without artificial chessboard noise.

### ENG-GEO-003: Geologically Correlated Subsurface Ore & Fuel Belts
- **ID:** `ENG-GEO-003`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Origin`
- **Requirement:** Subsurface mineral veins, fossil fuel beds, and chemical fluxes SHALL be placed according to simulated tectonic, magmatic, metamorphic, and sedimentary strata so trained surveyors can infer hidden deposit probability from surface rock types and topography.
- **Acceptance Criteria:** Statistical tests across generated worlds confirm positive spatial correlation between designated geological strata (orogenic belts for metallic veins; sedimentary basins for coal/hydrocarbons) and hidden deposit density, while retaining unrevealed subsurface variance in depth, seam grade, and water-table hazard.

### ENG-GEO-004: Axial Tilt, Prevailing Winds, Rain Shadows & Acyclic Rivers
- **ID:** `ENG-GEO-004`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Origin`
- **Requirement:** Planetary climate generation SHALL derive latitude-dependent prevailing wind belts from planetary spin, orographic rain shadows downwind of mountain barriers, and strictly downhill acyclic watershed drainage networks terminating in oceans or endorheic salt lakes.
- **Acceptance Criteria:** Hydrological verification asserts zero flow cycles in the watershed graph, strictly non-increasing elevation from river headwaters to sink cells, and measurable mean annual precipitation reduction on leeward slopes of major mountain ridges relative to windward slopes.

### ENG-GEO-005: Procedural Generation of Materials, Ecology & Pathogens
- **ID:** `ENG-GEO-005`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Origin`
- **Requirement:** For every newly generated planet, the engine SHALL procedurally sample natural world tables—metallic ores, trace mineral impurities, smelting fluxes, solid and liquid fuels, crops, draft and wild fauna, and endemic pathogens—constrained within invariant physical conservation laws and realistic engineering trade-off curves.
- **Acceptance Criteria:** Worlds generated from distinct seeds exhibit materially different optimal smelting flux pairings, alloy strength/brittleness curves, fuel energy densities, and crop temperature tolerances while never violating conservation of mass or exceeding physical thermodynamic bounds.

### ENG-GEO-006: Deterministic Seeded Planet & Material Table Reproducibility
- **ID:** `ENG-GEO-006`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `Determinism-Replay`
- **Lineage:** `Origin`
- **Requirement:** Generating a planet from the same 64-bit seed, planetary parameter file, and mod bundle SHALL produce bit-identical cell geometry, hydrology, hidden deposits, and procedural material tables across platforms.
- **Acceptance Criteria:** Two independent planet generations using identical seed and mod bundle inputs yield identical Blake3 hashes over terrain, mineral deposits, and procedural material tables.

---

## 6. Dynamic Hourly Weather & Surface Physics (`ENG-WTH`)

### ENG-WTH-001: Hourly Per-Cell Physical Weather State Variables
- **ID:** `ENG-WTH-001`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** Every planetary cell SHALL maintain continuous hourly physical weather state variables comprising precipitation rate, soil saturation and mud index, river discharge and flood stage, ambient temperature, snowpack and river ice thickness, surface wind vector, and optical visibility.
- **Acceptance Criteria:** Hourly weather tests verify physically bounded ranges on every variable across all cells (`saturation in [0.0, 1.0]`, `visibility in [0.0, 1.0]`, non-negative precipitation and snowpack) over a full `8,760`-tick annual cycle.

### ENG-WTH-002: Axial-Tilt Seasonal Solar Forcing Across Hemispheres
- **ID:** `ENG-WTH-002`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Origin`
- **Requirement:** Solar insolation and seasonal atmospheric circulation SHALL depend on simulated orbital longitude and planetary axial tilt, producing warm summers and cold winters simultaneously reversed between Northern and Southern hemispheres.
- **Acceptance Criteria:** Over an `8,760`-tick orbital run on a tilted planet, mean mid-latitude surface temperature peaks in opposite halves of the year between `+45` degrees latitude and `-45` degrees latitude.

### ENG-WTH-003: Hydrological Coupling From Precipitation & Snowmelt to Mud & Floods
- **ID:** `ENG-WTH-003`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Origin`
- **Requirement:** Surface precipitation and spring snowmelt SHALL increase local soil saturation (producing mud above threshold soil limits) and propagate runoff downstream through the watershed DAG to raise river discharge and flood stages with realistic downstream travel lag.
- **Acceptance Criteria:** Injecting a synthetic 12-hour torrential storm into upstream headwater cells produces measurable soil mud spikes locally within 6 hours and downstream river flood stage peaks delayed proportionally to watershed graph distance while preserving total water mass balance minus evaporation.

### ENG-WTH-004: Weather-Driven Mine Flooding & Construction Work Halts
- **ID:** `ENG-WTH-004`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Integration`
- **Lineage:** `Origin`
- **Requirement:** Sustained heavy rainfall, high water-table inflow, or rising flood stage SHALL flood un-drained open pits and deep mines (halting extraction until pumped or drained) and stall weather-sensitive outdoor construction steps (lime mortar masonry, earthworks, framing) during severe storms or hard freezes.
- **Acceptance Criteria:** Integration tests expose an un-pumped deep mine and an active masonry project to a multi-day storm, verifying extraction halts with rising shaft water level and construction progress pauses while still logging weather-idle crew maintenance costs.

### ENG-WTH-005: Weather-Driven Crop Stress & Unroofed Spoilage Acceleration
- **ID:** `ENG-WTH-005`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Integration`
- **Lineage:** `Origin`
- **Requirement:** Agricultural harvests SHALL suffer quantifiable yield penalties or failure from frost, drought, and harvest-season storms, and perishable goods stored outdoors or in damp, unroofed depots SHALL suffer accelerated rot proportional to temperature and humidity/precipitation.
- **Acceptance Criteria:** Comparative tests over identical harvests show strictly higher grain spoilage inside open yards exposed to rain than inside dry roofed granaries, and crop collapse when exposed to freezing temperatures outside crop tolerance bounds.

---

## 7. Epistemic Air-Gap, HQ Presence & Signal Latency (`ENG-EPI`)

### ENG-EPI-001: Strict Serialization Air-Gap Between `sim-core` and `player-view`
- **ID:** `ENG-EPI-001`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `IPC-Contract`
- **Lineage:** `Origin`
- **Requirement:** `sim-core` internal ground-truth structures (unexplored terrain elevation, unsurveyed subsurface mineral reserves, exact distant stockpiles, real-time remote convoy coordinates, and unarrived event payloads) SHALL NOT be exposed through public serialization or IPC schemas; only `player-view` `DiegeticSnapshot` types SHALL cross the frontend boundary.
- **Acceptance Criteria:** Automated contract tests verify that serializing `player-view` for a newly initialized colony omits all unexplored cell geology/terrain attributes and remote unseen stockpile figures while retaining strict parity between snapshot fields and arrived HQ records.

### ENG-EPI-002: Zero-Latency Ground-Truth Visibility Strictly at HQ Locality
- **ID:** `ENG-EPI-002`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** Cells within immediate physical observation radius of the player's current HQ coordinate SHALL update inside `player-view` on the current hourly tick with zero signal delay, whereas all locations beyond HQ sight radius SHALL update strictly upon arrival of couriers, travelers, or telegraph/radio signals.
- **Acceptance Criteria:** Modifying a stockpile or weather state at the HQ cell reflects immediately in `player-view` on tick `T + 1`, whereas modifying an equivalent distant settlement leaves `player-view` unchanged until a physical dispatch completes travel to HQ.

### ENG-EPI-003: Physical HQ Relocation Mechanics Shifting Latency Contours
- **ID:** `ENG-EPI-003`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Integration`
- **Lineage:** `Origin`
- **Requirement:** Issuing an HQ relocation command SHALL physically transport the player character and HQ staff along the route network over real travel hours; while in transit and upon arrival, HQ immediate-observation radius shifts to the player's real coordinate, tightening latency to nearby operations while lengthening latency back to former hubs.
- **Acceptance Criteria:** Relocating HQ from Colony Hub A to Outpost B places the player in transit along road cells for expected travel hours, gives zero-latency inspection at Outpost B upon arrival, and increases dispatch travel duration from Hub A to HQ proportionally to physical separation.

### ENG-EPI-004: Two-Way Physical Courier & Signal Propagation
- **ID:** `ENG-EPI-004`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Integration`
- **Lineage:** `Origin`
- **Requirement:** Every directive issued by the player and every report sent toward HQ SHALL instantiate a physical message carrier or signal transmission traversing actual infrastructure or signal links, accumulating travel time, weather delays, and interception/loss hazards in transit.
- **Acceptance Criteria:** Issuing a mandate to a distant outpost under severe rain delays order start at the outpost until the courier arrives, and killing or flooding the courier route delays or triggers a lost-dispatch/overdue event at HQ.

### ENG-EPI-005: Geological & Topographic Surveys With Error Bars & Timestamps
- **ID:** `ENG-EPI-005`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** Survey records archived in `player-view` SHALL store exact survey tick timestamp, authoring surveyor skill tier, instrument accuracy class, and bounded uncertainty intervals (`min_estimate`, `point_estimate`, `max_estimate`) rather than exact hidden deposit totals.
- **Acceptance Criteria:** Repeating surveys of an identical hidden ore vein with a novice surveyor versus an expert equipped with core-sampling tools produces strictly narrower confidence intervals for the expert while keeping true vein tonnage confidential inside `sim-core`.

### ENG-EPI-006: Subordinate Epistemic Trait Perturbation on Outgoing Reports
- **ID:** `ENG-EPI-006`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** Named subordinates compiling reports, ledgers, or alerts SHALL deterministically transform underlying measurements according to their epistemic traits (`honesty/sycophancy` softening or hiding negative shortfalls, `precision/sloppiness` widening numeric rounding noise, and `caution/alarmism` biasing threat estimates).
- **Acceptance Criteria:** Given identical factory output shortfall and identical seed, a sycophantic/sloppy deputy reports inflated or imprecise figures to HQ compared to an honest/precise deputy, and audit inspections detect the discrepancy.

### ENG-EPI-007: Dead-Reckoning Ghosted Schedule Projections & Overdue Alerts
- **ID:** `ENG-EPI-007`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** For active expeditions and convoys outside HQ line-of-sight, `player-view` SHALL compute deterministic dead-reckoning projected coordinates (`ghosted` status) from departure schedules and last confirmed sighting, snapping cleanly when verified sightings arrive and transitioning to `overdue` once expected arrival plus configurable grace window elapses without contact.
- **Acceptance Criteria:** Blocking a convoy in mud without reporting keeps its `player-view` projected position moving along its scheduled path until the ETA grace window expires, at which point `player-view` flips status to `overdue` and emits an alert while true position remains sealed in `sim-core`.

---

## 8. Command, Attention, Mandates & Bureaucracy (`ENG-CMD`)

### ENG-CMD-001: Bounded Personal Attention Capacity
- **ID:** `ENG-CMD-001`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** Every Named Individual, including the player character, SHALL carry a finite numeric **Attention Capacity** derived from base human limits modified by organizational competence traits and active administrative tooling.
- **Acceptance Criteria:** Unit tests confirm assigned direct reports and active supervised mandates sum cleanly against capacity and verify capacity remains strictly finite across character configurations.

### ENG-CMD-002: Non-Linear Oversight Dilution Under Attention Overload
- **ID:** `ENG-CMD-002`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Origin`
- **Requirement:** Whenever a leader's active direct supervision load exceeds their Attention Capacity, the engine SHALL apply non-linear **Oversight Dilution** penalties across supervised mandates, measurably increasing subordinate shirking, supply skimming, construction slippage, safety accidents, and mandate drift.
- **Acceptance Criteria:** Controlled simulations varying direct reports from `4` (within budget) to `12` (overloaded) demonstrate monotonic non-linear degradation in delivered output per worker-hour and elevated unreported material shrinkage.

### ENG-CMD-003: Cascading Decomposition of Strategic Mandates
- **ID:** `ENG-CMD-003`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Integration`
- **Lineage:** `Origin`
- **Requirement:** Issuing a high-level strategic mandate to an assigned subordinate leader SHALL cause that leader to autonomously decompose the objective into concrete operational sub-tasks (surveying, site selection, labor allocation, input procurement, transport assignment) using only information known within their own epistemic horizon.
- **Acceptance Criteria:** Issuing *"Increase iron/metal production"* to a Mining Overseer automatically generates survey expeditions or shaft-expansion tasks and fuel procurement requests without manual player building placement.

### ENG-CMD-004: Bottleneck & Proposal Escalation Beyond Delegated Authority
- **ID:** `ENG-CMD-004`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Integration`
- **Lineage:** `Origin`
- **Requirement:** When sub-task execution encounters constraints exceeding a subordinate's budget, labor authority, or legal mandate (for instance, shaft flooding, cross-ministry road congestion, or monopoly petitions), the subordinate SHALL generate a structured decision proposal containing competing resolution options and transmit it to the superior authority.
- **Acceptance Criteria:** Inducing deep mine flooding or cross-settlement haulage conflict halts unilateral subordinate override and dispatches an escalation proposal with at least two selectable policy/resource alternatives to HQ.

### ENG-CMD-005: Personal Attention Zoom-In Direct Supervision
- **ID:** `ENG-CMD-005`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** The player SHALL be able to allocate slices of Personal Attention directly to specific critical projects, expeditions, or offices—improving local oversight precision and suppressing metric gaming on that project while directly reducing available Attention across remaining direct reports.
- **Acceptance Criteria:** Toggling zoom-in Attention onto Project A reduces Project A oversight dilution to zero while measurably increasing Attention load percentage and dilution risk on simultaneously active Ministries B and C.

### ENG-CMD-006: Multi-Layer Organizational Hierarchy Frictions
- **ID:** `ENG-CMD-006`
- **Status:** `Superseded`
- **Milestone:** `M5`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Superseded-By: ENG-CMD-006.1, ENG-CMD-006.2, ENG-CMD-006.3, ENG-CMD-006.4`
- **Requirement:** Multi-tier bureaucratic hierarchies SHALL impose compounding signal lag and filtering across depth, Goodhart metric gaming on uninspected quantitative quotas, non-productive clerical labor and food overhead, and institutional inertia resisting structural reforms.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-CMD-006.1` through `ENG-CMD-006.4`.

### ENG-CMD-006.1: Hierarchy Hop Signal Lag & Upward Bad-News Filtering
- **ID:** `ENG-CMD-006.1`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Parent: ENG-CMD-006`
- **Requirement:** Passing directives downward and reports upward across hierarchical tiers SHALL impose cumulative per-hop latency (`hop_count * hierarchy_hop_latency_hours`) and upward bad-news attenuation when intermediate officials possess sycophantic bias.
- **Acceptance Criteria:** Property tests comparing 1-hop direct supervision against 3-hop bureaucratic routing under equal spatial separation assert strictly longer turnaround delay and softened shortfall reporting across extra hops.

### ENG-CMD-006.2: Goodhart Metric Gaming Under Uninspected Quotas
- **ID:** `ENG-CMD-006.2`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Parent: ENG-CMD-006`
- **Requirement:** Quantitative output quotas enforced without active physical inspection or audit SHALL induce subordinates to inflate nominal delivered mass at the expense of batch purity and tool/shaft wear until an inspection or audit triggers enforcement.
- **Acceptance Criteria:** Controlled tests issuing high uninspected tonnage quotas show nominal tonnage targets met alongside falling average batch purity and rising equipment wear compared to audited facilities.

### ENG-CMD-006.3: Non-Productive Clerical Overhead Draw
- **ID:** `ENG-CMD-006.3`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Parent: ENG-CMD-006`
- **Requirement:** Every active mandate and administrative bureau under clerical coordination SHALL consume proportional literate clerical labor hours and physical staple rations per day without directly extracting raw commodities.
- **Acceptance Criteria:** Switching from low-overhead patrimonial supervision to multi-bureau standardized administration measurably increases daily grain/wage draw per supervised settlement proportional to `clerical_overhead_per_mandate`.

### ENG-CMD-006.4: Institutional Reform Inertia Delay & Opposition
- **ID:** `ENG-CMD-006.4`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-CMD-006`
- **Requirement:** Switching governance models or abolishing vested offices SHALL incur deterministic institutional inertia (transition delay hours and temporary efficiency depression proportional to incumbent bureau depth and local official resistance).
- **Acceptance Criteria:** Triggering a governance reform on an entrenched bureaucracy enforces a non-zero transition stabilization period with temporary productivity friction before the new coordination weights settle.

### ENG-CMD-007: Five Governance & Coordination Models Shifting Friction Profiles
- **ID:** `ENG-CMD-007`
- **Status:** `Superseded`
- **Milestone:** `M5`
- **Verification Layer:** `Integration`
- **Lineage:** `Superseded-By: ENG-CMD-007.1, ENG-CMD-007.2`
- **Requirement:** Externalized governance models (Direct Personal/Patrimonial Rule, Devolved Fiefs/Tax Farming, Standardized State Bureaucracy, Guilds/Chartered Monopolies, and Open Markets/Contracts/Patents) SHALL shift which organizational frictions dominate without ever eliminating systemic friction.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-CMD-007.1` and `ENG-CMD-007.2`.

### ENG-CMD-007.1: Externalized Five Governance Model Definitions
- **ID:** `ENG-CMD-007.1`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Schema-Validation`
- **Lineage:** `Parent: ENG-CMD-007`
- **Requirement:** `sim-data` and `sim-plug-private-eco` SHALL load and activate all five externalized governance models (`patrimonial_rule`, `devolved_fiefs`, `state_bureaucracy`, `chartered_guilds`, `open_markets_patents`) with zero hardcoded governance branches.
- **Acceptance Criteria:** Integration tests switch an active campaign cleanly across all five loaded governance models from `mods/core-schemas/governance/institutions.toml` and verify valid Attention, clerical overhead, and hop latency updates.

### ENG-CMD-007.2: Non-Eliminable Trade-Off Matrix Across Governance Models
- **ID:** `ENG-CMD-007.2`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Parent: ENG-CMD-007`
- **Requirement:** Each of the five governance models SHALL shift the dominant friction vector—patrimonial low overhead versus scale collapse, fief low overhead versus skimming, bureaucracy scale versus clerical/Goodhart drag, guilds quality versus output restriction, and markets low state Attention versus price volatility—never reducing total structural friction to zero.
- **Acceptance Criteria:** Comparative benchmarks running identical settlement workloads across all five models prove monotonic trade-offs where the minimum-overhead model suffers higher skimming/scale failure and the highest-capacity model suffers higher clerical overhead.

---

## 9. Two-Tier Population, Named Individuals & Succession (`ENG-POP`)

### ENG-POP-001: Two-Tier Demographic Cohort & Named Individual Architecture
- **ID:** `ENG-POP-001`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** Population simulation SHALL represent general inhabitants as grouped **Demographic Cohorts** keyed by settlement/district and occupation/stratum while representing key leaders, specialists, commanders, and enterprise heads as **Named Individuals**, scaling smoothly from dozens to millions without per-citizen agent simulation.
- **Acceptance Criteria:** Micro-benchmark tests running identical logic on settlements totaling `200` inhabitants versus `2,000,000` inhabitants execute in constant `O(cohorts)` time per settlement.

### ENG-POP-002: Six-Attribute Cohort Model & Vital Transitions
- **ID:** `ENG-POP-002`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Origin`
- **Requirement:** Every Demographic Cohort SHALL maintain six tracked state dimensions—Age Structure (dependents, prime workers, elders), General Education and Literacy, Practical Domain Experience vector, Health/Nutrition/Stamina, Household Wealth and Material Reserves, and Social Cohesion/Disposition—updated across vital birth, aging, and mortality transitions.
- **Acceptance Criteria:** Multi-year demographic tests conserve cohort headcounts (`pop_t1 = pop_t0 + births + in_migration - deaths - out_migration`), shift cohorts smoothly across age brackets over simulated years, and respond properly to caloric deficits with rising mortality and declining fertility.

### ENG-POP-003: Practical Domain Experience Accrual & Generational Decay
- **ID:** `ENG-POP-003`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Origin`
- **Requirement:** Practical Domain Experience SHALL accrue exclusively through active vocational work or structured apprenticeship alongside experienced cohorts and SHALL decay during prolonged industrial idleness or unreplaced mortality of elder cohorts.
- **Acceptance Criteria:** Shutting down all smelting workshops for 5 simulated years or suffering high elder mortality without apprentices measurably lowers colony average smelting proficiency, reducing future metal purity and output rates.

### ENG-POP-004: Named Individual Promotion & Multi-Axis Trait Vectors
- **ID:** `ENG-POP-004`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** Named Individuals SHALL be promotable out of experienced cohorts or spawned via scenario cadres carrying explicit trait vectors spanning Domain Competence, Cognitive/Research traits, Epistemic Reporting traits, and Personal Drives (loyalty, ambition, greed/corruption, peer compatibility).
- **Acceptance Criteria:** Promoting a candidate from a high-literacy, experienced mining cohort generates a deterministic Named Individual inheriting cohort cultural/educational priors and populated trait bounds validated by unit assertions.

### ENG-POP-005: Cohort Nutrition, Stamina & Grievance Effects on Productivity & Unrest
- **ID:** `ENG-POP-005`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Origin`
- **Requirement:** Deficits in caloric intake, fuel, clothing, or wages combined with high tax pressure or low leadership trust SHALL depress labor productivity and social cohesion, progressing continuously from passive work slowdown to out-migration, organized strikes, banditry, and open revolt.
- **Acceptance Criteria:** Depriving an extraction cohort of winter fuel and adequate rations monotonically lowers effective worker output and raises grievance thresholds into measurable strike and migration events unless mitigated.

### ENG-POP-006: Mortal Leader Aging & Generational Succession Continuity
- **ID:** `ENG-POP-006`
- **Status:** `Superseded`
- **Milestone:** `M7`
- **Verification Layer:** `Integration`
- **Lineage:** `Superseded-By: ENG-POP-006.1, ENG-POP-006.2`
- **Requirement:** Player character death, retirement, or political deposition SHALL invoke externalized succession rules selecting an heir, elected magistrate, or factional successor who inherits strictly the physical cartographic archive, physical ledgers, existing institutions, debts, and entrenched officials without hidden player memory transfer.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-POP-006.1` and `ENG-POP-006.2`.

### ENG-POP-006.1: Mortal Player Ruler Aging, Mortality & Externalized Succession Trigger
- **ID:** `ENG-POP-006.1`
- **Status:** `Active`
- **Milestone:** `M7`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-POP-006`
- **Requirement:** The player ruler SHALL age chronologically with calendar time, accumulate health/campaign mortality risk, and trigger deterministic generational succession governed by active external `SuccessionRuleRecord` definitions (`council_senior_appointee`, `hereditary_charter_heir`, `elected_magistrate`), pausing the clock via `leader_succession` notification.
- **Acceptance Criteria:** Simulating multi-decade progression or mortal hazard triggers deterministic ruler demise, selects an eligible successor matching the active mod succession rule, and sets `ClockRunMode::AutoPaused`.

### ENG-POP-006.2: Physical-Only Archive Inheritance & Unrecorded Head-Memory Loss
- **ID:** `ENG-POP-006.2`
- **Status:** `Active`
- **Milestone:** `M7`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-POP-006`
- **Requirement:** Upon succession, the incoming ruler SHALL inherit strictly the recorded `PlayerKnowledgeArchive` survey stamps, physical commodity ledgers, standing institutions, private credit/debts, and named officials—while losing personal unrecorded observations held solely in the deceased ruler's head and incurring `transition_disruption_hours` administrative lag.
- **Acceptance Criteria:** Succession tests record surveyed maps and paper ledgers alongside unrecorded personal ruler notes before succession, confirming 100% map/ledger continuity and zero retention of unrecorded personal notes post-succession.

---

## 10. Unified Public/Private Economy & Physical Ledger (`ENG-ECO`)

### ENG-ECO-001: Single Physical Goods Ledger With Conservation of Mass
- **ID:** `ENG-ECO-001`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Origin`
- **Requirement:** Every commodity unit (foodstuffs, ores, refined metals, fuels, timber, tools, armaments, textiles, livestock) SHALL reside explicitly inside a named spatial stockpile, building yard, or carrier cargo hold tagged with legal owner ID, batch purity/quality grade, condition, and spoilage age, enforcing strict physical mass conservation across transfers and processes.
- **Acceptance Criteria:** Property tests assert global conservation (`sum(inputs) == sum(outputs + slag/waste + consumed_fuel + spoiled_mass)`) across every tick and reject negative stockpile quantities.

### ENG-ECO-002: Physical Equipment & Starting Tool Wear & Degradation
- **ID:** `ENG-ECO-002`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** Work processes (mining, quarrying, felling, farming, smelting, construction) SHALL apply measurable physical wear proportional to task intensity and material hardness against equipped tools and machinery, degrading tool efficiency and breaking worn-out tools unless repaired or replaced.
- **Acceptance Criteria:** Running extraction without tool fabrication in "The Unexplained Colony" depletes initial high-grade tool durability over predictable operating hours, reducing output and eventually stalling mechanized extraction until replacement tools are forged.

### ENG-ECO-003: Cohort Subsistence Consumption & Data-Driven Process Execution
- **ID:** `ENG-ECO-003`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Integration`
- **Lineage:** `Origin`
- **Requirement:** Cohorts SHALL consume physical calories, potable water, heating fuel, and clothing from local household or ration stocks, while productive buildings execute externalized `sim-data` extraction, refining, agricultural, and construction recipes gated by physical input delivery, labor hours, tooling, and site conditions.
- **Acceptance Criteria:** End-to-end colony tests verify starvation/cold responses when food/fuel stocks exhaust and confirm mine/smelter/farm batches execute strictly according to external `TOML` process definitions.

### ENG-ECO-004: Zero Hardcoded Public-Only or Private-Only Facility Restrictions
- **ID:** `ENG-ECO-004`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Schema-Validation`
- **Lineage:** `Origin`
- **Requirement:** Facility and enterprise schemas SHALL permit any productive facility type to be constructed and operated by public offices, chartered guilds, or private actors according to active legal policy rather than hardcoding exclusive public-only or private-only facility buckets.
- **Acceptance Criteria:** Schema validation and integration tests construct identical mine and smelter process definitions owned alternately by a state bureau and a private syndicate using the exact same simulation kernel paths.

### ENG-ECO-005: Autonomous Non-State Institutional Economic Actors
- **ID:** `ENG-ECO-005`
- **Status:** `Superseded`
- **Milestone:** `M5`
- **Verification Layer:** `Integration`
- **Lineage:** `Superseded-By: ENG-ECO-005.1, ENG-ECO-005.2, ENG-ECO-005.3, ENG-ECO-005.4`
- **Requirement:** Non-state economy execution SHALL instantiate explicit Households/Smallholders, Landed Estates/Clans, Merchant Houses/Hauling Guilds, and Syndicates/Craft Guilds/Chartered Companies operating persistent physical inventories, credit/capital ledgers, autonomous inter-regional price arbitrage, and capital expansion projects.
- **Acceptance Criteria:** Decomposed into per-actor-class leaf requirements `ENG-ECO-005.1` through `ENG-ECO-005.4`.

### ENG-ECO-005.1: Households & Smallholders Subsistence Retention & Residual Trade
- **ID:** `ENG-ECO-005.1`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-ECO-005`
- **Requirement:** Household/Smallholder actors SHALL reserve subsistence calories, heating fuel, and personal tool replacement before offering residual surplus into local exchange or paying rent obligations.
- **Acceptance Criteria:** Reducing household grain holdings near subsistence threshold shrinks market sell offers to zero while high harvests release measurable surplus onto the local settlement price board.

### ENG-ECO-005.2: Landed Estates & Clans Rent Extraction, Hoarding & Audit Resistance
- **ID:** `ENG-ECO-005.2`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-ECO-005`
- **Requirement:** Landed Estate actors SHALL collect in-kind crop shares and labor dues, withhold buffer grain during rising famine risk (hoarding), and inject downward reporting undercounts against central tax/census scrutiny.
- **Acceptance Criteria:** Under rising grain scarcity, estate private granary stocks expand faster than spot sales and reported estate taxable yield trails actual ledger mass until audited.

### ENG-ECO-005.3: Merchant Houses Autonomous Spatial Price-Arbitrage Caravans
- **ID:** `ENG-ECO-005.3`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-ECO-005`
- **Requirement:** Merchant House actors SHALL maintain lagged regional price intelligence and autonomously purchase low-price regional surpluses and dispatch real physical caravans (`LogisticsPort`) to sell at profitable destination shortages without player directives.
- **Acceptance Criteria:** Seeding a two-settlement price disparity greater than carrier haul cost triggers autonomous merchant cargo shipments along connected roads that narrow the inter-settlement price gap upon arrival.

### ENG-ECO-005.4: Syndicates, Guilds & Chartered Companies Autonomous Capital Projects
- **ID:** `ENG-ECO-005.4`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-ECO-005`
- **Requirement:** Syndicate and Chartered Guild actors SHALL pool private capital and physical construction inputs to establish and operate autonomous extraction or refining facilities whenever expected commercial margins exceed political/physical risk thresholds.
- **Acceptance Criteria:** High sustained metal/fuel market prices paired with concession or open-market rules trigger autonomous syndicate facility investments tagged with private `ActorId` ownership on the shared ledger.

### ENG-ECO-006: Era-Universal Multi-Medium Economic Accounting
- **ID:** `ENG-ECO-006`
- **Status:** `Superseded`
- **Milestone:** `M5`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Superseded-By: ENG-ECO-006.1, ENG-ECO-006.2`
- **Requirement:** Economic exchange and settlement clearing SHALL support pure physical barter, in-kind tribute and corvée obligations, commodity coinage, promissory credit, and state allocation quotas under one unified accounting engine without requiring fiat currency from tick `0`.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-ECO-006.1` and `ENG-ECO-006.2`.

### ENG-ECO-006.1: Zero-Currency Barter, Commodity Money, Tribute & Corvee Clearing
- **ID:** `ENG-ECO-006.1`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Parent: ENG-ECO-006`
- **Requirement:** Non-state exchange SHALL clear balanced bilateral trades directly in physical goods (barter ratios weighted by local scarcity) and settle in-kind crop tributes and compulsory labor hours when circulating coinage or credit equals zero.
- **Acceptance Criteria:** Property tests initializing an isolated colony with zero metallic coin or credit balances execute household-artisan exchanges and tribute deliveries while preserving strict physical mass conservation.

### ENG-ECO-006.2: Coinage, Promissory Credit & Command Quota Clearing
- **ID:** `ENG-ECO-006.2`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Parent: ENG-ECO-006`
- **Requirement:** When circulating specie or institutional credit accounts exist, merchant houses, syndicates, and state procurement offices SHALL clear multi-party trades across promissory credit balances, specie reserves, and requisitioned quotas on the same underlying batch ledger.
- **Acceptance Criteria:** Clearing mixed barter, credit settlement, and state purchase contracts conserves total commodity mass and balances net double-entry credit commitments deterministically across all involved `ActorId` accounts.

### ENG-ECO-007: Cross-Era Economic Policy Levers
- **ID:** `ENG-ECO-007`
- **Status:** `Superseded`
- **Milestone:** `M5`
- **Verification Layer:** `Integration`
- **Lineage:** `Superseded-By: ENG-ECO-007.1, ENG-ECO-007.2`
- **Requirement:** Policy interfaces SHALL enable players and regimes to fulfill production objectives via tribute/corvée levies, market purchase orders/subsidies/tariffs, exclusive concessions/monopolies, or direct state-owned enterprises, each generating authentic institutional side-effects.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-ECO-007.1` and `ENG-ECO-007.2`.

### ENG-ECO-007.1: Four Cross-Era Economic Policy Levers
- **ID:** `ENG-ECO-007.1`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-ECO-007`
- **Requirement:** Validated player commands SHALL support enacting `tribute_corvee`, `market_purchase_orders`, `chartered_concession`, and `direct_state_enterprise` policies per scenario/commodity loaded from `sim-data` `PolicyLawRecord` definitions.
- **Acceptance Criteria:** Enacting each policy lever updates active institutional rules deterministically and alters state versus private output and grievance trajectories in headless runs.

### ENG-ECO-007.2: Structural Policy Side-Effects & Oligarchic Entrenchment
- **ID:** `ENG-ECO-007.2`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-ECO-007`
- **Requirement:** Posted `market_purchase_orders` SHALL stimulate autonomous private supply mobilization at zero additional state Attention while driving scarcity profiteering spikes; `chartered_concession` SHALL shift upfront capital burden off the state while increasing syndicate oligarch leverage and output share; `direct_state_enterprise` SHALL guarantee allocation priority at direct clerical and Attention cost.
- **Acceptance Criteria:** Side-by-side tests prove zero state Attention drain under `market_purchase_orders`, growing syndicate share under `chartered_concession`, and direct Attention drain under `direct_state_enterprise`.

---

## 11. Physical Carriers & Shared Logistics Network (`ENG-LOG`)

### ENG-LOG-001: Mass-Proportional Carrier Operating Consumption
- **ID:** `ENG-LOG-001`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** Every active physical carrier (porters, pack animals, carts, barges, sailing ships, rail trains, motorized vehicles) SHALL consume route-local fodder, fuel, spare parts, and crew labor hours scaled to payload mass, distance, and terrain difficulty.
- **Acceptance Criteria:** Dispatching an ox-cart column over an unprovisioned desert corridor lacking fodder depots halts or starves draft animals along the route according to explicit load/fodder equations.

### ENG-LOG-002: Terrain, Bridge, Hydrology, Wind & Mud Penalties on Transit
- **ID:** `ENG-LOG-002`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** Carrier traversal speeds, fuel burn, and breakdown probabilities SHALL depend physically on terrain elevation slope, infrastructure class, bridge weight caps, river flow direction and winter ice stage, prevailing wind vectors for sail craft, and hourly soil mud/snow indices.
- **Acceptance Criteria:** Unit routing tests demonstrate slower uphill haul speeds, doubled animal strain in saturated mud, halted barge movement on frozen rivers, and washed-out ford blocks during river flood stage.

### ENG-LOG-002.1: Continuous Spherical Coordinate & Any-Angle Speed-Time Transit
- **ID:** `ENG-LOG-002.1`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Integration`
- **Lineage:** `Origin`
- **Requirement:** Active convoys, couriers, survey parties, and relocating HQ staff SHALL maintain continuous 3D unit-sphere coordinates `[f32; 3]` (`origin_unit_xyz`, `current_unit_xyz`, `destination_unit_xyz`), advancing continuously each transit step by `distance_km = effective_speed_kmh * dt_hours` along road/river splines and any-angle string-pulled great-circle arcs across passable terrain.
- **Acceptance Criteria:** Integration tests verify smooth fractional great-circle coordinate progression matching speed-time equations and strictly shorter path distances via string-pulled open-country arcs compared to constrained 60-degree mesh hops.

### ENG-LOG-003: Unified Transport for Cargo, Cohorts, Leaders & Dispatches
- **ID:** `ENG-LOG-003`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Integration`
- **Lineage:** `Origin`
- **Requirement:** All physical movements—freight deliveries, cohort migrations, military marches, traveling Named Individuals, geological assay samples, and courier dispatches—SHALL share the exact same physical graph pathfinder and carrier/infrastructure physics without separate teleporting movement channels.
- **Acceptance Criteria:** Washing out a river bridge on the main highway simultaneously delays supply carts, marching replacements, HQ staff relocation, and physical couriers traveling that edge.

### ENG-LOG-004: Strict Prohibition of Instantaneous Resource Teleportation
- **ID:** `ENG-LOG-004`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Origin`
- **Requirement:** A commodity unit located at settlement `A` SHALL NOT be consumed by any facility, household, or unit at settlement `B` (`A != B`) unless physically transported along connected infrastructure edges by carriers over elapsed simulation hours.
- **Acceptance Criteria:** Property tests severing all edges between Mining Camp A and Smelter B verify Smelter B runs exclusively on existing local stocks and halts once local ore exhausts, regardless of Mining Camp A surplus.

### ENG-LOG-005: Shared Road & Port Throughput Congestion
- **ID:** `ENG-LOG-005`
- **Status:** `Superseded`
- **Milestone:** `M5`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Superseded-By: ENG-LOG-005.1, ENG-LOG-005.2`
- **Requirement:** State Quartermaster convoys and private merchant haulers SHALL share finite edge throughput and port berth capacity, creating physical queuing delays and road degradation proportional to combined traffic axle load.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-LOG-005.1` and `ENG-LOG-005.2`.

### ENG-LOG-005.1: Shared Corridor & Port Axle-Load Congestion Slowdown
- **ID:** `ENG-LOG-005.1`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Parent: ENG-LOG-005`
- **Requirement:** `NonStateEconomyPlugPort::edge_congestion_multiplier` SHALL aggregate state and private carrier mass transiting each shared corridor/port per window and apply monotonic speed reduction (`< 1.0`) to both state and private convoys once combined tonnage approaches infrastructure throughput capacity.
- **Acceptance Criteria:** Dispatching heavy simultaneous state and private cargo across a shared mountain track slows both convoy streams strictly relative to uncongested single-convoy transit ticks.

### ENG-LOG-005.2: Cumulative Axle-Load Infrastructure Surface Wear
- **ID:** `ENG-LOG-005.2`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Parent: ENG-LOG-005`
- **Requirement:** Heavy combined axle tonnage traversing unpaved or lightly paved corridors SHALL accumulate physical surface wear (`wear_01`), worsening mud sensitivity and maintenance timber demand unless repaired.
- **Acceptance Criteria:** Running high cumulative wagon tonnage across an earth track raises tracked `wear_01` monotonically and degrades effective road speed compared to unloaded idle corridors.

### ENG-LOG-006: Dynamic Carrier Chartering & Civilian Freight Crowding-Out
- **ID:** `ENG-LOG-006`
- **Status:** `Superseded`
- **Milestone:** `M5`
- **Verification Layer:** `Integration`
- **Lineage:** `Superseded-By: ENG-LOG-006.1, ENG-LOG-006.2`
- **Requirement:** State requisitioning or chartering of regional wagons, barges, or ships SHALL draw directly from the finite shared carrier fleet, automatically driving up regional freight rates and reducing haul capacity available for civilian commerce.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-LOG-006.1` and `ENG-LOG-006.2`.

### ENG-LOG-006.1: Dynamic Regional Carrier Pool & Charter Rate Scaling
- **ID:** `ENG-LOG-006.1`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-LOG-006`
- **Requirement:** Each settlement region SHALL track a finite private carrier pool (`available_carriers / total_carriers`), computing chartering costs non-linearly upward as idle carrier slack shrinks.
- **Acceptance Criteria:** Chartering state carriers out of a regional pool directly increments utilization and monotonically raises quoted regional freight charter rates.

### ENG-LOG-006.2: State Chartering Crowding-Out Civilian Food & Fuel Haulage
- **ID:** `ENG-LOG-006.2`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-LOG-006`
- **Requirement:** Drawing private wagons or vessels into state/military charter service SHALL reduce remaining un-chartered private carrying capacity, throttling civilian merchant deliveries of staple food/fuel and lifting destination spot prices.
- **Acceptance Criteria:** Comparative simulations prove mass state carrier chartering lowers civilian commodity tonnage delivered by merchant caravans and raises destination scarcity prices relative to unchartered baseline runs.

---

## 12. Non-Bought Research & Fragile Knowledge (`ENG-RES`)

### ENG-RES-001: Strict Prohibition of Abstract Science Points & Purchasable Tech Trees
- **ID:** `ENG-RES-001`
- **Status:** `Superseded`
- **Milestone:** `M6`
- **Verification Layer:** `Static-Analysis & Property-Sim`
- **Lineage:** `Superseded-By: ENG-RES-001.1, ENG-RES-001.2`
- **Requirement:** The engine SHALL NOT model abstract research-point currencies or allow players to directly purchase technologies from a pre-revealed tech graph; all innovation SHALL arise from physical bottlenecks, experimentation, leisure study, and institutional incentives.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-RES-001.1` and `ENG-RES-001.2`.

### ENG-RES-001.1: Zero Global Science-Point Currency In Engine State
- **ID:** `ENG-RES-001.1`
- **Status:** `Active`
- **Milestone:** `M6`
- **Verification Layer:** `Static-Analysis & Property-Sim`
- **Lineage:** `Parent: ENG-RES-001`
- **Requirement:** Neither `sim-plug-research`, `ScenarioWorld`, nor `DiegeticSnapshot` SHALL store or emit an abstract spendable science/tech-point pool (`science_points`, `research_mana`, or unlock currency).
- **Acceptance Criteria:** Automated inspection of serialized research state and save payloads confirms every progression counter is tied to physical trials, bottlenecks, prototypes, or scholar hours with zero global point pools.

### ENG-RES-001.2: IPC & Command Rejection of Direct Technology-Purchase Commands
- **ID:** `ENG-RES-001.2`
- **Status:** `Active`
- **Milestone:** `M6`
- **Verification Layer:** `IPC-Contract`
- **Lineage:** `Parent: ENG-RES-001`
- **Requirement:** `player-view` `ValidatedPlayerCommand` parsing SHALL explicitly reject any command attempting to directly purchase or unlock a technology node (`unlockTechNode`, `buyResearch`, or equivalent), returning `ERR_BOUGHT_TECH_FORBIDDEN`.
- **Acceptance Criteria:** Contract tests submitting synthetic direct tech-unlock JSON commands assert strict rejection with `ERR_BOUGHT_TECH_FORBIDDEN` and zero mutation of settlement knowledge state.

### ENG-RES-002: Know-How Empirical Process & Component Cross-Pollination
- **ID:** `ENG-RES-002`
- **Status:** `Superseded`
- **Milestone:** `M6`
- **Verification Layer:** `Integration`
- **Lineage:** `Superseded-By: ENG-RES-002.1, ENG-RES-002.2`
- **Requirement:** Practical Know-How discoveries SHALL arise from workplace problems and material testing as reusable physical components whose physical existence inside working devices lowers discovery and adaptation cost for adjacent machines across workshops and observers.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-RES-002.1` and `ENG-RES-002.2`.

### ENG-RES-002.1: Workplace Bottleneck Empirical Know-How Discovery
- **ID:** `ENG-RES-002.1`
- **Status:** `Active`
- **Milestone:** `M6`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-RES-002`
- **Requirement:** Physical workplace bottlenecks (flooded deep mine shafts, brittle high-slag smelting, axle wear in mud) paired with active practitioners and small trial batches SHALL accumulate empirical component familiarity and surface working `TechComponentId` Know-How breakthroughs.
- **Acceptance Criteria:** Operating an un-pumped flooding mine with metallurgist/engineer labor advances drainage-valve Know-How and yields functional component adoption without spending any abstract points.

### ENG-RES-002.2: Proven Physical Component Cross-Pollination
- **ID:** `ENG-RES-002.2`
- **Status:** `Active`
- **Milestone:** `M6`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-RES-002`
- **Requirement:** Once a physical `TechComponentId` prototype operates in a workshop, observing mechanics SHALL cross-pollinate that component into related machines or processes within the same domain, cutting required empirical trial iterations by at least `35%`.
- **Acceptance Criteria:** Controlled comparison tests demonstrate that pre-existence of a working `check_valve_assembly` / `lever_and_fulcrum` component accelerates breakthrough of related mechanical/cylinder components compared to an isolated colony lacking working precedents.

### ENG-RES-003: Know-Why Theoretical Discovery Accelerating Directed Engineering
- **ID:** `ENG-RES-003`
- **Status:** `Superseded`
- **Milestone:** `M6`
- **Verification Layer:** `Integration`
- **Lineage:** `Superseded-By: ENG-RES-003.1, ENG-RES-003.2`
- **Requirement:** Individuals with surplus free time studying natural phenomena and existing machines SHALL generate theoretical Know-Why models that trail initial practical inventions but subsequently make directed applied R&D dramatically faster and higher-yield.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-RES-003.1` and `ENG-RES-003.2`.

### ENG-RES-003.1: Surplus-Leisure Scholarly Know-Why Synthesis
- **ID:** `ENG-RES-003.1`
- **Status:** `Active`
- **Milestone:** `M6`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-RES-003`
- **Requirement:** Scholars and leisure-supported specialists analyzing assayed materials and functioning workshop prototypes SHALL advance `TheoryDomainId` Know-Why mastery only when caloric surplus and paper/writing capacity permit freed intellectual labor.
- **Acceptance Criteria:** Under caloric famine, scholarly Know-Why stalls; under caloric surplus with functioning prototypes, `TheoryDomainId` mastery advances monotonically and emits published treatise records.

### ENG-RES-003.2: Know-Why Multiplication of Directed R&D Yield & Waste Reduction
- **ID:** `ENG-RES-003.2`
- **Status:** `Active`
- **Milestone:** `M6`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-RES-003`
- **Requirement:** Mature `TheoryDomainId` mastery (`>= 0.50`) SHALL multiply directed applied component experimentation speed (`>= 2.0x`) and cut experimental material trial waste (`>= 40%`).
- **Acceptance Criteria:** Controlled experiments comparing zero-theory colonies against mature-metallurgy/thermodynamics colonies confirm strictly faster applied refinement and lower trial batch consumption.

### ENG-RES-004: Four Institutional Modes of Innovation
- **ID:** `ENG-RES-004`
- **Status:** `Superseded`
- **Milestone:** `M6`
- **Verification Layer:** `Integration`
- **Lineage:** `Superseded-By: ENG-RES-004.1, ENG-RES-004.2`
- **Requirement:** The research model SHALL support emergent transition across workplace/leisure tinkering, state war prizes and military arsenals, private patent monopolies restricting rival adaptation until expiration or espionage, and institutionalized universities/corporate/state labs.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-RES-004.1` and `ENG-RES-004.2`.

### ENG-RES-004.1: Four Institutional Innovation Modes & Capability Gates
- **ID:** `ENG-RES-004.1`
- **Status:** `Active`
- **Milestone:** `M6`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-RES-004`
- **Requirement:** `sim-plug-research` SHALL support switching/operating `workplace_tinkering`, `state_arsenal_prize`, `patent_monopoly`, and `research_laboratory`, gating formal laboratories on sufficient population literacy and accumulated material/theoretical capital.
- **Acceptance Criteria:** Attempting `research_laboratory` mode under pre-literate baseline survival falls back cleanly or enforces low efficiency, whereas literate surplus colonies gain full laboratory multiplier benefits.

### ENG-RES-004.2: Private Patent Monopoly Exclusion Until Expiry
- **ID:** `ENG-RES-004.2`
- **Status:** `Active`
- **Milestone:** `M6`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-RES-004`
- **Requirement:** Under `patent_monopoly`, newly chartered private breakthroughs SHALL grant time-limited exclusive component usage (`patent_duration_years`) to the originating `ActorId`, blocking rival syndicates from licensed adoption until expiration ticks pass or reverse-engineering leaks occur.
- **Acceptance Criteria:** Issuing a patent on `forced_draft_tuyere` to Actor A rejects competing adoption by Actor B prior to `expiry_tick` and permits open adoption immediately after `expiry_tick`.

### ENG-RES-005: Four-Carrier Fragile Knowledge Persistence & Regression
- **ID:** `ENG-RES-005`
- **Status:** `Superseded`
- **Milestone:** `M6`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Superseded-By: ENG-RES-005.1, ENG-RES-005.2`
- **Requirement:** Every Know-How and Know-Why capability SHALL persist exclusively across practicing cohorts, living named specialists, physical prototypes/tooling, and written/printed archives; loss of all active carriers SHALL cause authentic loss of capability until rediscovered.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-RES-005.1` and `ENG-RES-005.2`.

### ENG-RES-005.1: Four-Carrier Physical Knowledge Accounting
- **ID:** `ENG-RES-005.1`
- **Status:** `Active`
- **Milestone:** `M6`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Parent: ENG-RES-005`
- **Requirement:** Every discovered component or theoretical treatise SHALL track non-abstract integer/entity counts across all four vulnerable physical carriers (`practicing_cohorts`, `living_specialists`, `workshop_prototypes`, `written_archives`).
- **Acceptance Criteria:** Emitting a settlement's knowledge snapshot exposes exact live counts across all four carrier channels and flags single-carrier fragility when three carrier types sit at zero.

### ENG-RES-005.2: Genuine Capability Regression Upon Total Physical Carrier Loss
- **ID:** `ENG-RES-005.2`
- **Status:** `Active`
- **Milestone:** `M6`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Parent: ENG-RES-005`
- **Requirement:** Whenever all four physical carriers for a capability reach zero locally and no printed copy remains reachable by active settlement links, `KnowledgeModifierPlugPort::process_prerequisites_met` SHALL revert to `false`, disabling dependent advanced recipes until rediscovered or retransmitted.
- **Acceptance Criteria:** Destroying physical workshop prototypes and losing master cohorts/specialists in an unarchived frontier settlement causes genuine capability regression (`process_prerequisites_met == false`), whereas preserving a written archive allows restoration.

---

## 13. Operational Warfare & Command Delegation (`ENG-WAR`)

### ENG-WAR-001: Prohibition of Direct Tactical Unit Micromanagement
- **ID:** `ENG-WAR-001`
- **Status:** `Superseded`
- **Milestone:** `M7`
- **Verification Layer:** `IPC-Contract`
- **Lineage:** `Superseded-By: ENG-WAR-001.1`
- **Requirement:** Player command interfaces SHALL NOT allow real-time hex-by-hex tactical micromanagement of individual combat units; all military direction flows through theater directives or operational battle plans subject to communication latency.
- **Acceptance Criteria:** Superseded by hex-free leaf requirement `ENG-WAR-001.1`.

### ENG-WAR-001.1: IPC & Command Rejection of Direct Tactical Unit Micromanagement
- **ID:** `ENG-WAR-001.1`
- **Status:** `Active`
- **Milestone:** `M7`
- **Verification Layer:** `IPC-Contract`
- **Lineage:** `Supersedes: ENG-WAR-001`
- **Requirement:** Both `ValidatedPlayerCommand` validation and `CommandPort` SHALL reject any command seeking direct instantaneous tactical step-by-step control of individual combat units (`moveFormationUnit`, `tacticalFireTarget`, or single-unit micro coordinate overrides), returning `ERR_TACTICAL_MICRO_FORBIDDEN`.
- **Acceptance Criteria:** Contract tests submitting direct unit tactical micro commands assert `ERR_TACTICAL_MICRO_FORBIDDEN` rejection across both Rust CLI/player-view and Electron IPC validators.

### ENG-WAR-002: "Churchill Mode" Theater Delegation & General Initiative
- **ID:** `ENG-WAR-002`
- **Status:** `Superseded`
- **Milestone:** `M7`
- **Verification Layer:** `Integration`
- **Lineage:** `Superseded-By: ENG-WAR-002.1, ENG-WAR-002.2`
- **Requirement:** Low-Attention strategic theater mandates SHALL assign overall goals and supply priorities while enabling field generals to execute tactical maneuvers, local withdrawals, and route pivots in real time without awaiting distant HQ approval.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-WAR-002.1` and `ENG-WAR-002.2`.

### ENG-WAR-002.1: Low-Attention Strategic Theater Directive Dispatch
- **ID:** `ENG-WAR-002.1`
- **Status:** `Active`
- **Milestone:** `M7`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-WAR-002`
- **Requirement:** Issuing a `TheaterDirective` ("Churchill Mode") SHALL consume low Personal Attention relative to operational planning while assigning theater commander, strategic objective coordinate, defensive/offensive posture, and logistics tonnage priority.
- **Acceptance Criteria:** Issuing a `TheaterDirective` costs strictly less Personal Attention than an equivalent `OperationalBattlePlan` and leaves subordinate formations governed by the theater general once the courier arrives.

### ENG-WAR-002.2: Autonomous Field General Maneuver Around Hazards & Flanks
- **ID:** `ENG-WAR-002.2`
- **Status:** `Active`
- **Milestone:** `M7`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-WAR-002`
- **Requirement:** Under an active `TheaterDirective`, field commanders SHALL autonomously pivot formation headings around washed-out river crossings, deep mud mires, or sudden enemy flank envelopments in real time without awaiting slow distant HQ confirmation.
- **Acceptance Criteria:** Exposed to a sudden flooded crossing and flank threat at high HQ courier latency, a competent general in `TheaterDirective` mode executes an autonomous flanking/withdrawal maneuver preventing encirclement losses.

### ENG-WAR-003: "Hitler Mode" Operational Battle Plans & Stale-Order Disasters
- **ID:** `ENG-WAR-003`
- **Status:** `Superseded`
- **Milestone:** `M7`
- **Verification Layer:** `Integration`
- **Lineage:** `Superseded-By: ENG-WAR-003.1, ENG-WAR-003.2`
- **Requirement:** High-Attention operational battle planning SHALL permit assigning division objectives, phase lines, and rigid doctrines (for example, *"No Retreat"*) while suppressing local officer initiative and exposing formations to stale-order entrapment under HQ distance latency.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-WAR-003.1` and `ENG-WAR-003.2`.

### ENG-WAR-003.1: High-Attention Operational Battle Plan & "No Retreat" Standing Orders
- **ID:** `ENG-WAR-003.1`
- **Status:** `Active`
- **Milestone:** `M7`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-WAR-003`
- **Requirement:** Issuing an `OperationalBattlePlan` ("Hitler Mode") SHALL invest high Personal Attention slices to specify explicit objective route coordinates, sequential phase lines, and optional `no_retreat: bool` doctrine overriding normal subordinate maneuver latitude.
- **Acceptance Criteria:** Issuing a `"No Retreat"` `OperationalBattlePlan` charges high Personal Attention capacity and locks compliant formations onto prescribed phase lines upon courier delivery.

### ENG-WAR-003.2: Stale-Order Encirclement Exposure vs. Commander Insubordination
- **ID:** `ENG-WAR-003.2`
- **Status:** `Active`
- **Milestone:** `M7`
- **Verification Layer:** `Integration`
- **Lineage:** `Parent: ENG-WAR-003`
- **Requirement:** When distant HQ latency delays updated withdrawal orders against flanking enemy pressure on an active `"No Retreat"` plan, obedient commanders SHALL hold stale coordinates into encirclement losses whereas high-initiative/low-obedience commanders may commit diegetic insubordination and withdraw independently, dispatching a delayed explanation report to HQ.
- **Acceptance Criteria:** Controlled scenario trials against identical enemy encirclement and distant HQ lag demonstrate severe encirclement casualties under obedient `"No Retreat"` commanders versus surviving withdrawal + insubordination dispatch under high-initiative generals.

### ENG-WAR-004: Hourly Weather-Coupled Combat Physics
- **ID:** `ENG-WAR-004`
- **Status:** `Superseded`
- **Milestone:** `M7`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Superseded-By: ENG-WAR-004.1, ENG-WAR-004.2`
- **Requirement:** Battle resolution SHALL couple directly into hourly cell weather states: deep mud bogging heavy wagons, cavalry, and artillery; flood-swollen rivers washing out crossings and splitting wings; wet weather fouling bowstrings and propellants; and low visibility shrinking recon and slowing battlefield couriers.
- **Acceptance Criteria:** Decomposed into leaf requirements `ENG-WAR-004.1` and `ENG-WAR-004.2`.

### ENG-WAR-004.1: Hourly Mud Miring & Flooded Crossing Wing Splitting
- **ID:** `ENG-WAR-004.1`
- **Status:** `Active`
- **Milestone:** `M7`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Parent: ENG-WAR-004`
- **Requirement:** Every hourly combat tick (`OperationalCombatPlugPort::step_hourly_combat`) SHALL sample local continuous weather mud and river flood states, reducing artillery/wagon formation march speed and combat readiness proportionally to `mud_index_01` and severing unbridged support across flooded river barriers (`flood_stage_01 > 0.65`).
- **Acceptance Criteria:** Identical military engagements simulated under dry surface versus high-mud and river-flood conditions produce measurably slower advance rates and lower supported combat effectiveness across flooded barriers.

### ENG-WAR-004.2: Precipitation Propellant Fouling & Visibility Recon Degradation
- **ID:** `ENG-WAR-004.2`
- **Status:** `Active`
- **Milestone:** `M7`
- **Verification Layer:** `Property-Sim`
- **Lineage:** `Parent: ENG-WAR-004`
- **Requirement:** Sustained precipitation (`precip_mm_hr > 2.0`) SHALL foul exposed propellants/bowstrings (mitigated only by active doctrine/shelter multipliers), while low `visibility_01` SHALL compress reconnaissance range and add stochastic delay to battlefield courier links.
- **Acceptance Criteria:** Comparative hourly combat tests verify lower ranged firepower multipliers and slower courier reporting during heavy rain and fog compared to clear dry daylight ticks.

---

## 14. Scenarios, Notifications & Headless CLI (`ENG-SCN`)

### ENG-SCN-001: Configurable Scenario Archetype Initialization
- **ID:** `ENG-SCN-001`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Integration`
- **Lineage:** `Origin`
- **Requirement:** Campaign initialization SHALL construct complete starting worlds exclusively from external scenario packs supporting Emergent Dawn, Expedition/Colony, and Governor/Successor starting profiles.
- **Acceptance Criteria:** Integration tests initialize distinct fixture packs for all three scenario archetypes through one unified loader path and verify expected starting settlements, cohorts, fog-of-war coverage, and initial ledger states.

### ENG-SCN-002: "The Unexplained Colony" Vertical-Slice Scenario Initialization
- **ID:** `ENG-SCN-002`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `CLI-Integration`
- **Lineage:** `Origin`
- **Requirement:** The default scenario pack (`mods/unexplained-colony/`) SHALL initialize an isolated single-settlement colony with unexplained external origin, multi-cohort founding population, 3 to 5 named subordinates, unexplored planetary surroundings, and finite degrading high-grade starting equipment.
- **Acceptance Criteria:** Loading `mods/unexplained-colony/` via `sim-cli` passes full schema and referential checks, verifies starting `player-view` shows unknown surrounding cells, and runs `8,760` ticks cleanly without crashes or unrecoverable NaN states.

### ENG-SCN-003: Per-Event-Class Four-Tier Notification & Auto-Pause Evaluation
- **ID:** `ENG-SCN-003`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Unit`
- **Lineage:** `Origin`
- **Requirement:** Notification routing SHALL evaluate every incoming `player-view` event against user-configurable per-event-class rules (`Log Only`, `Banner Alert`, `Dispatch Popup`, `Dispatch Popup + Auto-Pause`), halting the simulation clock immediately upon `Dispatch Popup + Auto-Pause` matches.
- **Acceptance Criteria:** Unit tests verify toggling an event class (for example, `overdue_expedition`) from `Log Only` to `Dispatch Popup + Auto-Pause` cleanly pauses the simulation clock on the exact arrival tick of the event at HQ.

### ENG-SCN-004: `sim-cli` Headless Multi-Decade Simulation Runner & Benchmark
- **ID:** `ENG-SCN-004`
- **Status:** `Active`
- **Milestone:** `M1`
- **Verification Layer:** `CLI-Integration`
- **Lineage:** `Origin`
- **Requirement:** `sim-cli` SHALL execute deterministic headless multi-year and multi-decade simulation runs from CLI arguments without Electron or GPU dependencies, reporting throughput (`ticks/sec`), population/economic summary statistics, and assertion checks.
- **Acceptance Criteria:** Running `sim-cli run --scenario mods/unexplained-colony --ticks 87600 --seed 42` completes deterministically within CI runtime budgets, exits `0`, and prints valid JSON summary metrics.

### ENG-SCN-005: Automated Workspace Traceability Auditor (`check-reqs`)
- **ID:** `ENG-SCN-005`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `CLI-Integration`
- **Lineage:** `Origin`
- **Requirement:** Workspace tooling SHALL provide an automated `check-reqs` command that parses `engine_requirements.md` and `ui_requirements.md`, validates structural schema and ID uniqueness, scans Rust and TypeScript test files for `@req` links, verifies every `Active` requirement up to the current milestone has at least one bound test, and rejects references to `Removed`, `Superseded`, or undefined IDs.
- **Acceptance Criteria:** Running `cargo xtask check-reqs --up-to-milestone M0` exits `0` when all `M0` `Active` requirements have linked tests and zero stale links exist, and exits non-zero on simulated orphan tests, duplicate IDs, or `Removed` ID references.

---

## 15. Retired & Superseded Requirements Registry

| ID | Former Title | Status | Lineage / Rationale | Date |
| :--- | :--- | :--- | :--- | :--- |
| `ENG-GEO-001` | Closed Goldberg Polyhedron Geodesic Sphere Topology | `Superseded` | Replaced by `ENG-GEO-001.1` (Option B continuous hex-free spherical terrain field) | 2026-11-15 |
| `ENG-CMD-006` | Multi-Layer Organizational Hierarchy Frictions | `Superseded` | Decomposed into `ENG-CMD-006.1`..`ENG-CMD-006.4` for M5 | 2026-11-15 |
| `ENG-CMD-007` | Five Governance & Coordination Models Shifting Friction Profiles | `Superseded` | Decomposed into `ENG-CMD-007.1`..`ENG-CMD-007.2` for M5 | 2026-11-15 |
| `ENG-POP-006` | Mortal Leader Aging & Generational Succession Continuity | `Superseded` | Decomposed into `ENG-POP-006.1`..`ENG-POP-006.2` for M7 | 2026-11-15 |
| `ENG-ECO-005` | Autonomous Non-State Institutional Economic Actors | `Superseded` | Decomposed into `ENG-ECO-005.1`..`ENG-ECO-005.4` for M5 | 2026-11-15 |
| `ENG-ECO-006` | Era-Universal Multi-Medium Economic Accounting | `Superseded` | Decomposed into `ENG-ECO-006.1`..`ENG-ECO-006.2` for M5 | 2026-11-15 |
| `ENG-ECO-007` | Cross-Era Economic Policy Levers | `Superseded` | Decomposed into `ENG-ECO-007.1`..`ENG-ECO-007.2` for M5 | 2026-11-15 |
| `ENG-LOG-005` | Shared Road & Port Throughput Congestion | `Superseded` | Decomposed into `ENG-LOG-005.1`..`ENG-LOG-005.2` for M5 | 2026-11-15 |
| `ENG-LOG-006` | Dynamic Carrier Chartering & Civilian Freight Crowding-Out | `Superseded` | Decomposed into `ENG-LOG-006.1`..`ENG-LOG-006.2` for M5 | 2026-11-15 |
| `ENG-RES-001` | Strict Prohibition of Abstract Science Points & Purchasable Tech Trees | `Superseded` | Decomposed into `ENG-RES-001.1`..`ENG-RES-001.2` for M6 | 2026-11-15 |
| `ENG-RES-002` | Know-How Empirical Process & Component Cross-Pollination | `Superseded` | Decomposed into `ENG-RES-002.1`..`ENG-RES-002.2` for M6 | 2026-11-15 |
| `ENG-RES-003` | Know-Why Theoretical Discovery Accelerating Directed Engineering | `Superseded` | Decomposed into `ENG-RES-003.1`..`ENG-RES-003.2` for M6 | 2026-11-15 |
| `ENG-RES-004` | Four Institutional Modes of Innovation | `Superseded` | Decomposed into `ENG-RES-004.1`..`ENG-RES-004.2` for M6 | 2026-11-15 |
| `ENG-RES-005` | Four-Carrier Fragile Knowledge Persistence & Regression | `Superseded` | Decomposed into `ENG-RES-005.1`..`ENG-RES-005.2` for M6 | 2026-11-15 |
| `ENG-WAR-001` | Prohibition of Direct Tactical Unit Micromanagement | `Superseded` | Replaced by `ENG-WAR-001.1` (Option B hex-free warfare IPC boundary) for M7 | 2026-11-15 |
| `ENG-WAR-002` | "Churchill Mode" Theater Delegation & General Initiative | `Superseded` | Decomposed into `ENG-WAR-002.1`..`ENG-WAR-002.2` for M7 | 2026-11-15 |
| `ENG-WAR-003` | "Hitler Mode" Operational Battle Plans & Stale-Order Disasters | `Superseded` | Decomposed into `ENG-WAR-003.1`..`ENG-WAR-003.2` for M7 | 2026-11-15 |
| `ENG-WAR-004` | Hourly Weather-Coupled Combat Physics | `Superseded` | Decomposed into `ENG-WAR-004.1`..`ENG-WAR-004.2` for M7 | 2026-11-15 |
