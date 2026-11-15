# UI, Globe Renderer, Windowing & Skinning Requirements Specification

**Document ID:** `UI-REQ-SPEC`  
**Scope:** Electron Shell, Three.js WebGPU/WebGL2 Geodesic Globe, Skinnable Window Manager, Data Views & Audio Pipeline  
**Companion Architecture:** [architecture.md](file:///home/roger/Projects/ng/requirements/architecture.md)  
**Companion Engine Specification:** [engine_requirements.md](file:///home/roger/Projects/ng/requirements/engine_requirements.md)  

---

## 1. Traceability, Lifecycle & Automated Test Binding Specification

### 1.1 Requirement Identifier Grammar

1. Top-level UI requirements use the immutable identifier format `UI-<DOMAIN>-<NNN>` where `<DOMAIN>` is a three-letter uppercase subsystem code (`LIC`, `SHL`, `GLB`, `MAP`, `WIN`, `DAT`, `HUD`, `SKN`, `AUD`) and `<NNN>` is a zero-padded three-digit integer starting at `001`.
2. Decomposed child requirements append dot-delimited sequence suffixes (`UI-<DOMAIN>-<NNN>.<M>`, supporting multi-level suffixes for deep decomposition).
3. Requirement identifiers are permanently immutable and SHALL NEVER be renumbered or reassigned after removal or decomposition.

### 1.2 Lifecycle Statuses

Every requirement block carries one of four normative lifecycle statuses:

1. `Active` — Normative leaf requirement verified automatically at or after its target `Milestone` (`M0` through `M7` in the current specification baseline).
2. `Draft` — High-level UI system-contract requirement reserved for decomposition into `Active` dot-suffixed leaf requirements before milestone implementation begins.
3. `Superseded` — Former requirement split into or replaced by newer identifiers listed in `Lineage: Superseded-By: <ID-list>`. Automated tests MUST update to target the replacement identifiers.
4. `Removed` — Permanently retired requirement archived in Section 11 (`Retired & Superseded Requirements Registry`) with `Lineage: Removed-Reason: <rationale>`. Any automated test still citing a `Removed` identifier fails validation.

### 1.3 Uniform Requirement Schema & Automated Test Binding

1. Every requirement block uses the identical seven-field schema (`ID`, `Status`, `Milestone`, `Verification Layer`, `Lineage`, `Requirement`, `Acceptance Criteria`) under a level-3 Markdown heading (`### <ID>: <Short Imperative Title>`).
2. TypeScript unit tests (`Vitest`) and Electron end-to-end tests (`Playwright` / WebGPU snapshot suites) bind to requirement identifiers via `// @req UI-<DOMAIN>-<NNN>` comments immediately above test blocks or `[UI-<DOMAIN>-<NNN>]` test title prefixes, verified by the workspace `check-reqs` auditor (`ENG-SCN-005`).

---

## 2. Licensing, Fonts & Steam Packaging Compliance (`UI-LIC`)

### UI-LIC-001: Permissive Royalty-Free NPM Dependency Allowlist
- **ID:** `UI-LIC-001`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `License-Audit`
- **Lineage:** `Origin`
- **Requirement:** Every production npm dependency bundled into `code/ui` SHALL carry a permissive, royalty-free SPDX license on the approved allowlist (`MIT`, `Apache-2.0`, `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `0BSD`, `CC0-1.0`) and SHALL NOT depend on paid commercial UI table/chart widgets, `GPL`, `AGPL`, static `LGPL`, or `SSPL` packages.
- **Acceptance Criteria:** Automated CI license checking across `package-lock.json` passes with zero unapproved, copyleft, or commercial-restricted dependencies.

### UI-LIC-002: Commercial-Safe Font & Baseline Skin Asset Licensing
- **ID:** `UI-LIC-002`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `License-Audit`
- **Lineage:** `Origin`
- **Requirement:** Every bundled font family and non-AI baseline graphical or audio asset shipped inside default skins (`skins/cartographer-parchment/` and companion assets) SHALL be covered by explicit commercial-permissive open licenses (`OFL-1.1`, `Apache-2.0`, `MIT`, or `CC0-1.0`) with attribution text included in release notices.
- **Acceptance Criteria:** Manifest validation verifies valid SPDX license entries and matching license text files for 100 percent of bundled fonts and non-AI baseline assets.

### UI-LIC-003: Steam Commercial Electron Packaging & `.asarUnpack` Rules
- **ID:** `UI-LIC-003`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `Build-Packaging`
- **Lineage:** `Origin`
- **Requirement:** Electron release packaging SHALL bundle Chromium and third-party notices (`LICENSES.chromium.html` and `THIRD_PARTY_NOTICES.txt`), configure `asarUnpack` so native `.node` addons, Rust libraries, and optional Steamworks dynamic libraries (`libsteam_api.so` on Linux and `steam_api64.dll` on Windows) reside unpacked outside `app.asar`, and run cleanly inside the Steam Linux Runtime and Windows `x86_64`.
- **Acceptance Criteria:** Executing the Linux and Windows package verification check confirms `LICENSES.chromium.html` presence, unpacked native libraries outside `app.asar`, and clean launch inside a sandboxed Steam Linux Runtime environment.

---

## 3. Electron Shell, WebGPU Context & IPC Bridge (`UI-SHL`)

### UI-SHL-001: Bundled Chromium Dawn WebGPU & WebGL2 Initialization
- **ID:** `UI-SHL-001`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `Globe-WebGPU`
- **Lineage:** `Origin`
- **Requirement:** `ui-shell` SHALL initialize Three.js rendering over Electron's bundled Chromium Dawn graphics stack (WebGPU primary with graceful WebGL2 fallback) consistently on Linux Wayland, Linux X11, and Windows without relying on system `WebKitGTK` libraries.
- **Acceptance Criteria:** Automated startup checks under headless/virtual display Linux and desktop environments confirm valid WebGPU or WebGL2 device creation, clean shader compilation, and zero `DMABUF` protocol crashes.

### UI-SHL-002: Typed Bidirectional IPC Bridge Sealed to `player-view`
- **ID:** `UI-SHL-002`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `IPC-Integration`
- **Lineage:** `Origin`
- **Requirement:** All communication between the Electron frontend and the Rust simulation backend SHALL flow across strictly typed preload IPC channels receiving exclusively `player-view` `DiegeticSnapshot` payloads and transmitting validated `PlayerCommand` requests.
- **Acceptance Criteria:** Contract integration tests verify TypeScript schema parity with Rust IPC types and reject malformed or unauthorized command payloads cleanly with typed error responses.

### UI-SHL-003: Electron Renderer Security Isolation
- **ID:** `UI-SHL-003`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** Electron `BrowserWindow` creation SHALL enforce `contextIsolation: true`, `nodeIntegration: false`, renderer sandboxing, and a strictly minimal `contextBridge` API inside the preload script.
- **Acceptance Criteria:** Playwright Electron inspection tests confirm `window.require` and `process` are `undefined` inside the renderer context while authorized `window.simBridge` IPC methods operate normally.

### UI-SHL-004: Structured Mod & Schema Diagnostic Error Screen
- **ID:** `UI-SHL-004`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** When backend mod loading or schema validation returns errors, `ui-shell` SHALL display a clean diagnostic window listing every offending file path, line and column location, field identifier, and remediation message while allowing safe mod reload or exit rather than crashing silently.
- **Acceptance Criteria:** Launching `ui-shell` against a corrupted fixture mod displays all structured validation diagnostics on screen within 2 seconds and allows recovery to a valid mod pack.

### UI-SHL-005: Deterministic Mock-Snapshot Playback Mode
- **ID:** `UI-SHL-005`
- **Status:** `Active`
- **Milestone:** `M0`
- **Verification Layer:** `Unit-Vitest`
- **Lineage:** `Origin`
- **Requirement:** `ui-shell` SHALL support launching against frozen `player-view` JSON snapshot fixtures in offline test mode so every globe shader, window interaction, table, chart, and skin test runs deterministically without executing a live Rust simulation.
- **Acceptance Criteria:** Vitest and Playwright UI suites render and verify complex multi-settlement colony fixtures directly from `tests/fixtures/*.json` in under 10 seconds with zero Rust daemon processes spawned.

---

## 4. Geodesic 3D Globe & Zoom-Blended Cartography (`UI-GLB`)

### UI-GLB-001: Seamless Goldberg Polyhedron Geodesic Mesh Rendering
- **ID:** `UI-GLB-001`
- **Status:** `Superseded`
- **Milestone:** `M2`
- **Verification Layer:** `Globe-WebGPU`
- **Lineage:** `Superseded-By: UI-GLB-001.1`
- **Requirement:** The globe renderer SHALL construct and render a watertight spherical Goldberg polyhedron mesh from `player-view` topology buffers (hexagonal cells plus 12 pentagonal cells) free of polar pinching, UV discontinuities, or edge cracks.
- **Acceptance Criteria:** Superseded by hex-free welded spherical icosphere rendering (`UI-GLB-001.1`).

### UI-GLB-001.1: Seamless Hex-Free Welded Spherical Terrain & Barycentric Relief
- **ID:** `UI-GLB-001.1`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Globe-WebGPU`
- **Lineage:** `Supersedes: UI-GLB-001`
- **Requirement:** The globe renderer SHALL construct and render a watertight welded subdivided icosphere triangle mesh (`10 * m^2 + 2` sample vertices, `20 * m^2` spherical triangles, Euler characteristic `2`) free of visible hex/pentagon grid borders, polar pinching, UV discontinuities, or edge cracks, interpolating coastlines, elevation relief, wetness, and epistemic horizons smoothly across spherical triangles with circular reticle indicators.
- **Acceptance Criteria:** GPU mesh integrity tests verify closed manifold indices (`unmatchedHalfEdges == 0`, `degenerateTriangles == 0`, `eulerCheckDual == 2`), zero hex-edge border lines in GLSL shaders, and crisp polar continuity across north/south poles and the antimeridian.

### UI-GLB-002: Orbital Camera Pan, Tilt, Compass Reset & Fly-To Navigation
- **ID:** `UI-GLB-002`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** The camera controller SHALL provide smooth quaternion-based orbital drag rotation, pitch/tilt adjustment, true-north compass orientation reset, animated smooth fly-to centering on any cell or hyperlinked entity coordinate, and continuous logarithmic altitude zoom.
- **Acceptance Criteria:** Playwright input automation driving mouse drags, wheel zoom, north-reset shortcut, and hyperlink fly-to commands moves the camera smoothly through expected spherical coordinates without gimbal lock at the poles.

### UI-GLB-003: Physical 3D Terrain Mode at Low & Medium Camera Altitude
- **ID:** `UI-GLB-003`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Globe-WebGPU`
- **Lineage:** `Origin`
- **Requirement:** At low and medium camera altitudes, globe shaders SHALL render extruded elevation relief, ocean bathymetry and waves, carved river splines, terrain biome PBR textures, settlement markers, and road/canal infrastructure geometry for surveyed regions.
- **Acceptance Criteria:** Visual golden-image checks at low altitude confirm visible shadow relief across mountain chains, distinct river channels, and accurate placement of settlement and road spline geometry.

### UI-GLB-004: Continuous Camera-Altitude Cross-Fade to Hand-Drawn Parchment Map
- **ID:** `UI-GLB-004`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Skin-Snapshot`
- **Lineage:** `Origin`
- **Requirement:** As camera altitude increases toward planetary scale, custom shaders SHALL smoothly cross-fade physical 3D relief into a stylized hand-drawn cartographic parchment map displaying inked topographic contours, hatched shading, sea-chart compass lines, and paper grain textures.
- **Acceptance Criteria:** Golden-image captures sampled at low (`1.0x`), transition (`0.5` blend), and high orbital (`0.0` relief blend) altitudes verify smooth visual interpolation without jarring pop-in cuts.

### UI-GLB-005: Smooth Asynchronous Frame Rate Under High-Speed Simulation
- **ID:** `UI-GLB-005`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Globe-WebGPU`
- **Lineage:** `Origin`
- **Requirement:** Globe rendering and window interaction loops SHALL run asynchronously from Rust tick stepping, maintaining responsive 60 FPS input and camera motion even when the simulation clock runs at multi-day-per-second acceleration.
- **Acceptance Criteria:** Automated performance tests running continuous high-speed tick ingestion verify renderer frame times stay under 16.7 milliseconds at 95th percentile during active pan/zoom maneuvers.

---

## 5. Diegetic Map Overlays, Fog-of-War & Weather Visuals (`UI-MAP`)

### UI-MAP-001: Three-State Diegetic Exploration & Confidence Shading
- **ID:** `UI-MAP-001`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Skin-Snapshot`
- **Lineage:** `Origin`
- **Requirement:** The globe SHALL visually distinguish three epistemic terrain tiers: unexplored cells rendered as blank parchment or frontier cloud/vignette, unverified rumored terrain rendered as tentative sketch outlines, and surveyed terrain stamped with visible survey-age dimming and confidence indicators.
- **Acceptance Criteria:** Screenshot assertions over a fixture containing unexplored, rumored, 2-year-old survey, and fresh survey cells verify visually distinct shader masks and tooltip metadata matching each tier.

### UI-MAP-002: Live Immediate-HQ Observation Halo Clarity
- **ID:** `UI-MAP-002`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `IPC-Integration`
- **Lineage:** `Origin`
- **Requirement:** Cells inside the player's current physical HQ immediate-sight radius SHALL render with crisp full-clarity indicators distinguishing live real-time observation from dated remote reports.
- **Acceptance Criteria:** Relocating HQ in test fixtures shifts the live clarity halo immediately to the active HQ coordinate while dimming the prior HQ zone back into timestamped survey shading.

### UI-MAP-003: Ghosted Dead-Reckoning Markers & Overdue Expedition Alerts
- **ID:** `UI-MAP-003`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** Out-of-sight expeditions and freight convoys SHALL render on the map as translucent dashed-outline ghost markers tracking `player-view` dead-reckoning projections, snapping visibly upon confirmed sightings and switching to distinct overdue warning glyphs when expected ETA grace windows expire.
- **Acceptance Criteria:** Playwright tests stepping mock snapshots from `en_route_projected` to `overdue` to `confirmed_sighting` assert matching CSS/sprite states and click-inspector coordinate updates.

### UI-MAP-004: Hourly Physical Weather Surface & Atmospheric Overlays
- **ID:** `UI-MAP-004`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Globe-WebGPU`
- **Lineage:** `Origin`
- **Requirement:** Within observed HQ radius and reported weather corridors, the map SHALL visualize hourly precipitation rain/snow sheets, darkened wet soil mud saturation, flooded river width expansion, winter snowpack and river ice, and animated surface wind streamlines.
- **Acceptance Criteria:** Rendering a storm-flood snapshot visibly alters cell wetness, expands river polygon width on flooded cells, and draws animated precipitation/wind vectors strictly within observed cells.

### UI-MAP-005: Occlusion-Aware Spherical Picking & Hover Inspection
- **ID:** `UI-MAP-005`
- **Status:** `Superseded`
- **Milestone:** `M2`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Superseded-By: UI-MAP-005.1`
- **Requirement:** Raycast picking on the globe SHALL accurately resolve hovered and clicked Goldberg cells, settlements, road/river edges, and convoy markers while ignoring pointer events landing over any open HUD bar or floating window.
- **Acceptance Criteria:** Superseded by continuous spherical coordinate picking (`UI-MAP-005.1`).

### UI-MAP-005.1: Occlusion-Aware Continuous Spherical Coordinate, Region, Spline & Marker Picking
- **ID:** `UI-MAP-005.1`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Supersedes: UI-MAP-005`
- **Requirement:** Raycast picking on the globe SHALL resolve continuous 3D unit-sphere coordinates (`unitXyz`, `latDeg`, `lonDeg`), nearest terrain sample region (`cellId`), settlements, road/river spline segments, and convoy/formation markers while ignoring pointer events landing over any open HUD bar, modal dialog, or floating window.
- **Acceptance Criteria:** Window and picking tests verify clicking exposed globe coordinates returns exact continuous `(latDeg, lonDeg)`, `unitXyz`, and `cellId`, whereas clicking the identical viewport coordinates beneath a floating window interacts solely with the window.

### UI-MAP-006: Reported Trade-Flow, Freight Congestion & Price-Spread Overlays
- **ID:** `UI-MAP-006`
- **Status:** `Superseded`
- **Milestone:** `M5`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Superseded-By: UI-MAP-006.1, UI-MAP-006.2`
- **Requirement:** Selectable cartographic map modes SHALL visualize inter-settlement trade-flow ribbons, shared road/port congestion bottlenecks, and timestamped regional commodity price disparities derived strictly from arrived market ledgers.
- **Acceptance Criteria:** Decomposed into leaf requirements `UI-MAP-006.1` and `UI-MAP-006.2`.

### UI-MAP-006.1: Inter-Settlement Trade Ribbons & Timestamped Regional Price Spreads
- **ID:** `UI-MAP-006.1`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Parent: UI-MAP-006`
- **Requirement:** Selecting the `trade` map lens (`TradeAndCongestionOverlayLayer`) SHALL display directional inter-settlement merchant/charter trade ribbons and regional price-spread summaries annotated with explicit arrived-ledger observation timestamps.
- **Acceptance Criteria:** Switching the overlay toolbar to `Trade` updates rendered trade ribbon geometries and legend summaries exclusively from snapshot `regionalPrices` and `trackedConvoys` without touching hidden ground truth.

### UI-MAP-006.2: Shared Corridor & Port Congestion Bottleneck Highlights
- **ID:** `UI-MAP-006.2`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Unit-Vitest`
- **Lineage:** `Parent: UI-MAP-006`
- **Requirement:** The `trade` overlay lens SHALL highlight shared transport corridors exhibiting elevated axle-load congestion multipliers or road surface wear (`corridorCongestion`), exposing bottleneck utilization counts in the legend.
- **Acceptance Criteria:** Snapshot tests with congested corridors verify accurate bottleneck counts and wear severity summary metrics inside `TradeAndCongestionOverlayLayer`.

### UI-MAP-007: Operational Warfare Fronts, Objectives & Phase-Line Overlay
- **ID:** `UI-MAP-007`
- **Status:** `Superseded`
- **Milestone:** `M7`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Superseded-By: UI-MAP-007.1, UI-MAP-007.2`
- **Requirement:** Military map mode SHALL display theater boundaries, division/corps operational objective arrows, phase lines, active *"No Retreat"* posture badges, and timestamped last-reported unit positions reflecting real communication lag.
- **Acceptance Criteria:** Decomposed into leaf requirements `UI-MAP-007.1` and `UI-MAP-007.2`.

### UI-MAP-007.1: Theater Objectives, Phase Lines & "No Retreat" Posture Markers
- **ID:** `UI-MAP-007.1`
- **Status:** `Active`
- **Milestone:** `M7`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Parent: UI-MAP-007`
- **Requirement:** Selecting the `warfare` map lens (`OperationalWarfareOverlayLayer`) SHALL display theater directive target vectors alongside Hitler-mode operational battle-plan phase lines and explicit *"No Retreat"* standing-order indicators.
- **Acceptance Criteria:** Switching to the `Warfare` lens on a snapshot containing both Churchill theater directives and `"No Retreat"` operational battle plans renders distinct visual indicators and legend tallies for each command mode.

### UI-MAP-007.2: Courier-Lagged Timestamped Formation Markers
- **ID:** `UI-MAP-007.2`
- **Status:** `Active`
- **Milestone:** `M7`
- **Verification Layer:** `Unit-Vitest`
- **Lineage:** `Parent: UI-MAP-007`
- **Requirement:** Operational formation markers rendered on the globe SHALL position each unit strictly at its last-reported coordinate stamped with courier observation age (`reportedTick`), visually distinguishing fresh HQ sightings from stale distant reports.
- **Acceptance Criteria:** Snapshot tests verify stale distant formations expose delayed `reportedTick` metadata and zero access to unarrived engine coordinates.

---

## 6. Desktop-Class Skinnable Window Manager (`UI-WIN`)

### UI-WIN-001: Viewport-Clamped Title-Bar Drag Repositioning
- **ID:** `UI-WIN-001`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** Every floating in-game window SHALL support smooth mouse drag repositioning via its title bar while clamping coordinates so the title bar and window grab handles can never be dragged completely outside the visible viewport.
- **Acceptance Criteria:** Playwright drag actions moving windows toward all four viewport borders assert exact tracked displacement and verify title-bar visibility remains strictly inside screen bounds.

### UI-WIN-002: Eight-Direction Edge & Corner Mouse Resizing With Bounds
- **ID:** `UI-WIN-002`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** Floating windows SHALL support pointer resizing along all four edges and all four corners with directional cursor feedback, strictly enforcing per-window `minWidth`, `minHeight`, `maxWidth`, and `maxHeight` constraints.
- **Acceptance Criteria:** Playwright tests dragging north, east, south, west, and corner handles confirm expected bounding-box growth and verify sizes clamp cleanly at declared minimum and maximum limits.

### UI-WIN-003: Minimize-to-Header Collapse & State-Preserving Restore
- **ID:** `UI-WIN-003`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** Every window SHALL provide a minimize/collapse toggle shrinking the window body down to its compact title header bar (or dock chip) and restoring it with one click while preserving table scroll offsets, active filters, and form inputs.
- **Acceptance Criteria:** Scrolling a virtualized ledger table inside Window A, collapsing it to header-only height, and restoring it confirms zero loss of scroll position or active filter query.

### UI-WIN-004: Window Close, Hotkeys & Pinned Multi-Instance Comparison
- **ID:** `UI-WIN-004`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** Windows SHALL support close buttons, configurable toggle hotkeys, and a Pin toggle allowing players to either reuse a single focused inspector window or pin multiple settlement/survey windows open simultaneously for side-by-side comparison.
- **Acceptance Criteria:** Playwright tests verify clicking two settlements with unpinned inspector reuses one window, whereas pinning Settlement A opens Settlement B in a second distinct window.

### UI-WIN-005: Deterministic Z-Order Focus Raising & Pointer Isolation
- **ID:** `UI-WIN-005`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** Clicking or scrolling anywhere inside a window SHALL immediately raise that window to the top of the z-order stack and strictly capture pointer, click, and mouse-wheel events so inputs never leak onto underlying windows or the 3D globe canvas.
- **Acceptance Criteria:** Stacking two overlapping windows above the globe and firing clicks and wheel-scroll events confirms top-window focus order switches deterministically while globe camera altitude remains unchanged.

### UI-WIN-006: Edge Snapping & Tab-Group Docking
- **ID:** `UI-WIN-006`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** The window manager SHALL support optional magnetic edge snapping between adjacent windows and viewport borders as well as dragging one window onto another's title bar to dock them into a tabbed multi-panel window and drag tabs back out.
- **Acceptance Criteria:** Playwright tests verify dragging Window B onto Window A merges both into a tabbed frame with switchable tabs and dragging Tab B outward detaches it back into an independent floating window.

### UI-WIN-007: Serializable Window Layout Persistence & User Presets
- **ID:** `UI-WIN-007`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Unit-Vitest`
- **Lineage:** `Origin`
- **Requirement:** Open window IDs, pixel/relative positions, sizes, collapsed states, z-indices, and tab docks SHALL serialize cleanly to JSON across saves/restarts and support user-named layout presets (war-room layout, economic audit layout, minimalist exploration layout) that adapt cleanly across screen resolutions.
- **Acceptance Criteria:** Round-trip layout tests restore identical window geometries across reload and proportionally clamp window bounds when re-hydrated at a smaller screen resolution.

---

## 7. High-Density Ledgers, Charts, Org Trees & Dispatches (`UI-DAT`)

### UI-DAT-001: Row-Virtualized 10,000+ Row Sortable & Filterable Ledgers
- **ID:** `UI-DAT-001`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** Inventory, stockpile, carrier, and cohort ledger views SHALL render via pure DOM row virtualization capable of smoothly scrolling, sorting, and filtering 10,000+ rows without paid grid libraries or frame drops.
- **Acceptance Criteria:** Loading a 15,000-row ledger fixture maintains DOM node count strictly proportional to visible viewport height (under 150 row nodes), sorts numerically/lexically in under 50 milliseconds, and scrolls without dropped animation frames.

### UI-DAT-002: Multi-Series Time-Series Charts With Permissive Rendering
- **ID:** `UI-DAT-002`
- **Status:** `Active`
- **Milestone:** `M3`
- **Verification Layer:** `Unit-Vitest`
- **Lineage:** `Origin`
- **Requirement:** Economic, demographic, stockpile, and weather history windows SHALL plot multi-series time-series data using permissive-licensed SVG/Canvas charting supporting zoomable tick spans, crosshairs, and data-point hover tooltips.
- **Acceptance Criteria:** Vitest rendering tests verify accurate axis scaling, legible crosshair tooltip outputs, and non-overlapping legends under arbitrary window resize events.

### UI-DAT-003: Interactive Delegation & Attention Org Tree
- **ID:** `UI-DAT-003`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** A dedicated Command & Delegation Org Tree window SHALL visualize the player HQ, appointed subordinates, cascading sub-mandates, Attention Capacity utilization meters, and Oversight Dilution warnings, allowing expansion/collapse and direct click-through inspection.
- **Acceptance Criteria:** Increasing direct reports above capacity in a test snapshot immediately displays Attention overload warning indicators along the affected leader node and highlights diluted mandates.

### UI-DAT-004: Rich Hyperlinked Dispatch & Survey Reader
- **ID:** `UI-DAT-004`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** Dispatch, ledger, and survey windows SHALL render inline hyperlinked entity chips for Named Individuals, settlements, expeditions, commodities, and map coordinates so clicking any link focuses or opens its inspector window or centers the globe camera.
- **Acceptance Criteria:** Clicking a settlement chip and a subordinate name chip inside an arrived dispatch respectively pans the globe to the settlement and opens the leader's profile window.

### UI-DAT-005: Explicit Epistemic Provenance Badges on Every Reported Metric
- **ID:** `UI-DAT-005`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Unit-Vitest`
- **Lineage:** `Origin`
- **Requirement:** Every non-HQ numeric figure displayed across tables, tooltips, and summaries SHALL carry a diegetic provenance indicator showing last measurement tick age, authoring official, and survey/ledger error bounds rather than disguising estimates as exact truth.
- **Acceptance Criteria:** Snapshot tests assert that remote ore estimates render confidence ranges (`min` to `max`) and age badges (`Reported 14 days ago by Clerk H. Vance`) while live HQ local counts mark `Direct Observation`.

### UI-DAT-006: Non-State Institution & Shared Logistics Inspector Windows
- **ID:** `UI-DAT-006`
- **Status:** `Superseded`
- **Milestone:** `M5`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Superseded-By: UI-DAT-006.1, UI-DAT-006.2`
- **Requirement:** Institutional inspector windows SHALL display known estates, merchant houses, craft guilds, and syndicates alongside reported freight contracts, regional price boards, and shared carrier bottlenecks.
- **Acceptance Criteria:** Decomposed into leaf requirements `UI-DAT-006.1` and `UI-DAT-006.2`.

### UI-DAT-006.1: Non-State Actors, Regional Spot Prices & Congestion Inspector Tabs
- **ID:** `UI-DAT-006.1`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Parent: UI-DAT-006`
- **Requirement:** `NonStateEconomyInspectorView` (`institutions_inspector`) SHALL render accessible inner tabs and `<details>` disclosures clearly separating state warehouses from reported Households, Landed Estates, Merchant Houses, and Syndicates, plus regional spot price spreads, charter rates, and shared corridor bottlenecks.
- **Acceptance Criteria:** DOM tests verify accurate separation of actor classes, `<summary>` disclosures without nested headings, accessible `tablist`/`tabpanel` (`hidden="until-found"`), and rendered provenance timestamps.

### UI-DAT-006.2: Economic Policy & Governance Reform Controls
- **ID:** `UI-DAT-006.2`
- **Status:** `Active`
- **Milestone:** `M5`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Parent: UI-DAT-006`
- **Requirement:** The institutional inspector SHALL expose policy controls enabling the player to switch governance coordination models, enact cross-era economic policy levers, and charter private carriers via validated IPC commands displaying active reform inertia feedback.
- **Acceptance Criteria:** Submitting governance reform, policy enactment, or carrier chartering actions from the inspector dispatches exact `ValidatedPlayerCommand` payloads (`switchGovernanceModel`, `enactEconomicPolicy`, `charterPrivateCarriers`).

### UI-DAT-007: Diegetic Archive, Workshop & Fragile Knowledge Windows
- **ID:** `UI-DAT-007`
- **Status:** `Superseded`
- **Milestone:** `M6`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Superseded-By: UI-DAT-007.1, UI-DAT-007.2`
- **Requirement:** Knowledge and assay windows SHALL display empirically tested material properties, proven physical components (Know-How), published theoretical treatises (Know-Why), active patents, and fragility alerts when critical masters or archives remain un-replicated.
- **Acceptance Criteria:** Decomposed into leaf requirements `UI-DAT-007.1` and `UI-DAT-007.2`.

### UI-DAT-007.1: Assayed Materials, Demonstrated Know-How & Know-Why Archive View
- **ID:** `UI-DAT-007.1`
- **Status:** `Active`
- **Milestone:** `M6`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Parent: UI-DAT-007`
- **Requirement:** `DiegeticKnowledgeArchiveView` (`knowledge_archive`) SHALL present assayed procedural materials, proven Know-How components, mature/in-progress Know-Why treatises, active patent monopolies, and innovation mode controls without ever revealing unresearched future tech-tree previews.
- **Acceptance Criteria:** Automated UI assertions inspect the rendered DOM across all knowledge tabs and prove zero unresearched future nodes or global science-point pools appear anywhere in the window.

### UI-DAT-007.2: Four-Carrier Fragility & Single-Carrier Extinction Warnings
- **ID:** `UI-DAT-007.2`
- **Status:** `Active`
- **Milestone:** `M6`
- **Verification Layer:** `Unit-Vitest`
- **Lineage:** `Parent: UI-DAT-007`
- **Requirement:** Every demonstrated capability inside `knowledge_archive` SHALL display live counts across all four physical carriers (cohorts, specialists, prototypes, written archives) and highlight extinction-risk badges whenever a capability survives on zero or one physical carrier type.
- **Acceptance Criteria:** Rendering a fixture containing both a multi-carrier replicated technique and an unarchived single-specialist technique displays explicit fragility warning badges solely on the endangered entry.

---

## 8. Clock Bar, Mandates, HQ Relocation & Notification Matrix (`UI-HUD`)

### UI-HUD-001: Simulation Clock Control Bar & Scenario Calendar Display
- **ID:** `UI-HUD-001`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** A persistent top/edge HUD bar SHALL display the formatted scenario date/hour, HQ location link, Personal Attention utilization summary, pause/resume toggle, `+1 Hour` single-step button, discrete speed controls, and clear indicator explaining when the engine auto-paused.
- **Acceptance Criteria:** Playwright tests verify clicking `+1 Hour`, pause, and speed tier buttons emits exact IPC clock commands and renders the updated simulated hour and auto-pause cause banner.

### UI-HUD-002: Strategic Mandate Authoring & Attention-Load Preview
- **ID:** `UI-HUD-002`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** The Mandate Issuance window SHALL allow selecting an outcome goal, target scope, priority, and responsible subordinate or procurement office while previewing both Attention Capacity impact and estimated outbound dispatch travel latency prior to dispatch.
- **Acceptance Criteria:** Selecting a distant overseer already at capacity previews both the Attention overload warning and outbound courier ETA before submitting the `IssueMandate` IPC command.

### UI-HUD-003: Bottleneck & Competing-Proposal Arbitration Window
- **ID:** `UI-HUD-003`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** Escalated subordinate proposals SHALL open in a structured Arbitration window comparing each candidate option's material inputs, labor shifts, institutional trade-offs, and estimated dispatch turnaround time, emitting a binding resolution command upon player selection.
- **Acceptance Criteria:** Rendering an escalated mine-flooding proposal presents distinct selectable resolution cards and sends the chosen option ID with clean window closure upon confirmation.

### UI-HUD-004: Personal Attention Zoom-In Allocation Panel
- **ID:** `UI-HUD-004`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** A Personal Attention panel SHALL show total player Attention slices and let the player toggle direct oversight onto or off specific projects, expeditions, or ministries with live feedback showing oversight dilution shifts elsewhere.
- **Acceptance Criteria:** Toggling zoom-in Attention onto a second active project dynamically updates Attention bar allocations and surfaces warning indicators on newly under-supervised branches.

### UI-HUD-005: Physical HQ Relocation Planner & Latency Contour Preview
- **ID:** `UI-HUD-005`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** The HQ Relocation interface SHALL let the player select a destination cell or settlement and preview expected travel route, duration in hours/days, and comparative before-versus-after dispatch latency to existing hubs before confirming travel.
- **Acceptance Criteria:** Selecting a frontier outpost inside the HQ planner renders projected route highlights, travel duration, and side-by-side latency deltas prior to dispatching the `RelocateHq` command.

### UI-HUD-006: Per-Event Notification & Auto-Pause Configuration Matrix
- **ID:** `UI-HUD-006`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** A settings window SHALL expose a complete matrix of all scenario event classes allowing independent assignment of `Log Only`, `Banner Alert`, `Dispatch Popup`, or `Dispatch Popup + Auto-Pause` delivery behavior, persisted immediately via `sim-data`/user settings.
- **Acceptance Criteria:** Changing an event class toggle inside the matrix persists across application restarts and updates live IPC event-delivery behavior without requiring scenario restart.

### UI-HUD-007: Operational Warfare Delegation & Succession Transition Windows
- **ID:** `UI-HUD-007`
- **Status:** `Superseded`
- **Milestone:** `M7`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Superseded-By: UI-HUD-007.1, UI-HUD-007.2`
- **Requirement:** Command UI shall provide "Churchill vs. Hitler" theater/battle-plan posture assignment controls and a Generational Succession summary screen presenting the incoming successor's inherited archives, institutions, debts, and official roster upon predecessor death.
- **Acceptance Criteria:** Decomposed into leaf requirements `UI-HUD-007.1` and `UI-HUD-007.2`.

### UI-HUD-007.1: Churchill vs. Hitler Operational Warfare Command Window
- **ID:** `UI-HUD-007.1`
- **Status:** `Active`
- **Milestone:** `M7`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Parent: UI-HUD-007`
- **Requirement:** `WarfareCommandWindow` (`warfare_command`) SHALL switch cleanly between low-Attention `TheaterDirective` ("Churchill Mode") controls and high-Attention `OperationalBattlePlan` ("Hitler Mode") phase-line + *"No Retreat"* orders, displaying Attention impact, courier delay from current HQ, and formation status timestamps while disallowing direct unit tactical micro.
- **Acceptance Criteria:** Issuing directives in both modes dispatches valid `issueTheaterDirective` and `issueOperationalBattlePlan` commands, previews differing Attention loads, and confirms zero unit micro buttons exist.

### UI-HUD-007.2: Native Modal Generational Succession Transition Screen
- **ID:** `UI-HUD-007.2`
- **Status:** `Active`
- **Milestone:** `M7`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Parent: UI-HUD-007`
- **Requirement:** Upon ruler demise/deposition signaled by `pendingTransition`, HUD SHALL open a modal summary built on a native `<dialog>` element (`showModal()`, labelled via `aria-labelledby` without redundant `role="dialog"` or `aria-modal` attributes) displaying predecessor cause of death, incoming successor profile, inherited physical archive/ledger/debt continuity, count of lost unrecorded head observations, and a sealing confirmation button dispatching `acknowledgeSuccession`.
- **Acceptance Criteria:** Injecting a snapshot containing `pendingTransition` opens the native `<dialog>` in modal state, renders inherited map/ledger counts and unrecorded loss notice, and sends `acknowledgeSuccession` while cleanly resetting `returnValue` on close.

---

## 9. External CSS Skinning, 9-Slice Chrome & AI Visual Assets (`UI-SKN`)

### UI-SKN-001: 100% External CSS Stylesheet & Token Styling
- **ID:** `UI-SKN-001`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Static-Analysis & Skin-Snapshot`
- **Lineage:** `Origin`
- **Requirement:** All colors, fonts, spacing, borders, corner ornaments, scrollbars, and animations across `ui-shell` SHALL derive exclusively from external skin `.css` stylesheets and CSS custom properties; TypeScript component logic SHALL NOT hardcode colors, pixel border styling, or font names.
- **Acceptance Criteria:** Static ESLint/grep checks confirm zero inline color literals or hardcoded font definitions inside `code/ui/src/`, and swapping skin CSS completely transforms typography and palette.

### UI-SKN-002: Distortion-Free CSS `border-image` 9-Slice Window Chrome
- **ID:** `UI-SKN-002`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Skin-Snapshot`
- **Lineage:** `Origin`
- **Requirement:** Window frames, title bars, buttons, wax seals, and panel dividers SHALL scale cleanly across arbitrary aspect ratios via CSS `border-image` 9-slice definitions loaded from external AI-normalized sprite sheets without stretching corners.
- **Acceptance Criteria:** Visual regression snapshots comparing a compact `260x180` popup against an expanded `1100x700` ledger confirm pixel-sharp corner ornaments and clean tiled/stretched edges.

### UI-SKN-003: Live Runtime Skin Switching & Development CSS Hot-Reload
- **ID:** `UI-SKN-003`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** Selecting an installed external skin pack at runtime (or modifying active skin CSS in development mode) SHALL reapply styles, 9-slice textures, globe palettes, and audio manifests live without closing open windows, resetting camera coordinates, or interrupting simulation ticks.
- **Acceptance Criteria:** Playwright tests open two windows, switch from `cartographer-parchment` to an alternate test skin pack, and verify computed CSS tokens update immediately while open window positions and tick index remain unchanged.

### UI-SKN-004: Skin-Driven Cartographic Globe Shader Palettes & Textures
- **ID:** `UI-SKN-004`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Globe-WebGPU`
- **Lineage:** `Origin`
- **Requirement:** The Three.js globe shaders SHALL read color gradients, contour ink values, ocean tint tables, seamless parchment paper overlays, compass motifs, and settlement icons directly from the active external skin manifest rather than embedding hard-coded shader palettes.
- **Acceptance Criteria:** Loading an alternate skin pack targeting high-contrast or industrial tones visibly updates rendered globe water, contour lines, and paper grain textures on the next frame.

### UI-SKN-005: Manifest-Driven AI Portraits, Icons & Dispatch Plates With Fallbacks
- **ID:** `UI-SKN-005`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Unit-Vitest`
- **Lineage:** `Origin`
- **Requirement:** UI components SHALL resolve character portraits (matching age bracket, vocation, era dress, and health), commodity/process icons, and dispatch header plates dynamically through external AI asset manifests, falling back cleanly to verified silhouette placeholders if an optional mod omits an icon.
- **Acceptance Criteria:** Unit tests resolving complete manifests and partial mod manifests confirm zero broken `<img>` elements or uncaught asset exceptions when optional icon entries are missing.

---

## 10. Altitude-Reactive Soundscapes, Music & Skin Foley (`UI-AUD`)

### UI-AUD-001: External Audio Manifest Loading Without Hardcoded Paths
- **ID:** `UI-AUD-001`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Audio-Manifest`
- **Lineage:** `Origin`
- **Requirement:** Music stems, environmental ambient loops, and UI foley clips SHALL load strictly from external skin and scenario audio manifest files validated at startup, containing zero hardcoded audio filenames in TypeScript.
- **Acceptance Criteria:** Static checks confirm zero direct `.ogg`/`.wav`/`.mp3` string literals in `code/ui/src/`, and manifest loader tests validate missing-file resilience and channel routing.

### UI-AUD-002: Camera-Altitude Environmental Audio Cross-Fade
- **ID:** `UI-AUD-002`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Unit-Vitest`
- **Lineage:** `Origin`
- **Requirement:** The Web Audio mixer SHALL smoothly cross-fade ambient audio stems based on camera altitude and local observed weather: surface weather (wind, rainfall, thunder) and settlement workshop bustle when zoomed in, transitioning seamlessly to study/cartographic room ambiance (clock tick, hearth crackle, faint paper/quill) at orbital map scale.
- **Acceptance Criteria:** Automated audio-gain curve tests stepping camera altitude from `0.0` (surface storm) to `1.0` (map table) verify smooth monotonic gain cross-fades summing cleanly without clipping.

### UI-AUD-003: Skin-Matched Diegetic UI Foley Triggers
- **ID:** `UI-AUD-003`
- **Status:** `Active`
- **Milestone:** `M4`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** User actions (window drag/snap, tab switch, dispatch arrival, arbitration seal stamp, clock speed toggle) SHALL trigger skin-mapped UI foley events routed through the Web Audio mixer with cooldown throttling to prevent audio stacking spam.
- **Acceptance Criteria:** Rapidly hovering or clicking multiple controls verifies foley playback triggers respect manifest mappings and per-channel debounce windows.

### UI-AUD-004: Independent Master, Music, Ambience & Foley Mixer Controls
- **ID:** `UI-AUD-004`
- **Status:** `Active`
- **Milestone:** `M2`
- **Verification Layer:** `Window-Playwright`
- **Lineage:** `Origin`
- **Requirement:** Audio settings SHALL expose persistent Master, Music, Environmental Ambience, and UI Foley volume sliders plus individual mute toggles cleanly wired to Web Audio gain nodes.
- **Acceptance Criteria:** Adjusting sliders or muting channels inside the settings window updates Web Audio gain node values immediately and preserves audio preferences across restarts.

---

## 11. Retired & Superseded Requirements Registry

| ID | Former Title | Status | Lineage / Rationale | Date |
| :--- | :--- | :--- | :--- | :--- |
| `UI-GLB-001` | Seamless Goldberg Polyhedron Geodesic Mesh Rendering | `Superseded` | Replaced by `UI-GLB-001.1` (Option B hex-free welded spherical terrain mesh) | 2026-11-15 |
| `UI-MAP-005` | Occlusion-Aware Spherical Picking & Hover Inspection | `Superseded` | Replaced by `UI-MAP-005.1` (Option B continuous spherical coordinate picking) | 2026-11-15 |
| `UI-MAP-006` | Reported Trade-Flow, Freight Congestion & Price-Spread Overlays | `Superseded` | Decomposed into `UI-MAP-006.1`..`UI-MAP-006.2` for M5 | 2026-11-15 |
| `UI-MAP-007` | Operational Warfare Fronts, Objectives & Phase-Line Overlay | `Superseded` | Decomposed into `UI-MAP-007.1`..`UI-MAP-007.2` for M7 | 2026-11-15 |
| `UI-DAT-006` | Non-State Institution & Shared Logistics Inspector Windows | `Superseded` | Decomposed into `UI-DAT-006.1`..`UI-DAT-006.2` for M5 | 2026-11-15 |
| `UI-DAT-007` | Diegetic Archive, Workshop & Fragile Knowledge Windows | `Superseded` | Decomposed into `UI-DAT-007.1`..`UI-DAT-007.2` for M6 | 2026-11-15 |
| `UI-HUD-007` | Operational Warfare Delegation & Succession Transition Windows | `Superseded` | Decomposed into `UI-HUD-007.1`..`UI-HUD-007.2` for M7 | 2026-11-15 |
